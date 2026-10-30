//! Settings changed from the tray, kept across restarts.

use std::{fs, path::PathBuf};

use serde::{Deserialize, Serialize};

use crate::paths;

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub logging: bool,
    /// Window management is on. Off until switched on.
    pub tile: bool,
    /// Key remapping is on. Off until switched on.
    pub key: bool,
}

fn file() -> PathBuf {
    paths::data_dir().join("settings.json")
}

pub fn load() -> Settings {
    fs::read_to_string(file())
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

/// Loads the settings, applies `change` and writes them back.
pub fn update(change: impl FnOnce(&mut Settings)) {
    let mut settings = load();
    change(&mut settings);
    let result = fs::create_dir_all(paths::data_dir())
        .and_then(|()| fs::write(file(), serde_json::to_string(&settings).unwrap_or_default()));
    if let Err(e) = result {
        tracing::warn!("failed to save settings: {e}");
    }
}
