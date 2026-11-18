use crate::{
    error::{AppError, AppResult},
    models::{DesktopSettings, Reminder},
};
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use uuid::Uuid;
pub fn settings(db: &Connection) -> AppResult<DesktopSettings> {
    let json: Option<String> = db
        .query_row(
            "SELECT value FROM settings WHERE key='desktop_settings'",
            [],
            |r| r.get(0),
        )
        .optional()?;
    match json {
        Some(json) => parse_settings(
            serde_json::from_str(&json)
                .map_err(|_| AppError::new("INVALID_SETTINGS", "桌面设置无效"))?,
        ),
        None => Ok(DesktopSettings::default()),
    }
}
pub fn parse_settings(mut value: serde_json::Value) -> AppResult<DesktopSettings> {
    use tauri_plugin_global_shortcut::Shortcut;
    let legacy = value.get("toggleShortcut").is_none();
    let capture = value["captureTrigger"]
        .as_str()
        .and_then(|key| key.parse::<Shortcut>().ok());
    let defaults = DesktopSettings::default();
    // Upgrade the built-in default while preserving existing custom capture bindings.
    if legacy
        && value["shortcut"] == "CommandOrControl+Shift+Space"
        && capture != defaults.shortcut.parse::<Shortcut>().ok()
    {
        value["shortcut"] = serde_json::Value::String(defaults.shortcut);
    }
    let mut settings: DesktopSettings = serde_json::from_value(value)
        .map_err(|_| AppError::new("INVALID_SETTINGS", "桌面设置无效"))?;
    // A newly introduced default must not invalidate a workspace that already uses that key.
    if legacy {
        let toggle = settings.toggle_shortcut.parse::<Shortcut>().ok();
        if toggle.is_some()
            && (toggle == settings.shortcut.parse::<Shortcut>().ok() || toggle == capture)
        {
            settings.toggle_shortcut.clear();
        }
    }
    Ok(settings)
}
pub fn validate_settings(input: &DesktopSettings) -> AppResult<()> {
    use tauri_plugin_global_shortcut::Shortcut;
    crate::shortcuts::bindings(input)?;
    let copy_shortcut: Shortcut = "CommandOrControl+C".parse().unwrap();
    if input
        .shortcut
        .parse::<Shortcut>()
        .is_ok_and(|quick| quick == copy_shortcut)
    {
        return Err(AppError::new(
            "SHORTCUT_CONFLICT",
            "快速添加不能占用系统复制快捷键",
        ));
    }
    #[cfg(target_os = "windows")]
    if crate::middle_capture::mouse_button(&input.capture_trigger).is_some_and(|button| button > 4)
    {
        return Err(AppError::new(
            "INVALID_SETTINGS",
            "Windows 仅支持系统识别的五个标准鼠标按键，请在鼠标驱动中映射后使用",
        ));
    }
    if crate::middle_capture::mouse_button(&input.capture_trigger).is_none() {
        if input.capture_trigger.len() > 120 {
            return Err(AppError::new("INVALID_SETTINGS", "采集快捷键过长"));
        }
        let shortcut = input.capture_trigger.parse::<Shortcut>().map_err(|_| {
            AppError::new(
                "INVALID_SETTINGS",
                "采集快捷键无效，请使用如 Alt+Shift+D 的组合",
            )
        })?;
        if shortcut == copy_shortcut {
            return Err(AppError::new(
                "SHORTCUT_CONFLICT",
                "采集不能占用系统复制快捷键，请更换组合",
            ));
        }
        let function_number = shortcut
            .key
            .to_string()
            .strip_prefix('F')
            .and_then(|key| key.parse::<u8>().ok());
        // F21–F24 have no keycode mapping in the current macOS hotkey implementation.
        if function_number.is_some_and(|number| number > 20) {
            return Err(AppError::new(
                "INVALID_SETTINGS",
                "功能键仅支持 F1–F20，请更换按键",
            ));
        }
        let function_key = matches!(function_number, Some(1..=20));
        if shortcut.mods.is_empty() && !function_key {
            return Err(AppError::new(
                "INVALID_SETTINGS",
                "采集快捷键需包含 Ctrl、Alt、Shift 或 Command/Win，或使用 F1–F20",
            ));
        }
        if [&input.shortcut, &input.toggle_shortcut]
            .iter()
            .any(|value| {
                value
                    .parse::<Shortcut>()
                    .is_ok_and(|global| global == shortcut)
            })
        {
            return Err(AppError::new(
                "SHORTCUT_CONFLICT",
                "采集快捷键不能与快捷添加待办或显示/隐藏窗口快捷键相同",
            ));
        }
    }

    if !(300..=3000).contains(&input.middle_hold_ms) || input.shortcut.len() > 120 {
        return Err(AppError::new(
            "INVALID_SETTINGS",
            "长按阈值需为 300–3000 毫秒",
        ));
    }
    Ok(())
}
pub fn save_settings(db: &Connection, input: &DesktopSettings) -> AppResult<()> {
    validate_settings(input)?;
    let json = serde_json::to_string(input)
        .map_err(|_| AppError::new("INVALID_SETTINGS", "无法保存设置"))?;
    db.execute(
        "INSERT OR REPLACE INTO settings(key,value) VALUES('desktop_settings',?1)",
        [json],
    )?;
    Ok(())
}
pub fn list(db: &Connection) -> AppResult<Vec<Reminder>> {
    let mut stmt=db.prepare("SELECT r.id,r.task_id,t.title,r.remind_at,r.state,r.read_at FROM reminders r JOIN tasks t ON t.id=r.task_id WHERE t.deleted_at IS NULL ORDER BY r.remind_at DESC")?;
    let rows = stmt
        .query_map([], |r| {
            Ok(Reminder {
                id: r.get(0)?,
                task_id: r.get(1)?,
                title: r.get(2)?,
                remind_at: r.get(3)?,
                state: r.get(4)?,
                read_at: r.get(5)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}
pub fn schedule(db: &Connection, task_id: &str, at: Option<&str>) -> AppResult<()> {
    let active:bool=db.query_row("SELECT EXISTS(SELECT 1 FROM tasks t LEFT JOIN projects p ON p.id=t.project_id WHERE t.id=?1 AND t.deleted_at IS NULL AND t.status IN ('todo','doing') AND (t.project_id IS NULL OR p.status='active'))",[task_id],|r|r.get(0))?;
    if !active {
        return Err(AppError::new(
            "TASK_UNAVAILABLE",
            "只有进行中项目的未完成待办可以设置提醒",
        ));
    }
    let Some(at) = at else { return Ok(()) };
    let at = DateTime::parse_from_rfc3339(at)
        .map_err(|_| AppError::new("INVALID_DATE", "提醒时间无效"))?
        .with_timezone(&Utc);
    if at <= Utc::now() {
        return Err(AppError::new("INVALID_DATE", "请选择未来的提醒时间"));
    }
    let at = at.to_rfc3339();
    let exists:bool=db.query_row("SELECT EXISTS(SELECT 1 FROM reminders WHERE task_id=?1 AND remind_at=?2 AND state='pending')",params![task_id,at],|r|r.get(0))?;
    if !exists {
        db.execute(
            "INSERT INTO reminders(id,task_id,remind_at) VALUES(?1,?2,?3)",
            params![Uuid::new_v4().to_string(), task_id, at],
        )?;
    }
    Ok(())
}
pub fn action(db: &Connection, id: &str, action: &str) -> AppResult<()> {
    match action {
        "read" => {
            db.execute(
                "UPDATE reminders SET read_at=?2 WHERE id=?1",
                params![id, Utc::now().to_rfc3339()],
            )?;
        }
        "dismiss" => {
            db.execute(
                "UPDATE reminders SET state='dismissed',read_at=?2 WHERE id=?1",
                params![id, Utc::now().to_rfc3339()],
            )?;
        }
        "retry" => {
            let available: bool = db.query_row(
                "SELECT EXISTS(SELECT 1 FROM reminders r JOIN tasks t ON t.id=r.task_id LEFT JOIN projects p ON p.id=t.project_id WHERE r.id=?1 AND t.deleted_at IS NULL AND t.status IN ('todo','doing') AND (t.project_id IS NULL OR p.status='active'))",
                [id], |row| row.get(0),
            )?;
            if !available {
                return Err(AppError::new(
                    "TASK_UNAVAILABLE",
                    "只有未完成待办可以再次提醒",
                ));
            }
            db.execute(
                "UPDATE reminders SET state='pending',read_at=NULL,sent_at=NULL WHERE id=?1",
                [id],
            )?;
        }
        "snooze" => {
            let task: String =
                db.query_row("SELECT task_id FROM reminders WHERE id=?1", [id], |r| {
                    r.get(0)
                })?;
            let transaction = db.unchecked_transaction()?;
            schedule(
                &transaction,
                &task,
                Some(&(Utc::now() + chrono::Duration::minutes(10)).to_rfc3339()),
            )?;
            transaction.execute(
                "UPDATE reminders SET state='dismissed',read_at=?2 WHERE id=?1",
                params![id, Utc::now().to_rfc3339()],
            )?;
            transaction.commit()?;
        }
        _ => return Err(AppError::new("INVALID_ACTION", "提醒操作无效")),
    };
    Ok(())
}
pub fn due(db: &Connection, now: &str) -> AppResult<Vec<(i32, Reminder)>> {
    let mut stmt=db.prepare("SELECT r.rowid,r.id,r.task_id,t.title,r.remind_at FROM reminders r JOIN tasks t ON t.id=r.task_id LEFT JOIN projects p ON p.id=t.project_id WHERE r.state='pending' AND julianday(r.remind_at)<=julianday(?1) AND t.deleted_at IS NULL AND t.status IN ('todo','doing') AND (t.project_id IS NULL OR p.status='active') ORDER BY julianday(r.remind_at) LIMIT 5")?;
    let rows = stmt
        .query_map([now], |r| {
            Ok((
                r.get(0)?,
                Reminder {
                    id: r.get(1)?,
                    task_id: r.get(2)?,
                    title: r.get(3)?,
                    remind_at: r.get(4)?,
                    state: "pending".into(),
                    read_at: None,
                },
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

#[cfg(test)]
mod capture_settings_tests {
    use super::*;
    #[test]
    fn legacy_defaults_upgrade_but_custom_disabled_and_explicit_bindings_are_retained() {
        let defaults = DesktopSettings::default();
        let old =
            parse_settings(serde_json::json!({"shortcut":"CommandOrControl+Shift+Space"})).unwrap();
        assert_eq!(old.shortcut, defaults.shortcut);
        assert_eq!(old.toggle_shortcut, defaults.toggle_shortcut);
        for value in ["Alt+Shift+F8", ""] {
            let loaded = parse_settings(serde_json::json!({"shortcut":value})).unwrap();
            assert_eq!(loaded.shortcut, value);
        }
        let conflict =
            parse_settings(serde_json::json!({"shortcut":defaults.toggle_shortcut})).unwrap();
        assert_eq!(conflict.shortcut, defaults.toggle_shortcut);
        assert!(conflict.toggle_shortcut.is_empty());
        assert!(validate_settings(&conflict).is_ok());
        let custom_capture = parse_settings(serde_json::json!({"shortcut":"CommandOrControl+Shift+Space", "captureTrigger":defaults.shortcut})).unwrap();
        assert_eq!(custom_capture.shortcut, "CommandOrControl+Shift+Space");
        assert!(validate_settings(&custom_capture).is_ok());
        let explicit = parse_settings(
            serde_json::json!({"shortcut":"CommandOrControl+Shift+Space", "toggleShortcut":""}),
        )
        .unwrap();
        assert_eq!(explicit.shortcut, "CommandOrControl+Shift+Space");
        assert_eq!(explicit.toggle_shortcut, "");
    }
    #[test]
    fn global_shortcuts_survive_portable_round_trip_and_cache_rebuild() {
        let directory = tempfile::tempdir().unwrap();
        let bootstrap = directory.path().join("app.db");
        crate::db::initialize(&bootstrap).unwrap();
        let root = directory.path().join("workspace");
        std::fs::create_dir(&root).unwrap();
        crate::portable::location::select(&bootstrap, &root, |_| true)
            .unwrap()
            .unwrap();
        let saved = DesktopSettings {
            shortcut: "Control+Alt+Shift+KeyK".into(),
            toggle_shortcut: "Control+Alt+Shift+KeyL".into(),
            ..DesktopSettings::default()
        };
        crate::portable::execute(&bootstrap, true, |db| save_settings(db, &saved)).unwrap();
        let config: serde_json::Value =
            serde_json::from_slice(&std::fs::read(root.join("config.json")).unwrap()).unwrap();
        assert_eq!(
            config["preferences"]["desktop"]["toggleShortcut"],
            saved.toggle_shortcut
        );
        assert_eq!(config["preferences"]["capture"]["shortcut"], saved.shortcut);
        let binding = crate::portable::binding(&bootstrap).unwrap().unwrap();
        std::fs::remove_file(crate::portable::cache_path(&bootstrap, &binding)).unwrap();
        crate::portable::execute(&bootstrap, false, |db| {
            let loaded = settings(db)?;
            assert_eq!(loaded.shortcut, saved.shortcut);
            assert_eq!(loaded.toggle_shortcut, saved.toggle_shortcut);
            Ok(())
        })
        .unwrap();
        let invalid = DesktopSettings {
            toggle_shortcut: "Alt+Control+Shift+K".into(),
            ..saved.clone()
        };
        assert!(
            crate::portable::execute(&bootstrap, true, |db| save_settings(db, &invalid)).is_err()
        );
        crate::portable::execute(&bootstrap, false, |db| {
            assert_eq!(settings(db)?.toggle_shortcut, saved.toggle_shortcut);
            Ok(())
        })
        .unwrap();
        let disabled = DesktopSettings {
            shortcut: "".into(),
            toggle_shortcut: "".into(),
            ..saved
        };
        crate::portable::execute(&bootstrap, true, |db| save_settings(db, &disabled)).unwrap();
        crate::portable::execute(&bootstrap, false, |db| {
            assert!(settings(db)?.shortcut.is_empty());
            assert!(settings(db)?.toggle_shortcut.is_empty());
            Ok(())
        })
        .unwrap();
    }
    #[test]
    fn legacy_settings_default_to_middle_and_custom_binding_survives_reopen() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("app.db");
        crate::db::initialize(&path).unwrap();
        let db = crate::db::connect(&path).unwrap();
        db.execute(
            "INSERT INTO settings(key,value) VALUES('desktop_settings',?1)",
            [r#"{"middleCapture":true,"middleHoldMs":800}"#],
        )
        .unwrap();
        let mut loaded = settings(&db).unwrap();
        assert_eq!(loaded.capture_trigger, "MouseMiddle");
        assert!(loaded.middle_capture);
        assert_eq!(loaded.middle_hold_ms, 800);
        loaded.capture_trigger = "Alt+Shift+D".into();
        save_settings(&db, &loaded).unwrap();
        drop(db);
        let loaded = settings(&crate::db::connect(&path).unwrap()).unwrap();
        assert_eq!(loaded.capture_trigger, "Alt+Shift+D");
        assert_eq!(loaded.middle_hold_ms, 800);
    }
    #[test]
    fn invalid_capture_settings_and_alias_conflicts_do_not_replace_the_previous_settings() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("app.db");
        crate::db::initialize(&path).unwrap();
        let db = crate::db::connect(&path).unwrap();
        let saved = DesktopSettings::default();
        save_settings(&db, &saved).unwrap();
        for trigger in [
            "",
            "D",
            "Shift",
            "Alt+UnknownKey",
            "MouseButton33",
            "MouseButton5",
            "F21",
            "F24",
            "CommandOrControl+C",
        ] {
            let input = DesktopSettings {
                capture_trigger: trigger.into(),
                ..saved.clone()
            };
            assert!(save_settings(&db, &input).is_err(), "{trigger}");
            assert_eq!(settings(&db).unwrap().capture_trigger, "MouseMiddle");
        }
        let conflict = DesktopSettings {
            shortcut: "Control+Alt+KeyD".into(),
            capture_trigger: "Alt+Ctrl+D".into(),
            ..saved
        };
        assert!(save_settings(&db, &conflict).is_err());
        assert_eq!(settings(&db).unwrap().capture_trigger, "MouseMiddle");
    }
    #[test]
    fn capture_accepts_mouse_buttons_modified_keys_and_standalone_function_keys() {
        for trigger in [
            "Control+Shift+D",
            "Alt+Digit1",
            "Super+Space",
            "F8",
            "F20",
            "MouseMiddle",
            "MouseRight",
            "MouseLeft",
            "MouseBack",
            "MouseForward",
        ] {
            let settings = DesktopSettings {
                capture_trigger: trigger.into(),
                ..DesktopSettings::default()
            };
            assert!(validate_settings(&settings).is_ok(), "{trigger}");
        }
    }
}

#[cfg(test)]
mod delivery_tests {
    use super::*;
    use crate::{db, models::CreateTask, services::tasks};
    #[test]
    fn retry_requeues_active_tasks_and_cannot_revive_completed_or_deleted_tasks() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("app.db");
        db::initialize(&path).unwrap();
        let db = db::connect(&path).unwrap();
        let task = tasks::create(
            &db,
            CreateTask {
                title: "重试提醒".into(),
                project_id: None,
                priority: "low".into(),
                plan_date: None,
            },
        )
        .unwrap();
        schedule(&db, &task.id, Some("2099-01-01T10:00:00Z")).unwrap();
        let reminder = list(&db).unwrap().remove(0);
        db.execute("UPDATE reminders SET state='sent',sent_at='2026-11-18T00:00:00Z',read_at='2026-11-18T00:00:00Z',remind_at='2026-11-01T00:00:00Z' WHERE id=?1", [&reminder.id]).unwrap();
        assert!(due(&db, &Utc::now().to_rfc3339()).unwrap().is_empty());
        action(&db, &reminder.id, "retry").unwrap();
        let ready = due(&db, &Utc::now().to_rfc3339()).unwrap();
        assert_eq!(ready.len(), 1);
        assert_eq!(ready[0].1.id, reminder.id);
        assert!(list(&db).unwrap()[0].read_at.is_none());
        tasks::set_completed(&db, &task.id, true).unwrap();
        assert!(due(&db, &Utc::now().to_rfc3339()).unwrap().is_empty());
        assert!(action(&db, &reminder.id, "retry").is_err());
        tasks::set_completed(&db, &task.id, false).unwrap();
        tasks::trash(&db, &task.id, false).unwrap();
        assert!(action(&db, &reminder.id, "retry").is_err());
        assert!(action(&db, "unknown-reminder", "retry").is_err());
    }
}
