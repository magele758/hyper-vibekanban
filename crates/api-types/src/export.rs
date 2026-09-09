use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

pub const WEB_EXPORT_FORMAT: &str = "vibe-kanban-web-export";
pub const WEB_EXPORT_VERSION: u32 = 1;
pub const WEB_EXPORT_MANIFEST_FILE: &str = "vk-export.json";

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct ExportRequest {
    pub organization_id: Uuid,
    /// If empty, exports all projects in the organization.
    pub project_ids: Vec<Uuid>,
    pub include_attachments: bool,
}

/// Machine-readable project/issue dump written into the web export ZIP.
/// Desktop full + lite clients import this (or the older CSV fallback).
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
pub struct WebExportManifest {
    pub format: String,
    pub version: u32,
    pub exported_at: String,
    pub projects: Vec<WebExportProject>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
pub struct WebExportProject {
    #[serde(default)]
    pub id: Option<String>,
    pub name: String,
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default)]
    pub issues: Vec<WebExportIssue>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
pub struct WebExportIssue {
    pub simple_id: String,
    pub title: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub priority: Option<String>,
    #[serde(default)]
    pub parent_simple_id: Option<String>,
}
