use serde::Serialize;

pub type AppResult<T> = Result<T, AppError>;

#[derive(Debug, Serialize)]
pub struct AppError {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

impl AppError {
    pub fn new(code: &str, message: &str) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            details: None,
        }
    }
    pub fn with_details(mut self, details: serde_json::Value) -> Self {
        self.details = Some(details);
        self
    }
}
impl From<rusqlite::Error> for AppError {
    fn from(error: rusqlite::Error) -> Self {
        eprintln!("Database operation failed: {error}");
        Self::new("DATABASE_ERROR", "本地数据库操作失败，请重试")
    }
}
impl From<std::io::Error> for AppError {
    fn from(error: std::io::Error) -> Self {
        let message = match error.kind() {
            std::io::ErrorKind::PermissionDenied => "没有目录访问权限，请检查存储位置",
            std::io::ErrorKind::AlreadyExists => "同名文件或文件夹已存在，请使用其他名称",
            std::io::ErrorKind::NotFound => "文件或目录不存在，请检查存储位置",
            _ => "本地文件操作失败，请检查磁盘空间和目录权限",
        };
        Self::new("FILESYSTEM_ERROR", message)
    }
}
