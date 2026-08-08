use std::{
    collections::VecDeque,
    sync::{Arc, Mutex, RwLock},
};

use futures::{StreamExt, future};
use tokio::{
    sync::{broadcast, oneshot},
    task::JoinHandle,
};
use tokio_stream::wrappers::{BroadcastStream, errors::BroadcastStreamRecvError};

use crate::{log_msg::LogMsg, stream_lines::LinesStreamExt};

// 100 MB Limit
const HISTORY_BYTES: usize = 100000 * 1024;

#[derive(Clone)]
struct StoredMsg {
    msg: LogMsg,
    bytes: usize,
}

struct Inner {
    history: VecDeque<StoredMsg>,
    total_bytes: usize,
}

pub struct MsgStore {
    inner: RwLock<Inner>,
    sender: broadcast::Sender<LogMsg>,
    /// Optional one-shot used by executors (e.g. Cursor) to signal turn completion
    /// so the container can stop a process that does not exit on its own.
    /// `true` = success, `false` = failure.
    exit_notifier: Mutex<Option<oneshot::Sender<bool>>>,
}

impl Default for MsgStore {
    fn default() -> Self {
        Self::new()
    }
}

impl MsgStore {
    pub fn new() -> Self {
        let (sender, _) = broadcast::channel(100000);
        Self {
            inner: RwLock::new(Inner {
                history: VecDeque::with_capacity(32),
                total_bytes: 0,
            }),
            sender,
            exit_notifier: Mutex::new(None),
        }
    }

    /// Install a notifier that `signal_executor_exit` can fire exactly once.
    pub fn set_exit_notifier(&self, tx: oneshot::Sender<bool>) {
        *self.exit_notifier.lock().unwrap() = Some(tx);
    }

    /// Signal that the coding-agent turn finished. No-op if no notifier is set
    /// or it was already consumed.
    pub fn signal_executor_exit(&self, success: bool) {
        if let Some(tx) = self.exit_notifier.lock().unwrap().take() {
            let _ = tx.send(success);
        }
    }

    pub fn push(&self, msg: LogMsg) {
        let _ = self.sender.send(msg.clone()); // live listeners
        let bytes = msg.approx_bytes();

        let mut inner = self.inner.write().unwrap();
        while inner.total_bytes.saturating_add(bytes) > HISTORY_BYTES {
            // Never evict stream sentinels. Historic normalize_logs snapshots
            // history via stdout/stderr streams that end on `Finished`; if
            // large JsonPatch churn (e.g. Grok message streaming) evicts
            // `Finished` before a late subscriber snapshots, that normalizer
            // waits on the live broadcast forever and normalized-logs WS
            // never emits `finished` — UI stuck loading.
            let Some(front) = inner.history.front() else {
                break;
            };
            if matches!(front.msg, LogMsg::Finished | LogMsg::Ready) {
                if inner.history.len() < 2 {
                    break;
                }
                let Some(removed) = inner.history.remove(1) else {
                    break;
                };
                inner.total_bytes = inner.total_bytes.saturating_sub(removed.bytes);
            } else if let Some(front) = inner.history.pop_front() {
                inner.total_bytes = inner.total_bytes.saturating_sub(front.bytes);
            } else {
                break;
            }
        }
        inner.history.push_back(StoredMsg { msg, bytes });
        inner.total_bytes = inner.total_bytes.saturating_add(bytes);
    }

    // Convenience
    pub fn push_stdout<S: Into<String>>(&self, s: S) {
        self.push(LogMsg::Stdout(s.into()));
    }

    pub fn push_patch(&self, patch: json_patch::Patch) {
        self.push(LogMsg::JsonPatch(patch));
    }

    pub fn push_session_id(&self, session_id: String) {
        self.push(LogMsg::SessionId(session_id));
    }

    pub fn push_message_id(&self, id: String) {
        self.push(LogMsg::MessageId(id));
    }

    pub fn push_finished(&self) {
        self.push(LogMsg::Finished);
    }

    pub fn get_receiver(&self) -> broadcast::Receiver<LogMsg> {
        self.sender.subscribe()
    }

    pub fn get_history(&self) -> Vec<LogMsg> {
        self.inner
            .read()
            .unwrap()
            .history
            .iter()
            .map(|s| s.msg.clone())
            .collect()
    }

    /// History then live, as `LogMsg`.
    pub fn history_plus_stream(
        &self,
    ) -> futures::stream::BoxStream<'static, Result<LogMsg, std::io::Error>> {
        let (history, rx) = (self.get_history(), self.get_receiver());

        let hist = futures::stream::iter(history.into_iter().map(Ok::<_, std::io::Error>));
        let live = BroadcastStream::new(rx).filter_map(|res| async move {
            match res {
                Ok(msg) => Some(Ok(msg)),
                Err(BroadcastStreamRecvError::Lagged(n)) => {
                    tracing::error!(
                        skipped = n,
                        "MsgStore broadcast lagged. {n} messages dropped for this subscriber"
                    );
                    None
                }
            }
        });

        Box::pin(hist.chain(live))
    }

    pub fn stdout_chunked_stream(
        &self,
    ) -> futures::stream::BoxStream<'static, Result<String, std::io::Error>> {
        self.stdio_chunked_stream(true)
    }

    pub fn stdout_lines_stream(
        &self,
    ) -> futures::stream::BoxStream<'static, std::io::Result<String>> {
        let history = self.get_history();
        // Historic replay: OS pipe chunks are NOT one JSON line per Stdout message.
        // Reassemble by concatenating then splitting on newlines. Use a finite
        // snapshot (not live broadcast) so we never hang waiting for `Finished`.
        if history.iter().any(|m| matches!(m, LogMsg::Finished)) {
            let mut buf = String::new();
            for msg in history
                .into_iter()
                .take_while(|m| !matches!(m, LogMsg::Finished))
            {
                if let LogMsg::Stdout(s) = msg {
                    buf.push_str(&s);
                }
            }
            let lines: Vec<String> = buf
                .lines()
                .filter(|line| !line.is_empty())
                .map(|line| line.to_string())
                .collect();
            return futures::stream::iter(lines.into_iter().map(Ok)).boxed();
        }

        self.stdout_chunked_stream().lines()
    }

    pub fn stderr_chunked_stream(
        &self,
    ) -> futures::stream::BoxStream<'static, Result<String, std::io::Error>> {
        self.stdio_chunked_stream(false)
    }

    /// Emit stdout (`stdout=true`) or stderr chunks until `Finished`.
    ///
    /// If `Finished` is already present in history (historic log replay), return a
    /// **finite** stream over the history snapshot only. Attaching to the live
    /// broadcast after a missed/`Finished`-evicted snapshot leaves normalize_logs
    /// waiting forever — conversation history UI spins on loading.
    fn stdio_chunked_stream(
        &self,
        stdout: bool,
    ) -> futures::stream::BoxStream<'static, Result<String, std::io::Error>> {
        let history = self.get_history();
        let finished_in_history = history.iter().any(|m| matches!(m, LogMsg::Finished));

        if finished_in_history {
            let chunks: Vec<String> = history
                .into_iter()
                .take_while(|m| !matches!(m, LogMsg::Finished))
                .filter_map(|m| match (stdout, m) {
                    (true, LogMsg::Stdout(s)) => Some(s),
                    (false, LogMsg::Stderr(s)) => Some(s),
                    _ => None,
                })
                .collect();
            return futures::stream::iter(chunks.into_iter().map(Ok)).boxed();
        }

        self.history_plus_stream()
            .take_while(|res| future::ready(!matches!(res, Ok(LogMsg::Finished))))
            .filter_map(move |res| async move {
                match (stdout, res) {
                    (true, Ok(LogMsg::Stdout(s))) => Some(Ok(s)),
                    (false, Ok(LogMsg::Stderr(s))) => Some(Ok(s)),
                    _ => None,
                }
            })
            .boxed()
    }

    /// Forward a stream of typed log messages into this store.
    pub fn spawn_forwarder<S, E>(self: Arc<Self>, stream: S) -> JoinHandle<()>
    where
        S: futures::Stream<Item = Result<LogMsg, E>> + Send + 'static,
        E: std::fmt::Display + Send + 'static,
    {
        tokio::spawn(async move {
            tokio::pin!(stream);

            while let Some(next) = stream.next().await {
                match next {
                    Ok(msg) => self.push(msg),
                    Err(e) => self.push(LogMsg::Stderr(format!("stream error: {e}"))),
                }
            }
        })
    }
}
