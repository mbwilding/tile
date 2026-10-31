//! A plain black desktop, for when no Explorer is there to paint one: a black window at the
//! bottom of the stack on each monitor. Tile never manages them, and they are gone when the
//! [`Backdrop`] is dropped.

use crate::monitors::monitors;

/// The black windows, kept to the monitors.
pub struct Backdrop(tile_hook::backdrop::Backdrop);

fn areas() -> Vec<tile_hook::backdrop::Area> {
    monitors()
        .iter()
        .map(|m| [m.rect.x, m.rect.y, m.rect.w, m.rect.h])
        .collect()
}

impl Backdrop {
    /// Covers every monitor in black. `None` if the windows couldn't be made.
    pub fn black() -> Option<Self> {
        tile_hook::backdrop::Backdrop::start(areas())
            .inspect_err(|e| tracing::warn!("couldn't make the desktop black: {e}"))
            .ok()
            .map(Self)
    }

    /// Fits the windows to the monitors, after they changed.
    pub fn refresh(&self) {
        self.0.set_areas(areas());
    }
}
