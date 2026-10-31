//! Starting programs for `exec` and the tray. An elevated Tile hands them to the launcher
//! (`tile --launcher`, running as the normal user), so they don't inherit its admin rights.

use std::{os::windows::process::CommandExt, path::Path, sync::OnceLock};

use tile_ipc::launcher::{self, LaunchRequest, LaunchServer};

/// Runs a command line through `start`, detached and without a console window.
fn run(command_line: &str) {
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let result = std::process::Command::new("cmd")
        .raw_arg(format!("/C start \"\" {command_line}"))
        .creation_flags(CREATE_NO_WINDOW)
        .spawn();
    if let Err(e) = result {
        tracing::error!("failed to run `{command_line}`: {e}");
    }
}

fn elevated() -> bool {
    static ELEVATED: OnceLock<bool> = OnceLock::new();
    *ELEVATED.get_or_init(tile_platform::is_elevated)
}

/// Runs a command line as the normal user. Must be called inside the Tokio runtime.
pub fn exec(command_line: &str) {
    if !elevated() {
        run(command_line);
        return;
    }
    let command_line = command_line.to_owned();
    tokio::spawn(async move {
        let request = LaunchRequest::Exec {
            command_line: command_line.clone(),
        };
        if let Err(e) = launcher::send(&request).await {
            tracing::warn!("no launcher ({e}); running `{command_line}` as administrator");
            run(&command_line);
        }
    });
}

/// Opens a file or folder with its default program.
pub fn open(path: &Path) {
    exec(&format!("\"{}\"", path.display()));
}

/// `tile --launcher`: runs launch requests until asked to exit. Returns at once if a launcher
/// is already running.
pub async fn serve() -> anyhow::Result<()> {
    if launcher::is_running().await {
        return Ok(());
    }
    let server = LaunchServer::bind()?;
    tracing::info!("launcher started");
    loop {
        match server.next().await {
            Ok(LaunchRequest::Exec { command_line }) => run(&command_line),
            Ok(LaunchRequest::Exit) => break,
            Err(e) => tracing::debug!("bad launch request: {e}"),
        }
    }
    tracing::info!("launcher stopped");
    Ok(())
}
