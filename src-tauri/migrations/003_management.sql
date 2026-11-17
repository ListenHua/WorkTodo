ALTER TABLE tasks ADD COLUMN description TEXT NOT NULL DEFAULT '';
ALTER TABLE tasks ADD COLUMN due_at TEXT;
ALTER TABLE tasks ADD COLUMN deleted_at TEXT;
ALTER TABLE tasks ADD COLUMN updated_at TEXT;
ALTER TABLE filesystem_operations ADD COLUMN detail TEXT;
INSERT INTO projects(id,name,normalized_name,directory_relpath,status,created_at)
VALUES('__inbox__','_收件箱','_收件箱','_收件箱','active',strftime('%Y-%m-%dT%H:%M:%fZ','now'));
CREATE TABLE task_files (
 task_id TEXT NOT NULL REFERENCES tasks(id),
 file_id TEXT NOT NULL REFERENCES project_files(id),
 PRIMARY KEY(task_id,file_id)
);
CREATE TABLE reminders (
 id TEXT PRIMARY KEY,
 task_id TEXT NOT NULL REFERENCES tasks(id),
 remind_at TEXT NOT NULL,
 state TEXT NOT NULL DEFAULT 'pending' CHECK(state IN ('pending','sent','dismissed')),
 read_at TEXT,
 sent_at TEXT
);
CREATE INDEX idx_reminders_due ON reminders(state,remind_at);
PRAGMA user_version = 3;
