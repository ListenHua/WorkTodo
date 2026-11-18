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
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preferences_survive_reopen_and_invalid_values_do_not_overwrite() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("app.db");
        crate::db::initialize(&path).unwrap();
        let db = crate::db::connect(&path).unwrap();
        assert_eq!(get(&db, "appearance").unwrap(), None);
        set(&db, "appearance", "system").unwrap();
        set(&db, "capture_draft", "本地草稿\n第二行").unwrap();
        assert!(set(&db, "appearance", "blue").is_err());
        assert!(set(&db, "storage_root", "/other").is_err());
        assert!(set(&db, "capture_draft", &"x".repeat(100_001)).is_err());
        set(&db, "changelog_seen", "1.0.0").unwrap();
        for invalid in ["", "1.0", "01.0.0", "1.0.0-beta.1", "1.0.999999", "../file"] {
            assert!(set(&db, "changelog_seen", invalid).is_err());
        }
        drop(db);
        let db = crate::db::connect(&path).unwrap();
        assert_eq!(get(&db, "appearance").unwrap().as_deref(), Some("system"));
        assert_eq!(
            get(&db, "changelog_seen").unwrap().as_deref(),
            Some("1.0.0")
        );
        assert_eq!(
            get(&db, "capture_draft").unwrap().as_deref(),
            Some("本地草稿\n第二行")
        );
    }
    #[test]
    fn project_sort_validates_before_overwriting() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("app.db");
        crate::db::initialize(&path).unwrap();
        let db = crate::db::connect(&path).unwrap();
        let id = uuid::Uuid::new_v4().to_string();
        let valid = serde_json::json!({"mode":"manual","order":[id]}).to_string();
        set(&db, "project_sort", &valid).unwrap();
        for value in [
            serde_json::json!({"mode":"unknown","order":[]}),
            serde_json::json!({"mode":"manual","order":[id,id]}),
            serde_json::json!({"mode":"manual","order":["../file"]}),
            serde_json::json!({"mode":"manual","order":"invalid"}),
            serde_json::json!({"mode":"manual","order":[],"extra":true}),
        ] {
            assert!(set(&db, "project_sort", &value.to_string()).is_err());
            assert_eq!(
                get(&db, "project_sort").unwrap().as_deref(),
                Some(valid.as_str())
            );
        }
        assert!(set(&db, "project_sort", &"x".repeat(512_001)).is_err());
        drop(db);
        let db = crate::db::connect(&path).unwrap();
        assert_eq!(
            get(&db, "project_sort").unwrap().as_deref(),
            Some(valid.as_str())
        );
    }
    #[test]
    fn project_sort_moves_with_the_folder_without_a_cache() {
        use crate::{db, portable, services::safe_fs};
        let temp = tempfile::tempdir().unwrap();
        let bootstrap = temp.path().join("first/app.db");
        std::fs::create_dir_all(bootstrap.parent().unwrap()).unwrap();
        db::initialize(&bootstrap).unwrap();
        let root = temp.path().join("workspace");
        std::fs::create_dir(&root).unwrap();
        portable::location::select(&bootstrap, &root).unwrap();
        let project = portable::execute(&bootstrap, true, |db| {
            crate::services::projects::create(
                db,
                crate::models::CreateProject {
                    name: "排序项目".into(),
                    description: String::new(),
                    color: "#1677FF".into(),
                    ..Default::default()
                },
            )
        })
        .unwrap();
        let saved = serde_json::json!({"mode":"manual","order":[project.id]}).to_string();
        portable::execute(&bootstrap, true, |db| set(db, "project_sort", &saved)).unwrap();
        let docs = portable::documents::load(&root).unwrap();
        assert_eq!(
            docs["config.json"]["preferences"]["extra"]["preference:project_sort"],
            saved
        );
        let second = temp.path().join("second/app.db");
        std::fs::create_dir_all(second.parent().unwrap()).unwrap();
        db::initialize(&second).unwrap();
        let copied = temp.path().join("copied");
        safe_fs::copy_tree(&root, &copied).unwrap();
        portable::open(&second, &copied).unwrap();
        portable::execute(&second, false, |db| {
            assert_eq!(get(db, "project_sort")?.as_deref(), Some(saved.as_str()));
            assert_eq!(crate::services::projects::list(db)?[0].id, project.id);
            Ok(())
        })
        .unwrap();
    }
}
