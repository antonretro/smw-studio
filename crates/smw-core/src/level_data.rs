//! Layer 1 object data: parse, edit, encode.
//!
//! Format (after a 5-byte header, ends with $FF):
//! - Standard object: `NBBYYYYY bbbbXXXX SSSSSSSS` (object number BBbbbb != 0)
//! - Extended object: `N00YYYYY 0000XXXX EEEEEEEE` (E = extended number)
//!   - E = 00: screen exit, 4 bytes: `000ppppp 0000w0sh 00000000 dddddddd`
//!   - E = 01: screen jump: `000HHHHH 00000000 00000001`
//!
//! Entries are kept in their original order (later objects draw over earlier ones);
//! screen flags and jumps are rebuilt on encode.

use crate::rom::Rom;

/// SNES address of the layer 1 pointer table (3 bytes per level).
pub const LAYER1_PTRS: u32 = 0x05E000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// Standard object; `id` 01-3F, `settings` is usually the size
    Standard,
    /// Extended object; `id` is the extended number (02-FF)
    Extended,
    /// Screen exit; `x` = screen number, `settings` = flags (w/s/h bits of byte 2), `id` = low byte of destination
    Exit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Entry {
    pub kind:     Kind,
    pub id:       u8,
    pub settings: u8,
    /// Position in blocks (absolute). Exits use `x` as the screen number.
    pub x:        u32,
    pub y:        u32,
}

#[derive(Debug, Clone)]
pub struct LevelData {
    pub header:   [u8; 5],
    pub entries:  Vec<Entry>,
    pub vertical: bool,
}

impl Entry {
    pub fn screen(&self, vertical: bool) -> u32 {
        match (self.kind, vertical) {
            (Kind::Exit, _) => self.x,
            (_, false) => self.x / 16,
            (_, true) => self.y / 16,
        }
    }
}

impl LevelData {
    /// Parse from bytes starting at the header. Returns the data and its length including the $FF.
    pub fn parse(bytes: &[u8], vertical: bool) -> Option<(Self, usize)> {
        let header: [u8; 5] = bytes.get(..5)?.try_into().ok()?;
        let mut entries = Vec::new();
        let mut screen = 0u32;
        let mut i = 5;
        loop {
            let b0 = *bytes.get(i)?;
            if b0 == 0xFF {
                return Some((Self { header, entries, vertical }, i + 1));
            }
            let (b1, b2) = (*bytes.get(i + 1)?, *bytes.get(i + 2)?);
            let num = ((b0 >> 5) & 3) << 4 | b1 >> 4;
            let (yf, xf) = ((b0 & 0x1F) as u32, (b1 & 0x0F) as u32);
            if num == 0 && b2 == 0 {
                let b3 = *bytes.get(i + 3)?;
                entries.push(Entry { kind: Kind::Exit, id: b3, settings: b1, x: yf, y: 0 });
                i += 4;
                continue;
            }
            if num == 0 && b2 == 1 {
                screen = yf;
                i += 3;
                continue;
            }
            if b0 & 0x80 != 0 {
                screen += 1;
            }
            let (x, y) = if vertical {
                ((yf & 0x10) + xf, screen * 16 + (yf & 0x0F))
            } else {
                (screen * 16 + xf, yf)
            };
            let (kind, id, settings) = if num == 0 { (Kind::Extended, b2, 0) } else { (Kind::Standard, num, b2) };
            entries.push(Entry { kind, id, settings, x, y });
            i += 3;
        }
    }

    pub fn encode(&self) -> Vec<u8> {
        self.encode_with_offsets().0
    }

    /// Encoded bytes plus the byte offset where each entry starts.
    pub fn encode_with_offsets(&self) -> (Vec<u8>, Vec<usize>) {
        let mut out = self.header.to_vec();
        let mut starts = Vec::with_capacity(self.entries.len());
        let mut screen = 0u32;
        for e in &self.entries {
            if e.kind == Kind::Exit {
                starts.push(out.len());
                // Exits don't move the screen counter, so they stay where they were
                // Low nibble: w (midway), LM water-destination bit, s (secondary), h (level bit 8)
                out.extend_from_slice(&[(e.x as u8) & 0x1F, e.settings & 0x0F, 0x00, e.id]);
                continue;
            }
            let s = e.screen(self.vertical);
            let mut new_screen = false;
            if s == screen + 1 {
                new_screen = true;
            } else if s != screen {
                out.extend_from_slice(&[(s as u8) & 0x1F, 0x00, 0x01]);
            }
            screen = s;
            starts.push(out.len());
            let (yf, xf) = if self.vertical {
                ((e.x & 0x10) as u8 | (e.y & 0x0F) as u8, (e.x & 0x0F) as u8)
            } else {
                ((e.y & 0x1F) as u8, (e.x & 0x0F) as u8)
            };
            let n = (new_screen as u8) << 7;
            match e.kind {
                Kind::Standard => out.extend_from_slice(&[n | (e.id & 0x30) << 1 | yf, (e.id & 0x0F) << 4 | xf, e.settings]),
                Kind::Extended => out.extend_from_slice(&[n | yf, xf, e.id]),
                Kind::Exit => unreachable!(),
            }
        }
        out.push(0xFF);
        (out, starts)
    }

    /// Keep objects ordered by screen (required by the format) while preserving
    /// drawing order inside each screen: a moved object goes last on its new screen.
    pub fn move_entry(&mut self, index: usize, x: u32, y: u32) -> usize {
        let mut e = self.entries.remove(index);
        let old_screen = e.screen(self.vertical);
        e.x = x;
        e.y = y;
        let new_screen = e.screen(self.vertical);
        if new_screen == old_screen {
            self.entries.insert(index, e);
            return index;
        }
        let pos = self
            .entries
            .iter()
            .rposition(|o| o.kind != Kind::Exit && o.screen(self.vertical) <= new_screen)
            .map(|p| p + 1)
            .unwrap_or_else(|| self.entries.iter().take_while(|o| o.kind == Kind::Exit).count());
        self.entries.insert(pos, e);
        pos
    }
}

/// Find a level's layer 1 data in the ROM: (SNES address, parsed data, byte length).
pub fn read_level(rom: &Rom, level: u16, vertical: bool) -> Option<(u32, LevelData, usize)> {
    let ptr = rom.read_u32(LAYER1_PTRS + level as u32 * 3)?;
    let mut bytes = Vec::new();
    // Read until the parser is satisfied (levels are well under 64 KB)
    for i in 0..0x10000u32 {
        bytes.push(rom.read(ptr + i)?);
        if bytes.len() > 5 && *bytes.last().unwrap() == 0xFF {
            if let Some((data, len)) = LevelData::parse(&bytes, vertical) {
                return Some((ptr, data, len));
            }
        }
    }
    None
}
