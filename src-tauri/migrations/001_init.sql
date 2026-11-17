CREATE TABLE settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
CREATE TABLE projects (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    normalized_name TEXT NOT NULL UNIQUE,
    directory_relpath TEXT NOT NULL UNIQUE,
    description TEXT NOT NULL DEFAULT '',
    color TEXT NOT NULL DEFAULT '#356859',
    status TEXT NOT NULL DEFAULT 'active' CHECK(status IN ('active','completed','archived')),
    created_at TEXT NOT NULL
);
CREATE TABLE tasks (
    id TEXT PRIMARY KEY,
    project_id TEXT REFERENCES projects(id),
    title TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'todo' CHECK(status IN ('todo','doing','done','canceled')),
    priority TEXT NOT NULL DEFAULT 'none' CHECK(priority IN ('none','low','medium','high')),
    plan_date TEXT,
    created_at TEXT NOT NULL,
    completed_at TEXT
);
CREATE INDEX idx_tasks_project_status ON tasks(project_id, status);
-- Filesystem changes and SQLite cannot share a transaction. Keep an intent record.
CREATE TABLE filesystem_operations (
    id TEXT PRIMARY KEY,
    kind TEXT NOT NULL,
    target TEXT NOT NULL,
    state TEXT NOT NULL DEFAULT 'planned' CHECK(state IN ('planned','completed','needs_attention')),
    created_at TEXT NOT NULL
);
PRAGMA user_version = 1;
