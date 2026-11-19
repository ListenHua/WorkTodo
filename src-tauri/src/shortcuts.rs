use crate::{
    desktop::{self, DesktopState},
    error::{AppError, AppResult},
    models::DesktopSettings,
};
use std::sync::atomic::Ordering;
use tauri::{AppHandle, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutEvent, ShortcutState};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    AddTask,
    ToggleWindow,
    Capture,
}
impl Action {
    fn label(self) -> &'static str {
        match self {
            Self::AddTask => "快捷添加待办",
            Self::ToggleWindow => "显示/隐藏窗口",
            Self::Capture => "快捷采集",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Binding {
    pub shortcut: Shortcut,
    pub action: Action,
}

pub fn parse(value: &str, action: Action) -> AppResult<Option<Binding>> {
    if value.trim().is_empty() {
        return Ok(None);
    }
    let invalid = || {
        AppError::new(
            "INVALID_SETTINGS",
            &format!(
                "{}快捷键无效，请使用带修饰键的组合或 F1–F20",
                action.label()
            ),
        )
    };
    if value.len() > 120 {
        return Err(invalid());
    }
    let shortcut = value.parse::<Shortcut>().map_err(|_| invalid())?;
    let function = shortcut
        .key
        .to_string()
        .strip_prefix('F')
        .and_then(|n| n.parse::<u8>().ok());
    if function.is_some_and(|n| n > 20)
        || (shortcut.mods.is_empty() && !matches!(function, Some(1..=20)))
    {
        return Err(invalid());
    }
    if shortcut == "CommandOrControl+C".parse::<Shortcut>().unwrap() {
        return Err(AppError::new(
            "SHORTCUT_CONFLICT",
            &format!("{}不能占用系统复制快捷键", action.label()),
        ));
    }
    Ok(Some(Binding { shortcut, action }))
}

pub fn bindings(settings: &DesktopSettings) -> AppResult<Vec<Binding>> {
    let mut result = Vec::new();
    for (value, action) in [
        (settings.shortcut.as_str(), Action::AddTask),
        (settings.toggle_shortcut.as_str(), Action::ToggleWindow),
    ] {
        if let Some(binding) = parse(value, action)? {
            result.push(binding);
        }
    }
    if settings.middle_capture {
        if let Some(shortcut) = desktop::capture_shortcut(settings)? {
            result.push(Binding {
                shortcut,
                action: Action::Capture,
            });
        }
    }
    for (index, binding) in result.iter().enumerate() {
        if let Some(other) = result[..index]
            .iter()
            .find(|other| other.shortcut == binding.shortcut)
        {
            return Err(AppError::new(
                "SHORTCUT_CONFLICT",
                &format!(
                    "{}与{}快捷键相同，请更换组合",
                    binding.action.label(),
                    other.action.label()
                ),
            ));
        }
    }
    Ok(result)
}

fn handle(app: &AppHandle, shortcut: &Shortcut, event: ShortcutEvent) {
    let state = app.state::<DesktopState>();
    if state.recording_shortcut.load(Ordering::SeqCst) {
        return;
    }
    let action = state
        .shortcut_bindings
        .lock()
        .unwrap()
        .iter()
        .find(|binding| binding.shortcut == *shortcut)
        .map(|binding| binding.action);
    match action {
        Some(Action::AddTask) if event.state == ShortcutState::Pressed => {
            if app
                .get_webview_window("main")
                .is_some_and(|window| window.is_focused().unwrap_or(false))
            {
                crate::tray::dispatch(app, crate::tray::TrayAction::QuickAddTask);
            } else {
                let _ = desktop::show_capture(app);
            }
        }
        Some(Action::ToggleWindow) if event.state == ShortcutState::Pressed => {
            crate::tray::toggle_main(app)
        }
        Some(Action::Capture) => desktop::handle_capture_shortcut(app, event),
        _ => {}
    }
}
fn register(app: &AppHandle, binding: &Binding) -> AppResult<()> {
    app.global_shortcut().on_shortcut(binding.shortcut, handle).map_err(|_| {
        AppError::new("SHORTCUT_CONFLICT", &format!("{}快捷键 {} 已被系统或其他应用占用，或当前系统不支持，请更换组合；原设置已保留", binding.action.label(), binding.shortcut))
    })
}

pub struct PreparedShortcuts {
    previous: Vec<Binding>,
    next: Vec<Binding>,
    added: Vec<Shortcut>,
}
impl PreparedShortcuts {
    pub fn rollback(&self, app: &AppHandle) {
        for shortcut in &self.added {
            let _ = app.global_shortcut().unregister(*shortcut);
        }
    }
    pub fn commit(self, app: &AppHandle) {
        let state = app.state::<DesktopState>();
        *state.capture_shortcut.lock().unwrap() = self
            .next
            .iter()
            .find(|binding| binding.action == Action::Capture)
            .map(|binding| binding.shortcut);
        *state.shortcut_bindings.lock().unwrap() = self.next.clone();
        for binding in self.previous {
            if !self
                .next
                .iter()
                .any(|next| next.shortcut == binding.shortcut)
            {
                let _ = app.global_shortcut().unregister(binding.shortcut);
            }
        }
    }
}
// Reuse shortcuts already owned by this app, including when two actions swap keys.
// Newly registered callbacks remain inactive until the settings have been saved.
fn prepare_changes(
    previous: Vec<Binding>,
    next: Vec<Binding>,
    mut register: impl FnMut(&Binding) -> AppResult<()>,
    mut unregister: impl FnMut(Shortcut),
) -> AppResult<PreparedShortcuts> {
    let mut added = Vec::new();
    for binding in &next {
        if previous.iter().any(|old| old.shortcut == binding.shortcut) {
            continue;
        }
        if let Err(error) = register(binding) {
            for shortcut in added {
                unregister(shortcut);
            }
            return Err(error);
        }
        added.push(binding.shortcut);
    }
    Ok(PreparedShortcuts {
        previous,
        next,
        added,
    })
}
pub fn prepare(app: &AppHandle, settings: &DesktopSettings) -> AppResult<PreparedShortcuts> {
    let previous = app
        .state::<DesktopState>()
        .shortcut_bindings
        .lock()
        .unwrap()
        .clone();
    prepare_changes(
        previous,
        bindings(settings)?,
        |binding| register(app, binding),
        |shortcut| {
            let _ = app.global_shortcut().unregister(shortcut);
        },
    )
}
pub fn prepare_capture(
    app: &AppHandle,
    settings: &DesktopSettings,
) -> AppResult<PreparedShortcuts> {
    let previous = app
        .state::<DesktopState>()
        .shortcut_bindings
        .lock()
        .unwrap()
        .clone();
    let mut next: Vec<_> = previous
        .iter()
        .copied()
        .filter(|binding| binding.action != Action::Capture)
        .collect();
    if let Some(shortcut) = desktop::capture_shortcut(settings)? {
        next.push(Binding {
            shortcut,
            action: Action::Capture,
        });
    }
    prepare_changes(
        previous,
        next,
        |binding| register(app, binding),
        |shortcut| {
            let _ = app.global_shortcut().unregister(shortcut);
        },
    )
}
// One unavailable shortcut must not prevent the other shortcuts from working on startup.
pub fn initialize(app: &AppHandle, settings: &DesktopSettings) {
    let state = app.state::<DesktopState>();
    let mut errors = Vec::new();
    let mut active = Vec::new();
    match bindings(settings) {
        Ok(bindings) => {
            for binding in bindings {
                match register(app, &binding) {
                    Ok(()) => active.push(binding),
                    Err(error) => errors.push(error.message),
                }
            }
        }
        Err(error) => errors.push(error.message),
    }
    *state.capture_shortcut.lock().unwrap() = active
        .iter()
        .find(|binding| binding.action == Action::Capture)
        .map(|binding| binding.shortcut);
    *state.shortcut_bindings.lock().unwrap() = active;
    *state.shortcut_error.lock().unwrap() = errors.join("；");
}

#[tauri::command]
pub fn shortcut_recording(window: tauri::WebviewWindow, active: bool) {
    if window.label() == "main" {
        let state = window.state::<DesktopState>();
        state.recording_shortcut.store(active, Ordering::SeqCst);
        state.keyboard_hold.lock().unwrap().reset();
    }
}
