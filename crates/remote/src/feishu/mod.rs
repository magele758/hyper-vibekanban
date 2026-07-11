//! Feishu (Lark) Open Platform helpers: event crypto + IM client.

mod client;
mod crypto;

pub use client::{FeishuClient, FeishuClientError};
pub use crypto::{decrypt_event, verify_signature};
