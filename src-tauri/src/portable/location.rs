//! Folder selection and relocation share one staged, reversible installation path.
use super::{documents, io, journal, transfer};
use crate::{
    error::{AppError, AppResult},
    models::WorkspaceStatus,
    services,
};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

fn exists(path: &Path) -> AppResult<bool> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error.into()),
    }
}

// Include empty directories and identities so an external replacement is detectable.
fn stamp(path: &Path) -> AppResult<BTreeMap<PathBuf, String>> {
    fn visit(
        path: &Path,
        relative: &Path,
        out: &mut BTreeMap<PathBuf, String>,
        depth: usize,
    ) -> AppResult<()> {
        if depth > 64 || out.len() > 100_000 {
            return Err(io::invalid("目录数量或层级超限"));
        }
        let meta = fs::symlink_metadata(path)?;
        if services::files::is_link(&meta) {
            return Err(io::invalid("待替换内容包含链接，请先移除链接"));
        }
        let value = if meta.is_dir() {
            format!("directory:{}", io::fingerprint(path)?)
        } else if meta.is_file() {
            format!("file:{}:{}", io::fingerprint(path)?, io::hash(path)?)
        } else {
            return Err(io::invalid("包含非常规文件"));
        };
        out.insert(relative.to_path_buf(), value);
        if meta.is_dir() {
            for entry in fs::read_dir(path)? {
                let entry = entry?;
                visit(
                    &entry.path(),
                    &relative.join(entry.file_name()),
                    out,
                    depth + 1,
                )?;
            }
        }
        Ok(())
    }
    let mut out = BTreeMap::new();
    if exists(path)? {
        visit(path, Path::new(""), &mut out, 0)?;
    }
    Ok(out)
}

fn remove(path: &Path) -> AppResult<()> {
    if fs::symlink_metadata(path)?.is_dir() {
        fs::remove_dir_all(path)?;
    } else {
        fs::remove_file(path)?;
    }
    Ok(())
}

struct Installed {
    name: PathBuf,
    saved: bool,
    installed: bool,
}
fn install(
    stage: &Path,
    target: &Path,
    saved: &Path,
    name: &Path,
    steps: &mut Vec<Installed>,
) -> AppResult<()> {
    steps.push(Installed {
        name: name.to_owned(),
        saved: false,
        installed: false,
    });
    if exists(&target.join(name))? {
        fs::rename(target.join(name), saved.join((steps.len() - 1).to_string()))?;
        steps.last_mut().unwrap().saved = true;
    }
    fs::rename(stage.join(name), target.join(name))?;
    steps.last_mut().unwrap().installed = true;
    Ok(())
}

/// None means the user declined replacement. Neither binding nor data are changed.
pub fn select(
    bootstrap: &Path,
    target: &Path,
    confirm: impl FnOnce(&[String]) -> bool,
) -> AppResult<Option<WorkspaceStatus>> {
    let target = target.canonicalize()?;
    if !target.is_dir() {
        return Err(io::invalid("请选择文件夹作为存储位置"));
    }
    let target_identity = io::fingerprint(&target)?;
    let binding = super::binding(bootstrap)?;
    let legacy_source = if binding.is_none() {
        services::workspace::root(&crate::db::connect(bootstrap)?)?
    } else {
        None
    };
    let source = binding
        .as_ref()
        .map(|b| b.root.as_path())
        .or(legacy_source.as_deref());
    if source == Some(target.as_path()) && binding.is_some() {
        return super::open(bootstrap, &target).map(Some);
    }
    if source.is_some_and(|source| target.starts_with(source) || source.starts_with(&target)) {
        return Err(AppError::new(
            "INVALID_DESTINATION",
            "新存储目录不能与当前目录互相包含",
        ));
    }
    let source_lock = if let Some(binding) = &binding {
        super::validate_binding(binding)?;
        let lock = io::lock(&binding.root)?;
        journal::recover(&binding.root)?;
        Some(lock)
    } else {
        None
    };
    let source_before = source.map(transfer::inventory).transpose()?;
    let stage = tempfile::Builder::new()
        .prefix(".workspace-migrate-")
        .tempdir_in(
            target
                .parent()
                .ok_or_else(|| io::invalid("不能使用磁盘根目录作为存储位置"))?,
        )?;
    transfer::prepare(bootstrap, stage.path())?;
    let docs = documents::load(stage.path())?;
    transfer::verify_resources(stage.path(), &docs)?;
    let staged_inventory = transfer::inventory(stage.path())?;
    let staged_lock = stage.path().join(".workspace/write.lock");
    if staged_lock.exists() {
        fs::remove_file(staged_lock)?;
    }

    let workspace = target.join(".workspace");
    let whole_workspace = exists(&workspace)? && !fs::symlink_metadata(&workspace)?.is_dir();
    let had_workspace = exists(&workspace)?;
    let had_lock = exists(&workspace.join("write.lock")).unwrap_or(false);
    // A workspace directory keeps its lock inode; replace its children individually.
    let mut names = Vec::new();
    for entry in fs::read_dir(stage.path())? {
        let entry = entry?;
        if entry.file_name() == ".workspace" && !whole_workspace {
            for child in fs::read_dir(entry.path())? {
                names.push(PathBuf::from(".workspace").join(child?.file_name()));
            }
        } else {
            names.push(PathBuf::from(entry.file_name()));
        }
    }
    names.sort();
    if whole_workspace {
        names.retain(|name| name != Path::new(".workspace"));
        names.insert(0, ".workspace".into());
    }
    let before = names
        .iter()
        .map(|name| stamp(&target.join(name)))
        .collect::<AppResult<Vec<_>>>()?;
    let conflicts = names
        .iter()
        .zip(&before)
        .filter(|(_, state)| !state.is_empty())
        .map(|(name, _)| name.to_string_lossy().replace('\\', "/"))
        .collect::<Vec<_>>();
    if !conflicts.is_empty() && !confirm(&conflicts) {
        return Ok(None);
    }
    if source.map(transfer::inventory).transpose()? != source_before {
        return Err(AppError::new(
            "SOURCE_CHANGED",
            "整理期间原目录发生变化，请重试；当前目录未切换",
        ));
    }
    let saved = tempfile::Builder::new()
        .prefix(".workspace-replaced-")
        .tempdir_in(target.parent().unwrap())?;
    // If the process is interrupted, the original entries remain in this sibling folder.
    io::write(
        &saved.path().join("restore.json"),
        &serde_json::json!({"target": target, "entries": names}),
    )?;
    let mut steps = Vec::new();
    let mut target_lock = None;
    let result = (|| {
        if io::fingerprint(&target)? != target_identity || target.canonicalize()? != target {
            return Err(AppError::new(
                "DESTINATION_CHANGED",
                "目标目录已被替换，请重新选择",
            ));
        }
        for (name, state) in names.iter().zip(&before) {
            if stamp(&target.join(name))? != *state {
                return Err(AppError::new(
                    "DESTINATION_CHANGED",
                    "确认期间目标内容发生变化，请重新选择；当前目录未切换",
                ));
            }
        }
        if whole_workspace {
            install(stage.path(), &target, saved.path(), &names[0], &mut steps)?;
        }
        target_lock = Some(io::lock(&target)?);
        // Validate again after acquiring the target lock, before touching any user entry.
        for (name, state) in names.iter().zip(&before).skip(usize::from(whole_workspace)) {
            if stamp(&target.join(name))? != *state {
                return Err(AppError::new(
                    "DESTINATION_CHANGED",
                    "目标内容发生变化，请重试；当前目录未切换",
                ));
            }
        }
        for name in names.iter().skip(usize::from(whole_workspace)) {
            install(stage.path(), &target, saved.path(), name, &mut steps)?;
        }
        for (name, hash) in &staged_inventory {
            if io::hash(&io::path(&target, name)?)? != *hash {
                return Err(io::invalid("迁移文件校验失败，当前目录未切换"));
            }
        }
        let docs = documents::load(&target)?;
        transfer::verify_resources(&target, &docs)?;
        if source.map(transfer::inventory).transpose()? != source_before {
            return Err(AppError::new(
                "SOURCE_CHANGED",
                "迁移期间原目录发生变化，请重试；当前目录未切换",
            ));
        }
        if let Some(binding) = &binding {
            super::validate_binding(binding)?;
        }
        if io::fingerprint(&target)? != target_identity {
            return Err(io::invalid("迁移期间目标目录身份发生变化"));
        }
        super::open_locked(bootstrap, &target)
    })();
    if result.is_err() {
        let held_lock = target_lock.is_some();
        // Keep the target lock throughout rollback unless the lock directory itself is replaced.
        if whole_workspace {
            drop(target_lock.take());
        }
        let rollback = (|| -> AppResult<()> {
            for (index, step) in steps.iter().enumerate().rev() {
                if step.installed {
                    remove(&target.join(&step.name))?;
                }
                if step.saved {
                    fs::rename(
                        saved.path().join(index.to_string()),
                        target.join(&step.name),
                    )?;
                }
            }
            Ok(())
        })();
        drop(target_lock.take());
        if held_lock && !whole_workspace && !had_lock && workspace.join("write.lock").is_file() {
            let _ = fs::remove_file(workspace.join("write.lock"));
        }
        if !had_workspace {
            let _ = fs::remove_dir(&workspace);
        }
        if rollback.is_err() {
            let path = saved.keep();
            return Err(AppError::new(
                "MIGRATION_ROLLBACK_FAILED",
                &format!(
                    "迁移未完成，替换前的内容已保留在 {}，请勿删除",
                    path.display()
                ),
            ));
        }
    }
    drop(target_lock);
    drop(source_lock);
    result.map(Some)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        db,
        models::{CreateProject, CreateTask},
        portable,
    };

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
        fn select(&self, target: &Path) {
            select(&self.bootstrap, target, |_| true).unwrap().unwrap();
        }
        fn task(&self, project_id: Option<String>) -> String {
            portable::execute(&self.bootstrap, true, |db| {
                Ok(services::tasks::create(
                    db,
                    CreateTask {
                        title: "迁移待办".into(),
                        project_id,
                        priority: "medium".into(),
                        plan_date: None,
                    },
                )?
                .id)
            })
            .unwrap()
        }
    }
    #[test]
    fn initial_selection_accepts_nonempty_folder_and_preserves_unrelated_content() {
        let f = Fixture::new();
        assert!(portable::binding(&f.bootstrap).unwrap().is_none());
        let target = f.folder("用户目录");
        fs::write(target.join("已有文档.txt"), b"keep").unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink("已有文档.txt", target.join("快捷方式")).unwrap();
        select(&f.bootstrap, &target, |_| {
            panic!("unrelated entries need no confirmation")
        })
        .unwrap()
        .unwrap();
        documents::load(&target).unwrap();
        assert!(target.join("Files").is_dir());
        assert!(target.join("todo.json").is_file());
        assert_eq!(fs::read(target.join("已有文档.txt")).unwrap(), b"keep");
        #[cfg(unix)]
        assert!(target.join("快捷方式").is_symlink());
    }
    #[test]
    fn declining_initial_replacement_creates_no_workspace_files() {
        let f = Fixture::new();
        let target = f.folder("已有配置");
        fs::write(target.join("config.json"), b"original").unwrap();
        assert!(select(&f.bootstrap, &target, |conflicts| {
            assert_eq!(conflicts, ["config.json"]);
            false
        })
        .unwrap()
        .is_none());
        assert_eq!(fs::read(target.join("config.json")).unwrap(), b"original");
        assert!(!target.join(".workspace").exists());
        assert!(!target.join("Files").exists());
        assert!(portable::binding(&f.bootstrap).unwrap().is_none());
    }
    #[test]
    fn initial_replacement_handles_same_name_files_and_directories_and_legacy_tasks() {
        let f = Fixture::new();
        let task = f.task(None);
        let target = f.folder("初始化");
        fs::create_dir(target.join("config.json")).unwrap();
        fs::write(target.join("config.json/old"), b"old").unwrap();
        fs::write(target.join("Files"), b"old file").unwrap();
        fs::write(target.join(".workspace"), b"old internal name").unwrap();
        fs::write(target.join("保留.txt"), b"keep").unwrap();
        f.select(&target);
        assert!(target.join("Files").is_dir());
        assert!(target.join(".workspace").is_dir());
        assert_eq!(
            documents::load(&target).unwrap()["todo.json"]["tasks"][0]["id"],
            task
        );
        assert_eq!(fs::read(target.join("保留.txt")).unwrap(), b"keep");
    }
    #[test]
    fn migration_preserves_projects_attachments_drafts_and_source() {
        let f = Fixture::new();
        let source = f.folder("原目录");
        f.select(&source);
        let project = portable::execute(&f.bootstrap, true, |db| {
            services::projects::create(
                db,
                CreateProject {
                    name: "农田管理".into(),
                    color: "#1677FF".into(),
                    ..Default::default()
                },
            )
        })
        .unwrap();
        let task = f.task(Some(project.id));
        let attachment = f.temp.path().join("照片.png");
        fs::write(&attachment, b"image").unwrap();
        portable::execute(&f.bootstrap, true, |db| {
            services::attachments::import(db, &task, &attachment)
        })
        .unwrap();
        portable::execute(&f.bootstrap, true, |db| {
            services::preferences::set(db, "capture_draft", "{\"title\":\"草稿\"}")
        })
        .unwrap();
        let before = transfer::inventory(&source).unwrap();
        let target = f.folder("目标目录");
        fs::create_dir(target.join("农田管理")).unwrap();
        fs::write(target.join("农田管理/旧文件"), b"old").unwrap();
        fs::write(target.join("保留.txt"), b"keep").unwrap();
        f.select(&target);
        assert_eq!(transfer::inventory(&source).unwrap(), before);
        assert_eq!(
            fs::read(target.join("农田管理/Files/照片.png")).unwrap(),
            b"image"
        );
        assert!(!target.join("农田管理/旧文件").exists());
        assert_eq!(
            portable::binding(&f.bootstrap).unwrap().unwrap().root,
            target.canonicalize().unwrap()
        );
        portable::execute(&f.bootstrap, false, |db| {
            assert_eq!(services::tasks::list(db)?[0].id, task);
            assert_eq!(services::attachments::list(db, &task)?.len(), 1);
            assert_eq!(
                services::preferences::get(db, "capture_draft")?.as_deref(),
                Some("{\"title\":\"草稿\"}")
            );
            Ok(())
        })
        .unwrap();
    }
    #[test]
    fn changes_during_confirmation_abort_without_replacing_files() {
        let f = Fixture::new();
        let source = f.folder("source");
        f.select(&source);
        let target = f.folder("target");
        fs::write(target.join("config.json"), b"original").unwrap();
        let result = select(&f.bootstrap, &target, |_| {
            fs::write(target.join("config.json"), b"external edit").unwrap();
            true
        });
        assert_eq!(result.unwrap_err().code, "DESTINATION_CHANGED");
        assert_eq!(
            fs::read(target.join("config.json")).unwrap(),
            b"external edit"
        );
        assert!(!target.join("Files").exists());
        assert_eq!(
            portable::binding(&f.bootstrap).unwrap().unwrap().root,
            source.canonicalize().unwrap()
        );
        let result = select(&f.bootstrap, &target, |_| {
            fs::write(source.join("external.txt"), b"external edit").unwrap();
            true
        });
        assert_eq!(result.unwrap_err().code, "SOURCE_CHANGED");
        assert_eq!(
            fs::read(target.join("config.json")).unwrap(),
            b"external edit"
        );
    }
    #[test]
    fn failed_binding_restores_replaced_content_and_keeps_source_binding() {
        for workspace_kind in ["absent", "file", "directory"] {
            let f = Fixture::new();
            let source = f.folder("source");
            f.select(&source);
            let original_binding = fs::read(f.bootstrap.with_file_name("device.json")).unwrap();
            let target = f.folder("target");
            fs::write(target.join("config.json"), b"original config").unwrap();
            fs::create_dir(target.join("Files")).unwrap();
            fs::write(target.join("Files/原文件"), b"original attachment").unwrap();
            if workspace_kind == "file" {
                fs::write(target.join(".workspace"), b"original internal file").unwrap();
            } else if workspace_kind == "directory" {
                fs::create_dir(target.join(".workspace")).unwrap();
                fs::create_dir(target.join(".workspace/transactions")).unwrap();
                fs::write(
                    target.join(".workspace/transactions/original"),
                    b"original journal",
                )
                .unwrap();
                fs::write(target.join(".workspace/unrelated"), b"keep").unwrap();
            }
            let before = transfer::inventory(&target).unwrap();
            // Force an error after the files are installed, before committing the binding.
            fs::remove_dir_all(f.bootstrap.parent().unwrap().join("cache")).unwrap();
            fs::write(
                f.bootstrap.parent().unwrap().join("cache"),
                b"block cache creation",
            )
            .unwrap();
            assert!(select(&f.bootstrap, &target, |_| true).is_err());
            assert_eq!(transfer::inventory(&target).unwrap(), before);
            assert_eq!(
                target.join(".workspace").exists(),
                workspace_kind != "absent"
            );
            assert_eq!(
                fs::read(f.bootstrap.with_file_name("device.json")).unwrap(),
                original_binding
            );
        }
    }
    #[test]
    fn active_target_lock_is_respected_and_kept_intact() {
        let f = Fixture::new();
        let target = f.folder("target");
        let lock = io::lock(&target).unwrap();
        let identity = io::fingerprint(&target.join(".workspace/write.lock")).unwrap();
        assert_eq!(
            select(&f.bootstrap, &target, |_| true).unwrap_err().code,
            "WORKSPACE_BUSY"
        );
        assert_eq!(
            io::fingerprint(&target.join(".workspace/write.lock")).unwrap(),
            identity
        );
        assert!(!target.join("config.json").exists());
        assert!(io::lock(&target).is_err());
        drop(lock);
    }
    #[test]
    fn same_directory_is_noop_and_nested_directory_is_rejected() {
        let f = Fixture::new();
        let source = f.folder("source");
        f.select(&source);
        let original = portable::binding(&f.bootstrap).unwrap().unwrap().id;
        select(&f.bootstrap, &source, |_| panic!("same directory"))
            .unwrap()
            .unwrap();
        assert_eq!(
            portable::binding(&f.bootstrap).unwrap().unwrap().id,
            original
        );
        let child = source.join("child");
        fs::create_dir(&child).unwrap();
        assert_eq!(
            select(&f.bootstrap, &child, |_| true).unwrap_err().code,
            "INVALID_DESTINATION"
        );
        assert_eq!(
            select(&f.bootstrap, f.temp.path(), |_| true)
                .unwrap_err()
                .code,
            "INVALID_DESTINATION"
        );
    }
}
