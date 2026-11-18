use super::io;
use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    cell::RefCell,
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
enum Action {
    Rename {
        from: String,
        to: String,
        identity: String,
    },
    Create {
        path: String,
        identity: String,
        sha256: String,
    },
    Retire {
        path: String,
        sha256: String,
    },
    Directory {
        path: String,
        identity: Option<String>,
    },
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Journal {
    format_version: u32,
    before: BTreeMap<String, Value>,
    after: Option<BTreeMap<String, Value>>,
    actions: Vec<Action>,
}
struct Active {
    root: PathBuf,
    journal: Journal,
}
thread_local! { static ACTIVE: RefCell<Option<Active>> = const { RefCell::new(None) }; }
fn location(root: &Path) -> AppResult<PathBuf> {
    io::path(root, ".workspace/transactions/current.json")
}
fn save(active: &Active) -> AppResult<()> {
    // Before the commit decision, recovery only needs filesystem actions and the root identity.
    // Full prior documents stay in memory until we know which JSON files actually changed.
    let before = if active.journal.after.is_none() {
        active
            .journal
            .before
            .iter()
            .filter(|(name, _)| name.as_str() == "config.json")
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    } else {
        active.journal.before.clone()
    };
    let persisted = Journal {
        format_version: 1,
        before,
        after: active.journal.after.clone(),
        actions: active.journal.actions.clone(),
    };
    io::write(
        &location(&active.root)?,
        &serde_json::to_value(persisted).map_err(|_| io::invalid("恢复记录无效"))?,
    )
}
pub fn begin(root: &Path, before: BTreeMap<String, Value>) -> AppResult<()> {
    let active = Active {
        root: root.into(),
        journal: Journal {
            format_version: 1,
            before,
            after: None,
            actions: vec![],
        },
    };
    save(&active)?;
    ACTIVE.with(|slot| *slot.borrow_mut() = Some(active));
    Ok(())
}
fn record(build: impl FnOnce(&Path) -> AppResult<Action>) -> AppResult<()> {
    ACTIVE.with(|slot| {
        let mut slot = slot.borrow_mut();
        if let Some(active) = slot.as_mut() {
            active.journal.actions.push(build(&active.root)?);
            save(active)?;
        }
        Ok(())
    })
}
fn rel(root: &Path, file: &Path) -> AppResult<String> {
    let value = file
        .strip_prefix(root)
        .map_err(|_| io::invalid("文件操作越出工作目录"))?
        .to_str()
        .ok_or_else(|| io::invalid("文件路径编码无效"))?
        .replace('\\', "/");
    io::relative(&value)?;
    Ok(value)
}
pub fn rename(from: &Path, to: &Path) -> AppResult<()> {
    record(|root| {
        Ok(Action::Rename {
            from: rel(root, from)?,
            to: rel(root, to)?,
            identity: io::fingerprint(from)?,
        })
    })
}
pub fn create(staged: &Path, target: &Path) -> AppResult<()> {
    record(|root| {
        Ok(Action::Create {
            path: rel(root, target)?,
            identity: io::fingerprint(staged)?,
            sha256: io::hash(staged)?,
        })
    })
}
pub fn retire(path: &Path, sha256: &str) -> AppResult<()> {
    record(|root| {
        Ok(Action::Retire {
            path: rel(root, path)?,
            sha256: sha256.into(),
        })
    })
}
pub fn directory(target: &Path) -> AppResult<()> {
    record(|root| {
        Ok(Action::Directory {
            path: rel(root, target)?,
            identity: None,
        })
    })?;
    fs::create_dir(target)?;
    ACTIVE.with(|slot| {
        let mut slot = slot.borrow_mut();
        if let Some(active) = slot.as_mut() {
            if let Some(Action::Directory { identity, .. }) = active.journal.actions.last_mut() {
                *identity = Some(io::fingerprint(target)?);
            }
            save(active)?;
        }
        Ok(())
    })
}
pub fn detach() {
    ACTIVE.with(|slot| *slot.borrow_mut() = None);
}
pub fn commit(documents: BTreeMap<String, Value>) -> AppResult<()> {
    ACTIVE.with(|slot| {
        let mut slot = slot.borrow_mut();
        let active = slot.as_mut().ok_or_else(|| io::invalid("缺少本地事务"))?;
        let changed = documents
            .iter()
            .filter(|(name, value)| active.journal.before.get(*name) != Some(*value))
            .map(|(name, value)| (name.clone(), value.clone()))
            .collect::<BTreeMap<_, _>>();
        active
            .journal
            .before
            .retain(|name, _| changed.contains_key(name) || !documents.contains_key(name));
        active.journal.after = Some(changed);
        save(active)
    })?;
    let root = ACTIVE.with(|slot| slot.borrow().as_ref().unwrap().root.clone());
    detach();
    recover(&root)
}
pub fn abort(root: &Path) -> AppResult<()> {
    detach();
    recover(root)
}
pub fn recover(root: &Path) -> AppResult<()> {
    let file = location(root)?;
    if !file.exists() {
        return Ok(());
    }
    let journal: Journal = serde_json::from_value(io::read(&file)?)
        .map_err(|_| io::invalid("事务记录损坏，请保留目录并恢复备份"))?;
    if journal.format_version != 1 {
        return Err(io::invalid("事务格式不受支持"));
    }
    if let Some(after) = &journal.after {
        let next_root = after
            .get("config.json")
            .ok_or_else(|| io::invalid("事务缺少根配置"))?;
        if next_root["formatVersion"] != 1 || next_root["kind"] != "workspace" {
            return Err(io::invalid("事务中的目录协议不受支持"));
        }
        let previous_root = journal.before.get("config.json");
        if previous_root.is_some_and(|root| root["workspaceId"] != next_root["workspaceId"]) {
            return Err(io::invalid("恢复事务不能更换工作目录身份"));
        }
        let members = |root: Option<&Value>| -> AppResult<std::collections::HashSet<String>> {
            let mut paths = std::collections::HashSet::from(["config.json".to_string()]);
            if let Some(root) = root {
                paths.insert("todo.json".into());
                for project in super::documents::items(root, "projects")? {
                    let directory = super::documents::text(project, "directory")?;
                    for name in [
                        format!("{directory}/config.json"),
                        format!("{directory}/todo.json"),
                    ] {
                        io::relative(&name)?;
                        paths.insert(name);
                    }
                }
                for draft in super::documents::items(root, "drafts")? {
                    let name = super::documents::text(draft, "document")?;
                    io::relative(name)?;
                    paths.insert(name.to_string());
                }
            }
            Ok(paths)
        };
        let previous_members = members(previous_root)?;
        let next_members = members(Some(next_root))?;
        if journal
            .before
            .keys()
            .any(|name| !previous_members.contains(name))
            || after.keys().any(|name| !next_members.contains(name))
        {
            return Err(io::invalid("恢复事务包含非业务文档"));
        }
        // Detect every already-visible conflict before publishing the first document.
        // Individual checks below also protect against later external replacements.
        for (name, value) in after {
            let file = io::path(root, name)?;
            if file.exists() {
                let current = io::read(&file)?;
                if &current != value && !journal.before.values().any(|before| before == &current) {
                    return Err(AppError::new(
                        "WORKSPACE_CONFLICT",
                        "恢复前检测到外部修改，所有原件与日志已保留",
                    ));
                }
            }
        }
        for (name, value) in journal
            .before
            .iter()
            .filter(|(name, _)| !after.contains_key(*name))
        {
            let file = io::path(root, name)?;
            if file.exists() && io::read(&file)? != *value {
                return Err(AppError::new(
                    "WORKSPACE_CONFLICT",
                    "已移出的文档被外部修改，已保留原件和日志",
                ));
            }
        }
        for (name, value) in after
            .iter()
            .filter(|(name, _)| name.as_str() != "config.json")
        {
            let target = io::path(root, name)?;
            if target.exists() {
                let current = io::read(&target)?;
                if current != *value && !journal.before.values().any(|before| before == &current) {
                    return Err(AppError::new(
                        "WORKSPACE_CONFLICT",
                        "恢复时检测到外部修改，已保留事务和原件",
                    ));
                }
            }
            io::write(&target, value)?;
        }
        let value = after
            .get("config.json")
            .ok_or_else(|| io::invalid("事务缺少根配置"))?;
        let target = io::path(root, "config.json")?;
        if target.exists() {
            let current = io::read(&target)?;
            if &current != value
                && journal
                    .before
                    .get("config.json")
                    .is_some_and(|before| before != &current)
            {
                return Err(AppError::new(
                    "WORKSPACE_CONFLICT",
                    "根配置已被外部修改，未覆盖",
                ));
            }
        }
        io::write(&target, value)?;
        // Retire only old protocol documents. Resource files are retained separately.
        // This also frees the fixed quick-draft path for the next capture.
        for (name, previous) in journal
            .before
            .iter()
            .filter(|(name, _)| !after.contains_key(*name))
        {
            let file = io::path(root, name)?;
            if file.exists() {
                if io::read(&file)? != *previous {
                    return Err(AppError::new(
                        "WORKSPACE_CONFLICT",
                        "已移出的配置被外部修改，已保留原件和恢复记录",
                    ));
                }
                fs::remove_file(file)?;
            }
        }
        // Drop only unchanged resources explicitly retired by a committed draft operation.
        // External replacements remain intact even when they are no longer referenced.
        for action in &journal.actions {
            if let Action::Retire { path, sha256 } = action {
                if let Ok(resource) = io::path(root, path) {
                    if resource.is_file() && io::hash(&resource).is_ok_and(|hash| hash == *sha256) {
                        // Cleanup is optional after a durable commit; unreadable/replaced
                        // resources stay intact and must not invalidate saved business data.
                        let _ = fs::remove_file(resource);
                    }
                }
            }
        }
    } else {
        let root_config = io::path(root, "config.json")?;
        if let Some(previous) = journal.before.get("config.json") {
            if root_config.exists() && io::read(&root_config)? != *previous {
                return Err(AppError::new(
                    "WORKSPACE_CONFLICT",
                    "未提交事务期间根配置被外部修改，已保留原件与日志",
                ));
            }
        }
        for action in journal.actions.iter().rev() {
            match action {
                Action::Rename { from, to, identity } => {
                    let a = io::path(root, from)?;
                    let b = io::path(root, to)?;
                    if !a.exists() && b.exists() {
                        if io::fingerprint(&b)? != *identity {
                            return Err(io::invalid("恢复文件身份变化，请核对事务"));
                        }
                        crate::services::safe_fs::rename_exclusive(&b, &a)?;
                    } else if !a.exists() || io::fingerprint(&a)? != *identity {
                        return Err(AppError::new(
                            "WORKSPACE_CONFLICT",
                            "未完成的移动操作被外部修改，已保留原件和恢复记录",
                        ));
                    }
                }
                Action::Create {
                    path,
                    identity,
                    sha256,
                } => {
                    let target = io::path(root, path)?;
                    if target.exists() && io::fingerprint(&target)? == *identity {
                        if io::hash(&target)? != *sha256 {
                            return Err(AppError::new(
                                "WORKSPACE_CONFLICT",
                                "未完成的新文件已被外部修改，已保留文件与恢复记录",
                            ));
                        }
                        fs::remove_file(target)?;
                    }
                }
                Action::Retire { .. } => {}
                Action::Directory { path, identity } => {
                    let target = io::path(root, path)?;
                    if target.is_dir()
                        && identity
                            .as_ref()
                            .is_some_and(|id| io::fingerprint(&target).ok().as_ref() == Some(id))
                        && fs::read_dir(&target)?.next().is_none()
                    {
                        fs::remove_dir(target)?;
                    }
                }
            }
        }
    }
    fs::remove_file(file)?;
    Ok(())
}

pub struct Scope;
impl Drop for Scope {
    fn drop(&mut self) {
        detach();
    }
}
pub fn scope() -> Scope {
    Scope
}
