use crate::{
    error::{AppError, AppResult},
    models::ProjectFile,
};
use rusqlite::{params, Connection, OptionalExtension};
use std::{path::Path, sync::atomic::AtomicBool};

pub fn bucket(connection: &Connection, task_id: &str) -> AppResult<String> {
    let project: Option<Option<String>> = connection
        .query_row(
            "SELECT project_id FROM tasks WHERE id=?1 AND deleted_at IS NULL",
            [task_id],
            |r| r.get(0),
        )
        .optional()?;
    Ok(project
        .ok_or_else(|| AppError::new("TASK_NOT_FOUND", "待办不存在或已删除"))?
        .unwrap_or_else(|| "__inbox__".into()))
}
pub fn list(connection: &Connection, task_id: &str) -> AppResult<Vec<ProjectFile>> {
    let project = bucket(connection, task_id)?;
    let mut statement = connection.prepare("SELECT file_id FROM task_files WHERE task_id=?1")?;
    let ids = statement
        .query_map([task_id], |r| r.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(super::files::list(connection, &project)?
        .into_iter()
        .filter(|f| ids.contains(&f.id))
        .collect())
}
pub fn link(connection: &Connection, task_id: &str, file_id: &str) -> AppResult<()> {
    let project = bucket(connection, task_id)?;
    let exists:bool=connection.query_row("SELECT EXISTS(SELECT 1 FROM project_files WHERE id=?1 AND project_id=?2 AND status='available')",params![file_id,project],|r|r.get(0))?;
    if !exists {
        return Err(AppError::new(
            "INVALID_ATTACHMENT",
            "附件必须属于待办所在项目且可访问",
        ));
    }
    connection.execute(
        "INSERT OR IGNORE INTO task_files(task_id,file_id) VALUES(?1,?2)",
        params![task_id, file_id],
    )?;
    Ok(())
}
pub fn unlink(connection: &Connection, task_id: &str, file_id: &str) -> AppResult<()> {
    bucket(connection, task_id)?;
    connection.execute(
        "DELETE FROM task_files WHERE task_id=?1 AND file_id=?2",
        params![task_id, file_id],
    )?;
    Ok(())
}
pub fn import(connection: &mut Connection, task_id: &str, source: &Path) -> AppResult<ProjectFile> {
    let project = bucket(connection, task_id)?;
    let file = super::files::import_file(
        connection,
        &project,
        source,
        &AtomicBool::new(false),
        |_, _| {},
    )?;
    link(connection, task_id, &file.id)?;
    Ok(file)
}
pub fn copy_for_move(
    connection: &mut Connection,
    task_id: &str,
    target: Option<&str>,
) -> AppResult<Vec<String>> {
    let project: Option<String> =
        connection.query_row("SELECT project_id FROM tasks WHERE id=?1", [task_id], |r| {
            r.get(0)
        })?;
    let mut statement = connection.prepare("SELECT file_id FROM task_files WHERE task_id=?1")?;
    let ids = statement
        .query_map([task_id], |r| r.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    let files = super::files::list(connection, project.as_deref().unwrap_or("__inbox__"))?
        .into_iter()
        .filter(|f| ids.contains(&f.id))
        .collect::<Vec<_>>();
    drop(statement);
    // Validate all sources before making copies; a missing attachment blocks the move.
    let sources = files
        .iter()
        .map(|f| super::files::resolve(connection, &f.project_id, &f.id))
        .collect::<AppResult<Vec<_>>>()?;
    let target = target.unwrap_or("__inbox__");
    let mut copied = Vec::new();
    for source in sources {
        copied.push(
            super::files::import_file(
                connection,
                target,
                &source,
                &AtomicBool::new(false),
                |_, _| {},
            )?
            .id,
        );
    }
    Ok(copied)
}
