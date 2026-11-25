use crate::error::{AppError, AppResult};
use rusqlite::Connection;
use std::{path::Path, time::Duration};

pub const SCHEMA_VERSION: i64 = 7;

pub fn connect(path: &Path) -> AppResult<Connection> {
    let connection = Connection::open(path)?;
    connection.busy_timeout(Duration::from_secs(5))?;
    connection.pragma_update(None, "foreign_keys", "ON")?;
    Ok(connection)
}

pub fn initialize(path: &Path) -> AppResult<()> {
    let mut connection = connect(path)?;
    let version: i64 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
    if version > SCHEMA_VERSION {
        return Err(AppError::new(
            "SCHEMA_TOO_NEW",
            "数据库版本高于当前应用，请使用较新版本打开",
        ));
    }
    connection.pragma_update(None, "journal_mode", "WAL")?;
    if version == 0 {
        let transaction = connection.transaction()?;
        transaction.execute_batch(include_str!("../migrations/001_init.sql"))?;
        transaction.commit()?;
    }
    if version < 2 {
        let transaction = connection.transaction()?;
        transaction.execute_batch(include_str!("../migrations/002_project_files.sql"))?;
        transaction.commit()?;
    }
    if version < 3 {
        let transaction = connection.transaction()?;
        transaction.execute_batch(include_str!("../migrations/003_management.sql"))?;
        transaction.commit()?;
    }
    if version < 4 {
        let transaction = connection.transaction()?;
        transaction.execute_batch(include_str!("../migrations/004_project_links.sql"))?;
        transaction.commit()?;
    }
    if version < 5 {
        let transaction = connection.transaction()?;
        transaction.execute_batch(include_str!("../migrations/005_project_pins.sql"))?;
        transaction.commit()?;
    }
    if version < 6 {
        let transaction = connection.transaction()?;
        transaction.execute_batch(include_str!("../migrations/006_project_tags.sql"))?;
        transaction.commit()?;
    }
    if version < 7 {
        let transaction = connection.transaction()?;
        transaction.execute_batch(include_str!("../migrations/007_code_directories.sql"))?;
        transaction.commit()?;
    }
    Ok(())
}
