//! Opening a ROM file: strip copier header, identify the version.

use std::sync::Arc;

use crate::rom::Rom;

/// SHA-1 of the clean, headerless Super Mario World (USA) ROM.
pub const SMW_USA_SHA1: &str = "6b47bb75d16514b6a476aa0c73a683a2a4c18765";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RomKind {
    /// Untouched SMW (USA): the base every hack and tool expects.
    CleanUsa,
    /// Internal title says SMW but the bytes differ: a hack or edited ROM.
    Modified { lunar_magic: bool },
    /// Not a Super Mario World ROM.
    NotSmw,
}

#[derive(Debug, Clone)]
pub struct RomInfo {
    pub kind:          RomKind,
    pub sha1:          String,
    pub size:          usize,
    pub had_header:    bool,
    pub title:         String,
}

/// Remove a 512-byte copier header if present.
pub fn strip_header(mut bytes: Vec<u8>) -> (Vec<u8>, bool) {
    if bytes.len() % 1024 == 512 {
        bytes.drain(..512);
        (bytes, true)
    } else {
        (bytes, false)
    }
}

pub fn identify(bytes: &[u8], had_header: bool) -> RomInfo {
    let sha1 = sha1_smol::Sha1::from(bytes).digest().to_string();
    let title = bytes
        .get(0x7FC0..0x7FD5)
        .map(|t| String::from_utf8_lossy(t).trim_end().to_string())
        .unwrap_or_default();
    let kind = if sha1 == SMW_USA_SHA1 {
        RomKind::CleanUsa
    } else if title.starts_with("SUPER MARIOWORLD") {
        // Lunar Magic stamps "Lunar Magic Version" into the ROM when it saves
        let lunar_magic = bytes.windows(19).any(|w| w == b"Lunar Magic Version");
        RomKind::Modified { lunar_magic }
    } else {
        RomKind::NotSmw
    };
    RomInfo { kind, sha1, size: bytes.len(), had_header, title }
}

/// Load a ROM for emulation, with the SMW (USA) symbol table.
pub fn load(bytes: Vec<u8>) -> (Arc<Rom>, RomInfo) {
    let (bytes, had_header) = strip_header(bytes);
    let info = identify(&bytes, had_header);
    let sa1 = crate::sa1::is_sa1(&bytes);
    let mut rom = Rom::new(bytes);
    rom.load_symbols(include_str!("../symbols/SMW_U.sym"));
    if sa1 {
        let mmc = crate::emu::sa1_start_blocks(Arc::new(rom.clone()));
        rom.set_mapper(crate::rom::Mapper::Sa1(mmc));
    }
    (Arc::new(rom), info)
}
