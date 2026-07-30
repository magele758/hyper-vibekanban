//! Parser for Feishu chat commands.
//!
//! A Feishu binding used to be a one-shot trigger: every inbound message minted
//! a fresh Issue. That makes "here's some news — is it worth building?" awkward,
//! because the follow-up ("yes, do it in project X") lands as an unrelated Issue.
//!
//! Commands give the chat an explicit verb, so a conversation can triage
//! information first and only then open work.

/// A parsed chat instruction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FeishuCommand {
    /// `/feature <project>: <text>` — open an Issue in a named project.
    Feature {
        /// Project name / slug as typed; resolved against the org later.
        project: Option<String>,
        text: String,
    },
    /// `/approve [note]` — accept the pending proposal in this thread.
    Approve { note: Option<String> },
    /// `/reject [note]` — drop the pending proposal in this thread.
    Reject { note: Option<String> },
    /// `/help` — list supported commands.
    Help,
    /// No command prefix: plain conversation with the bound agent.
    Chat { text: String },
}

/// Text of the help reply, kept next to the parser so the two cannot drift.
pub const HELP_TEXT: &str = "可用指令：\n\
     • `/feature <项目名>: <需求>` — 在指定项目开一个 Issue 并开始开发\n\
     • `/feature <需求>` — 在本机器人绑定的默认项目开 Issue\n\
     • `/approve [备注]` — 同意当前线程里待确认的提案\n\
     • `/reject [备注]` — 放弃当前线程里待确认的提案\n\
     • `/help` — 显示本说明\n\
     直接发普通消息则是与 Agent 对话（不会立刻建 Issue）。";

/// Parse an inbound message body into a command.
///
/// Mentions are expected to be stripped by the caller. Unknown slash words fall
/// through to [`FeishuCommand::Chat`] so a message like "/usr/bin is missing"
/// is not silently swallowed.
pub fn parse_command(input: &str) -> FeishuCommand {
    let text = input.trim();
    let Some(rest) = text.strip_prefix('/') else {
        return FeishuCommand::Chat {
            text: text.to_string(),
        };
    };

    let (verb, tail) = match rest.split_once(char::is_whitespace) {
        Some((v, t)) => (v, t.trim()),
        None => (rest, ""),
    };

    match verb.to_ascii_lowercase().as_str() {
        "feature" | "feat" => {
            if tail.is_empty() {
                return FeishuCommand::Help;
            }
            let (project, body) = split_project_prefix(tail);
            if body.is_empty() {
                return FeishuCommand::Help;
            }
            FeishuCommand::Feature {
                project,
                text: body,
            }
        }
        "approve" | "ok" | "yes" | "同意" => FeishuCommand::Approve { note: opt(tail) },
        "reject" | "no" | "拒绝" => FeishuCommand::Reject { note: opt(tail) },
        "help" | "?" => FeishuCommand::Help,
        _ => FeishuCommand::Chat {
            text: text.to_string(),
        },
    }
}

/// Split a leading `project:` qualifier off a feature request.
///
/// Only treats the prefix as a project when it looks like a repo name: ASCII,
/// short, no spaces. Repo names are ASCII in practice, and requiring that keeps
/// a natural-language prefix like `修复: 登录失败` from being read as a project.
fn split_project_prefix(tail: &str) -> (Option<String>, String) {
    let Some((head, body)) = tail.split_once(':') else {
        return (None, tail.to_string());
    };
    let head = head.trim();
    let body = body.trim();

    let looks_like_project = !head.is_empty()
        && head.len() <= 64
        && head
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '/'));

    if looks_like_project && !body.is_empty() {
        (Some(head.to_string()), body.to_string())
    } else {
        (None, tail.to_string())
    }
}

fn opt(s: &str) -> Option<String> {
    let t = s.trim();
    (!t.is_empty()).then(|| t.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_text_is_chat() {
        assert_eq!(
            parse_command("看看这条新闻有没有价值"),
            FeishuCommand::Chat {
                text: "看看这条新闻有没有价值".into()
            }
        );
    }

    #[test]
    fn feature_with_project_prefix() {
        assert_eq!(
            parse_command("/feature hyper-vibekanban: 加一个导出按钮"),
            FeishuCommand::Feature {
                project: Some("hyper-vibekanban".into()),
                text: "加一个导出按钮".into(),
            }
        );
    }

    #[test]
    fn feature_without_project_uses_binding_default() {
        assert_eq!(
            parse_command("/feature 加一个导出按钮"),
            FeishuCommand::Feature {
                project: None,
                text: "加一个导出按钮".into(),
            }
        );
    }

    #[test]
    fn sentence_colon_is_not_mistaken_for_a_project() {
        assert_eq!(
            parse_command("/feature 修复: 登录失败"),
            FeishuCommand::Feature {
                project: None,
                text: "修复: 登录失败".into(),
            }
        );
    }

    #[test]
    fn approve_and_reject_capture_notes() {
        assert_eq!(
            parse_command("/approve 就按方案二做"),
            FeishuCommand::Approve {
                note: Some("就按方案二做".into())
            }
        );
        assert_eq!(
            parse_command("/reject"),
            FeishuCommand::Reject { note: None }
        );
    }

    #[test]
    fn unknown_slash_word_stays_chat() {
        assert_eq!(
            parse_command("/usr/bin/env 找不到"),
            FeishuCommand::Chat {
                text: "/usr/bin/env 找不到".into()
            }
        );
    }

    #[test]
    fn bare_feature_asks_for_help() {
        assert_eq!(parse_command("/feature"), FeishuCommand::Help);
        assert_eq!(parse_command("/help"), FeishuCommand::Help);
    }
}
