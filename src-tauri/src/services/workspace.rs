use crate::{
    error::{AppError, AppResult},
    models::WorkspaceStatus,
};
use rusqlite::{Connection, OptionalExtension};
use std::path::PathBuf;

pub fn root(connection: &Connection) -> AppResult<Option<PathBuf>> {
    let value: Option<String> = connection
        .query_row(
            "SELECT value FROM settings WHERE key = 'storage_root'",
            [],
            |row| row.get(0),
        )
        .optional()?;
    Ok(value.map(PathBuf::from))
}

pub fn status(connection: &Connection) -> AppResult<WorkspaceStatus> {
    let root = root(connection)?;
    let pending_operations = connection.query_row(
        "SELECT COUNT(*) FROM filesystem_operations WHERE state != 'completed'",
        [],
        |row| row.get(0),
    )?;
    Ok(WorkspaceStatus {
        available: root.as_ref().is_some_and(|path| path.is_dir()),
        root_path: root.map(|path| path.to_string_lossy().into_owned()),
        pending_operations,
        read_only: connection
            .query_row(
                "SELECT value='1' FROM settings WHERE key='_read_only'",
                [],
                |r| r.get(0),
            )
            .unwrap_or(false),
        storage_error: connection
            .query_row(
                "SELECT value FROM settings WHERE key='_storage_error'",
                [],
                |r| r.get(0),
            )
            .optional()?,
        binding_id: connection
            .query_row(
                "SELECT value FROM settings WHERE key='_binding_id'",
                [],
                |r| r.get(0),
            )
            .optional()?,
    })
}

pub fn available_root(connection: &Connection) -> AppResult<PathBuf> {
    let stored = root(connection)?
        .ok_or_else(|| AppError::new("ROOT_NOT_CONFIGURED", "请先在设置中选择项目存储目录"))?;
    if !stored.is_dir() || stored.canonicalize()? != stored {
        return Err(AppError::new(
            "STORAGE_UNAVAILABLE",
            "项目目录不可访问或已被替换，请检查存储位置",
        ));
    }
    Ok(stored)
}
