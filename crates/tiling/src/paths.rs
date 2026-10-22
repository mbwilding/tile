use std::{fs, io, path::PathBuf};

use serde::{Deserialize, Serialize};

/// `%LOCALAPPDATA%\tiling`, for logs and runtime state.
pub fn data_dir() -> PathBuf {
    dirs::data_local_dir().unwrap_or_else(|| PathBuf::from(".")).join("tiling")
}

pub fn log_dir() -> PathBuf {
    data_dir().join("logs")
}

/// Changes made to other apps' windows that must be undone if we die.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Restore {
    /// Windows we hid (inactive workspaces, scrolled-off columns).
    pub hidden: Vec<isize>,
    /// Windows whose title bar we removed.
    pub undecorated: Vec<isize>,
}

impl Restore {
    pub fn is_empty(&self) -> bool {
        self.hidden.is_empty() && self.undecorated.is_empty()
    }

    /// Undoes everything recorded here.
    pub fn apply(&self) {
        tiling_platform::restore_windows(&self.hidden, &self.undecorated);
    }
}

fn restore_file() -> PathBuf {
    data_dir().join("restore.json")
}

/// Persists the pending restore actions, removing the file when there are none.
pub fn save_restore(restore: &Restore) {
    let path = restore_file();
    let result = if restore.is_empty() {
        match fs::remove_file(&path) {
            Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        }
    } else {
        fs::create_dir_all(data_dir())
            .and_then(|()| fs::write(&path, serde_json::to_string(restore).unwrap_or_default()))
    };
    if let Err(e) = result {
        tracing::warn!("failed to persist restore state: {e}");
    }
}

pub fn load_restore() -> Restore {
    fs::read_to_string(restore_file())
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}
