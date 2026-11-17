use crate::{
    error::{AppError, AppResult},
    models::*,
    services,
};
use rusqlite::Connection;
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};
use tauri::ipc::{Channel, Response};
use tauri::{AppHandle, State};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};
use tauri_plugin_opener::OpenerExt;

pub static IO_LOCK: Mutex<()> = Mutex::new(());

pub struct AppState {
    pub db_path: PathBuf,
    pub imports: Mutex<HashMap<String, Arc<AtomicBool>>>,
}

async fn blocking<T, F>(path: PathBuf, operation: F) -> AppResult<T>
where
    T: Send + 'static,
    F: FnOnce(&mut Connection) -> AppResult<T> + Send + 'static,
{
    let expected = crate::portable::binding(&path)?.map(|b| b.id);
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = IO_LOCK
            .lock()
            .map_err(|_| AppError::new("WORKER_ERROR", "本地操作暂不可用，请重启应用"))?;
        if crate::portable::binding(&path)?.map(|b| b.id) != expected {
            return Err(AppError::new(
                "WORKSPACE_CHANGED",
                "工作目录已切换，请重新打开当前内容",
            ));
        }
        crate::portable::execute(&path, true, operation)
    })
    .await
    .map_err(|_| AppError::new("WORKER_ERROR", "后台操作中断，请重试"))?
}

async fn blocking_read<T, F>(path: PathBuf, operation: F) -> AppResult<T>
where
    T: Send + 'static,
    F: FnOnce(&mut Connection) -> AppResult<T> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = IO_LOCK
            .lock()
            .map_err(|_| AppError::new("WORKER_ERROR", "本地操作暂不可用"))?;
        crate::portable::execute(&path, false, operation)
    })
    .await
    .map_err(|_| AppError::new("WORKER_ERROR", "后台操作中断"))?
}

#[tauri::command]
pub async fn workspace_status(state: State<'_, AppState>) -> AppResult<WorkspaceStatus> {
    blocking_read(state.db_path.clone(), |db| services::workspace::status(db)).await
}

fn confirm_storage_replacement(app: &AppHandle, names: &[String]) -> bool {
    let mut list = names
        .iter()
        .take(20)
        .map(|name| format!("• {name}"))
        .collect::<Vec<_>>()
        .join("\n");
    if names.len() > 20 {
        list.push_str(&format!("\n另有 {} 项", names.len() - 20));
    }
    app.dialog()
        .message(format!("所选文件夹中存在同名内容：\n\n{list}\n\n是否替换？同名文件夹将整体替换，其中原有内容会被覆盖；其他内容保留。取消后当前存储位置与数据不变。"))
        .title("替换同名内容")
        .kind(MessageDialogKind::Warning)
        .buttons(MessageDialogButtons::OkCancelCustom("替换".into(), "取消".into()))
        .blocking_show()
}

#[tauri::command]
pub async fn workspace_select_root(
    app: AppHandle,
    state: State<'_, AppState>,
) -> AppResult<WorkspaceStatus> {
    let dialog_app = app.clone();
    let selection = tauri::async_runtime::spawn_blocking(move || {
        dialog_app
            .dialog()
            .file()
            .set_title("选择项目文件存储位置")
            .blocking_pick_folder()
    })
    .await
    .map_err(|_| AppError::new("WORKER_ERROR", "目录选择中断"))?;
    let selected = selection
        .map(|s| {
            s.into_path()
                .map_err(|_| AppError::new("INVALID_DIRECTORY", "无法读取选择的本地目录"))
        })
        .transpose()?;
    let bootstrap = state.db_path.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = IO_LOCK
            .lock()
            .map_err(|_| AppError::new("WORKER_ERROR", "目录操作暂不可用"))?;
        match selected {
            Some(path) => {
                match crate::portable::location::select(&bootstrap, &path, |names| {
                    confirm_storage_replacement(&app, names)
                })? {
                    Some(result) => {
                        crate::desktop::reload_runtime(&app);
                        Ok(result)
                    }
                    None => crate::portable::execute(&bootstrap, false, |db| {
                        services::workspace::status(db)
                    }),
                }
            }
            None => {
                crate::portable::execute(&bootstrap, false, |db| services::workspace::status(db))
            }
        }
    })
    .await
    .map_err(|_| AppError::new("WORKER_ERROR", "目录操作中断"))?
}

#[tauri::command]
pub async fn projects_list(state: State<'_, AppState>) -> AppResult<Vec<Project>> {
    blocking_read(state.db_path.clone(), |db| services::projects::list(db)).await
}

#[tauri::command]
pub async fn projects_set_pinned(
    state: State<'_, AppState>,
    id: String,
    pinned: bool,
) -> AppResult<()> {
    blocking(state.db_path.clone(), move |db| {
        services::projects::set_pinned(db, &id, pinned)
    })
    .await
}

#[tauri::command]
pub async fn projects_create(
    state: State<'_, AppState>,
    input: CreateProject,
) -> AppResult<Project> {
    blocking(state.db_path.clone(), move |db| {
        services::projects::create(db, input)
    })
    .await
}

#[tauri::command]
pub async fn tasks_list(state: State<'_, AppState>) -> AppResult<Vec<Task>> {
    blocking_read(state.db_path.clone(), |db| services::tasks::list(db)).await
}

#[tauri::command]
pub async fn tasks_create(state: State<'_, AppState>, input: CreateTask) -> AppResult<Task> {
    blocking(state.db_path.clone(), move |db| {
        services::tasks::create(db, input)
    })
    .await
}

#[tauri::command]
pub async fn tasks_set_completed(
    state: State<'_, AppState>,
    id: String,
    completed: bool,
) -> AppResult<()> {
    blocking(state.db_path.clone(), move |db| {
        services::tasks::set_completed(db, &id, completed)
    })
    .await
}

#[tauri::command]
pub async fn projects_open_folder(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
) -> AppResult<()> {
    blocking_read(state.db_path.clone(), move |db| {
        let folder = services::projects::folder(db, &id)?;
        app.opener()
            .open_path(folder.to_string_lossy(), None::<&str>)
            .map_err(|_| {
                AppError::new(
                    "OPEN_DIRECTORY_FAILED",
                    "无法打开项目文件夹，请检查系统默认文件管理器",
                )
            })
    })
    .await
}

#[tauri::command]
pub async fn files_list(
    state: State<'_, AppState>,
    project_id: String,
) -> AppResult<Vec<ProjectFile>> {
    blocking_read(state.db_path.clone(), move |db| {
        services::files::list(db, &project_id)
    })
    .await
}

#[tauri::command]
pub async fn files_scan(
    state: State<'_, AppState>,
    project_id: String,
) -> AppResult<Vec<ProjectFile>> {
    blocking(state.db_path.clone(), move |db| {
        services::files::scan(db, &project_id)
    })
    .await
}

#[tauri::command]
pub async fn files_pick(app: AppHandle) -> AppResult<Vec<String>> {
    tauri::async_runtime::spawn_blocking(move || {
        app.dialog()
            .file()
            .set_title("添加项目文件")
            .blocking_pick_files()
            .unwrap_or_default()
            .into_iter()
            .map(|file| {
                file.into_path()
                    .map(|path| path.to_string_lossy().into_owned())
                    .map_err(|_| AppError::new("INVALID_FILE", "无法读取所选文件位置"))
            })
            .collect()
    })
    .await
    .map_err(|_| AppError::new("WORKER_ERROR", "文件选择中断，请重试"))?
}

#[tauri::command]
pub async fn files_import(
    state: State<'_, AppState>,
    project_id: String,
    source: String,
    request_id: String,
    on_progress: Channel<ImportProgress>,
) -> AppResult<ProjectFile> {
    if uuid::Uuid::parse_str(&request_id).is_err() {
        return Err(AppError::new("INVALID_REQUEST", "无效的文件操作"));
    }
    let canceled = Arc::new(AtomicBool::new(false));
    {
        let mut imports = state
            .imports
            .lock()
            .map_err(|_| AppError::new("WORKER_ERROR", "文件操作暂不可用"))?;
        if imports.contains_key(&request_id) {
            return Err(AppError::new("DUPLICATE_REQUEST", "该文件正在添加"));
        }
        imports.insert(request_id.clone(), canceled.clone());
    }
    let result = blocking(state.db_path.clone(), move |db| {
        services::files::import_file(
            db,
            &project_id,
            &PathBuf::from(source),
            &canceled,
            |copied, total| {
                let _ = on_progress.send(ImportProgress { copied, total });
            },
        )
    })
    .await;
    if let Ok(mut imports) = state.imports.lock() {
        imports.remove(&request_id);
    }
    result
}

#[tauri::command]
pub fn files_cancel_import(state: State<'_, AppState>, request_id: String) -> AppResult<()> {
    if let Some(canceled) = state
        .imports
        .lock()
        .map_err(|_| AppError::new("WORKER_ERROR", "无法取消文件操作"))?
        .get(&request_id)
    {
        canceled.store(true, Ordering::Relaxed);
    }
    Ok(())
}

#[tauri::command]
pub async fn files_delete(
    state: State<'_, AppState>,
    project_id: String,
    id: String,
) -> AppResult<&'static str> {
    blocking(state.db_path.clone(), move |db| {
        services::files::delete(db, &project_id, &id)
    })
    .await
}

#[tauri::command]
pub async fn files_preview(
    state: State<'_, AppState>,
    project_id: String,
    id: String,
) -> AppResult<Response> {
    let bytes = blocking_read(state.db_path.clone(), move |db| {
        services::files::preview(db, &project_id, &id)
    })
    .await?;
    Ok(Response::new(bytes))
}

#[tauri::command]
pub async fn files_open(
    app: AppHandle,
    state: State<'_, AppState>,
    project_id: String,
    id: String,
    reveal: bool,
) -> AppResult<()> {
    blocking_read(state.db_path.clone(), move |db| {
        let path = services::files::resolve(db, &project_id, &id)?;
        if reveal {
            app.opener()
                .reveal_item_in_dir(&path)
                .map_err(|_| AppError::new("OPEN_FILE_FAILED", "无法在文件管理器中定位文件"))
        } else {
            if !services::files::may_open(&path) {
                return Err(AppError::new("OPEN_UNSUPPORTED", "此格式请在文件夹中打开"));
            }
            app.opener()
                .open_path(path.to_string_lossy(), None::<&str>)
                .map_err(|_| AppError::new("OPEN_FILE_FAILED", "无法打开文件，请检查默认程序"))
        }
    })
    .await
}

#[tauri::command]
pub async fn tasks_update(
    state: State<'_, AppState>,
    id: String,
    input: UpdateTask,
) -> AppResult<Task> {
    blocking(state.db_path.clone(), move |db| {
        services::tasks::update(db, &id, input)
    })
    .await
}
#[tauri::command]
pub async fn tasks_trash(
    state: State<'_, AppState>,
    id: String,
    restore: bool,
    to_inbox: Option<bool>,
) -> AppResult<()> {
    blocking(state.db_path.clone(), move |db| {
        if restore {
            services::tasks::restore(db, &id, to_inbox.unwrap_or(false))
        } else {
            services::tasks::trash(db, &id, false)
        }
    })
    .await
}

#[tauri::command]
pub async fn projects_update(
    state: State<'_, AppState>,
    id: String,
    input: CreateProject,
) -> AppResult<Project> {
    blocking(state.db_path.clone(), move |db| {
        services::projects::update(db, &id, input)
    })
    .await
}
#[tauri::command]
pub async fn projects_set_status(
    state: State<'_, AppState>,
    id: String,
    status: String,
    policy: String,
) -> AppResult<()> {
    blocking(state.db_path.clone(), move |db| {
        services::projects::set_status(db, &id, &status, &policy)
    })
    .await
}

#[tauri::command]
pub async fn attachments_list(
    state: State<'_, AppState>,
    task_id: String,
) -> AppResult<Vec<ProjectFile>> {
    blocking_read(state.db_path.clone(), move |db| {
        services::attachments::list(db, &task_id)
    })
    .await
}
#[tauri::command]
pub async fn attachments_link(
    state: State<'_, AppState>,
    task_id: String,
    file_id: String,
    remove: bool,
) -> AppResult<()> {
    blocking(state.db_path.clone(), move |db| {
        if remove {
            services::attachments::unlink(db, &task_id, &file_id)
        } else {
            services::attachments::link(db, &task_id, &file_id)
        }
    })
    .await
}
#[tauri::command]
pub async fn attachments_import(
    state: State<'_, AppState>,
    task_id: String,
    source: String,
) -> AppResult<ProjectFile> {
    blocking(state.db_path.clone(), move |db| {
        services::attachments::import(db, &task_id, std::path::Path::new(&source))
    })
    .await
}
#[tauri::command]
pub async fn data_pending(
    state: State<'_, AppState>,
) -> AppResult<Vec<services::data::PendingOperation>> {
    blocking_read(state.db_path.clone(), move |db| services::data::pending(db)).await
}
#[tauri::command]
pub async fn data_recover(state: State<'_, AppState>, id: String) -> AppResult<()> {
    blocking(state.db_path.clone(), move |db| {
        services::data::recover(db, &id)
    })
    .await
}
#[tauri::command]
pub async fn data_action(
    app: AppHandle,
    state: State<'_, AppState>,
    action: String,
) -> AppResult<Option<String>> {
    let dialog = app.clone();
    let dialog_action = action.clone();
    let selection = tauri::async_runtime::spawn_blocking(move || {
        dialog
            .dialog()
            .file()
            .set_title(match dialog_action.as_str() {
                "backup" => "选择备份保存位置",
                "restore" => "选择项目工作台备份文件夹",
                "import" => "选择要导入为副本的项目文件夹",
                _ => "选择新的存储文件夹",
            })
            .blocking_pick_folder()
            .map(|s| s.into_path())
    })
    .await
    .map_err(|_| AppError::new("WORKER_ERROR", "目录选择中断"))?;
    let Some(selected) = selection else {
        return Ok(None);
    };
    let selected = selected.map_err(|_| AppError::new("INVALID_DIRECTORY", "目录无效"))?;
    // Determine the action again through an explicit argument, not a path sent by the webview.
    if action == "restore" {
        let dialog = app.clone();
        let target = tauri::async_runtime::spawn_blocking(move || {
            dialog
                .dialog()
                .file()
                .set_title("选择恢复文件的空文件夹")
                .blocking_pick_folder()
                .map(|s| s.into_path())
        })
        .await
        .map_err(|_| AppError::new("WORKER_ERROR", "目录选择中断"))?;
        let Some(target) = target else {
            return Ok(None);
        };
        let target = target.map_err(|_| AppError::new("INVALID_DIRECTORY", "目录无效"))?;
        let bootstrap = state.db_path.clone();
        return tauri::async_runtime::spawn_blocking(move || {
            let _guard = IO_LOCK
                .lock()
                .map_err(|_| AppError::new("WORKER_ERROR", "数据操作暂不可用"))?;
            // Keep current data intact; restore only into a new empty directory.
            let path = crate::portable::transfer::restore(&bootstrap, &selected, &target)?;
            crate::desktop::reload_runtime(&app);
            Ok(Some(path.to_string_lossy().into_owned()))
        })
        .await
        .map_err(|_| AppError::new("WORKER_ERROR", "恢复中断"))?;
    }
    let bootstrap = state.db_path.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = IO_LOCK
            .lock()
            .map_err(|_| AppError::new("WORKER_ERROR", "数据操作暂不可用"))?;
        let path = match action.as_str() {
            "backup" => crate::portable::transfer::backup(&bootstrap, &selected)?,
            "migrate" => {
                if crate::portable::location::select(&bootstrap, &selected, |names| {
                    confirm_storage_replacement(&app, names)
                })?
                .is_none()
                {
                    return Ok(None);
                }
                selected
            }
            "import" => crate::portable::transfer::import_project(&bootstrap, &selected)?,
            _ => return Err(AppError::new("INVALID_ACTION", "无效的数据操作")),
        };
        if action == "migrate" {
            crate::desktop::reload_runtime(&app);
        }
        Ok(Some(path.to_string_lossy().into_owned()))
    })
    .await
    .map_err(|_| AppError::new("WORKER_ERROR", "数据操作中断"))?
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopStatus {
    pub settings: DesktopSettings,
    pub autostart: bool,
    pub shortcut_error: String,
    pub middle_error: String,
    pub middle_permission: bool,
    pub input_monitoring_permission: bool,
    pub middle_listening: bool,
    pub is_macos: bool,
}
#[tauri::command]
pub async fn desktop_status(
    app: AppHandle,
    state: State<'_, AppState>,
) -> AppResult<DesktopStatus> {
    use tauri::Manager;
    use tauri_plugin_autostart::ManagerExt;
    let runtime = app.state::<crate::desktop::DesktopState>();
    let shortcut_error = runtime.shortcut_error.lock().unwrap().clone();
    let middle_error = runtime.middle_error.lock().unwrap().clone();
    let autostart = app.autolaunch().is_enabled().unwrap_or(false);
    let permissions = crate::middle_capture::permissions();
    let mouse_listening = runtime.middle_listening.load(Ordering::SeqCst);
    let registered_capture = *runtime.capture_shortcut.lock().unwrap();
    blocking_read(state.db_path.clone(), move |db| {
        let settings = services::reminders::settings(db)?;
        let middle_listening = services::reminders::validate_settings(&settings).is_ok()
            && if crate::middle_capture::mouse_button(&settings.capture_trigger).is_some() {
                mouse_listening
            } else {
                settings.middle_capture
                    && crate::desktop::capture_shortcut(&settings)? == registered_capture
                    && registered_capture.is_some()
            };
        Ok(DesktopStatus {
            settings,
            autostart,
            shortcut_error,
            middle_error,
            middle_permission: permissions.accessibility,
            input_monitoring_permission: permissions.input_monitoring,
            middle_listening,
            is_macos: cfg!(target_os = "macos"),
        })
    })
    .await
}
#[tauri::command]
pub fn middle_permission(app: AppHandle, kind: String) -> AppResult<()> {
    crate::middle_capture::open_permission(&app, &kind)
}
#[tauri::command]
pub async fn middle_retry(app: AppHandle, state: State<'_, AppState>) -> AppResult<()> {
    blocking(state.db_path.clone(), move |db| {
        crate::desktop::start_middle(&app, &services::reminders::settings(db)?);
        Ok(())
    })
    .await
}
#[tauri::command]
pub async fn desktop_save(
    app: AppHandle,
    state: State<'_, AppState>,
    input: DesktopSettings,
) -> AppResult<()> {
    use tauri::Manager;
    use tauri_plugin_global_shortcut::GlobalShortcutExt;
    let rollback_app = app.clone();
    let result = blocking(state.db_path.clone(), move |db| {
        let previous = services::reminders::settings(db)?;
        use tauri_plugin_global_shortcut::Shortcut;
        services::reminders::validate_settings(&input)?;
        let next_quick = if input.shortcut.is_empty() {
            None
        } else {
            Some(
                input
                    .shortcut
                    .parse::<Shortcut>()
                    .map_err(|_| AppError::new("SHORTCUT_CONFLICT", "快速添加快捷键无效"))?,
            )
        };
        let old_quick = previous.shortcut.parse::<Shortcut>().ok();
        let registered_quick = next_quick.is_some_and(|next| {
            old_quick != Some(next) || !app.global_shortcut().is_registered(next)
        });
        if registered_quick {
            crate::desktop::register_shortcut(&app, &input.shortcut)?;
        }
        let capture_change = match crate::desktop::prepare_capture_shortcut(&app, &input) {
            Ok(change) => change,
            Err(error) => {
                if registered_quick {
                    let _ = app.global_shortcut().unregister(next_quick.unwrap());
                }
                return Err(error);
            }
        };
        if let Err(error) = services::reminders::save_settings(db, &input) {
            capture_change.rollback(&app);
            if registered_quick {
                let _ = app.global_shortcut().unregister(next_quick.unwrap());
            }
            return Err(error);
        }
        capture_change.commit(&app);
        if old_quick != next_quick {
            if let Some(old) = old_quick {
                let _ = app.global_shortcut().unregister(old);
            }
        }
        *app.state::<crate::desktop::DesktopState>()
            .shortcut_error
            .lock()
            .unwrap() = String::new();
        crate::desktop::start_middle(&app, &input);
        Ok(())
    })
    .await;
    if result.is_err() {
        crate::desktop::reload_runtime(&rollback_app);
    }
    result
}
#[tauri::command]
pub async fn desktop_autostart(
    app: AppHandle,
    state: State<'_, AppState>,
    enabled: bool,
) -> AppResult<()> {
    use tauri_plugin_autostart::ManagerExt;
    let previous = app.autolaunch().is_enabled().unwrap_or(false);
    (if enabled {
        app.autolaunch().enable()
    } else {
        app.autolaunch().disable()
    })
    .map_err(|_| AppError::new("AUTOSTART_ERROR", "无法修改开机启动设置，请检查系统权限"))?;
    let result = blocking(state.db_path.clone(), move |db| {
        db.execute(
            "INSERT OR REPLACE INTO settings(key,value) VALUES('autostart_desired',?1)",
            [if enabled { "true" } else { "false" }],
        )?;
        Ok(())
    })
    .await;
    if result.is_err() {
        let _ = if previous {
            app.autolaunch().enable()
        } else {
            app.autolaunch().disable()
        };
    }
    result
}

#[tauri::command]
pub fn notification_test(app: AppHandle) -> AppResult<()> {
    use tauri_plugin_notification::NotificationExt;
    app.notification()
        .request_permission()
        .map_err(|_| AppError::new("NOTIFICATION_ERROR", "无法请求通知权限"))?;
    app.notification()
        .builder()
        .title("项目工作台")
        .body("通知测试成功")
        .show()
        .map_err(|_| AppError::new("NOTIFICATION_ERROR", "无法提交系统通知，请检查系统通知设置"))
}
#[tauri::command]
pub async fn reminders_list(state: State<'_, AppState>) -> AppResult<Vec<Reminder>> {
    blocking_read(state.db_path.clone(), |db| services::reminders::list(db)).await
}
#[tauri::command]
pub async fn reminders_schedule(
    state: State<'_, AppState>,
    task_id: String,
    at: Option<String>,
) -> AppResult<()> {
    blocking(state.db_path.clone(), move |db| {
        services::reminders::schedule(db, &task_id, at.as_deref())
    })
    .await
}
#[tauri::command]
pub async fn reminders_action(
    state: State<'_, AppState>,
    id: String,
    action: String,
) -> AppResult<()> {
    blocking(state.db_path.clone(), move |db| {
        services::reminders::action(db, &id, &action)
    })
    .await
}
#[tauri::command]
pub async fn capture_clipboard(app: AppHandle) -> AppResult<crate::desktop::Capture> {
    tauri::async_runtime::spawn_blocking(move || crate::desktop::clipboard(&app))
        .await
        .map_err(|_| AppError::new("WORKER_ERROR", "剪贴板读取中断"))?
}
#[tauri::command]
pub fn capture_selection(app: AppHandle) -> AppResult<Option<crate::desktop::Capture>> {
    use tauri::Manager;
    app.state::<crate::desktop::DesktopState>()
        .capture_ready
        .store(true, Ordering::SeqCst);
    let selection = app
        .state::<crate::desktop::DesktopState>()
        .selection
        .lock()
        .unwrap()
        .take();
    if selection.is_some() {
        return Ok(selection);
    }
    let _guard = IO_LOCK
        .lock()
        .map_err(|_| AppError::new("WORKER_ERROR", "草稿操作暂不可用"))?;
    let image = crate::portable::saved_capture_image(&app.state::<AppState>().db_path)?;
    let has_image = image.is_some();
    *app.state::<crate::desktop::DesktopState>()
        .image
        .lock()
        .unwrap() = image;
    Ok(has_image.then_some(crate::desktop::Capture {
        text: String::new(),
        has_image,
        error: None,
    }))
}
#[tauri::command]
pub fn capture_image(app: AppHandle) -> AppResult<Response> {
    use tauri::Manager;
    let state = app.state::<crate::desktop::DesktopState>();
    let slot = state.image.lock().unwrap();
    let file = slot
        .as_ref()
        .ok_or_else(|| AppError::new("IMAGE_NOT_FOUND", "采集图片已失效，请重新读取"))?;
    Ok(Response::new(std::fs::read(file)?))
}
#[tauri::command]
pub async fn capture_save(
    app: AppHandle,
    state: State<'_, AppState>,
    text: String,
    project_id: Option<String>,
    include_image: bool,
    request_id: String,
    binding_id: Option<String>,
) -> AppResult<Task> {
    use tauri::{Emitter, Manager};
    let action_app = app.clone();
    let bootstrap = state.db_path.clone();
    let task = blocking(state.db_path.clone(), move |db| {
        crate::portable::require_binding(&bootstrap, binding_id.as_deref())?;
        if services::tasks::list(db)?
            .iter()
            .any(|task| task.id == request_id)
        {
            return crate::portable::save_capture(db, &text, project_id, None, &request_id);
        }
        let image = if include_image {
            let state = action_app.state::<crate::desktop::DesktopState>();
            let image = state.image.lock().unwrap().clone();
            Some(image.ok_or_else(|| AppError::new("IMAGE_NOT_FOUND", "图片已失效，请重新读取"))?)
        } else {
            None
        };
        crate::portable::save_capture(db, &text, project_id, image.as_deref(), &request_id)
    })
    .await?;
    *app.state::<crate::desktop::DesktopState>()
        .image
        .lock()
        .unwrap() = None;
    let _ = app.emit("workspace-changed", ());
    Ok(task)
}
#[tauri::command]
pub fn capture_discard(app: AppHandle, binding_id: Option<String>) -> AppResult<()> {
    use tauri::Manager;
    let _guard = IO_LOCK
        .lock()
        .map_err(|_| AppError::new("WORKER_ERROR", "草稿操作暂不可用"))?;
    let bootstrap = &app.state::<AppState>().db_path;
    crate::portable::require_binding(bootstrap, binding_id.as_deref())?;
    crate::portable::discard_image(bootstrap)?;
    *app.state::<crate::desktop::DesktopState>()
        .image
        .lock()
        .unwrap() = None;
    Ok(())
}

#[tauri::command]
pub async fn tasks_create_full(state: State<'_, AppState>, input: UpdateTask) -> AppResult<Task> {
    blocking(state.db_path.clone(), move |db| {
        services::tasks::create_full(db, input)
    })
    .await
}

#[tauri::command]
pub async fn preference_get(state: State<'_, AppState>, key: String) -> AppResult<Option<String>> {
    blocking_read(state.db_path.clone(), move |db| {
        services::preferences::get(db, &key)
    })
    .await
}
#[tauri::command]
pub async fn preference_set(
    app: AppHandle,
    state: State<'_, AppState>,
    key: String,
    value: String,
    binding_id: Option<String>,
    draft_project_id: Option<String>,
) -> AppResult<()> {
    use tauri::Emitter;
    let changed_key = key.clone();
    let recovery_text = value.clone();
    let expected = binding_id.clone();
    let bootstrap = state.db_path.clone();
    let result = blocking(state.db_path.clone(), move |db| {
        crate::portable::require_binding(&bootstrap, binding_id.as_deref())?;
        services::preferences::set(db, &key, &value)?;
        if key == "capture_draft" {
            if let Some(project) = draft_project_id {
                services::preferences::set(db, "capture_project", &project)?;
            }
        }
        Ok(())
    })
    .await;
    if let Err(mut error) = result {
        if changed_key == "capture_draft" && !recovery_text.trim().is_empty() {
            let _guard = IO_LOCK
                .lock()
                .map_err(|_| AppError::new("WORKER_ERROR", "恢复稿暂存失败"))?;
            match crate::portable::recovery::save(
                &state.db_path,
                expected.as_deref(),
                Some(&recovery_text),
                None,
                &error.message,
            ) {
                Ok(_) => error.message.push_str("；草稿已暂存本机，尚未写入工作目录"),
                Err(_) => error
                    .message
                    .push_str("；本机暂存也失败，请保留当前窗口内容"),
            }
        }
        return Err(error);
    }

    let _ = app.emit("preference-changed", changed_key);
    Ok(())
}
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalDataPaths {
    database: String,
    projects: Option<String>,
}
#[tauri::command]
pub async fn local_data_paths(state: State<'_, AppState>) -> AppResult<LocalDataPaths> {
    let bootstrap = state.db_path.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let info = crate::portable::info(&bootstrap)?;
        Ok(LocalDataPaths {
            database: info["database"].as_str().unwrap_or("").into(),
            projects: info["root"].as_str().map(str::to_owned),
        })
    })
    .await
    .map_err(|_| AppError::new("WORKER_ERROR", "读取位置中断"))?
}

#[tauri::command]
pub async fn projects_copy_path(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
) -> AppResult<()> {
    use tauri_plugin_clipboard_manager::ClipboardExt;
    let path = blocking_read(state.db_path.clone(), move |db| {
        services::projects::folder(db, &id)
    })
    .await?;
    app.clipboard()
        .write_text(path.to_string_lossy().into_owned())
        .map_err(|_| AppError::new("CLIPBOARD_ERROR", "无法复制项目路径，请重试"))
}

#[tauri::command]
pub fn projects_open_link(app: AppHandle, url: String) -> AppResult<()> {
    let url = services::projects::validated_url(&url)?;
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|e| AppError::new("OPEN_LINK_FAILED", &e.to_string()))
}

#[tauri::command]
pub async fn projects_extract_link_icon(
    state: State<'_, AppState>,
    url: String,
) -> AppResult<String> {
    let bootstrap = state.db_path.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = IO_LOCK
            .lock()
            .map_err(|_| AppError::new("WORKER_ERROR", "本地操作暂不可用"))?;
        if crate::portable::strict_offline(&bootstrap)? {
            return Err(AppError::new(
                "OFFLINE_MODE",
                "严格离线已开启，请使用品牌图标或名称首字",
            ));
        }
        services::link_icons::extract(&url)
    })
    .await
    .map_err(|_| AppError::new("ICON_UNAVAILABLE", "网站图标提取中断，请重试"))?
}

#[tauri::command]
pub fn app_version_info(app: AppHandle) -> serde_json::Value {
    serde_json::json!({
        "version": app.package_info().version.to_string(),
        "platform": std::env::consts::OS,
        "arch": std::env::consts::ARCH,
    })
}

#[tauri::command]
pub async fn storage_info(state: State<'_, AppState>) -> AppResult<serde_json::Value> {
    let path = state.db_path.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = IO_LOCK
            .lock()
            .map_err(|_| AppError::new("WORKER_ERROR", "本地操作暂不可用"))?;
        crate::portable::info(&path)
    })
    .await
    .map_err(|_| AppError::new("WORKER_ERROR", "读取存储状态中断"))?
}
#[tauri::command]
pub async fn storage_clear_cache(state: State<'_, AppState>) -> AppResult<()> {
    let path = state.db_path.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = IO_LOCK
            .lock()
            .map_err(|_| AppError::new("WORKER_ERROR", "本地操作暂不可用"))?;
        crate::portable::clear_cache(&path)
    })
    .await
    .map_err(|_| AppError::new("WORKER_ERROR", "重建缓存中断"))?
}

#[tauri::command]
pub async fn storage_recovery_action(
    app: AppHandle,
    state: State<'_, AppState>,
    action: String,
) -> AppResult<Option<String>> {
    use tauri::Emitter;
    let destination = if action == "export" || action == "diagnostic" {
        let chosen = app
            .dialog()
            .file()
            .set_title("选择本机恢复稿导出位置")
            .blocking_pick_folder();
        let Some(chosen) = chosen else {
            return Ok(None);
        };
        Some(
            chosen
                .into_path()
                .map_err(|_| AppError::new("INVALID_PATH", "目录无效"))?,
        )
    } else {
        None
    };
    let bootstrap = state.db_path.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        let _guard = IO_LOCK
            .lock()
            .map_err(|_| AppError::new("WORKER_ERROR", "本地操作暂不可用"))?;
        match action.as_str() {
            "diagnostic" => Ok(Some(
                crate::portable::transfer::rescue(&bootstrap, destination.as_deref().unwrap())?
                    .to_string_lossy()
                    .into_owned(),
            )),
            "export" => Ok(Some(
                crate::portable::recovery::export(&bootstrap, destination.as_deref().unwrap())?
                    .to_string_lossy()
                    .into_owned(),
            )),
            "restore" => Ok(Some(format!(
                "已恢复 {} 项待办，恢复稿原件已归档到本机",
                crate::portable::recovery::restore_as_tasks(&bootstrap)?
            ))),
            _ => Err(AppError::new("VALIDATION_ERROR", "不支持的恢复稿操作")),
        }
    })
    .await
    .map_err(|_| AppError::new("WORKER_ERROR", "恢复稿操作中断"))?;
    if result.is_ok() {
        let _ = app.emit("workspace-changed", ());
    }
    result
}

#[tauri::command]
pub fn desktop_quit(app: AppHandle) {
    use tauri::Manager;
    app.state::<crate::desktop::DesktopState>()
        .flush_exit
        .store(true, Ordering::SeqCst);
    app.exit(0);
}

#[tauri::command]
pub async fn files_changed(
    state: State<'_, AppState>,
    project_id: String,
) -> AppResult<Vec<String>> {
    blocking_read(state.db_path.clone(), move |db| {
        crate::portable::file_versions::changed(db, &project_id)
    })
    .await
}
#[tauri::command]
pub async fn files_accept_changes(
    state: State<'_, AppState>,
    project_id: String,
    ids: Vec<String>,
) -> AppResult<()> {
    blocking(state.db_path.clone(), move |db| {
        crate::portable::file_versions::accept(db, &project_id, ids)
    })
    .await
}
