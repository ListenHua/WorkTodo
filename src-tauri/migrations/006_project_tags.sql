ALTER TABLE projects ADD COLUMN tags_json TEXT NOT NULL DEFAULT '[]';
PRAGMA user_version=6;
