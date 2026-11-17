//! Portable JSON is authoritative. SQL connections are disposable domain workspaces.
pub mod documents;
pub mod file_versions;
pub mod io;
pub mod journal;
pub mod recovery;
pub mod transfer;
use crate::{
    db,
    error::{AppError, AppResult},
    services,
};
use documents::Documents;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{
    fs,
    path::{Path, PathBuf},
};
use uuid::Uuid;

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Binding {
    pub id: String,
    pub workspace_id: String,
    pub root: PathBuf,
    pub fingerprint: String,
}
#[derive(Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Device {
    #[serde(default)]
    active: Option<Binding>,
}
fn device_file(bootstrap: &Path) -> PathBuf {
    bootstrap.with_file_name("device.json")
}
pub fn binding(bootstrap: &Path) -> AppResult<Option<Binding>> {
    let file = device_file(bootstrap);
    if !file.exists() {
        return Ok(None);
    }
    let device: Device = serde_json::from_value(io::read(&file)?)
        .map_err(|_| io::invalid("本机目录绑定损坏，请重新选择工作目录"))?;
    Ok(device.active)
}
pub fn cache_path(bootstrap: &Path, binding: &Binding) -> PathBuf {
    bootstrap
        .parent()
        .unwrap()
        .join("cache")
        .join(format!("{}.db", binding.id))
}
pub fn is_portable(db: &Connection) -> bool {
    db.query_row(
        "SELECT value='1' FROM settings WHERE key='_portable'",
        [],
        |r| r.get(0),
    )
    .unwrap_or(false)
}
pub fn connect_cache(bootstrap: &Path, binding: &Binding) -> AppResult<Connection> {
    let file = cache_path(bootstrap, binding);
    fs::create_dir_all(file.parent().unwrap())?;
    if db::initialize(&file).is_err() {
        let quarantine = file.with_extension(format!("damaged-{}", Uuid::new_v4()));
        fs::rename(&file, quarantine)?;
        for ext in ["db-wal", "db-shm"] {
            let side = file.with_extension(ext);
            if side.exists() {
                fs::rename(
                    &side,
                    side.with_extension(format!("damaged-{}", Uuid::new_v4())),
                )?;
            }
        }
        db::initialize(&file)?;
    }
    db::connect(&file)
}
pub fn validate_binding(binding: &Binding) -> AppResult<()> {
    if binding.root.canonicalize().ok().as_ref() != Some(&binding.root)
        || io::fingerprint(&binding.root).ok().as_ref() != Some(&binding.fingerprint)
    {
        return Err(AppError::new(
            "STORAGE_UNAVAILABLE",
            "工作目录离线或身份变化，请重新选择；未创建替代目录",
        ));
    }
    Ok(())
}
pub fn open(bootstrap: &Path, root: &Path) -> AppResult<crate::models::WorkspaceStatus> {
    let root = root.canonicalize()?;
    let _lock = io::lock(&root)?;
    journal::recover(&root)?;
    let docs = documents::load(&root)?;
    for (name, doc) in &docs {
        if matches!(doc["kind"].as_str(), Some("workspace" | "project"))
            && documents::items(doc, "files")?.is_empty()
        {
            let parent = Path::new(name).parent().unwrap();
            let relative = if parent.as_os_str().is_empty() {
                "Files".into()
            } else {
                format!("{}/Files", parent.to_string_lossy())
            };
            let directory = io::path(&root, &relative)?;
            if !directory.exists() {
                fs::create_dir(directory)?;
            }
        }
    }
    let old = match binding(bootstrap) {
        Ok(old) => old,
        Err(_) => {
            let damaged = device_file(bootstrap);
            if damaged.exists() {
                fs::copy(
                    &damaged,
                    damaged.with_file_name(format!("device.damaged-{}.json", Uuid::new_v4())),
                )?;
            }
            None
        }
    };
    let binding = Binding {
        id: old
            .filter(|b| b.root == root && b.workspace_id == docs["config.json"]["workspaceId"])
            .map(|b| b.id)
            .unwrap_or_else(|| Uuid::new_v4().to_string()),
        workspace_id: documents::text(&docs["config.json"], "workspaceId")?.into(),
        fingerprint: io::fingerprint(&root)?,
        root,
    };
    let mut cache = connect_cache(bootstrap, &binding)?;
    documents::hydrate(&mut cache, &binding.root, &docs)?;
    cache.execute(
        "INSERT OR REPLACE INTO settings(key,value) VALUES('_portable_commit',?1)",
        [documents::text(
            &docs["config.json"]["snapshot"],
            "commitId",
        )?],
    )?;
    cache.execute(
        "INSERT OR REPLACE INTO settings(key,value) VALUES('_binding_id',?1)",
        [&binding.id],
    )?;
    io::write(
        &device_file(bootstrap),
        &serde_json::to_value(Device {
            active: Some(binding),
        })
        .unwrap(),
    )?;
    services::workspace::status(&cache)
}
pub fn select(bootstrap: &Path, root: &Path) -> AppResult<crate::models::WorkspaceStatus> {
    if root.join("config.json").exists() {
        return open(bootstrap, root);
    }
    if binding(bootstrap)?.is_some() {
        return Err(AppError::new(
            "ROOT_MIGRATION_REQUIRED",
            "请使用迁移目录，或选择已有工作目录；当前数据未更改",
        ));
    }
    let db = db::connect(bootstrap)?;
    let count: i64 = db.query_row(
        "SELECT (SELECT COUNT(*) FROM projects WHERE id!='__inbox__')+(SELECT COUNT(*) FROM tasks)",
        [],
        |r| r.get(0),
    )?;
    if count > 0 || fs::read_dir(root)?.next().is_some() {
        return Err(AppError::new(
            "ROOT_MIGRATION_REQUIRED",
            "此目录非空或存在旧数据，请使用“升级/迁移目录”复制转换到空目录",
        ));
    }
    initialize(root, &db)?;
    open(bootstrap, root)
}
pub fn initialize(root: &Path, db: &Connection) -> AppResult<()> {
    fs::create_dir_all(root.join("Files"))?;
    fs::create_dir_all(root.join("_回收站"))?;
    let docs = documents::export(db, root, &Documents::new())?;
    let _lock = io::lock(root)?;
    journal::begin(root, Documents::new())?;
    let _scope = journal::scope();
    journal::commit(docs)
}
pub fn execute<T>(
    bootstrap: &Path,
    write: bool,
    operation: impl FnOnce(&mut Connection) -> AppResult<T>,
) -> AppResult<T> {
    let Some(binding) = binding(bootstrap)? else {
        return operation(&mut db::connect(bootstrap)?);
    };
    let readable = validate_binding(&binding).and_then(|()| {
        let lock = io::lock(&binding.root)?;
        journal::recover(&binding.root)?;
        let before = load_binding(&binding)?;
        Ok((lock, before))
    });
    let (_lock, before) = match readable {
        Ok(value) => value,
        Err(error) => {
            if !write && cache_path(bootstrap, &binding).exists() {
                let mut cache = connect_cache(bootstrap, &binding)?;
                let usable: bool = cache
                    .query_row(
                        "SELECT EXISTS(SELECT 1 FROM settings WHERE key='_portable_commit')",
                        [],
                        |r| r.get(0),
                    )
                    .unwrap_or(false);
                if usable {
                    cache.execute(
                        "INSERT OR REPLACE INTO settings(key,value) VALUES('_read_only','1')",
                        [],
                    )?;
                    cache.execute(
                        "INSERT OR REPLACE INTO settings(key,value) VALUES('_storage_error',?1)",
                        [&error.message],
                    )?;
                    return operation(&mut cache);
                }
            }
            return Err(error);
        }
    };
    let mut cache = connect_cache(bootstrap, &binding)?;
    let commit = documents::text(&before["config.json"]["snapshot"], "commitId")?;
    let cached: String = cache
        .query_row(
            "SELECT value FROM settings WHERE key='_portable_commit'",
            [],
            |r| r.get(0),
        )
        .unwrap_or_default();
    if cached != commit {
        documents::hydrate(&mut cache, &binding.root, &before)?;
        cache.execute(
            "INSERT OR REPLACE INTO settings(key,value) VALUES('_portable_commit',?1)",
            [commit],
        )?;
    }
    cache.execute(
        "DELETE FROM settings WHERE key IN ('_read_only','_storage_error')",
        [],
    )?;
    cache.execute(
        "INSERT OR REPLACE INTO settings(key,value) VALUES('_binding_id',?1)",
        [&binding.id],
    )?;
    if !write {
        return operation(&mut cache);
    }
    journal::begin(&binding.root, before.clone())?;
    let _scope = journal::scope();
    cache.execute("DELETE FROM settings WHERE key='_portable_commit'", [])?;
    let result = operation(&mut cache);
    match result {
        Ok(value) => {
            match documents::export(&cache, &binding.root, &before) {
                Ok(after) => {
                    cache.execute("DELETE FROM settings WHERE key IN ('_consume_capture','_accept_file_versions')", [])?;
                    if let Err(error) = validate_binding(&binding) {
                        journal::detach();
                        return Err(error);
                    }
                    let commit = after["config.json"]["snapshot"]["commitId"]
                        .as_str()
                        .unwrap()
                        .to_owned();
                    if after == before {
                        journal::abort(&binding.root)?;
                        cache.execute("INSERT OR REPLACE INTO settings(key,value) VALUES('_portable_commit',?1)", [commit])?;
                        return Ok(value);
                    }
                    if let Err(error) = journal::commit(after) {
                        journal::detach();
                        return Err(error);
                    }
                    cache.execute(
                        "INSERT OR REPLACE INTO settings(key,value) VALUES('_portable_commit',?1)",
                        [commit],
                    )?;
                    Ok(value)
                }
                Err(error) => {
                    journal::abort(&binding.root)?;
                    let _ = documents::hydrate(&mut cache, &binding.root, &before);
                    Err(error)
                }
            }
        }
        Err(error) => {
            journal::abort(&binding.root)?;
            let _ = documents::hydrate(&mut cache, &binding.root, &before);
            Err(error)
        }
    }
}
pub fn strict_offline(bootstrap: &Path) -> AppResult<bool> {
    execute(bootstrap, false, |db| {
        Ok(services::preferences::get(db, "strict_offline")?.as_deref() == Some("true"))
    })
}
pub fn capture_image(bootstrap: &Path, source: &Path) -> AppResult<PathBuf> {
    let Some(binding) = binding(bootstrap)? else {
        let directory = bootstrap.parent().unwrap().join("recovery/unbound");
        fs::create_dir_all(&directory)?;
        let target = directory.join("capture.png");
        io::atomic(&target, &fs::read(source)?)?;
        return Ok(target);
    };
    validate_binding(&binding)?;
    let _lock = io::lock(&binding.root)?;
    journal::recover(&binding.root)?;
    let mut docs = load_binding(&binding)?;
    let before = docs.clone();
    let id = documents::QUICK_DRAFT;
    let document = format!(".workspace/drafts/{id}/draft.json");
    let relative = format!(".workspace/drafts/{id}/Files/{}.png", Uuid::new_v4());
    let target = io::path(&binding.root, &relative)?;
    fs::create_dir_all(target.parent().unwrap())?;
    let mut staged = tempfile::NamedTempFile::new_in(target.parent().unwrap())?;
    std::io::Write::write_all(&mut staged, &fs::read(source)?)?;
    staged.as_file().sync_all()?;
    journal::begin(&binding.root, before)?;
    let _scope = journal::scope();
    if let Some(previous) = docs.get(&document) {
        retire_draft_images(&binding.root, previous)?;
    }
    journal::create(staged.path(), &target)?;
    staged
        .persist(&target)
        .map_err(|e| AppError::from(e.error))?;
    let mut draft=docs.get(&document).cloned().unwrap_or(json!({"formatVersion":1,"kind":"draft","workspaceId":binding.workspace_id,"id":id,"revision":0,"text":"","targetProjectId":null}));
    draft["revision"] = json!(draft["revision"].as_u64().unwrap_or(0) + 1);
    draft["updatedAt"] = json!(chrono::Utc::now().to_rfc3339());
    draft["attachments"] = json!([{"relativePath":format!("Files/{}",target.file_name().unwrap().to_string_lossy()),"mime":"image/png","sha256":io::hash(&target)?}]);
    docs.insert(document.clone(), draft);
    if !docs["config.json"]["drafts"]
        .as_array()
        .unwrap()
        .iter()
        .any(|v| v["id"] == id)
    {
        docs.get_mut("config.json").unwrap()["drafts"]
            .as_array_mut()
            .unwrap()
            .push(json!({"id":id,"document":document}));
    }
    finish_documents(&mut docs)?;
    journal::commit(docs)?;
    Ok(target)
}
pub fn saved_capture_image(bootstrap: &Path) -> AppResult<Option<PathBuf>> {
    let Some(binding) = binding(bootstrap)? else {
        let path = bootstrap
            .parent()
            .unwrap()
            .join("recovery/unbound/capture.png");
        return Ok(path.is_file().then_some(path));
    };
    validate_binding(&binding)?;
    let docs = load_binding(&binding)?;
    let name = format!(".workspace/drafts/{}/draft.json", documents::QUICK_DRAFT);
    let Some(draft) = docs.get(&name) else {
        return Ok(None);
    };
    let Some(file) = draft["attachments"].as_array().and_then(|v| v.first()) else {
        return Ok(None);
    };
    let relative = documents::text(file, "relativePath")?;
    let path = io::path(
        &binding.root,
        &format!(".workspace/drafts/{}/{relative}", documents::QUICK_DRAFT),
    )?;
    if io::hash(&path)? != documents::text(file, "sha256")? {
        return Err(io::invalid("草稿图片缺失或校验失败"));
    }
    Ok(Some(path))
}
pub fn discard_image(bootstrap: &Path) -> AppResult<()> {
    let Some(binding) = binding(bootstrap)? else {
        let file = bootstrap
            .parent()
            .unwrap()
            .join("recovery/unbound/capture.png");
        if file.exists() {
            fs::remove_file(file)?;
        }
        return Ok(());
    };
    validate_binding(&binding)?;
    let _lock = io::lock(&binding.root)?;
    journal::recover(&binding.root)?;
    let before = load_binding(&binding)?;
    let mut docs = before.clone();
    let name = format!(".workspace/drafts/{}/draft.json", documents::QUICK_DRAFT);
    if let Some(draft) = docs.get_mut(&name) {
        draft["attachments"] = json!([]);
        draft["revision"] = json!(draft["revision"].as_u64().unwrap_or(0) + 1);
        if draft["text"].as_str().unwrap_or("").is_empty() {
            docs.remove(&name);
            docs.get_mut("config.json").unwrap()["drafts"]
                .as_array_mut()
                .unwrap()
                .retain(|v| v["id"] != documents::QUICK_DRAFT);
        }
    }
    finish_documents(&mut docs)?;
    let retired = before.get(&name).cloned();
    journal::begin(&binding.root, before)?;
    let _scope = journal::scope();
    if let Some(draft) = retired {
        retire_draft_images(&binding.root, &draft)?;
    }
    journal::commit(docs)
}
pub fn finish_documents(docs: &mut Documents) -> AppResult<()> {
    let list=docs.iter().filter(|(name,_)|name.as_str()!="config.json").map(|(name,value)|Ok(json!({"path":name,"revision":value["revision"],"sha256":io::digest(&io::bytes(value)?)}))).collect::<AppResult<Vec<_>>>()?;
    let root = docs
        .get_mut("config.json")
        .ok_or_else(|| io::invalid("缺少根配置"))?;
    root["revision"] = json!(root["revision"].as_u64().unwrap_or(0) + 1);
    root["updatedAt"] = json!(chrono::Utc::now().to_rfc3339());
    root["snapshot"] = json!({"commitId":Uuid::new_v4().to_string(),"documents":list});
    Ok(())
}

/// Move an ordinary merged folder's contents under Files without replacing existing content.
pub fn prepare_merged_folder(target: &Path) -> AppResult<()> {
    let marker = target.join("config.json");
    if marker.is_file() && io::read(&marker).is_ok_and(|v| v["kind"] == "project") {
        return Err(AppError::new(
            "PROJECT_IMPORT_REQUIRED",
            "此文件夹已有项目配置，请使用“导入项目文件夹”，不要覆盖配置",
        ));
    }
    let root = target.parent().ok_or_else(|| io::invalid("项目目录无效"))?;
    let temporary = root.join(format!(".workspace/merge-{}", Uuid::new_v4()));
    journal::directory(&temporary)?;
    let entries = fs::read_dir(target)?
        .map(|e| e.map(|e| e.path()))
        .collect::<Result<Vec<_>, _>>()?;
    for source in entries {
        let name = source.file_name().unwrap();
        services::safe_fs::rename_exclusive(&source, &temporary.join(name))?;
    }
    journal::directory(&target.join("Files"))?;
    for entry in fs::read_dir(&temporary)? {
        let entry = entry?;
        services::safe_fs::rename_exclusive(
            &entry.path(),
            &target.join("Files").join(entry.file_name()),
        )?;
    }
    // Empty internal staging folders are harmless and removed only after commit by normal cleanup.
    Ok(())
}

pub fn icon_asset(root: &Path, directory: &str, data: &str) -> AppResult<serde_json::Value> {
    use base64::Engine;
    if !services::link_icons::valid_icon_data(data) {
        return Err(io::invalid("网站图标格式无效"));
    }
    let (header, payload) = data
        .split_once(',')
        .ok_or_else(|| io::invalid("图标数据无效"))?;
    let mime = header
        .strip_prefix("data:")
        .and_then(|s| s.strip_suffix(";base64"))
        .ok_or_else(|| io::invalid("图标数据格式无效"))?;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(payload)
        .map_err(|_| io::invalid("图标编码无效"))?;
    let digest = io::digest(&bytes);
    let ext = match mime {
        "image/png" => "png",
        "image/jpeg" => "jpg",
        "image/svg+xml" => "svg",
        "image/gif" => "gif",
        "image/webp" => "webp",
        _ => "ico",
    };
    let relative = format!(".assets/link-icons/{digest}.{ext}");
    let target = io::path(root, &format!("{directory}/{relative}"))?;
    if target.exists() {
        if io::hash(&target)? != digest {
            return Err(io::invalid("图标资源已被外部修改"));
        }
    } else {
        for path in [
            root.join(directory).join(".assets"),
            root.join(directory).join(".assets/link-icons"),
        ] {
            if !path.exists() {
                journal::directory(&path)?;
            }
        }
        let mut temp = tempfile::NamedTempFile::new_in(target.parent().unwrap())?;
        std::io::Write::write_all(&mut temp, &bytes)?;
        temp.as_file().sync_all()?;
        journal::create(temp.path(), &target)?;
        temp.persist_noclobber(&target)
            .map_err(|e| AppError::from(e.error))?;
    }
    Ok(json!({"path":relative,"mime":mime,"sha256":digest}))
}

/// Local notification delivery state is intentionally separate from disposable SQL indices.
pub fn notification_delivery(bootstrap: &Path, id: &str, state: Option<&str>) -> AppResult<bool> {
    let Some(binding) = binding(bootstrap)? else {
        return Ok(true);
    };
    Uuid::parse_str(id).map_err(|_| io::invalid("提醒 ID 无效"))?;
    let file = bootstrap
        .parent()
        .unwrap()
        .join("runtime")
        .join(&binding.id)
        .join(format!("{id}.json"));
    if let Some(state) = state {
        io::write(
            &file,
            &json!({"id":id,"state":state,"updatedAt":chrono::Utc::now().to_rfc3339()}),
        )?;
        return Ok(true);
    }
    Ok(!file.exists() || io::read(&file)?["state"] == "failed")
}

pub fn info(bootstrap: &Path) -> AppResult<serde_json::Value> {
    let current = match binding(bootstrap) {
        Ok(value) => value,
        Err(error) => {
            return Ok(
                json!({"mode":"portable","root":null,"configPath":null,"database":bootstrap,"state":"needsAttention","error":error.message,"bindingId":null,"recoveryCount":recovery_count(bootstrap)?}),
            )
        }
    };
    let Some(binding) = current else {
        let db = db::connect(bootstrap)?;
        let root = services::workspace::root(&db)?;
        let counts = json!({"projects":db.query_row("SELECT COUNT(*) FROM projects WHERE id!='__inbox__'",[],|r|r.get::<_,i64>(0))?,"tasks":db.query_row("SELECT COUNT(*) FROM tasks",[],|r|r.get::<_,i64>(0))?,"files":db.query_row("SELECT COUNT(*) FROM project_files",[],|r|r.get::<_,i64>(0))?});
        return Ok(
            json!({"mode":"legacy","root":root,"configPath":null,"database":bootstrap,"state":"legacy","counts":counts,"error":null,"bindingId":null,"recoveryCount":recovery_count(bootstrap)?}),
        );
    };
    let result = validate_binding(&binding).and_then(|()| load_binding(&binding));
    let (state, error, counts) = match result {
        Ok(docs) => {
            let length =
                |v: &serde_json::Value, key: &str| v[key].as_array().map(Vec::len).unwrap_or(0);
            let projects = length(&docs["config.json"], "projects");
            let tasks = docs
                .values()
                .filter(|v| v["kind"] == "todos")
                .map(|v| length(v, "tasks"))
                .sum::<usize>();
            let files = docs
                .values()
                .filter(|v| v["kind"] == "workspace" || v["kind"] == "project")
                .map(|v| length(v, "files"))
                .sum::<usize>();
            (
                "available",
                None,
                json!({"projects":projects,"tasks":tasks,"files":files}),
            )
        }
        Err(e) => (
            if e.code == "STORAGE_UNAVAILABLE" {
                "offline"
            } else {
                "needsAttention"
            },
            Some(e.message),
            serde_json::Value::Null,
        ),
    };
    Ok(
        json!({"mode":"portable","root":binding.root,"configPath":binding.root.join("config.json"),"database":cache_path(bootstrap,&binding),"state":state,"error":error,"counts":counts,"bindingId":binding.id,"recoveryCount":recovery_count(bootstrap)?}),
    )
}
fn recovery_count(bootstrap: &Path) -> AppResult<usize> {
    recovery::count(bootstrap)
}
pub fn clear_cache(bootstrap: &Path) -> AppResult<()> {
    let binding = binding(bootstrap)?.ok_or_else(|| io::invalid("请先升级为目录数据格式"))?;
    validate_binding(&binding)?;
    let _lock = io::lock(&binding.root)?;
    journal::recover(&binding.root)?;
    let docs = load_binding(&binding)?;
    let path = cache_path(bootstrap, &binding);
    for file in [
        path.clone(),
        path.with_extension("db-wal"),
        path.with_extension("db-shm"),
    ] {
        if file.exists() {
            fs::remove_file(file)?;
        }
    }
    let mut cache = connect_cache(bootstrap, &binding)?;
    documents::hydrate(&mut cache, &binding.root, &docs)?;
    cache.execute(
        "INSERT OR REPLACE INTO settings(key,value) VALUES('_portable_commit',?1)",
        [documents::text(
            &docs["config.json"]["snapshot"],
            "commitId",
        )?],
    )?;
    cache.execute(
        "INSERT OR REPLACE INTO settings(key,value) VALUES('_binding_id',?1)",
        [&binding.id],
    )?;
    Ok(())
}
pub fn edit_documents<T>(
    bootstrap: &Path,
    action: impl FnOnce(&Path, &mut Documents) -> AppResult<T>,
) -> AppResult<T> {
    let binding = binding(bootstrap)?.ok_or_else(|| io::invalid("请先升级为目录数据格式"))?;
    validate_binding(&binding)?;
    let _lock = io::lock(&binding.root)?;
    journal::recover(&binding.root)?;
    let before = load_binding(&binding)?;
    let mut after = before.clone();
    journal::begin(&binding.root, before.clone())?;
    let _scope = journal::scope();
    let result = (|| {
        let value = action(&binding.root, &mut after)?;
        finish_documents(&mut after)?;
        let mut cache = connect_cache(bootstrap, &binding)?;
        documents::hydrate(&mut cache, &binding.root, &after)?;
        validate_binding(&binding)?;
        journal::commit(after)?;
        Ok(value)
    })();
    if result.is_err() {
        journal::abort(&binding.root)?;
    }
    result
}

/// A caller keeps request_id across retries; the durable task ID is the receipt.
/// Draft removal and the new task are exported in the same workspace commit.
pub fn save_capture(
    db: &mut Connection,
    text: &str,
    project_id: Option<String>,
    image: Option<&Path>,
    request_id: &str,
) -> AppResult<crate::models::Task> {
    Uuid::parse_str(request_id).map_err(|_| io::invalid("采集请求标识无效"))?;
    if let Some(task) = services::tasks::list(db)?
        .into_iter()
        .find(|t| t.id == request_id)
    {
        return Ok(task);
    }
    let text = text.trim();
    if text.chars().count() > 10000 || (text.is_empty() && image.is_none()) {
        return Err(AppError::new(
            "VALIDATION_ERROR",
            "请输入内容或图片，内容最多 10000 字",
        ));
    }
    let title = if text.is_empty() {
        "图片待办".into()
    } else {
        text.lines()
            .next()
            .unwrap_or(text)
            .chars()
            .take(200)
            .collect()
    };
    let mut task = services::tasks::create(
        db,
        crate::models::CreateTask {
            title,
            project_id,
            priority: "none".into(),
            plan_date: None,
        },
    )?;
    db.execute(
        "UPDATE tasks SET id=?2,description=?3 WHERE id=?1",
        rusqlite::params![task.id, request_id, text],
    )?;
    task.id = request_id.into();
    task.description = text.into();
    let result = (|| {
        if let Some(image) = image {
            services::attachments::import(db, &task.id, image)?;
        }
        services::preferences::set(db, "capture_draft", "")?;
        services::preferences::set(db, "capture_project", "")?;
        db.execute(
            "INSERT OR REPLACE INTO settings(key,value) VALUES('_consume_capture','1')",
            [],
        )?;
        Ok(())
    })();
    if let Err(error) = result {
        let _ = db.execute("DELETE FROM task_files WHERE task_id=?1", [&task.id]);
        let _ = db.execute("DELETE FROM tasks WHERE id=?1", [&task.id]);
        return Err(error);
    }
    Ok(task)
}

pub fn require_binding(bootstrap: &Path, expected: Option<&str>) -> AppResult<()> {
    if let Some(expected) = expected {
        let current = binding(bootstrap)?;
        if current.as_ref().map(|b| b.id.as_str()).unwrap_or("unbound") != expected {
            return Err(AppError::new(
                "WORKSPACE_CHANGED",
                "工作目录已切换，原草稿未写入新目录",
            ));
        }
    }
    Ok(())
}

/// A copied/replaced root must be selected explicitly even when its path stayed the same.
pub fn load_binding(binding: &Binding) -> AppResult<Documents> {
    let docs = documents::load(&binding.root)?;
    if docs["config.json"]["workspaceId"] != binding.workspace_id {
        return Err(AppError::new(
            "STORAGE_UNAVAILABLE",
            "目录内容身份变化，请重新选择工作目录",
        ));
    }
    Ok(docs)
}

pub fn notification_confirmed(bootstrap: &Path, id: &str) -> AppResult<bool> {
    let Some(binding) = binding(bootstrap)? else {
        return Ok(false);
    };
    Uuid::parse_str(id).map_err(|_| io::invalid("提醒 ID 无效"))?;
    let file = bootstrap
        .parent()
        .unwrap()
        .join("runtime")
        .join(binding.id)
        .join(format!("{id}.json"));
    Ok(file.exists() && io::read(&file)?["state"] == "sent")
}

pub fn retire_draft_images(root: &Path, draft: &serde_json::Value) -> AppResult<()> {
    let id = documents::text(draft, "id")?;
    Uuid::parse_str(id).map_err(|_| io::invalid("草稿 ID 无效"))?;
    for resource in documents::items(draft, "attachments")? {
        let relative = documents::text(resource, "relativePath")?;
        if !relative.starts_with("Files/") {
            return Err(io::invalid("草稿资源位置无效"));
        }
        journal::retire(
            &io::path(root, &format!(".workspace/drafts/{id}/{relative}"))?,
            documents::text(resource, "sha256")?,
        )?;
    }
    Ok(())
}
