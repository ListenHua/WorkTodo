use super::io;
use crate::{error::AppResult, services};
use chrono::Utc;
use rusqlite::{params, types::Value as SqlValue, Connection};
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, HashSet},
    path::Path,
};
use uuid::Uuid;

pub type Documents = BTreeMap<String, Value>;
pub const QUICK_DRAFT: &str = "00000000-0000-4000-8000-000000000001";
pub fn text<'a>(v: &'a Value, key: &str) -> AppResult<&'a str> {
    v.get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| io::invalid(&format!("配置缺少合法字段：{key}")))
}
pub fn items<'a>(v: &'a Value, key: &str) -> AppResult<&'a Vec<Value>> {
    v.get(key)
        .and_then(Value::as_array)
        .ok_or_else(|| io::invalid(&format!("配置列表无效：{key}")))
}
fn check(v: &Value, kind: &str, workspace: Option<&str>) -> AppResult<()> {
    if v["formatVersion"] != 1 {
        return Err(crate::error::AppError::new(
            "FORMAT_UNSUPPORTED",
            "目录数据格式不受支持，请使用兼容版本；未修改原数据",
        ));
    }
    if v["kind"] != kind || workspace.is_some_and(|id| v["workspaceId"] != id) {
        return Err(io::invalid("配置身份、类型或版本无效"));
    }
    Ok(())
}
pub fn load(root: &Path) -> AppResult<Documents> {
    let config = io::read(&io::path(root, "config.json")?)?;
    check(&config, "workspace", None)?;
    let id = text(&config, "workspaceId")?;
    Uuid::parse_str(id).map_err(|_| io::invalid("工作目录 ID 无效"))?;
    let mut expected = HashSet::from(["todo.json".to_owned()]);
    let mut ids = HashSet::new();
    for p in items(&config, "projects")? {
        let pid = text(p, "id")?;
        Uuid::parse_str(pid).map_err(|_| io::invalid("项目 ID 无效"))?;
        if !ids.insert(pid) {
            return Err(io::invalid("项目 ID 重复"));
        }
        let dir = text(p, "directory")?;
        if dir.contains('/') || services::projects::normalized_name(dir).is_err() {
            return Err(io::invalid("项目目录名称无效"));
        }
        expected.insert(format!("{dir}/config.json"));
        expected.insert(format!("{dir}/todo.json"));
    }
    let mut draft_ids = HashSet::new();
    for draft in items(&config, "drafts")? {
        if !draft_ids.insert(text(draft, "id")?) {
            return Err(io::invalid("草稿 ID 重复"));
        }
        Uuid::parse_str(text(draft, "id")?).map_err(|_| io::invalid("草稿 ID 无效"))?;
        let name = text(draft, "document")?;
        if name != format!(".workspace/drafts/{}/draft.json", text(draft, "id")?) {
            return Err(io::invalid("草稿位置无效"));
        }
        expected.insert(name.into());
    }
    let mut documents = Documents::new();
    for name in expected {
        let path = io::path(root, &name)?;
        let v = io::read(&path).map_err(|error| error.with_details(json!({"path": path})))?;
        let kind = if name.ends_with("/draft.json") {
            "draft"
        } else if name.ends_with("config.json") {
            "project"
        } else {
            "todos"
        };
        check(&v, kind, Some(id))?;
        documents.insert(name, v);
    }
    for p in items(&config, "projects")? {
        let dir = text(p, "directory")?;
        let meta = &documents[&format!("{dir}/config.json")];
        let todo = &documents[&format!("{dir}/todo.json")];
        if meta["id"] != p["id"]
            || meta["name"] != dir
            || todo["owner"]["id"] != p["id"]
            || todo["owner"]["type"] != "project"
        {
            return Err(io::invalid("项目身份或待办归属不一致"));
        }
    }
    for draft in items(&config, "drafts")? {
        let value = &documents[text(draft, "document")?];
        if value["id"] != draft["id"] || !value["text"].is_string() {
            return Err(io::invalid("草稿身份或内容无效"));
        }
        for attachment in items(value, "attachments")? {
            io::relative(text(attachment, "relativePath")?)?;
        }
    }
    if documents["todo.json"]["owner"] != json!({"type":"workspace","id":id}) {
        return Err(io::invalid("未分类待办归属不一致"));
    }
    documents.insert("config.json".into(), config);
    Ok(documents)
}
/// Cache freshness follows current JSON content, including edits made outside the app.
pub fn cache_key(documents: &Documents) -> AppResult<String> {
    let bytes = serde_json::to_vec(documents).map_err(|_| io::invalid("目录数据无法读取"))?;
    Ok(io::digest(&bytes))
}

fn rows(db: &Connection, sql: &str, arg: Option<&str>) -> AppResult<Vec<Value>> {
    let mut stmt = db.prepare(sql)?;
    let names = stmt
        .column_names()
        .iter()
        .map(|v| v.to_string())
        .collect::<Vec<_>>();
    let mut cursor = if let Some(arg) = arg {
        stmt.query([arg])?
    } else {
        stmt.query([])?
    };
    let mut out = vec![];
    while let Some(row) = cursor.next()? {
        let mut obj = serde_json::Map::new();
        for (i, name) in names.iter().enumerate() {
            let v: SqlValue = row.get(i)?;
            obj.insert(
                name.clone(),
                match v {
                    SqlValue::Null => Value::Null,
                    SqlValue::Integer(i) => json!(i),
                    SqlValue::Real(f) => json!(f),
                    SqlValue::Text(s) => json!(s),
                    _ => return Err(io::invalid("数据包含不支持的二进制字段")),
                },
            );
        }
        out.push(Value::Object(obj));
    }
    Ok(out)
}
fn insert(db: &Connection, table: &str, columns: &[(&str, &str)], value: &Value) -> AppResult<()> {
    let names = columns
        .iter()
        .map(|(sql, _)| *sql)
        .collect::<Vec<_>>()
        .join(",");
    let placeholders = (0..columns.len())
        .map(|_| "?")
        .collect::<Vec<_>>()
        .join(",");
    let values = columns
        .iter()
        .map(|(_, field)| match &value[*field] {
            Value::Null => Ok(SqlValue::Null),
            Value::Bool(b) => Ok(SqlValue::Integer(i64::from(*b))),
            Value::String(s) => Ok(SqlValue::Text(s.clone())),
            Value::Number(n) => n
                .as_i64()
                .map(SqlValue::Integer)
                .ok_or_else(|| io::invalid("数值超出范围")),
            _ => Err(io::invalid("业务字段类型无效")),
        })
        .collect::<AppResult<Vec<_>>>()?;
    db.execute(
        &format!("INSERT INTO {table}({names}) VALUES({placeholders})"),
        rusqlite::params_from_iter(values),
    )?;
    Ok(())
}
const FILE_COLS: &[(&str, &str)] = &[
    ("id", "id"),
    ("project_id", "projectId"),
    ("relative_path", "relativePath"),
    ("size", "size"),
    ("modified_at", "modifiedAt"),
    ("status", "status"),
];
const TASK_COLS: &[(&str, &str)] = &[
    ("id", "id"),
    ("project_id", "projectId"),
    ("title", "title"),
    ("description", "description"),
    ("status", "status"),
    ("priority", "priority"),
    ("plan_date", "planDate"),
    ("due_at", "dueAt"),
    ("created_at", "createdAt"),
    ("updated_at", "updatedAt"),
    ("completed_at", "completedAt"),
    ("deleted_at", "deletedAt"),
];
fn set(db: &Connection, key: &str, value: &str) -> AppResult<()> {
    db.execute(
        "INSERT OR REPLACE INTO settings(key,value) VALUES(?1,?2)",
        params![key, value],
    )?;
    Ok(())
}
pub fn hydrate(db: &mut Connection, root: &Path, docs: &Documents) -> AppResult<()> {
    let config = &docs["config.json"];
    let tx = db.transaction()?;
    tx.execute_batch("DELETE FROM task_files; DELETE FROM reminders; DELETE FROM tasks; DELETE FROM project_files; DELETE FROM projects; DELETE FROM settings; DELETE FROM filesystem_operations;")?;
    let prefs = &config["preferences"];
    set(&tx, "storage_root", &root.to_string_lossy())?;
    set(&tx, "_portable", "1")?;
    set(
        &tx,
        "preference:appearance",
        prefs["appearance"].as_str().unwrap_or("system"),
    )?;
    set(
        &tx,
        "preference:strict_offline",
        if prefs["network"]["strictOffline"] == true {
            "true"
        } else {
            "false"
        },
    )?;
    set(
        &tx,
        "autostart_desired",
        if prefs["desktop"]["autostart"] == true {
            "true"
        } else {
            "false"
        },
    )?;
    let mut settings = serde_json::to_value(crate::models::DesktopSettings::default()).unwrap();
    if prefs["desktop"].get("toggleShortcut").is_none() {
        settings.as_object_mut().unwrap().remove("toggleShortcut");
    }
    for key in ["closeToTray", "notifications", "toggleShortcut"] {
        if let Some(v) = prefs["desktop"].get(key) {
            settings[key] = v.clone();
        }
    }
    for key in [
        "shortcut",
        "middleCapture",
        "captureTrigger",
        "middleHoldMs",
    ] {
        if let Some(v) = prefs["capture"].get(key) {
            settings[key] = v.clone();
        }
    }
    let parsed = services::reminders::parse_settings(settings)?;
    services::reminders::validate_settings(&parsed)?;
    set(
        &tx,
        "desktop_settings",
        &serde_json::to_string(&parsed).unwrap(),
    )?;
    if let Some(extra) = prefs["extra"].as_object() {
        for (k, v) in extra {
            if !k.starts_with('_')
                && ![
                    "preference:appearance",
                    "preference:strict_offline",
                    "preference:changelog_seen",
                    "preference:capture_draft",
                    "preference:capture_project",
                ]
                .contains(&k.as_str())
                && !matches!(
                    k.as_str(),
                    "storage_root"
                        | "desktop_settings"
                        | "capture_draft"
                        | "appearance"
                        | "strict_offline"
                        | "changelog_seen"
                        | "autostart_desired"
                )
            {
                if let Some(value) = v.as_str() {
                    set(&tx, k, value)?;
                }
            }
        }
    }
    if let Some(value) = config["uiState"]["changelogSeen"].as_str() {
        set(&tx, "preference:changelog_seen", value)?;
    }
    let quick = format!(".workspace/drafts/{QUICK_DRAFT}/draft.json");
    set(
        &tx,
        "preference:capture_draft",
        docs.get(&quick)
            .and_then(|v| v["text"].as_str())
            .unwrap_or(""),
    )?;
    set(
        &tx,
        "preference:capture_project",
        docs.get(&quick)
            .and_then(|v| v["targetProjectId"].as_str())
            .unwrap_or(""),
    )?;
    tx.execute("INSERT INTO projects(id,name,normalized_name,directory_relpath,status,created_at) VALUES('__inbox__','_收件箱','_收件箱','Files','active',?1)",[Utc::now().to_rfc3339()])?;
    let mut buckets = vec![(
        "__inbox__".to_string(),
        config.clone(),
        "todo.json".to_string(),
    )];
    for entry in items(config, "projects")? {
        let directory = text(entry, "directory")?;
        let mut p = docs[&format!("{directory}/config.json")].clone();
        services::projects::validated_tags(
            &serde_json::from_value::<Vec<crate::models::ProjectTag>>(p["tags"].clone())
                .map_err(|_| io::invalid("标签数据无效"))?,
        )?;
        let color = text(&p, "color")?;
        if color.len() != 7
            || !color.starts_with('#')
            || !color[1..].bytes().all(|c| c.is_ascii_hexdigit())
        {
            return Err(io::invalid("项目颜色无效"));
        }
        let normalized = services::projects::normalized_name(text(&p, "name")?)?;
        p["normalizedName"] = json!(normalized);
        p["directory"] = json!(directory);
        let mut links = items(&p, "links")?.clone();
        if links.len() > 50 {
            return Err(io::invalid("链接数量超限"));
        }
        for link in &mut links {
            if text(link, "id")?.is_empty()
                || text(link, "name")?.trim().is_empty()
                || text(link, "name")?.chars().count() > 80
                || !["", "github", "gitlab", "gitee", "website"].contains(&text(link, "icon")?)
            {
                return Err(io::invalid("项目链接无效"));
            }
            services::projects::validated_url(text(link, "url")?)?;
            if let Some(asset) = link.get("iconAsset") {
                let asset_path = io::path(root, &format!("{directory}/{}", text(asset, "path")?))?;
                if asset_path.exists() {
                    if std::fs::metadata(&asset_path)?.len() > 256 * 1024 {
                        return Err(io::invalid("链接图标资源过大"));
                    }
                    if io::hash(&asset_path)? != text(asset, "sha256")? {
                        return Err(io::invalid("图标资源校验失败"));
                    }
                    use base64::Engine;
                    link["iconData"] = json!(format!(
                        "data:{};base64,{}",
                        text(asset, "mime")?,
                        base64::engine::general_purpose::STANDARD
                            .encode(std::fs::read(asset_path)?)
                    ));
                }
            }
        }
        p["linksJson"] = json!(serde_json::to_string(&links).unwrap());
        p["tagsJson"] = json!(p["tags"].to_string());
        insert(
            &tx,
            "projects",
            &[
                ("id", "id"),
                ("name", "name"),
                ("normalized_name", "normalizedName"),
                ("directory_relpath", "directory"),
                ("description", "description"),
                ("color", "color"),
                ("status", "status"),
                ("created_at", "createdAt"),
                ("pinned", "pinned"),
                ("links_json", "linksJson"),
                ("tags_json", "tagsJson"),
            ],
            &p,
        )?;
        buckets.push((
            text(entry, "id")?.into(),
            p,
            format!("{directory}/todo.json"),
        ));
    }
    for (owner, meta, todo_name) in &buckets {
        for file in items(meta, "files")? {
            let mut file = file.clone();
            Uuid::parse_str(text(&file, "id")?).map_err(|_| io::invalid("文件 ID 无效"))?;
            io::relative(text(&file, "relativePath")?)?;
            file["projectId"] = json!(owner);
            file["size"] = file.get("size").cloned().unwrap_or(json!(0));
            file["modifiedAt"] = file.get("modifiedAt").cloned().unwrap_or(json!(""));
            let folder = if owner == "__inbox__" {
                "Files".into()
            } else {
                format!("{}/Files", text(meta, "directory")?)
            };
            let path = io::path(root, &format!("{folder}/{}", text(&file, "relativePath")?))?;
            file["status"] = json!(if path.is_file() {
                "available"
            } else {
                "missing"
            });
            insert(&tx, "project_files", FILE_COLS, &file)?;
        }
        for task in items(&docs[todo_name], "tasks")? {
            let mut task = task.clone();
            Uuid::parse_str(text(&task, "id")?).map_err(|_| io::invalid("待办 ID 无效"))?;
            let mut validation = task.clone();
            validation["projectId"] = Value::Null;
            validation["reminderAt"] = Value::Null;
            let input: crate::models::UpdateTask =
                serde_json::from_value(validation).map_err(|_| io::invalid("待办字段无效"))?;
            services::tasks::validate_edit(&input)?;
            task["priority"] = json!(services::tasks::normalize_priority(&input.priority));
            task["projectId"] = if owner == "__inbox__" {
                Value::Null
            } else {
                json!(owner)
            };
            insert(&tx, "tasks", TASK_COLS, &task)?;
        }
    }
    for (owner, _, todo_name) in &buckets {
        for task in items(&docs[todo_name], "tasks")? {
            let tid = text(task, "id")?;
            for fid in items(task, "attachmentIds")? {
                let fid = fid.as_str().ok_or_else(|| io::invalid("附件 ID 无效"))?;
                let same: bool = tx.query_row(
                    "SELECT EXISTS(SELECT 1 FROM project_files WHERE id=?1 AND project_id=?2)",
                    params![fid, owner],
                    |r| r.get(0),
                )?;
                if !same {
                    return Err(io::invalid("附件引用缺失或跨项目"));
                }
                tx.execute(
                    "INSERT INTO task_files(task_id,file_id) VALUES(?1,?2)",
                    params![tid, fid],
                )?;
            }
            for reminder in items(task, "reminders")? {
                let mut r = reminder.clone();
                Uuid::parse_str(text(&r, "id")?).map_err(|_| io::invalid("提醒 ID 无效"))?;
                chrono::DateTime::parse_from_rfc3339(text(&r, "remindAt")?)
                    .map_err(|_| io::invalid("提醒时间无效"))?;
                r["taskId"] = json!(tid);
                insert(
                    &tx,
                    "reminders",
                    &[
                        ("id", "id"),
                        ("task_id", "taskId"),
                        ("remind_at", "remindAt"),
                        ("state", "state"),
                        ("read_at", "readAt"),
                        ("sent_at", "sentAt"),
                    ],
                    &r,
                )?;
            }
        }
    }
    for operation in items(config, "fileOperations")? {
        let mut op = operation.clone();
        op["target"] = expand_value(&op["target"], root)?;
        if !op["detail"].is_null() {
            op["detail"] = json!(expand_value(&op["detail"], root)?.to_string());
        }
        insert(
            &tx,
            "filesystem_operations",
            &[
                ("id", "id"),
                ("kind", "kind"),
                ("target", "target"),
                ("state", "state"),
                ("created_at", "createdAt"),
                ("detail", "detail"),
            ],
            &op,
        )?;
    }
    tx.commit()?;
    Ok(())
}
pub fn expand_value(value: &Value, root: &Path) -> AppResult<Value> {
    match value {
        Value::Object(map) if map.len() == 1 && map.contains_key("workspacePath") => {
            let relative = map["workspacePath"]
                .as_str()
                .ok_or_else(|| io::invalid("操作记录路径无效"))?;
            Ok(json!(if relative.is_empty() {
                root.to_path_buf()
            } else {
                io::path(root, relative)?
            }
            .to_string_lossy()))
        }
        Value::Object(map) => Ok(Value::Object(
            map.iter()
                .map(|(k, v)| Ok((k.clone(), expand_value(v, root)?)))
                .collect::<AppResult<_>>()?,
        )),
        Value::Array(list) => Ok(Value::Array(
            list.iter()
                .map(|v| expand_value(v, root))
                .collect::<AppResult<_>>()?,
        )),
        _ => Ok(value.clone()),
    }
}
pub(super) fn portable_value(value: &Value, roots: &[String]) -> Value {
    match value {
        Value::String(text) => {
            for root in roots {
                if let Some(rest) = text.strip_prefix(root) {
                    if rest.is_empty() || rest.starts_with(['/', '\\']) {
                        return json!({"workspacePath":rest.trim_start_matches(['/', '\\']).replace('\\',"/")});
                    }
                }
            }
            value.clone()
        }
        Value::Object(map) => Value::Object(
            map.iter()
                .map(|(k, v)| (k.clone(), portable_value(v, roots)))
                .collect(),
        ),
        Value::Array(list) => Value::Array(list.iter().map(|v| portable_value(v, roots)).collect()),
        _ => value.clone(),
    }
}

fn retain(old: Option<&Value>, new: Value) -> Value {
    let mut out = old.cloned().filter(Value::is_object).unwrap_or(json!({}));
    for (k, v) in new.as_object().unwrap() {
        out[k] = v.clone();
    }
    out
}
fn previous_item<'a>(old: Option<&'a Value>, field: &str, id: &Value) -> Option<&'a Value> {
    old?.get(field)?.as_array()?.iter().find(|v| v["id"] == *id)
}
fn envelope(old: Option<&Value>, mut value: Value) -> Value {
    if let Some(old) = old {
        value["revision"] = old["revision"].clone();
        value["updatedAt"] = old["updatedAt"].clone();
    }
    let mut result = retain(old, value);
    if old != Some(&result) {
        result["revision"] = json!(old.and_then(|v| v["revision"].as_u64()).unwrap_or(0) + 1);
        result["updatedAt"] = json!(Utc::now().to_rfc3339());
    }
    result
}
fn file_rows(
    db: &Connection,
    root: &Path,
    owner: &str,
    directory: &str,
    old: Option<&Value>,
) -> AppResult<Vec<Value>> {
    let mut files=rows(db,"SELECT id,relative_path AS relativePath,size,modified_at AS modifiedAt FROM project_files WHERE project_id=?1 ORDER BY id",Some(owner))?;
    let accepted: Vec<String> = db
        .query_row(
            "SELECT value FROM settings WHERE key='_accept_file_versions'",
            [],
            |r| r.get::<_, String>(0),
        )
        .ok()
        .and_then(|v| serde_json::from_str(&v).ok())
        .unwrap_or_default();
    for f in &mut files {
        let prior = previous_item(old, "files", &f["id"]);
        let path = io::path(root, &format!("{directory}/{}", text(f, "relativePath")?))?;
        let keep = prior.is_some_and(|v| v["sha256"].is_string())
            && !accepted.iter().any(|id| f["id"] == *id);
        let digest = if keep {
            let prior = prior.unwrap();
            f["size"] = prior["size"].clone();
            f["modifiedAt"] = prior["modifiedAt"].clone();
            prior["sha256"].clone()
        } else if path.is_file() {
            let metadata = std::fs::metadata(&path)?;
            let modified: chrono::DateTime<Utc> = metadata.modified()?.into();
            f["size"] = json!(metadata.len());
            f["modifiedAt"] = json!(modified.to_rfc3339());
            json!(io::hash(&path)?)
        } else {
            prior.map(|v| v["sha256"].clone()).unwrap_or(Value::Null)
        };
        f["sha256"] = digest;
        *f = retain(prior, f.clone());
    }
    Ok(files)
}
fn file_tombstones(previous: Option<&Value>, current: &Value) -> AppResult<Value> {
    let mut marks = previous
        .map(|v| v["tombstones"].clone())
        .unwrap_or(json!([]));
    let list = marks
        .as_array_mut()
        .ok_or_else(|| io::invalid("删除记录无效"))?;
    if let Some(previous) = previous {
        for file in items(previous, "files")? {
            if !current
                .as_array()
                .unwrap()
                .iter()
                .any(|v| v["id"] == file["id"])
            {
                list.push(json!({"id":file["id"],"kind":"file","deletedAt":Utc::now().to_rfc3339(),"operationId":Uuid::new_v4().to_string()}));
            }
        }
    }
    Ok(marks)
}
pub fn export(db: &Connection, root: &Path, old: &Documents) -> AppResult<Documents> {
    let previous = old.get("config.json");
    let wid = previous
        .and_then(|v| v["workspaceId"].as_str())
        .map(str::to_owned)
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    let mut config = retain(
        previous,
        json!({"formatVersion":1,"kind":"workspace","workspaceId":wid,"projects":[],"drafts":previous.map(|v|v["drafts"].clone()).unwrap_or(json!([])),"files":[],"fileOperations":[],"tombstones":previous.map(|v|v["tombstones"].clone()).unwrap_or(json!([]))}),
    );
    let settings = services::reminders::settings(db)?;
    let values = rows(db, "SELECT key,value FROM settings ORDER BY key", None)?;
    let get = |key: &str| {
        let stored = if matches!(
            key,
            "appearance"
                | "strict_offline"
                | "changelog_seen"
                | "capture_draft"
                | "capture_project"
        ) {
            format!("preference:{key}")
        } else {
            key.to_owned()
        };
        values
            .iter()
            .find(|v| v["key"] == stored)
            .and_then(|v| v["value"].as_str())
    };
    let extra = values
        .iter()
        .filter(|v| {
            v["key"].as_str().is_some_and(|k| {
                !k.starts_with('_')
                    && ![
                        "preference:appearance",
                        "preference:strict_offline",
                        "preference:changelog_seen",
                        "preference:capture_draft",
                        "preference:capture_project",
                    ]
                    .contains(&k)
                    && !matches!(
                        k,
                        "storage_root"
                            | "desktop_settings"
                            | "appearance"
                            | "capture_draft"
                            | "changelog_seen"
                            | "strict_offline"
                            | "autostart_desired"
                            | "backup_format"
                    )
            })
        })
        .map(|v| (v["key"].as_str().unwrap().to_owned(), v["value"].clone()))
        .collect::<serde_json::Map<_, _>>();
    let mut preferences = json!({"appearance":get("appearance").unwrap_or("system"),"desktop":{"closeToTray":settings.close_to_tray,"notifications":settings.notifications,"toggleShortcut":settings.toggle_shortcut,"autostart":get("autostart_desired")==Some("true")},"capture":{"shortcut":settings.shortcut,"middleCapture":settings.middle_capture,"captureTrigger":settings.capture_trigger,"middleHoldMs":settings.middle_hold_ms},"network":{"strictOffline":get("strict_offline")==Some("true")},"extra":extra});
    if let Some(prior) = previous {
        for section in ["desktop", "capture", "network", "extra"] {
            preferences[section] = retain(
                prior["preferences"].get(section),
                preferences[section].clone(),
            );
        }
        preferences = retain(prior.get("preferences"), preferences);
    }
    config["preferences"] = preferences;
    config["uiState"] = json!({"changelogSeen":get("changelog_seen")});
    config["files"] = json!(file_rows(db, root, "__inbox__", "Files", previous)?);
    config["tombstones"] = file_tombstones(previous, &config["files"])?;
    let mut roots = vec![root.to_string_lossy().into_owned()];
    if let Some(legacy) = get("_legacy_root") {
        roots.push(legacy.to_owned());
    }
    let operations=rows(db,"SELECT id,kind,target,state,created_at AS createdAt,detail FROM filesystem_operations ORDER BY id",None)?.into_iter().map(|mut v|{
        v["target"]=portable_value(&v["target"],&roots);
        if let Some(detail)=v["detail"].as_str(){let parsed:Value=serde_json::from_str(detail).unwrap_or(json!(detail));v["detail"]=portable_value(&parsed,&roots);}v
    }).collect::<Vec<_>>();
    config["fileOperations"] = json!(operations);
    let mut documents = Documents::new();
    let projects=rows(db,"SELECT id,name,directory_relpath AS directory,description,color,status,created_at AS createdAt,pinned,links_json AS links,tags_json AS tags FROM projects WHERE id!='__inbox__' ORDER BY id",None)?;
    // Keep a deletion record in the workspace document after unregistering a project.
    if let Some(previous) = previous {
        for project in items(previous, "projects")? {
            if !projects
                .iter()
                .any(|current| current["id"] == project["id"])
            {
                config["tombstones"].as_array_mut().ok_or_else(|| io::invalid("删除记录无效"))?.push(
                    json!({"id":project["id"],"kind":"project","deletedAt":Utc::now().to_rfc3339(),"operationId":Uuid::new_v4().to_string()}),
                );
            }
        }
    }
    let mut buckets = vec![("__inbox__".to_owned(), "".to_owned())];
    for mut project in projects {
        let id = text(&project, "id")?.to_owned();
        let directory = text(&project, "directory")?.to_owned();
        config["projects"]
            .as_array_mut()
            .unwrap()
            .push(json!({"id":id,"directory":directory}));
        let name = format!("{directory}/config.json");
        let previous = old.get(&name).or_else(|| {
            old.values()
                .find(|v| v["kind"] == "project" && v["id"] == id)
        });
        project["formatVersion"] = json!(1);
        project["kind"] = json!("project");
        project["workspaceId"] = json!(wid);
        project["links"] = serde_json::from_str(text(&project, "links")?)
            .map_err(|_| io::invalid("项目链接损坏"))?;
        for link in project["links"]
            .as_array_mut()
            .ok_or_else(|| io::invalid("链接列表无效"))?
        {
            if let Some(data) = link["iconData"].as_str().map(str::to_owned) {
                link["iconAsset"] = super::icon_asset(root, &directory, &data)?;
                link.as_object_mut().unwrap().remove("iconData");
            }
        }
        for link in project["links"]
            .as_array_mut()
            .ok_or_else(|| io::invalid("链接列表无效"))?
        {
            let id = link["id"].clone();
            *link = retain(previous_item(previous, "links", &id), link.clone());
            if link["icon"] != "website" {
                link.as_object_mut().unwrap().remove("iconAsset");
            }
            link.as_object_mut().unwrap().remove("iconData");
        }
        project["tags"] = serde_json::from_str(text(&project, "tags")?)
            .map_err(|_| io::invalid("项目标签损坏"))?;
        project["pinned"] = json!(project["pinned"] == 1);
        project["files"] = json!(file_rows(
            db,
            root,
            &id,
            &format!("{directory}/Files"),
            previous
        )?);
        project["tombstones"] = file_tombstones(previous, &project["files"])?;
        let project = envelope(previous, project);
        documents.insert(name, project);
        buckets.push((id, directory));
    }
    let all=rows(db,"SELECT id,project_id AS projectId,title,description,status,CASE WHEN priority='none' THEN 'low' ELSE priority END AS priority,plan_date AS planDate,due_at AS dueAt,created_at AS createdAt,updated_at AS updatedAt,completed_at AS completedAt,deleted_at AS deletedAt FROM tasks ORDER BY id",None)?;
    for (owner, directory) in buckets {
        let name = if directory.is_empty() {
            "todo.json".into()
        } else {
            format!("{directory}/todo.json")
        };
        let prior = old.get(&name).or_else(|| {
            old.values()
                .find(|v| v["kind"] == "todos" && v["owner"]["id"] == owner)
        });
        let mut tasks = vec![];
        for task in all.iter().filter(|v| {
            if owner == "__inbox__" {
                v["projectId"].is_null()
            } else {
                v["projectId"] == owner
            }
        }) {
            let mut task = task.clone();
            let tid = text(&task, "id")?;
            let attachments = rows(
                db,
                "SELECT file_id AS id FROM task_files WHERE task_id=?1 ORDER BY file_id",
                Some(tid),
            )?;
            let reminders=rows(db,"SELECT id,remind_at AS remindAt,state,read_at AS readAt,sent_at AS sentAt FROM reminders WHERE task_id=?1 ORDER BY id",Some(tid))?;
            task["attachmentIds"] = json!(attachments
                .iter()
                .map(|v| v["id"].clone())
                .collect::<Vec<_>>());
            task["reminders"] = json!(reminders
                .into_iter()
                .map(|r| {
                    let old_task = previous_item(prior, "tasks", &task["id"]);
                    retain(previous_item(old_task, "reminders", &r["id"]), r)
                })
                .collect::<Vec<_>>());
            let tid = task["id"].clone();
            task.as_object_mut().unwrap().remove("projectId");
            tasks.push(retain(previous_item(prior, "tasks", &tid), task));
        }
        let mut tombstones = prior.map(|v| v["tombstones"].clone()).unwrap_or(json!([]));
        if let Some(prior) = prior {
            for task in items(prior, "tasks")? {
                if !tasks.iter().any(|v| v["id"] == task["id"]) {
                    let target = all.iter().find(|v| v["id"] == task["id"]);
                    tombstones.as_array_mut().ok_or_else(||io::invalid("删除记录无效"))?.push(json!({"id":task["id"],"kind":if target.is_some(){"moved"}else{"deleted"},"targetOwner":target.map(|v|v["projectId"].clone()),"operationId":Uuid::new_v4().to_string(),"deletedAt":Utc::now().to_rfc3339()}));
                }
            }
        }
        documents.insert(name,envelope(prior,json!({"formatVersion":1,"kind":"todos","workspaceId":wid,"owner":{"type":if owner=="__inbox__"{"workspace"}else{"project"},"id":if owner=="__inbox__"{&wid}else{&owner}},"tasks":tasks,"tombstones":tombstones})));
    }
    for (name, doc) in old.iter().filter(|(name, _)| name.ends_with("/draft.json")) {
        documents.insert(name.clone(), doc.clone());
    }
    let draft_name = format!(".workspace/drafts/{QUICK_DRAFT}/draft.json");
    let text = get("capture_draft").unwrap_or("");
    let consumed: bool = db.query_row(
        "SELECT EXISTS(SELECT 1 FROM settings WHERE key='_consume_capture')",
        [],
        |r| r.get(0),
    )?;
    if !consumed
        && (!text.is_empty()
            || documents
                .get(&draft_name)
                .is_some_and(|v| v["attachments"].as_array().is_some_and(|v| !v.is_empty())))
    {
        let prior = old.get(&draft_name);
        let draft = envelope(
            prior,
            json!({"formatVersion":1,"kind":"draft","workspaceId":wid,"id":QUICK_DRAFT,"text":text,"targetProjectId":get("capture_project").filter(|v|!v.is_empty()).map(|v|json!(v)).unwrap_or(Value::Null),"attachments":prior.map(|v|v["attachments"].clone()).unwrap_or(json!([]))}),
        );
        documents.insert(draft_name.clone(), draft);
        if !config["drafts"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v["id"] == QUICK_DRAFT)
        {
            config["drafts"]
                .as_array_mut()
                .unwrap()
                .push(json!({"id":QUICK_DRAFT,"document":draft_name}));
        }
    } else {
        if let Some(draft) = old.get(&draft_name) {
            super::retire_draft_images(root, draft)?;
        }
        documents.remove(&draft_name);
        config["drafts"]
            .as_array_mut()
            .unwrap()
            .retain(|v| v["id"] != QUICK_DRAFT);
    }
    config["snapshot"] = json!({"commitId":previous.map(|v|v["snapshot"]["commitId"].clone()).unwrap_or(json!("")),"documents":documents.iter().map(|(name,v)|Ok(json!({"path":name,"revision":v["revision"],"sha256":io::digest(&io::bytes(v)?)}))).collect::<AppResult<Vec<_>>>()?});
    let mut config = envelope(previous, config);
    if previous != Some(&config) {
        config["snapshot"]["commitId"] = json!(Uuid::new_v4().to_string());
    }
    documents.insert("config.json".into(), config);
    Ok(documents)
}
