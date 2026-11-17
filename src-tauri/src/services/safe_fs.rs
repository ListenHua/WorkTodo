use crate::error::AppResult;
use std::{fs, path::Path};
// Never replace a destination created between validation and publication.
pub fn rename_exclusive(source: &Path, target: &Path) -> AppResult<()> {
    crate::portable::journal::rename(source, target)?;
    #[cfg(target_os = "macos")]
    {
        use std::{ffi::CString, os::unix::ffi::OsStrExt};
        let from = CString::new(source.as_os_str().as_bytes())
            .map_err(|_| std::io::Error::from(std::io::ErrorKind::InvalidInput))?;
        let to = CString::new(target.as_os_str().as_bytes())
            .map_err(|_| std::io::Error::from(std::io::ErrorKind::InvalidInput))?;
        if unsafe { libc::renamex_np(from.as_ptr(), to.as_ptr(), libc::RENAME_EXCL) } != 0 {
            return Err(std::io::Error::last_os_error().into());
        }
    }
    #[cfg(target_os = "linux")]
    {
        use std::{ffi::CString, os::unix::ffi::OsStrExt};
        let from = CString::new(source.as_os_str().as_bytes())
            .map_err(|_| std::io::Error::from(std::io::ErrorKind::InvalidInput))?;
        let to = CString::new(target.as_os_str().as_bytes())
            .map_err(|_| std::io::Error::from(std::io::ErrorKind::InvalidInput))?;
        if unsafe {
            libc::renameat2(
                libc::AT_FDCWD,
                from.as_ptr(),
                libc::AT_FDCWD,
                to.as_ptr(),
                libc::RENAME_NOREPLACE,
            )
        } != 0
        {
            return Err(std::io::Error::last_os_error().into());
        }
    }
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        #[link(name = "kernel32")]
        unsafe extern "system" {
            #[link_name = "MoveFileExW"]
            fn move_file_ex_w(existing: *const u16, new: *const u16, flags: u32) -> i32;
        }
        let wide = |path: &Path| -> std::io::Result<Vec<u16>> {
            let mut encoded = path.as_os_str().encode_wide().collect::<Vec<_>>();
            if encoded.contains(&0) {
                return Err(std::io::ErrorKind::InvalidInput.into());
            }
            encoded.push(0);
            Ok(encoded)
        };
        let from = wide(source)?;
        let to = wide(target)?;
        // WRITE_THROUGH, deliberately without REPLACE_EXISTING or COPY_ALLOWED.
        if unsafe { move_file_ex_w(from.as_ptr(), to.as_ptr(), 0x8) } == 0 {
            return Err(std::io::Error::last_os_error().into());
        }
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
    {
        if target.exists() {
            return Err(std::io::Error::from(std::io::ErrorKind::AlreadyExists).into());
        }
        fs::rename(source, target)?;
    }
    Ok(())
}
pub fn copy_tree(source: &Path, target: &Path) -> AppResult<()> {
    copy_level(source, target, 0, &mut 0)
}
fn copy_level(source: &Path, target: &Path, depth: usize, count: &mut usize) -> AppResult<()> {
    if depth > 64 || *count > 100_000 {
        return Err(crate::error::AppError::new(
            "COPY_LIMIT",
            "目录内容过多或层级过深",
        ));
    }
    if super::files::is_link(&fs::symlink_metadata(source)?) {
        return Err(crate::error::AppError::new(
            "UNSAFE_DIRECTORY",
            "目录包含链接，未复制，请先处理链接",
        ));
    }
    fs::create_dir(target)?;
    for entry in fs::read_dir(source)? {
        *count += 1;
        let entry = entry?;
        let meta = fs::symlink_metadata(entry.path())?;
        if super::files::is_link(&meta) {
            return Err(crate::error::AppError::new(
                "UNSAFE_FILE",
                "备份或迁移目录包含链接，请先移除链接",
            ));
        }
        if meta.is_dir() {
            copy_level(
                &entry.path(),
                &target.join(entry.file_name()),
                depth + 1,
                count,
            )?;
        } else if meta.is_file() {
            let mut from = fs::File::open(entry.path())?;
            let mut to = fs::OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(target.join(entry.file_name()))?;
            std::io::copy(&mut from, &mut to)?;
            to.sync_all()?;
            let after = from.metadata()?;
            if meta.len() != after.len() || meta.modified()? != after.modified()? {
                return Err(crate::error::AppError::new(
                    "SOURCE_CHANGED",
                    "复制期间文件发生变化，请重试",
                ));
            }
        } else {
            return Err(crate::error::AppError::new(
                "UNSAFE_FILE",
                "目录包含非常规文件，无法完整复制",
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod exclusive_tests {
    use super::*;
    #[test]
    fn rename_never_replaces_an_existing_file_or_directory() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source");
        let target = root.path().join("target");
        fs::write(&source, b"source bytes").unwrap();
        fs::write(&target, b"target bytes").unwrap();
        assert!(rename_exclusive(&source, &target).is_err());
        assert_eq!(fs::read(&source).unwrap(), b"source bytes");
        assert_eq!(fs::read(&target).unwrap(), b"target bytes");
        let a = root.path().join("a");
        let b = root.path().join("b");
        fs::create_dir(&a).unwrap();
        fs::create_dir(&b).unwrap();
        assert!(rename_exclusive(&a, &b).is_err());
        assert!(a.is_dir() && b.is_dir());
    }
}
