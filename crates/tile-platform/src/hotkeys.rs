use std::{
    sync::{
        Arc, RwLock,
        atomic::{AtomicBool, Ordering},
    },
    thread,
};

use tile_config::{Keybind, Modifiers};
use tokio::sync::mpsc;
use win_hotkeys::{
    VKey,
    hook::{self, KeyAction, KeyboardEvent},
    state::KeyboardState,
};

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
#[derive(Clone)]
pub struct Hotkeys {
    table: Arc<RwLock<Vec<Binding>>>,
    paused: Arc<AtomicBool>,
}

fn held(state: &KeyboardState) -> Modifiers {
    let down = |key: VKey| state.is_down(key.to_vk_code());
    Modifiers {
        super_key: down(VKey::LWin) || down(VKey::RWin),
        shift: down(VKey::Shift),
        ctrl: down(VKey::Control),
        alt: down(VKey::Menu),
    }
}

impl Hotkeys {
    /// Starts the keyboard hook on its own thread.
    pub fn start(tx: mpsc::UnboundedSender<usize>) -> Self {
        let hotkeys = Self {
            table: Arc::new(RwLock::new(Vec::new())),
            paused: Arc::new(AtomicBool::new(false)),
        };
        let worker = hotkeys.clone();
        thread::Builder::new()
            .name("keyboard hook".into())
            .spawn(move || worker.run(&tx))
            .expect("spawn keyboard hook thread");
        hotkeys
    }

    fn run(&self, tx: &mpsc::UnboundedSender<usize>) {
        let hook = hook::start();
        while let Ok(event) = hook.recv() {
            let KeyboardEvent::KeyDown { vk_code, keyboard_state } = event else {
                continue;
            };
            let mods = held(&keyboard_state);
            let paused = self.paused.load(Ordering::Relaxed);
            let matched = self
                .table
                .read()
                .ok()
                .and_then(|t| {
                    t.iter()
                        .find(|b| b.vk == vk_code && b.modifiers == mods && (b.always || !paused))
                        .map(|b| b.index)
                });
            match matched {
                Some(index) => {
                    // `Replace` also injects a dummy key so releasing Win won't open Start.
                    hook.key_action(if mods.super_key { KeyAction::Replace } else { KeyAction::Block });
                    if tx.send(index).is_err() {
                        break;
                    }
                }
                None => hook.key_action(KeyAction::Allow),
            }
        }
        hook.exit();
    }

    /// Replaces the bind table. `always[i]` marks binds that work while paused.
    /// Returns a message for every bind whose key isn't recognised.
    pub fn set_binds(&self, binds: &[(Keybind, bool)]) -> Vec<String> {
        let mut errors = Vec::new();
        let table = binds
            .iter()
            .enumerate()
            .filter_map(|(index, (bind, always))| match VKey::from_keyname(&bind.key) {
                Ok(key) => Some(Binding { vk: key.to_vk_code(), modifiers: bind.modifiers, index, always: *always }),
                Err(e) => {
                    errors.push(format!("bind `{bind}`: {e}"));
                    None
                }
            })
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
