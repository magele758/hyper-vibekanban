use api_types::{ListProjectsResponse, MutationResponse, Project};
use rmcp::{
    ErrorData, handler::server::wrapper::Parameters, model::CallToolResult, schemars, tool,
    tool_router,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{McpServer, ToolError};

/// Default project color. Matches the remote initial project color (`H S% L%`).
const DEFAULT_PROJECT_COLOR: &str = "217 91% 60%";

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct McpListProjectsRequest {
    #[schemars(description = "The ID of the organization to list projects from")]
    organization_id: Uuid,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct McpCreateProjectRequest {
    #[schemars(description = "The ID of the organization that will own the project")]
    organization_id: Uuid,
    #[schemars(description = "The name of the project")]
    name: String,
    #[schemars(
        description = "Optional HSL color in the form 'H S% L%', for example '217 91% 60%'. Defaults to '217 91% 60%' when omitted."
    )]
    color: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct McpUpdateProjectRequest {
    #[schemars(description = "The ID of the project to update")]
    project_id: Uuid,
    #[schemars(description = "New name for the project. Omit to leave the name unchanged.")]
    name: Option<String>,
    #[schemars(
        description = "New HSL color in the form 'H S% L%'. Omit to leave the color unchanged."
    )]
    color: Option<String>,
    #[schemars(description = "New sort order. Omit to leave the sort order unchanged.")]
    sort_order: Option<i32>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct McpListProjectStatusesRequest {
    #[schemars(
        description = "The project to list statuses for. Optional if running inside a workspace linked to a remote project."
    )]
    project_id: Option<Uuid>,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
struct ProjectSummary {
    #[schemars(description = "The unique identifier of the project")]
    id: String,
    #[schemars(description = "The organization that owns the project")]
    organization_id: String,
    #[schemars(description = "The name of the project")]
    name: String,
    #[schemars(description = "HSL color in the form 'H S% L%'")]
    color: String,
    #[schemars(description = "Sort order within the organization")]
    sort_order: i32,
    #[schemars(description = "When the project was created")]
    created_at: String,
    #[schemars(description = "When the project was last updated")]
    updated_at: String,
}

impl ProjectSummary {
    fn from_remote_project(project: Project) -> Self {
        Self {
            id: project.id.to_string(),
            organization_id: project.organization_id.to_string(),
            name: project.name,
            color: project.color,
            sort_order: project.sort_order,
            created_at: project.created_at.to_rfc3339(),
            updated_at: project.updated_at.to_rfc3339(),
        }
    }
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
struct McpListProjectsResponse {
    projects: Vec<ProjectSummary>,
    count: usize,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
struct McpProjectResponse {
    project: ProjectSummary,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
struct ProjectStatusSummary {
    #[schemars(description = "The unique identifier of the status")]
    id: String,
    #[schemars(description = "The project this status belongs to")]
    project_id: String,
    #[schemars(description = "The display name of the status")]
    name: String,
    #[schemars(description = "HSL color in the form 'H S% L%'")]
    color: String,
    #[schemars(description = "Sort order within the project")]
    sort_order: i32,
    #[schemars(description = "Whether the status is hidden from the board")]
    hidden: bool,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
struct McpListProjectStatusesResponse {
    project_id: String,
    project_statuses: Vec<ProjectStatusSummary>,
    count: usize,
}

fn normalize_project_name(name: &str) -> Result<String, ToolError> {
    let name = name.trim();
    if name.is_empty() {
        return Err(ToolError::message("Project name is required"));
    }
    Ok(name.to_string())
}

fn require_hsl_color(color: &str) -> Result<String, ToolError> {
    let color = color.trim();
    if !is_valid_hsl_color(color) {
        return Err(ToolError::message(
            "Invalid color format. Expected HSL format: 'H S% L%' (for example '217 91% 60%')",
        ));
    }
    Ok(color.to_string())
}

/// Validates HSL colors the remote API accepts: `H S% L%`.
fn is_valid_hsl_color(color: &str) -> bool {
    let parts: Vec<&str> = color.split(' ').collect();
    if parts.len() != 3 {
        return false;
    }

    let Some(hue) = parts[0].parse::<u16>().ok() else {
        return false;
    };
    if hue > 360 {
        return false;
    }

    let Some(saturation) = parts[1].strip_suffix('%') else {
        return false;
    };
    let Some(saturation) = saturation.parse::<u8>().ok() else {
        return false;
    };
    if saturation > 100 {
        return false;
    }

    let Some(lightness) = parts[2].strip_suffix('%') else {
        return false;
    };
    let Some(lightness) = lightness.parse::<u8>().ok() else {
        return false;
    };
    lightness <= 100
}

#[tool_router(router = remote_projects_tools_router, vis = "pub")]
impl McpServer {
    #[tool(description = "List all the available projects")]
    async fn list_projects(
        &self,
        Parameters(McpListProjectsRequest { organization_id }): Parameters<McpListProjectsRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        let url = self.url(&format!(
            "/api/remote/projects?organization_id={}",
            organization_id
        ));
        let response: ListProjectsResponse = match self.send_json(self.client.get(&url)).await {
            Ok(r) => r,
            Err(e) => return Ok(Self::tool_error(e)),
        };

        let project_summaries: Vec<ProjectSummary> = response
            .projects
            .into_iter()
            .map(ProjectSummary::from_remote_project)
            .collect();

        McpServer::success(&McpListProjectsResponse {
            count: project_summaries.len(),
            projects: project_summaries,
        })
    }

    #[tool(
        description = "Create a project in an organization. `color` is an optional HSL value ('H S% L%') and defaults to '217 91% 60%'."
    )]
    async fn create_project(
        &self,
        Parameters(McpCreateProjectRequest {
            organization_id,
            name,
            color,
        }): Parameters<McpCreateProjectRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        let name = match normalize_project_name(&name) {
            Ok(name) => name,
            Err(e) => return Ok(Self::tool_error(e)),
        };
        let color = match color {
            Some(color) => match require_hsl_color(&color) {
                Ok(color) => color,
                Err(e) => return Ok(Self::tool_error(e)),
            },
            None => DEFAULT_PROJECT_COLOR.to_string(),
        };

        let payload = serde_json::json!({
            "organization_id": organization_id,
            "name": name,
            "color": color,
        });
        let url = self.url("/api/remote/projects");
        let response: MutationResponse<Project> =
            match self.send_json(self.client.post(&url).json(&payload)).await {
                Ok(response) => response,
                Err(e) => return Ok(Self::tool_error(e)),
            };

        McpServer::success(&McpProjectResponse {
            project: ProjectSummary::from_remote_project(response.data),
        })
    }

    #[tool(
        description = "Update a project's name, HSL color, or sort order. `project_id` is required. Omit fields that should stay unchanged."
    )]
    async fn update_project(
        &self,
        Parameters(McpUpdateProjectRequest {
            project_id,
            name,
            color,
            sort_order,
        }): Parameters<McpUpdateProjectRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        if name.is_none() && color.is_none() && sort_order.is_none() {
            return Ok(Self::tool_error(ToolError::message(
                "Provide name, color, or sort_order to update",
            )));
        }

        let mut payload = serde_json::Map::new();
        if let Some(name) = name {
            match normalize_project_name(&name) {
                Ok(name) => {
                    payload.insert("name".to_string(), serde_json::Value::String(name));
                }
                Err(e) => return Ok(Self::tool_error(e)),
            }
        }
        if let Some(color) = color {
            match require_hsl_color(&color) {
                Ok(color) => {
                    payload.insert("color".to_string(), serde_json::Value::String(color));
                }
                Err(e) => return Ok(Self::tool_error(e)),
            }
        }
        if let Some(sort_order) = sort_order {
            payload.insert("sort_order".to_string(), serde_json::json!(sort_order));
        }

        let url = self.url(&format!("/api/remote/projects/{project_id}"));
        let response: MutationResponse<Project> = match self
            .send_json(
                self.client
                    .patch(&url)
                    .json(&serde_json::Value::Object(payload)),
            )
            .await
        {
            Ok(response) => response,
            Err(e) => return Ok(Self::tool_error(e)),
        };

        McpServer::success(&McpProjectResponse {
            project: ProjectSummary::from_remote_project(response.data),
        })
    }

    #[tool(
        description = "List statuses for a project. `project_id` is optional if running inside a workspace linked to a remote project."
    )]
    async fn list_project_statuses(
        &self,
        Parameters(McpListProjectStatusesRequest { project_id }): Parameters<
            McpListProjectStatusesRequest,
        >,
    ) -> Result<CallToolResult, ErrorData> {
        let project_id = match self.resolve_project_id(project_id) {
            Ok(project_id) => project_id,
            Err(e) => return Ok(Self::tool_error(e)),
        };

        let mut statuses = match self.fetch_project_statuses(project_id).await {
            Ok(statuses) => statuses,
            Err(e) => return Ok(Self::tool_error(e)),
        };
        statuses.sort_by(|left, right| {
            left.sort_order
                .cmp(&right.sort_order)
                .then_with(|| left.name.cmp(&right.name))
        });

        let project_statuses: Vec<ProjectStatusSummary> = statuses
            .into_iter()
            .map(|status| ProjectStatusSummary {
                id: status.id.to_string(),
                project_id: status.project_id.to_string(),
                name: status.name,
                color: status.color,
                sort_order: status.sort_order,
                hidden: status.hidden,
            })
            .collect();

        McpServer::success(&McpListProjectStatusesResponse {
            project_id: project_id.to_string(),
            count: project_statuses.len(),
            project_statuses,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::{
        DEFAULT_PROJECT_COLOR, McpServer, is_valid_hsl_color, normalize_project_name,
        require_hsl_color,
    };

    #[test]
    fn remote_project_tools_include_management() {
        let names: BTreeSet<String> = McpServer::remote_projects_tools_router()
            .list_all()
            .into_iter()
            .map(|tool| tool.name.to_string())
            .collect();

        assert_eq!(
            names,
            BTreeSet::from([
                "create_project".to_string(),
                "list_project_statuses".to_string(),
                "list_projects".to_string(),
                "update_project".to_string(),
            ])
        );
    }

    #[test]
    fn project_name_and_color_match_remote_rules() {
        assert!(normalize_project_name("  ").is_err());
        assert_eq!(normalize_project_name("  Alpha  ").unwrap(), "Alpha");
        assert!(require_hsl_color("blue").is_err());
        assert_eq!(
            require_hsl_color(" 217 91% 60% ").unwrap(),
            DEFAULT_PROJECT_COLOR
        );
        assert!(is_valid_hsl_color(DEFAULT_PROJECT_COLOR));
        assert!(!is_valid_hsl_color("361 10% 10%"));
        assert!(!is_valid_hsl_color("10 101% 10%"));
    }
}
