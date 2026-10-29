//! In-editor play for SMW Studio: a libretro SNES core (Snes9x) driven from Rust.

pub mod audio;
pub mod libretro;
pub mod session;

pub use session::{Event, Options, Session};
