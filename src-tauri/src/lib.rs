mod commands;
mod db;
mod desktop;
mod error;
mod menu;
mod middle_capture;
mod models;
mod native_locale;
mod portable;
mod services;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    native_locale::initialize();
    let context = tauri::generate_context!();
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            desktop::show_main(app)
        }))
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_autostart::Builder::new().build())
        .manage(desktop::DesktopState::default())
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "quick" {
                    api.prevent_close();
                    let _ = window.hide();
                } else if let Some(state) = window.try_state::<commands::AppState>() {
                    if let Ok(settings) = portable::execute(&state.db_path, false, |db| {
                        services::reminders::settings(db)
                    }) {
                        if settings.close_to_tray {
                            api.prevent_close();
                            let _ = window.hide();
                        } else {
                            window.app_handle().exit(0);
                        }
                    }
                }
            }
        })
        .menu(menu::build)
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(move |app| {
            let data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&data_dir)?;
            let db_path = data_dir.join("app.db");
            // Portable startup does not depend on the retained legacy database.
            if !data_dir.join("device.json").exists() {
                db::initialize(&db_path).map_err(|error| std::io::Error::other(error.message))?;
            }
            app.manage(commands::AppState {
                db_path,
                imports: Default::default(),
            });
            desktop::initialize(app.handle()).map_err(|e| std::io::Error::other(e.message))?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::app_version_info,
            commands::preference_get,
            commands::preference_set,
            commands::local_data_paths,
            commands::desktop_status,
            commands::desktop_save,
            commands::middle_permission,
            commands::middle_retry,
            commands::desktop_autostart,
            commands::notification_test,
            commands::reminders_list,
            commands::reminders_schedule,
            commands::reminders_action,
            commands::capture_clipboard,
            commands::capture_selection,
            commands::desktop_quit,
            commands::capture_image,
            commands::capture_save,
            commands::capture_discard,
            commands::tasks_create_full,
            commands::tasks_update,
            commands::tasks_trash,
            commands::projects_update,
            commands::projects_open_link,
            commands::projects_extract_link_icon,
            commands::projects_set_status,
            commands::attachments_list,
            commands::attachments_link,
            commands::attachments_import,
            commands::data_pending,
            commands::data_recover,
            commands::data_action,
            commands::storage_info,
            commands::storage_clear_cache,
            commands::storage_recovery_action,
            commands::workspace_status,
            commands::workspace_select_root,
            commands::projects_list,
            commands::projects_set_pinned,
            commands::projects_create,
            commands::projects_open_folder,
            commands::projects_copy_path,
            commands::tasks_list,
            commands::tasks_create,
            commands::tasks_set_completed,
            commands::files_list,
            commands::files_scan,
            commands::files_pick,
            commands::files_import,
            commands::files_cancel_import,
            commands::files_preview,
            commands::files_open,
            commands::files_delete,
            commands::files_changed,
            commands::files_accept_changes,
        ])
        .build(context)
        .expect("无法启动项目工作台")
        .run(move |app, event| {
            if let tauri::RunEvent::ExitRequested { api, .. } = &event {
                use std::sync::atomic::Ordering;
                use tauri::Emitter;
                let state = app.state::<desktop::DesktopState>();
                if state.capture_ready.load(Ordering::SeqCst)
                    && !state.flush_exit.load(Ordering::SeqCst)
                    && app.get_webview_window("capture").is_some()
                {
                    api.prevent_exit();
                    let _ = app.emit_to("capture", "capture-before-exit", ());
                }
            }
            #[cfg(target_os = "macos")]
            if let tauri::RunEvent::Reopen { .. } = event {
                desktop::show_main(app);
            }
            #[cfg(not(target_os = "macos"))]
            let _ = (app, event);
        });
}
