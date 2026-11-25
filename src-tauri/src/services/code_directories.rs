use crate::{
    error::{AppError, AppResult},
    models::CodeDirectory,
};
use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use std::{
    collections::HashSet,
    fs,
    path::{Component, Path, PathBuf},
};
use uuid::Uuid;

fn invalid(message: &str) -> AppError {
    AppError::new("INVALID_CODE_DIRECTORY", message)
}
// Recognize stored paths on either platform so workspace metadata can still be
// loaded when a linked directory belongs to another computer.
fn absolute(path: &str) -> bool {
    path.starts_with('/')
        || (path.len() >= 3
            && path.as_bytes()[0].is_ascii_alphabetic()
            && &path.as_bytes()[1..3] == b":/")
}
fn validate_path(path: &str) -> AppResult<()> {
    let tail = if let Some(unc) = path.strip_prefix("//") {
        if unc.split('/').count() < 2 {
            return Err(invalid("代码目录路径无效"));
        }
        unc
    } else if let Some(unix) = path.strip_prefix('/') {
        unix
    } else if absolute(path) {
        &path[3..]
    } else {
        path
    };
    if path.is_empty()
        || path.len() > 2048
        || tail.contains(['\\', ':'])
        || path.chars().any(char::is_control)
        || (!tail.is_empty()
            && tail.split('/').any(|part| {
                part.is_empty()
                    || part == "."
                    || part == ".."
                    || part.eq_ignore_ascii_case(".git")
                    || part.eq_ignore_ascii_case(".workspace")
            }))
    {
        return Err(invalid("请选择有效的本地代码目录"));
    }
    Ok(())
}
fn stored_path(path: &Path) -> String {
    let path = path.to_string_lossy();
    let path = if cfg!(windows) {
        path.replace('\\', "/")
    } else {
        path.into_owned()
    };
    // Windows canonical paths use a verbatim prefix; keep the stored/displayed
    // form readable and usable on subsequent Windows launches.
    if let Some(unc) = path.strip_prefix("//?/UNC/") {
        format!("//{unc}")
    } else {
        path.strip_prefix("//?/").unwrap_or(&path).to_owned()
    }
}
fn overlaps(a: &str, b: &str) -> bool {
    let a = a.trim_end_matches('/').to_lowercase();
    let b = b.trim_end_matches('/').to_lowercase();
    a == b || a.starts_with(&format!("{b}/")) || b.starts_with(&format!("{a}/"))
}
pub fn validate_records(records: &[CodeDirectory]) -> AppResult<()> {
    if records.len() > 50 {
        return Err(invalid("每个项目最多关联 50 个代码目录"));
    }
    let mut ids = HashSet::new();
    for (index, record) in records.iter().enumerate() {
        if Uuid::parse_str(&record.id).is_err()
            || !ids.insert(&record.id)
            || record.name.trim().is_empty()
            || record.name.chars().count() > 80
            || record.name.chars().any(char::is_control)
            || chrono::DateTime::parse_from_rfc3339(&record.created_at).is_err()
        {
            return Err(invalid("代码目录名称或关联信息无效"));
        }
        validate_path(&record.path)?;
        if records[..index]
            .iter()
            .any(|old| overlaps(&old.path, &record.path))
        {
            return Err(invalid("代码目录重复或范围重叠，请分别选择互不包含的目录"));
        }
    }
    Ok(())
}
pub fn records(db: &Connection, project_id: &str) -> AppResult<Vec<CodeDirectory>> {
    let json: Option<String> = db
        .query_row(
            "SELECT code_directories_json FROM projects WHERE id=?1 AND id!='__inbox__'",
            [project_id],
            |row| row.get(0),
        )
        .optional()?;
    let json = json.ok_or_else(|| AppError::new("PROJECT_NOT_FOUND", "项目不存在"))?;
    serde_json::from_str(&json).map_err(|_| invalid("代码目录关联数据损坏"))
}
pub fn resolve(root: Option<&Path>, path: &str) -> AppResult<PathBuf> {
    validate_path(path)?;
    let unavailable = || {
        AppError::new(
            "CODE_DIRECTORY_UNAVAILABLE",
            "代码目录不可用，请重新选择或取消关联",
        )
    };
    let target = if absolute(path) {
        let target = PathBuf::from(path);
        // An absolute Windows path is unavailable on macOS (and conversely),
        // rather than being interpreted relative to the process directory.
        if !target.is_absolute() || (!cfg!(windows) && path.starts_with("//")) {
            return Err(unavailable());
        }
        target
    } else {
        root.ok_or_else(unavailable)?.join(path)
    };
    let mut candidate = PathBuf::new();
    for part in target.components() {
        candidate.push(part);
        if !matches!(part, Component::Normal(_)) {
            continue;
        }
        let meta = fs::symlink_metadata(&candidate).map_err(|_| unavailable())?;
        if !meta.is_dir() || super::files::is_link(&meta) {
            return Err(AppError::new(
                "CODE_DIRECTORY_UNAVAILABLE",
                "代码目录已变化或包含链接，请重新选择",
            ));
        }
    }
    target.canonicalize().map_err(|_| unavailable())
}
pub fn selected_path(root: Option<&Path>, selected: &Path) -> AppResult<String> {
    let selected = selected.canonicalize()?;
    let path = root
        .and_then(|root| selected.strip_prefix(root).ok())
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or(&selected);
    let path = stored_path(path);
    resolve(root, &path)?;
    Ok(path)
}
pub fn folder(db: &Connection, project_id: &str, code_id: &str) -> AppResult<PathBuf> {
    let record = records(db, project_id)?
        .into_iter()
        .find(|record| record.id == code_id)
        .ok_or_else(|| AppError::new("CODE_NOT_FOUND", "代码目录关联已移除，请重新选择"))?;
    let root = if absolute(&record.path) {
        None
    } else {
        Some(super::projects::folder(db, project_id)?)
    };
    resolve(root.as_deref(), &record.path)
}
pub fn save(
    db: &Connection,
    project_id: &str,
    code_id: Option<&str>,
    name: &str,
    path: &str,
) -> AppResult<Vec<CodeDirectory>> {
    let mut records = records(db, project_id)?;
    let existing = code_id
        .map(|id| {
            records
                .iter()
                .position(|record| record.id == id)
                .ok_or_else(|| AppError::new("CODE_NOT_FOUND", "代码目录关联已移除"))
        })
        .transpose()?;
    let root = super::projects::folder(db, project_id).ok();
    let path = if existing.is_none_or(|index| records[index].path != path) {
        let selected = resolve(root.as_deref(), path)?;
        selected_path(root.as_deref(), &selected)?
    } else {
        path.to_owned()
    };
    if let Some(index) = existing {
        records[index].name = name.trim().into();
        records[index].path = path;
    } else {
        records.push(CodeDirectory {
            id: Uuid::new_v4().to_string(),
            name: name.trim().into(),
            path,
            created_at: Utc::now().to_rfc3339(),
        });
    }
    validate_records(&records)?;
    // Compare the effective paths as well: a project root can now be linked,
    // and older associations may still be stored relative to that root.
    if let Some(root) = root {
        let paths: Vec<String> = records
            .iter()
            .map(|record| {
                if absolute(&record.path) {
                    record.path.clone()
                } else {
                    stored_path(&root.join(&record.path))
                }
            })
            .collect();
        for (index, path) in paths.iter().enumerate() {
            if paths[..index].iter().any(|old| overlaps(old, path)) {
                return Err(invalid("代码目录重复或范围重叠，请分别选择互不包含的目录"));
            }
        }
    }
    write(db, project_id, &records)?;
    Ok(records)
}
pub fn remove(db: &Connection, project_id: &str, code_id: &str) -> AppResult<Vec<CodeDirectory>> {
    let mut records = records(db, project_id)?;
    let before = records.len();
    records.retain(|record| record.id != code_id);
    if before == records.len() {
        return Err(AppError::new("CODE_NOT_FOUND", "代码目录关联已移除"));
    }
    write(db, project_id, &records)?;
    Ok(records)
}
fn write(db: &Connection, project_id: &str, records: &[CodeDirectory]) -> AppResult<()> {
    let json = serde_json::to_string(records).map_err(|_| invalid("代码目录关联信息无法保存"))?;
    db.execute(
        "UPDATE projects SET code_directories_json=?2 WHERE id=?1",
        params![project_id, json],
    )?;
    Ok(())
}
