use std::collections::HashMap;

use db::models::requests::{
    CreateAndStartWorkspaceRequest, CreateAndStartWorkspaceResponse, LinkedIssueInfo,
    WorkspaceRepoInput,
};
use executors::profile::ExecutorConfig;
use rmcp::{
    ErrorData, handler::server::wrapper::Parameters, model::CallToolResult, schemars, tool,
    tool_router,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{McpServer, ToolError};
use crate::ApiResponseEnvelope;

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct StartWorkspaceOnHostRequest {
    #[schemars(
        description = "Project to create the workspace for. Accepts a local project id on the target host, or the remote project id linked from that host."
    )]
    project_id: Uuid,
    #[schemars(
        description = "Optional issue to link. When set, the workspace prompt defaults to the issue title and description."
    )]
    issue_id: Option<Uuid>,
    #[schemars(
        description = "Paired host id from list_hosts. The workspace is created on this host only when it is online."
    )]
    host_id: Uuid,
    #[schemars(
        description = "Optional prompt for the first session. Defaults to the issue text, or a short prompt naming the project."
    )]
    prompt: Option<String>,
    #[schemars(
        description = "Optional coding agent executor ('CLAUDE_CODE', 'AMP', 'GEMINI', 'CODEX', 'OPENCODE', 'CURSOR_AGENT', 'QWEN_CODE', 'COPILOT', 'DROID', 'PI', 'GROK'). Defaults to CODEX."
    )]
    executor: Option<String>,
    #[schemars(description = "Optional executor variant")]
    variant: Option<String>,
    #[schemars(
        description = "Optional display name. Defaults to the issue title or the project name on the host."
    )]
    name: Option<String>,
    #[schemars(
        description = "Optional repositories that already exist on the target host. When omitted, repositories are taken from that host's project."
    )]
    repositories: Option<Vec<HostWorkspaceRepoInput>>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct HostWorkspaceRepoInput {
    #[schemars(description = "Repository ID as registered on the target host")]
    repo_id: Uuid,
    #[schemars(description = "Target branch for this repository")]
    branch: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
struct HostSummary {
    #[schemars(description = "Paired host id")]
    id: String,
    #[schemars(description = "Host display name")]
    name: String,
    #[schemars(description = "online when the relay host is connected, otherwise offline")]
    status: String,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
struct ListHostsResponse {
    hosts: Vec<HostSummary>,
    count: usize,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
struct StartWorkspaceOnHostResponse {
    #[schemars(description = "Workspace created on the target host")]
    workspace_id: String,
    #[schemars(description = "Host the workspace was created on")]
    host_id: String,
    #[schemars(description = "Project id passed to the tool")]
    project_id: String,
    #[schemars(description = "Linked issue id, when one was requested")]
    issue_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PairedHostBody {
    host_id: Uuid,
    host_name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PairedHostList {
    hosts: Vec<PairedHostBody>,
}

#[derive(Debug, Deserialize)]
struct SharedApiInfo {
    shared_api_base: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AccessTokenBody {
    access_token: String,
}

#[derive(Debug, Deserialize)]
struct RemoteRelayHost {
    id: Uuid,
    name: String,
    status: String,
}

#[derive(Debug, Deserialize)]
struct RemoteRelayHostList {
    hosts: Vec<RemoteRelayHost>,
}

#[derive(Debug, Deserialize)]
struct HostProject {
    id: Uuid,
    name: String,
    remote_project_id: Option<Uuid>,
    repos: Vec<HostRepo>,
}

#[derive(Debug, Deserialize)]
struct HostRepo {
    id: Uuid,
    name: String,
    default_target_branch: Option<String>,
    is_git: bool,
}

#[derive(Debug, Deserialize)]
struct HostScratch {
    payload: HostScratchPayload,
}

#[derive(Debug, Deserialize)]
struct HostScratchPayload {
    #[serde(rename = "type")]
    kind: String,
    data: HostScratchRepos,
}

#[derive(Debug, Deserialize)]
struct HostScratchRepos {
    repos: Vec<HostScratchRepo>,
}

#[derive(Debug, Deserialize)]
struct HostScratchRepo {
    repo_id: Uuid,
    target_branch: String,
}

struct ResolvedHostProject {
    local_project_id: Option<Uuid>,
    remote_project_id: Option<Uuid>,
    name: String,
    repos: Vec<WorkspaceRepoInput>,
    non_git_repo_names: Vec<String>,
}

fn host_api_path(host_id: Uuid, tail: &str) -> String {
    format!("/api/host/{host_id}/{}", tail.trim_start_matches('/'))
}

fn target_branch(value: Option<&str>) -> String {
    value
        .map(str::trim)
        .filter(|branch| !branch.is_empty())
        .unwrap_or("main")
        .to_string()
}

fn host_display_name(
    remote_name: Option<&str>,
    paired_name: Option<&str>,
    host_id: Uuid,
) -> String {
    if let Some(name) = remote_name.map(str::trim).filter(|name| !name.is_empty()) {
        return name.to_string();
    }
    if let Some(name) = paired_name.map(str::trim).filter(|name| !name.is_empty()) {
        return name.to_string();
    }
    host_id.to_string()
}

fn merge_paired_hosts(paired: &[PairedHostBody], remote: &[RemoteRelayHost]) -> Vec<HostSummary> {
    let remote_by_id: HashMap<Uuid, &RemoteRelayHost> =
        remote.iter().map(|host| (host.id, host)).collect();
    let mut hosts: Vec<HostSummary> = paired
        .iter()
        .map(|host| {
            let remote_host = remote_by_id.get(&host.host_id).copied();
            let status = if remote_host.is_some_and(|host| host.status == "online") {
                "online"
            } else {
                "offline"
            };
            HostSummary {
                id: host.host_id.to_string(),
                name: host_display_name(
                    remote_host.map(|host| host.name.as_str()),
                    host.host_name.as_deref(),
                    host.host_id,
                ),
                status: status.to_string(),
            }
        })
        .collect();
    hosts.sort_by(|left, right| left.name.cmp(&right.name).then(left.id.cmp(&right.id)));
    hosts
}

fn require_online_host(hosts: &[HostSummary], host_id: Uuid) -> Result<(), String> {
    let host_key = host_id.to_string();
    let Some(host) = hosts.iter().find(|host| host.id == host_key) else {
        return Err(format!(
            "Host {host_id} is not paired. Pair it before creating a workspace on it."
        ));
    };
    if host.status != "online" {
        return Err(format!(
            "Host '{}' ({host_id}) is offline. Refusing to create a workspace on this machine.",
            host.name
        ));
    }
    Ok(())
}

fn find_host_project(projects: &[HostProject], project_id: Uuid) -> Option<&HostProject> {
    projects
        .iter()
        .find(|project| project.id == project_id)
        .or_else(|| {
            projects
                .iter()
                .find(|project| project.remote_project_id == Some(project_id))
        })
}

fn git_repos_from_host(repos: &[HostRepo]) -> (Vec<WorkspaceRepoInput>, Vec<String>) {
    let mut git_repos = Vec::new();
    let mut non_git = Vec::new();
    for repo in repos {
        if repo.is_git {
            git_repos.push(WorkspaceRepoInput {
                repo_id: repo.id,
                target_branch: target_branch(repo.default_target_branch.as_deref()),
            });
        } else {
            non_git.push(repo.name.clone());
        }
    }
    (git_repos, non_git)
}

fn explicit_host_repos(
    repos: Option<Vec<HostWorkspaceRepoInput>>,
) -> Option<Vec<WorkspaceRepoInput>> {
    let repos = repos.filter(|repos| !repos.is_empty())?;
    Some(
        repos
            .into_iter()
            .map(|repo| WorkspaceRepoInput {
                repo_id: repo.repo_id,
                target_branch: target_branch(Some(repo.branch.as_str())),
            })
            .collect(),
    )
}

fn issue_belongs_to_requested_project(
    issue_project_id: Uuid,
    requested_project_id: Uuid,
    host_remote_project_id: Option<Uuid>,
) -> bool {
    issue_project_id == requested_project_id || host_remote_project_id == Some(issue_project_id)
}

fn prompt_from_issue(title: &str, description: Option<&str>) -> Option<String> {
    let title = title.trim();
    let description = description
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("");
    if title.is_empty() && description.is_empty() {
        return None;
    }
    if description.is_empty() {
        return Some(title.to_string());
    }
    if title.is_empty() {
        return Some(description.to_string());
    }
    Some(format!("{title}\n\n{description}"))
}

fn workspace_prompt(
    explicit: Option<&str>,
    issue_title: Option<&str>,
    issue_description: Option<&str>,
    project_name: &str,
) -> String {
    if let Some(prompt) = explicit.map(str::trim).filter(|prompt| !prompt.is_empty()) {
        return prompt.to_string();
    }
    if let Some(title) = issue_title
        && let Some(prompt) = prompt_from_issue(title, issue_description)
    {
        return prompt;
    }
    let name = project_name.trim();
    if name.is_empty() {
        "Work on this project.".to_string()
    } else {
        format!("Work on project \"{name}\".")
    }
}

fn workspace_name(explicit: Option<&str>, issue_title: Option<&str>, project_name: &str) -> String {
    if let Some(name) = explicit
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .or_else(|| issue_title.map(str::trim).filter(|title| !title.is_empty()))
    {
        return name.to_string();
    }
    let name = project_name.trim();
    if name.is_empty() {
        "workspace".to_string()
    } else {
        name.to_string()
    }
}

fn scratch_repos(scratch: &HostScratch) -> Vec<WorkspaceRepoInput> {
    if scratch.payload.kind != "PROJECT_REPO_DEFAULTS" {
        return Vec::new();
    }
    scratch
        .payload
        .data
        .repos
        .iter()
        .map(|repo| WorkspaceRepoInput {
            repo_id: repo.repo_id,
            target_branch: target_branch(Some(repo.target_branch.as_str())),
        })
        .collect()
}

#[tool_router(router = hosts_tools_router, vis = "pub")]
impl McpServer {
    #[tool(
        description = "List paired relay hosts with name, id, and online/offline status. Call this before start_workspace_on_host."
    )]
    async fn list_hosts(&self) -> Result<CallToolResult, ErrorData> {
        let hosts = match self.load_paired_hosts().await {
            Ok(hosts) => hosts,
            Err(error) => return Ok(Self::tool_error(error)),
        };
        McpServer::success(&ListHostsResponse {
            count: hosts.len(),
            hosts,
        })
    }

    #[tool(
        description = "Create and start a workspace on a paired online host. Requires project_id and host_id. issue_id is optional. If the host is offline or not paired, returns an error and does not create a workspace on this machine."
    )]
    async fn start_workspace_on_host(
        &self,
        Parameters(request): Parameters<StartWorkspaceOnHostRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        let executor_name = match Self::normalize_executor_name(request.executor.as_deref()) {
            Ok(executor) => executor,
            Err(error) => return Ok(Self::tool_error(error)),
        };
        let base_executor = match Self::parse_executor_agent(&executor_name) {
            Ok(executor) => executor,
            Err(error) => return Ok(Self::tool_error(error)),
        };
        let variant = request.variant.and_then(|variant| {
            let trimmed = variant.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        });

        let hosts = match self.load_paired_hosts().await {
            Ok(hosts) => hosts,
            Err(error) => return Ok(Self::tool_error(error)),
        };
        if let Err(message) = require_online_host(&hosts, request.host_id) {
            return Self::err(message, None::<String>);
        }

        let issue = if let Some(issue_id) = request.issue_id {
            match self.fetch_issue(issue_id).await {
                Ok(issue) => Some(issue),
                Err(error) => return Ok(Self::tool_error(error)),
            }
        } else {
            None
        };

        let resolved = match self
            .resolve_host_project(request.host_id, request.project_id, request.repositories)
            .await
        {
            Ok(resolved) => resolved,
            Err(error) => return Ok(Self::tool_error(error)),
        };

        if let Some(issue) = &issue
            && !issue_belongs_to_requested_project(
                issue.project_id,
                request.project_id,
                resolved.remote_project_id,
            )
        {
            return Self::err(
                format!(
                    "Issue {} does not belong to project {}.",
                    issue.id, request.project_id
                ),
                None::<String>,
            );
        }

        if resolved.repos.is_empty() {
            let found = if resolved.non_git_repo_names.is_empty() {
                "none".to_string()
            } else {
                resolved.non_git_repo_names.join(", ")
            };
            return Self::err(
                format!(
                    "Project '{}' on host {} has no git repositories to create a workspace (found: {found}). Attach a git repository on that host.",
                    resolved.name, request.host_id
                ),
                None::<String>,
            );
        }

        let prompt = workspace_prompt(
            request.prompt.as_deref(),
            issue.as_ref().map(|issue| issue.title.as_str()),
            issue
                .as_ref()
                .and_then(|issue| issue.description.as_deref()),
            &resolved.name,
        );
        let name = workspace_name(
            request.name.as_deref(),
            issue.as_ref().map(|issue| issue.title.as_str()),
            &resolved.name,
        );
        let linked_issue = issue.as_ref().map(|issue| LinkedIssueInfo {
            remote_project_id: issue.project_id,
            issue_id: issue.id,
        });
        let mut executor_config = ExecutorConfig::new(base_executor);
        executor_config.variant = variant;

        // Only the host-scoped route. /api/workspaces/start would create the
        // workspace on this machine, including when an offline host id would
        // otherwise loop back through the local relay proxy.
        let start_path = host_api_path(request.host_id, "workspaces/start");
        debug_assert_ne!(start_path, "/api/workspaces/start");
        let started: CreateAndStartWorkspaceResponse = match self
            .send_json(self.client.post(self.url(&start_path)).json(
                &CreateAndStartWorkspaceRequest {
                    name: Some(name),
                    repos: resolved.repos,
                    linked_issue,
                    executor_config,
                    prompt,
                    attachment_ids: None,
                    kind: Default::default(),
                    project_id: resolved.local_project_id,
                },
            ))
            .await
        {
            Ok(response) => response,
            Err(error) => return Ok(Self::tool_error(error)),
        };

        McpServer::success(&StartWorkspaceOnHostResponse {
            workspace_id: started.workspace.id.to_string(),
            host_id: request.host_id.to_string(),
            project_id: request.project_id.to_string(),
            issue_id: request.issue_id.map(|issue_id| issue_id.to_string()),
        })
    }
}

impl McpServer {
    async fn load_paired_hosts(&self) -> Result<Vec<HostSummary>, ToolError> {
        let paired_url = self.url("/api/relay-auth/client/hosts");
        let paired: PairedHostList = self.send_json(self.client.get(&paired_url)).await?;
        let remote = self.fetch_remote_relay_hosts().await?;
        Ok(merge_paired_hosts(&paired.hosts, &remote))
    }

    async fn fetch_remote_relay_hosts(&self) -> Result<Vec<RemoteRelayHost>, ToolError> {
        let info_url = self.url("/api/info");
        let info: SharedApiInfo = self.send_json(self.client.get(&info_url)).await?;
        let Some(base) = info
            .shared_api_base
            .as_deref()
            .map(str::trim)
            .filter(|base| !base.is_empty())
        else {
            return Err(ToolError::message(
                "Remote API base is not configured, so host online/offline status is unavailable.",
            ));
        };

        let token_url = self.url("/api/auth/token");
        let token: AccessTokenBody = self.send_json(self.client.get(&token_url)).await?;
        let hosts_url = format!("{}/v1/hosts", base.trim_end_matches('/'));
        let response = self
            .client
            .get(&hosts_url)
            .bearer_auth(token.access_token.trim())
            .send()
            .await
            .map_err(|error| {
                ToolError::new("Failed to list relay hosts", Some(error.to_string()))
            })?;
        if !response.status().is_success() {
            return Err(ToolError::new(
                "Failed to list relay hosts",
                Some(format!("GET /v1/hosts returned {}", response.status())),
            ));
        }
        let list = response
            .json::<RemoteRelayHostList>()
            .await
            .map_err(|error| {
                ToolError::new("Failed to parse relay host list", Some(error.to_string()))
            })?;
        Ok(list.hosts)
    }

    async fn fetch_issue(&self, issue_id: Uuid) -> Result<api_types::Issue, ToolError> {
        let url = self.url(&format!("/api/remote/issues/{issue_id}"));
        self.send_json(self.client.get(&url)).await
    }

    async fn resolve_host_project(
        &self,
        host_id: Uuid,
        project_id: Uuid,
        repositories: Option<Vec<HostWorkspaceRepoInput>>,
    ) -> Result<ResolvedHostProject, ToolError> {
        let projects_url = self.url(&host_api_path(host_id, "projects"));
        let projects: Vec<HostProject> = self.send_json(self.client.get(&projects_url)).await?;
        let matched = find_host_project(&projects, project_id);
        let explicit = explicit_host_repos(repositories);
        let (repos, non_git_repo_names) = if let Some(repos) = explicit {
            (repos, Vec::new())
        } else if let Some(project) = matched {
            let (git_repos, non_git) = git_repos_from_host(&project.repos);
            if git_repos.is_empty() {
                let mut scratch_ids = vec![project_id];
                if let Some(remote_project_id) = project.remote_project_id
                    && remote_project_id != project_id
                {
                    scratch_ids.push(remote_project_id);
                }
                (
                    self.discover_scratch_repos(host_id, &scratch_ids).await?,
                    non_git,
                )
            } else {
                (git_repos, non_git)
            }
        } else {
            (
                self.discover_scratch_repos(host_id, &[project_id]).await?,
                Vec::new(),
            )
        };

        let name = matched
            .map(|project| project.name.clone())
            .unwrap_or_default();
        Ok(ResolvedHostProject {
            local_project_id: matched.map(|project| project.id),
            remote_project_id: matched.and_then(|project| project.remote_project_id),
            name,
            repos,
            non_git_repo_names,
        })
    }

    async fn discover_scratch_repos(
        &self,
        host_id: Uuid,
        project_ids: &[Uuid],
    ) -> Result<Vec<WorkspaceRepoInput>, ToolError> {
        let mut seen = Vec::new();
        for project_id in project_ids {
            if seen.contains(project_id) {
                continue;
            }
            seen.push(*project_id);
            let repos = self.host_scratch_repos(host_id, *project_id).await?;
            if !repos.is_empty() {
                return Ok(repos);
            }
        }
        Ok(Vec::new())
    }

    async fn host_scratch_repos(
        &self,
        host_id: Uuid,
        project_id: Uuid,
    ) -> Result<Vec<WorkspaceRepoInput>, ToolError> {
        let url = self.url(&host_api_path(
            host_id,
            &format!("scratch/PROJECT_REPO_DEFAULTS/{project_id}"),
        ));
        let response = self.client.get(&url).send().await.map_err(|error| {
            ToolError::new("Failed to connect to VK API", Some(error.to_string()))
        })?;
        if response.status() == reqwest::StatusCode::BAD_REQUEST
            || response.status() == reqwest::StatusCode::NOT_FOUND
        {
            return Ok(Vec::new());
        }
        if !response.status().is_success() {
            return Err(ToolError::message(format!(
                "VK API returned error status: {}",
                response.status()
            )));
        }
        let scratch = response
            .json::<ApiResponseEnvelope<HostScratch>>()
            .await
            .map_err(|error| {
                ToolError::new("Failed to parse VK API response", Some(error.to_string()))
            })?;
        if !scratch.success {
            return Ok(Vec::new());
        }
        Ok(scratch.data.as_ref().map(scratch_repos).unwrap_or_default())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex, Once};

    use rmcp::handler::server::wrapper::Parameters;
    use serde_json::Value;
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
    };
    use uuid::Uuid;

    use super::{
        HostRepo, HostSummary, McpServer, PairedHostBody, RemoteRelayHost,
        StartWorkspaceOnHostRequest, find_host_project, git_repos_from_host, host_api_path,
        host_display_name, issue_belongs_to_requested_project, merge_paired_hosts,
        prompt_from_issue, require_online_host, workspace_prompt,
    };

    static RUSTLS_PROVIDER: Once = Once::new();

    fn install_rustls_provider() {
        RUSTLS_PROVIDER.call_once(|| {
            let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
        });
    }

    #[test]
    fn host_workspace_start_path_is_host_scoped() {
        let host_id = Uuid::nil();
        let path = host_api_path(host_id, "workspaces/start");
        assert_eq!(
            path,
            "/api/host/00000000-0000-0000-0000-000000000000/workspaces/start"
        );
        assert_ne!(path, "/api/workspaces/start");
    }

    #[test]
    fn merge_marks_paired_hosts_online_or_offline() {
        let online_id = Uuid::from_u128(1);
        let offline_id = Uuid::from_u128(2);
        let unknown_id = Uuid::from_u128(3);
        let hosts = merge_paired_hosts(
            &[
                PairedHostBody {
                    host_id: unknown_id,
                    host_name: Some("paired-only".to_string()),
                },
                PairedHostBody {
                    host_id: offline_id,
                    host_name: Some("stale".to_string()),
                },
                PairedHostBody {
                    host_id: online_id,
                    host_name: Some("ignored".to_string()),
                },
            ],
            &[
                RemoteRelayHost {
                    id: online_id,
                    name: "build-box".to_string(),
                    status: "online".to_string(),
                },
                RemoteRelayHost {
                    id: offline_id,
                    name: "laptop".to_string(),
                    status: "offline".to_string(),
                },
            ],
        );

        assert_eq!(
            hosts,
            vec![
                HostSummary {
                    id: online_id.to_string(),
                    name: "build-box".to_string(),
                    status: "online".to_string(),
                },
                HostSummary {
                    id: offline_id.to_string(),
                    name: "laptop".to_string(),
                    status: "offline".to_string(),
                },
                HostSummary {
                    id: unknown_id.to_string(),
                    name: "paired-only".to_string(),
                    status: "offline".to_string(),
                },
            ]
        );
        assert_eq!(
            host_display_name(None, None, online_id),
            online_id.to_string()
        );
    }

    #[test]
    fn offline_or_unpaired_host_is_rejected_before_create() {
        let online_id = Uuid::from_u128(1);
        let offline_id = Uuid::from_u128(2);
        let hosts = vec![
            HostSummary {
                id: online_id.to_string(),
                name: "build-box".to_string(),
                status: "online".to_string(),
            },
            HostSummary {
                id: offline_id.to_string(),
                name: "laptop".to_string(),
                status: "offline".to_string(),
            },
        ];

        assert!(require_online_host(&hosts, online_id).is_ok());
        let offline = require_online_host(&hosts, offline_id).unwrap_err();
        assert!(offline.contains("offline"));
        assert!(offline.contains("this machine"));
        assert!(offline.contains(&offline_id.to_string()));
        let missing = require_online_host(&hosts, Uuid::from_u128(9)).unwrap_err();
        assert!(missing.contains("not paired"));
    }

    #[test]
    fn project_repos_come_from_the_host_project() {
        let local_id = Uuid::from_u128(10);
        let remote_id = Uuid::from_u128(11);
        let git_repo = Uuid::from_u128(12);
        let projects = vec![super::HostProject {
            id: local_id,
            name: "Demo".to_string(),
            remote_project_id: Some(remote_id),
            repos: vec![
                HostRepo {
                    id: git_repo,
                    name: "app".to_string(),
                    default_target_branch: None,
                    is_git: true,
                },
                HostRepo {
                    id: Uuid::from_u128(13),
                    name: "notes".to_string(),
                    default_target_branch: Some("dev".to_string()),
                    is_git: false,
                },
            ],
        }];

        let by_remote = find_host_project(&projects, remote_id).unwrap();
        assert_eq!(by_remote.id, local_id);
        let (repos, non_git) = git_repos_from_host(&by_remote.repos);
        assert_eq!(repos.len(), 1);
        assert_eq!(repos[0].repo_id, git_repo);
        assert_eq!(repos[0].target_branch, "main");
        assert_eq!(non_git, vec!["notes".to_string()]);
        assert!(issue_belongs_to_requested_project(
            remote_id,
            local_id,
            Some(remote_id)
        ));
        assert!(!issue_belongs_to_requested_project(
            Uuid::from_u128(99),
            remote_id,
            Some(remote_id)
        ));
        assert_eq!(
            prompt_from_issue("Fix login", Some("details")),
            Some("Fix login\n\ndetails".to_string())
        );
        assert_eq!(
            workspace_prompt(None, Some("Fix login"), Some("details"), "Demo"),
            "Fix login\n\ndetails"
        );
        assert_eq!(
            workspace_prompt(None, None, None, "Demo"),
            "Work on project \"Demo\"."
        );
    }

    type ApiHandler = Arc<dyn Fn(&Hit) -> (u16, String) + Send + Sync>;

    #[derive(Clone)]
    struct Hit {
        method: String,
        path: String,
        body: String,
        authorization: Option<String>,
    }

    fn envelope(data: Value) -> String {
        serde_json::json!({
            "success": true,
            "data": data,
            "message": null
        })
        .to_string()
    }

    async fn read_request(socket: &mut tokio::net::TcpStream) -> std::io::Result<Hit> {
        let mut buf = Vec::new();
        let mut tmp = [0u8; 4096];
        let header_end = loop {
            let read = socket.read(&mut tmp).await?;
            if read == 0 {
                break 0;
            }
            buf.extend_from_slice(&tmp[..read]);
            if let Some(pos) = buf.windows(4).position(|window| window == b"\r\n\r\n") {
                break pos + 4;
            }
        };
        let header_text = String::from_utf8_lossy(&buf[..header_end]).to_string();
        let mut body = buf[header_end..].to_vec();
        let content_length = header_text
            .lines()
            .find_map(|line| {
                let (name, value) = line.split_once(':')?;
                if name.eq_ignore_ascii_case("content-length") {
                    value.trim().parse::<usize>().ok()
                } else {
                    None
                }
            })
            .unwrap_or(0);
        while body.len() < content_length {
            let read = socket.read(&mut tmp).await?;
            if read == 0 {
                break;
            }
            body.extend_from_slice(&tmp[..read]);
        }
        let request_line = header_text.lines().next().unwrap_or("");
        let mut parts = request_line.split_whitespace();
        let method = parts.next().unwrap_or("").to_string();
        let path = parts
            .next()
            .unwrap_or("")
            .split('?')
            .next()
            .unwrap_or("")
            .to_string();
        let authorization = header_text.lines().find_map(|line| {
            let (name, value) = line.split_once(':')?;
            if name.eq_ignore_ascii_case("authorization") {
                Some(value.trim().to_string())
            } else {
                None
            }
        });
        Ok(Hit {
            method,
            path,
            body: String::from_utf8_lossy(&body[..content_length.min(body.len())]).to_string(),
            authorization,
        })
    }

    fn http_json(status: u16, body: &str) -> String {
        let reason = if status == 200 { "OK" } else { "Error" };
        format!(
            "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
    }

    async fn spawn_api(handler: ApiHandler) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move {
            loop {
                let Ok((mut socket, _)) = listener.accept().await else {
                    break;
                };
                let handler = handler.clone();
                tokio::spawn(async move {
                    let Ok(hit) = read_request(&mut socket).await else {
                        return;
                    };
                    let (status, body) = handler(&hit);
                    let _ = socket.write_all(http_json(status, &body).as_bytes()).await;
                    let _ = socket.shutdown().await;
                });
            }
        });
        format!("http://{address}")
    }

    fn tool_text(result: rmcp::model::CallToolResult) -> (bool, String) {
        let text = result
            .content
            .first()
            .and_then(|content| content.as_text())
            .map(|text| text.text.clone())
            .unwrap_or_default();
        (result.is_error == Some(true), text)
    }

    fn server(base: &str) -> McpServer {
        install_rustls_provider();
        McpServer::new_global(base)
    }

    #[derive(Clone)]
    struct Fixture {
        online_host: Uuid,
        offline_host: Uuid,
        local_project: Uuid,
        remote_project: Uuid,
        issue_id: Uuid,
        repo_id: Uuid,
        workspace_id: Uuid,
    }

    fn fixture() -> Fixture {
        Fixture {
            online_host: Uuid::from_u128(1),
            offline_host: Uuid::from_u128(2),
            local_project: Uuid::from_u128(10),
            remote_project: Uuid::from_u128(11),
            issue_id: Uuid::from_u128(12),
            repo_id: Uuid::from_u128(13),
            workspace_id: Uuid::from_u128(14),
        }
    }

    fn api_handler(
        base_slot: Arc<Mutex<String>>,
        hits: Arc<Mutex<Vec<Hit>>>,
        fx: Fixture,
    ) -> ApiHandler {
        Arc::new(move |hit: &Hit| {
            hits.lock().expect("hits").push(hit.clone());
            let fx = &fx;
            let body = match hit.path.as_str() {
                "/api/relay-auth/client/hosts" => envelope(serde_json::json!({
                    "hosts": [
                        {"host_id": fx.online_host, "host_name": "ignored", "paired_at": null},
                        {"host_id": fx.offline_host, "host_name": "laptop", "paired_at": null}
                    ]
                })),
                "/api/info" => {
                    let base = base_slot.lock().expect("base").clone();
                    envelope(serde_json::json!({ "shared_api_base": base }))
                }
                "/api/auth/token" => envelope(serde_json::json!({
                    "access_token": "test-token",
                    "expires_at": null
                })),
                "/v1/hosts" => serde_json::json!({
                    "hosts": [
                        {"id": fx.online_host, "name": "build-box", "status": "online"},
                        {"id": fx.offline_host, "name": "laptop", "status": "offline"}
                    ]
                })
                .to_string(),
                path if path == format!("/api/host/{}/projects", fx.online_host) => {
                    envelope(serde_json::json!([{
                        "id": fx.local_project,
                        "name": "Demo",
                        "remote_project_id": fx.remote_project,
                        "repos": [{
                            "id": fx.repo_id,
                            "name": "app",
                            "default_target_branch": "trunk",
                            "is_git": true
                        }]
                    }]))
                }
                path if path == format!("/api/remote/issues/{}", fx.issue_id) => {
                    envelope(serde_json::json!({
                        "id": fx.issue_id,
                        "project_id": fx.remote_project,
                        "issue_number": 7,
                        "simple_id": "DEMO-7",
                        "status_id": Uuid::from_u128(15),
                        "title": "Fix login",
                        "description": "session drops",
                        "priority": null,
                        "start_date": null,
                        "target_date": null,
                        "completed_at": null,
                        "sort_order": 0.0,
                        "parent_issue_id": null,
                        "parent_issue_sort_order": null,
                        "extension_metadata": {},
                        "creator_user_id": null,
                        "created_at": "2026-01-01T00:00:00Z",
                        "updated_at": "2026-01-01T00:00:00Z"
                    }))
                }
                path if path == format!("/api/host/{}/workspaces/start", fx.online_host) => {
                    envelope(serde_json::json!({
                        "workspace": {
                            "id": fx.workspace_id,
                            "task_id": null,
                            "container_ref": null,
                            "branch": "vk/demo",
                            "setup_completed_at": null,
                            "created_at": "2026-01-01T00:00:00Z",
                            "updated_at": "2026-01-01T00:00:00Z",
                            "archived": false,
                            "pinned": false,
                            "name": "Fix login",
                            "worktree_deleted": false,
                            "kind": "worktree"
                        },
                        "execution_process": {
                            "id": Uuid::from_u128(16),
                            "session_id": Uuid::from_u128(17),
                            "run_reason": "codingagent",
                            "executor_action": {"ignored": true},
                            "status": "running",
                            "exit_code": null,
                            "dropped": false,
                            "started_at": "2026-01-01T00:00:00Z",
                            "completed_at": null,
                            "created_at": "2026-01-01T00:00:00Z",
                            "updated_at": "2026-01-01T00:00:00Z"
                        }
                    }))
                }
                _ => {
                    return (
                        500,
                        serde_json::json!({
                            "success": false,
                            "message": format!("unexpected {} {}", hit.method, hit.path)
                        })
                        .to_string(),
                    );
                }
            };
            (200, body)
        })
    }

    async fn boot_api(hits: Arc<Mutex<Vec<Hit>>>, fx: Fixture) -> String {
        let base_slot = Arc::new(Mutex::new(String::new()));
        let base = spawn_api(api_handler(base_slot.clone(), hits, fx)).await;
        *base_slot.lock().expect("base") = base.clone();
        base
    }

    #[tokio::test]
    async fn list_hosts_returns_paired_online_status() {
        let hits = Arc::new(Mutex::new(Vec::new()));
        let fx = fixture();
        let base = boot_api(hits.clone(), fx.clone()).await;
        let (is_error, text) = tool_text(server(&base).list_hosts().await.unwrap());
        assert!(!is_error, "{text}");
        let body: Value = serde_json::from_str(&text).unwrap();
        let hosts = body["hosts"].as_array().unwrap();
        assert_eq!(hosts.len(), 2);
        assert_eq!(hosts[0]["name"], "build-box");
        assert_eq!(hosts[0]["id"], fx.online_host.to_string());
        assert_eq!(hosts[0]["status"], "online");
        assert_eq!(hosts[1]["name"], "laptop");
        assert_eq!(hosts[1]["status"], "offline");
        let recorded = hits.lock().expect("hits").clone();
        assert!(recorded.iter().any(|hit| {
            hit.path == "/v1/hosts" && hit.authorization.as_deref() == Some("Bearer test-token")
        }));
    }

    #[tokio::test]
    async fn offline_host_does_not_create_a_local_workspace() {
        let hits = Arc::new(Mutex::new(Vec::new()));
        let fx = fixture();
        let base = boot_api(hits.clone(), fx.clone()).await;
        let (is_error, text) = tool_text(
            server(&base)
                .start_workspace_on_host(Parameters(StartWorkspaceOnHostRequest {
                    project_id: fx.remote_project,
                    issue_id: Some(fx.issue_id),
                    host_id: fx.offline_host,
                    prompt: None,
                    executor: None,
                    variant: None,
                    name: None,
                    repositories: None,
                }))
                .await
                .unwrap(),
        );
        assert!(is_error, "{text}");
        assert!(text.contains("offline"), "{text}");
        assert!(text.contains("this machine"), "{text}");
        let recorded = hits.lock().expect("hits").clone();
        assert!(
            recorded
                .iter()
                .all(|hit| hit.path != "/api/workspaces/start"
                    && !hit.path.starts_with("/api/host/"))
        );
    }

    #[tokio::test]
    async fn start_workspace_on_host_posts_only_to_the_online_host() {
        let hits = Arc::new(Mutex::new(Vec::new()));
        let fx = fixture();
        let base = boot_api(hits.clone(), fx.clone()).await;
        let (is_error, text) = tool_text(
            server(&base)
                .start_workspace_on_host(Parameters(StartWorkspaceOnHostRequest {
                    project_id: fx.local_project,
                    issue_id: Some(fx.issue_id),
                    host_id: fx.online_host,
                    prompt: None,
                    executor: Some("CODEX".to_string()),
                    variant: None,
                    name: None,
                    repositories: None,
                }))
                .await
                .unwrap(),
        );
        assert!(!is_error, "{text}");
        let body: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(body["workspace_id"], fx.workspace_id.to_string());
        assert_eq!(body["host_id"], fx.online_host.to_string());
        assert_eq!(body["issue_id"], fx.issue_id.to_string());

        let recorded = hits.lock().expect("hits").clone();
        assert!(
            recorded
                .iter()
                .all(|hit| hit.path != "/api/workspaces/start")
        );
        let start = recorded
            .iter()
            .find(|hit| hit.method == "POST" && hit.path.ends_with("/workspaces/start"))
            .expect("host start request");
        assert_eq!(
            start.path,
            format!("/api/host/{}/workspaces/start", fx.online_host)
        );
        let payload: Value = serde_json::from_str(&start.body).unwrap();
        assert_eq!(payload["project_id"], fx.local_project.to_string());
        assert_eq!(payload["repos"][0]["repo_id"], fx.repo_id.to_string());
        assert_eq!(payload["repos"][0]["target_branch"], "trunk");
        assert_eq!(payload["linked_issue"]["issue_id"], fx.issue_id.to_string());
        assert_eq!(
            payload["linked_issue"]["remote_project_id"],
            fx.remote_project.to_string()
        );
        assert_eq!(payload["prompt"], "Fix login\n\nsession drops");
    }
}
