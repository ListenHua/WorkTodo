use crate::{
    commands::{AppState, IO_LOCK},
    error::{AppError, AppResult},
    middle_capture,
    models::DesktopSettings,
    services,
};
use std::{
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Mutex,
    },
    time::Duration,
};
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

#[derive(Default)]
pub struct DesktopState {
    pub pending_tray_actions: Mutex<Vec<crate::tray::TrayAction>>,
    pub flush_exit: AtomicBool,
    pub capture_ready: AtomicBool,
    pub shortcut_error: Mutex<String>,
    pub middle_error: Mutex<String>,
    pub listener_started: AtomicBool,
    pub middle_listening: AtomicBool,
    pub middle_control: middle_capture::HoldControl,
    pub capture_config: Mutex<CaptureConfig>,
    pub capture_epoch: AtomicU64,
    pub keyboard_hold: Mutex<middle_capture::KeyboardHold>,
    pub capture_shortcut: Mutex<Option<Shortcut>>,
    pub shortcut_bindings: Mutex<Vec<crate::shortcuts::Binding>>,
    pub recording_shortcut: AtomicBool,
    pub capture_busy: AtomicBool,
    pub selection: Mutex<Option<Capture>>,
    pub image: Mutex<Option<std::path::PathBuf>>,
}
#[derive(Clone, PartialEq, Eq)]
pub struct CaptureConfig {
    pub enabled: bool,
    pub trigger: String,
    pub hold_ms: u64,
}
impl Default for CaptureConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            trigger: "MouseMiddle".into(),
            hold_ms: 650,
        }
    }
}
impl From<&DesktopSettings> for CaptureConfig {
    fn from(settings: &DesktopSettings) -> Self {
        Self {
            enabled: settings.middle_capture,
            trigger: settings.capture_trigger.clone(),
            hold_ms: settings.middle_hold_ms,
        }
    }
}

pub fn capture_shortcut(settings: &DesktopSettings) -> AppResult<Option<Shortcut>> {
    if !settings.middle_capture
        || crate::middle_capture::mouse_button(&settings.capture_trigger).is_some()
    {
        return Ok(None);
    }
    settings
        .capture_trigger
        .parse::<Shortcut>()
        .map(Some)
        .map_err(|_| AppError::new("INVALID_SETTINGS", "采集快捷键无效"))
}

pub fn handle_capture_shortcut(
    app: &AppHandle,
    event: tauri_plugin_global_shortcut::ShortcutEvent,
) {
    let state = app.state::<DesktopState>();
    let epoch = state.capture_epoch.load(Ordering::SeqCst);
    let config = state.capture_config.lock().unwrap().clone();
    if !config.enabled || crate::middle_capture::mouse_button(&config.trigger).is_some() {
        return;
    }
    let mut hold = state.keyboard_hold.lock().unwrap();
    if state.capture_epoch.load(Ordering::SeqCst) != epoch {
        return;
    }
    match event.state {
        ShortcutState::Pressed => hold.press(std::time::Instant::now(), config.hold_ms),
        ShortcutState::Released => {
            let should_capture = hold.release(std::time::Instant::now());
            drop(hold);
            if should_capture {
                collect_capture(app.clone(), epoch, true);
            }
        }
    }
}
pub fn show_main(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}
pub fn show_capture(app: &AppHandle) -> AppResult<()> {
    if let Some(window) = app.get_webview_window("quick") {
        window
            .show()
            .map_err(|_| AppError::new("WINDOW_ERROR", "无法打开快速添加"))?;
        let _ = window.unminimize();
        let _ = window.set_focus();
    } else {
        let builder =
            WebviewWindowBuilder::new(app, "quick", WebviewUrl::App("index.html#/capture".into()));
        builder
            .title("快速添加")
            .inner_size(520., 570.)
            .min_inner_size(440., 480.)
            .center()
            .resizable(true)
            .build()
            .map_err(|_| AppError::new("WINDOW_ERROR", "无法创建快速添加窗口"))?;
    }
    Ok(())
}
pub fn initialize(app: &AppHandle) -> AppResult<()> {
    crate::tray::initialize(app)?;
    let path = app.state::<AppState>().db_path.clone();
    let settings = crate::portable::execute(&path, false, |db| services::reminders::settings(db))
        .unwrap_or_default();
    crate::shortcuts::initialize(app, &settings);
    crate::notifications::initialize(app)?;
    start_middle(app, &settings);
    let timer_app = app.clone();
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_secs(5));
        let Ok(_guard) = IO_LOCK.try_lock() else {
            continue;
        };
        let _ = crate::portable::execute(&path, true, |db| {
            if !services::reminders::settings(db).is_ok_and(|s| s.notifications) {
                return Ok(());
            }
            if let Ok(due) = services::reminders::due(db, &chrono::Utc::now().to_rfc3339()) {
                for (numeric_id, reminder) in due {
                    if !crate::portable::notification_delivery(&path, &reminder.id, None)? {
                        if crate::portable::notification_confirmed(&path, &reminder.id)? {
                            db.execute("UPDATE reminders SET state='sent',sent_at=?2 WHERE id=?1 AND state='pending'",rusqlite::params![reminder.id,chrono::Utc::now().to_rfc3339()])?;
                        }
                        continue;
                    }
                    crate::portable::notification_delivery(&path, &reminder.id, Some("intent"))?;

                    if crate::notifications::send(
                        &timer_app,
                        &reminder.id,
                        numeric_id,
                        "待办提醒",
                        &reminder.title,
                        Some(&reminder.task_id),
                    )
                    .is_ok()
                    {
                        crate::portable::notification_delivery(&path, &reminder.id, Some("sent"))?;
                        let _=db.execute("UPDATE reminders SET state='sent',sent_at=?2 WHERE id=?1 AND state='pending'",rusqlite::params![reminder.id,chrono::Utc::now().to_rfc3339()]);
                        let _ = timer_app.emit("reminders-changed", ());
                    } else {
                        crate::portable::notification_delivery(
                            &path,
                            &reminder.id,
                            Some("failed"),
                        )?;
                        let _ = timer_app.emit("reminders-changed", ());
                    }
                }
            }
            Ok(())
        });
    });
    Ok(())
}
#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Capture {
    pub error: Option<String>,
    pub text: String,
    pub has_image: bool,
}
pub fn clipboard(app: &AppHandle) -> AppResult<Capture> {
    let _guard = IO_LOCK
        .lock()
        .map_err(|_| AppError::new("WORKER_ERROR", "采集暂不可用"))?;
    let text = app.clipboard().read_text().unwrap_or_default();
    let image = app.clipboard().read_image().ok();
    let state = app.state::<DesktopState>();
    let mut slot = state
        .image
        .lock()
        .map_err(|_| AppError::new("CLIPBOARD_ERROR", "剪贴板暂不可用"))?;
    *slot = None;
    if let Some(image) = image {
        if image.rgba().len() > 80 * 1024 * 1024 {
            return Err(AppError::new(
                "FILE_TOO_LARGE",
                "剪贴板图片过大，请通过文件添加",
            ));
        }
        let buffer =
            image::RgbaImage::from_raw(image.width(), image.height(), image.rgba().to_vec())
                .ok_or_else(|| AppError::new("CLIPBOARD_ERROR", "图片数据无效"))?;
        let file = tempfile::Builder::new()
            .prefix("采集图片-")
            .suffix(".png")
            .tempfile()?;
        buffer
            .save_with_format(file.path(), image::ImageFormat::Png)
            .map_err(|_| AppError::new("CLIPBOARD_ERROR", "无法保存剪贴板图片"))?;
        if file.as_file().metadata()?.len() > services::files::IMAGE_LIMIT {
            return Err(AppError::new("FILE_TOO_LARGE", "剪贴板图片不能超过 20 MB"));
        }
        let bootstrap = &app.state::<AppState>().db_path;
        *slot = Some(
            match crate::portable::capture_image(bootstrap, file.path()) {
                Ok(path) => path,
                Err(error) => crate::portable::recovery::save(
                    bootstrap,
                    None,
                    Some(&text),
                    Some(file.path()),
                    &error.message,
                )?,
            },
        );
    }
    Ok(Capture {
        error: None,
        text: text.chars().take(10000).collect(),
        has_image: slot.is_some(),
    })
}
// Event callbacks do no database/clipboard work. Only the chosen trigger starts a capture.
pub fn start_middle(app: &AppHandle, settings: &DesktopSettings) {
    let state = app.state::<DesktopState>();
    let config = CaptureConfig::from(settings);
    let mut previous = state.capture_config.lock().unwrap();
    if *previous != config {
        state.capture_epoch.fetch_add(1, Ordering::SeqCst);
        state.keyboard_hold.lock().unwrap().reset();
        state.middle_control.release();
        *state.middle_error.lock().unwrap() = String::new();
        *previous = config.clone();
    }
    drop(previous);
    if let Err(error) = services::reminders::validate_settings(settings) {
        state.middle_control.configure(false, config.hold_ms);
        *state.middle_error.lock().unwrap() = error.message;
        return;
    }
    state
        .middle_control
        .configure(config.enabled, config.hold_ms);
    match crate::shortcuts::prepare_capture(app, settings) {
        Ok(change) => change.commit(app),
        Err(error) => {
            *state.middle_error.lock().unwrap() = error.message;
            return;
        }
    }
    if !config.enabled {
        return;
    }
    if !middle_capture::permissions().ready() {
        *state.middle_error.lock().unwrap() =
            "请先允许 WorkTodo 的辅助功能权限，再点击重新检测".into();
        return;
    }
    if crate::middle_capture::mouse_button(&config.trigger).is_none() {
        *state.middle_error.lock().unwrap() = String::new();
        return;
    }
    if state.listener_started.swap(true, Ordering::SeqCst) {
        return;
    }
    *state.middle_error.lock().unwrap() = String::new();
    let app = app.clone();
    std::thread::spawn(move || {
        let listen_app = app.clone();
        let result = middle_capture::listen(
            move |button, down| {
                let state = listen_app.state::<DesktopState>();
                let epoch = state.capture_epoch.load(Ordering::SeqCst);
                let config = state.capture_config.lock().unwrap().clone();
                if !config.enabled || crate::middle_capture::mouse_button(&config.trigger).is_none()
                {
                    return;
                }
                if button.is_none() {
                    state.middle_control.release();
                    return;
                }
                if button != crate::middle_capture::mouse_button(&config.trigger) {
                    return;
                }
                if !down {
                    state.middle_control.release();
                    return;
                }
                let Some((ticket, hold_ms)) = state.middle_control.press() else {
                    return;
                };
                let app = listen_app.clone();
                std::thread::spawn(move || {
                    std::thread::sleep(Duration::from_millis(hold_ms));
                    if app
                        .state::<DesktopState>()
                        .middle_control
                        .is_current(ticket)
                    {
                        collect_capture(app, epoch, false);
                    }
                });
            },
            || {
                app.state::<DesktopState>()
                    .middle_listening
                    .store(true, Ordering::SeqCst);
            },
        );
        let state = app.state::<DesktopState>();
        state.middle_control.release();
        state.middle_listening.store(false, Ordering::SeqCst);
        state.listener_started.store(false, Ordering::SeqCst);
        let config = state.capture_config.lock().unwrap().clone();
        if config.enabled && crate::middle_capture::mouse_button(&config.trigger).is_some() {
            *state.middle_error.lock().unwrap() = result
                .err()
                .unwrap_or_else(|| "鼠标监听已停止，请重新检测".into());
            let _ = app.emit_to("main", "middle-status-changed", ());
        }
    });
}

fn collect_capture(app: AppHandle, epoch: u64, keyboard: bool) {
    std::thread::spawn(move || {
        let state = app.state::<DesktopState>();
        let valid = || {
            state.middle_control.enabled() && state.capture_epoch.load(Ordering::SeqCst) == epoch
        };
        if !valid() {
            return;
        }
        if app
            .get_webview_window("quick")
            .is_some_and(|w| w.is_focused().unwrap_or(false))
        {
            return;
        }
        if state.capture_busy.swap(true, Ordering::SeqCst) {
            return;
        }
        let result = (|| -> Result<Capture, String> {
            if keyboard {
                // A key-up can arrive before Ctrl/Alt/Shift/Command are physically released.
                for _ in 0..80 {
                    if !valid() || !middle_capture::modifiers_down() {
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(25));
                }
                if middle_capture::modifiers_down() {
                    return Err("请松开快捷键的所有按键后重试".into());
                }
            }
            if !valid() {
                return Err("采集设置已变更".into());
            }
            if !middle_capture::permissions().ready() {
                return Err("采集权限不可用，请在设置中检查系统授权".into());
            }
            let revision = middle_capture::clipboard_revision();
            middle_capture::copy_selection()?;
            let mut changed = revision.is_none();
            for _ in 0..25 {
                std::thread::sleep(Duration::from_millis(40));
                if revision.is_none() || middle_capture::clipboard_revision() != revision {
                    changed = true;
                    break;
                }
            }
            if !changed {
                return Err("未能复制所选内容。请先选中文字；图片可先使用原应用的“复制图像”，再点击读取剪贴板。".into());
            }
            let capture = clipboard(&app).map_err(|e| e.message)?;
            if capture.text.trim().is_empty() && !capture.has_image {
                return Err("未读取到文字或图片，请确认当前应用支持复制所选内容".into());
            }
            Ok(capture)
        })();
        if !valid() {
            state.capture_busy.store(false, Ordering::SeqCst);
            return;
        }
        let capture = match result {
            Ok(capture) => {
                *state.middle_error.lock().unwrap() = String::new();
                capture
            }
            Err(error) => {
                *state.middle_error.lock().unwrap() = error.clone();
                Capture {
                    text: String::new(),
                    has_image: false,
                    error: Some(error),
                }
            }
        };
        *state.selection.lock().unwrap() = Some(capture);
        if let Err(error) = show_capture(&app) {
            *state.middle_error.lock().unwrap() = error.message;
        } else {
            let _ = app.emit_to("quick", "capture-selection", ());
        }
        state.capture_busy.store(false, Ordering::SeqCst);
    });
}

/// Rebind OS integrations after choosing another workspace or a failed durable settings write.
pub fn reload_runtime(app: &AppHandle) {
    let result = crate::portable::execute(&app.state::<AppState>().db_path, false, |db| {
        services::reminders::settings(db)
    });
    let Ok(settings) = result else {
        return;
    };
    let _ = app.global_shortcut().unregister_all();
    *app.state::<DesktopState>().capture_shortcut.lock().unwrap() = None;
    app.state::<DesktopState>()
        .shortcut_bindings
        .lock()
        .unwrap()
        .clear();
    crate::shortcuts::initialize(app, &settings);
    app.state::<DesktopState>()
        .capture_epoch
        .fetch_add(1, Ordering::SeqCst);
    *app.state::<DesktopState>().selection.lock().unwrap() = None;
    *app.state::<DesktopState>().image.lock().unwrap() = None;
    start_middle(app, &settings);
}
