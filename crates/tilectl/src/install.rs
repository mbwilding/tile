//! `tilectl install` / `uninstall`: a per-user copy of Tile that starts at sign-in, with
//! `tilectl` on the user's `PATH`. No admin rights needed.

use std::{
    fs, io,
    os::windows::process::CommandExt,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};

use anyhow::{Context, bail};
use tile_ipc::{Client, Request};
use winsafe::{self as w, co};

/// How long a stopped Tile (and its watchdog) may take to let go of its files.
const RELEASE_TIMEOUT: Duration = Duration::from_secs(5);

const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const RUN_VALUE: &str = "Tile";
const ENV_KEY: &str = "Environment";
const PATH_VALUE: &str = "Path";

const DETACHED_PROCESS: u32 = 0x0000_0008;
const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
const CREATE_BREAKAWAY_FROM_JOB: u32 = 0x0100_0000;
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// `%LOCALAPPDATA%\Programs\Tile`, the usual place for per-user programs.
fn install_dir() -> PathBuf {
    dirs::data_local_dir().unwrap_or_else(|| PathBuf::from(".")).join("Programs").join("Tile")
}

/// The programs `install` copies.
const EXES: [&str; 2] = ["tile.exe", "tilectl.exe"];

/// The folder holding the build to install: this `tilectl.exe`'s own, or for the installed
/// `tilectl`, `target\release` under the current folder (a Tile checkout).
fn build_dir(dir: &Path) -> anyhow::Result<PathBuf> {
    let exe = std::env::current_exe()?;
    let own = exe.parent().context("no folder for tilectl.exe")?;
    let source = if own == dir { std::env::current_dir()?.join("target").join("release") } else { own.to_owned() };
    for name in EXES {
        if !source.join(name).exists() {
            bail!(
                "no build to install: {} not found. Run `cargo build --release --workspace` in a Tile \
                 checkout, then `tilectl install` from there",
                source.join(name).display()
            );
        }
    }
    Ok(source)
}

/// Copies `tile.exe` and `tilectl.exe` from the build to [`install_dir`], puts that on the user's
/// `PATH`, registers Tile to start at sign-in and starts it, replacing a running Tile.
pub async fn install() -> anyhow::Result<()> {
    let dir = install_dir();
    let source = build_dir(&dir)?;
    stop_running().await?;
    fs::create_dir_all(&dir).with_context(|| format!("failed to create {}", dir.display()))?;
    remove_replaced(&dir);
    for name in EXES {
        replace(&source.join(name), &dir.join(name))?;
    }
    let target = dir.join("tile.exe");
    set_run_at_login(Some(&format!("\"{}\"", target.display())))
        .context("failed to register Tile to start at sign-in")?;
    set_on_path(&dir, true)?;
    start_detached(&target).context("failed to start the installed Tile")?;
    println!("Installed to {}. Tile is running and starts at sign-in.", dir.display());
    println!("`tilectl` is on your PATH in new terminals.");
    Ok(())
}

/// Stops Tile, undoes [`install`] and deletes [`install_dir`]. The config and logs are kept.
pub async fn uninstall() -> anyhow::Result<()> {
    stop_running().await?;
    set_run_at_login(None).context("failed to remove the sign-in registration")?;
    let dir = install_dir();
    set_on_path(&dir, false)?;
    let inside = std::env::current_exe().is_ok_and(|exe| exe.starts_with(&dir));
    if inside {
        // A running exe can't delete itself: let `cmd` do it once we've exited.
        Command::new("cmd")
            .raw_arg(format!("/C ping -n 3 127.0.0.1 >nul & rmdir /S /Q \"{}\"", dir.display()))
            .creation_flags(DETACHED_PROCESS | CREATE_NO_WINDOW)
            .spawn()
            .with_context(|| format!("failed to schedule deleting {}", dir.display()))?;
    } else {
        match retry(|| fs::remove_dir_all(&dir)) {
            Err(e) if e.kind() != io::ErrorKind::NotFound => {
                return Err(e).with_context(|| format!("failed to delete {}", dir.display()));
            }
            _ => {}
        }
    }
    println!("Uninstalled. Your config (%APPDATA%\\Tile) and logs were kept.");
    Ok(())
}

/// Asks a running Tile to exit and waits until it has.
async fn stop_running() -> anyhow::Result<()> {
    if !tile_ipc::is_running().await {
        return Ok(());
    }
    Client::send(&Request::Dispatch { command: "exit".into() }).await?;
    let deadline = Instant::now() + RELEASE_TIMEOUT;
    while tile_ipc::is_running().await {
        if Instant::now() > deadline {
            bail!("the running Tile didn't exit; quit it from the tray and try again");
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    println!("Stopped the running Tile.");
    Ok(())
}

/// Retries `op` while the files may still be held by a Tile that is shutting down.
fn retry(op: impl Fn() -> io::Result<()>) -> io::Result<()> {
    let deadline = Instant::now() + RELEASE_TIMEOUT;
    loop {
        match op() {
            Err(e) if Instant::now() < deadline && e.kind() != io::ErrorKind::NotFound => {
                std::thread::sleep(Duration::from_millis(200));
            }
            result => return result,
        }
    }
}

/// Copies `from` over `to`. A running exe can't be overwritten, but it can be renamed: the old
/// one is moved aside first, so a running `tilectl` or a stopping Tile never blocks an update.
fn replace(from: &Path, to: &Path) -> anyhow::Result<()> {
    if to.exists() {
        let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_nanos();
        let aside = to.with_extension(format!("{nanos}.old"));
        fs::rename(to, &aside).with_context(|| format!("failed to move {} aside", to.display()))?;
    }
    fs::copy(from, to).with_context(|| format!("failed to copy to {}", to.display()))?;
    Ok(())
}

/// Deletes the exes [`replace`] moved aside, except those still running.
fn remove_replaced(dir: &Path) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for path in entries.flatten().map(|e| e.path()) {
        if path.extension().is_some_and(|ext| ext == "old") {
            let _ = fs::remove_file(path);
        }
    }
}

/// Starts `exe` without a console and outside our job object if allowed, so it outlives us
/// (e.g. when `cargo run` kills its job on exit).
fn start_detached(exe: &Path) -> io::Result<()> {
    let launch = |flags: u32| {
        Command::new(exe)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .creation_flags(flags)
            .spawn()
    };
    let base = DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP;
    launch(base | CREATE_BREAKAWAY_FROM_JOB).or_else(|_| launch(base)).map(drop)
}

fn io_error(e: co::ERROR) -> io::Error {
    io::Error::from_raw_os_error(e.raw() as i32)
}

/// Registers `command` to run at sign-in, or removes the registration with `None`.
fn set_run_at_login(command: Option<&str>) -> io::Result<()> {
    match command {
        Some(command) => w::HKEY::CURRENT_USER.RegSetKeyValue(
            Some(RUN_KEY),
            Some(RUN_VALUE),
            w::RegistryValue::Sz(command.to_owned()),
        ),
        None => w::HKEY::CURRENT_USER
            .RegOpenKeyEx(Some(RUN_KEY), co::REG_OPTION::default(), co::KEY::SET_VALUE)
            .and_then(|key| key.RegDeleteValue(Some(RUN_VALUE)))
            .or_else(|e| if e == co::ERROR::FILE_NOT_FOUND { Ok(()) } else { Err(e) }),
    }
    .map_err(io_error)
}

/// Adds `dir` to (`present`) or removes it from the user's `PATH`, keeping its other entries
/// and its registry type, and tells running apps when it changed.
fn set_on_path(dir: &Path, present: bool) -> anyhow::Result<()> {
    let flags = co::RRF::RT_REG_SZ | co::RRF::RT_REG_EXPAND_SZ | co::RRF::NOEXPAND;
    let (path, expand) = match w::HKEY::CURRENT_USER.RegGetValue(Some(ENV_KEY), Some(PATH_VALUE), flags) {
        Ok(w::RegistryValue::Sz(s)) => (s, false),
        Ok(w::RegistryValue::ExpandSz(s)) => (s, true),
        _ => (String::new(), true),
    };
    let Some(path) = edit_path(&path, &dir.to_string_lossy(), present) else {
        return Ok(());
    };
    let value = if expand { w::RegistryValue::ExpandSz(path) } else { w::RegistryValue::Sz(path) };
    w::HKEY::CURRENT_USER
        .RegSetKeyValue(Some(ENV_KEY), Some(PATH_VALUE), value)
        .map_err(io_error)
        .context("failed to update your PATH")?;
    announce_environment_change();
    Ok(())
}

/// `path` with `dir` added or removed, or `None` if it already is the way `present` asks.
/// Entries match case-insensitively, ignoring a trailing backslash.
fn edit_path(path: &str, dir: &str, present: bool) -> Option<String> {
    let same = |entry: &str| entry.trim_end_matches('\\').eq_ignore_ascii_case(dir.trim_end_matches('\\'));
    let mut entries: Vec<&str> = path.split(';').filter(|e| !e.is_empty()).collect();
    if entries.iter().any(|e| same(e)) == present {
        return None;
    }
    if present {
        entries.push(dir);
    } else {
        entries.retain(|e| !same(e));
    }
    Some(entries.join(";"))
}

/// Broadcasts `WM_SETTINGCHANGE` ("Environment") so Explorer, and the terminals it starts from
/// now on, pick up a changed `PATH`. Done through PowerShell: the call takes a raw pointer,
/// which can't be made without `unsafe`.
fn announce_environment_change() {
    const SCRIPT: &str = r#"Add-Type -Namespace W -Name U -MemberDefinition '[DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern System.IntPtr SendMessageTimeout(System.IntPtr h, uint m, System.UIntPtr w, string l, uint f, uint t, out System.UIntPtr r);'; $r = [UIntPtr]::Zero; [void][W.U]::SendMessageTimeout([IntPtr]0xffff, 0x1A, [UIntPtr]::Zero, 'Environment', 2, 5000, [ref]$r)"#;
    let result = Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", SCRIPT])
        .creation_flags(CREATE_NO_WINDOW)
        .status();
    if let Err(e) = result {
        eprintln!("warning: couldn't announce the PATH change (sign out and in to pick it up): {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIR: &str = r"C:\Users\me\AppData\Local\Programs\Tile";

    #[test]
    fn adds_once_keeping_other_entries() {
        let path = r"%USERPROFILE%\bin;C:\tools;";
        assert_eq!(edit_path(path, DIR, true).unwrap(), format!(r"%USERPROFILE%\bin;C:\tools;{DIR}"));
        assert_eq!(edit_path(&format!(r"C:\tools;{DIR}\"), DIR, true), None);
        assert_eq!(edit_path("", DIR, true).unwrap(), DIR);
    }

    #[test]
    fn removes_every_matching_entry() {
        let path = format!(r"C:\tools;{};%USERPROFILE%\bin;{DIR}\", DIR.to_lowercase());
        assert_eq!(edit_path(&path, DIR, false).unwrap(), r"C:\tools;%USERPROFILE%\bin");
        assert_eq!(edit_path(r"C:\tools", DIR, false), None);
    }
}
