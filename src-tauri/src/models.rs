use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceStatus {
    pub root_path: Option<String>,
    pub available: bool,
    pub pending_operations: i64,
    pub read_only: bool,
    pub storage_error: Option<String>,
    pub binding_id: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub id: String,
    pub name: String,
    pub description: String,
    pub color: String,
    pub status: String,
    pub directory_relpath: String,
    pub created_at: String,
    pub task_count: i64,
    pub completed_count: i64,
    pub links: Vec<ProjectLink>,
    pub tags: Vec<ProjectTag>,
    pub pinned: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectLink {
    pub id: String,
    pub name: String,
    pub url: String,
    #[serde(default)]
    pub icon: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon_data: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectTag {
    pub name: String,
    pub color: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectDirectoryMerge {
    pub path: String,
    pub fingerprint: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateProject {
    #[serde(default)]
    pub directory_merge: Option<ProjectDirectoryMerge>,
    pub name: String,
    pub description: String,
    pub color: String,
    #[serde(default)]
    pub links: Option<Vec<ProjectLink>>,
    #[serde(default)]
    pub tags: Option<Vec<ProjectTag>>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub pending_policy: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Task {
    pub id: String,
    pub project_id: Option<String>,
    pub title: String,
    pub status: String,
    pub priority: String,
    pub plan_date: Option<String>,
    pub description: String,
    pub due_at: Option<String>,
    pub deleted_at: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateTask {
    pub title: String,
    pub project_id: Option<String>,
    pub priority: String,
    pub plan_date: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectFile {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub relative_path: String,
    pub size: u64,
    pub modified_at: String,
    pub status: String,
    pub kind: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportProgress {
    pub copied: u64,
    pub total: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateTask {
    pub title: String,
    pub project_id: Option<String>,
    pub priority: String,
    pub plan_date: Option<String>,
    pub description: String,
    pub due_at: Option<String>,
    pub status: String,
    #[serde(default)]
    pub reminder_at: Option<String>,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Reminder {
    pub id: String,
    pub task_id: String,
    pub title: String,
    pub remind_at: String,
    pub state: String,
    pub read_at: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct DesktopSettings {
    pub close_to_tray: bool,
    pub shortcut: String,
    pub middle_capture: bool,
    pub middle_hold_ms: u64,
    pub capture_trigger: String,
    pub notifications: bool,
}
impl Default for DesktopSettings {
    fn default() -> Self {
        Self {
            close_to_tray: true,
            shortcut: "CommandOrControl+Shift+Space".into(),
            middle_capture: false,
            middle_hold_ms: 650,
            capture_trigger: "MouseMiddle".into(),
            notifications: true,
        }
    }
}
