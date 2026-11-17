//! Emergency drafts stay on this device and are never reported as a workspace save.
use super::{binding, execute, io, save_capture, transfer};
use crate::error::{AppError, AppResult};
use serde_json::json;
use std::{
    fs,
    path::{Path, PathBuf},
};
use uuid::Uuid;

pub fn save(
    bootstrap: &Path,
    expected: Option<&str>,
    text: Option<&str>,
    image: Option<&Path>,
    reason: &str,
) -> AppResult<PathBuf> {
    let current = binding(bootstrap)?;
    let label = expected
        .or_else(|| current.as_ref().map(|b| b.id.as_str()))
        .unwrap_or("unbound");
    if label != "unbound" {
        Uuid::parse_str(label).map_err(|_| io::invalid("恢复稿绑定标识无效"))?;
    }
    let dir = bootstrap.parent().unwrap().join("recovery").join(label);
    let file = dir.join("draft.json");
    let mut draft = if file.exists() {
        io::read(&file)?
    } else {
        json!({"formatVersion":1,"kind":"recoveryDraft","id":Uuid::new_v4().to_string(),"bindingId":label,"text":"","image":null})
    };
    draft["reason"] = json!(reason);
    draft["savedAt"] = json!(chrono::Utc::now().to_rfc3339());
    if let Some(text) = text {
        draft["text"] = json!(text);
    }
    if let Some(source) = image {
        // Publish the resource before the recovery manifest; preserve every captured image.
        let name = format!("{}.png", Uuid::new_v4());
        let target = dir.join(&name);
        io::atomic(&target, &fs::read(source)?)?;
        let mut images = draft["images"].as_array().cloned().unwrap_or_default();
        images.push(json!({"path":name,"sha256":io::hash(&target)?}));
        draft["images"] = json!(images);
        draft["image"] = json!(target.file_name().unwrap().to_string_lossy());
        io::write(&file, &draft)?;
        return Ok(target);
    }
    io::write(&file, &draft)?;
    Ok(file)
}
pub fn count(bootstrap: &Path) -> AppResult<usize> {
    let root = bootstrap.parent().unwrap().join("recovery");
    if !root.exists() {
        return Ok(0);
    }
    Ok(transfer::inventory(&root)?
        .keys()
        .filter(|p| p.ends_with("draft.json") || p.ends_with("capture.png"))
        .count())
}
pub fn export(bootstrap: &Path, destination: &Path) -> AppResult<PathBuf> {
    let root = bootstrap.parent().unwrap().join("recovery");
    let destination = destination.canonicalize()?;
    if !root.exists() || destination.starts_with(&root) {
        return Err(io::invalid("没有可导出的恢复稿，或导出位置无效"));
    }
    let target = destination.join(format!("本机恢复稿-{}", Uuid::new_v4()));
    crate::services::safe_fs::copy_tree(&root, &target)?;
    if transfer::inventory(&root)? != transfer::inventory(&target)? {
        return Err(AppError::new(
            "SOURCE_CHANGED",
            "恢复稿发生变化，请重试；原件已保留",
        ));
    }
    io::write(
        &target.join("说明.json"),
        &json!({"formatVersion":1,"kind":"recoveryExport","completeBusinessBackup":false,"note":"本机尚未归档的采集内容，不是完整工作目录备份"}),
    )?;
    Ok(target)
}
pub fn restore_as_tasks(bootstrap: &Path) -> AppResult<usize> {
    let root = bootstrap.parent().unwrap().join("recovery");
    if !root.exists() {
        return Ok(0);
    }
    let unbound = root.join("unbound/capture.png");
    if unbound.is_file() && !root.join("unbound/draft.json").exists() {
        io::write(
            &root.join("unbound/draft.json"),
            &json!({"formatVersion":1,"kind":"recoveryDraft","id":Uuid::new_v4().to_string(),"bindingId":"unbound","text":"","images":[{"path":"capture.png","sha256":io::hash(&unbound)?}]}),
        )?;
    }
    let mut count = 0;
    for name in transfer::inventory(&root)?
        .keys()
        .filter(|p| p.ends_with("draft.json"))
    {
        let file = io::path(&root, name)?;
        let record = io::read(&file)?;
        if record["formatVersion"] != 1 || record["kind"] != "recoveryDraft" {
            return Err(io::invalid("恢复稿格式无效，已保留原件"));
        }
        let id = super::documents::text(&record, "id")?;
        let text = super::documents::text(&record, "text")?;
        let parent = file.parent().unwrap();
        let images = record["images"].as_array().cloned().unwrap_or_default();
        let paths = images
            .iter()
            .map(|v| {
                let path = io::path(parent, super::documents::text(v, "path")?)?;
                if io::hash(&path)? != super::documents::text(v, "sha256")? {
                    return Err(io::invalid("恢复图片校验失败"));
                }
                Ok(path)
            })
            .collect::<AppResult<Vec<_>>>()?;
        if text.trim().is_empty() && paths.is_empty() {
            continue;
        }
        let created = execute(bootstrap, true, |db| {
            // A recovered item never consumes a different draft already saved in the workspace.
            let existed = crate::services::tasks::list(db)?.iter().any(|t| t.id == id);
            if !existed {
                let draft =
                    crate::services::preferences::get(db, "capture_draft")?.unwrap_or_default();
                let target =
                    crate::services::preferences::get(db, "capture_project")?.unwrap_or_default();
                let task = save_capture(db, text, None, paths.first().map(PathBuf::as_path), id)?;
                for path in paths.iter().skip(1) {
                    crate::services::attachments::import(db, &task.id, path)?;
                }
                crate::services::preferences::set(db, "capture_draft", &draft)?;
                crate::services::preferences::set(db, "capture_project", &target)?;
                db.execute("DELETE FROM settings WHERE key='_consume_capture'", [])?;
            }
            Ok(!existed)
        })?;
        let archive = bootstrap
            .parent()
            .unwrap()
            .join("recovered")
            .join(format!("{id}-{}", Uuid::new_v4()));
        fs::create_dir_all(archive.parent().unwrap())?;
        if archive.exists() {
            return Err(io::invalid("恢复归档目录已存在，原件已保留"));
        }
        fs::rename(parent, archive)?;
        count += usize::from(created);
    }
    Ok(count)
}
