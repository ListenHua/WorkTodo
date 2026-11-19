use crate::{
    desktop::{show_main, DesktopState},
    error::{AppError, AppResult},
};
use serde::Serialize;
use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::TrayIconBuilder,
    AppHandle, Emitter, Manager, State, WebviewWindow,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum TrayAction {
    AddTask,
    QuickAddTask,
    ViewTasks,
    AddProject,
    ViewProjects,
    Settings,
}
impl TrayAction {
    fn from_id(id: &str) -> Option<Self> {
        match id {
            "tray-add-task" => Some(Self::AddTask),
            "tray-view-tasks" => Some(Self::ViewTasks),
            "tray-add-project" => Some(Self::AddProject),
            "tray-view-projects" => Some(Self::ViewProjects),
            "tray-settings" => Some(Self::Settings),
            _ => None,
        }
    }
}

// Queue before notifying so actions also survive a click during frontend startup.
// Only the main window drains this queue; the quick capture window cannot consume it.
#[tauri::command]
pub fn tray_take_actions(
    window: WebviewWindow,
    state: State<'_, DesktopState>,
) -> AppResult<Vec<TrayAction>> {
    if window.label() != "main" {
        return Err(AppError::new("WINDOW_ERROR", "请在主窗口执行托盘操作"));
    }
    let mut pending = state
        .pending_tray_actions
        .lock()
        .map_err(|_| AppError::new("TRAY_ERROR", "无法读取托盘操作"))?;
    Ok(std::mem::take(&mut *pending))
}

pub fn dispatch(app: &AppHandle, action: TrayAction) {
    app.state::<DesktopState>()
        .pending_tray_actions
        .lock()
        .unwrap()
        .push(action);
    show_main(app);
    let _ = app.emit_to("main", "tray-actions-available", ());
}

pub fn toggle_main(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        if window.is_visible().unwrap_or(false) && !window.is_minimized().unwrap_or(false) {
            let _ = window.hide();
        } else {
            show_main(app);
        }
    }
}

pub fn initialize(app: &AppHandle) -> AppResult<()> {
    let build = || -> tauri::Result<()> {
        let add_task = MenuItem::with_id(app, "tray-add-task", "添加待办", true, None::<&str>)?;
        let view_tasks = MenuItem::with_id(app, "tray-view-tasks", "查看待办", true, None::<&str>)?;
        let add_project =
            MenuItem::with_id(app, "tray-add-project", "添加项目", true, None::<&str>)?;
        let view_projects =
            MenuItem::with_id(app, "tray-view-projects", "查看项目", true, None::<&str>)?;
        let toggle = MenuItem::with_id(app, "tray-toggle", "显示/隐藏", true, None::<&str>)?;
        let settings = MenuItem::with_id(app, "tray-settings", "设置", true, None::<&str>)?;
        let quit = MenuItem::with_id(app, "tray-quit", "退出", true, None::<&str>)?;
        let menu = Menu::with_items(
            app,
            &[
                &add_task,
                &view_tasks,
                &add_project,
                &view_projects,
                &PredefinedMenuItem::separator(app)?,
                &toggle,
                &settings,
                &PredefinedMenuItem::separator(app)?,
                &quit,
            ],
        )?;
        let pixels = image::load_from_memory(include_bytes!("../icons/tray.png"))
            .expect("bundled tray icon must be a valid PNG")
            .into_rgba8();
        let (width, height) = pixels.dimensions();
        TrayIconBuilder::with_id("workspace-tray")
            .tooltip("WorkTodo")
            .icon(tauri::image::Image::new_owned(
                pixels.into_raw(),
                width,
                height,
            ))
            // macOS uses the alpha mask to keep the mark legible on either menu bar theme.
            .icon_as_template(true)
            .menu(&menu)
            .show_menu_on_left_click(true)
            .on_menu_event(|app, event| {
                let id = event.id.as_ref();
                if let Some(action) = TrayAction::from_id(id) {
                    dispatch(app, action);
                } else {
                    match id {
                        "tray-toggle" => toggle_main(app),
                        "tray-quit" => app.exit(0),
                        _ => {}
                    }
                }
            })
            .build(app)?;
        Ok(())
    };
    build().map_err(|_| AppError::new("TRAY_ERROR", "无法创建托盘菜单或图标"))
}
