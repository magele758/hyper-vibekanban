use std::sync::Arc;

use sqlx::PgPool;

use super::{auth::JwtService, config::RelayServerConfig, relay_registry::RelayRegistry};

#[derive(Clone)]
pub struct RelayAppState {
    pub pool: PgPool,
    pub config: RelayServerConfig,
    pub jwt: Arc<JwtService>,
    pub relay_registry: RelayRegistry,
}

impl RelayAppState {
    pub async fn new(
        pool: PgPool,
        config: RelayServerConfig,
        jwt: Arc<JwtService>,
    ) -> Result<Self, sqlx::Error> {
        let relay_registry = RelayRegistry::default();
        relay_registry.load_existing_hosts(&pool).await?;
        let (host_count, online_count) = relay_registry.presence_counts().await;
        tracing::info!(
            host_count,
            online_count,
            "loaded existing relay hosts from database"
        );
        Ok(Self {
            pool,
            config,
            jwt,
            relay_registry,
        })
    }
}
