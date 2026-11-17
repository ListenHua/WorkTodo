use crate::{
    error::{AppError, AppResult},
    models::ProjectFile,
    services::projects,
};
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use std::{
    collections::HashSet,
    fs::{self, File, Metadata},
    io::{Read, Write},
    path::{Component, Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
};
use unicode_normalization::UnicodeNormalization;
use uuid::Uuid;

pub const IMAGE_LIMIT: u64 = 20 * 1024 * 1024;
pub const FILE_LIMIT: u64 = 200 * 1024 * 1024;

pub fn folder(connection: &Connection, project_id: &str) -> AppResult<PathBuf> {
    let base = projects::folder(connection, project_id)?;
    if crate::portable::is_portable(connection) && project_id != "__inbox__" {
        let files = base.join("Files");
        if !files.is_dir() || is_link(&fs::symlink_metadata(&files)?) {
            return Err(unsafe_path());
        }
        Ok(files.canonicalize()?)
    } else {
        Ok(base)
    }
}

fn normalized(value: &str) -> String {
    value.nfc().collect::<String>().to_lowercase()
}

pub fn is_link(metadata: &Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        // Junctions and other reparse points must not bypass the project boundary.
        metadata.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        metadata.file_type().is_symlink()
    }
}

pub fn image_mime(name: &str) -> Option<&'static str> {
    match Path::new(name)
        .extension()?
        .to_str()?
        .to_lowercase()
        .as_str()
    {
        "png" => Some("image/png"),
        "jpg" | "jpeg" => Some("image/jpeg"),
        "gif" => Some("image/gif"),
        "webp" => Some("image/webp"),
        "bmp" => Some("image/bmp"),
        _ => None,
    }
}

fn kind(name: &str) -> &'static str {
    if image_mime(name).is_some()
        || matches!(
            Path::new(name)
                .extension()
                .and_then(|s| s.to_str())
                .map(str::to_lowercase)
                .as_deref(),
            Some("heic" | "heif" | "tif" | "tiff" | "avif")
        )
    {
        "image"
    } else if matches!(
        Path::new(name)
            .extension()
            .and_then(|s| s.to_str())
            .map(str::to_lowercase)
            .as_deref(),
        Some(
            "pdf"
                | "doc"
                | "docx"
                | "xls"
                | "xlsx"
                | "ppt"
                | "pptx"
                | "txt"
                | "md"
                | "csv"
                | "rtf"
                | "odt"
                | "ods"
        )
    ) {
        "document"
    } else {
        "other"
    }
}

fn relative_path(root: &Path, path: &Path) -> AppResult<String> {
    let relative = path.strip_prefix(root).map_err(|_| unsafe_path())?;
    let parts = relative
        .components()
        .map(|part| match part {
            Component::Normal(value) => value.to_str().ok_or_else(unsafe_path),
            _ => Err(unsafe_path()),
        })
        .collect::<AppResult<Vec<_>>>()?;
    if parts.is_empty() {
        return Err(unsafe_path());
    }
    Ok(parts.join("/"))
}

fn unsafe_path() -> AppError {
    AppError::new("UNSAFE_FILE", "文件位置无效或包含链接，请检查项目目录")
}

fn checked_path(root: &Path, relative: &str) -> AppResult<PathBuf> {
    let mut path = root.to_path_buf();
    if relative.is_empty() {
        return Err(unsafe_path());
    }
    for component in Path::new(relative).components() {
        let Component::Normal(value) = component else {
            return Err(unsafe_path());
        };
        path.push(value);
        if is_link(&fs::symlink_metadata(&path)?) {
            return Err(unsafe_path());
        }
    }
    let actual = path.canonicalize()?;
    if !actual.starts_with(root) || actual == root || !actual.is_file() {
        return Err(unsafe_path());
    }
    Ok(actual)
}

fn file_record(
    project_id: &str,
    id: String,
    relative: String,
    metadata: &Metadata,
) -> AppResult<ProjectFile> {
    let modified: DateTime<Utc> = metadata.modified()?.into();
    let name = relative.rsplit('/').next().unwrap_or(&relative).to_string();
    Ok(ProjectFile {
        id,
        project_id: project_id.into(),
        kind: kind(&name).into(),
        name,
        relative_path: relative,
        size: metadata.len(),
        modified_at: modified.to_rfc3339(),
        status: "available".into(),
    })
}

fn save_record(connection: &Connection, file: &ProjectFile) -> AppResult<()> {
    connection.execute(
        "INSERT INTO project_files(id,project_id,relative_path,size,modified_at,status) VALUES(?1,?2,?3,?4,?5,'available')
         ON CONFLICT(project_id,relative_path) DO UPDATE SET size=excluded.size,modified_at=excluded.modified_at,status='available'",
        params![file.id,file.project_id,file.relative_path,i64::try_from(file.size).map_err(|_| AppError::new("FILE_TOO_LARGE", "文件大小超出可索引范围"))?,file.modified_at],
    )?;
    Ok(())
}

pub fn list(connection: &Connection, project_id: &str) -> AppResult<Vec<ProjectFile>> {
    // Cached records remain readable when the physical project directory is unavailable.
    let exists: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM projects WHERE id=?1)",
        [project_id],
        |row| row.get(0),
    )?;
    if !exists {
        return Err(AppError::new("PROJECT_NOT_FOUND", "项目不存在"));
    }
    let mut statement = connection.prepare("SELECT id,relative_path,size,modified_at,status FROM project_files WHERE project_id=?1 ORDER BY status,modified_at DESC,relative_path")?;
    let rows = statement.query_map([project_id], |row| {
        let relative: String = row.get(1)?;
        let name = relative.rsplit('/').next().unwrap_or(&relative).to_string();
        Ok(ProjectFile {
            id: row.get(0)?,
            project_id: project_id.into(),
            kind: kind(&name).into(),
            name,
            relative_path: relative,
            size: row.get::<_, i64>(2)? as u64,
            modified_at: row.get(3)?,
            status: row.get(4)?,
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

fn walk(
    root: &Path,
    directory: &Path,
    excluded: &HashSet<PathBuf>,
    records: &mut Vec<ProjectFile>,
    project_id: &str,
    visited: &mut usize,
    depth: usize,
) -> AppResult<()> {
    if depth > 64 {
        return Err(AppError::new("SCAN_LIMIT", "目录层级过深，未更新文件索引"));
    }
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        *visited += 1;
        if *visited > 50_000 {
            return Err(AppError::new(
                "SCAN_LIMIT",
                "目录包含超过 50000 个条目，未更新文件索引",
            ));
        }
        let path = entry.path();
        if entry
            .file_name()
            .to_string_lossy()
            .starts_with(".project-manage-import-")
            || excluded.contains(&path)
        {
            continue;
        }
        let metadata = fs::symlink_metadata(&path)?;
        if is_link(&metadata) {
            continue;
        }
        if metadata.is_dir() {
            walk(
                root,
                &path,
                excluded,
                records,
                project_id,
                visited,
                depth + 1,
            )?;
        } else if metadata.is_file()
            && !matches!(
                entry.file_name().to_str(),
                Some(".DS_Store" | "Thumbs.db" | "desktop.ini")
            )
        {
            let relative = relative_path(root, &path)?;
            checked_path(root, &relative)?;
            records.push(file_record(
                project_id,
                Uuid::new_v4().to_string(),
                relative,
                &metadata,
            )?);
        }
    }
    Ok(())
}

pub fn scan(connection: &mut Connection, project_id: &str) -> AppResult<Vec<ProjectFile>> {
    let root = folder(connection, project_id)?;
    // Serialize the index snapshot with import reservation, including other app processes.
    let transaction =
        connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    let excluded: HashSet<PathBuf> = {
        let mut statement = transaction.prepare("SELECT target FROM filesystem_operations WHERE kind='import_file' AND state!='completed'")?;
        let rows = statement.query_map([], |row| Ok(PathBuf::from(row.get::<_, String>(0)?)))?;
        rows.collect::<Result<_, _>>()?
    };
    let mut records = Vec::new();
    walk(&root, &root, &excluded, &mut records, project_id, &mut 0, 0)?;
    if folder(&transaction, project_id)? != root {
        return Err(unsafe_path());
    }
    // No rows change until the entire directory has been read successfully.
    transaction.execute(
        "UPDATE project_files SET status='missing' WHERE project_id=?1",
        [project_id],
    )?;
    for file in records {
        save_record(&transaction, &file)?;
    }
    transaction.commit()?;
    list(connection, project_id)
}

fn validate_name(name: &str) -> AppResult<()> {
    let stem = normalized(name).split('.').next().unwrap_or("").to_string();
    if name.is_empty()
        || name.starts_with(".project-manage-import-")
        || name.chars().count() > 255
        || name.ends_with([' ', '.'])
        || name
            .chars()
            .any(|c| c.is_control() || "<>:\"/\\|?*".contains(c))
        || [".", ".."].contains(&name)
        || ["con", "prn", "aux", "nul"].contains(&stem.as_str())
        || (1..=9).any(|i| stem == format!("com{i}") || stem == format!("lpt{i}"))
    {
        return Err(AppError::new(
            "INVALID_FILE_NAME",
            "文件名含有不可用字符或保留名称，请重命名后重试",
        ));
    }
    Ok(())
}

fn same_file(left: &Metadata, right: &Metadata) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        left.dev() == right.dev() && left.ino() == right.ino()
    }
    #[cfg(not(unix))]
    {
        left.len() == right.len()
            && left.created().ok() == right.created().ok()
            && left.modified().ok() == right.modified().ok()
    }
}

fn remove_owned_copy(
    connection: &Connection,
    project_id: &str,
    root: &Path,
    target: &Path,
    owned: &Metadata,
) -> bool {
    if folder(connection, project_id).is_ok_and(|path| path == root)
        && fs::symlink_metadata(target)
            .is_ok_and(|current| !is_link(&current) && same_file(&current, owned))
    {
        return fs::remove_file(target).is_ok();
    }
    false
}

fn publish(
    staging: &mut Option<tempfile::TempPath>,
    root: &Path,
    name: &str,
) -> AppResult<PathBuf> {
    let existing: HashSet<String> = fs::read_dir(root)?
        .map(|entry| entry.map(|e| normalized(&e.file_name().to_string_lossy())))
        .collect::<Result<_, _>>()?;
    let original = Path::new(name);
    let stem = original
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(name);
    let extension = original.extension().and_then(|s| s.to_str());
    for index in 0..10_000 {
        let candidate = if index == 0 {
            name.to_string()
        } else {
            match extension {
                Some(ext) => format!("{stem} ({index}).{ext}"),
                None => format!("{stem} ({index})"),
            }
        };
        if existing.contains(&normalized(&candidate)) {
            continue;
        }
        let target = root.join(candidate);
        let staged = staging
            .take()
            .ok_or_else(|| AppError::new("IMPORT_ERROR", "临时文件不可用"))?;
        crate::portable::journal::create(&staged, &target)?;
        match staged.persist_noclobber(&target) {
            Ok(()) => return Ok(target),
            Err(error) => {
                *staging = Some(error.path);
                if error.error.kind() != std::io::ErrorKind::AlreadyExists {
                    return Err(error.error.into());
                }
            }
        }
    }
    Err(AppError::new(
        "NAME_CONFLICT",
        "同名文件过多，请重命名后重试",
    ))
}

pub fn import_file(
    connection: &mut Connection,
    project_id: &str,
    source: &Path,
    canceled: &AtomicBool,
    mut progress: impl FnMut(u64, u64),
) -> AppResult<ProjectFile> {
    let root = folder(connection, project_id)?;
    let active: bool = connection.query_row(
        "SELECT status='active' FROM projects WHERE id=?1",
        [project_id],
        |row| row.get(0),
    )?;
    if !active {
        return Err(AppError::new("PROJECT_INACTIVE", "请先将项目恢复为进行中"));
    }
    if !source.is_absolute() || is_link(&fs::symlink_metadata(source)?) {
        return Err(unsafe_path());
    }
    let source = if source.starts_with(&root) {
        checked_path(&root, &relative_path(&root, source)?)?
    } else {
        source.canonicalize()?
    };
    let mut input = File::open(&source)?;
    let metadata = input.metadata()?;
    if !metadata.is_file() {
        return Err(AppError::new("INVALID_FILE", "只能添加普通文件"));
    }
    let name = source
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or_else(unsafe_path)?;
    let limit = if kind(name) == "image" {
        IMAGE_LIMIT
    } else {
        FILE_LIMIT
    };
    if metadata.len() > limit {
        return Err(AppError::new(
            "FILE_TOO_LARGE",
            if limit == IMAGE_LIMIT {
                "图片不能超过 20 MB"
            } else {
                "文件不能超过 200 MB"
            },
        ));
    }
    if canceled.load(Ordering::Relaxed) {
        return Err(AppError::new("IMPORT_CANCELED", "已取消添加"));
    }
    if source.starts_with(&root) {
        if name.starts_with(".project-manage-import-") {
            return Err(AppError::new("FILE_BUSY", "临时副本尚未完成添加"));
        }
        let copying: bool = connection.query_row("SELECT EXISTS(SELECT 1 FROM filesystem_operations WHERE kind='import_file' AND state!='completed' AND target=?1)", [source.to_string_lossy()], |row| row.get(0))?;
        if copying {
            return Err(AppError::new("FILE_BUSY", "该文件尚未完成添加，请稍后重试"));
        }
        // An existing project file is indexed in place, including files in ordinary subdirectories.
        let relative = relative_path(&root, &source)?;
        let existing: Option<String> = connection
            .query_row(
                "SELECT id FROM project_files WHERE project_id=?1 AND relative_path=?2",
                params![project_id, relative],
                |row| row.get(0),
            )
            .optional()?;
        let record = file_record(
            project_id,
            existing.unwrap_or_else(|| Uuid::new_v4().to_string()),
            relative,
            &metadata,
        )?;
        save_record(connection, &record)?;
        progress(metadata.len(), metadata.len());
        return Ok(record);
    }
    validate_name(name)?;
    let operation_id = Uuid::new_v4().to_string();
    let staged = tempfile::Builder::new()
        .prefix(".project-manage-import-")
        .tempfile_in(&root)?;
    connection.execute("INSERT INTO filesystem_operations(id,kind,target,created_at) VALUES(?1,'import_file',?2,?3)", params![operation_id,staged.path().to_string_lossy(),Utc::now().to_rfc3339()])?;
    let (mut output, temporary_path) = staged.into_parts();
    let mut staging = Some(temporary_path);
    let mut published: Option<PathBuf> = None;
    let result = (|| -> AppResult<ProjectFile> {
        let mut buffer = [0_u8; 64 * 1024];
        let mut copied = 0;
        progress(0, metadata.len());
        let mut last_progress = std::time::Instant::now();
        loop {
            if canceled.load(Ordering::Relaxed) {
                return Err(AppError::new("IMPORT_CANCELED", "已取消添加"));
            }
            let count = input.read(&mut buffer)?;
            if count == 0 {
                break;
            }
            copied += count as u64;
            if copied > limit {
                return Err(AppError::new(
                    "FILE_TOO_LARGE",
                    "文件在复制时变大并超过大小限制",
                ));
            }
            output.write_all(&buffer[..count])?;
            if copied >= metadata.len() || last_progress.elapsed().as_millis() >= 40 {
                progress(copied, metadata.len().max(copied));
                last_progress = std::time::Instant::now();
            }
        }
        output.sync_all()?;
        if canceled.load(Ordering::Relaxed) {
            return Err(AppError::new("IMPORT_CANCELED", "已取消添加"));
        }
        if input.metadata()?.len() != metadata.len()
            || input.metadata()?.modified()? != metadata.modified()?
            || copied != metadata.len()
        {
            return Err(AppError::new(
                "FILE_CHANGED",
                "源文件在复制过程中发生变化，请重试",
            ));
        }
        if folder(connection, project_id)? != root {
            return Err(unsafe_path());
        }
        let transaction =
            connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let target = publish(&mut staging, &root, name)?;
        published = Some(target.clone());
        let record = file_record(
            project_id,
            Uuid::new_v4().to_string(),
            relative_path(&root, &target)?,
            &output.metadata()?,
        )?;
        save_record(&transaction, &record)?;
        transaction.execute(
            "UPDATE filesystem_operations SET state='completed' WHERE id=?1",
            [&operation_id],
        )?;
        transaction.commit()?;
        Ok(record)
    })();
    let owned = output.metadata()?;
    drop(output);
    if result.is_err() {
        let mut cleaned = true;
        if let Some(target) = published {
            cleaned = remove_owned_copy(connection, project_id, &root, &target, &owned);
        }
        if let Some(temporary) = staging.take() {
            // Disarm automatic path deletion and verify that the directory and file
            // still belong to this import before cleaning up a failed copy.
            cleaned &= temporary
                .keep()
                .is_ok_and(|path| remove_owned_copy(connection, project_id, &root, &path, &owned));
        }
        let state = if cleaned {
            "completed"
        } else {
            "needs_attention"
        };
        let _ = connection.execute(
            "UPDATE filesystem_operations SET state=?2 WHERE id=?1",
            params![operation_id, state],
        );
    }
    result
}

pub fn resolve(connection: &Connection, project_id: &str, id: &str) -> AppResult<PathBuf> {
    let relative: Option<String> = connection
        .query_row(
            "SELECT relative_path FROM project_files WHERE id=?1 AND project_id=?2",
            params![id, project_id],
            |row| row.get(0),
        )
        .optional()?;
    let relative = relative.ok_or_else(|| AppError::new("FILE_NOT_FOUND", "文件记录不存在"))?;
    checked_path(&folder(connection, project_id)?, &relative)
}

// A missing file can still have symlinked ancestors. Validate the complete stored
// path first, and only classify NotFound as missing (never permission failures).
fn delete_source(root: &Path, relative: &str) -> AppResult<Option<PathBuf>> {
    let parts = Path::new(relative).components().collect::<Vec<_>>();
    if parts.is_empty() || parts.iter().any(|c| !matches!(c, Component::Normal(_))) {
        return Err(unsafe_path());
    }
    let mut path = root.to_path_buf();
    for part in parts {
        path.push(part.as_os_str());
        match fs::symlink_metadata(&path) {
            Ok(metadata) if is_link(&metadata) => return Err(unsafe_path()),
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error.into()),
        }
    }
    checked_path(root, relative).map(Some)
}

fn delete_index(connection: &Connection, project_id: &str, id: &str) -> AppResult<()> {
    connection.execute("DELETE FROM task_files WHERE file_id=?1", [id])?;
    let deleted = connection.execute(
        "DELETE FROM project_files WHERE id=?1 AND project_id=?2",
        params![id, project_id],
    )?;
    if deleted != 1 {
        return Err(AppError::new("FILE_NOT_FOUND", "文件记录不存在"));
    }
    Ok(())
}

// Keep deleted bytes in a unique folder outside every project scan. The intent
// survives a process exit between the filesystem move and the SQLite commit.
pub fn delete(connection: &mut Connection, project_id: &str, id: &str) -> AppResult<&'static str> {
    let (relative, directory): (String, String) = connection.query_row(
        "SELECT f.relative_path,p.directory_relpath FROM project_files f JOIN projects p ON p.id=f.project_id WHERE f.id=?1 AND f.project_id=?2",
        params![id, project_id], |row| Ok((row.get(0)?, row.get(1)?)),
    ).optional()?.ok_or_else(|| AppError::new("FILE_NOT_FOUND", "文件记录不存在"))?;
    let root = super::workspace::available_root(connection)?;
    let directory = if crate::portable::is_portable(connection) && project_id != "__inbox__" {
        format!("{directory}/Files")
    } else {
        directory
    };
    let source = delete_source(&root, &format!("{directory}/{relative}"))?;
    let Some(source) = source else {
        let tx = connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        delete_index(&tx, project_id, id)?;
        tx.commit()?;
        return Ok("removed");
    };
    let trash = root.join("_回收站");
    let metadata = fs::symlink_metadata(&trash)?;
    if is_link(&metadata) || !metadata.is_dir() || trash.canonicalize()? != trash {
        return Err(unsafe_path());
    }
    let operation = Uuid::new_v4().to_string();
    let directory = trash.join(&operation);
    crate::portable::journal::directory(&directory)?;
    let target = directory.join(source.file_name().ok_or_else(unsafe_path)?);
    let detail =
        serde_json::json!({"project": project_id, "file": id, "relative": relative}).to_string();
    if let Err(error) = connection.execute(
        "INSERT INTO filesystem_operations(id,kind,target,detail,created_at) VALUES(?1,'delete_file',?2,?3,?4)",
        params![operation, target.to_string_lossy(), detail, Utc::now().to_rfc3339()],
    ) {
        let _ = fs::remove_dir(&directory);
        return Err(error.into());
    }
    let mut moved = false;
    let result = (|| -> AppResult<()> {
        let tx = connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        delete_index(&tx, project_id, id)?;
        super::safe_fs::rename_exclusive(&source, &target)?;
        moved = true;
        tx.execute(
            "UPDATE filesystem_operations SET state='completed' WHERE id=?1",
            [&operation],
        )?;
        tx.commit()?;
        Ok(())
    })();
    if let Err(error) = result {
        let restored = !moved || super::safe_fs::rename_exclusive(&target, &source).is_ok();
        let _ = connection.execute(
            "UPDATE filesystem_operations SET state=?2 WHERE id=?1",
            params![
                operation,
                if restored {
                    "completed"
                } else {
                    "needs_attention"
                }
            ],
        );
        let _ = fs::remove_dir(&directory);
        if !restored {
            return Err(AppError::new(
                "DELETE_NEEDS_ATTENTION",
                "文件已保留在 _回收站，索引未更新，请在设置中检查并恢复文件操作",
            ));
        }
        return Err(error);
    }
    Ok("recycled")
}

pub fn recover_delete(
    connection: &mut Connection,
    operation: &str,
    detail: &serde_json::Value,
) -> AppResult<()> {
    Uuid::parse_str(operation).map_err(|_| unsafe_path())?;
    let project = detail["project"].as_str().ok_or_else(unsafe_path)?;
    let id = detail["file"].as_str().ok_or_else(unsafe_path)?;
    let relative = detail["relative"].as_str().ok_or_else(unsafe_path)?;
    let directory: String = connection.query_row(
        "SELECT directory_relpath FROM projects WHERE id=?1",
        [project],
        |row| row.get(0),
    )?;
    // Reject traversal even if the original no longer exists.
    if relative.is_empty()
        || Path::new(relative)
            .components()
            .any(|c| !matches!(c, Component::Normal(_)))
    {
        return Err(unsafe_path());
    }

    let root = super::workspace::available_root(connection)?;
    let target_relative = format!(
        "_回收站/{operation}/{}",
        Path::new(relative)
            .file_name()
            .and_then(|s| s.to_str())
            .ok_or_else(unsafe_path)?
    );
    let directory = if crate::portable::is_portable(connection) && project != "__inbox__" {
        format!("{directory}/Files")
    } else {
        directory
    };
    let has_source = delete_source(&root, &format!("{directory}/{relative}"))?.is_some();
    let has_target = delete_source(&root, &target_relative)?.is_some();
    if has_source == has_target {
        return Err(AppError::new(
            "RECOVERY_ERROR",
            "删除操作的文件位置存在歧义，请先核对项目目录与 _回收站",
        ));
    }
    let tx = connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    if has_target {
        let stored: Option<(String, String)> = tx
            .query_row(
                "SELECT project_id,relative_path FROM project_files WHERE id=?1",
                [id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        if stored
            .as_ref()
            .is_some_and(|(bucket, path)| bucket != project || path != relative)
        {
            return Err(unsafe_path());
        }
        tx.execute("DELETE FROM task_files WHERE file_id=?1", [id])?;
        tx.execute(
            "DELETE FROM project_files WHERE id=?1 AND project_id=?2",
            params![id, project],
        )?;
    }
    tx.execute(
        "UPDATE filesystem_operations SET state='completed' WHERE id=?1",
        [operation],
    )?;
    tx.commit()?;
    Ok(())
}

pub fn preview(connection: &Connection, project_id: &str, id: &str) -> AppResult<Vec<u8>> {
    let path = resolve(connection, project_id, id)?;
    let mime = path
        .file_name()
        .and_then(|s| s.to_str())
        .and_then(image_mime)
        .ok_or_else(|| AppError::new("PREVIEW_UNSUPPORTED", "此格式请使用系统程序打开"))?;
    let mut bytes = Vec::new();
    File::open(path)?
        .take(IMAGE_LIMIT + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > IMAGE_LIMIT {
        return Err(AppError::new(
            "FILE_TOO_LARGE",
            "超过 20 MB 的图片请使用系统程序打开",
        ));
    }
    let valid = match mime {
        "image/png" => bytes.starts_with(b"\x89PNG\r\n\x1a\n"),
        "image/jpeg" => bytes.starts_with(b"\xff\xd8\xff"),
        "image/gif" => bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a"),
        "image/webp" => bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP"),
        "image/bmp" => bytes.starts_with(b"BM"),
        _ => false,
    };
    if !valid {
        return Err(AppError::new(
            "INVALID_IMAGE",
            "文件内容与图片格式不匹配，无法预览",
        ));
    }
    Ok(bytes)
}

pub fn may_open(path: &Path) -> bool {
    let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
    kind(name) != "other"
}
