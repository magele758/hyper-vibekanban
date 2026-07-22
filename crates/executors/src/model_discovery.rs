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

    let read_outputs = async {
        let mut stdout_buf = String::new();
        let mut stderr_buf = String::new();
        let (stdout_res, stderr_res) = tokio::join!(
            stdout.read_to_string(&mut stdout_buf),
            stderr.read_to_string(&mut stderr_buf)
        );
        stdout_res?;
        stderr_res?;
        Ok::<_, std::io::Error>((stdout_buf, stderr_buf))
    };

    let result = time::timeout(DISCOVERY_TIMEOUT, async {
        let outputs = read_outputs.await?;
        let status = child.inner().wait().await?;
        Ok::<_, std::io::Error>((outputs, status))
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

        let id = format!("{provider}/{model}");
        models.push(ModelInfo {
            id: id.clone(),
            name: id,
            provider_id: Some(provider),
            reasoning_options: if supports_thinking {
                pi_thinking_options()
            } else {
                vec![]
            },
        });
    }

    // Prefer user-configured providers (non-catalog) first for UX.
    models.sort_by(|a, b| {
        let a_hf = a.provider_id.as_deref() == Some("huggingface");
        let b_hf = b.provider_id.as_deref() == Some("huggingface");
        match (a_hf, b_hf) {
            (false, true) => std::cmp::Ordering::Less,
            (true, false) => std::cmp::Ordering::Greater,
            _ => a.id.cmp(&b.id),
        }
    });

    (!models.is_empty()).then_some(models)
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
    let raw = std::fs::read_to_string(path).ok()?;
    let value: serde_json::Value = serde_json::from_str(&raw).ok()?;
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
            let id = format!("{provider_id}/{model_id}");
            let supports_thinking = entry
                .get("reasoning")
                .and_then(|v| v.as_bool())
                .or_else(|| entry.get("thinking").and_then(|v| v.as_bool()))
                .unwrap_or(false);
            models.push(ModelInfo {
                id: id.clone(),
                name: id,
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
        // Custom providers first.
        assert_eq!(models[0].id, "tokenpony/kimi-k3");
        assert_eq!(models[0].provider_id.as_deref(), Some("tokenpony"));
        assert!(models[0].reasoning_options.is_empty());
        assert_eq!(models[1].id, "xunmeng/claude-opus-4-8");
        assert_eq!(models[2].id, "huggingface/deepseek-ai/DeepSeek-R1");
        assert!(!models[2].reasoning_options.is_empty());
    }

    #[test]
    fn parse_pi_list_models_rejects_help_text() {
        let output = "No models available. Use /login to log into a provider via OAuth or API key.";
        assert!(parse_pi_list_models(output).is_none());
    }
}
