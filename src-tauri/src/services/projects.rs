use crate::{
    error::{AppError, AppResult},
    models::{CreateProject, Project, ProjectDirectoryMerge, ProjectLink, ProjectTag, Task},
    services::workspace,
};
use chrono::Utc;
use rusqlite::{params, Connection};
use std::{collections::HashSet, fs, path::Path};
use unicode_normalization::UnicodeNormalization;
use uuid::Uuid;

pub fn normalized_name(name: &str) -> AppResult<String> {
    let name = name.trim();
    if name.is_empty()
        || name.chars().count() > 100
        || name.ends_with([' ', '.'])
        || name
            .chars()
            .any(|c| c.is_control() || "<>:\"/\\|?*".contains(c))
        || [".", "..", "_收件箱", "_回收站"].contains(&name)
    {
        return Err(AppError::new(
            "INVALID_PROJECT_NAME",
            "项目名需为 1–100 字，且不能包含路径字符或保留名称",
        ));
    }
    let normalized = name.nfc().collect::<String>().to_lowercase();
    if ["files", "config.json", "todo.json", ".workspace", ".assets"].contains(&normalized.as_str())
    {
        return Err(AppError::new(
            "INVALID_PROJECT_NAME",
            "该名称用于应用数据目录，请更换项目名称",
        ));
    }
    let stem = normalized.split('.').next().unwrap_or("");
    if ["con", "prn", "aux", "nul"].contains(&stem)
        || (1..=9).any(|i| stem == format!("com{i}") || stem == format!("lpt{i}"))
    {
        return Err(AppError::new(
            "INVALID_PROJECT_NAME",
            "该名称为 Windows 保留名称，请更换",
        ));
    }
    Ok(normalized)
}

pub fn list(connection: &Connection) -> AppResult<Vec<Project>> {
    let mut statement = connection.prepare(
        "SELECT p.id,p.name,p.description,p.color,p.status,p.directory_relpath,p.created_at,
         p.links_json,COUNT(t.id),COALESCE(SUM(CASE WHEN t.status='done' THEN 1 ELSE 0 END),0),p.pinned,p.tags_json
         FROM projects p LEFT JOIN tasks t ON t.project_id=p.id AND t.status!='canceled' AND t.deleted_at IS NULL
         WHERE p.id!='__inbox__' GROUP BY p.id ORDER BY p.created_at DESC,p.id",
    )?;
    let rows = statement.query_map([], |row| {
        Ok(Project {
            id: row.get(0)?,
            name: row.get(1)?,
            description: row.get(2)?,
            color: row.get(3)?,
            status: row.get(4)?,
            directory_relpath: row.get(5)?,
            created_at: row.get(6)?,
            links: serde_json::from_str(&row.get::<_, String>(7)?).map_err(|error| {
                rusqlite::Error::FromSqlConversionFailure(
                    7,
                    rusqlite::types::Type::Text,
                    Box::new(error),
                )
            })?,
            task_count: row.get(8)?,
            completed_count: row.get(9)?,
            pinned: row.get(10)?,
            tags: serde_json::from_str(&row.get::<_, String>(11)?).map_err(|error| {
                rusqlite::Error::FromSqlConversionFailure(
                    11,
                    rusqlite::types::Type::Text,
                    Box::new(error),
                )
            })?,
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

pub fn validated_url(raw: &str) -> AppResult<String> {
    let raw = raw.trim();
    let invalid = || AppError::new("INVALID_LINK", "请输入有效的 http 或 https 网站地址");
    if raw.is_empty() || raw.chars().any(|c| c.is_whitespace() || c == '\\') {
        return Err(invalid());
    }
    let lower = raw.to_ascii_lowercase();
    let explicit = lower.starts_with("http://") || lower.starts_with("https://");
    let value = if explicit {
        raw.to_string()
    } else if raw.starts_with("//") {
        format!("https:{raw}")
    } else {
        if let Some((prefix, rest)) = raw.split_once(':') {
            let scheme = prefix
                .chars()
                .next()
                .is_some_and(|c| c.is_ascii_alphabetic())
                && prefix
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || "+-.".contains(c));
            let port = rest.split(['/', '?', '#']).next().unwrap_or("");
            if scheme && (port.is_empty() || !port.chars().all(|c| c.is_ascii_digit())) {
                return Err(invalid());
            }
        }
        format!("https://{raw}")
    };
    let mut url = tauri::Url::parse(&value).map_err(|_| invalid())?;
    if value.len() > 2048
        || !["http", "https"].contains(&url.scheme())
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err(invalid());
    }
    let host = url.host_str().unwrap_or("");
    let local = host == "localhost"
        || host.ends_with(".localhost")
        || host.ends_with(".local")
        || host == "[::1]"
        || host
            .parse::<std::net::Ipv4Addr>()
            .is_ok_and(|ip| ip.is_loopback() || ip.is_private());
    if !explicit && !raw.starts_with("//") && local {
        url.set_scheme("http").map_err(|_| invalid())?;
    }
    Ok(url.to_string())
}
fn validated_links(links: &[ProjectLink]) -> AppResult<Vec<ProjectLink>> {
    let mut ids = std::collections::HashSet::new();
    if links.len() > 50 {
        return Err(AppError::new("INVALID_LINK", "每个项目最多保存 50 个链接"));
    }
    links
        .iter()
        .map(|link| {
            if link.id.is_empty()
                || link.id.len() > 100
                || !ids.insert(&link.id)
                || link.name.trim().is_empty()
                || link.name.trim().chars().count() > 80
                || !["", "github", "gitlab", "gitee", "website"].contains(&link.icon.as_str())
            {
                return Err(AppError::new("INVALID_LINK", "链接名称、图标或标识无效"));
            }
            if link.icon == "website"
                && !link
                    .icon_data
                    .as_deref()
                    .is_some_and(super::link_icons::valid_icon_data)
            {
                return Err(AppError::new("INVALID_LINK", "请重新提取网站图标"));
            }
            Ok(ProjectLink {
                id: link.id.clone(),
                name: link.name.trim().into(),
                url: validated_url(&link.url)?,
                icon: link.icon.clone(),
                icon_data: if link.icon == "website" {
                    link.icon_data.clone()
                } else {
                    None
                },
            })
        })
        .collect()
}
pub fn validated_tags(tags: &[ProjectTag]) -> AppResult<Vec<ProjectTag>> {
    if tags.len() > 20 {
        return Err(AppError::new("INVALID_TAG", "每个项目最多添加 20 个标签"));
    }
    let mut names = HashSet::new();
    tags.iter()
        .map(|tag| {
            let name = tag.name.trim().nfc().collect::<String>();
            if name.is_empty() || name.chars().count() > 30 || name.chars().any(char::is_control) {
                return Err(AppError::new(
                    "INVALID_TAG",
                    "标签名称需为 1–30 字，且不能包含控制字符",
                ));
            }
            if !names.insert(name.to_lowercase()) {
                return Err(AppError::new("INVALID_TAG", "项目标签名称不能重复"));
            }
            if tag.color.len() != 7
                || !tag.color.starts_with('#')
                || !tag.color[1..].chars().all(|c| c.is_ascii_hexdigit())
            {
                return Err(AppError::new("INVALID_TAG", "请选择有效的标签颜色"));
            }
            Ok(ProjectTag {
                name,
                color: tag.color.to_ascii_uppercase(),
            })
        })
        .collect()
}

fn directory_changed() -> AppError {
    AppError::new(
        "DIRECTORY_CHANGED",
        "文件夹或存储位置已变化，请重新创建并确认合并",
    )
}

fn directory_fingerprint(path: &Path) -> AppResult<String> {
    let metadata = fs::symlink_metadata(path)?;
    if super::files::is_link(&metadata) || !metadata.is_dir() {
        return Err(AppError::new(
            "DIRECTORY_NOT_MERGEABLE",
            "同名位置是文件或链接，无法合并，请更换项目名称",
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        Ok(format!(
            "{}:{}:{:?}",
            metadata.dev(),
            metadata.ino(),
            metadata.created().ok()
        ))
    }
    #[cfg(not(unix))]
    {
        Ok(format!(
            "{:?}:{:?}",
            metadata.created()?,
            metadata.modified()?
        ))
    }
}

fn verify_merge(
    connection: &Connection,
    root: &Path,
    target: &Path,
    confirmation: &ProjectDirectoryMerge,
) -> AppResult<()> {
    if workspace::available_root(connection)? != root
        || target.to_string_lossy() != confirmation.path
        || target.canonicalize().ok().as_deref().and_then(Path::parent) != Some(root)
        || directory_fingerprint(target).ok().as_deref() != Some(confirmation.fingerprint.as_str())
    {
        return Err(directory_changed());
    }
    Ok(())
}

pub fn create(connection: &mut Connection, input: CreateProject) -> AppResult<Project> {
    if input.status.as_deref().is_some_and(|s| s != "active") {
        return Err(AppError::new("INVALID_STATUS", "新项目状态应为进行中"));
    }
    let links = validated_links(input.links.as_deref().unwrap_or(&[]))?;
    let links_json = serde_json::to_string(&links)
        .map_err(|_| AppError::new("INVALID_LINK", "无法保存项目链接"))?;
    let tags = validated_tags(input.tags.as_deref().unwrap_or(&[]))?;
    let tags_json = serde_json::to_string(&tags)
        .map_err(|_| AppError::new("INVALID_TAG", "无法保存项目标签"))?;
    let mut name = input.name.trim().to_string();
    let normalized = normalized_name(&input.name)?;
    if input.description.chars().count() > 2000 {
        return Err(AppError::new("VALIDATION_ERROR", "项目描述最多 2000 字"));
    }
    if input.color.len() != 7
        || !input.color.starts_with('#')
        || !input.color[1..].chars().all(|c| c.is_ascii_hexdigit())
    {
        return Err(AppError::new("VALIDATION_ERROR", "请选择有效的项目颜色"));
    }
    let root = workspace::available_root(connection)?;
    let exists: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM projects WHERE normalized_name=?1)",
        [&normalized],
        |row| row.get(0),
    )?;
    if exists {
        return Err(AppError::new(
            "PROJECT_EXISTS",
            "已存在同名项目，请更换名称",
        ));
    }
    // Case/Unicode-equivalent entries can coexist on case-sensitive volumes.
    let mut matches = Vec::new();
    for entry in fs::read_dir(&root)? {
        let entry = entry?;
        if entry
            .file_name()
            .to_string_lossy()
            .nfc()
            .collect::<String>()
            .to_lowercase()
            == normalized
        {
            matches.push(entry);
        }
    }
    if matches.len() > 1 {
        return Err(AppError::new(
            "DIRECTORY_AMBIGUOUS",
            "存在多个大小写或 Unicode 等价的同名位置，请先整理文件夹",
        ));
    }
    let target = if let Some(entry) = matches.pop() {
        let actual_name = entry.file_name().into_string().map_err(|_| {
            AppError::new("DIRECTORY_NOT_MERGEABLE", "文件夹名称无法识别，请更换名称")
        })?;
        normalized_name(&actual_name)?;
        let target = entry.path();
        let fingerprint = directory_fingerprint(&target)?;
        if target.canonicalize()?.parent() != Some(root.as_path()) {
            return Err(AppError::new(
                "DIRECTORY_NOT_MERGEABLE",
                "文件夹不在当前存储位置，无法合并",
            ));
        }
        let mut statement = connection.prepare("SELECT directory_relpath FROM projects")?;
        for row in statement.query_map([], |row| row.get::<_, String>(0))? {
            if row?.nfc().collect::<String>().to_lowercase() == normalized {
                return Err(AppError::new(
                    "PROJECT_EXISTS",
                    "该文件夹已关联项目，请打开已有项目",
                ));
            }
        }
        if let Some(confirmation) = &input.directory_merge {
            verify_merge(connection, &root, &target, confirmation)?;
        } else {
            return Err(AppError::new("DIRECTORY_EXISTS", "同名文件夹已存在，请确认是否合并")
                .with_details(serde_json::json!({"name":actual_name,"path":target.to_string_lossy(),"fingerprint":fingerprint})));
        }
        // Keep the project title and directory name consistent, including case/NFC form.
        name = actual_name;
        target
    } else {
        if input.directory_merge.is_some() {
            return Err(directory_changed());
        }
        root.join(&name)
    };
    let id = Uuid::new_v4().to_string();
    let created_at = Utc::now().to_rfc3339();
    let merging = input.directory_merge.is_some();
    if merging
        && crate::portable::is_portable(connection)
        && target.join("config.json").is_file()
        && crate::portable::io::read(&target.join("config.json"))
            .is_ok_and(|v| v["kind"] == "project")
    {
        return Err(AppError::new(
            "PROJECT_IMPORT_REQUIRED",
            "此文件夹已有项目配置，请使用导入项目功能",
        ));
    }
    if !merging {
        connection.execute("INSERT INTO filesystem_operations(id,kind,target,created_at) VALUES(?1,'create_project',?2,?3)", params![id,name,created_at])?;
        // Only the new-directory branch owns a directory that can be removed on failure.
        if let Err(error) = crate::portable::journal::directory(&target) {
            connection.execute(
                "UPDATE filesystem_operations SET state='completed' WHERE id=?1",
                [&id],
            )?;
            return Err(if target.exists() {
                directory_changed()
            } else {
                error
            });
        }
    }
    let result = (|| -> AppResult<()> {
        let transaction = connection.transaction()?;
        transaction.execute("INSERT INTO projects(id,name,normalized_name,directory_relpath,description,color,created_at,links_json,tags_json) VALUES(?1,?2,?3,?2,?4,?5,?6,?7,?8)", params![id,name,normalized,input.description,input.color,created_at,links_json,tags_json])?;
        if !merging {
            transaction.execute(
                "UPDATE filesystem_operations SET state='completed' WHERE id=?1",
                [&id],
            )?;
        }
        if let Some(confirmation) = &input.directory_merge {
            verify_merge(&transaction, &root, &target, confirmation)?;
        }
        transaction.commit()?;
        Ok(())
    })();
    if let Err(error) = result {
        if merging {
            return Err(error);
        }
        // Only remove our just-created directory if it is still empty.
        let state = if fs::remove_dir(&target).is_ok() {
            "completed"
        } else {
            "needs_attention"
        };
        let _ = connection.execute(
            "UPDATE filesystem_operations SET state=?2 WHERE id=?1",
            params![id, state],
        );
        return Err(error);
    }
    if crate::portable::is_portable(connection) {
        if merging {
            crate::portable::prepare_merged_folder(&target)?;
        }
        let files = target.join("Files");
        if !files.exists() {
            crate::portable::journal::directory(&files)?;
        }
    }
    Ok(Project {
        id,
        name: name.clone(),
        directory_relpath: name,
        description: input.description,
        color: input.color,
        status: "active".into(),
        created_at,
        task_count: 0,
        completed_count: 0,
        pinned: false,
        links,
        tags,
    })
}

pub fn folder(connection: &Connection, id: &str) -> AppResult<std::path::PathBuf> {
    use rusqlite::OptionalExtension;
    let relative: Option<String> = connection
        .query_row(
            "SELECT directory_relpath FROM projects WHERE id=?1",
            [id],
            |row| row.get(0),
        )
        .optional()?;
    let relative = relative.ok_or_else(|| AppError::new("PROJECT_NOT_FOUND", "项目不存在"))?;
    let root = workspace::available_root(connection)?;
    let candidate = root.join(&relative);
    if super::files::is_link(&fs::symlink_metadata(&candidate)?) {
        return Err(AppError::new(
            "UNSAFE_DIRECTORY",
            "项目目录被替换为链接，请重新检查目录",
        ));
    }
    let path = candidate.canonicalize()?;
    if !path.is_dir() || path.parent() != Some(root.as_path()) {
        return Err(AppError::new(
            "UNSAFE_DIRECTORY",
            "项目目录不在预期的存储位置",
        ));
    }
    Ok(path)
}

pub fn update(connection: &mut Connection, id: &str, input: CreateProject) -> AppResult<Project> {
    let old = list(connection)?
        .into_iter()
        .find(|p| p.id == id)
        .ok_or_else(|| AppError::new("PROJECT_NOT_FOUND", "项目不存在"))?;
    let normalized = normalized_name(&input.name)?;
    if input.description.chars().count() > 2000
        || input.color.len() != 7
        || !input.color.starts_with('#')
        || !input.color[1..].chars().all(|c| c.is_ascii_hexdigit())
    {
        return Err(AppError::new("VALIDATION_ERROR", "项目描述或颜色无效"));
    }
    let duplicate: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM projects WHERE normalized_name=?1 AND id!=?2)",
        params![normalized, id],
        |r| r.get(0),
    )?;
    if duplicate {
        return Err(AppError::new("PROJECT_EXISTS", "已存在同名项目"));
    }
    let name = input.name.trim();
    let root = workspace::available_root(connection)?;
    let source = folder(connection, id)?;
    let target = root.join(name);
    let renamed = old.directory_relpath != name;
    if renamed {
        // Equivalent names need a two-step rename on case-insensitive volumes.
        for entry in fs::read_dir(&root)? {
            let entry = entry?;
            if entry
                .file_name()
                .to_string_lossy()
                .nfc()
                .collect::<String>()
                .to_lowercase()
                == normalized
                && entry.path() != source
            {
                return Err(AppError::new(
                    "DIRECTORY_EXISTS",
                    "目标目录已存在，请更换名称",
                ));
            }
        }
    }
    let links = validated_links(input.links.as_deref().unwrap_or(&old.links))?;
    let links_json = serde_json::to_string(&links)
        .map_err(|_| AppError::new("INVALID_LINK", "无法保存项目链接"))?;
    let tags = validated_tags(input.tags.as_deref().unwrap_or(&old.tags))?;
    let tags_json = serde_json::to_string(&tags)
        .map_err(|_| AppError::new("INVALID_TAG", "无法保存项目标签"))?;
    let status = input.status.as_deref().unwrap_or(&old.status);
    let policy = input.pending_policy.as_deref().unwrap_or("inbox");
    validate_status(status, policy)?;
    let status_change = if status != old.status {
        Some(prepare_status(connection, id, status, policy)?)
    } else {
        None
    };
    let operation = Uuid::new_v4().to_string();
    let intermediate = root.join(format!(".project-manage-rename-{operation}"));
    let detail=serde_json::json!({"projectId":id,"source":old.directory_relpath,"target":name,"intermediate":intermediate.file_name().unwrap().to_string_lossy()}).to_string();
    if renamed {
        connection.execute("INSERT INTO filesystem_operations(id,kind,target,detail,created_at) VALUES(?1,'rename_project',?2,?3,?4)",params![operation,name,detail,Utc::now().to_rfc3339()])?;
        super::safe_fs::rename_exclusive(&source, &intermediate)?;
        if let Err(error) = super::safe_fs::rename_exclusive(&intermediate, &target) {
            let _ = super::safe_fs::rename_exclusive(&intermediate, &source);
            return Err(error);
        }
    }
    let result = (|| -> AppResult<()> {
        let transaction = connection.transaction()?;
        transaction.execute("UPDATE projects SET name=?2,normalized_name=?3,directory_relpath=?2,description=?4,color=?5,links_json=?6,tags_json=?7 WHERE id=?1",params![id,name,normalized,input.description,input.color,links_json,tags_json])?;
        if let Some(prepared) = &status_change {
            apply_status(&transaction, id, status, policy, prepared)?;
        }
        if renamed {
            transaction.execute(
                "UPDATE filesystem_operations SET state='completed' WHERE id=?1",
                [&operation],
            )?;
        }
        transaction.commit()?;
        Ok(())
    })();
    if let Err(error) = result {
        if renamed && super::safe_fs::rename_exclusive(&target, &source).is_ok() {
            let _ = connection.execute(
                "UPDATE filesystem_operations SET state='completed' WHERE id=?1",
                [&operation],
            );
        }
        return Err(error);
    }
    list(connection)?
        .into_iter()
        .find(|p| p.id == id)
        .ok_or_else(|| AppError::new("PROJECT_NOT_FOUND", "项目不存在"))
}
fn validate_status(status: &str, policy: &str) -> AppResult<()> {
    if !["active", "completed", "archived"].contains(&status)
        || !["inbox", "complete", "cancel"].contains(&policy)
    {
        return Err(AppError::new(
            "INVALID_STATUS",
            "项目状态或未完成待办处理方式无效",
        ));
    }
    Ok(())
}
struct PreparedStatus {
    tasks: Vec<Task>,
    files: Vec<(String, Vec<String>)>,
}
fn prepare_status(
    connection: &mut Connection,
    id: &str,
    status: &str,
    policy: &str,
) -> AppResult<PreparedStatus> {
    let tasks: Vec<_> = super::tasks::list(connection)?
        .into_iter()
        .filter(|t| {
            t.project_id.as_deref() == Some(id)
                && t.deleted_at.is_none()
                && ["todo", "doing"].contains(&t.status.as_str())
        })
        .collect();
    let mut files = Vec::new();
    if status != "active" && policy == "inbox" {
        for task in &tasks {
            files.push((
                task.id.clone(),
                super::attachments::copy_for_move(connection, &task.id, None)?,
            ));
        }
    }
    Ok(PreparedStatus { tasks, files })
}
fn apply_status(
    transaction: &rusqlite::Transaction<'_>,
    id: &str,
    status: &str,
    policy: &str,
    prepared: &PreparedStatus,
) -> AppResult<()> {
    if status != "active" {
        transaction.execute("UPDATE reminders SET state='dismissed' WHERE state='pending' AND task_id IN (SELECT id FROM tasks WHERE project_id=?1)",[id])?;
        for task in &prepared.tasks {
            match policy {
                "inbox" => {
                    transaction.execute(
                        "UPDATE tasks SET project_id=NULL,updated_at=?2 WHERE id=?1",
                        params![task.id, Utc::now().to_rfc3339()],
                    )?;
                }
                "complete" => {
                    transaction.execute(
                        "UPDATE tasks SET status='done',completed_at=?2,updated_at=?2 WHERE id=?1",
                        params![task.id, Utc::now().to_rfc3339()],
                    )?;
                }
                _ => {
                    transaction.execute(
                        "UPDATE tasks SET status='canceled',updated_at=?2 WHERE id=?1",
                        params![task.id, Utc::now().to_rfc3339()],
                    )?;
                }
            }
        }
        for (task, files) in &prepared.files {
            transaction.execute("DELETE FROM task_files WHERE task_id=?1", [task])?;
            for file in files {
                transaction.execute(
                    "INSERT INTO task_files(task_id,file_id) VALUES(?1,?2)",
                    params![task, file],
                )?;
            }
        }
    }
    transaction.execute(
        "UPDATE projects SET status=?2 WHERE id=?1",
        params![id, status],
    )?;
    Ok(())
}
pub fn set_status(
    connection: &mut Connection,
    id: &str,
    status: &str,
    policy: &str,
) -> AppResult<()> {
    validate_status(status, policy)?;
    if id == "__inbox__" {
        return Err(AppError::new("INVALID_STATUS", "项目状态无效"));
    }
    let exists: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM projects WHERE id=?1)",
        [id],
        |r| r.get(0),
    )?;
    if !exists {
        return Err(AppError::new("PROJECT_NOT_FOUND", "项目不存在"));
    }
    let prepared = prepare_status(connection, id, status, policy)?;
    let transaction = connection.transaction()?;
    apply_status(&transaction, id, status, policy, &prepared)?;
    transaction.commit()?;
    Ok(())
}

pub fn set_pinned(connection: &Connection, id: &str, pinned: bool) -> AppResult<()> {
    let changed = connection.execute(
        "UPDATE projects SET pinned=?2 WHERE id=?1 AND id!='__inbox__'",
        params![id, pinned],
    )?;
    if changed == 0 {
        return Err(AppError::new("PROJECT_NOT_FOUND", "项目不存在"));
    }
    Ok(())
}
