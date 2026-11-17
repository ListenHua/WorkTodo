//! Validated preferences; portable workspaces persist them via the JSON repository.
use crate::error::{AppError, AppResult};
use rusqlite::{Connection, OptionalExtension};

fn validate_key(key: &str) -> AppResult<()> {
    if !matches!(
        key,
        "appearance" | "capture_project" | "capture_draft" | "changelog_seen" | "strict_offline"
    ) {
        return Err(AppError::new("INVALID_PREFERENCE", "不支持的本地配置项"));
    }
    Ok(())
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
}
