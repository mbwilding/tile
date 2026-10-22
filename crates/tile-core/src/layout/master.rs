use super::{Layout, LayoutSettings, Tile};
use crate::{
    WindowId,
    geometry::{Axis, Rect, ratio_of},
};

/// Master/stack: `count` master windows on the left, the rest stacked on the right.
#[derive(Debug, Clone)]
pub struct Master {
    windows: Vec<WindowId>,
    mfact: f32,
}

impl Master {
    pub fn new(s: &LayoutSettings) -> Self {
        Self { windows: Vec::new(), mfact: s.master_mfact }
    }

    pub fn swap_with_master(&mut self, id: WindowId, s: &LayoutSettings) {
        let Some(pos) = self.windows.iter().position(|w| *w == id) else { return };
        if pos < s.master_count.max(1) {
            if self.windows.len() > s.master_count {
                self.windows.swap(pos, s.master_count);
            }
        } else {
            self.windows.swap(pos, 0);
        }
    }
}

impl Layout for Master {
    fn insert(&mut self, id: WindowId, _focused: Option<WindowId>, _area: Rect, s: &LayoutSettings) {
        if s.master_new_is_master {
            self.windows.insert(0, id);
        } else {
            self.windows.push(id);
        }
    }

    fn remove(&mut self, id: WindowId) {
        self.windows.retain(|w| *w != id);
    }

    fn windows(&self) -> Vec<WindowId> {
        self.windows.clone()
    }

    fn arrange(&mut self, area: Rect, _focused: Option<WindowId>, s: &LayoutSettings) -> Vec<Tile> {
        let area = area.shrink(s.gaps_out);
        let count = s.master_count.max(1);
        let column = |rect: Rect, ids: &[WindowId]| -> Vec<Tile> {
            rect.split_weighted(Axis::Vertical, &vec![1.0; ids.len()], s.gaps_in)
                .into_iter()
                .zip(ids)
                .map(|(rect, id)| Tile { id: *id, rect, visible: true })
                .collect()
        };
        if self.windows.len() <= count {
            return column(area, &self.windows);
        }
        let (master_area, stack_area) = area.split(Axis::Horizontal, self.mfact, s.gaps_in);
        let (masters, stack) = self.windows.split_at(count);
        let mut tiles = column(master_area, masters);
        tiles.extend(column(stack_area, stack));
        tiles
    }

    fn swap(&mut self, a: WindowId, b: WindowId) {
        let pa = self.windows.iter().position(|w| *w == a);
        let pb = self.windows.iter().position(|w| *w == b);
        if let (Some(pa), Some(pb)) = (pa, pb) {
            self.windows.swap(pa, pb);
        }
    }

    fn resize(&mut self, id: WindowId, dx: i32, _dy: i32, area: Rect, s: &LayoutSettings) {
        let Some(pos) = self.windows.iter().position(|w| *w == id) else { return };
        let change = ratio_of(dx, area.shrink(s.gaps_out).w);
        let signed = if pos < s.master_count.max(1) { change } else { -change };
        self.mfact = (self.mfact + signed).clamp(0.1, 0.9);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn master_and_stack() {
        let s = LayoutSettings { gaps_in: 0, gaps_out: 0, master_mfact: 0.5, ..LayoutSettings::default() };
        let area = Rect::new(0, 0, 1000, 900);
        let mut m = Master::new(&s);
        for id in 1..=4 {
            m.insert(WindowId(id), None, area, &s);
        }
        let tiles = m.arrange(area, None, &s);
        assert_eq!(tiles[0].rect, Rect::new(0, 0, 500, 900));
        assert_eq!(tiles[1].rect, Rect::new(500, 0, 500, 300));
        assert_eq!(tiles[3].rect, Rect::new(500, 600, 500, 300));

        m.swap_with_master(WindowId(3), &s);
        assert_eq!(m.windows()[0], WindowId(3));

        m.resize(WindowId(3), 100, 0, area, &s);
        assert_eq!(m.arrange(area, None, &s)[0].rect.w, 600);
    }
}
