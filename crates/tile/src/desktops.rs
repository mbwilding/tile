use std::collections::HashMap;

use tile_core::{
    Effect, LayoutKind, LayoutSettings, ManageOptions, MonitorInfo, State, WindowId, WindowInfo,
};
use tile_platform::desktops::DesktopId;

/// One tiling [`State`] (workspaces + layouts) per native virtual desktop.
///
/// Windows on other desktops are cloaked by the shell, not closed, so they stay managed by
/// their desktop's state and keep their place in its layouts.
pub struct Desktops {
    states: HashMap<DesktopId, State>,
    current: DesktopId,
    owner: HashMap<WindowId, DesktopId>,
    settings: LayoutSettings,
    layout: LayoutKind,
    monitors: Vec<MonitorInfo>,
}

/// Effects from a background desktop must never steal focus.
fn background(mut effects: Vec<Effect>) -> Vec<Effect> {
    effects.retain(|e| !matches!(e, Effect::Focus(_)));
    effects
}

impl Desktops {
    pub fn new(current: DesktopId, settings: LayoutSettings, layout: LayoutKind) -> Self {
        let mut states = HashMap::new();
        states.insert(current, State::new(settings.clone(), layout));
        Self {
            states,
            current,
            owner: HashMap::new(),
            settings,
            layout,
            monitors: Vec::new(),
        }
    }

    pub const fn current_id(&self) -> DesktopId {
        self.current
    }

    fn state_mut(&mut self, desktop: DesktopId) -> &mut State {
        let (settings, layout, monitors) = (&self.settings, self.layout, &self.monitors);
        self.states.entry(desktop).or_insert_with(|| {
            let mut state = State::new(settings.clone(), layout);
            state.set_monitors(monitors.clone());
            state
        })
    }

    pub fn current(&mut self) -> &mut State {
        self.state_mut(self.current)
    }

    pub fn current_ref(&self) -> &State {
        // The current desktop's state is created on construction and on every switch.
        &self.states[&self.current]
    }

    pub fn owner(&self, id: WindowId) -> Option<DesktopId> {
        self.owner.get(&id).copied()
    }

    pub fn is_managed(&self, id: WindowId) -> bool {
        self.owner.contains_key(&id)
    }

    pub fn managed(&self) -> Vec<WindowId> {
        self.owner.keys().copied().collect()
    }

    /// Windows hidden by any desktop's workspaces.
    pub fn hidden(&self) -> Vec<WindowId> {
        self.states.values().flat_map(State::hidden).collect()
    }

    fn owner_ref(&self, id: WindowId) -> Option<&State> {
        self.states.get(&self.owner(id)?)
    }

    pub fn is_hidden(&self, id: WindowId) -> bool {
        self.owner_ref(id).is_some_and(|s| s.is_hidden(id))
    }

    pub fn is_floating(&self, id: WindowId) -> bool {
        self.owner_ref(id).is_some_and(|s| s.is_floating(id))
    }

    /// Runs `f` against the state that manages `id`.
    pub fn with_owner(
        &mut self,
        id: WindowId,
        f: impl FnOnce(&mut State) -> Vec<Effect>,
    ) -> Vec<Effect> {
        let Some(desktop) = self.owner(id) else {
            return Vec::new();
        };
        let effects = f(self.state_mut(desktop));
        if desktop == self.current {
            effects
        } else {
            background(effects)
        }
    }

    pub fn open(
        &mut self,
        desktop: DesktopId,
        id: WindowId,
        info: WindowInfo,
        opts: ManageOptions,
    ) -> Vec<Effect> {
        self.owner.insert(id, desktop);
        let effects = self.state_mut(desktop).window_opened(id, info, opts);
        if desktop == self.current {
            effects
        } else {
            background(effects)
        }
    }

    pub fn close(&mut self, id: WindowId) -> Vec<Effect> {
        let effects = self.with_owner(id, |s| s.window_closed(id));
        self.owner.remove(&id);
        effects
    }

    pub fn switch_to(&mut self, desktop: DesktopId) -> Vec<Effect> {
        self.current = desktop;
        self.current().sync()
    }

    fn for_all(&mut self, mut f: impl FnMut(&mut State) -> Vec<Effect>) -> Vec<Effect> {
        let current = self.current;
        self.states
            .iter_mut()
            .flat_map(|(desktop, state)| {
                let effects = f(state);
                if *desktop == current {
                    effects
                } else {
                    background(effects)
                }
            })
            .collect()
    }

    pub fn set_monitors(&mut self, monitors: Vec<MonitorInfo>) -> Vec<Effect> {
        self.monitors.clone_from(&monitors);
        self.for_all(|s| s.set_monitors(monitors.clone()))
    }

    pub fn set_settings(&mut self, settings: LayoutSettings, layout: LayoutKind) -> Vec<Effect> {
        self.settings = settings.clone();
        self.layout = layout;
        self.for_all(|s| s.set_settings(settings.clone(), layout))
    }
}
