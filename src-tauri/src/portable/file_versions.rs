use super::{documents, io};
use crate::{error::AppResult, services};
use rusqlite::Connection;

/// Inspect only files whose metadata differs. Full backups always verify every digest.
pub fn changed(db: &Connection, project: &str) -> AppResult<Vec<String>> {
    if !super::is_portable(db) {
        return Ok(vec![]);
    }
    let root = services::workspace::available_root(db)?;
    let docs = documents::load(&root)?;
    let meta = if project == "__inbox__" {
        &docs["config.json"]
    } else {
        docs.values()
            .find(|v| v["kind"] == "project" && v["id"] == project)
            .ok_or_else(|| io::invalid("项目不存在"))?
    };
    let directory = if project == "__inbox__" {
        "Files".to_string()
    } else {
        format!("{}/Files", documents::text(meta, "directory")?)
    };
    let mut out = vec![];
    for file in documents::items(meta, "files")? {
        let path = io::path(
            &root,
            &format!("{directory}/{}", documents::text(file, "relativePath")?),
        )?;
        if !path.is_file() {
            continue;
        }
        let metadata = std::fs::metadata(&path)?;
        let modified: chrono::DateTime<chrono::Utc> = metadata.modified()?.into();
        if file["size"] != metadata.len() || file["modifiedAt"] != modified.to_rfc3339() {
            if let Some(digest) = file["sha256"].as_str() {
                if io::hash(&path)? != digest {
                    out.push(documents::text(file, "id")?.to_owned());
                }
            }
        }
    }
    Ok(out)
}
pub fn accept(db: &Connection, project: &str, ids: Vec<String>) -> AppResult<()> {
    let changed = changed(db, project)?;
    if ids.iter().any(|id| !changed.contains(id)) {
        return Err(io::invalid("文件列表已变化，请重新检查"));
    }
    db.execute(
        "INSERT OR REPLACE INTO settings(key,value) VALUES('_accept_file_versions',?1)",
        [serde_json::to_string(&ids).unwrap()],
    )?;
    Ok(())
}
