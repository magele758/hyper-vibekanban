//! Feishu (Lark) Open Platform helpers: event crypto + IM client.

mod client;
mod command;
mod crypto;

pub use client::{FeishuClient, FeishuClientError};
pub use command::{FeishuCommand, HELP_TEXT, parse_command};
pub use crypto::{decrypt_event, verify_signature};
