//! A level's sprite list (enemies, items, shooters...) as stored in the ROM.
//!
//! One header byte, then 3 bytes per sprite, then $FF:
//! `yyyyEESY XXXXssss NNNNNNNN` (y/Y: row, X: column in the screen, s/S: screen,
//! EE: extra bits, N: sprite number). For vertical levels the screen counts down the level
//! and the two position nibbles swap roles (see `parse`).
//!
//! The game spawns sprites as their screen scrolls into view, walking the list in order,
//! so `encode` keeps the list sorted by screen.

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct SpriteEntry {
    pub id:    u8,
    /// Position in blocks (absolute)
    pub x:     u32,
    pub y:     u32,
    /// The two extra bits (EE)
    pub extra: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpriteList {
    pub header:   u8,
    pub sprites:  Vec<SpriteEntry>,
    pub vertical: bool,
}

const NAMES: &str = include_str!("../data/sprite_names.json");

/// Lunar Magic's short name for a sprite number ("Green Koopa", "Bob-omb"...).
pub fn name(id: u8) -> String {
    thread_local! {
        static MAP: std::collections::HashMap<String, String> =
            serde_json::from_str(NAMES.trim_start_matches('\u{feff}')).unwrap_or_default();
    }
    MAP.with(|m| m.get(&format!("{id:02X}")).cloned()).unwrap_or_else(|| format!("Sprite {id:02X}"))
}

impl SpriteList {
    /// Parse from the header byte on. Returns the list and its length including the $FF.
    pub fn parse(bytes: &[u8], vertical: bool) -> Option<(SpriteList, usize)> {
        let header = *bytes.first()?;
        let mut sprites = Vec::new();
        let mut i = 1;
        loop {
            let b0 = *bytes.get(i)?;
            if b0 == 0xFF {
                return Some((SpriteList { header, sprites, vertical }, i + 1));
            }
            let (b1, id) = (*bytes.get(i + 1)?, *bytes.get(i + 2)?);
            let screen = ((b0 as u32 & 0x02) << 3) | (b1 as u32 & 0x0F);
            let lo = b1 as u32 >> 4;
            let hi = ((b0 as u32 & 0x01) << 4) | (b0 as u32 >> 4);
            let (x, y) = if vertical { (lo + (hi >> 4) * 16, (hi & 0x0F) + screen * 16) } else { (lo + screen * 16, hi) };
            sprites.push(SpriteEntry { id, x, y, extra: (b0 >> 2) & 3 });
            i += 3;
            if sprites.len() > 0x400 {
                return None;
            }
        }
    }

    fn screen_of(&self, s: &SpriteEntry) -> u32 {
        if self.vertical { s.y / 16 } else { s.x / 16 }
    }

    /// Keep the list in screen order (stable, so sprites on one screen keep their order).
    pub fn sort(&mut self) {
        let v = self.vertical;
        self.sprites.sort_by_key(|s| if v { s.y / 16 } else { s.x / 16 });
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut list = self.clone();
        list.sort();
        let mut out = vec![self.header];
        for s in &list.sprites {
            let screen = self.screen_of(s);
            let (lo, hi) = if self.vertical { (s.x % 16, ((s.x / 16) << 4) | (s.y % 16)) } else { (s.x % 16, s.y & 0x1F) };
            let b0 = ((hi & 0x0F) << 4) as u8 | (s.extra & 3) << 2 | (((screen >> 4) & 1) << 1) as u8 | ((hi >> 4) & 1) as u8;
            let b1 = (lo << 4) as u8 | (screen & 0x0F) as u8;
            out.extend_from_slice(&[b0, b1, s.id]);
        }
        out.push(0xFF);
        out
    }

    /// Clamp a position to what the format can store for this level.
    pub fn clamp(&self, x: u32, y: u32) -> (u32, u32) {
        if self.vertical { (x.min(31), y.min(32 * 16 - 1)) } else { (x.min(32 * 16 - 1), y.min(31)) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_both_orientations() {
        for vertical in [false, true] {
            let mut l = SpriteList { header: 0x04, sprites: vec![], vertical };
            for (i, (x, y)) in [(3, 20), (40, 7), (17, 31), (300, 2), (511, 25)].into_iter().enumerate() {
                let (x, y) = if vertical { (y % 32, x) } else { (x, y) };
                l.sprites.push(SpriteEntry { id: i as u8 * 7, x, y, extra: (i % 4) as u8 });
            }
            l.sort();
            let bytes = l.encode();
            let (back, len) = SpriteList::parse(&bytes, vertical).unwrap();
            assert_eq!(len, bytes.len());
            assert_eq!(back, l, "vertical={vertical}");
        }
    }
}
