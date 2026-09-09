use std::{
    collections::BTreeMap,
    io::{Cursor, Read},
};

use api_types::{
    WEB_EXPORT_FORMAT, WEB_EXPORT_MANIFEST_FILE, WEB_EXPORT_VERSION, WebExportIssue,
    WebExportManifest, WebExportProject,
};
use zip::ZipArchive;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ImportParseError {
    #[error("{0}")]
    Invalid(String),
}

pub fn parse_web_export(bytes: &[u8]) -> Result<WebExportManifest, ImportParseError> {
    if looks_like_zip(bytes) {
        return parse_zip(bytes);
    }
    parse_manifest_json(bytes)
}

fn looks_like_zip(bytes: &[u8]) -> bool {
    bytes.starts_with(b"PK")
}

fn parse_manifest_json(bytes: &[u8]) -> Result<WebExportManifest, ImportParseError> {
    let manifest: WebExportManifest = serde_json::from_slice(bytes)
        .map_err(|error| ImportParseError::Invalid(format!("invalid vk-export.json: {error}")))?;
    validate_manifest(&manifest)?;
    Ok(manifest)
}

fn validate_manifest(manifest: &WebExportManifest) -> Result<(), ImportParseError> {
    if manifest.format != WEB_EXPORT_FORMAT {
        return Err(ImportParseError::Invalid(format!(
            "unsupported export format '{}'",
            manifest.format
        )));
    }
    if manifest.version == 0 || manifest.version > WEB_EXPORT_VERSION {
        return Err(ImportParseError::Invalid(format!(
            "unsupported export version {}",
            manifest.version
        )));
    }
    if manifest.projects.is_empty() {
        return Err(ImportParseError::Invalid(
            "export contains no projects".to_string(),
        ));
    }
    Ok(())
}

fn parse_zip(bytes: &[u8]) -> Result<WebExportManifest, ImportParseError> {
    let mut archive = ZipArchive::new(Cursor::new(bytes))
        .map_err(|error| ImportParseError::Invalid(format!("invalid zip: {error}")))?;

    if let Some(manifest_bytes) = read_zip_file(&mut archive, WEB_EXPORT_MANIFEST_FILE) {
        return parse_manifest_json(&manifest_bytes);
    }

    let projects_csv = read_zip_file(&mut archive, "projects.csv");
    let issues_csv = read_zip_file(&mut archive, "issues.csv").ok_or_else(|| {
        ImportParseError::Invalid("zip is missing vk-export.json and issues.csv".to_string())
    })?;

    parse_csv_export(projects_csv.as_deref(), &issues_csv)
}

fn read_zip_file(archive: &mut ZipArchive<Cursor<&[u8]>>, name: &str) -> Option<Vec<u8>> {
    let mut file = archive.by_name(name).ok()?;
    let mut buf = Vec::new();
    file.read_to_end(&mut buf).ok()?;
    Some(buf)
}

fn parse_csv_export(
    projects_csv: Option<&[u8]>,
    issues_csv: &[u8],
) -> Result<WebExportManifest, ImportParseError> {
    let mut projects_by_name: BTreeMap<String, WebExportProject> = BTreeMap::new();

    if let Some(projects_csv) = projects_csv {
        let mut reader = csv::ReaderBuilder::new()
            .flexible(true)
            .from_reader(projects_csv);
        let headers = reader
            .headers()
            .map_err(|error| ImportParseError::Invalid(format!("projects.csv: {error}")))?
            .clone();
        let name_idx = column_index(&headers, &["Name", "name"]).ok_or_else(|| {
            ImportParseError::Invalid("projects.csv is missing a Name column".to_string())
        })?;
        for record in reader.records() {
            let record = record
                .map_err(|error| ImportParseError::Invalid(format!("projects.csv: {error}")))?;
            let name = record.get(name_idx).unwrap_or("").trim();
            if name.is_empty() {
                continue;
            }
            projects_by_name
                .entry(name.to_string())
                .or_insert_with(|| WebExportProject {
                    id: None,
                    name: name.to_string(),
                    color: None,
                    issues: Vec::new(),
                });
        }
    }

    let mut reader = csv::ReaderBuilder::new()
        .flexible(true)
        .from_reader(issues_csv);
    let headers = reader
        .headers()
        .map_err(|error| ImportParseError::Invalid(format!("issues.csv: {error}")))?
        .clone();
    let simple_id_idx =
        column_index(&headers, &["Issue ID", "issue_id", "id"]).ok_or_else(|| {
            ImportParseError::Invalid("issues.csv is missing an Issue ID column".to_string())
        })?;
    let title_idx = column_index(&headers, &["Title", "title"]).ok_or_else(|| {
        ImportParseError::Invalid("issues.csv is missing a Title column".to_string())
    })?;
    let description_idx = column_index(&headers, &["Description", "description"]);
    let status_idx = column_index(&headers, &["Status", "status"]);
    let priority_idx = column_index(&headers, &["Priority", "priority"]);
    let project_idx = column_index(&headers, &["Project", "project"]);
    let parent_idx = column_index(&headers, &["Parent Issue", "parent_issue", "parent"]);

    for record in reader.records() {
        let record =
            record.map_err(|error| ImportParseError::Invalid(format!("issues.csv: {error}")))?;
        let title = record.get(title_idx).unwrap_or("").trim();
        if title.is_empty() {
            continue;
        }
        let project_name = project_idx
            .and_then(|idx| record.get(idx))
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .unwrap_or("Imported")
            .to_string();
        let issue = WebExportIssue {
            simple_id: record
                .get(simple_id_idx)
                .map(str::trim)
                .filter(|id| !id.is_empty())
                .unwrap_or(title)
                .to_string(),
            title: title.to_string(),
            description: optional_csv_field(record.get(description_idx.unwrap_or(usize::MAX))),
            status: optional_csv_field(record.get(status_idx.unwrap_or(usize::MAX))),
            priority: optional_csv_field(record.get(priority_idx.unwrap_or(usize::MAX))),
            parent_simple_id: optional_csv_field(record.get(parent_idx.unwrap_or(usize::MAX))),
        };
        projects_by_name
            .entry(project_name.clone())
            .or_insert_with(|| WebExportProject {
                id: None,
                name: project_name,
                color: None,
                issues: Vec::new(),
            })
            .issues
            .push(issue);
    }

    let projects: Vec<WebExportProject> = projects_by_name.into_values().collect();
    if projects.is_empty() {
        return Err(ImportParseError::Invalid(
            "export contains no projects".to_string(),
        ));
    }

    Ok(WebExportManifest {
        format: WEB_EXPORT_FORMAT.to_string(),
        version: WEB_EXPORT_VERSION,
        exported_at: String::new(),
        projects,
    })
}

fn column_index(headers: &csv::StringRecord, aliases: &[&str]) -> Option<usize> {
    headers.iter().position(|header| {
        aliases
            .iter()
            .any(|alias| header.trim().eq_ignore_ascii_case(alias))
    })
}

fn optional_csv_field(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

pub fn map_issue_status(status: Option<&str>) -> db::models::task::TaskStatus {
    use db::models::task::TaskStatus;
    match status.map(|s| s.trim().to_ascii_lowercase()).as_deref() {
        Some("in progress" | "in_progress" | "inprogress") => TaskStatus::InProgress,
        Some("in review" | "in_review" | "inreview") => TaskStatus::InReview,
        Some("done" | "completed" | "complete") => TaskStatus::Done,
        Some("cancelled" | "canceled") => TaskStatus::Cancelled,
        _ => TaskStatus::Todo,
    }
}

#[cfg(test)]
mod tests {
    use std::io::{Cursor, Write};

    use zip::{ZipWriter, write::SimpleFileOptions};

    use super::*;

    fn zip_with_files(files: &[(&str, &str)]) -> Vec<u8> {
        let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
        let options = SimpleFileOptions::default();
        for (name, body) in files {
            zip.start_file(*name, options).unwrap();
            zip.write_all(body.as_bytes()).unwrap();
        }
        zip.finish().unwrap().into_inner()
    }

    #[test]
    fn parses_manifest_json() {
        let manifest = r#"{
            "format": "vibe-kanban-web-export",
            "version": 1,
            "exported_at": "2026-09-09T00:00:00Z",
            "projects": [{
                "id": "11111111-1111-1111-1111-111111111111",
                "name": "Alpha",
                "issues": [{
                    "simple_id": "VK-1",
                    "title": "First issue",
                    "description": "Hello",
                    "status": "In progress"
                }]
            }]
        }"#;
        let parsed = parse_web_export(manifest.as_bytes()).unwrap();
        assert_eq!(parsed.projects[0].name, "Alpha");
        assert_eq!(parsed.projects[0].issues[0].simple_id, "VK-1");
    }

    #[test]
    fn parses_legacy_csv_zip() {
        let projects = "Name,Created,Updated\nAlpha,2026-01-01,2026-01-02\n";
        let issues = "Issue ID,Title,Description,Status,Priority,Project,Assignee(s),Creator,Created,Updated,Start Date,Due Date,Completed,Parent Issue\nVK-1,First issue,Hello,In progress,High,Alpha,,,,,,,\n";
        let bytes = zip_with_files(&[("projects.csv", projects), ("issues.csv", issues)]);
        let parsed = parse_web_export(&bytes).unwrap();
        assert_eq!(parsed.projects.len(), 1);
        assert_eq!(parsed.projects[0].name, "Alpha");
        assert_eq!(parsed.projects[0].issues[0].title, "First issue");
        assert_eq!(
            parsed.projects[0].issues[0].status.as_deref(),
            Some("In progress")
        );
    }

    #[test]
    fn prefers_manifest_over_csv() {
        let manifest = r#"{
            "format": "vibe-kanban-web-export",
            "version": 1,
            "exported_at": "2026-09-09T00:00:00Z",
            "projects": [{"name": "FromJson", "issues": []}]
        }"#;
        let bytes = zip_with_files(&[
            (WEB_EXPORT_MANIFEST_FILE, manifest),
            (
                "issues.csv",
                "Issue ID,Title,Project\nVK-9,Csv,CsvProject\n",
            ),
        ]);
        let parsed = parse_web_export(&bytes).unwrap();
        assert_eq!(parsed.projects[0].name, "FromJson");
    }

    #[test]
    fn maps_status_names() {
        assert_eq!(
            map_issue_status(Some("In progress")),
            db::models::task::TaskStatus::InProgress
        );
        assert_eq!(
            map_issue_status(Some("Backlog")),
            db::models::task::TaskStatus::Todo
        );
        assert_eq!(
            map_issue_status(Some("Done")),
            db::models::task::TaskStatus::Done
        );
    }

    #[test]
    fn rejects_empty_export() {
        let err = parse_web_export(
            br#"{"format":"vibe-kanban-web-export","version":1,"exported_at":"","projects":[]}"#,
        )
        .unwrap_err();
        assert!(err.to_string().contains("no projects"));
    }
}
