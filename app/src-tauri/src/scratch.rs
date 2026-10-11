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

/// Best effort; whatever is still locked goes on the next launch.
pub fn remove(dir: &Path) {
    let _ = std::fs::remove_dir_all(dir);
}
