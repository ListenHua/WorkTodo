//! Selecting a folder opens its own data; it never copies the previous workspace.
use super::io;
use crate::{
    db,
    error::{AppError, AppResult},
    models::WorkspaceStatus,
};
use std::{fs, path::Path};

fn conflict(path: &Path) -> AppError {
    AppError::new(
        "STORAGE_FILE_CONFLICT",
        &format!(
            "{} 已存在且不是可用的 WorkTodo 数据，请更换目录或处理该文件；原文件未修改",
            path.display()
        ),
    )
}

pub fn select(bootstrap: &Path, target: &Path) -> AppResult<WorkspaceStatus> {
    let target = target.canonicalize()?;
    if !target.is_dir() {
        return Err(io::invalid("请选择文件夹作为存储目录"));
    }
    fs::read_dir(&target)?;
    let config = io::path(&target, "config.json")?;
    if config.try_exists()? {
        let value = io::read(&config)
            .map_err(|error| error.with_details(serde_json::json!({"path": config})))?;
        if value["kind"] != "workspace" {
            return Err(conflict(&config));
        }
        return super::open(bootstrap, &target);
    }
    // Only initialize absent metadata. Ordinary files and folders stay in place.
    for name in ["todo.json", ".workspace/transactions/current.json"] {
        let path = io::path(&target, name)?;
        if path.try_exists()? {
            return Err(conflict(&path));
        }
    }
    for name in ["Files", "_回收站", ".workspace"] {
        let path = io::path(&target, name)?;
        if path.try_exists()? && !path.is_dir() {
            return Err(conflict(&path));
        }
    }
    let _lock = io::lock(&target)?;
    // Another app may have initialized the folder while this process waited for the lock.
    if config.try_exists()? {
        return super::open_locked(bootstrap, &target);
    }
    for name in ["todo.json", ".workspace/transactions/current.json"] {
        let path = io::path(&target, name)?;
        if path.try_exists()? {
            return Err(conflict(&path));
        }
    }
    // A new folder starts empty, even if the previous folder or legacy database has data.
    let temporary = tempfile::tempdir()?;
    let database = temporary.path().join("empty.db");
    db::initialize(&database)?;
    super::initialize_locked(&target, &db::connect(&database)?)?;
    super::open_locked(bootstrap, &target)
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub name: String,
    pub directory: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub directory_merge: Option<crate::models::ProjectDirectoryMerge>,
}

/// Show ordinary root entries in place; metadata and registered projects have their own views.
pub fn entries(db: &rusqlite::Connection) -> AppResult<Vec<Entry>> {
    let Some(root) = crate::services::workspace::root(db)? else {
        return Ok(vec![]);
    };
    let mut statement =
        db.prepare("SELECT directory_relpath FROM projects WHERE id!='__inbox__'")?;
    let projects = statement
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<Result<std::collections::HashSet<_>, _>>()?;
    let mut out = Vec::new();
    for entry in fs::read_dir(&root)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().to_string();
        if matches!(
            name.as_str(),
            "config.json"
                | "todo.json"
                | ".workspace"
                | "_回收站"
                | ".DS_Store"
                | "Thumbs.db"
                | "desktop.ini"
        ) || projects.contains(&name)
        {
            continue;
        }
        let meta = fs::symlink_metadata(entry.path())?;
        if crate::services::files::is_link(&meta) || (!meta.is_file() && !meta.is_dir()) {
            continue;
        }
        if name == "Files" && meta.is_dir() && fs::read_dir(entry.path())?.next().is_none() {
            continue;
        }
        let directory_merge = if meta.is_dir()
            && name.to_lowercase() != "files"
            && crate::services::projects::normalized_name(&name).is_ok()
        {
            Some(crate::models::ProjectDirectoryMerge {
                path: entry.path().to_string_lossy().into_owned(),
                fingerprint: crate::services::projects::directory_fingerprint(&entry.path())?,
            })
        } else {
            None
        };
        out.push(Entry {
            name,
            directory: meta.is_dir(),
            directory_merge,
        });
    }
    out.sort_by(|a, b| {
        b.directory
            .cmp(&a.directory)
            .then_with(|| a.name.cmp(&b.name))
    });
    Ok(out)
}

pub fn entry_path(db: &rusqlite::Connection, name: &str) -> AppResult<std::path::PathBuf> {
    if !entries(db)?.iter().any(|entry| entry.name == name) {
        return Err(io::invalid("文件已不存在，请刷新目录"));
    }
    io::path(&crate::services::workspace::available_root(db)?, name)
}
