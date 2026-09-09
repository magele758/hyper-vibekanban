CREATE TABLE project_workspaces (
    project_id   BLOB NOT NULL,
    workspace_id BLOB NOT NULL,
    created_at   TEXT NOT NULL DEFAULT (datetime('now', 'subsec')),
    PRIMARY KEY (project_id, workspace_id),
    FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE,
    FOREIGN KEY (workspace_id) REFERENCES workspaces(id) ON DELETE CASCADE
);

CREATE INDEX idx_project_workspaces_workspace_id
    ON project_workspaces(workspace_id);
