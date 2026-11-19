//! Project deletion removes workspace records without reading project files.
//! Directory data is removed only when the user explicitly opts in.
use super::{documents, io, journal};
use crate::{
    db,
    error::{AppError, AppResult},
    services,
};
use chrono::Utc;
use serde_json::{json, Value};
use std::path::Path;
use uuid::Uuid;

pub fn delete(bootstrap: &Path, id: &str) -> AppResult<()> {
    delete_with_files(bootstrap, id, false)
}

pub fn delete_with_files(bootstrap: &Path, id: &str, delete_files: bool) -> AppResult<()> {
    let Some(binding) = super::binding(bootstrap)? else {
        if delete_files {
            return Err(AppError::new(
                "ROOT_NOT_CONFIGURED",
                "当前项目未关联存储目录，无法删除本地文件",
            ));
        }
        return services::projects::delete(&mut db::connect(bootstrap)?, id);
    };
    let root = &binding.root;
    let _lock = io::lock(root)?;
    journal::recover(root)?;
    let original = io::read(&io::path(root, "config.json")?)?;
    documents::check(&original, "workspace", Some(&binding.workspace_id))?;
    let project = documents::items(&original, "projects")?
        .iter()
        .find(|project| project["id"] == id)
        .ok_or_else(|| AppError::new("PROJECT_NOT_FOUND", "项目不存在"))?;
    let prefix = format!("{}/", documents::text(project, "directory")?);
    let mut before = documents::Documents::from([("config.json".into(), original.clone())]);
    let mut after = before.clone();
    let mut config = original.clone();
    config["projects"]
        .as_array_mut()
        .unwrap()
        .retain(|project| project["id"] != id);
    let now = Utc::now().to_rfc3339();
    config["tombstones"]
        .as_array_mut()
        .ok_or_else(|| io::invalid("删除记录无效"))?
        .push(json!({"id":id,"kind":"project","deletedAt":now,"operationId":Uuid::new_v4().to_string()}));

    // The cache contains the records shown in the app. Missing project documents
    // never block unregistering it, including when the cache has been removed.
    let mut cache = super::connect_cache(bootstrap, &binding).ok();
    if let Some(cache) = &cache {
        let tasks = services::tasks::list(cache)?;
        let mut moved = Vec::new();
        for task in tasks
            .into_iter()
            .filter(|task| task.project_id.as_deref() == Some(id))
        {
            let mut reminders = documents::rows(cache,
                "SELECT id,remind_at AS remindAt,state,read_at AS readAt,sent_at AS sentAt FROM reminders WHERE task_id=?1 ORDER BY id",
                Some(&task.id))?;
            for reminder in &mut reminders {
                if reminder["state"] == "pending" {
                    reminder["state"] = json!("dismissed");
                }
            }
            let mut task = serde_json::to_value(task).map_err(|_| io::invalid("待办记录无效"))?;
            task.as_object_mut().unwrap().remove("projectId");
            if task["deletedAt"].is_null() {
                task["deletedAt"] = json!(now);
            }
            task["updatedAt"] = json!(now);
            task["attachmentIds"] = json!([]);
            task["reminders"] = json!(reminders);
            moved.push(task);
        }
        if !moved.is_empty() {
            let previous = io::read(&io::path(root, "todo.json")?)?;
            documents::check(&previous, "todos", Some(&binding.workspace_id))?;
            let mut todo = previous.clone();
            let tasks = todo["tasks"]
                .as_array_mut()
                .ok_or_else(|| io::invalid("待办列表无效"))?;
            // Root records win if an external edit already moved the same task.
            for task in moved {
                if !tasks.iter().any(|existing| existing["id"] == task["id"]) {
                    tasks.push(task);
                }
            }
            before.insert("todo.json".into(), previous.clone());
            after.insert(
                "todo.json".into(),
                documents::envelope(Some(&previous), todo),
            );
        }
    }

    // Keep the quick-add draft, clearing only its project selection.
    let draft_name = format!(".workspace/drafts/{}/draft.json", documents::QUICK_DRAFT);
    if documents::items(&config, "drafts")?
        .iter()
        .any(|draft| draft["document"] == draft_name)
    {
        let previous = io::read(&io::path(root, &draft_name)?)?;
        if previous["targetProjectId"] == id {
            let mut draft = previous.clone();
            draft["targetProjectId"] = Value::Null;
            before.insert(draft_name.clone(), previous.clone());
            after.insert(draft_name, documents::envelope(Some(&previous), draft));
        }
    }
    if let Some(extra) = config["preferences"]["extra"].as_object_mut() {
        for key in ["capture_project", "preference:capture_project"] {
            if extra.get(key).is_some_and(|value| value == id) {
                extra.remove(key);
            }
        }
    }
    if let Some(entries) = config["snapshot"]["documents"].as_array_mut() {
        entries.retain(|entry| {
            !entry["path"]
                .as_str()
                .is_some_and(|path| path.starts_with(&prefix))
        });
        for (name, document) in after
            .iter()
            .filter(|(name, _)| name.as_str() != "config.json")
        {
            entries.retain(|entry| entry["path"] != *name);
            entries.push(json!({"path":name,"revision":document["revision"],"sha256":io::digest(&io::bytes(document)?)}));
        }
    }
    config["snapshot"]["commitId"] = json!(Uuid::new_v4().to_string());
    after.insert(
        "config.json".into(),
        documents::envelope(Some(&original), config),
    );
    journal::begin(root, before)?;
    let _scope = journal::scope();
    // Project files only participate when the user explicitly requested deletion.
    if delete_files {
        let directory = prefix.trim_end_matches('/');
        if let Err(error) = journal::delete_project_directory(root, directory, id) {
            journal::abort(root)?;
            return Err(error);
        }
    }
    journal::commit(after)?;
    if let Some(cache) = &mut cache {
        // JSON is authoritative. A cache error must not turn a completed deletion
        // into a failure; the next successful workspace read rebuilds this cache.
        if services::projects::delete(cache, id).is_err() {
            let _ = cache.execute("DELETE FROM settings WHERE key='_portable_commit'", []);
        }
    }
    Ok(())
}
