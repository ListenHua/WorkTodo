use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs,
    io::Read,
    path::{Component, Path},
    process::{Command, Stdio},
    sync::atomic::{AtomicBool, Ordering},
    thread,
    time::{Duration, Instant},
};

#[derive(Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitInfo {
    pub state: String,
    pub message: String,
    pub branch: String,
    pub scope: String,
    pub upstream: String,
    pub ahead: u64,
    pub behind: u64,
    pub changes: Vec<GitChange>,
    pub total_changes: usize,
    pub conflicts: usize,
    pub truncated: bool,
    pub remotes: Vec<GitRemote>,
    pub commit: Option<GitCommit>,
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitChange {
    pub path: String,
    pub original_path: Option<String>,
    pub index: String,
    pub worktree: String,
    pub conflict: bool,
}
#[derive(Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitRemote {
    pub name: String,
    pub fetch_url: String,
    pub push_url: String,
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitCommit {
    pub hash: String,
    pub subject: String,
    pub time: String,
}
struct GitOutput {
    success: bool,
    stdout: Vec<u8>,
    stderr: String,
}
fn git(folder: &Path, args: &[&str]) -> AppResult<GitOutput> {
    let mut command = Command::new("git");
    command
        .args(["--no-optional-locks", "-c", "core.fsmonitor=false", "-C"])
        .arg(folder)
        .args(args)
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_OPTIONAL_LOCKS", "0")
        .env("LC_ALL", "C")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    let mut child = command
        .spawn()
        .map_err(|_| AppError::new("GIT_UNAVAILABLE", "Git 不可用，请安装 Git 后重试"))?;
    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();
    let reader = thread::spawn(move || {
        let mut bytes = Vec::new();
        let _ = stdout.take(8 * 1024 * 1024 + 1).read_to_end(&mut bytes);
        bytes
    });
    let error_reader = thread::spawn(move || {
        let mut bytes = Vec::new();
        let _ = stderr.take(16 * 1024).read_to_end(&mut bytes);
        String::from_utf8_lossy(&bytes).into_owned()
    });
    let started = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Ok(status),
            Ok(None) if started.elapsed() < Duration::from_secs(10) => {
                thread::sleep(Duration::from_millis(20));
            }
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                break Err(AppError::new("GIT_TIMEOUT", "Git 状态读取超时，请稍后刷新"));
            }
        }
    };
    let stdout = reader.join().unwrap_or_default();
    let stderr = error_reader.join().unwrap_or_default();
    let status = status?;
    if stdout.len() > 8 * 1024 * 1024 {
        return Err(AppError::new(
            "GIT_OUTPUT_LIMIT",
            "Git 变动过多，请在 Git 客户端查看",
        ));
    }
    Ok(GitOutput {
        success: status.success(),
        stdout,
        stderr,
    })
}
fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes)
        .trim_end_matches(['\r', '\n'])
        .into()
}
fn safe_remote(raw: &str) -> String {
    let raw = raw.trim();
    if let Some((scheme, rest)) = raw.split_once("://") {
        let authority_end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
        let (authority, path) = rest.split_at(authority_end);
        let authority = match authority.rsplit_once('@') {
            Some((user, host)) if scheme.eq_ignore_ascii_case("ssh") => {
                format!("{}@{host}", user.split(':').next().unwrap_or(user))
            }
            Some((_, host)) => host.to_owned(),
            None => authority.to_owned(),
        };
        let path = path.split(['?', '#']).next().unwrap_or(path);
        return format!("{scheme}://{authority}{path}");
    }
    raw.into()
}
pub fn inspect(folder: &Path) -> AppResult<GitInfo> {
    let root = match git(folder, &["rev-parse", "--show-toplevel"]) {
        Ok(output) if output.success => text(&output.stdout),
        Ok(output) => {
            if !output.stderr.contains("not a git repository")
                && !output.stderr.contains("developer")
                && !output.stderr.contains("xcrun")
            {
                return Err(AppError::new(
                    "GIT_ACCESS_FAILED",
                    "无法读取 Git 工作区，请检查仓库权限后刷新",
                ));
            }
            let unavailable =
                output.stderr.contains("developer") || output.stderr.contains("xcrun");
            return Ok(GitInfo {
                state: if unavailable { "unavailable" } else { "none" }.into(),
                message: if unavailable {
                    "Git 不可用，请安装 Git 后重试"
                } else {
                    "当前项目不是 Git 工作区"
                }
                .into(),
                ..Default::default()
            });
        }
        Err(error) if error.code == "GIT_UNAVAILABLE" => {
            return Ok(GitInfo {
                state: "unavailable".into(),
                message: error.message,
                ..Default::default()
            })
        }
        Err(error) => return Err(error),
    };
    let status = git(
        folder,
        &[
            "status",
            "--porcelain=v2",
            "--branch",
            "--untracked-files=all",
            "-z",
            "--",
            ".",
        ],
    )?;
    if !status.success {
        return Err(AppError::new(
            "GIT_STATUS_FAILED",
            "无法读取 Git 状态，请检查仓库后刷新",
        ));
    }
    let repository = Path::new(&root).canonicalize()?;
    let scope = folder.strip_prefix(&repository).unwrap_or(Path::new(""));
    let mut info = GitInfo {
        state: "ready".into(),
        scope: scope.to_string_lossy().replace('\\', "/"),
        ..Default::default()
    };
    let mut records = status.stdout.split(|byte| *byte == 0);
    while let Some(record) = records.next() {
        let line = String::from_utf8_lossy(record);
        if let Some(value) = line.strip_prefix("# branch.head ") {
            info.branch = if value == "(detached)" {
                "游离 HEAD".into()
            } else {
                value.into()
            };
            continue;
        }
        if let Some(value) = line.strip_prefix("# branch.upstream ") {
            info.upstream = value.into();
            continue;
        }
        if let Some(value) = line.strip_prefix("# branch.ab ") {
            let mut parts = value.split_whitespace();
            info.ahead = parts
                .next()
                .unwrap_or("+0")
                .trim_start_matches('+')
                .parse()
                .unwrap_or(0);
            info.behind = parts
                .next()
                .unwrap_or("-0")
                .trim_start_matches('-')
                .parse()
                .unwrap_or(0);
            continue;
        }
        let kind = line.as_bytes().first().copied();
        let fields = match kind {
            Some(b'1') => 9,
            Some(b'2') => 10,
            Some(b'u') => 11,
            Some(b'?') => 2,
            _ => continue,
        };
        let parts: Vec<_> = line.splitn(fields, ' ').collect();
        if parts.len() != fields {
            continue;
        }
        let original = if kind == Some(b'2') {
            records.next().map(text)
        } else {
            None
        };
        info.total_changes += 1;
        if kind == Some(b'u') {
            info.conflicts += 1;
        }
        if info.changes.len() >= 500 {
            info.truncated = true;
            continue;
        }
        let relative = |path: &str| {
            Path::new(path)
                .strip_prefix(scope)
                .unwrap_or(Path::new(path))
                .to_string_lossy()
                .replace('\\', "/")
        };
        let xy = if kind == Some(b'?') { "??" } else { parts[1] };
        info.changes.push(GitChange {
            path: relative(parts[fields - 1]),
            original_path: original.map(|p| relative(&p)),
            index: xy.chars().next().unwrap_or('.').to_string(),
            worktree: xy.chars().nth(1).unwrap_or('.').to_string(),
            conflict: kind == Some(b'u'),
        });
    }
    if let Ok(output) = git(
        folder,
        &[
            "config",
            "--null",
            "--get-regexp",
            "^remote\\..*\\.(url|pushurl)$",
        ],
    ) {
        let mut remotes: BTreeMap<String, GitRemote> = BTreeMap::new();
        for record in output.stdout.split(|b| *b == 0) {
            let line = String::from_utf8_lossy(record);
            let Some((key, value)) = line.split_once('\n') else {
                continue;
            };
            let Some(key) = key.strip_prefix("remote.") else {
                continue;
            };
            let (name, push) = if let Some(name) = key.strip_suffix(".pushurl") {
                (name, true)
            } else if let Some(name) = key.strip_suffix(".url") {
                (name, false)
            } else {
                continue;
            };
            if remotes.len() >= 20 && !remotes.contains_key(name) {
                continue;
            }
            let remote = remotes.entry(name.into()).or_insert_with(|| GitRemote {
                name: name.into(),
                ..Default::default()
            });
            let value = safe_remote(value);
            if value.len() > 2048 || value.chars().any(char::is_control) {
                continue;
            }
            if push && remote.push_url.is_empty() {
                remote.push_url = value;
            } else if !push && remote.fetch_url.is_empty() {
                remote.fetch_url = value;
            }
        }
        info.remotes = remotes
            .into_values()
            .map(|mut remote| {
                if remote.push_url.is_empty() {
                    remote.push_url = remote.fetch_url.clone();
                }
                remote
            })
            .collect();
        info.remotes
            .sort_by_key(|remote| (remote.name != "origin", remote.name.clone()));
    }
    if let Ok(output) = git(folder, &["log", "-1", "--format=%h%x00%s%x00%cI"]) {
        if output.success {
            let values: Vec<_> = output.stdout.split(|b| *b == 0).map(text).collect();
            if values.len() == 3 {
                info.commit = Some(GitCommit {
                    hash: values[0].clone(),
                    subject: values[1].clone(),
                    time: values[2].clone(),
                });
            }
        }
    }
    Ok(info)
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Dependency {
    pub path: String,
    pub bytes: u64,
    pub fingerprint: String,
}
#[derive(Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpaceUsage {
    pub bytes: u64,
    pub dependency_bytes: u64,
    pub files: u64,
    pub dependencies: Vec<Dependency>,
    pub directories: Vec<DirectorySize>,
    pub warnings: Vec<String>,
    pub scanned_at: String,
}
#[derive(Serialize)]
pub struct DirectorySize {
    pub name: String,
    pub bytes: u64,
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodeProgress {
    pub files: u64,
    pub bytes: u64,
    pub current: String,
}
fn fingerprint(metadata: &fs::Metadata) -> String {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        format!("{}:{}", metadata.dev(), metadata.ino())
    }
    #[cfg(not(unix))]
    {
        format!("{:?}", metadata.created().ok())
    }
}
struct Scanner<'a, F> {
    root: &'a Path,
    result: SpaceUsage,
    canceled: &'a AtomicBool,
    progress: F,
    last: Instant,
}
impl<F: FnMut(CodeProgress)> Scanner<'_, F> {
    fn walk(
        &mut self,
        path: &Path,
        dependency: bool,
        git_data: bool,
        depth: usize,
    ) -> AppResult<u64> {
        if self.canceled.load(Ordering::Relaxed) {
            return Err(AppError::new("CANCELED", "已取消体积统计"));
        }
        if depth > 128 || self.result.files >= 2_000_000 {
            return Err(AppError::new(
                "SCAN_LIMIT",
                "目录层级或文件数量过多，请缩小项目范围",
            ));
        }
        let meta = match fs::symlink_metadata(path) {
            Ok(meta) => meta,
            Err(_) => {
                self.warning(path);
                return Ok(0);
            }
        };
        if super::files::is_link(&meta) {
            return Ok(0);
        }
        if meta.is_file() {
            self.result.files += 1;
            self.result.bytes += meta.len();
            if self.last.elapsed() > Duration::from_millis(150) {
                let current = self.relative(path);
                (self.progress)(CodeProgress {
                    files: self.result.files,
                    bytes: self.result.bytes,
                    current,
                });
                self.last = Instant::now();
            }
            return Ok(meta.len());
        }
        if !meta.is_dir() {
            return Ok(0);
        }
        let is_git = git_data || path.file_name().is_some_and(|n| n == ".git");
        let is_dependency = !is_git && path.file_name().is_some_and(|n| n == "node_modules");
        let start = self.result.dependencies.len();
        if is_dependency && !dependency {
            self.result.dependencies.push(Dependency {
                path: self.relative(path),
                bytes: 0,
                fingerprint: fingerprint(&meta),
            });
        }
        let entries = match fs::read_dir(path) {
            Ok(entries) => entries,
            Err(_) => {
                self.warning(path);
                return Ok(0);
            }
        };
        let mut bytes = 0;
        for entry in entries {
            match entry {
                Ok(entry) => {
                    bytes += self.walk(
                        &entry.path(),
                        dependency || is_dependency,
                        is_git,
                        depth + 1,
                    )?;
                }
                Err(_) => self.warning(path),
            }
        }
        if is_dependency && !dependency {
            self.result.dependencies[start].bytes = bytes;
            self.result.dependency_bytes += bytes;
        }
        Ok(bytes)
    }
    fn relative(&self, path: &Path) -> String {
        path.strip_prefix(self.root)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/")
    }
    fn warning(&mut self, path: &Path) {
        if self.result.warnings.len() < 8 {
            self.result
                .warnings
                .push(format!("无法完整统计 {}", self.relative(path)));
        }
    }
}
pub fn scan(
    folder: &Path,
    canceled: &AtomicBool,
    progress: impl FnMut(CodeProgress),
) -> AppResult<SpaceUsage> {
    if canceled.load(Ordering::Relaxed) {
        return Err(AppError::new("CANCELED", "已取消体积统计"));
    }
    let mut scanner = Scanner {
        root: folder,
        canceled,
        progress,
        result: SpaceUsage::default(),
        last: Instant::now(),
    };
    for entry in fs::read_dir(folder)? {
        let entry = entry?;
        let bytes = scanner.walk(&entry.path(), false, false, 0)?;
        scanner.result.directories.push(DirectorySize {
            name: entry.file_name().to_string_lossy().into_owned(),
            bytes,
        });
    }
    scanner
        .result
        .directories
        .sort_by_key(|item| std::cmp::Reverse(item.bytes));
    scanner.result.directories.truncate(8);
    scanner.result.scanned_at = chrono::Utc::now().to_rfc3339();
    Ok(scanner.result)
}
#[derive(Serialize)]
pub struct CleanFailure {
    pub path: String,
    pub message: String,
}
#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CleanResult {
    pub succeeded: Vec<String>,
    pub failed: Vec<CleanFailure>,
}
pub fn clean(
    folder: &Path,
    targets: &[Dependency],
    mut progress: impl FnMut(CodeProgress),
) -> AppResult<CleanResult> {
    if targets.is_empty() || targets.len() > 100 {
        return Err(AppError::new(
            "INVALID_TARGETS",
            "请选择需要清理的依赖目录（最多 100 个）",
        ));
    }
    let mut result = CleanResult::default();
    for (index, target) in targets.iter().enumerate() {
        progress(CodeProgress {
            files: index as u64,
            bytes: 0,
            current: target.path.clone(),
        });
        let attempt = || -> AppResult<()> {
            let relative = Path::new(&target.path);
            if relative.is_absolute()
                || relative
                    .file_name()
                    .is_none_or(|name| name != "node_modules")
                || !relative
                    .components()
                    .all(|c| matches!(c, Component::Normal(name) if name != ".git"))
            {
                return Err(AppError::new(
                    "INVALID_TARGET",
                    "只允许清理项目内的 node_modules 目录",
                ));
            }
            let mut path = folder.to_path_buf();
            for component in relative.components() {
                path.push(component);
                let meta = fs::symlink_metadata(&path)?;
                if super::files::is_link(&meta) || !meta.is_dir() {
                    return Err(AppError::new(
                        "DIRECTORY_CHANGED",
                        "目录已变化或包含链接，请重新统计",
                    ));
                }
            }
            let meta = fs::symlink_metadata(&path)?;
            if fingerprint(&meta) != target.fingerprint || !path.canonicalize()?.starts_with(folder)
            {
                return Err(AppError::new(
                    "DIRECTORY_CHANGED",
                    "依赖目录已变化，请重新统计后清理",
                ));
            }
            match git(folder, &["ls-files", "-z", "--", &target.path]) {
                Ok(output) if output.success && !output.stdout.is_empty() => {
                    return Err(AppError::new(
                        "TRACKED_DEPENDENCY",
                        "目录含受 Git 跟踪的文件，已保留，请在 Git 客户端处理",
                    ))
                }
                Ok(output)
                    if !output.success && !output.stderr.contains("not a git repository") =>
                {
                    return Err(AppError::new(
                        "GIT_CHECK_FAILED",
                        "无法检查依赖目录，请检查 Git 仓库后重试",
                    ))
                }
                Err(error) if error.code == "GIT_UNAVAILABLE" => {
                    let repository_possible = folder.ancestors().any(|parent| {
                        !matches!(fs::symlink_metadata(parent.join(".git")),
                            Err(error) if error.kind() == std::io::ErrorKind::NotFound)
                    });
                    if repository_possible {
                        return Err(AppError::new(
                            "GIT_UNAVAILABLE",
                            "Git 不可用，无法确认依赖是否受版本管理，请安装 Git 后重试",
                        ));
                    }
                }
                Err(error) => return Err(error),
                _ => {}
            }
            // Git checks can take time; confirm the same directory is still selected.
            let mut verified = folder.to_path_buf();
            for component in relative.components() {
                verified.push(component);
                let meta = fs::symlink_metadata(&verified)?;
                if super::files::is_link(&meta) || !meta.is_dir() {
                    return Err(AppError::new("DIRECTORY_CHANGED", "目录已变化，请重新统计"));
                }
            }
            if fingerprint(&fs::symlink_metadata(&verified)?) != target.fingerprint
                || !verified.canonicalize()?.starts_with(folder)
            {
                return Err(AppError::new(
                    "DIRECTORY_CHANGED",
                    "依赖目录已变化，请重新统计",
                ));
            }
            fs::remove_dir_all(&verified)?;
            Ok(())
        };
        match attempt() {
            Ok(()) => result.succeeded.push(target.path.clone()),
            Err(error) => result.failed.push(CleanFailure {
                path: target.path.clone(),
                message: error.message,
            }),
        }
    }
    progress(CodeProgress {
        files: targets.len() as u64,
        bytes: 0,
        current: String::new(),
    });
    Ok(result)
}
