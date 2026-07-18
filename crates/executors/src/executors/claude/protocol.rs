use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::{ChildStdin, ChildStdout},
    sync::Mutex,
};
use tokio_util::sync::CancellationToken;

use super::types::{CLIMessage, ControlRequestType, ControlResponseMessage, ControlResponseType};
use crate::{
    approvals::ExecutorApprovalError,
    executors::{
        ExecutorError,
        claude::{
            client::ClaudeAgentClient,
            types::{Message, PermissionMode, SDKControlRequest, SDKControlRequestType},
        },
    },
};

/// Handles bidirectional control protocol communication
#[derive(Clone)]
pub struct ProtocolPeer {
    stdin: Arc<Mutex<Option<ChildStdin>>>,
    /// Set after the follow-up/user prompt has been written to stdin.
    ///
    /// Claude Code may emit an early `result` for orphaned background-task
    /// notifications during `--resume` (origin `task-notification`, often
    /// `num_turns: 0`) *before* it reads our user message. Previously we
    /// broke the read loop on any `result`, which dropped `ChildStdin` and
    /// sent EOF — so Claude exited with only SessionStart hooks visible and
    /// never processed the prompt. We only close stdin after the prompt has
    /// been sent (and a later `result` arrives), so Claude can drain the
    /// prompt and then exit cleanly.
    prompt_sent: Arc<AtomicBool>,
    stdin_closed: Arc<AtomicBool>,
}

impl ProtocolPeer {
    pub fn spawn(
        stdin: ChildStdin,
        stdout: ChildStdout,
        client: Arc<ClaudeAgentClient>,
        cancel: CancellationToken,
    ) -> Self {
        let peer = Self {
            stdin: Arc::new(Mutex::new(Some(stdin))),
            prompt_sent: Arc::new(AtomicBool::new(false)),
            stdin_closed: Arc::new(AtomicBool::new(false)),
        };

        let reader_peer = peer.clone();
        tokio::spawn(async move {
            if let Err(e) = reader_peer.read_loop(stdout, client, cancel).await {
                tracing::error!("Protocol reader loop error: {}", e);
            }
        });

        peer
    }

    async fn read_loop(
        &self,
        stdout: ChildStdout,
        client: Arc<ClaudeAgentClient>,
        cancel: CancellationToken,
    ) -> Result<(), ExecutorError> {
        let mut reader = BufReader::new(stdout);
        let mut buffer = String::new();
        let mut interrupt_sent = false;

        loop {
            buffer.clear();
            tokio::select! {
                biased;
                _ = cancel.cancelled(), if !interrupt_sent => {
                    interrupt_sent = true;
                    tracing::info!("Cancellation received in read_loop, sending interrupt to Claude");
                    if let Err(e) = self.interrupt().await {
                        tracing::warn!("Failed to send interrupt to Claude: {e}");
                    }
                    // Continue the loop to read Claude's response (it should send a result)
                }
                line_result = reader.read_line(&mut buffer) => {
                    match line_result {
                        Ok(0) => break, // EOF
                        Ok(_) => {
                            let line = buffer.trim();
                            if line.is_empty() {
                                continue;
                            }
                            client.log_message(line).await?;

                            // Parse and handle control messages
                            match serde_json::from_str::<CLIMessage>(line) {
                                Ok(CLIMessage::ControlRequest {
                                    request_id,
                                    request,
                                }) => {
                                    self.handle_control_request(&client, request_id, request)
                                        .await;
                                }
                                Ok(CLIMessage::Result(result)) => {
                                    self.on_result_message(&result).await;
                                }
                                _ => {}
                            }
                        }
                        Err(e) => {
                            tracing::error!("Error reading stdout: {}", e);
                            break;
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// After the user prompt is in-flight, close stdin on `result` so Claude
    /// leaves `-p` / stream-json mode. Ignore earlier results (e.g. resume
    /// task-notification drains) so we do not EOF before the prompt is written.
    async fn on_result_message(&self, result: &serde_json::Value) {
        if !self.prompt_sent.load(Ordering::SeqCst) {
            let origin = result
                .get("origin")
                .and_then(|o| o.get("kind"))
                .and_then(|k| k.as_str())
                .unwrap_or("unknown");
            let num_turns = result.get("num_turns").and_then(|t| t.as_u64());
            tracing::warn!(
                origin,
                num_turns,
                "Ignoring Claude result before user prompt was sent (keeping stdin open)"
            );
            return;
        }

        if let Err(e) = self.close_stdin().await {
            tracing::warn!("Failed to close Claude stdin after result: {e}");
        }
    }

    async fn close_stdin(&self) -> Result<(), ExecutorError> {
        if self.stdin_closed.swap(true, Ordering::SeqCst) {
            return Ok(());
        }
        let mut guard = self.stdin.lock().await;
        if let Some(mut stdin) = guard.take() {
            // Half-close the write side so Claude sees EOF and can exit after
            // finishing the current turn, while we keep reading stdout.
            if let Err(e) = stdin.shutdown().await {
                tracing::debug!("Claude stdin shutdown: {e}");
            }
        }
        Ok(())
    }

    async fn handle_control_request(
        &self,
        client: &Arc<ClaudeAgentClient>,
        request_id: String,
        request: ControlRequestType,
    ) {
        match request {
            ControlRequestType::CanUseTool {
                tool_name,
                input,
                permission_suggestions,
                blocked_paths: _,
                tool_use_id,
            } => {
                match client
                    .on_can_use_tool(tool_name, input, permission_suggestions, tool_use_id)
                    .await
                {
                    Ok(result) => {
                        if let Err(e) = self
                            .send_hook_response(request_id, serde_json::to_value(result).unwrap())
                            .await
                        {
                            tracing::error!("Failed to send permission result: {e}");
                        }
                    }
                    Err(ExecutorError::ExecutorApprovalError(ExecutorApprovalError::Cancelled)) => {
                    }
                    Err(e) => {
                        tracing::error!("Error in on_can_use_tool: {e}");
                        if let Err(e2) = self.send_error(request_id, e.to_string()).await {
                            tracing::error!("Failed to send error response: {e2}");
                        }
                    }
                }
            }
            ControlRequestType::HookCallback {
                callback_id,
                input,
                tool_use_id,
            } => {
                match client
                    .on_hook_callback(callback_id, input, tool_use_id)
                    .await
                {
                    Ok(hook_output) => {
                        if let Err(e) = self.send_hook_response(request_id, hook_output).await {
                            tracing::error!("Failed to send hook callback result: {e}");
                        }
                    }
                    Err(e) => {
                        tracing::error!("Error in on_hook_callback: {e}");
                        if let Err(e2) = self.send_error(request_id, e.to_string()).await {
                            tracing::error!("Failed to send error response: {e2}");
                        }
                    }
                }
            }
        }
    }

    pub async fn send_hook_response(
        &self,
        request_id: String,
        hook_output: serde_json::Value,
    ) -> Result<(), ExecutorError> {
        self.send_json(&ControlResponseMessage::new(ControlResponseType::Success {
            request_id,
            response: Some(hook_output),
        }))
        .await
    }

    /// Send error response to CLI
    async fn send_error(&self, request_id: String, error: String) -> Result<(), ExecutorError> {
        self.send_json(&ControlResponseMessage::new(ControlResponseType::Error {
            request_id,
            error: Some(error),
        }))
        .await
    }

    async fn send_json<T: serde::Serialize>(&self, message: &T) -> Result<(), ExecutorError> {
        let json = serde_json::to_string(message)?;
        let mut guard = self.stdin.lock().await;
        let stdin = guard.as_mut().ok_or_else(|| {
            ExecutorError::Io(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "Claude stdin already closed",
            ))
        })?;
        stdin.write_all(json.as_bytes()).await?;
        stdin.write_all(b"\n").await?;
        stdin.flush().await?;
        Ok(())
    }

    pub async fn send_user_message(&self, content: String) -> Result<(), ExecutorError> {
        let message = Message::new_user(content);
        self.send_json(&message).await?;
        // Mark only after a successful write so a failed send cannot cause a
        // premature stdin close on a later orphan result.
        self.prompt_sent.store(true, Ordering::SeqCst);
        Ok(())
    }

    pub async fn initialize(&self, hooks: Option<serde_json::Value>) -> Result<(), ExecutorError> {
        self.send_json(&SDKControlRequest::new(SDKControlRequestType::Initialize {
            hooks,
        }))
        .await
    }
    pub async fn interrupt(&self) -> Result<(), ExecutorError> {
        self.send_json(&SDKControlRequest::new(SDKControlRequestType::Interrupt {}))
            .await
    }

    pub async fn set_permission_mode(&self, mode: PermissionMode) -> Result<(), ExecutorError> {
        self.send_json(&SDKControlRequest::new(
            SDKControlRequestType::SetPermissionMode { mode },
        ))
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn early_result_without_prompt_keeps_stdin_open_flag() {
        let prompt_sent = AtomicBool::new(false);
        assert!(!prompt_sent.load(Ordering::SeqCst));
        prompt_sent.store(true, Ordering::SeqCst);
        assert!(prompt_sent.load(Ordering::SeqCst));
    }

    #[test]
    fn task_notification_result_shape_is_detectable() {
        let result = serde_json::json!({
            "type": "result",
            "subtype": "success",
            "num_turns": 0,
            "origin": { "kind": "task-notification" },
            "result": ""
        });
        let origin = result
            .get("origin")
            .and_then(|o| o.get("kind"))
            .and_then(|k| k.as_str());
        assert_eq!(origin, Some("task-notification"));
        assert_eq!(result.get("num_turns").and_then(|t| t.as_u64()), Some(0));
    }
}
