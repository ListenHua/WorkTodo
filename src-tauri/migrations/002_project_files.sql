CREATE TABLE project_files (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id),
    relative_path TEXT NOT NULL,
    size INTEGER NOT NULL CHECK(size >= 0),
    modified_at TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'available' CHECK(status IN ('available','missing')),
    UNIQUE(project_id, relative_path)
);
CREATE INDEX idx_project_files_project ON project_files(project_id, status);
PRAGMA user_version = 2;
