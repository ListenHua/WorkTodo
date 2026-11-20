use crate::{
    error::{AppError, AppResult},
    models::{CreateTask, Task, TrashAction, TrashFailure, TrashRequest, TrashResult, UpdateTask},
};
use chrono::{NaiveDate, Utc};
use rusqlite::{params, Connection};
use uuid::Uuid;

// Older workspaces used "none"; read and persist those tasks as low priority.
pub fn normalize_priority(priority: &str) -> &str {
    if priority == "none" {
        "low"
    } else {
        priority
    }
}

pub fn list(connection: &Connection) -> AppResult<Vec<Task>> {
    let mut statement = connection.prepare("SELECT id,project_id,title,status,priority,plan_date,created_at,description,due_at,deleted_at,completed_at FROM tasks ORDER BY CASE priority WHEN 'high' THEN 0 WHEN 'medium' THEN 1 ELSE 2 END,created_at DESC,id")?;
    let rows = statement.query_map([], |row| {
        Ok(Task {
            id: row.get(0)?,
            project_id: row.get(1)?,
            title: row.get(2)?,
            status: row.get(3)?,
            priority: normalize_priority(&row.get::<_, String>(4)?).into(),
            plan_date: row.get(5)?,
            created_at: row.get(6)?,
            description: row.get(7)?,
            due_at: row.get(8)?,
            deleted_at: row.get(9)?,
            completed_at: row.get(10)?,
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

pub fn create(connection: &Connection, input: CreateTask) -> AppResult<Task> {
    let title = input.title.trim().to_string();
    if title.is_empty() || title.chars().count() > 200 {
        return Err(AppError::new("VALIDATION_ERROR", "待办标题需为 1–200 字"));
    }
    if !["none", "low", "medium", "high"].contains(&input.priority.as_str()) {
        return Err(AppError::new("VALIDATION_ERROR", "优先级无效"));
    }
    if let Some(date) = input.plan_date.as_ref() {
        if date.len() != 10 || NaiveDate::parse_from_str(date, "%Y-%m-%d").is_err() {
            return Err(AppError::new("VALIDATION_ERROR", "计划日期无效"));
        }
    }
    if let Some(project_id) = input.project_id.as_ref() {
        let active: bool = connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM projects WHERE id=?1 AND status='active' AND id!='__inbox__')",
            [project_id],
            |row| row.get(0),
        )?;
        if !active {
            return Err(AppError::new("PROJECT_UNAVAILABLE", "项目不存在或已结束"));
        }
    }
    let task = Task {
        id: Uuid::new_v4().to_string(),
        project_id: input.project_id,
        title,
        status: "todo".into(),
        priority: normalize_priority(&input.priority).into(),
        plan_date: input.plan_date,
        description: String::new(),
        due_at: None,
        deleted_at: None,
        completed_at: None,
        created_at: Utc::now().to_rfc3339(),
    };
    connection.execute("INSERT INTO tasks(id,project_id,title,priority,plan_date,created_at) VALUES(?1,?2,?3,?4,?5,?6)", params![task.id,task.project_id,task.title,task.priority,task.plan_date,task.created_at])?;
    Ok(task)
}

pub fn set_completed(connection: &Connection, id: &str, completed: bool) -> AppResult<()> {
    if !completed {
        let unavailable:bool=connection.query_row("SELECT EXISTS(SELECT 1 FROM tasks t JOIN projects p ON p.id=t.project_id WHERE t.id=?1 AND p.status!='active')",[id],|r|r.get(0))?;
        if unavailable {
            return Err(AppError::new("PROJECT_ARCHIVED", "请先恢复项目再重开待办"));
        }
    }
    let transaction = connection.unchecked_transaction()?;
    let status = if completed { "done" } else { "todo" };
    let completed_at = completed.then(|| Utc::now().to_rfc3339());
    let changed = transaction.execute("UPDATE tasks SET status=?2,completed_at=CASE WHEN ?2='done' AND status='done' THEN completed_at ELSE ?3 END WHERE id=?1 AND deleted_at IS NULL AND status IN ('todo','doing','done')", params![id,status,completed_at])?;
    if changed == 0 {
        return Err(AppError::new("TASK_NOT_FOUND", "待办不存在或无法修改"));
    }
    if completed {
        transaction.execute(
            "UPDATE reminders SET state='dismissed' WHERE task_id=?1 AND state='pending'",
            [id],
        )?;
    }
    transaction.commit()?;
    Ok(())
}

pub fn update(connection: &mut Connection, id: &str, input: UpdateTask) -> AppResult<Task> {
    let old = list(connection)?
        .into_iter()
        .find(|t| t.id == id && t.deleted_at.is_none())
        .ok_or_else(|| AppError::new("TASK_NOT_FOUND", "待办不存在或已删除"))?;
    validate_edit(&input)?;
    let title = input.title.trim();
    if let Some(project) = &input.project_id {
        let active: bool = connection.query_row("SELECT EXISTS(SELECT 1 FROM projects WHERE id=?1 AND status='active' AND id!='__inbox__' AND id!='__inbox__')",[project],|r| r.get(0))?;
        if !active
            && (old.project_id != input.project_id
                || (["done", "canceled"].contains(&old.status.as_str())
                    && ["todo", "doing"].contains(&input.status.as_str())))
        {
            return Err(AppError::new("PROJECT_UNAVAILABLE", "请选择进行中的项目"));
        }
    }
    // Copy first; publish the task and all new references in one SQLite transaction.
    // Originals stay in their old project, including files shared by other tasks.
    let copied = if old.project_id != input.project_id {
        super::attachments::copy_for_move(connection, id, input.project_id.as_deref())?
    } else {
        Vec::new()
    };
    let completed_at = if input.status == "done" {
        if old.status == "done" {
            old.completed_at.clone()
        } else {
            Some(Utc::now().to_rfc3339())
        }
    } else {
        None
    };
    let transaction = connection.transaction()?;
    transaction.execute("UPDATE tasks SET title=?2,project_id=?3,priority=?4,plan_date=?5,description=?6,due_at=?7,status=?8,updated_at=?9,completed_at=?10 WHERE id=?1",
        params![id,title,input.project_id,normalize_priority(&input.priority),input.plan_date,input.description,input.due_at,input.status,Utc::now().to_rfc3339(),completed_at])?;
    if old.project_id != input.project_id {
        transaction.execute("DELETE FROM task_files WHERE task_id=?1", [id])?;
        for file in copied {
            transaction.execute(
                "INSERT INTO task_files(task_id,file_id) VALUES(?1,?2)",
                params![id, file],
            )?;
        }
    }
    if ["done", "canceled"].contains(&input.status.as_str()) {
        transaction.execute(
            "UPDATE reminders SET state='dismissed' WHERE task_id=?1 AND state='pending'",
            [id],
        )?;
    }
    if let Some(at) = input.reminder_at {
        super::reminders::schedule(&transaction, id, Some(&at))?;
    }
    transaction.commit()?;
    list(connection)?
        .into_iter()
        .find(|t| t.id == id)
        .ok_or_else(|| AppError::new("TASK_NOT_FOUND", "待办不存在"))
}
pub fn trash(connection: &Connection, id: &str, restore: bool) -> AppResult<()> {
    let transaction = connection.unchecked_transaction()?;
    let count = transaction.execute(
        "UPDATE tasks SET deleted_at=?2,updated_at=?3 WHERE id=?1",
        params![
            id,
            if restore {
                None
            } else {
                Some(Utc::now().to_rfc3339())
            },
            Utc::now().to_rfc3339()
        ],
    )?;
    if count == 0 {
        return Err(AppError::new("TASK_NOT_FOUND", "待办不存在"));
    }
    if !restore {
        transaction.execute(
            "UPDATE reminders SET state='dismissed' WHERE task_id=?1 AND state='pending'",
            [id],
        )?;
    } else {
        dismiss_past_reminders(&transaction, id)?;
    }
    transaction.commit()?;
    Ok(())
}

pub(crate) fn validate_edit(input: &UpdateTask) -> AppResult<()> {
    let title = input.title.trim();
    if title.is_empty()
        || title.chars().count() > 200
        || input.description.chars().count() > 10000
        || !["todo", "doing", "done", "canceled"].contains(&input.status.as_str())
        || !["none", "low", "medium", "high"].contains(&input.priority.as_str())
    {
        return Err(AppError::new(
            "VALIDATION_ERROR",
            "请检查待办内容、状态与优先级",
        ));
    }
    if let Some(date) = &input.plan_date {
        if date.len() != 10 || NaiveDate::parse_from_str(date, "%Y-%m-%d").is_err() {
            return Err(AppError::new("VALIDATION_ERROR", "计划日期无效"));
        }
    }
    if let Some(date) = &input.due_at {
        chrono::DateTime::parse_from_rfc3339(date)
            .map_err(|_| AppError::new("VALIDATION_ERROR", "截止时间无效"))?;
    }

    if let Some(at) = &input.reminder_at {
        let at = chrono::DateTime::parse_from_rfc3339(at)
            .map_err(|_| AppError::new("INVALID_DATE", "提醒时间无效"))?;
        if at <= Utc::now() || !["todo", "doing"].contains(&input.status.as_str()) {
            return Err(AppError::new(
                "INVALID_DATE",
                "请为未完成待办选择未来的提醒时间",
            ));
        }
    }
    Ok(())
}
pub fn create_full(connection: &mut Connection, input: UpdateTask) -> AppResult<Task> {
    validate_edit(&input)?;
    let transaction = connection.transaction()?;
    let task = create(
        &transaction,
        CreateTask {
            title: input.title.clone(),
            project_id: input.project_id.clone(),
            priority: input.priority.clone(),
            plan_date: input.plan_date.clone(),
        },
    )?;
    transaction.execute(
        "UPDATE tasks SET description=?2,due_at=?3,status=?4,completed_at=?5 WHERE id=?1",
        params![
            task.id,
            input.description,
            input.due_at,
            input.status,
            (input.status == "done").then(|| Utc::now().to_rfc3339())
        ],
    )?;
    if let Some(at) = input.reminder_at {
        super::reminders::schedule(&transaction, &task.id, Some(&at))?;
    }
    transaction.commit()?;
    list(connection)?
        .into_iter()
        .find(|t| t.id == task.id)
        .ok_or_else(|| AppError::new("TASK_NOT_FOUND", "待办不存在"))
}

pub fn restore(connection: &mut Connection, id: &str, to_inbox: bool) -> AppResult<()> {
    let task = list(connection)?
        .into_iter()
        .find(|t| t.id == id && t.deleted_at.is_some())
        .ok_or_else(|| AppError::new("TASK_NOT_FOUND", "回收站中没有此待办"))?;
    let missing: bool = connection.query_row(
        "SELECT project_id IS NOT NULL AND NOT EXISTS(
            SELECT 1 FROM projects WHERE id=tasks.project_id) FROM tasks WHERE id=?1",
        [id],
        |row| row.get(0),
    )?;
    if missing {
        let transaction = connection.transaction()?;
        transaction.execute("DELETE FROM task_files WHERE task_id=?1", [id])?;
        transaction.execute(
            "UPDATE tasks SET deleted_at=NULL,project_id=NULL,updated_at=?2 WHERE id=?1",
            params![id, Utc::now().to_rfc3339()],
        )?;
        dismiss_past_reminders(&transaction, id)?;
        transaction.commit()?;
        return Ok(());
    }
    let archived = if let Some(project) = &task.project_id {
        connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM projects WHERE id=?1 AND status!='active')",
            [project],
            |r| r.get::<_, bool>(0),
        )?
    } else {
        false
    };
    if archived && ["todo", "doing"].contains(&task.status.as_str()) && !to_inbox {
        return Err(AppError::new(
            "PROJECT_ARCHIVED",
            "原项目已归档，请选择恢复为无项目或先恢复项目",
        ));
    }
    if !to_inbox {
        return trash(connection, id, true);
    }
    let copied = super::attachments::copy_for_move(connection, id, None)?;
    let transaction = connection.transaction()?;
    transaction.execute(
        "UPDATE tasks SET deleted_at=NULL,project_id=NULL,updated_at=?2 WHERE id=?1",
        params![id, Utc::now().to_rfc3339()],
    )?;
    transaction.execute("DELETE FROM task_files WHERE task_id=?1", [id])?;
    for file in copied {
        transaction.execute(
            "INSERT INTO task_files(task_id,file_id) VALUES(?1,?2)",
            params![id, file],
        )?;
    }
    dismiss_past_reminders(&transaction, id)?;
    transaction.commit()?;
    Ok(())
}

fn dismiss_past_reminders(connection: &Connection, id: &str) -> AppResult<()> {
    connection.execute(
        "UPDATE reminders SET state='dismissed' WHERE task_id=?1 AND state='pending'
         AND julianday(remind_at)<=julianday('now')",
        [id],
    )?;
    Ok(())
}

/// Manage only the explicit trash snapshot confirmed by the user. Files remain untouched on purge.
pub fn manage_trash(connection: &mut Connection, input: TrashRequest) -> AppResult<TrashResult> {
    let mut seen = std::collections::HashSet::new();
    let mut result = TrashResult::default();
    let mut targets = Vec::new();
    for id in input.ids {
        if !seen.insert(id.clone()) {
            continue;
        }
        let deleted: bool = connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM tasks WHERE id=?1 AND deleted_at IS NOT NULL)",
            [&id],
            |row| row.get(0),
        )?;
        if deleted {
            targets.push(id);
        } else {
            result.failed.push(TrashFailure {
                id,
                message: "待办已恢复或不在回收站中".into(),
            });
        }
    }
    match input.action {
        TrashAction::Purge => {
            let transaction = connection.transaction()?;
            for id in &targets {
                transaction.execute("DELETE FROM task_files WHERE task_id=?1", [id])?;
                transaction.execute("DELETE FROM reminders WHERE task_id=?1", [id])?;
                transaction.execute(
                    "DELETE FROM tasks WHERE id=?1 AND deleted_at IS NOT NULL",
                    [id],
                )?;
            }
            transaction.commit()?;
            result.succeeded_ids = targets;
        }
        TrashAction::Restore => {
            for id in targets {
                let rehome: bool = connection.query_row(
                    "SELECT project_id IS NOT NULL AND
                     (NOT EXISTS(SELECT 1 FROM projects WHERE id=tasks.project_id) OR
                      (status IN ('todo','doing') AND NOT EXISTS(
                        SELECT 1 FROM projects WHERE id=tasks.project_id AND status='active')))
                     FROM tasks WHERE id=?1",
                    [&id],
                    |row| row.get(0),
                )?;
                let missing: bool = connection.query_row(
                    "SELECT project_id IS NOT NULL AND NOT EXISTS(
                       SELECT 1 FROM projects WHERE id=tasks.project_id) FROM tasks WHERE id=?1",
                    [&id],
                    |row| row.get(0),
                )?;
                match restore(
                    connection,
                    &id,
                    missing || (rehome && input.restore_to_inbox),
                ) {
                    Ok(()) => result.succeeded_ids.push(id),
                    Err(error) => result.failed.push(TrashFailure {
                        id,
                        message: error.message,
                    }),
                }
            }
        }
    }
    Ok(result)
}
