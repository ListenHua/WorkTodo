//! Selecting a folder opens its own data; it never copies the previous workspace.
use super::io;
use crate::{
    db,
    error::{AppError, AppResult},
    models::WorkspaceStatus,
};
use std::{fs, path::Path};

fn conflict(path: &Path) -> AppError {
    AppError::new(
        "STORAGE_FILE_CONFLICT",
        &format!(
            "{} 已存在且不是可用的 WorkTodo 数据，请更换目录或处理该文件；原文件未修改",
            path.display()
        ),
    )
}

pub fn select(bootstrap: &Path, target: &Path) -> AppResult<WorkspaceStatus> {
    let target = target.canonicalize()?;
    if !target.is_dir() {
        return Err(io::invalid("请选择文件夹作为存储目录"));
    }
    fs::read_dir(&target)?;
    let config = io::path(&target, "config.json")?;
    if config.try_exists()? {
        let value = io::read(&config)
            .map_err(|error| error.with_details(serde_json::json!({"path": config})))?;
        if value["kind"] != "workspace" {
            return Err(conflict(&config));
        }
        return super::open(bootstrap, &target);
    }
    // Only initialize absent metadata. Ordinary files and folders stay in place.
    for name in ["todo.json", ".workspace/transactions/current.json"] {
        let path = io::path(&target, name)?;
        if path.try_exists()? {
            return Err(conflict(&path));
        }
    }
    for name in ["Files", "_回收站", ".workspace"] {
        let path = io::path(&target, name)?;
        if path.try_exists()? && !path.is_dir() {
            return Err(conflict(&path));
        }
    }
    let _lock = io::lock(&target)?;
    // Another app may have initialized the folder while this process waited for the lock.
    if config.try_exists()? {
        return super::open_locked(bootstrap, &target);
    }
    for name in ["todo.json", ".workspace/transactions/current.json"] {
        let path = io::path(&target, name)?;
        if path.try_exists()? {
            return Err(conflict(&path));
        }
    }
    // A new folder starts empty, even if the previous folder or legacy database has data.
    let temporary = tempfile::tempdir()?;
    let database = temporary.path().join("empty.db");
    db::initialize(&database)?;
    super::initialize_locked(&target, &db::connect(&database)?)?;
    super::open_locked(bootstrap, &target)
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub name: String,
    pub directory: bool,
}

/// Show ordinary root entries in place; metadata and registered projects have their own views.
pub fn entries(db: &rusqlite::Connection) -> AppResult<Vec<Entry>> {
    let Some(root) = crate::services::workspace::root(db)? else {
        return Ok(vec![]);
    };
    let mut statement =
        db.prepare("SELECT directory_relpath FROM projects WHERE id!='__inbox__'")?;
    let projects = statement
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<Result<std::collections::HashSet<_>, _>>()?;
    let mut out = Vec::new();
    for entry in fs::read_dir(&root)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().to_string();
        if matches!(
            name.as_str(),
            "config.json"
                | "todo.json"
                | ".workspace"
                | "_回收站"
                | ".DS_Store"
                | "Thumbs.db"
                | "desktop.ini"
        ) || projects.contains(&name)
        {
            continue;
        }
        let meta = fs::symlink_metadata(entry.path())?;
        if crate::services::files::is_link(&meta) || (!meta.is_file() && !meta.is_dir()) {
            continue;
        }
        if name == "Files" && meta.is_dir() && fs::read_dir(entry.path())?.next().is_none() {
            continue;
        }
        out.push(Entry {
            name,
            directory: meta.is_dir(),
        });
    }
    out.sort_by(|a, b| {
        b.directory
            .cmp(&a.directory)
            .then_with(|| a.name.cmp(&b.name))
    });
    Ok(out)
}

pub fn entry_path(db: &rusqlite::Connection, name: &str) -> AppResult<std::path::PathBuf> {
    if !entries(db)?.iter().any(|entry| entry.name == name) {
        return Err(io::invalid("文件已不存在，请刷新目录"));
    }
    io::path(&crate::services::workspace::available_root(db)?, name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        models::{CreateProject, CreateTask},
        portable::{self, documents, transfer},
        services,
    };
    use std::path::PathBuf;

    struct Fixture {
        temp: tempfile::TempDir,
        bootstrap: PathBuf,
    }
    impl Fixture {
        fn new() -> Self {
            let temp = tempfile::tempdir().unwrap();
            let bootstrap = temp.path().join("device/app.db");
            fs::create_dir(bootstrap.parent().unwrap()).unwrap();
            db::initialize(&bootstrap).unwrap();
            Self { temp, bootstrap }
        }
        fn folder(&self, name: &str) -> PathBuf {
            let path = self.temp.path().join(name);
            fs::create_dir(&path).unwrap();
            path
        }
        fn select(&self, root: &Path) -> WorkspaceStatus {
            select(&self.bootstrap, root).unwrap()
        }
        fn task(&self, title: &str, project_id: Option<String>) -> String {
            portable::execute(&self.bootstrap, true, |db| {
                Ok(services::tasks::create(
                    db,
                    CreateTask {
                        title: title.into(),
                        project_id,
                        priority: "low".into(),
                        plan_date: None,
                    },
                )?
                .id)
            })
            .unwrap()
        }
        fn titles(&self) -> Vec<String> {
            portable::execute(&self.bootstrap, false, |db| {
                Ok(services::tasks::list(db)?
                    .into_iter()
                    .map(|task| task.title)
                    .collect())
            })
            .unwrap()
        }
    }

    #[test]
    fn switching_folders_keeps_projects_tasks_attachments_drafts_and_source_independent() {
        let f = Fixture::new();
        let a = f.folder("A");
        let b = f.folder("B");
        let first = f.select(&a);
        let project = portable::execute(&f.bootstrap, true, |db| {
            services::projects::create(
                db,
                CreateProject {
                    name: "项目A".into(),
                    color: "#123456".into(),
                    ..Default::default()
                },
            )
        })
        .unwrap();
        let task = f.task("A的待办", Some(project.id.clone()));
        let attachment = f.temp.path().join("附件.png");
        fs::write(&attachment, b"image").unwrap();
        portable::execute(&f.bootstrap, true, |db| {
            services::attachments::import(db, &task, &attachment)?;
            services::preferences::set(db, "capture_draft", "A的草稿")?;
            services::preferences::set(db, "appearance", "dark")
        })
        .unwrap();
        let a_before = transfer::inventory(&a).unwrap();
        let second = f.select(&b);
        assert_ne!(first.binding_id, second.binding_id);
        assert!(f.titles().is_empty());
        portable::execute(&f.bootstrap, false, |db| {
            assert!(services::projects::list(db)?.is_empty());
            assert_eq!(
                services::preferences::get(db, "capture_draft")?.as_deref(),
                Some("")
            );
            assert_eq!(
                services::preferences::get(db, "appearance")?.as_deref(),
                Some("system")
            );
            Ok(())
        })
        .unwrap();
        assert_eq!(transfer::inventory(&a).unwrap(), a_before);
        assert!(!b.join("项目A").exists());
        f.task("B的待办", None);
        let b_before = transfer::inventory(&b).unwrap();
        f.select(&a);
        assert_eq!(f.titles(), vec!["A的待办"]);
        portable::execute(&f.bootstrap, false, |db| {
            assert_eq!(services::attachments::list(db, &task)?.len(), 1);
            assert_eq!(
                services::preferences::get(db, "capture_draft")?.as_deref(),
                Some("A的草稿")
            );
            Ok(())
        })
        .unwrap();
        assert_eq!(transfer::inventory(&b).unwrap(), b_before);
        f.select(&b);
        assert_eq!(f.titles(), vec!["B的待办"]);
    }

    #[test]
    fn ordinary_folder_is_initialized_without_moving_or_overwriting_any_content() {
        let f = Fixture::new();
        let root = f.folder("普通目录");
        fs::write(root.join("说明.txt"), b"original").unwrap();
        fs::create_dir(root.join("普通子目录")).unwrap();
        fs::write(root.join("普通子目录/文件.txt"), b"nested").unwrap();
        fs::create_dir(root.join("Files")).unwrap();
        fs::write(root.join("Files/已有文件.txt"), b"files").unwrap();
        f.select(&root);
        assert_eq!(fs::read(root.join("说明.txt")).unwrap(), b"original");
        assert_eq!(
            fs::read(root.join("普通子目录/文件.txt")).unwrap(),
            b"nested"
        );
        assert_eq!(fs::read(root.join("Files/已有文件.txt")).unwrap(), b"files");
        assert!(documents::load(&root).unwrap()["config.json"]["projects"]
            .as_array()
            .unwrap()
            .is_empty());
        assert!(f.titles().is_empty());
    }

    #[test]
    fn unavailable_source_does_not_block_selecting_or_reopening_another_folder() {
        let f = Fixture::new();
        let a = f.folder("A");
        let b = f.folder("B");
        f.select(&a);
        f.task("原待办", None);
        let moved = f.temp.path().join("移动后的A");
        fs::rename(&a, &moved).unwrap();
        let binding = portable::binding(&f.bootstrap).unwrap().unwrap();
        let cache = portable::cache_path(&f.bootstrap, &binding);
        for file in [
            cache.clone(),
            cache.with_extension("db-wal"),
            cache.with_extension("db-shm"),
        ] {
            if file.exists() {
                fs::remove_file(file).unwrap();
            }
        }
        let status = portable::workspace_status(&f.bootstrap).unwrap();
        assert_eq!(status.root_path.as_deref(), binding.root.to_str());
        assert!(!status.available);
        assert!(status.read_only);
        f.select(&b);
        assert!(f.titles().is_empty());
        assert!(!a.exists());
        f.select(&moved);
        assert_eq!(f.titles(), vec!["原待办"]);
    }

    #[test]
    fn invalid_target_preserves_binding_and_all_original_files() {
        let f = Fixture::new();
        let a = f.folder("A");
        let b = f.folder("B");
        f.select(&a);
        f.task("原待办", None);
        let before = fs::read(f.bootstrap.with_file_name("device.json")).unwrap();
        let config = b.join("config.json");
        fs::write(&config, b"{\"user\":true}").unwrap();
        assert_eq!(
            select(&f.bootstrap, &b).unwrap_err().code,
            "STORAGE_FILE_CONFLICT"
        );
        assert_eq!(fs::read(&config).unwrap(), b"{\"user\":true}");
        assert!(!b.join("todo.json").exists());
        assert!(!b.join(".workspace").exists());
        assert_eq!(
            fs::read(f.bootstrap.with_file_name("device.json")).unwrap(),
            before
        );
        assert_eq!(f.titles(), vec!["原待办"]);
    }

    #[test]
    fn existing_todo_or_reserved_file_is_never_overwritten() {
        let f = Fixture::new();
        let root = f.folder("冲突目录");
        fs::write(root.join("todo.json"), b"personal todo").unwrap();
        assert_eq!(
            select(&f.bootstrap, &root).unwrap_err().code,
            "STORAGE_FILE_CONFLICT"
        );
        assert_eq!(fs::read(root.join("todo.json")).unwrap(), b"personal todo");
        assert!(!root.join("config.json").exists());
        let root = f.folder("冲突文件");
        fs::write(root.join("Files"), b"user file").unwrap();
        assert_eq!(
            select(&f.bootstrap, &root).unwrap_err().code,
            "STORAGE_FILE_CONFLICT"
        );
        assert_eq!(fs::read(root.join("Files")).unwrap(), b"user file");
    }

    #[test]
    fn external_json_edits_refresh_cache_without_historical_hash_or_revision_approval() {
        let f = Fixture::new();
        let root = f.folder("目录");
        f.select(&root);
        f.task("旧标题", None);
        let path = root.join("todo.json");
        let mut todo = io::read(&path).unwrap();
        todo["tasks"][0]["title"] = serde_json::json!("外部修改");
        todo.as_object_mut().unwrap().remove("revision");
        io::write(&path, &todo).unwrap();
        assert_eq!(f.titles(), vec!["外部修改"]);
        let config = root.join("config.json");
        let mut value = io::read(&config).unwrap();
        value.as_object_mut().unwrap().remove("snapshot");
        io::write(&config, &value).unwrap();
        f.select(&root);
        assert_eq!(f.titles(), vec!["外部修改"]);
        f.task("新增", None);
        assert_eq!(f.titles().len(), 2);
    }

    #[test]
    fn directory_copy_at_same_path_does_not_require_inode_approval() {
        let f = Fixture::new();
        let root = f.folder("目录");
        f.select(&root);
        f.task("保留待办", None);
        let original = f.temp.path().join("old");
        fs::rename(&root, &original).unwrap();
        services::safe_fs::copy_tree(&original, &root).unwrap();
        assert_eq!(f.titles(), vec!["保留待办"]);
        f.task("复制后新增", None);
        assert_eq!(f.titles().len(), 2);
        assert_eq!(
            documents::load(&original).unwrap()["todo.json"]["tasks"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
    }

    #[test]
    fn selecting_same_directory_refreshes_without_changing_binding() {
        let f = Fixture::new();
        let root = f.folder("目录");
        let first = f.select(&root);
        f.task("保留", None);
        let before = transfer::inventory(&root).unwrap();
        assert_eq!(f.select(&root).binding_id, first.binding_id);
        assert_eq!(transfer::inventory(&root).unwrap(), before);
    }

    #[test]
    fn first_selection_does_not_copy_legacy_database_tasks() {
        let f = Fixture::new();
        f.task("旧数据库待办", None);
        let root = f.folder("目录");
        f.select(&root);
        assert!(f.titles().is_empty());
        assert_eq!(
            services::tasks::list(&db::connect(&f.bootstrap).unwrap()).unwrap()[0].title,
            "旧数据库待办"
        );
    }

    #[test]
    fn invalid_json_does_not_switch_binding_or_modify_target() {
        let f = Fixture::new();
        let a = f.folder("A");
        let b = f.folder("B");
        f.select(&a);
        f.task("A待办", None);
        f.select(&b);
        f.task("B待办", None);
        f.select(&a);
        fs::write(b.join("todo.json"), b"broken json").unwrap();
        let before = transfer::inventory(&b).unwrap();
        let binding = portable::binding(&f.bootstrap).unwrap().unwrap().id;
        assert!(select(&f.bootstrap, &b).is_err());
        assert_eq!(
            portable::binding(&f.bootstrap).unwrap().unwrap().id,
            binding
        );
        assert_eq!(transfer::inventory(&b).unwrap(), before);
        assert_eq!(f.titles(), vec!["A待办"]);
    }

    #[test]
    fn target_in_use_does_not_switch_binding() {
        let f = Fixture::new();
        let a = f.folder("A");
        let b = f.folder("B");
        f.select(&a);
        f.select(&b);
        f.select(&a);
        let lock = io::lock(&b).unwrap();
        assert_eq!(select(&f.bootstrap, &b).unwrap_err().code, "WORKSPACE_BUSY");
        assert_eq!(
            portable::binding(&f.bootstrap).unwrap().unwrap().root,
            a.canonicalize().unwrap()
        );
        drop(lock);
    }
    #[test]
    fn ordinary_entries_are_visible_in_place_without_exposing_metadata_or_registered_projects() {
        let f = Fixture::new();
        let root = f.folder("目录");
        fs::create_dir(root.join("普通子目录")).unwrap();
        fs::write(root.join("说明.txt"), b"original").unwrap();
        f.select(&root);
        portable::execute(&f.bootstrap, true, |db| {
            services::projects::create(
                db,
                CreateProject {
                    name: "已登记项目".into(),
                    color: "#123456".into(),
                    ..Default::default()
                },
            )
        })
        .unwrap();
        portable::execute(&f.bootstrap, false, |db| {
            let items = entries(db)?;
            assert_eq!(
                items
                    .iter()
                    .map(|entry| entry.name.as_str())
                    .collect::<Vec<_>>(),
                vec!["普通子目录", "说明.txt"]
            );
            assert!(items[0].directory);
            assert_eq!(
                entry_path(db, "说明.txt")?,
                root.canonicalize()?.join("说明.txt")
            );
            assert!(entry_path(db, "../device/app.db").is_err());
            assert!(entry_path(db, "config.json").is_err());
            assert!(entry_path(db, "已登记项目").is_err());
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn edits_during_a_write_are_preserved_instead_of_overwritten() {
        let f = Fixture::new();
        let root = f.folder("目录");
        f.select(&root);
        f.task("原标题", None);
        let path = root.join("todo.json");
        let error = portable::execute(&f.bootstrap, true, |db| {
            let mut todo = io::read(&path)?;
            todo["tasks"][0]["title"] = serde_json::json!("写入期间的外部修改");
            io::write(&path, &todo)?;
            services::tasks::create(
                db,
                CreateTask {
                    title: "应用新增".into(),
                    project_id: None,
                    priority: "low".into(),
                    plan_date: None,
                },
            )
        })
        .unwrap_err();
        assert_eq!(error.code, "WORKSPACE_CONFLICT");
        let todo = io::read(&path).unwrap();
        assert_eq!(todo["tasks"][0]["title"], "写入期间的外部修改");
        assert_eq!(todo["tasks"].as_array().unwrap().len(), 1);
        let next = f.folder("其他目录");
        f.select(&next);
        assert!(f.titles().is_empty());
    }

    #[test]
    fn reminders_follow_the_selected_folder_and_stale_drafts_are_recovered_locally() {
        let f = Fixture::new();
        let a = f.folder("A");
        let b = f.folder("B");
        let original = f.select(&a);
        let task = f.task("待提醒", None);
        let at = (chrono::Utc::now() + chrono::Duration::hours(1)).to_rfc3339();
        portable::execute(&f.bootstrap, true, |db| {
            services::reminders::schedule(db, &task, Some(&at))
        })
        .unwrap();
        f.select(&b);
        let later = (chrono::Utc::now() + chrono::Duration::hours(2)).to_rfc3339();
        portable::execute(&f.bootstrap, false, |db| {
            assert!(services::reminders::due(db, &later)?.is_empty());
            Ok(())
        })
        .unwrap();
        assert_eq!(
            portable::require_binding(&f.bootstrap, original.binding_id.as_deref())
                .unwrap_err()
                .code,
            "WORKSPACE_CHANGED"
        );
        let recovery = portable::recovery::save(
            &f.bootstrap,
            original.binding_id.as_deref(),
            Some("A的未保存草稿"),
            None,
            "目录已更换",
        )
        .unwrap();
        assert_eq!(io::read(&recovery).unwrap()["text"], "A的未保存草稿");
        portable::execute(&f.bootstrap, false, |db| {
            assert_eq!(
                services::preferences::get(db, "capture_draft")?.as_deref(),
                Some("")
            );
            Ok(())
        })
        .unwrap();
        f.select(&a);
        portable::execute(&f.bootstrap, false, |db| {
            assert_eq!(services::reminders::due(db, &later)?.len(), 1);
            Ok(())
        })
        .unwrap();
    }
}
