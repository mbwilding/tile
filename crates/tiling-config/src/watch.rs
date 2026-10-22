use std::path::{Path, PathBuf};

use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};

/// Calls `on_change` whenever the file at `path` is written, created or replaced.
///
/// The parent directory is watched because many editors save by replacing the file.
/// Keep the returned watcher alive for as long as notifications are wanted.
pub fn watch(path: &Path, on_change: impl Fn() + Send + 'static) -> notify::Result<RecommendedWatcher> {
    let target: PathBuf = path.to_path_buf();
    let file_name = target.file_name().map(ToOwned::to_owned);
    let mut watcher = notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
        let Ok(event) = event else { return };
        if !matches!(event.kind, EventKind::Create(_) | EventKind::Modify(_)) {
            return;
        }
        if event.paths.iter().any(|p| p.file_name() == file_name.as_deref()) {
            on_change();
        }
    })?;
    let dir = target.parent().unwrap_or_else(|| Path::new("."));
    watcher.watch(dir, RecursiveMode::NonRecursive)?;
    Ok(watcher)
}
