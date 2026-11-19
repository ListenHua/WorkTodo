//! Validated preferences; portable workspaces persist them via the JSON repository.
use crate::error::{AppError, AppResult};
use rusqlite::{Connection, OptionalExtension};

fn validate_key(key: &str) -> AppResult<()> {
    if !matches!(
        key,
        "appearance"
            | "capture_project"
            | "capture_draft"
            | "changelog_seen"
            | "strict_offline"
            | "project_sort"
    ) {
        return Err(AppError::new("INVALID_PREFERENCE", "不支持的本地配置项"));
    }
    Ok(())
}
fn valid_project_sort(value: &str) -> bool {
    #[derive(serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    struct ProjectSort {
        mode: String,
        order: Vec<String>,
    }
    if value.len() > 512_000 {
        return false;
    }
    let Ok(sort) = serde_json::from_str::<ProjectSort>(value) else {
        return false;
    };
    let mut ids = std::collections::HashSet::new();
    matches!(
        sort.mode.as_str(),
        "manual" | "name-asc" | "name-desc" | "created-desc" | "created-asc"
    ) && sort.order.len() <= 10_000
        && sort
            .order
            .iter()
            .all(|id| uuid::Uuid::parse_str(id).is_ok() && ids.insert(id))
}
pub fn get(db: &Connection, key: &str) -> AppResult<Option<String>> {
    validate_key(key)?;
    Ok(db
        .query_row(
            "SELECT value FROM settings WHERE key=?1",
            [format!("preference:{key}")],
            |r| r.get(0),
        )
        .optional()?)
}
pub fn set(db: &Connection, key: &str, value: &str) -> AppResult<()> {
    validate_key(key)?;
    let valid = match key {
        "strict_offline" => matches!(value, "true" | "false"),
        "appearance" => matches!(value, "light" | "dark" | "system"),
        "capture_project" => value.is_empty() || uuid::Uuid::parse_str(value).is_ok(),
        "capture_draft" => value.len() <= 100_000,
        "project_sort" => valid_project_sort(value),
        "changelog_seen" => {
            let parts: Vec<_> = value.split('.').collect();
            parts.len() == 3
                && parts.iter().all(|part| {
                    !part.is_empty()
                        && (part.len() == 1 || !part.starts_with('0'))
                        && part.bytes().all(|byte| byte.is_ascii_digit())
                        && part.parse::<u16>().is_ok()
                })
        }
        _ => false,
    };
    if !valid {
        return Err(AppError::new(
            "INVALID_PREFERENCE",
            "本地配置内容无效或过长",
        ));
    }
    db.execute("INSERT INTO settings(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value", [format!("preference:{key}"),value.to_string()])?;
    Ok(())
}
