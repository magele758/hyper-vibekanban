use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use serde::Deserialize;
use thiserror::Error;
use tokio::sync::Mutex;

#[derive(Debug, Error)]
pub enum FeishuClientError {
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("feishu api error code={code}: {msg}")]
    Api { code: i64, msg: String },
    #[error("missing tenant access token")]
    MissingToken,
}

#[derive(Clone)]
pub struct FeishuClient {
    http: reqwest::Client,
    app_id: String,
    app_secret: String,
    token: Arc<Mutex<Option<CachedToken>>>,
}

struct CachedToken {
    value: String,
    expires_at: Instant,
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    code: i64,
    msg: Option<String>,
    tenant_access_token: Option<String>,
    expire: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct ApiResponse {
    code: i64,
    msg: Option<String>,
}

impl FeishuClient {
    pub fn new(app_id: impl Into<String>, app_secret: impl Into<String>) -> Self {
        Self {
            http: reqwest::Client::new(),
            app_id: app_id.into(),
            app_secret: app_secret.into(),
            token: Arc::new(Mutex::new(None)),
        }
    }

    async fn get_tenant_access_token(&self) -> Result<String, FeishuClientError> {
        {
            let guard = self.token.lock().await;
            if let Some(cached) = guard.as_ref()
                && Instant::now() < cached.expires_at
            {
                return Ok(cached.value.clone());
            }
        }

        let resp = self
            .http
            .post("https://open.feishu.cn/open-apis/auth/v3/tenant_access_token/internal")
            .json(&serde_json::json!({
                "app_id": self.app_id,
                "app_secret": self.app_secret,
            }))
            .send()
            .await?
            .error_for_status()?
            .json::<TokenResponse>()
            .await?;

        if resp.code != 0 {
            return Err(FeishuClientError::Api {
                code: resp.code,
                msg: resp.msg.unwrap_or_default(),
            });
        }

        let token = resp
            .tenant_access_token
            .ok_or(FeishuClientError::MissingToken)?;
        let expire_secs = resp.expire.unwrap_or(7200).saturating_sub(300) as u64;

        let mut guard = self.token.lock().await;
        *guard = Some(CachedToken {
            value: token.clone(),
            expires_at: Instant::now() + Duration::from_secs(expire_secs),
        });
        Ok(token)
    }

    /// Reply to a Feishu message (preferred for group/p2p threads).
    pub async fn reply_text(&self, message_id: &str, text: &str) -> Result<(), FeishuClientError> {
        let token = self.get_tenant_access_token().await?;
        let url = format!("https://open.feishu.cn/open-apis/im/v1/messages/{message_id}/reply");
        let resp = self
            .http
            .post(&url)
            .bearer_auth(&token)
            .json(&serde_json::json!({
                "msg_type": "text",
                "content": serde_json::json!({ "text": text }).to_string(),
            }))
            .send()
            .await?
            .error_for_status()?
            .json::<ApiResponse>()
            .await?;

        if resp.code != 0 {
            return Err(FeishuClientError::Api {
                code: resp.code,
                msg: resp.msg.unwrap_or_default(),
            });
        }
        Ok(())
    }

    /// Send a text message to a chat (fallback when reply fails).
    pub async fn send_text_to_chat(
        &self,
        chat_id: &str,
        text: &str,
    ) -> Result<(), FeishuClientError> {
        let token = self.get_tenant_access_token().await?;
        let resp = self
            .http
            .post("https://open.feishu.cn/open-apis/im/v1/messages?receive_id_type=chat_id")
            .bearer_auth(&token)
            .json(&serde_json::json!({
                "receive_id": chat_id,
                "msg_type": "text",
                "content": serde_json::json!({ "text": text }).to_string(),
            }))
            .send()
            .await?
            .error_for_status()?
            .json::<ApiResponse>()
            .await?;

        if resp.code != 0 {
            return Err(FeishuClientError::Api {
                code: resp.code,
                msg: resp.msg.unwrap_or_default(),
            });
        }
        Ok(())
    }
}
