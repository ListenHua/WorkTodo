use crate::{
    db,
    error::{AppError, AppResult},
};
use chrono::Utc;
use rusqlite::{params, Connection};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, io::Read};
use std::{
    fs,
    path::{Path, PathBuf},
    time::Duration,
};
use uuid::Uuid;

fn empty_destination(path: &Path, source: &Path) -> AppResult<PathBuf> {
    let path = path.canonicalize()?;
    if !path.is_dir()
        || path.starts_with(source)
        || source.starts_with(&path)
        || fs::read_dir(&path)?.next().is_some()
    {
        return Err(AppError::new(
            "INVALID_DESTINATION",
            "请选择与当前存储目录互不包含的空文件夹",
        ));
    }
    Ok(path)
}
fn require_settled(db: &Connection) -> AppResult<()> {
    let pending: i64 = db.query_row(
        "SELECT COUNT(*) FROM filesystem_operations WHERE state!='completed'",
        [],
        |r| r.get(0),
    )?;
    if pending != 0 {
        return Err(AppError::new(
            "PENDING_OPERATIONS",
            "请先检查并处理未完成的文件操作",
        ));
    }
    Ok(())
}
// A backup is a portable folder containing a consistent SQLite snapshot and all local files.
// Publication is exclusive; failure leaves the current database and source tree intact.
pub fn backup(db: &Connection, destination: &Path) -> AppResult<PathBuf> {
    require_settled(db)?;
    let root = super::workspace::root(db)?;
    if root.is_none()
        && db.query_row("SELECT COUNT(*) FROM project_files", [], |r| {
            r.get::<_, i64>(0)
        })? > 0
    {
        return Err(AppError::new("STORAGE_UNAVAILABLE", "请先恢复项目存储位置"));
    }
    let destination = destination.canonicalize()?;
    if root
        .as_ref()
        .is_some_and(|root| destination.starts_with(root) || root.starts_with(&destination))
    {
        return Err(AppError::new(
            "INVALID_DESTINATION",
            "备份目录不能与项目存储目录互相包含",
        ));
    }
    let name = format!(
        "WorkTodo备份-{}-{}",
        Utc::now().format("%Y%m%d-%H%M%S"),
        &Uuid::new_v4().to_string()[..8]
    );
    let stage = tempfile::Builder::new()
        .prefix(".project-manage-backup-")
        .tempdir_in(&destination)?;
    if let Some(root) = root {
        super::safe_fs::copy_tree(&root, &stage.path().join("files"))?;
    } else {
        fs::create_dir(stage.path().join("files"))?;
        fs::create_dir(stage.path().join("files/_收件箱"))?;
        fs::create_dir(stage.path().join("files/_回收站"))?;
    }
    db.backup("main", stage.path().join("app.db"), None)?;
    let snapshot = db::connect(&stage.path().join("app.db"))?;
    snapshot.execute("DELETE FROM settings WHERE key='storage_root'", [])?;
    snapshot.execute(
        "INSERT OR REPLACE INTO settings(key,value) VALUES('backup_format','1')",
        [],
    )?;
    drop(snapshot);
    let inventory = inventory(stage.path())?;
    fs::write(stage.path().join("manifest.json"),serde_json::json!({"format":1,"schema":db::SCHEMA_VERSION,"appVersion":env!("CARGO_PKG_VERSION"),"createdAt":Utc::now().to_rfc3339(),"sha256":inventory}).to_string())?;
    let final_path = destination.join(name);
    super::safe_fs::rename_exclusive(stage.path(), &final_path)?;
    Ok(final_path)
}
pub fn restore(
    db: &mut Connection,
    backup_path: &Path,
    destination: &Path,
    safety_dir: &Path,
) -> AppResult<PathBuf> {
    let backup_path = backup_path.canonicalize()?;
    if super::files::is_link(&fs::symlink_metadata(backup_path.join("app.db"))?) {
        return Err(AppError::new("INVALID_BACKUP", "备份数据库不能是链接"));
    }
    let manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(backup_path.join("manifest.json"))?)
            .map_err(|_| AppError::new("INVALID_BACKUP", "备份清单无效"))?;
    if manifest["format"] != 1 {
        return Err(AppError::new("INVALID_BACKUP", "不支持的备份格式"));
    }
    let current = super::workspace::root(db)?.unwrap_or_else(|| backup_path.clone());
    let destination = empty_destination(destination, &current)?;
    let expected = manifest
        .get("sha256")
        .ok_or_else(|| AppError::new("INVALID_BACKUP", "备份缺少文件校验信息"))?;
    let actual = serde_json::to_value(inventory(&backup_path)?)
        .map_err(|_| AppError::new("INVALID_BACKUP", "无法校验备份"))?;
    if *expected != actual {
        return Err(AppError::new(
            "INVALID_BACKUP",
            "备份文件校验失败，未替换当前数据",
        ));
    }
    if destination.starts_with(&backup_path) || backup_path.starts_with(&destination) {
        return Err(AppError::new(
            "INVALID_DESTINATION",
            "恢复位置不能包含备份目录",
        ));
    }
    fs::create_dir_all(safety_dir)?;
    let safety = if (super::workspace::root(db)?.is_none()
        || super::workspace::available_root(db).is_ok())
        && require_settled(db).is_ok()
    {
        backup(db, safety_dir)?
    } else {
        // Disaster recovery must work even when the old disk is missing. Preserve
        // the original database and its operation journal without pretending to
        // have backed up inaccessible files.
        let folder = safety_dir.join(format!("恢复前数据库-{}", Uuid::new_v4()));
        fs::create_dir(&folder)?;
        db.backup("main", folder.join("app.db"), None)?;
        fs::write(folder.join("恢复说明.json"),serde_json::json!({"complete":false,"reason":"原目录不可访问或存在中断记录，仅保留原数据库，用户文件仍在原位置","originalRoot":super::workspace::root(db)?.map(|p|p.to_string_lossy().into_owned())}).to_string())?;
        folder
    };
    let stage = tempfile::Builder::new()
        .prefix(".project-manage-restore-")
        .tempdir_in(
            destination
                .parent()
                .ok_or_else(|| AppError::new("INVALID_DESTINATION", "无法使用该目录"))?,
        )?;
    let database = stage.path().join("app.db");
    fs::copy(backup_path.join("app.db"), &database)?;
    db::initialize(&database)?;
    let snapshot = db::connect(&database)?;
    let check: String = snapshot.query_row("PRAGMA integrity_check", [], |r| r.get(0))?;
    if check != "ok" {
        return Err(AppError::new("INVALID_BACKUP", "备份数据库完整性检查失败"));
    }
    let foreign_keys: bool = snapshot.prepare("PRAGMA foreign_key_check")?.exists([])?;
    if foreign_keys {
        return Err(AppError::new("INVALID_BACKUP", "备份关联数据校验失败"));
    }
    super::safe_fs::copy_tree(&backup_path.join("files"), &stage.path().join("files"))?;
    // Reject path escapes in a manually modified backup before publishing it.
    let mut statement = snapshot.prepare("SELECT directory_relpath FROM projects")?;
    for relative in statement.query_map([], |r| r.get::<_, String>(0))? {
        let relative = relative?;
        if Path::new(&relative).components().count() != 1
            || !matches!(
                Path::new(&relative).components().next(),
                Some(std::path::Component::Normal(_))
            )
        {
            return Err(AppError::new("INVALID_BACKUP", "备份项目目录无效"));
        }
    }
    drop(statement);
    fs::remove_dir(&destination)?;
    super::safe_fs::rename_exclusive(&stage.path().join("files"), &destination)?;
    snapshot.execute(
        "INSERT OR REPLACE INTO settings(key,value) VALUES('storage_root',?1)",
        [destination.to_string_lossy().as_ref()],
    )?;
    snapshot.execute("DELETE FROM settings WHERE key='backup_format'", [])?;
    // SQLite's backup API commits atomically to the existing connection, including WAL readers.
    rusqlite::backup::Backup::new(&snapshot, db)?.run_to_completion(
        128,
        Duration::from_millis(5),
        None,
    )?;
    Ok(safety)
}
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingOperation {
    pub id: String,
    pub kind: String,
    pub target: String,
    pub created_at: String,
}
pub fn pending(db: &Connection) -> AppResult<Vec<PendingOperation>> {
    let mut stmt=db.prepare("SELECT id,kind,target,created_at FROM filesystem_operations WHERE state!='completed' ORDER BY created_at")?;
    let rows = stmt
        .query_map([], |r| {
            Ok(PendingOperation {
                id: r.get(0)?,
                kind: r.get(1)?,
                target: r.get(2)?,
                created_at: r.get(3)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}
pub fn recover(db: &mut Connection, id: &str) -> AppResult<()> {
    let (kind, detail): (String, Option<String>) = db.query_row(
        "SELECT kind,detail FROM filesystem_operations WHERE id=?1 AND state!='completed'",
        [id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    if kind == "delete_file" {
        let detail = serde_json::from_str(&detail.unwrap_or_default())
            .map_err(|_| AppError::new("RECOVERY_ERROR", "文件删除恢复信息无效"))?;
        return super::files::recover_delete(db, id, &detail);
    }
    if kind == "rename_project" {
        let detail: serde_json::Value = serde_json::from_str(detail.as_deref().unwrap_or("{}"))
            .map_err(|_| AppError::new("RECOVERY_ERROR", "操作记录不完整，请检查项目目录"))?;
        let project = detail["projectId"]
            .as_str()
            .ok_or_else(|| AppError::new("RECOVERY_ERROR", "缺少项目信息"))?;
        let expected: String = db.query_row(
            "SELECT directory_relpath FROM projects WHERE id=?1",
            [project],
            |r| r.get(0),
        )?;
        let root = super::workspace::available_root(db)?;
        let expected_path = root.join(expected);
        if !expected_path.exists() {
            let mut candidates = Vec::new();
            for key in ["intermediate", "source", "target"] {
                if let Some(name) = detail[key].as_str() {
                    if Path::new(name).components().count() != 1
                        || !matches!(
                            Path::new(name).components().next(),
                            Some(std::path::Component::Normal(_))
                        )
                    {
                        return Err(AppError::new("RECOVERY_ERROR", "恢复路径无效"));
                    }
                    let path = root.join(name);
                    if path.exists()
                        && !super::files::is_link(&fs::symlink_metadata(&path)?)
                        && !candidates.contains(&path)
                    {
                        candidates.push(path);
                    }
                }
            }
            if candidates.len() != 1 {
                return Err(AppError::new(
                    "RECOVERY_ERROR",
                    "目录状态存在歧义，请先在文件管理器中核对",
                ));
            }
            super::safe_fs::rename_exclusive(&candidates[0], &expected_path)?;
        }
        super::projects::folder(db, project)?;
    }
    // Reconcile visible files, but never guess whether staging/unregistered files may be deleted.
    for project in super::projects::list(db)? {
        super::files::scan(db, &project.id)?;
    }
    super::files::scan(db, "__inbox__")?;
    db.execute(
        "UPDATE filesystem_operations SET state='completed' WHERE id=?1",
        params![id],
    )?;
    Ok(())
}

fn inventory(root: &Path) -> AppResult<BTreeMap<String, String>> {
    fn walk(
        root: &Path,
        path: &Path,
        result: &mut BTreeMap<String, String>,
        depth: usize,
    ) -> AppResult<()> {
        if depth > 64 || result.len() > 100000 {
            return Err(AppError::new("COPY_LIMIT", "备份目录过大"));
        }
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            let path = entry.path();
            let meta = fs::symlink_metadata(&path)?;
            if super::files::is_link(&meta) {
                return Err(AppError::new("INVALID_BACKUP", "备份包含链接"));
            }
            if meta.is_dir() {
                walk(root, &path, result, depth + 1)?;
            } else if meta.is_file() {
                let relative = path
                    .strip_prefix(root)
                    .map_err(|_| AppError::new("INVALID_BACKUP", "备份目录无效"))?
                    .to_string_lossy()
                    .replace('\\', "/");
                if relative == "manifest.json" {
                    continue;
                }
                let mut source = fs::File::open(path)?;
                let mut hash = Sha256::new();
                let mut buffer = [0u8; 65536];
                loop {
                    let read = source.read(&mut buffer)?;
                    if read == 0 {
                        break;
                    }
                    hash.update(&buffer[..read]);
                }
                result.insert(relative, format!("{:x}", hash.finalize()));
            } else {
                return Err(AppError::new("INVALID_BACKUP", "备份包含非常规文件"));
            }
        }
        Ok(())
    }
    let mut result = BTreeMap::new();
    walk(root, root, &mut result, 0)?;
    Ok(result)
}
