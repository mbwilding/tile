use std::collections::{BTreeMap, HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::{
    Command, MonitorId, WindowId,
    geometry::{Direction, Rect, nearest_in_direction, scale},
    layout::{LayoutKind, LayoutSettings, LayoutState, Tile},
};

/// Descriptive information about a window, used for rules and IPC.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WindowInfo {
    pub title: String,
    pub class: String,
    pub exe: String,
}

/// How a newly discovered window should be managed (resolved from window rules).
#[derive(Debug, Clone, Default)]
pub struct ManageOptions {
    pub floating: bool,
    pub workspace: Option<u8>,
    pub monitor: Option<usize>,
    /// The window's current visible frame, used as its floating geometry.
    pub rect: Rect,
    /// Whether the new window should receive focus.
    pub focus: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MonitorInfo {
    pub id: MonitorId,
    pub rect: Rect,
    pub work_area: Rect,
}

/// A side effect for the platform layer to apply.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    /// Move and resize a window so its visible frame matches `rect`.
    Place { id: WindowId, rect: Rect },
    Show(WindowId),
    Hide(WindowId),
    Focus(WindowId),
    Close(WindowId),
    Exec(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowSnapshot {
    pub id: isize,
    pub title: String,
    pub class: String,
    pub exe: String,
    pub workspace: u8,
    pub floating: bool,
    pub fullscreen: bool,
    pub focused: bool,
    pub hidden: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceSnapshot {
    pub id: u8,
    pub monitor: Option<usize>,
    pub visible: bool,
    pub focused: bool,
    pub layout: LayoutKind,
    pub windows: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitorSnapshot {
    pub index: usize,
    pub rect: Rect,
    pub work_area: Rect,
    pub workspace: u8,
    pub focused: bool,
}

#[derive(Debug, Clone)]
struct Monitor {
    info: MonitorInfo,
    workspace: u8,
}

#[derive(Debug, Clone)]
struct Workspace {
    monitor: MonitorId,
    layout: LayoutState,
    floating: Vec<WindowId>,
    minimized: Vec<WindowId>,
    fullscreen: Option<WindowId>,
    /// Focus history, most recent last.
    history: Vec<WindowId>,
}

impl Workspace {
    fn new(monitor: MonitorId, kind: LayoutKind, s: &LayoutSettings) -> Self {
        Self {
            monitor,
            layout: LayoutState::new(kind, s),
            floating: Vec::new(),
            minimized: Vec::new(),
            fullscreen: None,
            history: Vec::new(),
        }
    }

    fn all_windows(&self) -> impl Iterator<Item = WindowId> + '_ {
        self.layout
            .windows()
            .into_iter()
            .chain(self.floating.iter().copied())
            .chain(self.minimized.iter().copied())
    }

    fn touch(&mut self, id: WindowId) {
        self.history.retain(|w| *w != id);
        self.history.push(id);
    }

    fn forget(&mut self, id: WindowId) {
        self.layout.remove(id);
        self.floating.retain(|w| *w != id);
        self.minimized.retain(|w| *w != id);
        self.history.retain(|w| *w != id);
        if self.fullscreen == Some(id) {
            self.fullscreen = None;
        }
    }

    fn focus_candidate(&self) -> Option<WindowId> {
        self.history
            .iter()
            .rev()
            .copied()
            .find(|w| !self.minimized.contains(w))
            .or_else(|| self.layout.windows().first().copied())
            .or_else(|| self.floating.first().copied())
    }

    fn len(&self) -> usize {
        self.layout.windows().len() + self.floating.len() + self.minimized.len()
    }
}

#[derive(Debug, Clone)]
struct Managed {
    info: WindowInfo,
    workspace: u8,
    floating: bool,
    float_rect: Rect,
}

/// The window manager's model of the desktop.
#[derive(Debug, Clone)]
pub struct State {
    monitors: Vec<Monitor>,
    workspaces: BTreeMap<u8, Workspace>,
    windows: HashMap<WindowId, Managed>,
    hidden: HashSet<WindowId>,
    focused: Option<WindowId>,
    focused_monitor: usize,
    settings: LayoutSettings,
    default_layout: LayoutKind,
}

impl State {
    pub fn new(settings: LayoutSettings, default_layout: LayoutKind) -> Self {
        Self {
            monitors: Vec::new(),
            workspaces: BTreeMap::new(),
            windows: HashMap::new(),
            hidden: HashSet::new(),
            focused: None,
            focused_monitor: 0,
            settings,
            default_layout,
        }
    }

    pub const fn focused(&self) -> Option<WindowId> {
        self.focused
    }

    pub fn is_managed(&self, id: WindowId) -> bool {
        self.windows.contains_key(&id)
    }

    pub fn managed(&self) -> impl Iterator<Item = WindowId> + '_ {
        self.windows.keys().copied()
    }

    /// Windows the manager has hidden (inactive workspaces, scrolled-off columns).
    pub fn hidden(&self) -> impl Iterator<Item = WindowId> + '_ {
        self.hidden.iter().copied()
    }

    pub fn is_hidden(&self, id: WindowId) -> bool {
        self.hidden.contains(&id)
    }

    pub fn is_floating(&self, id: WindowId) -> bool {
        self.windows.get(&id).is_some_and(|m| m.floating)
    }

    pub fn set_info(&mut self, id: WindowId, info: WindowInfo) {
        if let Some(m) = self.windows.get_mut(&id) {
            m.info = info;
        }
    }

    pub fn monitor_count(&self) -> usize {
        self.monitors.len()
    }

    pub fn focused_workspace(&self) -> Option<u8> {
        self.monitors.get(self.focused_monitor).map(|m| m.workspace)
    }

    pub fn active_layout(&self) -> Option<LayoutKind> {
        let ws = self.focused_workspace()?;
        self.workspaces.get(&ws).map(|w| w.layout.kind())
    }

    /// Applies new settings from a config reload.
    pub fn set_settings(&mut self, settings: LayoutSettings, default_layout: LayoutKind) -> Vec<Effect> {
        self.settings = settings;
        self.default_layout = default_layout;
        self.sync()
    }

    fn monitor_index(&self, id: MonitorId) -> Option<usize> {
        self.monitors.iter().position(|m| m.info.id == id)
    }

    fn showing(&self, ws: u8) -> Option<usize> {
        self.monitors.iter().position(|m| m.workspace == ws)
    }

    fn area_of(&self, ws: u8) -> Rect {
        let index = self
            .workspaces
            .get(&ws)
            .and_then(|w| self.monitor_index(w.monitor))
            .unwrap_or(self.focused_monitor);
        self.monitors.get(index).map(|m| m.info.work_area).unwrap_or_default()
    }

    fn ensure_workspace(&mut self, n: u8, monitor: MonitorId) {
        if !self.workspaces.contains_key(&n) {
            let ws = Workspace::new(monitor, self.default_layout, &self.settings);
            self.workspaces.insert(n, ws);
        }
    }

    fn focused_monitor_id(&self) -> Option<MonitorId> {
        self.monitors.get(self.focused_monitor).map(|m| m.info.id)
    }

    /// Replaces the monitor set, keeping workspaces on monitors that still exist.
    pub fn set_monitors(&mut self, infos: Vec<MonitorInfo>) -> Vec<Effect> {
        if infos.is_empty() {
            return Vec::new();
        }
        let old = std::mem::take(&mut self.monitors);
        let present: HashSet<MonitorId> = infos.iter().map(|i| i.id).collect();
        let mut used: HashSet<u8> = HashSet::new();
        for info in infos {
            let previous = old
                .iter()
                .find(|m| m.info.id == info.id)
                .map(|m| m.workspace)
                .filter(|ws| !used.contains(ws));
            let ws = previous.unwrap_or_else(|| {
                let owned = self
                    .workspaces
                    .iter()
                    .find(|(n, w)| w.monitor == info.id && !used.contains(*n))
                    .map(|(n, _)| *n);
                owned.unwrap_or_else(|| {
                    (1..=u8::MAX)
                        .find(|n| {
                            !used.contains(n)
                                && self
                                    .workspaces
                                    .get(n)
                                    .is_none_or(|w| !present.contains(&w.monitor))
                        })
                        .unwrap_or(u8::MAX)
                })
            });
            used.insert(ws);
            self.ensure_workspace(ws, info.id);
            if let Some(w) = self.workspaces.get_mut(&ws) {
                w.monitor = info.id;
            }
            self.monitors.push(Monitor { info, workspace: ws });
        }
        let fallback = self.monitors[0].info.id;
        for ws in self.workspaces.values_mut() {
            if !present.contains(&ws.monitor) {
                ws.monitor = fallback;
            }
        }
        self.focused_monitor = self.focused_monitor.min(self.monitors.len() - 1);
        self.sync()
    }

    /// Recomputes every window's placement and visibility.
    pub fn sync(&mut self) -> Vec<Effect> {
        let Self { monitors, workspaces, windows, hidden, focused, settings, .. } = self;
        let mut hides = Vec::new();
        let mut places = Vec::new();
        let mut shows = Vec::new();
        for (n, ws) in workspaces.iter_mut() {
            let Some(monitor) = monitors.iter().find(|m| m.workspace == *n) else {
                for id in ws.all_windows() {
                    if hidden.insert(id) {
                        hides.push(Effect::Hide(id));
                    }
                }
                continue;
            };
            let focus = focused
                .filter(|f| windows.get(f).is_some_and(|m| m.workspace == *n))
                .or_else(|| ws.history.last().copied());
            let mut tiles = ws.layout.arrange(monitor.info.work_area, focus, settings);
            tiles.extend(ws.floating.iter().filter_map(|id| {
                windows.get(id).map(|m| Tile { id: *id, rect: m.float_rect, visible: true })
            }));
            for tile in tiles {
                if tile.visible {
                    let rect = if ws.fullscreen == Some(tile.id) {
                        monitor.info.rect
                    } else {
                        tile.rect
                    };
                    places.push(Effect::Place { id: tile.id, rect });
                    if hidden.remove(&tile.id) {
                        shows.push(Effect::Show(tile.id));
                    }
                } else if hidden.insert(tile.id) {
                    hides.push(Effect::Hide(tile.id));
                }
            }
            for id in &ws.minimized {
                if hidden.remove(id) {
                    shows.push(Effect::Show(*id));
                }
            }
        }
        hides.extend(places);
        hides.extend(shows);
        hides
    }

    fn focus_window(&mut self, id: WindowId) -> Vec<Effect> {
        let Some(n) = self.windows.get(&id).map(|m| m.workspace) else { return Vec::new() };
        if let Some(ws) = self.workspaces.get_mut(&n) {
            ws.touch(id);
        }
        if self.showing(n).is_none() {
            return self.focus_workspace(n);
        }
        if let Some(i) = self.showing(n) {
            self.focused_monitor = i;
        }
        self.focused = Some(id);
        let mut effects = self.sync();
        effects.push(Effect::Focus(id));
        effects
    }

    fn focus_workspace(&mut self, n: u8) -> Vec<Effect> {
        let Some(fallback) = self.focused_monitor_id() else { return Vec::new() };
        self.ensure_workspace(n, fallback);
        let owner = self.workspaces[&n].monitor;
        let index = self.monitor_index(owner).unwrap_or(self.focused_monitor);
        self.monitors[index].workspace = n;
        if let Some(ws) = self.workspaces.get_mut(&n) {
            ws.monitor = self.monitors[index].info.id;
        }
        self.focused_monitor = index;
        let target = self.workspaces[&n].focus_candidate();
        self.focused = target;
        let mut effects = self.sync();
        effects.extend(target.map(Effect::Focus));
        effects
    }

    fn focus_monitor(&mut self, dir: Direction) -> Vec<Effect> {
        let Some(current) = self.monitors.get(self.focused_monitor) else { return Vec::new() };
        let from = current.info.rect;
        let others = self
            .monitors
            .iter()
            .enumerate()
            .filter(|(i, _)| *i != self.focused_monitor)
            .map(|(i, m)| (i, m.info.rect));
        let Some(index) = nearest_in_direction(&from, dir, others) else { return Vec::new() };
        let n = self.monitors[index].workspace;
        self.focus_workspace(n)
    }

    fn monitor_in_direction(&self, from: usize, dir: Direction) -> Option<usize> {
        let rect = self.monitors.get(from)?.info.rect;
        nearest_in_direction(
            &rect,
            dir,
            self.monitors
                .iter()
                .enumerate()
                .filter(|(i, _)| *i != from)
                .map(|(i, m)| (i, m.info.rect)),
        )
    }

    /// Removes a window from its workspace structures, returning the workspace number.
    fn detach(&mut self, id: WindowId) -> Option<u8> {
        let n = self.windows.get(&id)?.workspace;
        if let Some(ws) = self.workspaces.get_mut(&n) {
            ws.forget(id);
        }
        Some(n)
    }

    /// Adds a managed window to workspace `n`, next to that workspace's focused window.
    fn attach(&mut self, id: WindowId, n: u8) {
        let area = self.area_of(n);
        let Some(m) = self.windows.get_mut(&id) else { return };
        let Some(ws) = self.workspaces.get_mut(&n) else { return };
        m.workspace = n;
        if m.floating {
            ws.floating.push(id);
        } else {
            let near = ws.history.last().copied();
            ws.layout.insert(id, near, area, &self.settings);
        }
        ws.touch(id);
    }

    fn centered(&self, size: Rect, n: u8) -> Rect {
        let area = self.area_of(n);
        let w = size.w.clamp(100, area.w.max(100));
        let h = size.h.clamp(100, area.h.max(100));
        Rect::new(area.x + (area.w - w) / 2, area.y + (area.h - h) / 2, w, h)
    }

    /// Starts managing a window. Returns no effects if it was already managed.
    pub fn window_opened(&mut self, id: WindowId, info: WindowInfo, opts: ManageOptions) -> Vec<Effect> {
        if self.windows.contains_key(&id) || self.monitors.is_empty() {
            return Vec::new();
        }
        let monitor = opts
            .monitor
            .filter(|i| *i < self.monitors.len())
            .unwrap_or(self.focused_monitor);
        let n = opts.workspace.unwrap_or(self.monitors[monitor].workspace);
        self.ensure_workspace(n, self.monitors[monitor].info.id);
        self.windows.insert(
            id,
            Managed { info, workspace: n, floating: opts.floating, float_rect: opts.rect },
        );
        self.attach(id, n);
        if opts.focus && self.showing(n).is_some() {
            return self.focus_window(id);
        }
        self.sync()
    }

    /// Stops managing a window (destroyed, hidden by its app, or became unmanageable).
    pub fn window_closed(&mut self, id: WindowId) -> Vec<Effect> {
        let Some(n) = self.detach(id) else { return Vec::new() };
        self.windows.remove(&id);
        self.hidden.remove(&id);
        if self.focused == Some(id) {
            self.focused = None;
            if let Some(next) = self.workspaces.get(&n).and_then(Workspace::focus_candidate)
                && self.showing(n).is_some()
            {
                return self.focus_window(next);
            }
        }
        self.sync()
    }

    /// The OS reported a new foreground window.
    pub fn window_focused(&mut self, id: WindowId) -> Vec<Effect> {
        let Some(n) = self.windows.get(&id).map(|m| m.workspace) else {
            self.focused = None;
            return Vec::new();
        };
        if self.focused == Some(id) && self.showing(n).is_some() {
            return Vec::new();
        }
        if let Some(ws) = self.workspaces.get_mut(&n) {
            ws.touch(id);
        }
        if self.showing(n).is_none() {
            return self.focus_workspace(n);
        }
        self.focused = Some(id);
        if let Some(i) = self.showing(n) {
            self.focused_monitor = i;
        }
        self.sync()
    }

    pub fn window_minimized(&mut self, id: WindowId) -> Vec<Effect> {
        let Some(n) = self.detach(id) else { return Vec::new() };
        if let Some(ws) = self.workspaces.get_mut(&n) {
            ws.minimized.push(id);
        }
        if self.focused == Some(id) {
            self.focused = None;
        }
        self.sync()
    }

    pub fn window_restored(&mut self, id: WindowId) -> Vec<Effect> {
        let Some(n) = self.windows.get(&id).map(|m| m.workspace) else { return Vec::new() };
        let Some(ws) = self.workspaces.get_mut(&n) else { return Vec::new() };
        if !ws.minimized.contains(&id) {
            return Vec::new();
        }
        ws.minimized.retain(|w| *w != id);
        self.attach(id, n);
        self.focus_window(id)
    }

    /// The user finished dragging or resizing a window with the mouse.
    pub fn window_moved(&mut self, id: WindowId, rect: Rect, cursor: (i32, i32)) -> Vec<Effect> {
        let Some(m) = self.windows.get(&id) else { return Vec::new() };
        let n = m.workspace;
        let floating = m.floating;
        let target_monitor = self
            .monitors
            .iter()
            .position(|m| m.info.rect.contains_point(cursor.0, cursor.1));
        let current_monitor = self.showing(n);

        if floating && let Some(m) = self.windows.get_mut(&id) {
            m.float_rect = rect;
        }
        if let (Some(target), Some(current)) = (target_monitor, current_monitor)
            && target != current
        {
            let dest = self.monitors[target].workspace;
            self.detach(id);
            self.attach(id, dest);
            return self.focus_window(id);
        }
        if floating {
            return self.sync();
        }

        let area = self.area_of(n);
        let settings = self.settings.clone();
        let Some(ws) = self.workspaces.get_mut(&n) else { return Vec::new() };
        let tiles = ws.layout.arrange(area, Some(id), &settings);
        let under = tiles
            .iter()
            .find(|t| t.visible && t.id != id && t.rect.contains_point(cursor.0, cursor.1));
        if let Some(other) = under {
            ws.layout.swap(id, other.id);
        } else if let Some(own) = tiles.iter().find(|t| t.id == id) {
            let dx = rect.w - own.rect.w;
            let dy = rect.h - own.rect.h;
            if dx.abs() > 2 || dy.abs() > 2 {
                ws.layout.resize(id, dx, dy, area, &settings);
            }
        }
        self.sync()
    }

    /// Executes a dispatcher against the focused window / workspace.
    pub fn apply(&mut self, command: &Command) -> Vec<Effect> {
        if self.monitors.is_empty() {
            return Vec::new();
        }
        match command {
            Command::Exec(cmd) => vec![Effect::Exec(cmd.clone())],
            Command::KillActive => self.focused.map(Effect::Close).into_iter().collect(),
            Command::ToggleFloating => self.toggle_floating(),
            Command::Fullscreen => self.toggle_fullscreen(),
            Command::MoveFocus(dir) => self.move_focus(*dir),
            Command::MoveWindow(dir) => self.move_window(*dir),
            Command::ResizeActive(dx, dy) => self.resize_active(*dx, *dy),
            Command::Workspace(n) => self.focus_workspace(*n),
            Command::MoveToWorkspace(n) => self.move_to_workspace(*n, true),
            Command::MoveToWorkspaceSilent(n) => self.move_to_workspace(*n, false),
            Command::FocusMonitor(dir) => self.focus_monitor(*dir),
            Command::Layout(kind) => self.set_layout(*kind),
            Command::CycleLayout => {
                let next = self.active_layout().unwrap_or(self.default_layout).next();
                self.set_layout(next)
            }
            Command::SwapWithMaster
            | Command::ToggleSplit
            | Command::ScrollerProportion(_)
            | Command::CycleProportion
            | Command::ConsumeWindow(_)
            | Command::ExpelWindow
            | Command::CenterColumn => self.layout_specific(command),
            // Handled by the daemon.
            Command::Desktop(_)
            | Command::MoveToDesktop(_)
            | Command::Pin
            | Command::Reload
            | Command::TogglePause
            | Command::Exit => Vec::new(),
        }
    }

    /// The focused window if it lives on the focused workspace.
    fn active(&self) -> Option<(WindowId, u8)> {
        let id = self.focused?;
        let n = self.windows.get(&id)?.workspace;
        Some((id, n))
    }

    fn toggle_floating(&mut self) -> Vec<Effect> {
        let Some((id, n)) = self.active() else { return Vec::new() };
        let float_rect = self.windows[&id].float_rect;
        let centered = self.centered(float_rect, n);
        let area = self.area_of(n);
        let Some(m) = self.windows.get_mut(&id) else { return Vec::new() };
        let Some(ws) = self.workspaces.get_mut(&n) else { return Vec::new() };
        if m.floating {
            m.floating = false;
            ws.floating.retain(|w| *w != id);
            let near = ws.history.iter().rev().copied().find(|w| ws.layout.contains(*w));
            ws.layout.insert(id, near, area, &self.settings);
        } else {
            m.floating = true;
            m.float_rect = centered;
            ws.layout.remove(id);
            ws.floating.push(id);
        }
        let mut effects = self.sync();
        effects.push(Effect::Focus(id));
        effects
    }

    fn toggle_fullscreen(&mut self) -> Vec<Effect> {
        let Some((id, n)) = self.active() else { return Vec::new() };
        let Some(ws) = self.workspaces.get_mut(&n) else { return Vec::new() };
        ws.fullscreen = if ws.fullscreen == Some(id) { None } else { Some(id) };
        let mut effects = self.sync();
        effects.push(Effect::Focus(id));
        effects
    }

    fn move_focus(&mut self, dir: Direction) -> Vec<Effect> {
        let Some(n) = self.focused_workspace() else { return Vec::new() };
        let current = self.active().filter(|(_, ws)| *ws == n).map(|(id, _)| id);
        let Some(id) = current else {
            // Nothing focused here (e.g. the desktop has focus): start from a window.
            return match self.workspaces.get(&n).and_then(Workspace::focus_candidate) {
                Some(candidate) => self.focus_window(candidate),
                None => self.focus_monitor(dir),
            };
        };
        let area = self.area_of(n);
        let settings = self.settings.clone();
        let Some(ws) = self.workspaces.get_mut(&n) else { return Vec::new() };
        let mut tiles = ws.layout.arrange(area, Some(id), &settings);
        let target = if ws.floating.contains(&id) {
            tiles.extend(ws.floating.iter().filter_map(|f| {
                self.windows.get(f).map(|m| Tile { id: *f, rect: m.float_rect, visible: true })
            }));
            let from = self.windows[&id].float_rect;
            nearest_in_direction(
                &from,
                dir,
                tiles.iter().filter(|t| t.visible && t.id != id).map(|t| (t.id, t.rect)),
            )
        } else {
            ws.layout.neighbor(id, dir, &tiles)
        };
        match target {
            Some(t) => self.focus_window(t),
            None => self.focus_monitor(dir),
        }
    }

    fn move_window(&mut self, dir: Direction) -> Vec<Effect> {
        let Some((id, n)) = self.active() else { return Vec::new() };
        let area = self.area_of(n);
        if self.is_floating(id) {
            let step_x = scale(area.w, 0.05);
            let step_y = scale(area.h, 0.05);
            if let Some(m) = self.windows.get_mut(&id) {
                match dir {
                    Direction::Left => m.float_rect.x -= step_x,
                    Direction::Right => m.float_rect.x += step_x,
                    Direction::Up => m.float_rect.y -= step_y,
                    Direction::Down => m.float_rect.y += step_y,
                }
            }
            return self.sync();
        }
        let settings = self.settings.clone();
        let Some(ws) = self.workspaces.get_mut(&n) else { return Vec::new() };
        let tiles = ws.layout.arrange(area, Some(id), &settings);
        if ws.layout.move_window(id, dir, &tiles) {
            return self.sync();
        }
        let Some(from) = self.showing(n) else { return Vec::new() };
        let Some(target) = self.monitor_in_direction(from, dir) else { return Vec::new() };
        let dest = self.monitors[target].workspace;
        self.detach(id);
        self.attach(id, dest);
        self.focus_window(id)
    }

    fn resize_active(&mut self, dx: i32, dy: i32) -> Vec<Effect> {
        let Some((id, n)) = self.active() else { return Vec::new() };
        if self.is_floating(id) {
            if let Some(m) = self.windows.get_mut(&id) {
                m.float_rect.w = (m.float_rect.w + dx).max(100);
                m.float_rect.h = (m.float_rect.h + dy).max(100);
            }
            return self.sync();
        }
        let area = self.area_of(n);
        let settings = self.settings.clone();
        if let Some(ws) = self.workspaces.get_mut(&n) {
            ws.layout.resize(id, dx, dy, area, &settings);
        }
        self.sync()
    }

    fn move_to_workspace(&mut self, n: u8, follow: bool) -> Vec<Effect> {
        let Some((id, current)) = self.active() else { return Vec::new() };
        if current == n {
            return Vec::new();
        }
        let Some(monitor) = self.focused_monitor_id() else { return Vec::new() };
        self.ensure_workspace(n, monitor);
        let old_area = self.area_of(current);
        let new_area = self.area_of(n);
        self.detach(id);
        if let Some(m) = self.windows.get_mut(&id) {
            m.float_rect.x += new_area.x - old_area.x;
            m.float_rect.y += new_area.y - old_area.y;
        }
        self.attach(id, n);
        if follow {
            return self.focus_workspace(n);
        }
        self.focused = None;
        match self.workspaces.get(&current).and_then(Workspace::focus_candidate) {
            Some(next) => self.focus_window(next),
            None => self.sync(),
        }
    }

    fn set_layout(&mut self, kind: LayoutKind) -> Vec<Effect> {
        let Some(n) = self.focused_workspace() else { return Vec::new() };
        let area = self.area_of(n);
        if let Some(ws) = self.workspaces.get_mut(&n) {
            ws.layout = ws.layout.convert(kind, area, &self.settings);
        }
        self.sync()
    }

    fn layout_specific(&mut self, command: &Command) -> Vec<Effect> {
        let Some((id, n)) = self.active() else { return Vec::new() };
        let area = self.area_of(n);
        let s = &self.settings;
        let Some(ws) = self.workspaces.get_mut(&n) else { return Vec::new() };
        match (&mut ws.layout, command) {
            (LayoutState::Master(m), Command::SwapWithMaster) => m.swap_with_master(id, s),
            (LayoutState::Dwindle(d), Command::ToggleSplit) => d.toggle_split(id, area, s),
            (LayoutState::Scrolling(l), Command::ScrollerProportion(p)) => l.set_proportion(id, *p),
            (LayoutState::Scrolling(l), Command::CycleProportion) => l.cycle_proportion(id, s),
            (LayoutState::Scrolling(l), Command::ConsumeWindow(dir)) => l.consume(id, *dir),
            (LayoutState::Scrolling(l), Command::ExpelWindow) => l.expel(id, s),
            (LayoutState::Scrolling(l), Command::CenterColumn) => l.center(),
            _ => return Vec::new(),
        }
        self.sync()
    }

    pub fn snapshot_windows(&self) -> Vec<WindowSnapshot> {
        let mut out: Vec<_> = self
            .windows
            .iter()
            .map(|(id, m)| WindowSnapshot {
                id: id.0,
                title: m.info.title.clone(),
                class: m.info.class.clone(),
                exe: m.info.exe.clone(),
                workspace: m.workspace,
                floating: m.floating,
                fullscreen: self.workspaces.get(&m.workspace).and_then(|w| w.fullscreen) == Some(*id),
                focused: self.focused == Some(*id),
                hidden: self.hidden.contains(id),
            })
            .collect();
        out.sort_by_key(|w| (w.workspace, w.id));
        out
    }

    pub fn snapshot_active_window(&self) -> Option<WindowSnapshot> {
        let id = self.focused?;
        self.snapshot_windows().into_iter().find(|w| w.id == id.0)
    }

    pub fn snapshot_workspaces(&self) -> Vec<WorkspaceSnapshot> {
        let focused_ws = self.focused_workspace();
        self.workspaces
            .iter()
            .map(|(n, ws)| WorkspaceSnapshot {
                id: *n,
                monitor: self.monitor_index(ws.monitor),
                visible: self.showing(*n).is_some(),
                focused: focused_ws == Some(*n),
                layout: ws.layout.kind(),
                windows: ws.len(),
            })
            .collect()
    }

    pub fn snapshot_monitors(&self) -> Vec<MonitorSnapshot> {
        self.monitors
            .iter()
            .enumerate()
            .map(|(i, m)| MonitorSnapshot {
                index: i,
                rect: m.info.rect,
                work_area: m.info.work_area,
                workspace: m.workspace,
                focused: i == self.focused_monitor,
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const W: WindowId = WindowId(1);
    const X: WindowId = WindowId(2);
    const Y: WindowId = WindowId(3);

    fn monitor(id: isize, x: i32) -> MonitorInfo {
        MonitorInfo {
            id: MonitorId(id),
            rect: Rect::new(x, 0, 1000, 1000),
            work_area: Rect::new(x, 0, 1000, 960),
        }
    }

    fn state(monitors: usize) -> State {
        let settings = LayoutSettings { gaps_in: 0, gaps_out: 0, ..LayoutSettings::default() };
        let mut s = State::new(settings, LayoutKind::Dwindle);
        let infos = (0..monitors).map(|i| monitor(i as isize + 1, i as i32 * 1000)).collect();
        s.set_monitors(infos);
        s
    }

    fn open(s: &mut State, id: WindowId) -> Vec<Effect> {
        s.window_opened(id, WindowInfo::default(), ManageOptions { focus: true, ..Default::default() })
    }

    fn placed(effects: &[Effect], id: WindowId) -> Option<Rect> {
        effects.iter().rev().find_map(|e| match e {
            Effect::Place { id: i, rect } if *i == id => Some(*rect),
            _ => None,
        })
    }

    #[test]
    fn monitors_get_distinct_workspaces() {
        let s = state(2);
        let ws: Vec<_> = s.snapshot_monitors().iter().map(|m| m.workspace).collect();
        assert_eq!(ws, vec![1, 2]);
    }

    #[test]
    fn opening_tiles_and_focuses() {
        let mut s = state(1);
        open(&mut s, W);
        let effects = open(&mut s, X);
        assert_eq!(placed(&effects, W), Some(Rect::new(0, 0, 500, 960)));
        assert_eq!(placed(&effects, X), Some(Rect::new(500, 0, 500, 960)));
        assert_eq!(s.focused(), Some(X));
    }

    #[test]
    fn workspace_switch_hides_and_shows() {
        let mut s = state(1);
        open(&mut s, W);
        let effects = s.apply(&Command::Workspace(2));
        assert!(effects.contains(&Effect::Hide(W)));
        assert_eq!(s.focused(), None);
        let effects = s.apply(&Command::Workspace(1));
        assert!(effects.contains(&Effect::Show(W)));
        assert!(effects.contains(&Effect::Focus(W)));
    }

    #[test]
    fn move_to_workspace_silent_keeps_view() {
        let mut s = state(1);
        open(&mut s, W);
        open(&mut s, X);
        let effects = s.apply(&Command::MoveToWorkspaceSilent(3));
        assert!(effects.contains(&Effect::Hide(X)));
        assert_eq!(placed(&effects, W), Some(Rect::new(0, 0, 1000, 960)));
        assert_eq!(s.focused(), Some(W));
        assert_eq!(s.focused_workspace(), Some(1));
    }

    #[test]
    fn focus_crosses_monitors() {
        let mut s = state(2);
        open(&mut s, W);
        let effects = s.apply(&Command::MoveFocus(Direction::Right));
        assert_eq!(s.focused_workspace(), Some(2));
        assert!(!effects.iter().any(|e| matches!(e, Effect::Focus(_))));
        s.apply(&Command::MoveFocus(Direction::Left));
        assert_eq!(s.focused(), Some(W));
    }

    #[test]
    fn move_window_to_other_monitor_at_edge() {
        let mut s = state(2);
        open(&mut s, W);
        let effects = s.apply(&Command::MoveWindow(Direction::Right));
        assert_eq!(placed(&effects, W), Some(Rect::new(1000, 0, 1000, 960)));
        assert_eq!(s.focused_workspace(), Some(2));
    }

    #[test]
    fn closing_focuses_previous() {
        let mut s = state(1);
        open(&mut s, W);
        open(&mut s, X);
        open(&mut s, Y);
        s.window_focused(X);
        let effects = s.window_closed(X);
        assert!(effects.contains(&Effect::Focus(Y)));
    }

    #[test]
    fn floating_and_fullscreen() {
        let mut s = state(1);
        open(&mut s, W);
        open(&mut s, X);
        let effects = s.apply(&Command::ToggleFloating);
        assert_eq!(placed(&effects, W), Some(Rect::new(0, 0, 1000, 960)));
        assert!(s.is_floating(X));
        s.window_focused(W);
        let effects = s.apply(&Command::Fullscreen);
        assert_eq!(placed(&effects, W), Some(Rect::new(0, 0, 1000, 1000)));
    }

    #[test]
    fn minimize_and_restore() {
        let mut s = state(1);
        open(&mut s, W);
        open(&mut s, X);
        let effects = s.window_minimized(X);
        assert_eq!(placed(&effects, W), Some(Rect::new(0, 0, 1000, 960)));
        let effects = s.window_restored(X);
        assert_eq!(placed(&effects, X), Some(Rect::new(500, 0, 500, 960)));
    }

    #[test]
    fn scrolling_hides_offscreen_columns() {
        let mut s = state(1);
        s.apply(&Command::Layout(LayoutKind::Scrolling));
        open(&mut s, W);
        open(&mut s, X);
        let effects = open(&mut s, Y);
        assert!(effects.contains(&Effect::Hide(W)));
        let effects = s.apply(&Command::MoveFocus(Direction::Left));
        let effects = [effects, s.apply(&Command::MoveFocus(Direction::Left))].concat();
        assert!(effects.contains(&Effect::Show(W)));
        assert_eq!(s.focused(), Some(W));
    }

    #[test]
    fn monitor_removal_keeps_windows() {
        let mut s = state(2);
        s.apply(&Command::FocusMonitor(Direction::Right));
        open(&mut s, W);
        s.set_monitors(vec![monitor(1, 0)]);
        assert_eq!(s.monitor_count(), 1);
        let effects = s.apply(&Command::Workspace(2));
        assert_eq!(placed(&effects, W), Some(Rect::new(0, 0, 1000, 960)));
    }

    #[test]
    fn drag_onto_other_tile_swaps() {
        let mut s = state(1);
        open(&mut s, W);
        open(&mut s, X);
        let effects = s.window_moved(W, Rect::new(600, 10, 500, 960), (700, 100));
        assert_eq!(placed(&effects, W), Some(Rect::new(500, 0, 500, 960)));
    }
}
