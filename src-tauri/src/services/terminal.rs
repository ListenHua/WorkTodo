use crate::error::{AppError, AppResult};
use std::path::Path;

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn command(folder: &Path) -> std::process::Command {
    #[cfg(target_os = "macos")]
    {
        let mut command = std::process::Command::new("/usr/bin/open");
        command.args(["-b", "com.apple.Terminal"]).arg(folder);
        command
    }
    #[cfg(target_os = "windows")]
    {
        use std::{os::windows::process::CommandExt, path::PathBuf};
        let system_root = std::env::var_os("SystemRoot")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(r"C:\Windows"));
        let mut command = std::process::Command::new(
            system_root.join("System32/WindowsPowerShell/v1.0/powershell.exe"),
        );
        // Pass the folder as the process working directory, never as shell code.
        command
            .args(["-NoLogo", "-NoExit"])
            .current_dir(folder)
            .creation_flags(0x00000010); // CREATE_NEW_CONSOLE
        command
    }
}

pub fn open(folder: &Path) -> AppResult<()> {
    let failed = || {
        AppError::new(
            "OPEN_TERMINAL_FAILED",
            "无法打开系统终端，请检查终端是否可用",
        )
    };
    #[cfg(target_os = "macos")]
    {
        let status = command(folder).status().map_err(|_| failed())?;
        if status.success() {
            Ok(())
        } else {
            Err(failed())
        }
    }
    #[cfg(target_os = "windows")]
    {
        let mut child = command(folder).spawn().map_err(|_| failed())?;
        std::thread::spawn(move || {
            let _ = child.wait();
        });
        Ok(())
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = (folder, failed);
        Err(AppError::new(
            "UNSUPPORTED_PLATFORM",
            "当前平台暂不支持打开终端",
        ))
    }
}
