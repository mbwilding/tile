use std::collections::{HashMap, VecDeque};

use crate::{Action, Chord, Config, Key, LayerId, MAIN, Mods};

/// A physical key going down (or repeating) or up. `time` is in milliseconds, from any
/// steady clock.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyEvent {
    pub key: Key,
    pub down: bool,
    pub time: u64,
}

/// A key to send.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Out {
    Press(Key),
    Release(Key),
    /// Tap a key that does nothing, so releasing Alt or Win alone doesn't open a menu.
    Guard,
}

/// What to do with a physical event: let it through unchanged (`pass`, only ever with no
/// `out`), or swallow it and send `out` instead.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Verdict {
    pub pass: bool,
    pub out: Vec<Out>,
}

/// What a held physical key resolved to when it was pressed.
#[derive(Debug, Clone, Copy)]
enum Held {
    /// Let through untouched.
    Pass,
    Noop,
    /// Sending this key.
    Key(Key),
    Layer(LayerId),
    Overload {
        layer: LayerId,
        tap: Chord,
        from: LayerId,
    },
    /// Waiting to see if it's a tap or a hold.
    Pending,
}

#[derive(Debug)]
struct Pending {
    key: Key,
    layer: LayerId,
    tap: Chord,
    /// The layer the binding was found in.
    from: LayerId,
    deadline: u64,
}

/// keyd's remapping, one physical event at a time.
///
/// Layers stack: the most recently activated one is searched first, then the others, then
/// `main`. A key no layer binds is sent as itself, with the modifiers of every active layer.
/// A bound key is sent with only the modifiers of the other active layers, plus its own.
#[derive(Debug)]
pub struct Engine {
    config: Config,
    /// Active layers in activation order (a layer held by two keys appears twice).
    active: Vec<LayerId>,
    held: HashMap<Key, Held>,
    pending: Option<Pending>,
    /// Events that arrived while a key was pending, in order.
    queue: VecDeque<KeyEvent>,
    /// Non-modifier keys we're holding down.
    down: Vec<Key>,
    /// Modifiers we're holding down.
    mods: Mods,
    last_pressed: Option<Key>,
    last_out: Option<Key>,
}

impl Engine {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            active: Vec::new(),
            held: HashMap::new(),
            pending: None,
            queue: VecDeque::new(),
            down: Vec::new(),
            mods: Mods::NONE,
            last_pressed: None,
            last_out: None,
        }
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    pub fn event(&mut self, event: KeyEvent) -> Verdict {
        let mut out = Vec::new();
        let pass = self.step(event, true, &mut out);
        Verdict { pass, out }
    }

    /// When [`tick`](Self::tick) must be called next, if a key is pending.
    pub fn deadline(&self) -> Option<u64> {
        self.pending.as_ref().map(|p| p.deadline)
    }

    /// Resolves a pending key whose timeout has passed by `now`.
    pub fn tick(&mut self, now: u64) -> Vec<Out> {
        let mut out = Vec::new();
        while self.pending.as_ref().is_some_and(|p| now >= p.deadline) {
            self.hold_pending(&mut out);
            self.replay(&mut out);
        }
        out
    }

    /// Forgets all state, returning the releases for every key we're holding down.
    pub fn reset(&mut self) -> Vec<Out> {
        let mut out: Vec<Out> = self.down.drain(..).map(Out::Release).collect();
        for (m, key) in Mods::KEYS {
            if self.mods.contains(m) {
                out.push(Out::Release(key));
            }
        }
        self.mods = Mods::NONE;
        self.active.clear();
        self.held.clear();
        self.pending = None;
        self.queue.clear();
        self.last_pressed = None;
        self.last_out = None;
        out
    }

    /// Handles one event. `live` is false for events replayed from the queue, which can no
    /// longer be let through and are sent again instead.
    fn step(&mut self, event: KeyEvent, live: bool, out: &mut Vec<Out>) -> bool {
        while self
            .pending
            .as_ref()
            .is_some_and(|p| event.time >= p.deadline)
        {
            self.hold_pending(out);
            self.replay(out);
        }
        if let Some(pending) = &self.pending {
            if event.key != pending.key {
                self.queue.push_back(event);
            } else if !event.down {
                self.tap_pending(out);
                self.replay(out);
            }
            return false;
        }
        if event.down {
            self.press(event, live, out)
        } else {
            self.release(event, live, out)
        }
    }

    fn press(&mut self, event: KeyEvent, live: bool, out: &mut Vec<Out>) -> bool {
        // Auto-repeat of a key already down.
        if let Some(&held) = self.held.get(&event.key) {
            return match held {
                Held::Pass => self.pass(event, live, out),
                Held::Key(key) => {
                    out.push(Out::Press(key));
                    false
                }
                _ => false,
            };
        }
        self.last_pressed = Some(event.key);
        let (action, from) = self.lookup(event.key);
        let held = match action {
            None if live && out.is_empty() && self.active.is_empty() && self.mods.is_empty() => {
                self.last_out = Some(event.key);
                self.held.insert(event.key, Held::Pass);
                return true;
            }
            None => {
                self.press_chord(Chord::key(event.key), from, out);
                Held::Key(event.key)
            }
            Some(Action::Noop) => Held::Noop,
            Some(Action::Chord(chord)) => {
                self.press_chord(chord, from, out);
                Held::Key(chord.key)
            }
            Some(Action::Layer(layer)) => {
                self.activate(layer, out);
                Held::Layer(layer)
            }
            Some(Action::Overload { layer, tap }) => {
                self.activate(layer, out);
                Held::Overload { layer, tap, from }
            }
            Some(Action::OverloadTimeout {
                layer,
                tap,
                timeout_ms,
            }) => {
                let deadline = event.time + timeout_ms;
                self.pending = Some(Pending {
                    key: event.key,
                    layer,
                    tap,
                    from,
                    deadline,
                });
                Held::Pending
            }
        };
        self.held.insert(event.key, held);
        false
    }

    fn release(&mut self, event: KeyEvent, live: bool, out: &mut Vec<Out>) -> bool {
        // Not seen going down (pressed before we started, or while paused): let it go.
        let Some(held) = self.held.remove(&event.key) else {
            return self.pass(event, live, out);
        };
        match held {
            Held::Pass => return self.pass(event, live, out),
            Held::Noop | Held::Pending => {}
            Held::Key(key) => self.release_key(key, out),
            Held::Layer(layer) => self.deactivate(layer, out),
            Held::Overload { layer, tap, from } => {
                self.deactivate(layer, out);
                if self.last_pressed == Some(event.key) {
                    self.press_chord(tap, from, out);
                    self.release_key(tap.key, out);
                }
            }
        }
        false
    }

    /// Lets the event through, or sends it if that would put it out of order.
    fn pass(&mut self, event: KeyEvent, live: bool, out: &mut Vec<Out>) -> bool {
        if live && out.is_empty() {
            return true;
        }
        out.push(if event.down {
            Out::Press(event.key)
        } else {
            Out::Release(event.key)
        });
        false
    }

    fn hold_pending(&mut self, out: &mut Vec<Out>) {
        let Some(pending) = self.pending.take() else {
            return;
        };
        self.held.insert(pending.key, Held::Layer(pending.layer));
        self.activate(pending.layer, out);
    }

    fn tap_pending(&mut self, out: &mut Vec<Out>) {
        let Some(pending) = self.pending.take() else {
            return;
        };
        self.held.remove(&pending.key);
        self.press_chord(pending.tap, pending.from, out);
        self.release_key(pending.tap.key, out);
    }

    /// Runs the queued events again, now the pending key is decided.
    fn replay(&mut self, out: &mut Vec<Out>) {
        for event in std::mem::take(&mut self.queue) {
            self.step(event, false, out);
        }
    }

    /// The binding for `key` and the layer it was found in. `None` means unbound.
    fn lookup(&self, key: Key) -> (Option<Action>, LayerId) {
        let layers = &self.config.layers;
        // Composite layers whose parts are all active come first, the most specific first.
        let composite = layers
            .iter()
            .enumerate()
            .filter(|(_, l)| {
                l.is_composite() && l.constituents.iter().all(|c| self.active.contains(c))
            })
            .filter_map(|(id, l)| l.binds.get(&key).map(|&a| (l.constituents.len(), id, a)))
            .max_by_key(|&(parts, ..)| parts);
        if let Some((_, id, action)) = composite {
            return (Some(action), id);
        }
        for &id in self.active.iter().rev() {
            if let Some(&action) = layers[id].binds.get(&key) {
                return (Some(action), id);
            }
        }
        (layers[MAIN].binds.get(&key).copied(), MAIN)
    }

    /// The modifiers of every active layer, except `excluded` (and its parts, if composite).
    fn layer_mods(&self, excluded: Option<LayerId>) -> Mods {
        let layers = &self.config.layers;
        let parts = excluded.map_or(&[][..], |id| &layers[id].constituents[..]);
        self.active
            .iter()
            .filter(|&&id| Some(id) != excluded && !parts.contains(&id))
            .fold(Mods::NONE, |mods, &id| mods | layers[id].mods)
    }

    fn activate(&mut self, layer: LayerId, out: &mut Vec<Out>) {
        self.active.push(layer);
        self.set_mods(self.layer_mods(None), out);
    }

    fn deactivate(&mut self, layer: LayerId, out: &mut Vec<Out>) {
        if let Some(i) = self.active.iter().rposition(|&l| l == layer) {
            self.active.remove(i);
        }
        self.set_mods(self.layer_mods(None), out);
    }

    fn press_chord(&mut self, chord: Chord, from: LayerId, out: &mut Vec<Out>) {
        self.set_mods(self.layer_mods(Some(from)) | chord.mods, out);
        out.push(Out::Press(chord.key));
        if !self.down.contains(&chord.key) {
            self.down.push(chord.key);
        }
        self.last_out = Some(chord.key);
    }

    fn release_key(&mut self, key: Key, out: &mut Vec<Out>) {
        out.push(Out::Release(key));
        self.down.retain(|&k| k != key);
        self.set_mods(self.layer_mods(None), out);
    }

    /// Presses and releases modifier keys until exactly `target` is held.
    fn set_mods(&mut self, target: Mods, out: &mut Vec<Out>) {
        for (m, key) in Mods::KEYS {
            match (target.contains(m), self.mods.contains(m)) {
                (true, false) => {
                    out.push(Out::Press(key));
                    self.last_out = Some(key);
                }
                (false, true) => {
                    let guarded = m == Mods::ALT || m == Mods::META;
                    if self.config.modifier_guard && guarded && self.last_out == Some(key) {
                        out.push(Out::Guard);
                    }
                    out.push(Out::Release(key));
                }
                _ => {}
            }
        }
        self.mods = target;
    }
}
