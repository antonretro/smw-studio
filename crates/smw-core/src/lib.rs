//! SMW Studio core.
//!
//! `emu.rs` and `rom.rs` come from smw-editor (MIT, see crates/LICENSE-smw-editor).
//! Levels are loaded by running SMW's own loader routines on an emulated 65816,
//! so ROMs edited by Lunar Magic (whose changes live in the ROM's code) load too.

pub mod emu;
pub mod framehook;
pub mod level_data;
pub mod hackparts;
pub mod lunar;
pub mod midi;
pub mod music;
pub mod palette;
pub mod patches;
pub mod project;
pub mod render;
pub mod rom;
pub mod amk;
pub mod names;
pub mod overworld;
pub mod sa1;
pub mod statusbar;
pub mod romfile;
pub mod sprite_data;
pub mod sprites;
pub mod toolbox;
pub mod upgrades;

pub type Cpu = wdc65816::Cpu<emu::CheckedMem>;
