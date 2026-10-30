mod dwindle;
mod even;
mod master;
mod monocle;
mod scrolling;

use std::{fmt, str::FromStr};

use serde::{Deserialize, Serialize};

pub use dwindle::Dwindle;
pub use even::Even;
pub use master::Master;
pub use monocle::Monocle;
pub use scrolling::Scrolling;

use crate::{
    WindowId,
    geometry::{Direction, Rect, nearest_in_direction},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LayoutKind {
    Dwindle,
    Master,
    Monocle,
    Scrolling,
    Even,
}

impl LayoutKind {
    pub const ALL: [Self; 5] = [
        Self::Dwindle,
        Self::Master,
        Self::Monocle,
        Self::Scrolling,
        Self::Even,
    ];

    pub fn next(self) -> Self {
        let i = Self::ALL.iter().position(|k| *k == self).unwrap_or(0);
        Self::ALL[(i + 1) % Self::ALL.len()]
    }
}

impl FromStr for LayoutKind {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "dwindle" => Ok(Self::Dwindle),
            "master" => Ok(Self::Master),
            "monocle" => Ok(Self::Monocle),
            "even" => Ok(Self::Even),
            "scrolling" | "scroller" => Ok(Self::Scrolling),
            _ => Err(()),
        }
    }
}

impl fmt::Display for LayoutKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Dwindle => "dwindle",
            Self::Master => "master",
            Self::Monocle => "monocle",
            Self::Even => "even",
            Self::Scrolling => "scrolling",
        })
    }
}

/// When the scrolling layout re-centers the focused column.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CenterMode {
    /// Keep the focused column centered at all times.
    Always,
    /// Center the focused column whenever it would otherwise be (partly) off screen.
    #[default]
    OnOverflow,
    /// Scroll just enough to bring the focused column into view.
    Never,
}

/// Layout tunables, derived from the user config.
#[derive(Debug, Clone, PartialEq)]
pub struct LayoutSettings {
    /// Space between neighbouring windows.
    pub gaps_in: i32,
    /// Space between windows and the edge of the area.
    pub gaps_out: i32,
    pub dwindle_split_ratio: f32,
    pub master_mfact: f32,
    pub master_count: usize,
    pub master_new_is_master: bool,
    pub scrolling_proportion: f32,
    pub scrolling_presets: Vec<f32>,
    pub scrolling_center: CenterMode,
    /// Each monitor has workspaces of its own. Off, switching workspace switches every monitor.
    pub independent_workspaces: bool,
}

impl Default for LayoutSettings {
    fn default() -> Self {
        Self {
            gaps_in: 10,
            gaps_out: 10,
            dwindle_split_ratio: 0.5,
            master_mfact: 0.55,
            master_count: 1,
            master_new_is_master: false,
            scrolling_proportion: 0.5,
            scrolling_presets: vec![1.0 / 3.0, 0.5, 2.0 / 3.0, 1.0],
            scrolling_center: CenterMode::OnOverflow,
            independent_workspaces: true,
        }
    }
}

/// A window's computed position. Invisible tiles (scrolled off screen) must be hidden.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tile {
    pub id: WindowId,
    pub rect: Rect,
    pub visible: bool,
}

/// Behaviour common to every tiling layout.
pub trait Layout {
    /// Adds a window, placing it relative to `focused` when that is part of the layout.
    fn insert(&mut self, id: WindowId, focused: Option<WindowId>, area: Rect, s: &LayoutSettings);
    fn remove(&mut self, id: WindowId);
    /// All windows in a stable, layout-defined order.
    fn windows(&self) -> Vec<WindowId>;
    fn arrange(&mut self, area: Rect, focused: Option<WindowId>, s: &LayoutSettings) -> Vec<Tile>;
    fn swap(&mut self, a: WindowId, b: WindowId);
    fn resize(&mut self, id: WindowId, dx: i32, dy: i32, area: Rect, s: &LayoutSettings);

    fn contains(&self, id: WindowId) -> bool {
        self.windows().contains(&id)
    }

    fn neighbor(&self, id: WindowId, dir: Direction, tiles: &[Tile]) -> Option<WindowId> {
        let from = tiles.iter().find(|t| t.id == id)?.rect;
        nearest_in_direction(
            &from,
            dir,
            tiles.iter().filter(|t| t.id != id).map(|t| (t.id, t.rect)),
        )
    }

    /// Moves a window within the layout. Returns `false` when it is already at the edge,
    /// letting the caller move it to another monitor instead.
    fn move_window(&mut self, id: WindowId, dir: Direction, tiles: &[Tile]) -> bool {
        match self.neighbor(id, dir, tiles) {
            Some(other) => {
                self.swap(id, other);
                true
            }
            None => false,
        }
    }
}

/// The active layout of a workspace.
#[derive(Debug, Clone)]
pub enum LayoutState {
    Dwindle(Dwindle),
    Master(Master),
    Monocle(Monocle),
    Scrolling(Scrolling),
    Even(Even),
}

impl LayoutState {
    pub fn new(kind: LayoutKind, s: &LayoutSettings) -> Self {
        match kind {
            LayoutKind::Dwindle => Self::Dwindle(Dwindle::default()),
            LayoutKind::Master => Self::Master(Master::new(s)),
            LayoutKind::Monocle => Self::Monocle(Monocle::default()),
            LayoutKind::Scrolling => Self::Scrolling(Scrolling::default()),
            LayoutKind::Even => Self::Even(Even::default()),
        }
    }

    pub const fn kind(&self) -> LayoutKind {
        match self {
            Self::Dwindle(_) => LayoutKind::Dwindle,
            Self::Master(_) => LayoutKind::Master,
            Self::Monocle(_) => LayoutKind::Monocle,
            Self::Scrolling(_) => LayoutKind::Scrolling,
            Self::Even(_) => LayoutKind::Even,
        }
    }

    /// Rebuilds the layout as `kind`, keeping the window order.
    pub fn convert(&self, kind: LayoutKind, area: Rect, s: &LayoutSettings) -> Self {
        let mut next = Self::new(kind, s);
        let mut prev = None;
        for id in self.windows() {
            next.insert(id, prev, area, s);
            prev = Some(id);
        }
        next
    }

    fn inner(&self) -> &dyn Layout {
        match self {
            Self::Dwindle(l) => l,
            Self::Master(l) => l,
            Self::Monocle(l) => l,
            Self::Scrolling(l) => l,
            Self::Even(l) => l,
        }
    }

    fn inner_mut(&mut self) -> &mut dyn Layout {
        match self {
            Self::Dwindle(l) => l,
            Self::Master(l) => l,
            Self::Monocle(l) => l,
            Self::Scrolling(l) => l,
            Self::Even(l) => l,
        }
    }

    pub fn insert(
        &mut self,
        id: WindowId,
        focused: Option<WindowId>,
        area: Rect,
        s: &LayoutSettings,
    ) {
        self.inner_mut().insert(id, focused, area, s);
    }

    pub fn remove(&mut self, id: WindowId) {
        self.inner_mut().remove(id);
    }

    pub fn windows(&self) -> Vec<WindowId> {
        self.inner().windows()
    }

    pub fn contains(&self, id: WindowId) -> bool {
        self.inner().contains(id)
    }

    pub fn arrange(
        &mut self,
        area: Rect,
        focused: Option<WindowId>,
        s: &LayoutSettings,
    ) -> Vec<Tile> {
        self.inner_mut().arrange(area, focused, s)
    }

    pub fn swap(&mut self, a: WindowId, b: WindowId) {
        self.inner_mut().swap(a, b);
    }

    pub fn resize(&mut self, id: WindowId, dx: i32, dy: i32, area: Rect, s: &LayoutSettings) {
        self.inner_mut().resize(id, dx, dy, area, s);
    }

    pub fn neighbor(&self, id: WindowId, dir: Direction, tiles: &[Tile]) -> Option<WindowId> {
        self.inner().neighbor(id, dir, tiles)
    }

    pub fn move_window(&mut self, id: WindowId, dir: Direction, tiles: &[Tile]) -> bool {
        self.inner_mut().move_window(id, dir, tiles)
    }
}
