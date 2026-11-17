use super::{documents, io, journal};
use crate::{
    db,
    error::{AppError, AppResult},
    services,
};
use serde_json::json;
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};
use uuid::Uuid;
fn empty(target: &Path, source: Option<&Path>) -> AppResult<PathBuf> {
    let target = target.canonicalize()?;
    if fs::read_dir(&target)?.next().is_some()
        || source.is_some_and(|s| target.starts_with(s) || s.starts_with(&target))
    {
        return Err(AppError::new(
            "INVALID_DESTINATION",
            "请选择与当前目录互不包含的空文件夹",
        ));
    }
    Ok(target)
}
fn copy_contents(source: &Path, target: &Path) -> AppResult<()> {
    fn copy(
        root: &Path,
        source: &Path,
        target: &Path,
        depth: usize,
        count: &mut usize,
    ) -> AppResult<()> {
        if depth > 64 || *count > 100_000 {
            return Err(io::invalid("目录数量或层级超限"));
        }
        for entry in fs::read_dir(source)? {
            let entry = entry?;
            *count += 1;
            let relative = entry
                .path()
                .strip_prefix(root)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            if relative == ".workspace/write.lock" {
                continue;
            }
            let dest = target.join(entry.file_name());
            let meta = fs::symlink_metadata(entry.path())?;
            if services::files::is_link(&meta) {
                return Err(io::invalid("目录包含链接，不能完整复制"));
            }
            if meta.is_dir() {
                fs::create_dir(&dest)?;
                copy(root, &entry.path(), &dest, depth + 1, count)?;
            } else if meta.is_file() {
                let mut from = fs::File::open(entry.path())?;
                let mut to = fs::OpenOptions::new()
                    .create_new(true)
                    .write(true)
                    .open(&dest)?;
                std::io::copy(&mut from, &mut to)?;
                to.sync_all()?;
                let after = from.metadata()?;
                if after.len() != meta.len() || after.modified()? != meta.modified()? {
                    return Err(AppError::new(
                        "SOURCE_CHANGED",
                        "复制期间文件发生变化，请重试",
                    ));
                }
            } else {
                return Err(io::invalid("包含非常规文件"));
            }
        }
        Ok(())
    }
    copy(source, source, target, 0, &mut 0)
}

pub fn upgrade(bootstrap: &Path, target: &Path) -> AppResult<PathBuf> {
    if let Some(binding) = super::binding(bootstrap)? {
        super::validate_binding(&binding)?;
        let _lock = io::lock(&binding.root)?;
        journal::recover(&binding.root)?;
        let docs = super::load_binding(&binding)?;
        verify_resources(&binding.root, &docs)?;
        let source_inventory = inventory(&binding.root)?;
        let target = empty(target, Some(&binding.root))?;
        copy_contents(&binding.root, &target)?;
        if inventory(&target)? != source_inventory || inventory(&binding.root)? != source_inventory
        {
            return Err(AppError::new(
                "SOURCE_CHANGED",
                "复制期间源目录发生变化；原目录未切换",
            ));
        }
        documents::load(&target)?;
        drop(_lock);
        super::open(bootstrap, &target)?;
        return Ok(target);
    }
    let legacy = db::connect(bootstrap)?;
    if !services::data::pending(&legacy)?.is_empty() {
        return Err(AppError::new(
            "PENDING_OPERATIONS",
            "请先在设置中恢复旧版未完成的文件操作，再升级目录",
        ));
    }
    let source = services::workspace::root(&legacy)?;
    let original_inventory = source.as_ref().map(|root| inventory(root)).transpose()?;
    let target = empty(target, source.as_deref())?;
    let stage = tempfile::Builder::new()
        .prefix(".workspace-upgrade-")
        .tempdir_in(target.parent().unwrap())?;
    let old = stage.path().join("legacy.db");
    legacy.backup("main", &old, None)?;
    let mut cache = db::connect(&old)?;
    let projects = services::projects::list(&cache)?;
    fs::create_dir(stage.path().join("Files"))?;
    if let Some(source) = &source {
        for project in &projects {
            let src = services::projects::folder(&cache, &project.id)?;
            let destination = stage.path().join(&project.directory_relpath);
            fs::create_dir(&destination)?;
            services::safe_fs::copy_tree(&src, &destination.join("Files"))?;
        }
        if source.join("_收件箱").is_dir() {
            copy_contents(&source.join("_收件箱"), &stage.path().join("Files"))?;
        }
        if source.join("_回收站").is_dir() {
            services::safe_fs::copy_tree(&source.join("_回收站"), &stage.path().join("_回收站"))?;
        } else {
            fs::create_dir(stage.path().join("_回收站"))?;
        }
        // Preserve unregistered root content as explicit unclassified files rather than omit it.
        for entry in fs::read_dir(source)? {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().into_owned();
            if matches!(name.as_str(), "_收件箱" | "_回收站")
                || projects.iter().any(|p| p.directory_relpath == name)
            {
                continue;
            }
            let dest = stage.path().join("Files").join(&name);
            if entry.file_type()?.is_dir() {
                services::safe_fs::copy_tree(&entry.path(), &dest)?;
            } else if services::files::is_link(&fs::symlink_metadata(entry.path())?) {
                return Err(io::invalid("未管理内容包含链接，请先处理"));
            } else {
                fs::copy(entry.path(), dest)?;
            }
        }
    }
    let tx = cache.transaction()?;
    if let Some(source) = &source {
        tx.execute(
            "INSERT OR REPLACE INTO settings(key,value) VALUES('_legacy_root',?1)",
            [source.to_string_lossy().as_ref()],
        )?;
    }

    tx.execute(
        "INSERT OR REPLACE INTO settings(key,value) VALUES('_portable','1')",
        [],
    )?;
    tx.execute(
        "UPDATE projects SET directory_relpath='Files' WHERE id='__inbox__'",
        [],
    )?;
    tx.execute(
        "INSERT OR REPLACE INTO settings(key,value) VALUES('storage_root',?1)",
        [stage.path().to_string_lossy().as_ref()],
    )?;
    tx.commit()?;
    for project in &projects {
        services::files::scan(&mut cache, &project.id)?;
    }
    services::files::scan(&mut cache, "__inbox__")?;
    super::initialize(stage.path(), &cache)?;
    drop(cache);
    fs::remove_file(&old)?;
    for ext in ["db-wal", "db-shm"] {
        let side = old.with_extension(ext);
        if side.exists() {
            fs::remove_file(side)?;
        }
    }
    documents::load(stage.path())?;
    if let Some(source) = &source {
        if Some(inventory(source)?) != original_inventory {
            return Err(AppError::new(
                "SOURCE_CHANGED",
                "升级期间原目录发生变化，请重试；原数据未切换",
            ));
        }
    }
    let staged_inventory = inventory(stage.path())?;
    copy_contents(stage.path(), &target)?;
    if inventory(&target)? != staged_inventory {
        return Err(io::invalid("升级目标校验失败，原数据未切换"));
    }
    super::open(bootstrap, &target)?;
    Ok(target)
}
pub fn inventory(root: &Path) -> AppResult<BTreeMap<String, String>> {
    fn walk(
        root: &Path,
        dir: &Path,
        out: &mut BTreeMap<String, String>,
        depth: usize,
    ) -> AppResult<()> {
        if depth > 64 || out.len() > 100_000 {
            return Err(io::invalid("备份层级或文件数量超限"));
        }
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let name = entry
                .path()
                .strip_prefix(root)
                .unwrap()
                .to_str()
                .ok_or_else(|| io::invalid("文件路径编码无效"))?
                .replace('\\', "/");
            if name == "manifest.json" || name == ".workspace/write.lock" {
                continue;
            }
            let meta = fs::symlink_metadata(entry.path())?;
            if services::files::is_link(&meta) {
                return Err(io::invalid("备份包含链接"));
            }
            if meta.is_dir() {
                walk(root, &entry.path(), out, depth + 1)?;
            } else if meta.is_file() {
                out.insert(name, io::hash(&entry.path())?);
            } else {
                return Err(io::invalid("包含非常规文件"));
            }
        }
        Ok(())
    }
    let mut out = BTreeMap::new();
    walk(root, root, &mut out, 0)?;
    Ok(out)
}
pub fn backup(bootstrap: &Path, destination: &Path) -> AppResult<PathBuf> {
    let Some(binding) = super::binding(bootstrap)? else {
        return services::data::backup(&db::connect(bootstrap)?, destination);
    };
    super::validate_binding(&binding)?;
    let _lock = io::lock(&binding.root)?;
    journal::recover(&binding.root)?;
    let docs = super::load_binding(&binding)?;
    verify_resources(&binding.root, &docs)?;
    let destination = destination.canonicalize()?;
    if destination.starts_with(&binding.root) {
        return Err(io::invalid("备份位置不能与工作目录互相包含"));
    }
    let stage = tempfile::Builder::new()
        .prefix(".workspace-backup-")
        .tempdir_in(&destination)?;
    fs::create_dir(stage.path().join("workspace"))?;
    copy_contents(&binding.root, &stage.path().join("workspace"))?;
    let copied = stage.path().join("workspace/.workspace/write.lock");
    if copied.exists() {
        fs::remove_file(copied)?;
    }
    let before = inventory(&binding.root)?;
    let after = inventory(&stage.path().join("workspace"))?;
    if before != after {
        return Err(AppError::new(
            "SOURCE_CHANGED",
            "备份期间源文件变化，请重试",
        ));
    }
    let recovery = bootstrap.parent().unwrap().join("recovery");
    let recovery_count = if recovery.is_dir() {
        let count = inventory(&recovery)?.len();
        if count > 0 {
            fs::create_dir(stage.path().join("recovery"))?;
            copy_contents(&recovery, &stage.path().join("recovery"))?;
        }
        count
    } else {
        0
    };
    let hashes = inventory(stage.path())?;
    io::write(
        &stage.path().join("manifest.json"),
        &json!({"format":2,"formatVersion":1,"recoveryCount":recovery_count,"recoveryNote":"恢复附录未自动合并到待办","appVersion":env!("CARGO_PKG_VERSION"),"createdAt":chrono::Utc::now().to_rfc3339(),"sha256":hashes}),
    )?;
    let final_path = destination.join(format!(
        "项目工作台备份-{}-{}",
        chrono::Utc::now().format("%Y%m%d-%H%M%S"),
        Uuid::new_v4()
    ));
    services::safe_fs::rename_exclusive(stage.path(), &final_path)?;
    Ok(final_path)
}
pub fn restore(bootstrap: &Path, source: &Path, target: &Path) -> AppResult<PathBuf> {
    let manifest = io::read(&source.join("manifest.json"))?;
    let current = super::binding(bootstrap)?;
    let legacy_root = if current.is_none() {
        services::workspace::root(&db::connect(bootstrap)?)?
    } else {
        None
    };
    let target = empty(
        target,
        current
            .as_ref()
            .map(|b| b.root.as_path())
            .or(legacy_root.as_deref()),
    )?;

    if manifest["format"] == 1 {
        let temporary = tempfile::tempdir()?;
        let old = temporary.path().join("app.db");
        db::initialize(&old)?;
        let mut legacy = db::connect(&old)?;
        let legacy_root = temporary.path().join("legacy-files");
        fs::create_dir(&legacy_root)?;
        let safety = temporary.path().join("safety");
        fs::create_dir(&safety)?;
        services::data::restore(&mut legacy, source, &legacy_root, &safety)?;
        drop(legacy);
        safety_backup(bootstrap, target.parent().unwrap())?;
        let result = upgrade(&old, &target)?;
        super::open(bootstrap, &result)?;
        return Ok(result);
    }
    if manifest["format"] != 2 {
        return Err(io::invalid("备份格式不受支持"));
    }
    let expected: BTreeMap<String, String> = serde_json::from_value(manifest["sha256"].clone())
        .map_err(|_| io::invalid("备份清单无效"))?;
    if inventory(source)? != expected {
        return Err(io::invalid("备份损坏或缺少文件"));
    }
    let target = empty(
        &target,
        super::binding(bootstrap)?
            .as_ref()
            .map(|b| b.root.as_path()),
    )?;
    let snapshot = documents::load(&source.join("workspace"))?;
    verify_resources(&source.join("workspace"), &snapshot)?;
    safety_backup(bootstrap, target.parent().unwrap())?;
    copy_contents(&source.join("workspace"), &target)?;
    let expected_workspace = expected
        .iter()
        .filter_map(|(name, hash)| {
            name.strip_prefix("workspace/")
                .map(|name| (name.to_string(), hash.clone()))
        })
        .collect::<BTreeMap<_, _>>();
    if inventory(&target)? != expected_workspace {
        return Err(io::invalid("恢复目标校验失败，原数据未切换"));
    }
    verify_resources(&target, &documents::load(&target)?)?;
    if source.join("recovery").is_dir() {
        let recovery = bootstrap.parent().unwrap().join("recovery");
        fs::create_dir_all(&recovery)?;
        services::safe_fs::copy_tree(
            &source.join("recovery"),
            &recovery.join(format!("imported-{}", Uuid::new_v4())),
        )?;
    }
    super::open(bootstrap, &target)?;
    Ok(target)
}

pub fn import_project(bootstrap: &Path, source: &Path) -> AppResult<PathBuf> {
    let source = source.canonicalize()?;
    let mut project = io::read(&io::path(&source, "config.json")?)?;
    let mut todo = io::read(&io::path(&source, "todo.json")?)?;
    if project["formatVersion"] != 1
        || project["kind"] != "project"
        || todo["formatVersion"] != 1
        || todo["kind"] != "todos"
        || todo["owner"]["id"] != project["id"]
    {
        return Err(io::invalid("请选择受支持的完整项目文件夹"));
    }
    verify_resources(
        &source,
        &BTreeMap::from([
            ("config.json".into(), project.clone()),
            ("todo.json".into(), todo.clone()),
        ]),
    )?;
    let name = documents::text(&project, "name")?.to_string();
    services::projects::normalized_name(&name)?;
    let original = inventory(&source)?;
    super::edit_documents(bootstrap, |root, docs| {
        if source == root || root.starts_with(&source) {
            return Err(io::invalid("不能将工作目录本身作为项目导入"));
        }
        let mut name = name.clone();
        let base = name.chars().take(80).collect::<String>();
        let existing = documents::items(&docs["config.json"], "projects")?;
        for i in 0..10000 {
            let normalized = services::projects::normalized_name(&name)?;
            if !root.join(&name).exists()
                && !existing.iter().any(|p| {
                    services::projects::normalized_name(p["directory"].as_str().unwrap_or(""))
                        .ok()
                        .as_ref()
                        == Some(&normalized)
                })
            {
                break;
            }
            name = if i == 0 {
                format!("{base} (副本)")
            } else {
                format!("{base} (副本 {})", i + 1)
            };
        }
        // Keep rollback staging alive until the outer journal has committed or recovered.
        let stage = root
            .join(".workspace")
            .join(format!("import-{}", Uuid::new_v4()));
        fs::create_dir(&stage)?;
        let copied = stage.join("project");
        services::safe_fs::copy_tree(&source, &copied)?;
        if inventory(&copied)? != original {
            return Err(AppError::new(
                "SOURCE_CHANGED",
                "导入期间源项目发生变化，请重试",
            ));
        }
        fs::remove_file(copied.join("config.json"))?;
        fs::remove_file(copied.join("todo.json"))?;
        let mut ids = BTreeMap::new();
        let pid = Uuid::new_v4().to_string();
        ids.insert(documents::text(&project, "id")?.to_owned(), pid.clone());
        for file in documents::items(&project, "files")? {
            ids.insert(
                documents::text(file, "id")?.to_owned(),
                Uuid::new_v4().to_string(),
            );
        }
        for link in documents::items(&project, "links")? {
            ids.insert(
                documents::text(link, "id")?.to_owned(),
                Uuid::new_v4().to_string(),
            );
        }
        for task in documents::items(&todo, "tasks")? {
            ids.insert(
                documents::text(task, "id")?.to_owned(),
                Uuid::new_v4().to_string(),
            );
            for reminder in documents::items(task, "reminders")? {
                ids.insert(
                    documents::text(reminder, "id")?.to_owned(),
                    Uuid::new_v4().to_string(),
                );
            }
        }
        for field in ["files", "links"] {
            for item in project[field].as_array_mut().unwrap() {
                item["id"] = json!(ids[documents::text(item, "id")?]);
            }
        }
        for task in todo["tasks"].as_array_mut().unwrap() {
            task["id"] = json!(ids[documents::text(task, "id")?]);
            for fid in task["attachmentIds"].as_array_mut().unwrap() {
                *fid = json!(ids
                    .get(fid.as_str().ok_or_else(|| io::invalid("附件引用无效"))?)
                    .ok_or_else(|| io::invalid("附件引用不属于项目"))?);
            }
            for r in task["reminders"].as_array_mut().unwrap() {
                r["id"] = json!(ids[documents::text(r, "id")?]);
            }
            task.as_object_mut().unwrap().remove("sourceDraftId");
        }
        let wid = docs["config.json"]["workspaceId"].clone();
        project["id"] = json!(pid);
        project["name"] = json!(name);
        project["directory"] = json!(name);
        project["workspaceId"] = wid.clone();
        project["revision"] = json!(1);
        todo["workspaceId"] = wid;
        todo["owner"] = json!({"type":"project","id":pid});
        todo["revision"] = json!(1);
        // Old deletion markers belong to the source identity; a new copy has fresh identities.
        project["tombstones"] = json!([]);
        todo["tombstones"] = json!([]);
        let target = root.join(&name);
        services::safe_fs::rename_exclusive(&copied, &target)?;
        docs.get_mut("config.json").unwrap()["projects"]
            .as_array_mut()
            .unwrap()
            .push(json!({"id":pid,"directory":name}));
        docs.insert(format!("{name}/config.json"), project);
        docs.insert(format!("{name}/todo.json"), todo);
        Ok(target)
    })
}

fn verify_resources(root: &Path, docs: &documents::Documents) -> AppResult<()> {
    let check = |path: String, expected: &serde_json::Value| -> AppResult<()> {
        let file = io::path(root, &path)?;
        if io::hash(&file)?
            != expected
                .as_str()
                .ok_or_else(|| io::invalid("文件缺失，无法报告完整备份"))?
        {
            return Err(io::invalid("资源缺失或发生变化，请先检查文件"));
        }
        Ok(())
    };
    for (name, doc) in docs {
        let parent = Path::new(name).parent().unwrap().to_string_lossy();
        let prefix = if parent.is_empty() {
            String::new()
        } else {
            format!("{parent}/")
        };
        if matches!(doc["kind"].as_str(), Some("workspace" | "project")) {
            for file in documents::items(doc, "files")? {
                check(
                    format!("{prefix}Files/{}", documents::text(file, "relativePath")?),
                    &file["sha256"],
                )?;
            }
            if let Some(links) = doc["links"].as_array() {
                for link in links {
                    if let Some(asset) = link.get("iconAsset") {
                        check(
                            format!("{prefix}{}", documents::text(asset, "path")?),
                            &asset["sha256"],
                        )?;
                    }
                }
            }
        }
        if doc["kind"] == "draft" {
            for file in documents::items(doc, "attachments")? {
                check(
                    format!("{prefix}{}", documents::text(file, "relativePath")?),
                    &file["sha256"],
                )?;
            }
        }
    }
    Ok(())
}
fn safety_backup(bootstrap: &Path, parent: &Path) -> AppResult<()> {
    let target = parent.join(format!("恢复前备份-{}", Uuid::new_v4()));
    fs::create_dir(&target)?;
    if backup(bootstrap, &target).is_ok() {
        return Ok(());
    }
    // A damaged/offline workspace is retained as a rescue export, never labelled a full backup.
    let info = super::info(bootstrap)?;
    if let Some(root) = info["root"]
        .as_str()
        .map(PathBuf::from)
        .filter(|p| p.is_dir())
    {
        fs::create_dir(target.join("raw-workspace"))?;
        copy_contents(&root, &target.join("raw-workspace"))?;
    }
    if bootstrap.exists() {
        db::connect(bootstrap)?.backup("main", target.join("legacy.db"), None)?;
    }
    let device = bootstrap.with_file_name("device.json");
    if device.exists() {
        fs::copy(device, target.join("device.json"))?;
    }
    io::write(
        &target.join("rescue.json"),
        &json!({"kind":"rescue","completeBusinessBackup":false,"storageState":info["state"],"createdAt":chrono::Utc::now().to_rfc3339()}),
    )
}

/// A raw, explicitly incomplete rescue copy is available even when protocol validation fails.
pub fn rescue(bootstrap: &Path, destination: &Path) -> AppResult<PathBuf> {
    let destination = destination.canonicalize()?;
    let info = super::info(bootstrap)?;
    let source = info["root"]
        .as_str()
        .map(PathBuf::from)
        .filter(|p| p.is_dir());
    if source.as_ref().is_some_and(|p| destination.starts_with(p)) {
        return Err(io::invalid("诊断导出位置不能在当前工作目录内"));
    }
    let target = destination.join(format!("目录诊断副本-{}", Uuid::new_v4()));
    fs::create_dir(&target)?;
    if let Some(source) = source {
        let before = inventory(&source)?;
        fs::create_dir(target.join("raw-workspace"))?;
        copy_contents(&source, &target.join("raw-workspace"))?;
        if inventory(&target.join("raw-workspace"))? != before || inventory(&source)? != before {
            return Err(AppError::new(
                "SOURCE_CHANGED",
                "导出期间原目录发生变化，请重试；原件未修改",
            ));
        }
    }
    let recovery = bootstrap.parent().unwrap().join("recovery");
    if recovery.is_dir() {
        crate::services::safe_fs::copy_tree(&recovery, &target.join("recovery"))?;
    }
    io::write(
        &target.join("诊断说明.json"),
        &json!({"formatVersion":1,"kind":"rescueExport","completeBusinessBackup":false,"storage":info,"note":"原始目录与恢复记录，未作为完整业务备份验证。请保留原件，修复后打开已有目录或从完整备份恢复。"}),
    )?;
    Ok(target)
}
