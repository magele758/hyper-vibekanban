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
    model_selector::{ModelInfo, ReasoningOption},
};

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
}
