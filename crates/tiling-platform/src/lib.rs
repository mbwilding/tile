//! Windows integration for `tiling`, built entirely on safe wrappers:
//! `winsafe` for Win32/DWM, `wineventhook` for window events, `win-hotkeys` for the
//! low-level keyboard hook, `uiautomation` for closing windows and `tray-icon` for the tray.

pub mod desktops;
mod events;
mod hotkeys;
mod monitors;
mod tray;
mod windows;

pub use events::{WinEvent, WinEventHooks};
pub use hotkeys::Hotkeys;
pub use monitors::monitors;
pub use tray::{Tray, TrayEvent};
pub use windows::{
    Discovered, PlaceError, Windows, cursor_pos, foreground, is_shell_window, restore_windows,
    wait_for_exit,
};
