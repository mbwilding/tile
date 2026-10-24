// Release builds run without a console window; debug builds keep it for logs.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod daemon;
mod desktops;
mod launch;
mod logging;
mod paths;
mod remap;

use std::path::PathBuf;

use clap::Parser;
use tile_config::Loaded;

/// A dynamic tiling window manager and keyd-style key remapper for Windows.
#[derive(Parser)]
#[command(version, about)]
struct Args {
    /// Config file to use instead of %APPDATA%\Tile\config.toml.
    #[arg(long, short)]
    config: Option<PathBuf>,
    /// Log filter, e.g. `info` or `tile=debug`; also switches logging on. `RUST_LOG` takes
    /// precedence. Without either, the tray's Logging setting decides (default off, `info`).
    #[arg(long)]
    log: Option<String>,
    /// Internal: wait for the daemon with this process id to exit, then undo its changes.
    #[arg(long, hide = true)]
    watchdog: Option<u32>,
    /// Internal: start programs for an elevated Tile, as the normal user.
    #[arg(long, hide = true)]
    launcher: bool,
}

/// Runs in a separate process so windows are restored even if the daemon is killed.
fn watchdog(pid: u32) {
    tile_platform::wait_for_exit(pid);
    let restore = paths::load_restore();
    if !restore.is_empty() {
        restore.apply();
        paths::save_restore(&paths::Restore::default());
    }
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    if let Some(pid) = args.watchdog {
        watchdog(pid);
        return Ok(());
    }
    if args.launcher {
        let _guard = logging::init("launcher.log", args.log.as_deref());
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        return runtime.block_on(launch::serve());
    }
    let _guard = logging::init("tile.log", args.log.as_deref());

    // Never leave windows hidden or without title bars, even if we crash.
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        tracing::error!("panic: {info}");
        paths::load_restore().apply();
        previous(info);
    }));

    let config_path = args.config.unwrap_or_else(tile_config::default_path);
    if let Err(e) = tile_config::ensure_exists(&config_path) {
        tracing::warn!("could not create {}: {e}", config_path.display());
    }
    let loaded = Loaded::load_or_default(&config_path).unwrap_or_else(|e| {
        tracing::error!("{e}; using the built-in defaults");
        Loaded::defaults()
    });

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    let result = runtime.block_on(daemon::run(config_path, loaded));
    if let Err(e) = &result {
        tracing::error!("{e:#}");
    }
    result
}
