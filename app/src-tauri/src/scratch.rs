//! The only place the app writes on its own: a session folder in the OS
//! temp directory, deleted on exit and swept again on the next launch.

use std::path::{Path, PathBuf};

const PREFIX: &str = "schedule-spotter-session-";

/// Creates this session's scratch folder after removing any left behind by
/// earlier sessions (a crash, or WebView files still locked at exit).
pub fn create() -> std::io::Result<PathBuf> {
    let temp = std::env::temp_dir();
    sweep(&temp);
    let dir = temp.join(format!("{PREFIX}{}", std::process::id()));
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

fn sweep(temp: &Path) {
    let Ok(entries) = std::fs::read_dir(temp) else {
        return;
    };
    let mine = format!("{PREFIX}{}", std::process::id());
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with(PREFIX) && name != mine {
            let _ = std::fs::remove_dir_all(entry.path());
        }
    }
}

/// Removes this session's folder on exit. On Windows, WebView2's own
/// processes can hold files in it a few seconds past the app's exit (CI
/// install test, Phase 6), so a hidden copy of the app finishes the job once
/// they let go (`--remove-scratch`). Anything still left goes on the next
/// launch.
pub fn remove(dir: &Path) {
    let _ = std::fs::remove_dir_all(dir);
    #[cfg(windows)]
    if dir.exists() {
        remove_after_exit(dir);
    }
}

#[cfg(windows)]
fn remove_after_exit(dir: &Path) {
    use std::os::windows::process::CommandExt;
    const DETACHED_PROCESS: u32 = 0x0000_0008;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    if let Ok(exe) = std::env::current_exe() {
        let _ = std::process::Command::new(exe)
            .arg("--remove-scratch")
            .arg(dir)
            .arg(std::process::id().to_string())
            .creation_flags(DETACHED_PROCESS | CREATE_NO_WINDOW)
            .spawn();
    }
}

/// Only ever one of our session folders, directly in the temp folder.
fn is_session_dir(dir: &Path) -> bool {
    let temp = std::env::temp_dir();
    dir.parent() == Some(temp.as_path())
        && dir
            .file_name()
            .is_some_and(|n| n.to_string_lossy().starts_with(PREFIX))
}

/// `--remove-scratch <dir> <pid>`: waits for the app (`pid`) to exit, then
/// removes its session folder, trying for up to a minute. Returns the exit
/// code.
pub fn remove_when_free(dir: &Path, pid: u32) -> i32 {
    if !is_session_dir(dir) {
        return 2;
    }
    wait_for_exit(pid);
    for _ in 0..120 {
        if std::fs::remove_dir_all(dir).is_ok() || !dir.exists() {
            return 0;
        }
        std::thread::sleep(std::time::Duration::from_millis(500));
    }
    1
}

#[cfg(windows)]
fn wait_for_exit(pid: u32) {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::{
        OpenProcess, PROCESS_SYNCHRONIZE, WaitForSingleObject,
    };
    // SAFETY: plain Win32 calls; a null handle (the app already gone) is
    // checked before use, and the handle is closed once.
    unsafe {
        let h = OpenProcess(PROCESS_SYNCHRONIZE, 0, pid);
        if !h.is_null() {
            WaitForSingleObject(h, 30_000);
            CloseHandle(h);
        }
    }
}

#[cfg(not(windows))]
fn wait_for_exit(_pid: u32) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn removes_only_session_folders_in_temp() {
        let temp = std::env::temp_dir();
        assert!(is_session_dir(&temp.join(format!("{PREFIX}123"))));
        assert!(!is_session_dir(&temp.join("something-else")));
        assert!(!is_session_dir(
            &temp.join("sub").join(format!("{PREFIX}123"))
        ));
        assert!(!is_session_dir(Path::new("/")));
        assert_eq!(remove_when_free(&temp.join("not-ours"), 1), 2);
    }

    #[test]
    fn removes_a_session_folder() {
        let dir = std::env::temp_dir().join(format!("{PREFIX}test-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("webview")).unwrap();
        std::fs::write(dir.join("webview").join("f"), b"x").unwrap();
        assert_eq!(remove_when_free(&dir, u32::MAX), 0);
        assert!(!dir.exists());
    }
}
