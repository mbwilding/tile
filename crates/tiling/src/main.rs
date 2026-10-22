// Release builds run without a console window; debug builds keep it for logs.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod daemon;
mod desktops;
mod paths;

use std::path::PathBuf;

use clap::Parser;
use tiling_config::{DEFAULT_TOML, Loaded};
use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt};

/// A Hyprland-style dynamic tiling window manager for Windows.
#[derive(Parser)]
#[command(version, about)]
struct Args {
    /// Config file to use instead of %APPDATA%\tiling\config.toml.
    #[arg(long, short)]
    config: Option<PathBuf>,
    /// Log filter, e.g. `info` or `tiling=debug`. `RUST_LOG` takes precedence.
    #[arg(long, default_value = "info")]
    log: String,
    /// Internal: wait for the daemon with this process id to exit, then undo its changes.
    #[arg(long, hide = true)]
    watchdog: Option<u32>,
}

/// Runs in a separate process so windows are restored even if the daemon is killed.
fn watchdog(pid: u32) {
    tiling_platform::wait_for_exit(pid);
    let restore = paths::load_restore();
    if !restore.is_empty() {
        restore.apply();
        paths::save_restore(&paths::Restore::default());
    }
}

fn init_logging(filter: &str) -> tracing_appender::non_blocking::WorkerGuard {
    let appender = tracing_appender::rolling::daily(paths::log_dir(), "tiling.log");
    let (file, guard) = tracing_appender::non_blocking(appender);
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(filter));
    tracing_subscriber::registry()
        .with(filter)
        .with(fmt::layer().with_writer(file).with_ansi(false))
        .with(cfg!(debug_assertions).then(|| fmt::layer().with_writer(std::io::stderr)))
        .init();
    guard
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    if let Some(pid) = args.watchdog {
        watchdog(pid);
        return Ok(());
    }
    let _guard = init_logging(&args.log);

    // Never leave windows hidden or without title bars, even if we crash.
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        tracing::error!("panic: {info}");
        paths::load_restore().apply();
        previous(info);
    }));

    let config_path = match args.config {
        Some(path) => path,
        None => {
            let path = tiling_config::default_path();
            if let Err(e) = tiling_config::ensure_exists(&path) {
                tracing::warn!("could not write default config to {}: {e}", path.display());
            }
            path
        }
    };
    let loaded = Loaded::load(&config_path).or_else(|e| {
        tracing::error!("{e}; using the built-in defaults");
        Loaded::parse(DEFAULT_TOML)
    })?;

    let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build()?;
    let result = runtime.block_on(daemon::run(config_path, loaded));
    if let Err(e) = &result {
        tracing::error!("{e:#}");
    }
    result
}
