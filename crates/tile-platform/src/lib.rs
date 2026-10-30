//! Windows integration for Tile, built on safe wrappers:
//! `winsafe` for Win32/DWM, `wineventhook` for window events, `tile-hook` for the
//! low-level keyboard hooks, `uiautomation` for closing windows and `tray-icon` for the tray.

mod click_to_do;
pub mod desktops;
mod drag;
mod events;
mod hotkeys;
mod lock;
mod monitors;
mod mover;
mod programs;
mod token;
mod tray;
mod windows;

pub use click_to_do::{click_to_do_available, click_to_do_disabled, disable_click_to_do};
pub use drag::{DragEvent, WinDrag};
pub use events::{WinEvent, WinEventHooks};
pub use hotkeys::{HotkeyEvent, Hotkeys};
pub use lock::{GRANT_SCRIPT, lock_disabled, lock_workstation, set_lock_disabled};
pub use monitors::monitors;
pub use mover::{Animation, Insets, Mover};
pub use programs::installed;
pub use token::{is_elevated, user_sid};
pub use tray::{Tray, TrayEvent};
pub use windows::{
    Discovered, PlaceError, Windows, close_foreground, cursor_pos, focus_desktop, focus_exe,
    foreground, is_shell_window, mouse_button_down, restore_windows, set_cursor_pos, wait_for_exit,
};
