use std::{
    io,
    sync::{
        Arc, RwLock,
        atomic::{AtomicBool, Ordering},
    },
    time::Instant,
};

use tile_config::{Keybind, Modifiers};
use tile_hook::{Decision, Handler, Input, RawKey};
use tokio::sync::mpsc;
use win_hotkeys::VKey;
use winsafe::{self as w, co};

/// An unassigned virtual key: tapping it while Win is down stops Win's release opening Start.
const SILENT_VK: u16 = 0xE8;

#[derive(Debug, Clone)]
struct Binding {
    vk: u16,
    modifiers: Modifiers,
    index: usize,
    /// Still fires while paused (the pause toggle itself).
    always: bool,
}

/// Global hotkeys via a `WH_KEYBOARD_LL` hook, including Win-key combos.
///
/// Matched keys are swallowed (with the Win key's Start menu suppressed) and the index of
/// the bind is sent to the daemon. The bind table can be swapped at any time.
///
/// Start this before the remapper: Windows calls the newest hook first, so the remapper's
/// hook then sees physical keys and this one sees the keys it sends.
#[derive(Clone)]
pub struct Hotkeys {
    table: Arc<RwLock<Vec<Binding>>>,
    paused: Arc<AtomicBool>,
}

/// Matches key presses against the bind table, on the hook thread.
struct Matcher {
    hotkeys: Hotkeys,
    tx: mpsc::UnboundedSender<usize>,
}

fn held() -> Modifiers {
    let down = w::GetAsyncKeyState;
    Modifiers {
        super_key: down(co::VK::LWIN) || down(co::VK::RWIN),
        shift: down(co::VK::SHIFT),
        ctrl: down(co::VK::CONTROL),
        alt: down(co::VK::MENU),
    }
}

impl Handler for Matcher {
    fn key(&mut self, key: RawKey) -> Decision {
        if !key.down || key.vk == SILENT_VK {
            return Decision::Pass;
        }
        let mods = held();
        let paused = self.hotkeys.paused.load(Ordering::Relaxed);
        let matched = self.hotkeys.table.read().ok().and_then(|t| {
            t.iter()
                .find(|b| b.vk == key.vk && b.modifiers == mods && (b.always || !paused))
                .map(|b| b.index)
        });
        let Some(index) = matched else {
            return Decision::Pass;
        };
        if mods.super_key {
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
        }
        let _ = self.tx.send(index);
        Decision::Block
    }

    fn deadline(&self) -> Option<Instant> {
        None
    }

    fn tick(&mut self) {}

    fn wake(&mut self) -> bool {
        true
    }
}

impl Hotkeys {
    /// Installs the keyboard hook on its own thread. Returns once it is installed.
    pub fn start(tx: mpsc::UnboundedSender<usize>) -> io::Result<Self> {
        let hotkeys = Self {
            table: Arc::new(RwLock::new(Vec::new())),
            paused: Arc::new(AtomicBool::new(false)),
        };
        let matcher = Matcher {
            hotkeys: hotkeys.clone(),
            tx,
        };
        tile_hook::spawn("hotkey hook", move || matcher)?;
        Ok(hotkeys)
    }

    /// Replaces the bind table. The `bool` marks binds that also work while paused.
    /// Returns a message for every bind whose key isn't recognised.
    pub fn set_binds(&self, binds: &[(Keybind, bool)]) -> Vec<String> {
        let mut errors = Vec::new();
        let table = binds
            .iter()
            .enumerate()
            .filter_map(
                |(index, (bind, always))| match VKey::from_keyname(&bind.key) {
                    Ok(key) => Some(Binding {
                        vk: key.to_vk_code(),
                        modifiers: bind.modifiers,
                        index,
                        always: *always,
                    }),
                    Err(e) => {
                        errors.push(format!("bind `{bind}`: {e}"));
                        None
                    }
                },
            )
            .collect();
        if let Ok(mut t) = self.table.write() {
            *t = table;
        }
        errors
    }

    pub fn set_paused(&self, paused: bool) {
        self.paused.store(paused, Ordering::Relaxed);
    }
}
