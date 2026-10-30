use super::{Layout, LayoutSettings, Tile};
use crate::{
    WindowId,
    geometry::{Axis, Rect},
};

/// Every window gets an equal share, side by side on landscape and stacked on portrait.
#[derive(Debug, Clone, Default)]
pub struct Even {
    windows: Vec<WindowId>,
}

impl Layout for Even {
    fn insert(
        &mut self,
        id: WindowId,
        focused: Option<WindowId>,
        _area: Rect,
        _s: &LayoutSettings,
    ) {
        let at = focused
            .and_then(|f| self.windows.iter().position(|w| *w == f))
            .map_or(self.windows.len(), |i| i + 1);
        self.windows.insert(at, id);
    }

    fn remove(&mut self, id: WindowId) {
        self.windows.retain(|w| *w != id);
    }

    fn windows(&self) -> Vec<WindowId> {
        self.windows.clone()
    }

    fn arrange(&mut self, area: Rect, _focused: Option<WindowId>, s: &LayoutSettings) -> Vec<Tile> {
        let area = area.shrink(s.gaps_out);
        let axis = Axis::for_rect(&area);
        area.split_weighted(axis, &vec![1.0; self.windows.len()], s.gaps_in)
            .into_iter()
            .zip(&self.windows)
            .map(|(rect, id)| Tile {
                id: *id,
                rect,
                visible: true,
            })
            .collect()
    }

    fn swap(&mut self, a: WindowId, b: WindowId) {
        let pa = self.windows.iter().position(|w| *w == a);
        let pb = self.windows.iter().position(|w| *w == b);
        if let (Some(pa), Some(pb)) = (pa, pb) {
            self.windows.swap(pa, pb);
        }
    }

    fn resize(&mut self, _id: WindowId, _dx: i32, _dy: i32, _area: Rect, _s: &LayoutSettings) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    fn filled(area: Rect, n: isize) -> Vec<Tile> {
        let s = LayoutSettings {
            gaps_in: 0,
            gaps_out: 0,
            ..LayoutSettings::default()
        };
        let mut l = Even::default();
        let mut prev = None;
        for id in 1..=n {
            l.insert(WindowId(id), prev, area, &s);
            prev = Some(WindowId(id));
        }
        l.arrange(area, None, &s)
    }

    #[test]
    fn portrait_stacks_equal_rows() {
        let tiles = filled(Rect::new(0, 0, 1080, 1920), 3);
        assert_eq!(tiles[0].rect, Rect::new(0, 0, 1080, 640));
        assert_eq!(tiles[1].rect, Rect::new(0, 640, 1080, 640));
        assert_eq!(tiles[2].rect, Rect::new(0, 1280, 1080, 640));
    }

    #[test]
    fn landscape_sits_in_equal_columns() {
        let tiles = filled(Rect::new(0, 0, 1800, 900), 3);
        assert_eq!(tiles[1].rect, Rect::new(600, 0, 600, 900));
    }
}
