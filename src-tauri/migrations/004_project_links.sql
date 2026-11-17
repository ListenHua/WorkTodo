ALTER TABLE projects ADD COLUMN links_json TEXT NOT NULL DEFAULT '[]';
PRAGMA user_version = 4;
