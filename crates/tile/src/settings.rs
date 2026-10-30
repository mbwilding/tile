//! The tray's switches (Tile, Key, Hotkey, logging), kept in the config's `[tray]` table so
//! they survive a restart.

use std::{
    path::{Path, PathBuf},
    sync::OnceLock,
};

pub use tile_config::Tray as Settings;
use tile_config::{Loaded, default_path, write_tray};

static CONFIG: OnceLock<PathBuf> = OnceLock::new();

/// Says which config file holds the switches, when it isn't the default one.
pub fn init(path: &Path) {
    let _ = CONFIG.set(path.to_path_buf());
}

fn file() -> PathBuf {
    CONFIG.get().cloned().unwrap_or_else(default_path)
}

pub fn load() -> Settings {
    Loaded::load_or_default(&file()).map_or_else(|_| Settings::default(), |l| l.config.tray)
}

/// Loads the switches, applies `change` and writes them back to the config file.
pub fn update(change: impl FnOnce(&mut Settings)) {
    let mut settings = load();
    change(&mut settings);
    if let Err(e) = write_tray(&file(), &settings) {
        tracing::warn!("failed to save settings: {e}");
    }
}
