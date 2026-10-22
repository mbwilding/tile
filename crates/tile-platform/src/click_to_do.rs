//! Windows' Click to Do (Copilot+ PCs): holding Win shows a glow around the pointer, ready
//! for Win + click. It gets in the way of Win-based keybinds, so Tile can switch it off.
//!
//! This flips the same per-user switch as Settings → Privacy & security → Click to Do.

use std::{os::windows::process::CommandExt, path::Path, process::Command};

use winsafe::{self as w, co};

const KEY: &str = r"Software\Microsoft\Windows\Shell\ClickToDo";
const VALUE: &str = "DisableClickToDo";
const EXE: &str = r"C:\Windows\SystemApps\MicrosoftWindows.Client.CoreAI_cw5n1h2txyewy\ClickToDo.exe";
const APP_ID: &str = r"shell:AppsFolder\MicrosoftWindows.Client.CoreAI_cw5n1h2txyewy!ClickToDoApp";
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Whether this PC has Click to Do at all.
pub fn click_to_do_available() -> bool {
    Path::new(EXE).exists()
}

/// Whether Click to Do is switched off (by the user or by us).
pub fn click_to_do_disabled() -> bool {
    matches!(
        w::HKEY::CURRENT_USER.RegGetValue(Some(KEY), Some(VALUE), co::RRF::RT_REG_DWORD),
        Ok(w::RegistryValue::Dword(v)) if v != 0
    )
}

/// Switches Click to Do off (and stops it) or back on (and starts it again).
/// Returns whether the switch could be changed.
pub fn set_click_to_do(enabled: bool) -> bool {
    let result = if enabled {
        w::HKEY::CURRENT_USER
            .RegOpenKeyEx(Some(KEY), co::REG_OPTION::default(), co::KEY::SET_VALUE)
            .and_then(|key| key.RegDeleteValue(Some(VALUE)))
            .or_else(|e| if e == co::ERROR::FILE_NOT_FOUND { Ok(()) } else { Err(e) })
    } else {
        w::HKEY::CURRENT_USER.RegSetKeyValue(Some(KEY), Some(VALUE), w::RegistryValue::Dword(1))
    };
    if let Err(e) = result {
        tracing::warn!("failed to {} Click to Do: {e}", if enabled { "enable" } else { "disable" });
        return false;
    }
    // The running instance doesn't notice the switch, and nothing restarts it on its own.
    let command = if enabled {
        Command::new("explorer.exe").arg(APP_ID).creation_flags(CREATE_NO_WINDOW).spawn()
    } else {
        Command::new("taskkill")
            .args(["/IM", "ClickToDo.exe", "/F"])
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()
    };
    if let Err(e) = command {
        tracing::debug!("could not {} Click to Do: {e}", if enabled { "start" } else { "stop" });
    }
    true
}
