//! `hyper-vibekanban join`: log in to the data service, register this machine
//! with Relay, and store the SPAKE2 client where the data service can read it.

use std::time::Duration;

use anyhow::Context as _;
use base64::{
    Engine as _,
    engine::general_purpose::{STANDARD as BASE64_STANDARD, URL_SAFE_NO_PAD},
};
use deployment::{Deployment as _, RemoteClientNotConfigured};
use ed25519_dalek::SigningKey;
use rand::rngs::OsRng;
use relay_types::{
    FinishSpake2EnrollmentRequest, FinishSpake2EnrollmentResponse, RelayHost,
    StartSpake2EnrollmentRequest, StartSpake2EnrollmentResponse,
};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use services::services::{
    config::save_config_to_file,
    remote_client::{RemoteClient, RemoteClientError},
};
use spake2::{Ed25519Group, Identity, Password, Spake2, SysRng, UnwrapErr};
use tokio_util::sync::CancellationToken;
use trusted_key_auth::{
    key_confirmation::{build_client_proof, verify_server_proof},
    spake2::normalize_enrollment_code,
    trusted_keys::parse_public_key_base64,
};
use utils::assets::config_path;
use uuid::Uuid;

use crate::DeploymentImpl;

const SPAKE2_CLIENT_ID: &[u8] = b"vibe-kanban-browser";
const SPAKE2_SERVER_ID: &[u8] = b"vibe-kanban-server";

enum LinkOutcome {
    Saved { host_name: String, host_id: Uuid },
    ApiMissing { host_name: String, host_id: Uuid },
    Retry(String),
}

struct LinkedClient {
    client_id: Uuid,
    client_name: String,
    public_key_b64: String,
    private_key_jwk: serde_json::Value,
    server_public_key_b64: String,
    signing_session_id: Uuid,
}

#[derive(Debug, Serialize)]
struct UpsertHostLinkRequest {
    host_name: String,
    client_id: Uuid,
    client_name: String,
    public_key_b64: String,
    private_key_jwk: serde_json::Value,
    server_public_key_b64: String,
    signing_session_id: Uuid,
}

#[derive(Debug, Deserialize)]
struct ApiEnvelope<T> {
    success: bool,
    data: Option<T>,
    message: Option<String>,
}

#[derive(Debug, Deserialize)]
struct EnrollmentCodeBody {
    enrollment_code: String,
}

pub async fn prepare_join_session(deployment: &DeploymentImpl) -> anyhow::Result<()> {
    let email = std::env::var("VK_LOCAL_LOGIN_EMAIL")
        .context("VK_LOCAL_LOGIN_EMAIL is required")?
        .trim()
        .to_string();
    let password =
        std::env::var("VK_LOCAL_LOGIN_PASSWORD").context("VK_LOCAL_LOGIN_PASSWORD is required")?;
    let nickname = std::env::var("VK_HOST_NICKNAME")
        .context("VK_HOST_NICKNAME is required")?
        .trim()
        .to_string();
    if email.is_empty() || password.is_empty() {
        anyhow::bail!("VK_LOCAL_LOGIN_EMAIL and VK_LOCAL_LOGIN_PASSWORD must be non-empty");
    }
    if nickname.is_empty() {
        anyhow::bail!("VK_HOST_NICKNAME must be non-empty");
    }

    let remote_client = deployment
        .remote_client()
        .map_err(|_: RemoteClientNotConfigured| {
            anyhow::anyhow!("VK_SHARED_API_BASE is not configured")
        })?;
    remote_client
        .login_with_password(&email, &password)
        .await
        .context("failed to log in to the data service")?;
    eprintln!("join: logged in as {email}");

    {
        let mut config = deployment.config().write().await;
        config.host_nickname = Some(nickname.clone());
        config.relay_enabled = true;
        let snapshot = config.clone();
        drop(config);
        save_config_to_file(&snapshot, &config_path())
            .await
            .context("failed to save host nickname")?;
    }
    eprintln!("join: registering host \"{nickname}\"");
    Ok(())
}

pub async fn maintain_host_link(
    deployment: DeploymentImpl,
    port: u16,
    shutdown: CancellationToken,
) {
    loop {
        if shutdown.is_cancelled() {
            return;
        }
        match link_once(&deployment, port, &shutdown).await {
            LinkOutcome::Saved { host_name, host_id } => {
                eprintln!(
                    "Joined host \"{host_name}\" ({host_id}). Keep this process running so the host stays online."
                );
                return;
            }
            LinkOutcome::ApiMissing { host_name, host_id } => {
                eprintln!(
                    "Host \"{host_name}\" ({host_id}) is registered with Relay, but this data service does not store host links yet. The host can go online after that service is updated. Keeping the relay connection."
                );
                return;
            }
            LinkOutcome::Retry(message) => {
                eprintln!("join: {message}");
                tokio::select! {
                    _ = shutdown.cancelled() => return,
                    _ = tokio::time::sleep(Duration::from_secs(10)) => {}
                }
            }
        }
    }
}

async fn link_once(
    deployment: &DeploymentImpl,
    port: u16,
    shutdown: &CancellationToken,
) -> LinkOutcome {
    let remote_client = match deployment.remote_client() {
        Ok(client) => client,
        Err(_) => return LinkOutcome::Retry("VK_SHARED_API_BASE is not configured".to_string()),
    };
    let machine_id = deployment.user_id().to_string();
    let host = match wait_for_host(&remote_client, &machine_id, shutdown).await {
        Ok(host) => host,
        Err(error) => return LinkOutcome::Retry(error),
    };
    let linked = match enroll_local_client(port, &host.name).await {
        Ok(linked) => linked,
        Err(error) => return LinkOutcome::Retry(error.to_string()),
    };
    match upload_host_link(&remote_client, &host, linked).await {
        Ok(()) => LinkOutcome::Saved {
            host_name: host.name,
            host_id: host.id,
        },
        Err(UploadError::MissingApi) => LinkOutcome::ApiMissing {
            host_name: host.name,
            host_id: host.id,
        },
        Err(UploadError::Other(message)) => LinkOutcome::Retry(message),
    }
}

async fn wait_for_host(
    client: &RemoteClient,
    machine_id: &str,
    shutdown: &CancellationToken,
) -> Result<RelayHost, String> {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
    loop {
        if shutdown.is_cancelled() {
            return Err("shutdown while waiting for relay registration".to_string());
        }
        match client.list_relay_hosts().await {
            Ok(hosts) => {
                if let Some(host) = hosts.into_iter().find(|host| host.machine_id == machine_id) {
                    return Ok(host);
                }
            }
            Err(error) => {
                tracing::debug!(%error, "host list not ready");
            }
        }
        if tokio::time::Instant::now() >= deadline {
            return Err(
                "timed out waiting for this machine to appear in the data service host list"
                    .to_string(),
            );
        }
        tokio::select! {
            _ = shutdown.cancelled() => {
                return Err("shutdown while waiting for relay registration".to_string());
            }
            _ = tokio::time::sleep(Duration::from_secs(1)) => {}
        }
    }
}

enum UploadError {
    MissingApi,
    Other(String),
}

async fn upload_host_link(
    client: &RemoteClient,
    host: &RelayHost,
    linked: LinkedClient,
) -> Result<(), UploadError> {
    let path = format!("/v1/hosts/{}/link", host.id);
    let body = UpsertHostLinkRequest {
        host_name: host.name.clone(),
        client_id: linked.client_id,
        client_name: linked.client_name,
        public_key_b64: linked.public_key_b64,
        private_key_jwk: linked.private_key_jwk,
        server_public_key_b64: linked.server_public_key_b64,
        signing_session_id: linked.signing_session_id,
    };
    match client
        .post_authed::<serde_json::Value, _>(&path, Some(&body))
        .await
    {
        Ok(_) => Ok(()),
        Err(RemoteClientError::Http { status: 404, .. }) => Err(UploadError::MissingApi),
        Err(error) => Err(UploadError::Other(format!(
            "failed to store the host link: {error}"
        ))),
    }
}

async fn enroll_local_client(port: u16, host_name: &str) -> anyhow::Result<LinkedClient> {
    let http = Client::builder()
        .timeout(Duration::from_secs(20))
        .build()
        .context("failed to build local HTTP client")?;
    let base = format!("http://127.0.0.1:{port}/api/relay-auth/server");

    let enrollment: EnrollmentCodeBody = post_local(
        &http,
        &format!("{base}/enrollment-code"),
        &serde_json::json!({}),
    )
    .await?;
    let normalized_code = normalize_enrollment_code(&enrollment.enrollment_code)
        .map_err(|error| anyhow::anyhow!(error.to_string()))?;

    let password = Password::new(normalized_code.as_bytes());
    let (client_state, client_message) = Spake2::<Ed25519Group>::start_a_with_rng(
        &password,
        &Identity::new(SPAKE2_CLIENT_ID),
        &Identity::new(SPAKE2_SERVER_ID),
        UnwrapErr(SysRng),
    );
    let start: StartSpake2EnrollmentResponse = post_local(
        &http,
        &format!("{base}/spake2/start"),
        &StartSpake2EnrollmentRequest {
            enrollment_code: normalized_code,
            client_message_b64: BASE64_STANDARD.encode(client_message),
        },
    )
    .await?;

    let server_message = BASE64_STANDARD
        .decode(&start.server_message_b64)
        .context("invalid server_message_b64")?;
    let shared_key = client_state
        .finish(&server_message)
        .map_err(|_| anyhow::anyhow!("failed to complete relay PAKE handshake"))?;

    let signing_key = SigningKey::generate(&mut OsRng);
    let verifying_key = signing_key.verifying_key();
    let public_key_b64 = BASE64_STANDARD.encode(verifying_key.as_bytes());
    let client_proof_b64 =
        build_client_proof(&shared_key, &start.enrollment_id, verifying_key.as_bytes())
            .map_err(|_| anyhow::anyhow!("failed to build relay PAKE client proof"))?;

    let os = os_info::get();
    let client_id = Uuid::new_v4();
    let client_name = format!("hyper-vibekanban join ({host_name})");
    let finish: FinishSpake2EnrollmentResponse = post_local(
        &http,
        &format!("{base}/spake2/finish"),
        &FinishSpake2EnrollmentRequest {
            enrollment_id: start.enrollment_id,
            client_id,
            client_name: client_name.clone(),
            client_browser: "cli".to_string(),
            client_os: format!("{} {}", os.os_type(), os.version()),
            client_device: "cli".to_string(),
            public_key_b64: public_key_b64.clone(),
            client_proof_b64,
        },
    )
    .await?;

    let server_public_key = parse_public_key_base64(&finish.server_public_key_b64)
        .map_err(|_| anyhow::anyhow!("invalid server_public_key_b64"))?;
    verify_server_proof(
        &shared_key,
        &start.enrollment_id,
        verifying_key.as_bytes(),
        server_public_key.as_bytes(),
        &finish.server_proof_b64,
    )
    .map_err(|_| anyhow::anyhow!("relay server proof verification failed"))?;

    Ok(LinkedClient {
        client_id,
        client_name,
        public_key_b64,
        private_key_jwk: serde_json::json!({
            "kty": "OKP",
            "crv": "Ed25519",
            "d": URL_SAFE_NO_PAD.encode(signing_key.to_bytes()),
            "x": URL_SAFE_NO_PAD.encode(verifying_key.as_bytes()),
        }),
        server_public_key_b64: finish.server_public_key_b64,
        signing_session_id: finish.signing_session_id,
    })
}

async fn post_local<T, B>(http: &Client, url: &str, body: &B) -> anyhow::Result<T>
where
    T: for<'de> Deserialize<'de>,
    B: Serialize,
{
    let response = http
        .post(url)
        .json(body)
        .send()
        .await
        .with_context(|| format!("request to {url} failed"))?;
    let status = response.status();
    let bytes = response
        .bytes()
        .await
        .with_context(|| format!("reading {url} failed"))?;
    if !status.is_success() {
        let text = String::from_utf8_lossy(&bytes);
        let text = text.chars().take(300).collect::<String>();
        anyhow::bail!("{url} returned {status}: {text}");
    }
    let envelope: ApiEnvelope<T> =
        serde_json::from_slice(&bytes).with_context(|| format!("invalid JSON from {url}"))?;
    if !envelope.success {
        anyhow::bail!(
            "{url} failed: {}",
            envelope
                .message
                .unwrap_or_else(|| "unknown error".to_string())
        );
    }
    envelope
        .data
        .context(format!("{url} returned an empty payload"))
}

#[cfg(test)]
mod tests {
    use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
    use ed25519_dalek::SigningKey;
    use rand::rngs::OsRng;

    #[test]
    fn ed25519_jwk_uses_unpadded_base64url() {
        let signing_key = SigningKey::generate(&mut OsRng);
        let encoded = URL_SAFE_NO_PAD.encode(signing_key.to_bytes());
        assert!(!encoded.contains('+'));
        assert!(!encoded.contains('/'));
        assert!(!encoded.contains('='));
        assert_eq!(encoded.len(), 43);
    }
}
