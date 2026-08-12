use std::{collections::HashMap, time::Duration};

use convert_case::{Case, Casing};
use serde::Deserialize;
use tokio::{io::AsyncReadExt, process::Command, time};
use workspace_utils::{command_ext::GroupSpawnNoWindowExt, shell::resolve_executable_path};

use crate::{
    command::{CmdOverrides, CommandBuilder, apply_overrides},
    executors::{
        ExecutorError,
        cursor::{CursorAgent, cursor_reasoning_options},
    },
    model_selector::{ModelInfo, ModelProvider, ReasoningOption},
};

fn pi_thinking_options() -> Vec<ReasoningOption> {
    ReasoningOption::from_names(
        ["off", "minimal", "low", "medium", "high", "xhigh", "max"].map(String::from),
    )
}

const DISCOVERY_TIMEOUT: Duration = Duration::from_secs(45);

pub async fn discover_cursor_models(
    base_command: &str,
    cmd: &CmdOverrides,
) -> Result<Vec<ModelInfo>, ExecutorError> {
    // Prefer the primary Cursor CLI name (`agent`) and fall back to the legacy
    // `cursor-agent` name when no override is configured.
    let resolved_base = if cmd.base_command_override.is_some() {
        base_command.to_string()
    } else {
        let mut executable = None;
        for name in CursorAgent::executable_names() {
            if let Some(path) = resolve_executable_path(name).await {
                executable = Some(path);
                break;
            }
        }
        executable
            .ok_or_else(|| ExecutorError::ExecutableNotFound {
                program: base_command.to_string(),
            })?
            .to_string_lossy()
            .into_owned()
    };

    let builder = apply_overrides(
        CommandBuilder::new(resolved_base).extend_params(["--list-models"]),
        cmd,
    )
    .map_err(|err| ExecutorError::Io(std::io::Error::other(err.to_string())))?;

    let output = run_command_capture(&builder, &[], &cmd_env(cmd)).await?;
    parse_cursor_list_models(&output).ok_or_else(|| {
        ExecutorError::Io(std::io::Error::other(
            "failed to parse Cursor agent --list-models output",
        ))
    })
}

pub async fn discover_pi_models(
    base_command: &str,
    cmd: &CmdOverrides,
) -> Result<Vec<ModelInfo>, ExecutorError> {
    let builder = apply_overrides(
        CommandBuilder::new(base_command).extend_params(["--list-models"]),
        cmd,
    )
    .map_err(|err| ExecutorError::Io(std::io::Error::other(err.to_string())))?;

    match run_command_capture(&builder, &[], &cmd_env(cmd)).await {
        Ok(output) => {
            if let Some(models) = parse_pi_list_models(&output) {
                return Ok(models);
            }
            // `pi --list-models` prints a help message (and often exits 0) when
            // no auth/config is available. Fall through to models.json.
            tracing::debug!("pi --list-models returned no parseable models; trying models.json");
        }
        Err(error) => {
            tracing::warn!(
                ?error,
                "pi --list-models failed; trying ~/.pi/agent/models.json"
            );
        }
    }

    load_pi_models_from_config().ok_or_else(|| {
        ExecutorError::Io(std::io::Error::other(
            "failed to discover Pi models via --list-models or ~/.pi/agent/models.json",
        ))
    })
}

/// Discover Oh My Pi (`omp`) models via `omp models --json`.
///
/// Unlike `pi --list-models` (ASCII table), omp emits structured JSON, so no
/// column slicing is needed. Falls back to `~/.omp/agent/models.json`.
pub async fn discover_oh_my_pi_models(
    base_command: &str,
    cmd: &CmdOverrides,
) -> Result<Vec<ModelInfo>, ExecutorError> {
    let builder = apply_overrides(
        CommandBuilder::new(base_command).extend_params(["models", "--json"]),
        cmd,
    )
    .map_err(|err| ExecutorError::Io(std::io::Error::other(err.to_string())))?;

    match run_command_capture(&builder, &[], &cmd_env(cmd)).await {
        Ok(output) => {
            if let Some(models) = parse_oh_my_pi_models_json(&output) {
                return Ok(models);
            }
            tracing::debug!("omp models --json returned no parseable models; trying models.json");
        }
        Err(error) => {
            tracing::warn!(
                ?error,
                "omp models --json failed; trying ~/.omp/agent/models.json"
            );
        }
    }

    load_oh_my_pi_models_from_config(base_command, cmd)
        .await
        .ok_or_else(|| {
            ExecutorError::Io(std::io::Error::other(
                "failed to discover Oh My Pi models via `omp models --json` or models.yml",
            ))
        })
}

/// Parse `omp models --json` output: `{"models":[{provider,id,selector,...}]}`.
pub fn parse_oh_my_pi_models_json(output: &str) -> Option<Vec<ModelInfo>> {
    // omp may print warnings before the JSON payload; start at the first `{`.
    let start = output.find('{')?;
    let value: serde_json::Value = serde_json::from_str(output.get(start..)?).ok()?;
    let entries = value.get("models")?.as_array()?;

    let mut models = Vec::new();
    for entry in entries {
        let provider = entry.get("provider").and_then(|v| v.as_str()).unwrap_or("");
        let model_id = entry.get("id").and_then(|v| v.as_str()).unwrap_or("");
        if provider.is_empty() || model_id.is_empty() {
            continue;
        }
        // Keep `id` bare (provider-relative). The UI composes
        // `{provider_id}/{id}` for ExecutorConfig.model_id / `--model`.
        // Using the omp `selector` here previously caused a double prefix
        // (`xunmeng/xunmeng/claude-opus-5`) and broke default-model matching.
        let name = entry
            .get("name")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .unwrap_or(model_id)
            .to_string();
        let supports_thinking = entry
            .get("reasoning")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
            || entry.get("thinking").is_some_and(|v| !v.is_null());
        models.push(ModelInfo {
            id: model_id.to_string(),
            name,
            provider_id: Some(provider.to_string()),
            reasoning_options: if supports_thinking {
                pi_thinking_options()
            } else {
                vec![]
            },
        });
    }

    sort_pi_like_models(&mut models);
    (!models.is_empty()).then_some(models)
}

/// Read configured custom providers from omp's `models.yml`.
///
/// omp stores providers as YAML (pi uses `models.json`), but the shape is the
/// same: `providers.<id>.models[].id`. The workspace has no YAML dependency and
/// this is only a fallback for when `omp models --json` fails, so the narrow
/// subset emitted by omp is parsed directly.
async fn load_oh_my_pi_models_from_config(
    base_command: &str,
    cmd: &CmdOverrides,
) -> Option<Vec<ModelInfo>> {
    let agent_dir = match oh_my_pi_agent_dir(base_command, cmd).await {
        Some(dir) => dir,
        None => dirs::home_dir()?.join(".omp").join("agent"),
    };
    let raw = std::fs::read_to_string(agent_dir.join("models.yml")).ok()?;
    parse_oh_my_pi_models_yaml(&raw)
}

/// Parse the `providers:` section of omp's `models.yml`.
///
/// Handles the shape omp writes: two-space indented provider keys, each with a
/// `models:` sequence of `- id: <name>` entries (or bare `- <name>`).
pub fn parse_oh_my_pi_models_yaml(raw: &str) -> Option<Vec<ModelInfo>> {
    let mut models = Vec::new();
    let mut provider: Option<String> = None;
    let mut in_providers = false;
    let mut in_models = false;

    for line in raw.lines() {
        // Strip comments and skip blanks.
        let line = line.split_once('#').map_or(line, |(head, _)| head);
        if line.trim().is_empty() {
            continue;
        }
        let indent = line.len() - line.trim_start().len();
        let trimmed = line.trim();

        if indent == 0 {
            in_providers = trimmed.starts_with("providers:");
            provider = None;
            in_models = false;
            continue;
        }
        if !in_providers {
            continue;
        }

        // A provider key: `  <id>:` at the first nesting level.
        if indent == 2 {
            provider = trimmed
                .strip_suffix(':')
                .filter(|id| !id.is_empty())
                .map(str::to_string);
            in_models = false;
            continue;
        }

        let Some(provider_id) = provider.as_deref() else {
            continue;
        };

        // Inside a provider: track whether we are under its `models:` key.
        if !trimmed.starts_with('-') {
            in_models = trimmed.starts_with("models:");
            continue;
        }
        if !in_models {
            continue;
        }

        // Sequence entry: `- id: name`, `- id: "name"` or bare `- name`.
        let entry = trimmed.trim_start_matches('-').trim();
        let model_id = entry
            .strip_prefix("id:")
            .map(str::trim)
            .unwrap_or(entry)
            .trim_matches(['"', '\''])
            .trim();
        if model_id.is_empty() || model_id.ends_with(':') {
            continue;
        }

        models.push(ModelInfo {
            id: model_id.to_string(),
            name: model_id.to_string(),
            provider_id: Some(provider_id.to_string()),
            // models.yml carries no reasoning flag; omp accepts --thinking on
            // any model, so expose the full set rather than hiding the control.
            reasoning_options: pi_thinking_options(),
        });
    }

    sort_pi_like_models(&mut models);
    (!models.is_empty()).then_some(models)
}

/// Providers extracted from a discovered Pi model list (stable order).
pub fn pi_providers_from_models(models: &[ModelInfo]) -> Vec<ModelProvider> {
    let mut seen = std::collections::HashSet::new();
    let mut providers = Vec::new();
    for model in models {
        let Some(provider_id) = model.provider_id.as_deref() else {
            continue;
        };
        if seen.insert(provider_id.to_string()) {
            providers.push(ModelProvider {
                id: provider_id.to_string(),
                name: provider_id.to_string(),
            });
        }
    }
    providers
}

pub async fn discover_codex_models(
    base_command: &str,
    cmd: &CmdOverrides,
) -> Result<Vec<ModelInfo>, ExecutorError> {
    let builder = apply_overrides(
        CommandBuilder::new(base_command).extend_params(["debug", "models"]),
        cmd,
    )
    .map_err(|err| ExecutorError::Io(std::io::Error::other(err.to_string())))?;

    let output = run_command_capture(&builder, &[], &cmd_env(cmd)).await?;
    parse_codex_models_json(&output).ok_or_else(|| {
        ExecutorError::Io(std::io::Error::other(
            "failed to parse codex debug models output",
        ))
    })
}

pub const ANTIGRAVITY_DEFAULT_MODEL: &str = "gemini-3.6-flash-high";

pub fn antigravity_default_models() -> Vec<ModelInfo> {
    // Fallback catalog matching `agy models` ids (`id\tdisplay name`).
    [
        ("gemini-3.6-flash-high", "Gemini 3.6 Flash (High)"),
        ("gemini-3.6-flash-medium", "Gemini 3.6 Flash (Medium)"),
        ("gemini-3.6-flash-low", "Gemini 3.6 Flash (Low)"),
        ("gemini-3.5-flash-high", "Gemini 3.5 Flash (High)"),
        ("gemini-3.5-flash-medium", "Gemini 3.5 Flash (Medium)"),
        ("gemini-3.5-flash-low", "Gemini 3.5 Flash (Low)"),
        ("gemini-3.1-pro-high", "Gemini 3.1 Pro (High)"),
        ("gemini-3.1-pro-low", "Gemini 3.1 Pro (Low)"),
        ("claude-sonnet-4-6", "Claude Sonnet 4.6 (Thinking)"),
        ("claude-opus-4-6-thinking", "Claude Opus 4.6 (Thinking)"),
        ("gpt-oss-120b-medium", "GPT-OSS 120B (Medium)"),
    ]
    .into_iter()
    .map(|(id, name)| ModelInfo {
        id: id.to_string(),
        name: name.to_string(),
        provider_id: None,
        reasoning_options: vec![],
    })
    .collect()
}

pub async fn discover_antigravity_models(
    base_command: &str,
    cmd: &CmdOverrides,
) -> Result<Vec<ModelInfo>, ExecutorError> {
    let builder = apply_overrides(
        CommandBuilder::new(base_command).extend_params(["models"]),
        cmd,
    )
    .map_err(|err| ExecutorError::Io(std::io::Error::other(err.to_string())))?;

    match run_command_capture(&builder, &[], &cmd_env(cmd)).await {
        Ok(output) => {
            if let Some(models) = parse_antigravity_models_output(&output) {
                return Ok(models);
            }
            tracing::debug!("agy models returned no parseable models; trying fallbacks");
        }
        Err(error) => {
            tracing::warn!(
                ?error,
                "agy models failed; trying config/default Antigravity models"
            );
        }
    }

    if let Some(models) = load_antigravity_models_from_config() {
        return Ok(models);
    }

    Ok(antigravity_default_models())
}

fn model_info_from_json_item(item: &serde_json::Value) -> Option<ModelInfo> {
    let id = item
        .get("id")
        .or_else(|| item.get("model"))
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())?;
    let name = item
        .get("name")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or(id);
    Some(ModelInfo {
        id: id.to_string(),
        name: name.to_string(),
        provider_id: None,
        reasoning_options: vec![],
    })
}

/// Parse `agy models` output.
///
/// Real CLI emits TSV lines: `gemini-3.6-flash-high\tGemini 3.6 Flash (High)`.
/// Also accepts JSON arrays / `{ "models": [...] }` when present.
pub fn parse_antigravity_models_output(output: &str) -> Option<Vec<ModelInfo>> {
    if let Some(start) = output.find('[') {
        if let Ok(serde_json::Value::Array(items)) =
            serde_json::from_str::<serde_json::Value>(&output[start..])
        {
            let models: Vec<_> = items.iter().filter_map(model_info_from_json_item).collect();
            if !models.is_empty() {
                return Some(models);
            }
        }
    }

    if let Some(start) = output.find('{') {
        if let Ok(serde_json::Value::Object(map)) =
            serde_json::from_str::<serde_json::Value>(&output[start..])
        {
            if let Some(serde_json::Value::Array(items)) = map.get("models") {
                let models: Vec<_> = items.iter().filter_map(model_info_from_json_item).collect();
                if !models.is_empty() {
                    return Some(models);
                }
            }
        }
    }

    let mut models = Vec::new();
    for line in output.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let lower = trimmed.to_ascii_lowercase();
        if lower.starts_with("fetching")
            || lower.starts_with("usage")
            || lower.starts_with("available models")
        {
            continue;
        }

        // Preferred: `id\tdisplay name`
        if let Some((id, name)) = trimmed.split_once('\t') {
            let id = id.trim();
            let name = name.trim();
            if id.is_empty() {
                continue;
            }
            models.push(ModelInfo {
                id: id.to_string(),
                name: if name.is_empty() {
                    id.to_string()
                } else {
                    name.to_string()
                },
                provider_id: None,
                reasoning_options: vec![],
            });
            continue;
        }

        // Fallback: first whitespace token looks like a model id.
        let id = trimmed.split_whitespace().next().unwrap_or("");
        let id_lower = id.to_ascii_lowercase();
        if !id.is_empty()
            && (id_lower.contains("gemini")
                || id_lower.contains("claude")
                || id_lower.contains("gpt")
                || id_lower.contains("flash")
                || id_lower.contains("sonnet")
                || id_lower.contains("opus"))
        {
            models.push(ModelInfo {
                id: id.to_string(),
                name: id.to_string(),
                provider_id: None,
                reasoning_options: vec![],
            });
        }
    }

    (!models.is_empty()).then_some(models)
}

fn load_antigravity_models_from_config() -> Option<Vec<ModelInfo>> {
    let home = dirs::home_dir()?;
    let path = home
        .join(".gemini")
        .join("antigravity-cli")
        .join("models.json");
    let content = std::fs::read_to_string(path).ok()?;
    parse_antigravity_models_output(&content)
}

async fn run_command_capture(
    builder: &CommandBuilder,
    additional_args: &[String],
    env: &HashMap<String, String>,
) -> Result<String, ExecutorError> {
    let command_parts = builder
        .build_follow_up(additional_args)
        .map_err(|err| ExecutorError::Io(std::io::Error::other(err.to_string())))?;
    let (executable_path, args) = command_parts.into_resolved().await?;

    let mut command = Command::new(executable_path);
    command
        .kill_on_drop(true)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .env("NPM_CONFIG_LOGLEVEL", "error")
        .args(&args);

    for (key, value) in env {
        command.env(key, value);
    }

    let mut child = command.group_spawn_no_window()?;

    let mut stdout = child
        .inner()
        .stdout
        .take()
        .ok_or_else(|| ExecutorError::Io(std::io::Error::other("missing stdout")))?;
    let mut stderr = child
        .inner()
        .stderr
        .take()
        .ok_or_else(|| ExecutorError::Io(std::io::Error::other("missing stderr")))?;

    // Wait for exit concurrently with pipe reads. Some CLIs (e.g. `agy`) leave
    // grandchildren holding inherited stdout/stderr, so EOF may never arrive —
    // after the leader exits, drain briefly then return whatever we have.
    let result = time::timeout(DISCOVERY_TIMEOUT, async {
        let mut stdout_buf = String::new();
        let mut stderr_buf = String::new();
        let read_stdout = stdout.read_to_string(&mut stdout_buf);
        let read_stderr = stderr.read_to_string(&mut stderr_buf);
        let wait = child.inner().wait();
        tokio::pin!(read_stdout, read_stderr, wait);

        let mut stdout_done = false;
        let mut stderr_done = false;
        let mut status = None;

        loop {
            tokio::select! {
                res = &mut read_stdout, if !stdout_done => {
                    res?;
                    stdout_done = true;
                }
                res = &mut read_stderr, if !stderr_done => {
                    res?;
                    stderr_done = true;
                }
                res = &mut wait, if status.is_none() => {
                    status = Some(res?);
                }
                _ = time::sleep(Duration::from_millis(250)),
                    if status.is_some() && (!stdout_done || !stderr_done) =>
                {
                    break;
                }
            }

            if status.is_some() && stdout_done && stderr_done {
                break;
            }
        }

        let status = status.ok_or_else(|| {
            std::io::Error::other("model discovery process ended without exit status")
        })?;
        Ok::<_, std::io::Error>(((stdout_buf, stderr_buf), status))
    })
    .await;

    match result {
        Ok(Ok(((stdout_buf, stderr_buf), status))) if status.success() => Ok(stdout_buf),
        Ok(Ok(((_, stderr_buf), _))) => Err(ExecutorError::Io(std::io::Error::other(format!(
            "model discovery command failed: {stderr_buf}"
        )))),
        Ok(Err(err)) => Err(ExecutorError::Io(err)),
        Err(_) => {
            let _ = child.kill().await;
            Err(ExecutorError::Io(std::io::Error::other(
                "model discovery command timed out",
            )))
        }
    }
}

fn cmd_env(cmd: &CmdOverrides) -> HashMap<String, String> {
    cmd.env.clone().unwrap_or_default()
}

/// Read omp's configured default model (`modelRoles.default`).
///
/// omp resolves the model itself when no `--model` is passed, so this is only
/// used to *display* the active choice in the model selector. Returns e.g.
/// `xunmeng/claude-opus-5`.
pub async fn oh_my_pi_default_model(base_command: &str, cmd: &CmdOverrides) -> Option<String> {
    let builder = apply_overrides(
        CommandBuilder::new(base_command).extend_params(["config", "get", "modelRoles", "--json"]),
        cmd,
    )
    .ok()?;
    let output = run_command_capture(&builder, &[], &cmd_env(cmd))
        .await
        .ok()?;
    parse_oh_my_pi_default_model(&output)
}

/// Extract `modelRoles.default` from `omp config get modelRoles --json`.
pub fn parse_oh_my_pi_default_model(output: &str) -> Option<String> {
    let start = output.find('{')?;
    let value: serde_json::Value = serde_json::from_str(output.get(start..)?).ok()?;
    // `config get --json` wraps the value: {key, value, type, description}.
    // Fall back to a bare record in case that envelope ever changes.
    let roles = value.get("value").unwrap_or(&value);
    let default = roles.get("default")?.as_str()?.trim();
    (!default.is_empty()).then(|| default.to_string())
}

/// Ask omp for its active agent (config) directory via `omp config path`.
///
/// omp resolves this at runtime from `--profile`, XDG dirs and
/// `PI_CODING_AGENT_DIR`, so `~/.omp/agent` is only correct in the plain case.
pub async fn oh_my_pi_agent_dir(
    base_command: &str,
    cmd: &CmdOverrides,
) -> Option<std::path::PathBuf> {
    let builder = apply_overrides(
        CommandBuilder::new(base_command).extend_params(["config", "path"]),
        cmd,
    )
    .ok()?;
    let output = run_command_capture(&builder, &[], &cmd_env(cmd))
        .await
        .ok()?;
    let path = output.trim();
    // Guard against help text / banners on stdout.
    (!path.is_empty() && !path.contains('\n')).then(|| std::path::PathBuf::from(path))
}

pub fn parse_cursor_list_models(output: &str) -> Option<Vec<ModelInfo>> {
    let mut models = Vec::new();

    for line in output.lines() {
        let line = line.trim();
        if line.is_empty()
            || line.eq_ignore_ascii_case("available models")
            || line.starts_with("Tip:")
        {
            continue;
        }

        let (id, name) = line.split_once(" - ")?;
        let id = id.trim();
        if id.is_empty() {
            continue;
        }

        let name = name
            .trim()
            .trim_end_matches(" (current, default)")
            .trim_end_matches(" (current)")
            .trim_end_matches(" (default)")
            .trim()
            .to_string();

        models.push(ModelInfo {
            id: id.to_string(),
            name,
            provider_id: None,
            reasoning_options: cursor_reasoning_options(id),
        });
    }

    (!models.is_empty()).then_some(models)
}

#[derive(Debug, Deserialize)]
struct CodexModelsResponse {
    models: Vec<CodexCatalogModel>,
}

#[derive(Debug, Deserialize)]
struct CodexCatalogModel {
    slug: String,
    display_name: String,
    #[serde(default)]
    supported_reasoning_levels: Vec<CodexReasoningLevel>,
    #[serde(default)]
    visibility: Option<String>,
    #[serde(default)]
    supported_in_api: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct CodexReasoningLevel {
    effort: String,
    #[serde(default)]
    description: Option<String>,
}

/// Parse `pi --list-models` aligned table output.
///
/// Example:
/// ```text
/// provider     model                                context  max-out  thinking  images
/// tokenpony    kimi-k3                              128K     16.4K    no        no
/// ```
pub fn parse_pi_list_models(output: &str) -> Option<Vec<ModelInfo>> {
    let lines: Vec<&str> = output
        .lines()
        .map(str::trim_end)
        .filter(|line| !line.trim().is_empty())
        .collect();
    if lines.is_empty() {
        return None;
    }

    // Help / empty catalog messages are not tables.
    let header_idx = lines.iter().position(|line| {
        let lower = line.to_ascii_lowercase();
        lower.contains("provider") && lower.contains("model") && lower.contains("thinking")
    })?;
    let header = lines[header_idx];
    let provider_start = header.to_ascii_lowercase().find("provider")?;
    let model_start = header.to_ascii_lowercase().find("model")?;
    let context_start = header.to_ascii_lowercase().find("context")?;
    let thinking_start = header.to_ascii_lowercase().find("thinking")?;
    let images_start = header.to_ascii_lowercase().find("images");

    let mut models = Vec::new();
    for line in &lines[header_idx + 1..] {
        if line.len() <= model_start {
            continue;
        }
        let provider = slice_col(line, provider_start, model_start);
        let model = slice_col(line, model_start, context_start);
        if provider.is_empty()
            || model.is_empty()
            || provider.eq_ignore_ascii_case("provider")
            || model.eq_ignore_ascii_case("model")
        {
            continue;
        }

        let thinking_end = images_start.unwrap_or(line.len());
        let thinking = slice_col(line, thinking_start, thinking_end).to_ascii_lowercase();
        let supports_thinking = matches!(thinking.as_str(), "yes" | "true" | "y");

        // Bare provider-relative id; UI prefixes with provider_id for --model.
        models.push(ModelInfo {
            id: model.clone(),
            name: model,
            provider_id: Some(provider),
            reasoning_options: if supports_thinking {
                pi_thinking_options()
            } else {
                vec![]
            },
        });
    }

    sort_pi_like_models(&mut models);

    (!models.is_empty()).then_some(models)
}

/// Prefer user-configured providers (non-catalog) first for UX.
fn sort_pi_like_models(models: &mut [ModelInfo]) {
    models.sort_by(|a, b| {
        let a_hf = a.provider_id.as_deref() == Some("huggingface");
        let b_hf = b.provider_id.as_deref() == Some("huggingface");
        match (a_hf, b_hf) {
            (false, true) => std::cmp::Ordering::Less,
            (true, false) => std::cmp::Ordering::Greater,
            // Group by provider, then bare model id (ids are provider-relative).
            _ => a
                .provider_id
                .cmp(&b.provider_id)
                .then_with(|| a.id.cmp(&b.id)),
        }
    });
}

fn slice_col(line: &str, start: usize, end: usize) -> String {
    let start = start.min(line.len());
    let end = end.min(line.len()).max(start);
    line.get(start..end).unwrap_or("").trim().to_string()
}

/// Read configured custom providers from `~/.pi/agent/models.json`.
fn load_pi_models_from_config() -> Option<Vec<ModelInfo>> {
    let path = dirs::home_dir()?
        .join(".pi")
        .join("agent")
        .join("models.json");
    parse_pi_models_config(&std::fs::read_to_string(path).ok()?)
}

/// Parse a pi/omp `models.json` config (`{"providers":{"<id>":{"models":[..]}}}`).
fn parse_pi_models_config(raw: &str) -> Option<Vec<ModelInfo>> {
    let value: serde_json::Value = serde_json::from_str(raw).ok()?;
    let providers = value.get("providers")?.as_object()?;

    let mut models = Vec::new();
    for (provider_id, provider) in providers {
        let Some(list) = provider.get("models").and_then(|m| m.as_array()) else {
            continue;
        };
        for entry in list {
            let model_id = entry
                .get("id")
                .and_then(|v| v.as_str())
                .or_else(|| entry.as_str())
                .unwrap_or("")
                .trim();
            if model_id.is_empty() {
                continue;
            }
            let supports_thinking = entry
                .get("reasoning")
                .and_then(|v| v.as_bool())
                .or_else(|| entry.get("thinking").and_then(|v| v.as_bool()))
                .unwrap_or(false);
            models.push(ModelInfo {
                id: model_id.to_string(),
                name: model_id.to_string(),
                provider_id: Some(provider_id.clone()),
                reasoning_options: if supports_thinking {
                    pi_thinking_options()
                } else {
                    vec![]
                },
            });
        }
    }

    (!models.is_empty()).then_some(models)
}

pub fn parse_codex_models_json(output: &str) -> Option<Vec<ModelInfo>> {
    let payload: CodexModelsResponse = serde_json::from_str(output.trim()).ok()?;
    let models = payload
        .models
        .into_iter()
        .filter(|model| model.supported_in_api.unwrap_or(true))
        .filter(|model| {
            model
                .visibility
                .as_deref()
                .is_none_or(|visibility| visibility == "list")
        })
        .map(|model| {
            let reasoning_options = if model.supported_reasoning_levels.is_empty() {
                vec![]
            } else {
                model
                    .supported_reasoning_levels
                    .into_iter()
                    .map(|level| ReasoningOption {
                        id: level.effort.clone(),
                        label: level
                            .description
                            .filter(|description| !description.is_empty())
                            .unwrap_or_else(|| level.effort.to_case(Case::Title)),
                        is_default: false,
                    })
                    .collect::<Vec<_>>()
            };

            ModelInfo {
                id: model.slug,
                name: model.display_name,
                provider_id: None,
                reasoning_options,
            }
        })
        .collect::<Vec<_>>();

    (!models.is_empty()).then_some(models)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_cursor_list_models_output() {
        let output = r"Available models

auto - Auto
gpt-5.3-codex-high - Codex 5.3 High
composer-2.5-fast - Composer 2.5 Fast (current, default)

Tip: use --model <id> to switch.";
        let models = parse_cursor_list_models(output).expect("models");
        assert_eq!(models.len(), 3);
        assert_eq!(models[0].id, "auto");
        assert_eq!(models[2].name, "Composer 2.5 Fast");
    }

    #[test]
    fn parse_codex_models_json_output() {
        let output = r#"{"models":[{"slug":"gpt-5.5","display_name":"GPT-5.5","supported_reasoning_levels":[{"effort":"low","description":"Fast"}],"visibility":"list","supported_in_api":true}]}"#;
        let models = parse_codex_models_json(output).expect("models");
        assert_eq!(models.len(), 1);
        assert_eq!(models[0].id, "gpt-5.5");
        assert_eq!(models[0].reasoning_options.len(), 1);
    }

    #[test]
    fn parse_pi_list_models_output() {
        let output = r"provider     model                                context  max-out  thinking  images
huggingface  deepseek-ai/DeepSeek-R1              64K      32.8K    yes       no    
tokenpony    kimi-k3                              128K     16.4K    no        no    
xunmeng      claude-opus-4-8                      128K     16.4K    no        no    
";
        let models = parse_pi_list_models(output).expect("models");
        assert_eq!(models.len(), 3);
        // Custom providers first; ids are provider-relative (bare).
        assert_eq!(models[0].id, "kimi-k3");
        assert_eq!(models[0].provider_id.as_deref(), Some("tokenpony"));
        assert!(models[0].reasoning_options.is_empty());
        assert_eq!(models[1].id, "claude-opus-4-8");
        assert_eq!(models[1].provider_id.as_deref(), Some("xunmeng"));
        assert_eq!(models[2].id, "deepseek-ai/DeepSeek-R1");
        assert_eq!(models[2].provider_id.as_deref(), Some("huggingface"));
        assert!(!models[2].reasoning_options.is_empty());
    }

    #[test]
    fn parse_pi_list_models_rejects_help_text() {
        let output = "No models available. Use /login to log into a provider via OAuth or API key.";
        assert!(parse_pi_list_models(output).is_none());
    }

    #[test]
    fn parse_oh_my_pi_models_json_output() {
        let output = r#"{"models":[
            {"provider":"huggingface","id":"deepseek-ai/DeepSeek-R1","selector":"huggingface/deepseek-ai/DeepSeek-R1","reasoning":true,"thinking":null},
            {"provider":"xunmeng","id":"claude-opus-5","selector":"xunmeng/claude-opus-5","reasoning":false,"thinking":null},
            {"provider":"","id":"broken","reasoning":false}
        ]}"#;
        let models = parse_oh_my_pi_models_json(output).expect("models");
        // The entry with an empty provider is skipped.
        assert_eq!(models.len(), 2);
        // Custom providers sort before the huggingface catalog.
        assert_eq!(models[0].id, "claude-opus-5");
        assert_eq!(models[0].provider_id.as_deref(), Some("xunmeng"));
        assert!(models[0].reasoning_options.is_empty());
        assert_eq!(models[1].id, "deepseek-ai/DeepSeek-R1");
        assert_eq!(models[1].provider_id.as_deref(), Some("huggingface"));
        assert!(!models[1].reasoning_options.is_empty());
    }

    #[test]
    fn parse_oh_my_pi_models_json_skips_leading_noise() {
        let output = "warning: catalog refresh failed\n{\"models\":[{\"provider\":\"openai\",\"id\":\"gpt-4o\",\"selector\":\"openai/gpt-4o\",\"reasoning\":false}]}";
        let models = parse_oh_my_pi_models_json(output).expect("models");
        assert_eq!(models.len(), 1);
        assert_eq!(models[0].id, "gpt-4o");
        assert_eq!(models[0].provider_id.as_deref(), Some("openai"));
    }

    #[test]
    fn parse_oh_my_pi_default_model_from_config_get() {
        // Exact envelope from `omp config get modelRoles --json`.
        let output = r#"{
  "key": "modelRoles",
  "value": {
    "default": "xunmeng/claude-opus-5"
  },
  "type": "record",
  "description": ""
}"#;
        assert_eq!(
            parse_oh_my_pi_default_model(output).as_deref(),
            Some("xunmeng/claude-opus-5")
        );
    }

    #[test]
    fn parse_oh_my_pi_default_model_accepts_bare_record() {
        // `omp config get modelRoles` (no --json) prints the bare record.
        let output = r#"{"default":"tokenpony/kimi-k3"}"#;
        assert_eq!(
            parse_oh_my_pi_default_model(output).as_deref(),
            Some("tokenpony/kimi-k3")
        );
    }

    #[test]
    fn parse_oh_my_pi_default_model_handles_missing_or_empty() {
        // No roles configured yet.
        assert!(parse_oh_my_pi_default_model(r#"{"key":"modelRoles","value":{}}"#).is_none());
        // Other roles set, but no default.
        assert!(parse_oh_my_pi_default_model(r#"{"value":{"smol":"a/b"}}"#).is_none());
        assert!(parse_oh_my_pi_default_model(r#"{"value":{"default":"  "}}"#).is_none());
        assert!(parse_oh_my_pi_default_model("Unknown setting: modelRoles").is_none());
    }

    #[test]
    fn parse_oh_my_pi_models_yaml_real_shape() {
        // Byte-for-byte the shape omp writes to ~/.omp/agent/models.yml.
        let raw = r#"providers:
  xunmeng:
    baseUrl: http://localhost:33000/v1
    api: openai-completions
    apiKey: sk-secret
    models:
      - id: claude-opus-5
      - id: gpt-5.6-sol
      - id: claude-opus-4-8
  tokenpony:
    baseUrl: https://api.tokenpony.cn/v1
    api: openai-completions
    apiKey: sk-other
    models:
      - id: glm-5.2
      - id: kimi-k2.7-code
      - id: kimi-k3
"#;
        let models = parse_oh_my_pi_models_yaml(raw).expect("models");
        assert_eq!(models.len(), 6);
        let ids: Vec<&str> = models.iter().map(|m| m.id.as_str()).collect();
        assert!(ids.contains(&"claude-opus-5"));
        assert!(ids.contains(&"kimi-k3"));
        // baseUrl/api/apiKey must never be mistaken for model entries.
        assert!(!ids.iter().any(|id| id.contains("baseUrl")));
        assert!(!ids.iter().any(|id| id.contains("apiKey")));
        assert!(
            !ids.iter()
                .any(|id| id.contains("api") && id.ends_with("completions"))
        );
        // Provider attribution stays correct across the provider boundary.
        let opus = models.iter().find(|m| m.id == "claude-opus-5").unwrap();
        assert_eq!(opus.provider_id.as_deref(), Some("xunmeng"));
    }

    #[test]
    fn parse_oh_my_pi_models_yaml_handles_bare_and_quoted_entries() {
        let raw = "providers:\n  local:\n    models:\n      - plain-model\n      - id: \"quoted-model\"\n";
        let models = parse_oh_my_pi_models_yaml(raw).expect("models");
        let ids: Vec<&str> = models.iter().map(|m| m.id.as_str()).collect();
        assert!(ids.contains(&"plain-model"));
        assert!(ids.contains(&"quoted-model"));
        assert!(
            models
                .iter()
                .all(|m| m.provider_id.as_deref() == Some("local"))
        );
    }

    #[test]
    fn parse_oh_my_pi_models_yaml_ignores_non_provider_sections() {
        // config.yml-style keys and comments must not yield models.
        let raw = "modelRoles:\n  default: xunmeng/claude-opus-5\nsetupVersion: 1\n";
        assert!(parse_oh_my_pi_models_yaml(raw).is_none());
        assert!(parse_oh_my_pi_models_yaml("# just a comment\n").is_none());
        assert!(parse_oh_my_pi_models_yaml("providers:\n").is_none());
    }

    #[test]
    fn parse_oh_my_pi_models_json_rejects_non_json() {
        assert!(parse_oh_my_pi_models_json("No models available.").is_none());
        assert!(parse_oh_my_pi_models_json("{\"models\":[]}").is_none());
    }
}

#[cfg(test)]
mod oh_my_pi_real_file_tests {
    use super::*;

    /// Parse the developer's actual models.yml when present. Skipped in CI where
    /// the file does not exist, so it never turns into a flaky failure.
    #[test]
    fn parses_real_models_yml_if_present() {
        let Some(path) = dirs::home_dir().map(|h| h.join(".omp/agent/models.yml")) else {
            return;
        };
        let Ok(raw) = std::fs::read_to_string(&path) else {
            return;
        };
        let models = parse_oh_my_pi_models_yaml(&raw)
            .unwrap_or_else(|| panic!("failed to parse real {}", path.display()));
        assert!(!models.is_empty());
        for m in &models {
            let provider = m.provider_id.as_deref().unwrap_or("");
            assert!(!provider.is_empty(), "model {} lost its provider", m.id);
            // Ids are provider-relative; do not embed the provider prefix.
            assert!(
                !m.id.starts_with(&format!("{provider}/")),
                "id should be bare, got {}",
                m.id
            );
            // Config keys must never leak in as models.
            for key in ["baseUrl", "apiKey", "api:", "models"] {
                assert!(!m.id.contains(key), "config key leaked into id: {}", m.id);
            }
        }
        eprintln!("parsed {} models from real models.yml", models.len());
    }
}

#[cfg(test)]
mod oh_my_pi_live_tests {
    use super::*;

    fn omp_installed() -> bool {
        workspace_utils::shell::resolve_executable_path_blocking("omp").is_some()
    }

    /// End-to-end: the configured default model must appear in the discovered
    /// list, otherwise the selector would show a value the user cannot pick.
    #[tokio::test]
    async fn configured_default_model_is_in_discovered_list() {
        if !omp_installed() {
            return;
        }
        let cmd = CmdOverrides::default();
        let Some(default_model) = oh_my_pi_default_model("omp", &cmd).await else {
            eprintln!("no modelRoles.default configured; skipping");
            return;
        };
        let models = discover_oh_my_pi_models("omp", &cmd)
            .await
            .expect("model discovery");
        let matched = models.iter().any(|m| {
            m.id == default_model
                || m.provider_id
                    .as_ref()
                    .is_some_and(|p| default_model == format!("{p}/{}", m.id))
        });
        eprintln!(
            "default={default_model} discovered={} match={matched}",
            models.len(),
        );
        assert!(
            matched,
            "configured default {default_model} missing from {} discovered models",
            models.len()
        );
    }

    /// `omp config path` must resolve to a real directory holding the config.
    #[tokio::test]
    async fn agent_dir_resolves_to_existing_config_dir() {
        if !omp_installed() {
            return;
        }
        let dir = oh_my_pi_agent_dir("omp", &CmdOverrides::default())
            .await
            .expect("agent dir");
        eprintln!("agent dir: {}", dir.display());
        assert!(dir.is_dir(), "{} is not a directory", dir.display());
        assert!(
            dir.join("config.yml").exists(),
            "no config.yml in {}",
            dir.display()
        );
    }

    #[test]
    fn test_parse_antigravity_models_output() {
        let json_input = r#"[{"id":"gemini-3.6-flash-high","name":"Gemini 3.6 Flash (High)"},{"id":"gemini-3.1-pro-high","name":"Gemini 3.1 Pro (High)"}]"#;
        let models = parse_antigravity_models_output(json_input).expect("models");
        assert_eq!(models.len(), 2);
        assert_eq!(models[0].id, "gemini-3.6-flash-high");

        let tsv = "Fetching available models...\ngemini-3.6-flash-high\tGemini 3.6 Flash (High)\nclaude-sonnet-4-6\tClaude Sonnet 4.6 (Thinking)\n";
        let models = parse_antigravity_models_output(tsv).expect("tsv models");
        assert_eq!(models.len(), 2);
        assert_eq!(models[0].id, "gemini-3.6-flash-high");
        assert_eq!(models[0].name, "Gemini 3.6 Flash (High)");
        assert_eq!(models[1].id, "claude-sonnet-4-6");

        let defaults = antigravity_default_models();
        assert!(defaults.iter().any(|m| m.id == ANTIGRAVITY_DEFAULT_MODEL));
    }
}

#[cfg(test)]
mod antigravity_live_tests {
    use super::*;

    #[tokio::test]
    async fn discover_antigravity_models_live() {
        if workspace_utils::shell::resolve_executable_path_blocking("agy").is_none() {
            return;
        }
        let t0 = std::time::Instant::now();
        let models = discover_antigravity_models("agy", &CmdOverrides::default())
            .await
            .expect("discover");
        eprintln!(
            "elapsed={:?} count={} first={:?}",
            t0.elapsed(),
            models.len(),
            models.first()
        );
        assert!(!models.is_empty());
        assert!(
            models
                .iter()
                .any(|m| m.id.contains("flash") || m.id.contains("gemini")),
            "unexpected models: {:?}",
            models.iter().map(|m| &m.id).collect::<Vec<_>>()
        );
        // Pipe-sticky grandchildren previously forced a 45s timeout; keep this snappy.
        assert!(
            t0.elapsed() < Duration::from_secs(30),
            "discovery took too long: {:?}",
            t0.elapsed()
        );
    }
}
