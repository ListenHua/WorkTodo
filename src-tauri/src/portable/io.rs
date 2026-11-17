use crate::error::{AppError, AppResult};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File},
    io::{Read, Write},
    path::{Component, Path, PathBuf},
};

pub fn invalid(message: &str) -> AppError {
    AppError::new("INVALID_WORKSPACE", message)
}
pub fn relative(value: &str) -> AppResult<()> {
    if value.is_empty()
        || value.contains('\\')
        || value.contains(':')
        || value
            .split('/')
            .any(|p| p.is_empty() || p == "." || p == "..")
        || Path::new(value)
            .components()
            .any(|c| !matches!(c, Component::Normal(_)))
    {
        return Err(invalid("数据包含越界或无效的相对路径"));
    }
    Ok(())
}
pub fn path(root: &Path, value: &str) -> AppResult<PathBuf> {
    relative(value)?;
    let mut result = root.to_path_buf();
    for part in value.split('/') {
        result.push(part);
        match fs::symlink_metadata(&result) {
            Ok(meta) if crate::services::files::is_link(&meta) => {
                return Err(invalid("工作目录包含链接，请先处理链接"))
            }
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(e.into()),
            _ => {}
        }
    }
    Ok(result)
}
pub fn bytes(value: &Value) -> AppResult<Vec<u8>> {
    let mut result = serde_json::to_vec_pretty(value).map_err(|_| invalid("无法编码本地数据"))?;
    result.push(b'\n');
    Ok(result)
}
pub fn read(file: &Path) -> AppResult<Value> {
    let meta = fs::symlink_metadata(file)?;
    if crate::services::files::is_link(&meta) || !meta.is_file() || meta.len() > 64 * 1024 * 1024 {
        return Err(invalid("配置文件类型或大小无效"));
    }
    serde_json::from_reader(File::open(file)?)
        .map_err(|_| invalid("JSON 损坏，请从备份恢复；未覆盖原文件"))
}
pub fn atomic(file: &Path, data: &[u8]) -> AppResult<()> {
    let parent = file.parent().ok_or_else(|| invalid("保存位置无效"))?;
    fs::create_dir_all(parent)?;
    if fs::symlink_metadata(file).is_ok_and(|m| crate::services::files::is_link(&m)) {
        return Err(invalid("配置不能是链接"));
    }
    let mut temp = tempfile::Builder::new()
        .prefix(".workspace-write-")
        .tempfile_in(parent)?;
    temp.write_all(data)?;
    temp.as_file().sync_all()?;
    temp.persist(file).map_err(|e| AppError::from(e.error))?;
    #[cfg(unix)]
    File::open(parent)?.sync_all()?;
    Ok(())
}
pub fn write(file: &Path, value: &Value) -> AppResult<()> {
    atomic(file, &bytes(value)?)
}
pub fn digest(data: &[u8]) -> String {
    format!("{:x}", Sha256::digest(data))
}
pub fn hash(file: &Path) -> AppResult<String> {
    let mut input = File::open(file)?;
    let before = input.metadata()?;
    let mut digest = Sha256::new();
    let mut buffer = [0; 65536];
    loop {
        let n = input.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        digest.update(&buffer[..n]);
    }
    let after = input.metadata()?;
    if before.len() != after.len() || before.modified()? != after.modified()? {
        return Err(AppError::new("SOURCE_CHANGED", "文件正在变化，请稍后重试"));
    }
    Ok(format!("{:x}", digest.finalize()))
}
pub fn fingerprint(file: &Path) -> AppResult<String> {
    let m = fs::symlink_metadata(file)?;
    if crate::services::files::is_link(&m) {
        return Err(invalid("不允许链接"));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        Ok(format!("{}:{}", m.dev(), m.ino()))
    }
    #[cfg(not(unix))]
    {
        Ok(format!("{:?}:{}", m.created()?, m.is_dir()))
    }
}
pub fn lock(root: &Path) -> AppResult<File> {
    let directory = path(root, ".workspace")?;
    if !root.is_dir() {
        return Err(invalid("工作目录离线，未创建替代目录"));
    }
    if !directory.exists() {
        fs::create_dir(&directory)?;
    }
    let file = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(path(root, ".workspace/write.lock")?)?;
    file.try_lock()
        .map_err(|_| AppError::new("WORKSPACE_BUSY", "其他进程正在使用此目录，请稍后重试"))?;
    Ok(file)
}
