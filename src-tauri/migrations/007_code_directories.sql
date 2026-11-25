ALTER TABLE projects ADD COLUMN code_directories_json TEXT NOT NULL DEFAULT '[]';
PRAGMA user_version=7;
