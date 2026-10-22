use super::{Layout, LayoutSettings, Tile};
use crate::{
    WindowId,
    geometry::{Direction, Rect},
};

/// Every window fills the workspace; focus cycles through them.
#[derive(Debug, Clone, Default)]
pub struct Monocle {
    windows: Vec<WindowId>,
}

impl Layout for Monocle {
    fn insert(&mut self, id: WindowId, _focused: Option<WindowId>, _area: Rect, _s: &LayoutSettings) {
        self.windows.push(id);
    }

    fn remove(&mut self, id: WindowId) {
        self.windows.retain(|w| *w != id);
    }

    fn windows(&self) -> Vec<WindowId> {
        self.windows.clone()
    }

    fn arrange(&mut self, area: Rect, _focused: Option<WindowId>, s: &LayoutSettings) -> Vec<Tile> {
        let rect = area.shrink(s.gaps_out);
        self.windows.iter().map(|id| Tile { id: *id, rect, visible: true }).collect()
    }

    fn swap(&mut self, a: WindowId, b: WindowId) {
        let pa = self.windows.iter().position(|w| *w == a);
        let pb = self.windows.iter().position(|w| *w == b);
        if let (Some(pa), Some(pb)) = (pa, pb) {
            self.windows.swap(pa, pb);
        }
    }

    fn resize(&mut self, _id: WindowId, _dx: i32, _dy: i32, _area: Rect, _s: &LayoutSettings) {}

    fn neighbor(&self, id: WindowId, dir: Direction, _tiles: &[Tile]) -> Option<WindowId> {
        let len = self.windows.len();
        let pos = self.windows.iter().position(|w| *w == id)?;
        if len < 2 {
            return None;
        }
        let next = if dir.is_backward() { (pos + len - 1) % len } else { (pos + 1) % len };
        Some(self.windows[next])
    }
}
