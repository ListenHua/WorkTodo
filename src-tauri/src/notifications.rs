//! System notification permission and delivery, separate from reminder persistence.
use crate::error::{AppError, AppResult};
use serde::Serialize;
use std::sync::Mutex;
use tauri::{AppHandle, Manager};

#[cfg(target_os = "macos")]
mod macos;

#[derive(Default)]
pub struct NotificationState(pub Mutex<String>);

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NotificationStatus {
    pub permission: String,
    pub enabled: bool,
    pub error: String,
}

pub fn initialize(app: &AppHandle) -> AppResult<()> {
    app.manage(NotificationState::default());
    #[cfg(target_os = "macos")]
    macos::initialize(app);
    #[cfg(not(target_os = "macos"))]
    {
        use tauri::Emitter;
        use tauri_plugin_notification::NotificationExt;
        let action_app = app.clone();
        app.notification()
            .on_action(move |action| {
                if let Some(task) = action
                    .notification()
                    .and_then(|n| n.extra().get("taskId"))
                    .and_then(|v| v.as_str())
                {
                    crate::desktop::show_main(&action_app);
                    let _ = action_app.emit_to("main", "open-task", task);
                }
            })
            .map_err(|_| AppError::new("NOTIFICATION_ERROR", "无法监听系统通知"))?;
    }
    Ok(())
}

pub fn permission(app: &AppHandle, request: bool) -> AppResult<String> {
    #[cfg(target_os = "macos")]
    {
        let _ = app;
        macos::permission(request)
    }
    #[cfg(not(target_os = "macos"))]
    {
        use tauri_plugin_notification::NotificationExt;
        let state = if request {
            app.notification().request_permission()
        } else {
            app.notification().permission_state()
        }
        .map_err(|_| AppError::new("NOTIFICATION_ERROR", "无法读取系统通知权限"))?;
        Ok(match state {
            tauri::plugin::PermissionState::Granted => "granted",
            tauri::plugin::PermissionState::Denied => "denied",
            _ => "notDetermined",
        }
        .into())
    }
}

pub fn send(
    app: &AppHandle,
    id: &str,
    numeric_id: i32,
    title: &str,
    body: &str,
    task_id: Option<&str>,
) -> AppResult<()> {
    let result = (|| {
        if permission(app, false)? != "granted" {
            return Err(AppError::new(
                "NOTIFICATION_PERMISSION",
                "系统通知未开启，请允许项目工作台发送通知",
            ));
        }
        #[cfg(target_os = "macos")]
        {
            let _ = numeric_id;
            macos::send(id, title, body, task_id)
        }
        #[cfg(not(target_os = "macos"))]
        {
            use tauri_plugin_notification::NotificationExt;
            let mut builder = app
                .notification()
                .builder()
                .id(numeric_id)
                .title(title)
                .body(body)
                .extra("reminderId", id);
            if let Some(task) = task_id {
                builder = builder.extra("taskId", task);
            }
            builder
                .show()
                .map_err(|_| AppError::new("NOTIFICATION_ERROR", "系统未接受通知，请检查通知设置"))
        }
    })();
    if let Ok(mut error) = app.state::<NotificationState>().0.lock() {
        *error = result
            .as_ref()
            .err()
            .map(|e| e.message.clone())
            .unwrap_or_default();
    }
    result
}

pub fn open_settings(app: &AppHandle) -> AppResult<()> {
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    use tauri_plugin_opener::OpenerExt;
    #[cfg(target_os = "macos")]
    let target =
        "x-apple.systempreferences:com.apple.preference.notifications?id=com.projectmanage.desktop";
    #[cfg(target_os = "windows")]
    let target = "ms-settings:notifications";
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = app;
        Err(AppError::new(
            "NOTIFICATION_ERROR",
            "请在系统设置中打开项目工作台的通知权限",
        ))
    }
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    app.opener()
        .open_url(target, None::<&str>)
        .map_err(|_| AppError::new("NOTIFICATION_ERROR", "无法打开系统通知设置"))
}

/// A previous crash between intent and OS acknowledgement must not strand a reminder.
pub fn can_retry_delivery(value: &serde_json::Value, now: chrono::DateTime<chrono::Utc>) -> bool {
    match value["state"].as_str() {
        Some("sent") => false,
        Some("intent") => value["updatedAt"]
            .as_str()
            .and_then(|value| chrono::DateTime::parse_from_rfc3339(value).ok())
            .is_none_or(|updated| now.signed_duration_since(updated).num_seconds() >= 30),
        _ => true,
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, Utc};
    use serde_json::json;
    #[test]
    fn failed_and_interrupted_deliveries_can_retry_without_repeating_confirmed_ones() {
        let now = Utc::now();
        assert!(can_retry_delivery(&json!({"state":"failed"}), now));
        assert!(!can_retry_delivery(&json!({"state":"sent"}), now));
        assert!(!can_retry_delivery(
            &json!({"state":"intent","updatedAt":now.to_rfc3339()}),
            now
        ));
        assert!(can_retry_delivery(
            &json!({"state":"intent","updatedAt":(now-Duration::seconds(31)).to_rfc3339()}),
            now
        ));
        assert!(can_retry_delivery(
            &json!({"state":"intent","updatedAt":"invalid"}),
            now
        ));
    }
}
