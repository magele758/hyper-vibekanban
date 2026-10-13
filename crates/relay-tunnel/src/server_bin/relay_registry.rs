//! Host presence shared across Relay replicas, plus this process's sockets.
//!
//! Registration, display name, and online/offline status are rows in the
//! `hosts` table. That table lives in the Postgres database Relay already
//! shares with Remote (`DATABASE_URL` / `SERVER_DATABASE_URL`). Each replica
//! loads the table when it starts ([`RelayRegistry::load_existing_hosts`]).
//! The table is the shared source of truth; call `load_existing_hosts` again
//! to refresh this process's snapshot after another replica writes.
//!
//! [`ActiveRelay`] is only the yamux control channel for a WebSocket accepted
//! by this pod. Tunnel bytes are not forwarded to a connection owned by
//! another replica.
//!
//! Load balancers must use ClientIP session affinity so one executor's
//! long-lived WebSocket, and the browser traffic for that host, stay on the
//! same Relay pod.

use std::{collections::HashMap, sync::Arc};

use relay_tunnel_core::server::SharedControl;
use sqlx::PgPool;
use tokio::sync::Mutex;
use uuid::Uuid;

use super::db::hosts::{HostRecord, HostRepository};

/// Live yamux control for a host connected to this process.
///
/// This value is not stored in Postgres and is not visible to other pods.
pub struct ActiveRelay {
    /// Open yamux streams to the connected local host.
    pub control: SharedControl,
}

impl ActiveRelay {
    pub fn new(control: SharedControl) -> Self {
        Self { control }
    }
}

/// Online/offline as stored in `hosts.status`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostPresenceStatus {
    Online,
    Offline,
}

impl HostPresenceStatus {
    fn parse(value: &str) -> Self {
        match value {
            "online" => Self::Online,
            _ => Self::Offline,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Online => "online",
            Self::Offline => "offline",
        }
    }
}

/// Host registration loaded from the shared `hosts` table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KnownHost {
    pub id: Uuid,
    pub owner_user_id: Uuid,
    pub machine_id: String,
    pub name: String,
    pub status: HostPresenceStatus,
}

impl From<HostRecord> for KnownHost {
    fn from(record: HostRecord) -> Self {
        Self {
            id: record.id,
            owner_user_id: record.owner_user_id,
            machine_id: record.machine_id,
            name: record.name,
            status: HostPresenceStatus::parse(&record.status),
        }
    }
}

/// Local control channels plus a snapshot of shared host presence.
///
/// The snapshot is filled from Postgres. The connection map starts empty on
/// every process: a host that is online in the table may still have its
/// WebSocket on a different pod.
#[derive(Default, Clone)]
pub struct RelayRegistry {
    connections: Arc<Mutex<HashMap<Uuid, Arc<ActiveRelay>>>>,
    known_hosts: Arc<Mutex<HashMap<Uuid, KnownHost>>>,
}

impl RelayRegistry {
    /// Replace the in-memory host snapshot with the current `hosts` rows.
    pub async fn load_existing_hosts(&self, pool: &PgPool) -> Result<(), sqlx::Error> {
        let records = HostRepository::new(pool).list_hosts().await?;
        let mut known = self.known_hosts.lock().await;
        known.clear();
        for record in records {
            known.insert(record.id, KnownHost::from(record));
        }
        tracing::debug!(
            host_count = known.len(),
            "refreshed relay host presence from database"
        );
        Ok(())
    }

    pub async fn known_host(&self, host_id: Uuid) -> Option<KnownHost> {
        self.known_hosts.lock().await.get(&host_id).cloned()
    }

    pub async fn known_hosts(&self) -> Vec<KnownHost> {
        let mut hosts: Vec<_> = self.known_hosts.lock().await.values().cloned().collect();
        hosts.sort_by_key(|host| host.id);
        hosts
    }

    pub async fn presence_counts(&self) -> (usize, usize) {
        let known = self.known_hosts.lock().await;
        let online = known
            .values()
            .filter(|host| host.status == HostPresenceStatus::Online)
            .count();
        (known.len(), online)
    }

    /// Insert or update the host row (name included) and refresh the snapshot.
    pub async fn register_host(
        &self,
        pool: &PgPool,
        owner_user_id: Uuid,
        machine_id: &str,
        name: &str,
        agent_version: Option<&str>,
    ) -> Result<Uuid, sqlx::Error> {
        let host_id = HostRepository::new(pool)
            .upsert_host(owner_user_id, machine_id, name, agent_version)
            .await?;
        self.load_existing_hosts(pool).await?;
        Ok(host_id)
    }

    /// Mark the host online for this connection and refresh the snapshot.
    pub async fn publish_online(
        &self,
        pool: &PgPool,
        host_id: Uuid,
        agent_version: Option<&str>,
        connection_token: Uuid,
    ) -> Result<(), sqlx::Error> {
        HostRepository::new(pool)
            .mark_host_online(host_id, agent_version, connection_token)
            .await?;
        self.load_existing_hosts(pool).await?;
        Ok(())
    }

    /// Mark the host offline when this connection still owns the row.
    ///
    /// Returns `false` when a newer connection token is stored. The snapshot
    /// is refreshed either way so this pod does not keep a stale status.
    pub async fn publish_offline(
        &self,
        pool: &PgPool,
        host_id: Uuid,
        connection_token: Uuid,
    ) -> Result<bool, sqlx::Error> {
        let updated = HostRepository::new(pool)
            .mark_host_offline(host_id, connection_token)
            .await?;
        self.load_existing_hosts(pool).await?;
        Ok(updated)
    }

    /// Register a relay for a host. Replaces any existing relay for that host
    /// on this pod only.
    pub async fn insert(&self, host_id: Uuid, relay: Arc<ActiveRelay>) {
        self.connections.lock().await.insert(host_id, relay);
    }

    /// Remove the relay for a host from this pod.
    pub async fn remove(&self, host_id: &Uuid) {
        self.connections.lock().await.remove(host_id);
    }

    /// Remove the relay for a host only when it still matches the provided relay.
    pub async fn remove_if_same(&self, host_id: &Uuid, relay: &Arc<ActiveRelay>) -> bool {
        let mut relays = self.connections.lock().await;
        if relays
            .get(host_id)
            .is_some_and(|current| Arc::ptr_eq(current, relay))
        {
            relays.remove(host_id);
            true
        } else {
            false
        }
    }

    /// Look up the active relay socket for a host on this pod.
    ///
    /// `None` means this pod does not hold the WebSocket. The host may still
    /// be online in Postgres because another pod accepted the connection.
    /// There is no cross-pod byte forwarding; ClientIP session affinity has
    /// to keep that host's long-lived connection on one Relay pod.
    pub async fn get(&self, host_id: &Uuid) -> Option<Arc<ActiveRelay>> {
        self.connections.lock().await.get(host_id).cloned()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use secrecy::SecretString;
    use sqlx::postgres::PgPoolOptions;
    use uuid::Uuid;

    use super::{
        super::{auth::JwtService, config::RelayServerConfig, state::RelayAppState},
        HostPresenceStatus, RelayRegistry,
    };

    #[tokio::test]
    async fn fresh_registry_has_no_local_connection() {
        let registry = RelayRegistry::default();
        let host_id = Uuid::new_v4();
        assert!(registry.get(&host_id).await.is_none());
        assert!(registry.known_host(host_id).await.is_none());
    }

    fn test_database_url() -> Option<String> {
        std::env::var("RELAY_TEST_DATABASE_URL")
            .ok()
            .filter(|url| !url.is_empty())
    }

    async fn insert_user(pool: &sqlx::PgPool) -> Uuid {
        let email = format!("relay-presence-{}@example.com", Uuid::new_v4());
        sqlx::query_scalar("INSERT INTO users (email) VALUES ($1) RETURNING id")
            .bind(email)
            .fetch_one(pool)
            .await
            .expect("insert user")
    }

    #[tokio::test]
    async fn second_replica_loads_shared_host_presence() {
        let Some(url) = test_database_url() else {
            eprintln!(
                "skip second_replica_loads_shared_host_presence: RELAY_TEST_DATABASE_URL is not set"
            );
            return;
        };

        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect(&url)
            .await
            .expect("connect test database");
        let user_id = insert_user(&pool).await;

        let replica_a = RelayRegistry::default();
        let host_id = replica_a
            .register_host(&pool, user_id, "machine-1", "builder", Some("0.1.0"))
            .await
            .expect("register host");
        let token_a = Uuid::new_v4();
        replica_a
            .publish_online(&pool, host_id, Some("0.1.0"), token_a)
            .await
            .expect("mark online");

        let seen_on_a = replica_a.known_host(host_id).await.expect("snapshot on A");
        assert_eq!(seen_on_a.name, "builder");
        assert_eq!(seen_on_a.status, HostPresenceStatus::Online);
        assert!(replica_a.get(&host_id).await.is_none());

        let config = RelayServerConfig {
            database_url: url.clone(),
            listen_addr: "127.0.0.1:0".to_string(),
            jwt_secret: SecretString::new("test-secret".into()),
        };
        let replica_b = RelayAppState::new(
            pool.clone(),
            config,
            Arc::new(JwtService::new(SecretString::new("test-secret".into()))),
        )
        .await
        .expect("startup load");
        let seen_on_b = replica_b
            .relay_registry
            .known_host(host_id)
            .await
            .expect("startup snapshot");
        assert_eq!(seen_on_b.name, "builder");
        assert_eq!(seen_on_b.machine_id, "machine-1");
        assert_eq!(seen_on_b.owner_user_id, user_id);
        assert_eq!(seen_on_b.status, HostPresenceStatus::Online);
        assert_eq!(seen_on_b.status.as_str(), "online");
        assert!(
            replica_b.relay_registry.get(&host_id).await.is_none(),
            "WebSocket stays on the pod that accepted it"
        );

        replica_a
            .register_host(
                &pool,
                user_id,
                "machine-1",
                "builder-renamed",
                Some("0.1.0"),
            )
            .await
            .expect("rename");
        replica_b
            .relay_registry
            .load_existing_hosts(&pool)
            .await
            .expect("refresh after rename");
        let renamed = replica_b
            .relay_registry
            .known_host(host_id)
            .await
            .expect("renamed host");
        assert_eq!(renamed.name, "builder-renamed");
        assert_eq!(renamed.status, HostPresenceStatus::Online);

        let version: Option<String> =
            sqlx::query_scalar("SELECT agent_version FROM hosts WHERE id = $1")
                .bind(host_id)
                .fetch_one(&pool)
                .await
                .expect("agent version");
        assert_eq!(version.as_deref(), Some("0.1.0"));

        let token_b = Uuid::new_v4();
        replica_b
            .relay_registry
            .publish_online(&pool, host_id, None, token_b)
            .await
            .expect("replica B session");
        let cleared = replica_a
            .publish_offline(&pool, host_id, token_a)
            .await
            .expect("stale disconnect");
        assert!(!cleared);
        assert_eq!(
            replica_a.known_host(host_id).await.unwrap().status,
            HostPresenceStatus::Online
        );
        replica_b
            .relay_registry
            .load_existing_hosts(&pool)
            .await
            .expect("refresh after stale disconnect");
        assert_eq!(
            replica_b
                .relay_registry
                .known_host(host_id)
                .await
                .unwrap()
                .status,
            HostPresenceStatus::Online
        );

        let stored_token: Option<Uuid> =
            sqlx::query_scalar("SELECT relay_connection_token FROM hosts WHERE id = $1")
                .bind(host_id)
                .fetch_one(&pool)
                .await
                .expect("token");
        assert_eq!(stored_token, Some(token_b));

        let cleared = replica_b
            .relay_registry
            .publish_offline(&pool, host_id, token_b)
            .await
            .expect("owning disconnect");
        assert!(cleared);
        replica_a
            .load_existing_hosts(&pool)
            .await
            .expect("refresh after offline");
        assert_eq!(
            replica_a.known_host(host_id).await.unwrap().status,
            HostPresenceStatus::Offline
        );
        let stored_token: Option<Uuid> =
            sqlx::query_scalar("SELECT relay_connection_token FROM hosts WHERE id = $1")
                .bind(host_id)
                .fetch_one(&pool)
                .await
                .expect("cleared token");
        assert_eq!(stored_token, None);

        sqlx::query("DELETE FROM users WHERE id = $1")
            .bind(user_id)
            .execute(&pool)
            .await
            .expect("delete user");
    }
}
