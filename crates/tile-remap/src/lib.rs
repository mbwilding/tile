//! Tile's key remapping engine: keyd's layers and overloads as a pure state machine, fed physical
//! key events and returning what to send. No Windows code lives here.

mod config;
mod engine;
mod keys;

pub use config::{Action, Chord, Config, Layer, LayerId, MAIN};
pub use engine::{Engine, KeyEvent, Out, Verdict};
pub use keys::{Key, Mods, shifted};
