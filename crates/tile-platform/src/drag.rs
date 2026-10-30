use std::{
    io,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use tile_hook::{Decision, Input, MouseEvent, MouseHandler, MouseHook};
use tokio::sync::mpsc;
use winsafe::{self as w, co};

use crate::{
    hotkeys::{SILENT_VK, WIN_USED},
    windows::is_shell_window,
};

/// A Win+left-button drag, as the mouse hook sees it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DragEvent {
    /// Pressed over the top-level window `window`.
    Start {
        window: isize,
        x: i32,
        y: i32,
    },
    Move {
        x: i32,
        y: i32,
    },
    End {
        x: i32,
        y: i32,
    },
}

/// Turns Win + left-drag into [`DragEvent`]s via a `WH_MOUSE_LL` hook. The press and release
/// are swallowed, so the window under the pointer never sees the click.
pub struct WinDrag {
    enabled: Arc<AtomicBool>,
    _hook: MouseHook,
}

struct Dragger {
    enabled: Arc<AtomicBool>,
    tx: mpsc::UnboundedSender<DragEvent>,
    active: bool,
}

fn win_held() -> bool {
    w::GetAsyncKeyState(co::VK::LWIN) || w::GetAsyncKeyState(co::VK::RWIN)
}

fn window_at(x: i32, y: i32) -> Option<isize> {
    let hwnd = w::HWND::WindowFromPoint(w::POINT::with(x, y))?;
    let root = hwnd.GetAncestor(co::GA::ROOT).unwrap_or(hwnd);
    Some(root.ptr() as isize)
}

impl MouseHandler for Dragger {
    fn mouse(&mut self, event: MouseEvent, x: i32, y: i32) -> Decision {
        match event {
            MouseEvent::LeftDown
                if !self.active && self.enabled.load(Ordering::Relaxed) && win_held() =>
            {
                let Some(window) = window_at(x, y).filter(|w| !is_shell_window(*w)) else {
                    return Decision::Pass;
                };
                self.active = true;
                WIN_USED.store(true, Ordering::Relaxed);
                // Stops Win's release opening the Start menu.
                tile_hook::send(&[
                    Input::Vk {
                        vk: SILENT_VK,
                        up: false,
                    },
                    Input::Vk {
                        vk: SILENT_VK,
                        up: true,
                    },
                ]);
                let _ = self.tx.send(DragEvent::Start { window, x, y });
                Decision::Block
            }
            MouseEvent::Move if self.active => {
                let _ = self.tx.send(DragEvent::Move { x, y });
                Decision::Pass
            }
            MouseEvent::LeftUp if self.active => {
                self.active = false;
                let _ = self.tx.send(DragEvent::End { x, y });
                Decision::Block
            }
            _ => Decision::Pass,
        }
    }
}

impl WinDrag {
    pub fn start(tx: mpsc::UnboundedSender<DragEvent>) -> io::Result<Self> {
        let enabled = Arc::new(AtomicBool::new(false));
        let dragger = Dragger {
            enabled: enabled.clone(),
            tx,
            active: false,
        };
        let hook = tile_hook::spawn_mouse("mouse hook", move || dragger)?;
        Ok(Self {
            enabled,
            _hook: hook,
        })
    }

    /// Whether Win + drag is taken over. A drag already in progress still finishes.
    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::Relaxed);
    }
}
