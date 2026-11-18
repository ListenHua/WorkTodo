use tauri::{
    menu::{AboutMetadata, Menu, PredefinedMenuItem, Submenu, HELP_SUBMENU_ID, WINDOW_SUBMENU_ID},
    AppHandle, Runtime,
};

pub fn build<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<Menu<R>> {
    let name = app.config().product_name.as_deref().unwrap_or("WorkTodo");
    let about = AboutMetadata {
        name: Some(name.to_owned()),
        version: Some(app.package_info().version.to_string()),
        comments: Some("本地项目与待办管理".into()),
        ..Default::default()
    };

    // Keep Tauri's reserved IDs so macOS recognizes its Window and Help menus.
    let window = Submenu::with_id_and_items(
        app,
        WINDOW_SUBMENU_ID,
        "窗口",
        true,
        &[
            &PredefinedMenuItem::minimize(app, Some("最小化"))?,
            &PredefinedMenuItem::maximize(
                app,
                Some(if cfg!(target_os = "macos") {
                    "缩放"
                } else {
                    "最大化"
                }),
            )?,
            #[cfg(target_os = "macos")]
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::close_window(app, Some("关闭窗口"))?,
        ],
    )?;
    let help = Submenu::with_id_and_items(
        app,
        HELP_SUBMENU_ID,
        "帮助",
        true,
        &[
            #[cfg(not(target_os = "macos"))]
            &PredefinedMenuItem::about(app, Some("关于WorkTodo"), Some(about))?,
        ],
    )?;

    Menu::with_items(
        app,
        &[
            #[cfg(target_os = "macos")]
            &Submenu::with_items(
                app,
                name,
                true,
                &[
                    &PredefinedMenuItem::about(app, Some("关于WorkTodo"), Some(about))?,
                    &PredefinedMenuItem::separator(app)?,
                    &PredefinedMenuItem::services(app, Some("服务"))?,
                    &PredefinedMenuItem::separator(app)?,
                    &PredefinedMenuItem::hide(app, Some("隐藏WorkTodo"))?,
                    &PredefinedMenuItem::hide_others(app, Some("隐藏其他应用"))?,
                    &PredefinedMenuItem::show_all(app, Some("显示全部"))?,
                    &PredefinedMenuItem::separator(app)?,
                    &PredefinedMenuItem::quit(app, Some("退出WorkTodo"))?,
                ],
            )?,
            &Submenu::with_items(
                app,
                "文件",
                true,
                &[
                    &PredefinedMenuItem::close_window(app, Some("关闭窗口"))?,
                    #[cfg(not(target_os = "macos"))]
                    &PredefinedMenuItem::quit(app, Some("退出WorkTodo"))?,
                ],
            )?,
            &Submenu::with_items(
                app,
                "编辑",
                true,
                &[
                    // Native undo/redo menu roles are supported by Tauri on macOS only.
                    #[cfg(target_os = "macos")]
                    &PredefinedMenuItem::undo(app, Some("撤销"))?,
                    #[cfg(target_os = "macos")]
                    &PredefinedMenuItem::redo(app, Some("重做"))?,
                    #[cfg(target_os = "macos")]
                    &PredefinedMenuItem::separator(app)?,
                    &PredefinedMenuItem::cut(app, Some("剪切"))?,
                    &PredefinedMenuItem::copy(app, Some("复制"))?,
                    &PredefinedMenuItem::paste(app, Some("粘贴"))?,
                    &PredefinedMenuItem::select_all(app, Some("全选"))?,
                ],
            )?,
            #[cfg(target_os = "macos")]
            &Submenu::with_items(
                app,
                "视图",
                true,
                &[&PredefinedMenuItem::fullscreen(app, Some("切换全屏"))?],
            )?,
            &window,
            &help,
        ],
    )
}
