CREATE TABLE IF NOT EXISTS session_queued_messages (
    id BLOB PRIMARY KEY NOT NULL,
    session_id BLOB NOT NULL,
    position INTEGER NOT NULL,
    message TEXT NOT NULL,
    executor_config TEXT NOT NULL,
    queued_at TEXT NOT NULL DEFAULT (datetime('now', 'subsec')),
    FOREIGN KEY (session_id) REFERENCES sessions(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_session_queued_messages_session_position
    ON session_queued_messages (session_id, position);
