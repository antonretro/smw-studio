//! An open ROM with unsaved level edits, undo history, and saving.

use std::{collections::HashMap, path::PathBuf, sync::Arc};

use crate::{
    emu,
    level_data::{self, Entry, Kind, LevelData, LAYER1_PTRS},
    palette::{self, PaletteItem},
    sprite_data::{SpriteEntry, SpriteList},
    render::{self, Image, LevelInfo},
    rom::Rom,
    romfile::{self, RomInfo},
    Cpu,
};

/// Level modes that are vertical (header byte 1, low 5 bits).
const VERTICAL_MODES: [u8; 4] = [0x07, 0x08, 0x0A, 0x0D];

#[derive(Clone)]
pub struct LevelEdit {
    pub data:  LevelData,
    pub ptr:   u32,
    pub len:   usize,
    pub dirty: bool,
    /// The level's sprite list, where it was read from, and its length (incl. $FF)
    pub sprites:    SpriteList,
    pub sprite_ptr: u32,
    pub sprite_len: usize,
    /// Main entrance bytes from the tables at $05:F000/F200/F400/F600, then Lunar Magic's
    /// $05:DE00 and $06:FC00 (exact-position format; $FF/$00 when the ROM doesn't have it)
    pub entrance:   [u8; 6],
    /// Secondary entrances that lead into this level: (number, bytes from $05:F800/FA00/FC00/FE00)
    pub secondaries: Vec<(u16, [u8; 4])>,
    /// Song played in the level (AddmusicK number), 0 = the header's music setting
    pub song:       u8,
    undo:      Vec<Snapshot>,
    redo:      Vec<Snapshot>,
}

/// Everything undo/redo restores for a level.
#[derive(Clone)]
struct Snapshot {
    data:        LevelData,
    sprites:     SpriteList,
    entrance:    [u8; 6],
    secondaries: Vec<(u16, [u8; 4])>,
    song:        u8,
}

impl LevelEdit {
    fn snapshot(&self) -> Snapshot {
        Snapshot { data: self.data.clone(), sprites: self.sprites.clone(), entrance: self.entrance, secondaries: self.secondaries.clone(), song: self.song }
    }

    fn restore(&mut self, s: Snapshot) -> Snapshot {
        let now = self.snapshot();
        self.data = s.data;
        self.sprites = s.sprites;
        self.entrance = s.entrance;
        self.secondaries = s.secondaries;
        self.song = s.song;
        now
    }
}

#[derive(Clone)]
pub struct Project {
    pub path:  PathBuf,
    pub info:  RomInfo,
    bytes:     Vec<u8>,
    rom:       Arc<Rom>,
    pub edits: HashMap<u16, LevelEdit>,
    backed_up: bool,
    /// Code was changed (engine upgrades) and not saved yet
    code_dirty: bool,
    palettes:  HashMap<[u8; 5], Vec<PaletteItem>>,
    /// Sprite palettes per (enemy graphics set, sprite palette)
    sprite_palettes: HashMap<[u8; 2], Vec<SpritePaletteItem>>,
}

/// Where Mario appears: the screen, one of 8 horizontal spots, one of 16 heights, and how he
/// enters (Lunar Magic's names: do nothing, pipe exits, slanted pipe...).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Entrance {
    pub screen: u8,
    pub x:      u8,
    pub y:      u8,
    pub action: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct SecondaryEntrance {
    pub number:   u16,
    pub entrance: Entrance,
}

/// Where pipes and doors on one screen lead.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ScreenExit {
    pub screen:    u8,
    /// Level number, or secondary entrance number when `secondary`
    pub dest:      u16,
    pub secondary: bool,
    /// Lunar Magic: the destination is a water level (secondary exits)
    pub water:     bool,
}

/// One entry of the sprite palette.
#[derive(Clone)]
pub struct SpritePaletteItem {
    pub id:      u8,
    pub name:    String,
    /// Drawn by the game in this level's graphics; None if it draws nothing by itself
    pub preview: Option<Image>,
    /// Where the preview's top-left is relative to the sprite's position, in pixels
    pub offset:  (i32, i32),
}

pub struct LevelRender {
    pub info:    LevelInfo,
    /// Everything together
    pub image:   Image,
    /// The layers on their own: back area colour + background (or layer 2), layer 1, and
    /// sprites (the last two see-through)
    pub back:    Image,
    pub front:   Image,
    pub sprite_layer: Image,
    pub sprites: usize,
    /// Each sprite's box on the image [x, y, w, h], in sprite list order (empty if not drawn)
    pub sprite_boxes: Vec<[i32; 4]>,
    /// Per block (row-major, image size / 16): entry index + 1, or 0 for none
    pub owner:   Vec<u16>,
    pub cols:    u32,
    pub rows:    u32,
}

/// SNES checksum: a size that isn't a power of two counts its last part repeated up to the
/// next power of two (a 6 MB ROM is 4 MB + the last 2 MB twice).
fn checksum(b: &[u8]) -> u16 {
    let sum = |b: &[u8]| b.iter().fold(0u16, |s, &x| s.wrapping_add(x as u16));
    if b.len().is_power_of_two() || b.is_empty() {
        return sum(b);
    }
    let base = 1 << (usize::BITS - 1 - b.len().leading_zeros());
    let rest = &b[base..];
    let times = base / (rest.len().next_power_of_two());
    sum(&b[..base]).wrapping_add(checksum(rest).wrapping_mul(times as u16))
}

/// Set the header checksum of headerless ROM bytes.
pub fn fix_checksum_bytes(bytes: &mut [u8]) {
    bytes[0x7FDC..0x7FE0].copy_from_slice(&[0xFF, 0xFF, 0x00, 0x00]);
    let sum = checksum(bytes);
    bytes[0x7FDC..0x7FDE].copy_from_slice(&(sum ^ 0xFFFF).to_le_bytes());
    bytes[0x7FDE..0x7FE0].copy_from_slice(&sum.to_le_bytes());
}

fn make_rom(bytes: &[u8]) -> Arc<Rom> {
    let mut rom = Rom::new(bytes.to_vec());
    rom.load_symbols(include_str!("../symbols/SMW_U.sym"));
    if crate::sa1::is_sa1(bytes) {
        // SA-1: ROM addresses go through the blocks the game picks at start-up
        let mmc = crate::emu::sa1_start_blocks(Arc::new(rom.clone()));
        rom.set_mapper(crate::rom::Mapper::Sa1(mmc));
    }
    Arc::new(rom)
}

impl Project {
    pub fn open(path: PathBuf) -> Result<Self, String> {
        let raw = std::fs::read(&path).map_err(|e| format!("Could not read file: {e}"))?;
        let (bytes, had_header) = romfile::strip_header(raw);
        let info = romfile::identify(&bytes, had_header);
        if info.kind == romfile::RomKind::NotSmw {
            return Err(format!("This isn't a Super Mario World ROM (title: \"{}\").", info.title));
        }
        let rom = make_rom(&bytes);
        Ok(Self { path, info, bytes, rom, edits: HashMap::new(), backed_up: false, code_dirty: false, palettes: HashMap::new(), sprite_palettes: HashMap::new() })
    }

    /// The ROM as it is now (unsaved patches included; unsaved level edits not).
    pub fn rom_arc(&self) -> Arc<Rom> {
        self.rom.clone()
    }

    pub fn is_dirty(&self) -> bool {
        self.code_dirty || self.edits.values().any(|e| e.dirty)
    }

    /// Change ROM bytes directly (engine upgrades). Saved with the next save.
    pub fn patch(&mut self, addr: u32, data: &[u8]) {
        self.write_snes(addr, data);
        self.code_dirty = true;
        self.rom = make_rom(&self.bytes);
    }

    /// Put code in free space (protected so other tools leave it alone); returns its address.
    pub(crate) fn alloc_code(&mut self, code: &[u8]) -> Result<u32, String> {
        let at = self.alloc_rats(code)?;
        self.code_dirty = true;
        Ok(at)
    }

    fn edit(&mut self, level: u16) -> Result<&mut LevelEdit, String> {
        if !self.edits.contains_key(&level) {
            let ptr = self.rom.read_u32(LAYER1_PTRS + level as u32 * 3).ok_or("Bad level pointer")?;
            let mode = self.rom.read(ptr + 1).ok_or("Bad level header")? & 0x1F;
            let vertical = VERTICAL_MODES.contains(&mode);
            let (ptr, data, len) =
                level_data::read_level(&self.rom, level, vertical).ok_or("Could not read level data")?;
            let sprite_ptr = self.sprite_ptr(level).ok_or("Bad sprite pointer")?;
            let raw: Vec<u8> = (0..0x1000u32).map_while(|i| self.read_snes(sprite_ptr + i)).collect();
            // A list we can't read is left exactly as it is (sprite_len 0 = don't touch)
            let (sprites, sprite_len) = SpriteList::parse(&raw, vertical).unwrap_or((SpriteList { header: 0, sprites: vec![], vertical }, 0));
            let mut entrance = [0, 0, 0, 0, 0xFF, 0];
            for (i, t) in [0x05F000u32, 0x05F200, 0x05F400, 0x05F600].into_iter().enumerate() {
                entrance[i] = self.read_snes(t + level as u32).unwrap_or(0);
            }
            if self.has_exact_entrances() {
                entrance[4] = self.read_snes(0x05DE00 + level as u32).unwrap_or(0xFF);
                entrance[5] = self.read_snes(0x06FC00 + level as u32).unwrap_or(0);
            }
            let secondaries = self.secondaries_into(level);
            let song = self.song_table().and_then(|t| self.read_snes(t + level as u32)).unwrap_or(0);
            self.edits.insert(level, LevelEdit { data, ptr, len, dirty: false, sprites, sprite_ptr, sprite_len, entrance, secondaries, song, undo: vec![], redo: vec![] });
        }
        Ok(self.edits.get_mut(&level).unwrap())
    }

    pub fn level_data(&mut self, level: u16) -> Result<&LevelData, String> {
        Ok(&self.edit(level)?.data)
    }

    // ---- entrances ----
    //
    // Main entrance (per level), 4 tables of $200 at $05:F000/F200/F400/F600:
    //   F000: low 4 = Mario Y (index into $05:D730/D740), high 4 = layer 2 scroll
    //   F200: low 3 = Mario X (index into $05:D750), bits 3-5 = entry style ($192A), top 2 = layer 3
    //   F400: camera heights;  F600: low 5 = screen, bits 5-6 = layout, bit 7 = flag
    // Secondary entrances (per entrance number) at $05:F800/FA00/FC00/FE00:
    //   F800: destination level (low byte)
    //   FA00: low 4 = Mario Y, high 4 = camera heights
    //   FC00: low 5 = screen, top 3 = Mario X
    //   FE00: low 3 = entry style, bit 3 = destination level bit 8 (Lunar Magic)
    // Newer Lunar Magic can put the main entrance on any 16x16 tile (its code at $05:DD30):
    // when bit 5 of $05:DE00 is set, Mario X = tile (F200 bits 0-2, DE00 bits 3-4) * 16 and
    // Mario Y = tile (F000 low 4, $06:FC00 bits 0-5) * 16.

    /// Whether this ROM has Lunar Magic's exact-position main entrances (its JSL at $05:D97D)
    pub fn has_exact_entrances(&self) -> bool {
        (0..4).map(|i| self.read_snes(0x05D97D + i)).collect::<Vec<_>>() == [Some(0x22), Some(0x30), Some(0xDD), Some(0x05)]
    }

    /// Whether this level's main entrance is on an exact tile: then Entrance x/y are tile
    /// numbers (x 0-15 within the screen, y from the top) instead of spot/height indexes.
    pub fn main_entrance_exact(&mut self, level: u16) -> Result<bool, String> {
        let b = self.edit(level)?.entrance;
        Ok(b[4] != 0xFF && b[4] & 0x20 != 0)
    }

    pub fn main_entrance(&mut self, level: u16) -> Result<Entrance, String> {
        let exact = self.main_entrance_exact(level)?;
        let b = self.edit(level)?.entrance;
        let (x, y) = if exact {
            ((b[1] & 7) | ((b[4] >> 3) & 1) << 3, (b[0] & 0x0F) | (b[5] & 0x0F) << 4)
        } else {
            (b[1] & 7, b[0] & 0x0F)
        };
        Ok(Entrance { screen: b[3] & 0x1F, x, y, action: (b[1] >> 3) & 7 })
    }

    /// Where Mario starts for the main entrance: pixel X within the screen, and pixel Y
    /// (horizontal levels; the screen is `main_entrance().screen`).
    pub fn main_entrance_pixel(&mut self, level: u16) -> Result<(u16, u16), String> {
        let e = self.main_entrance(level)?;
        if self.main_entrance_exact(level)? {
            return Ok((e.x as u16 * 16, e.y as u16 * 16));
        }
        let (xs, ys) = self.entrance_positions();
        Ok((xs[e.x as usize], ys[e.y as usize]))
    }

    pub fn set_main_entrance(&mut self, level: u16, e: Entrance) -> Result<(), String> {
        let ed = self.edit(level)?;
        let snap = ed.snapshot();
        ed.undo.push(snap);
        ed.redo.clear();
        let b = &mut ed.entrance;
        let exact = b[4] != 0xFF && b[4] & 0x20 != 0;
        b[0] = (b[0] & 0xF0) | (e.y & 0x0F);
        b[1] = (b[1] & 0xC0) | (e.action & 7) << 3 | (e.x & 7);
        b[3] = (b[3] & 0xE0) | (e.screen & 0x1F);
        if exact {
            // X tile bit 3 is DE00 bit 3 (bit 4 would be past the screen); Y tile high bits in $06:FC00
            b[4] = (b[4] & !0x18) | ((e.x >> 3) & 1) << 3;
            b[5] = (b[5] & 0xC0) | (e.y >> 4);
        }
        ed.dirty = true;
        Ok(())
    }

    /// Screen Mario restarts on after the midway gate (high 4 bits of $05:F400, so 00-0F);
    /// he uses the main entrance's spot and height there.
    pub fn midway_screen(&mut self, level: u16) -> Result<u8, String> {
        Ok(self.edit(level)?.entrance[2] >> 4)
    }

    pub fn set_midway_screen(&mut self, level: u16, screen: u8) -> Result<(), String> {
        if screen > 0x0F {
            return Err("The midway point can be on screens 00-0F".into());
        }
        let ed = self.edit(level)?;
        let snap = ed.snapshot();
        ed.undo.push(snap);
        ed.redo.clear();
        ed.entrance[2] = (ed.entrance[2] & 0x0F) | screen << 4;
        ed.dirty = true;
        Ok(())
    }

    /// Every secondary entrance in use: (number, destination level).
    pub fn all_secondary_entrances(&self) -> Vec<(u16, u16)> {
        let mut out: Vec<(u16, u16)> = (0..0x200u16)
            .filter_map(|n| {
                let b = [0x05F800u32, 0x05FA00, 0x05FC00, 0x05FE00].map(|t| self.read_snes(t + n as u32).unwrap_or(0));
                (b != [0; 4]).then(|| (n, (((b[3] >> 3) & 1) as u16) << 8 | b[0] as u16))
            })
            .collect();
        // Unsaved new ones and edits
        for (&level, e) in &self.edits {
            for (n, _) in &e.secondaries {
                if let Some(x) = out.iter_mut().find(|x| x.0 == *n) {
                    x.1 = level;
                } else {
                    out.push((*n, level));
                }
            }
        }
        out.sort();
        out
    }

    /// Make a new secondary entrance into `level`, at the main entrance's position.
    /// Returns its number (the highest free one).
    pub fn add_secondary_entrance(&mut self, level: u16) -> Result<u16, String> {
        let taken: std::collections::HashSet<u16> = self.all_secondary_entrances().into_iter().map(|(n, _)| n).collect();
        // The original game reaches a secondary entrance's level in the same half (000-0FF or
        // 100-1FF) as the entrance number; Lunar Magic stores the level's high bit itself
        let any_half = self.per_screen_exit_flags();
        let n = (1..0x200u16)
            .rev()
            .find(|&n| !taken.contains(&n) && (any_half || n >> 8 == level >> 8))
            .ok_or("No free secondary entrance numbers left")?;
        let mut main = self.edit(level)?.entrance;
        if self.main_entrance_exact(level)? {
            // Secondary entrances use spots and heights: take the nearest to the exact tile
            let e = self.main_entrance(level)?;
            let (xs, ys) = self.entrance_positions();
            let near = |list: &[u16], v: u16| (0..list.len()).min_by_key(|&i| (list[i] as i32 - v as i32).abs()).unwrap_or(0) as u8;
            main[0] = (main[0] & 0xF0) | near(&ys, e.y as u16 * 16);
            main[1] = (main[1] & 0xF8) | near(&xs, e.x as u16 * 16);
        }
        let ed = self.edit(level)?;
        let snap = ed.snapshot();
        ed.undo.push(snap);
        ed.redo.clear();
        // Same spot as the main entrance: Y and camera from F000/F400, X and screen, style
        // Camera heights: main F400 has layer 2 in bits 0-1 and layer 1 in bits 2-3;
        // secondary FA00 has layer 1 in bits 4-5 and layer 2 in bits 6-7
        let fa = (main[0] & 0x0F) | ((main[2] >> 2) & 3) << 4 | (main[2] & 3) << 6;
        let fc = (main[1] & 7) << 5 | (main[3] & 0x1F);
        let fe = ((main[1] >> 3) & 7) | (((level >> 8) & 1) as u8) << 3;
        ed.secondaries.push((n, [level as u8, fa, fc, fe]));
        ed.dirty = true;
        Ok(n)
    }

    /// Secondary entrances leading into `level` (numbers 000-1FF).
    pub fn secondary_entrances(&mut self, level: u16) -> Result<Vec<SecondaryEntrance>, String> {
        Ok(self
            .edit(level)?
            .secondaries
            .iter()
            .map(|&(number, b)| SecondaryEntrance {
                number,
                entrance: Entrance { screen: b[2] & 0x1F, x: b[2] >> 5, y: b[1] & 0x0F, action: b[3] & 7 },
            })
            .collect())
    }

    pub fn set_secondary_entrance(&mut self, level: u16, number: u16, e: Entrance) -> Result<(), String> {
        let ed = self.edit(level)?;
        let Some(i) = ed.secondaries.iter().position(|(n, _)| *n == number) else {
            return Err(format!("Secondary entrance {number:03X} doesn't lead into this level"));
        };
        let snap = ed.snapshot();
        ed.undo.push(snap);
        ed.redo.clear();
        let b = &mut ed.secondaries[i].1;
        b[1] = (b[1] & 0xF0) | (e.y & 0x0F);
        b[2] = (e.x & 7) << 5 | (e.screen & 0x1F);
        b[3] = (b[3] & 0xF8) | (e.action & 7);
        ed.dirty = true;
        Ok(())
    }

    /// Secondary entrances whose destination is `level`. Lunar Magic keeps the destination's
    /// bit 8 in $05:FE00 bit 3; the original game takes it from the map you came from, so
    /// on original ROMs any entrance with the same low byte is listed.
    fn secondaries_into(&self, level: u16) -> Vec<(u16, [u8; 4])> {
        let lm = self.per_screen_exit_flags();
        (0..0x200u16)
            .filter_map(|n| {
                let b = [0x05F800u32, 0x05FA00, 0x05FC00, 0x05FE00].map(|t| self.read_snes(t + n as u32).unwrap_or(0));
                let dest = (((b[3] >> 3) & 1) as u16) << 8 | b[0] as u16;
                let matches = if lm { dest == level } else { b[0] as u16 == level & 0xFF };
                (matches && b != [0; 4]).then_some((n, b))
            })
            .collect()
    }

    /// Pixel positions the entrance indexes stand for, from the ROM's own tables:
    /// X within the screen (8 spots) and Y (16 heights).
    pub fn entrance_positions(&self) -> ([u16; 8], [u16; 16]) {
        let mut xs = [0u16; 8];
        let mut ys = [0u16; 16];
        for i in 0..8 {
            xs[i] = self.read_snes(0x05D750 + i as u32).unwrap_or(0) as u16;
        }
        for i in 0..16 {
            ys[i] = self.read_snes(0x05D730 + i as u32).unwrap_or(0) as u16 | (self.read_snes(0x05D740 + i as u32).unwrap_or(0) as u16) << 8;
        }
        (xs, ys)
    }

    fn save_entrances(&mut self, level: u16) {
        let (main, secs) = {
            let e = &self.edits[&level];
            (e.entrance, e.secondaries.clone())
        };
        for (i, t) in [0x05F000u32, 0x05F200, 0x05F400, 0x05F600].into_iter().enumerate() {
            self.write_snes(t + level as u32, &[main[i]]);
        }
        if main[4] != 0xFF && self.has_exact_entrances() {
            self.write_snes(0x05DE00 + level as u32, &[main[4]]);
            self.write_snes(0x06FC00 + level as u32, &[main[5]]);
        }
        for (n, b) in secs {
            for (i, t) in [0x05F800u32, 0x05FA00, 0x05FC00, 0x05FE00].into_iter().enumerate() {
                self.write_snes(t + n as u32, &[b[i]]);
            }
        }
    }

    /// The level's screen exits (where pipes and doors on each screen lead), by screen.
    pub fn exits(&mut self, level: u16) -> Result<Vec<ScreenExit>, String> {
        let per_screen = self.per_screen_exit_flags();
        let mut out: Vec<ScreenExit> = self
            .edit(level)?
            .data
            .entries
            .iter()
            .filter(|e| e.kind == Kind::Exit)
            .map(|e| {
                // Lunar Magic only reads the high bit and water from the exit when bit 2 is
                // set; otherwise (and always in the original game) the high bit comes from
                // where Mario entered from the map: the same half as this level
                let own = per_screen && e.settings & 4 != 0;
                let high = if own { e.settings as u16 & 1 } else { (level >= 0x100) as u16 };
                ScreenExit {
                    screen:    e.x as u8,
                    dest:      high << 8 | e.id as u16,
                    secondary: e.settings & 2 != 0,
                    water:     per_screen && e.settings & 0x0C == 0x0C,
                }
            })
            .collect();
        out.sort_by_key(|x| x.screen);
        Ok(out)
    }

    /// The raw flag nibble of each screen exit (for tests)
    pub fn exit_settings(&mut self, level: u16) -> Result<Vec<u8>, String> {
        Ok(self.edit(level)?.data.entries.iter().filter(|e| e.kind == Kind::Exit).map(|e| e.settings & 0x0F).collect())
    }

    /// Whether each screen exit keeps its own flags (Lunar Magic: `AND #$0F` at $0D:A532).
    /// Lunar Magic's flags (read at $05:DC50): bit 0 destination high bit, bit 1 secondary,
    /// bit 2 "use this exit's own flags", bit 3 water. In the original game only the high bit
    /// is per screen; the secondary flag is one setting for the whole level and there's no water.
    pub fn per_screen_exit_flags(&self) -> bool {
        self.read_snes(0x0DA532) == Some(0x0F)
    }

    /// Add or replace the exit for `x.screen` (with undo). On original ROMs the secondary
    /// choice is applied to every exit in the level, since the game treats it that way.
    pub fn set_exit(&mut self, level: u16, x: ScreenExit) -> Result<(), String> {
        let per_screen = self.per_screen_exit_flags();
        let x = if per_screen { x } else { ScreenExit { water: false, ..x } };
        if x.dest > 0x1FF {
            return Err("Exit destinations go up to 1FF".into());
        }
        if x.screen >= 0x20 {
            return Err("Levels have at most 32 screens (00-1F)".into());
        }
        if !per_screen && x.dest >> 8 != level >> 8 {
            return Err(format!(
                "In this ROM, exits from level {level:03X} can only lead to {}. (Lunar Magic-saved ROMs don't have this limit.)",
                if level >= 0x100 { "100-1FF" } else { "000-0FF" }
            ));
        }
        let settings = if per_screen { 4 | (x.water as u8) << 3 } else { 0 } | (x.secondary as u8) << 1 | (x.dest >> 8) as u8;
        let entry = Entry { kind: Kind::Exit, id: x.dest as u8, settings, x: x.screen as u32, y: 0 };
        self.change(level, |d| {
            if let Some(e) = d.entries.iter_mut().find(|e| e.kind == Kind::Exit && e.x == x.screen as u32) {
                *e = entry;
            } else {
                // Exits don't take part in the screen counter, so the front is a safe place
                d.entries.insert(0, entry);
            }
            if !per_screen {
                for e in d.entries.iter_mut().filter(|e| e.kind == Kind::Exit) {
                    e.settings = (e.settings & !2) | (x.secondary as u8) << 1;
                }
            }
        })
    }

    pub fn remove_exit(&mut self, level: u16, screen: u8) -> Result<(), String> {
        self.change(level, |d| d.entries.retain(|e| !(e.kind == Kind::Exit && e.x == screen as u32)))
    }

    /// Change the level's sprites (with undo). The list is kept in screen order.
    pub fn change_sprites(&mut self, level: u16, f: impl FnOnce(&mut SpriteList)) -> Result<(), String> {
        let e = self.edit(level)?;
        if e.sprite_len == 0 {
            return Err("This level's sprite list is in a format SMW Studio can't edit yet, so it's left as it is".into());
        }
        let snap = e.snapshot();
        e.undo.push(snap);
        e.redo.clear();
        f(&mut e.sprites);
        e.sprites.sort();
        e.dirty = true;
        Ok(())
    }

    pub fn sprites(&mut self, level: u16) -> Result<SpriteList, String> {
        Ok(self.edit(level)?.sprites.clone())
    }

    /// Add a sprite; returns its index in the (screen-sorted) list.
    pub fn add_sprite(&mut self, level: u16, id: u8, x: u32, y: u32, extra: u8) -> Result<usize, String> {
        let mut at = 0;
        self.change_sprites(level, |l| {
            let (x, y) = l.clamp(x, y);
            let s = SpriteEntry { id, x, y, extra: extra & 3 };
            l.sprites.push(s);
            l.sort();
            at = l.sprites.iter().rposition(|t| *t == s).unwrap_or(0);
        })?;
        Ok(at)
    }

    /// Move sprite `index`; returns its new index.
    pub fn move_sprite(&mut self, level: u16, index: usize, x: u32, y: u32) -> Result<usize, String> {
        let mut at = index;
        self.change_sprites(level, |l| {
            let (x, y) = l.clamp(x, y);
            if let Some(s) = l.sprites.get_mut(index) {
                s.x = x;
                s.y = y;
                let moved = *s;
                l.sort();
                at = l.sprites.iter().position(|t| *t == moved).unwrap_or(index);
            }
        })?;
        Ok(at)
    }

    pub fn set_sprite(&mut self, level: u16, index: usize, id: u8, extra: u8) -> Result<(), String> {
        self.change_sprites(level, |l| {
            if let Some(s) = l.sprites.get_mut(index) {
                s.id = id;
                s.extra = extra & 3;
            }
        })
    }

    pub fn delete_sprite(&mut self, level: u16, index: usize) -> Result<(), String> {
        self.change_sprites(level, |l| {
            if index < l.sprites.len() {
                l.sprites.remove(index);
            }
        })
    }

    /// Apply a change with undo support.
    pub fn change(&mut self, level: u16, f: impl FnOnce(&mut LevelData)) -> Result<(), String> {
        let e = self.edit(level)?;
        let snap = e.snapshot();
        e.undo.push(snap);
        e.redo.clear();
        f(&mut e.data);
        e.dirty = true;
        Ok(())
    }

    pub fn undo(&mut self, level: u16) -> Result<bool, String> {
        let e = self.edit(level)?;
        Ok(match e.undo.pop() {
            Some(prev) => {
                let now = e.restore(prev);
                e.redo.push(now);
                e.dirty = true;
                true
            }
            None => false,
        })
    }

    pub fn redo(&mut self, level: u16) -> Result<bool, String> {
        let e = self.edit(level)?;
        Ok(match e.redo.pop() {
            Some(next) => {
                let now = e.restore(next);
                e.undo.push(now);
                e.dirty = true;
                true
            }
            None => false,
        })
    }

    /// How long the level is: header byte 0, low 5 bits, holds screens - 1. Horizontal levels
    /// can have 1-32 screens, vertical ones 1-14 (their screens stack two by two).
    pub fn set_screens(&mut self, level: u16, screens: u8) -> Result<u8, String> {
        let vertical = self.edit(level)?.data.vertical;
        let max = if vertical { 14 } else { 32 };
        let n = screens.clamp(1, max);
        self.change(level, |d| d.header[0] = (d.header[0] & 0xE0) | (n - 1))?;
        Ok(n)
    }

    /// The level's 5 header bytes (screens, palettes, level mode, music, sprite set, timer,
    /// scroll, tileset... see the Level Settings panel for the bit layout).
    pub fn header(&mut self, level: u16) -> Result<[u8; 5], String> {
        Ok(self.edit(level)?.data.header)
    }

    /// Change some bits of one header byte (with undo). Screens and level mode are handled
    /// elsewhere because they change how the level data is laid out.
    pub fn set_header_bits(&mut self, level: u16, byte: usize, mask: u8, value: u8) -> Result<(), String> {
        if byte > 4 || (byte == 0 && mask & 0x1F != 0) || (byte == 1 && mask & 0x1F != 0) {
            return Err("That header setting can't be changed this way".into());
        }
        self.change(level, |d| d.header[byte] = (d.header[byte] & !mask) | (value & mask))
    }

    /// Tables the header options point into, read from this ROM (hacks change them):
    /// the song played for each of the 8 level music choices, and the 8 back area colours.
    pub fn header_tables(&self) -> ([u8; 8], [u16; 8]) {
        let mut music = [0u8; 8];
        let mut colours = [0u16; 8];
        for i in 0..8 {
            music[i] = self.read_snes(0x0584DB + i as u32).unwrap_or(0);
            let lo = self.read_snes(0x00B0A0 + i as u32 * 2).unwrap_or(0);
            let hi = self.read_snes(0x00B0A1 + i as u32 * 2).unwrap_or(0);
            colours[i] = u16::from_le_bytes([lo, hi]);
        }
        (music, colours)
    }

    /// Render the level from the (possibly edited) layer 1 data.
    pub fn render(&mut self, level: u16, sprites: bool) -> Result<LevelRender, String> {
        let rom = self.rom.clone();
        let e = self.edit(level)?;
        let (bytes, starts) = e.data.encode_with_offsets();
        let mut cpu = Cpu::new(emu::CheckedMem::new(rom));
        emu::load_level_with(&mut cpu, level, Some(&bytes));
        let info = render::level_info(&mut cpu);
        let (back, front) = render::render_level_layers(&mut cpu);
        // Sprites come from the edited list, not the ROM, so changes show right away
        let list: Vec<crate::sprites::LevelSprite> = e.sprites.sprites.iter().map(|s| crate::sprites::LevelSprite { id: s.id, x: s.x * 16, y: s.y * 16, extra: s.extra }).collect();
        let mut sprite_layer = Image::new(back.width, back.height, [0, 0, 0, 0]);
        let sprite_boxes = if sprites { render::draw_sprite_list(&mut cpu, &mut sprite_layer, &list) } else { vec![] };
        let mut image = back.clone();
        image.overlay(&front);
        image.overlay(&sprite_layer);
        let sprite_count = list.len();

        // Turn "data offset last read when this tile was written" into entry indices per block
        let (cols, rows) = (image.width / 16, image.height / 16);
        let mut owner = vec![0u16; (cols * rows) as usize];
        let has_layer2 = render::has_layer2(&mut cpu);
        for (idx, &off) in cpu.mem.owner.iter().enumerate() {
            if off == u32::MAX || (off as usize) < 5 {
                continue;
            }
            let entry = match starts.binary_search(&(off as usize)) {
                Ok(i) => i,
                Err(0) => continue,
                Err(i) => i - 1,
            };
            if let Some((bx, by)) = render::layer1_block(idx as u32, info.vertical, has_layer2) {
                if bx < cols && by < rows {
                    owner[(by * cols + bx) as usize] = entry as u16 + 1;
                }
            }
        }
        Ok(LevelRender { info, image, back, front, sprite_layer, sprites: sprite_count, sprite_boxes, owner, cols, rows })
    }

    /// Every named sprite, drawn by the game in this level's enemy graphics and palette
    /// (cached per enemy graphics set + sprite palette).
    pub fn sprite_palette(&mut self, level: u16) -> Result<Vec<SpritePaletteItem>, String> {
        let header = self.edit(level)?.data.header;
        let key = [header[2] & 0x0F, header[3] & 0x38];
        if let Some(items) = self.sprite_palettes.get(&key) {
            return Ok(items.clone());
        }
        let mut cpu = Cpu::new(emu::CheckedMem::new(self.rom.clone()));
        let bytes = self.edit(level)?.data.encode();
        emu::load_level_with(&mut cpu, level, Some(&bytes));
        let items: Vec<SpritePaletteItem> = (0..=0xFFu8)
            .filter_map(|id| {
                let name = crate::sprite_data::name(id);
                if name.starts_with("Sprite ") {
                    return None; // not a sprite Lunar Magic lists
                }
                // Only normal sprites (00-C8) run code we can draw; the rest are shooters,
                // generators and run-once commands
                let drawn = if id <= 0xC8 { render::sprite_preview_at(&cpu, id) } else { None };
                let offset = drawn.as_ref().map(|d| (d.1, d.2)).unwrap_or((0, 0));
                Some(SpritePaletteItem { id, name, preview: drawn.map(|d| d.0), offset })
            })
            .collect();
        self.sprite_palettes.insert(key, items.clone());
        Ok(items)
    }

    /// Object palette with previews in this level's tileset (cached per tileset).
    pub fn palette(&mut self, level: u16) -> Result<Vec<PaletteItem>, String> {
        let header = self.edit(level)?.data.header;
        let key = [header[0] & 0xE0, header[1] & 0xE0, header[2], header[3], header[4]];
        if let Some(items) = self.palettes.get(&key) {
            return Ok(items.clone());
        }
        let mut items = palette::catalog();
        palette::render_previews(self.rom.clone(), level, header, &mut items);
        self.palettes.insert(key, items.clone());
        Ok(items)
    }

    /// Place a new object; returns its entry index.
    pub fn add_entry(&mut self, level: u16, kind: Kind, id: u8, settings: u8, x: u32, y: u32) -> Result<usize, String> {
        let mut index = 0;
        self.change(level, |d| {
            // Start at the very end, then let move_entry put it last on its screen
            d.entries.push(Entry { kind, id, settings, x: 0, y: 0 });
            let last = d.entries.len() - 1;
            let screen_of_end = d.entries.iter().filter(|e| e.kind != Kind::Exit).map(|e| e.screen(d.vertical)).max().unwrap_or(0);
            d.entries[last].x = if d.vertical { 0 } else { screen_of_end * 16 };
            d.entries[last].y = if d.vertical { screen_of_end * 16 } else { 0 };
            index = d.move_entry(last, x, y);
        })?;
        Ok(index)
    }

    /// Place several objects as one change (one undo step): (kind, id, settings, x, y).
    pub fn add_entries(&mut self, level: u16, items: &[(Kind, u8, u8, u32, u32)]) -> Result<(), String> {
        self.change(level, |d| {
            for &(kind, id, settings, x, y) in items {
                d.entries.push(Entry { kind, id, settings, x: 0, y: 0 });
                let last = d.entries.len() - 1;
                let screen_of_end = d.entries.iter().filter(|e| e.kind != Kind::Exit).map(|e| e.screen(d.vertical)).max().unwrap_or(0);
                d.entries[last].x = if d.vertical { 0 } else { screen_of_end * 16 };
                d.entries[last].y = if d.vertical { screen_of_end * 16 } else { 0 };
                d.move_entry(last, x, y);
            }
        })
    }

    /// Delete several objects as one change (one undo step).
    pub fn delete_entries(&mut self, level: u16, indices: &[usize]) -> Result<(), String> {
        let mut sorted = indices.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        self.change(level, |d| {
            for &i in sorted.iter().rev() {
                if i < d.entries.len() {
                    d.entries.remove(i);
                }
            }
        })
    }

    /// Place several sprites as one change: (id, x, y, extra) in blocks.
    pub fn add_sprites(&mut self, level: u16, items: &[(u8, u32, u32, u8)]) -> Result<(), String> {
        self.change_sprites(level, |l| {
            for &(id, x, y, extra) in items {
                let (x, y) = l.clamp(x, y);
                let s = SpriteEntry { id, x, y, extra: extra & 3 };
                if !l.sprites.contains(&s) {
                    l.sprites.push(s);
                }
            }
        })
    }

    /// Delete several sprites as one change.
    pub fn delete_sprites(&mut self, level: u16, indices: &[usize]) -> Result<(), String> {
        let mut sorted = indices.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        self.change_sprites(level, |l| {
            for &i in sorted.iter().rev() {
                if i < l.sprites.len() {
                    l.sprites.remove(i);
                }
            }
        })
    }

    // ---- saving ----

    pub fn save(&mut self) -> Result<Vec<String>, String> {
        let mut report = Vec::new();
        if !self.backed_up {
            let bak = self.path.with_extension(format!(
                "{}.bak",
                self.path.extension().and_then(|e| e.to_str()).unwrap_or("sfc")
            ));
            if !bak.exists() {
                std::fs::copy(&self.path, &bak).map_err(|e| format!("Backup failed: {e}"))?;
                report.push(format!("Backup saved to {}", bak.display()));
            }
            self.backed_up = true;
        }

        self.apply_edits(&mut report)?;
        let mut out = Vec::with_capacity(self.bytes.len() + 512);
        if self.info.had_header {
            out.extend_from_slice(&[0; 512]);
        }
        out.extend_from_slice(&self.bytes);
        std::fs::write(&self.path, out).map_err(|e| format!("Could not write ROM: {e}"))?;
        self.code_dirty = false;
        self.rom = make_rom(&self.bytes);
        self.palettes.clear();
        self.info = romfile::identify(&self.bytes, self.info.had_header);
        report.push(format!("Wrote {}", self.path.display()));
        Ok(report)
    }

    /// The ROM as it would be saved right now (unsaved edits included), without touching the file.
    pub fn build_rom(&self) -> Result<Vec<u8>, String> {
        let mut copy = self.clone();
        copy.apply_edits(&mut Vec::new())?;
        Ok(copy.bytes)
    }

    /// Like build_rom, plus a hook that makes every normal level load (game mode $11, not a
    /// secondary exit) load `level` instead, without the new-file intro message. Used for
    /// in-editor play: boot the game, press through the title, and you land in `level`.
    pub fn build_play_rom(&self, level: u16) -> Result<Vec<u8>, String> {
        let mut copy = self.clone();
        copy.apply_edits(&mut Vec::new())?;
        // $05D8B7 is `REP #$30 / LDA $0E` right after the level number is stored in $0E/$0F
        // (same bytes in vanilla and Lunar Magic hacks; some hacks move it a few bytes on);
        // replace it with a JSL to the hook.
        let Some(site) = (0x05D8B0..0x05D8D0).find(|&a| (0..4).map(|i| copy.read_snes(a + i)).eq([0xC2, 0x30, 0xA5, 0x0E].map(Some))) else {
            let orig: Vec<u8> = (0..4).filter_map(|i| copy.read_snes(0x05D8B7 + i)).collect();
            return Err(format!("This ROM's level loader is different (found {orig:02X?} at $05D8B7), so in-editor play can't jump to a level yet"));
        };
        // RAM $0100-$1FFF: bank $7E, or BW-RAM bank $40 on SA-1 hacks (SA-1 Pack moves it there)
        let ram = if copy.is_sa1() { 0x40 } else { 0x7E };
        #[rustfmt::skip]
        // The map tile the level is entered from ($13BF) and the submap ($1F11): exits pick
        // which half (000-0FF or 100-1FF) they lead to from these, so make them match the level
        let tile = match level {
            0x000..=0x024 => level as u8,
            0x101..=0x13B => (level - 0xDC) as u8,
            0x100..=0x1FF => 0x25,
            _ => 0x00,
        };
        let hook = [
            0xAF, 0x00, 0x01, ram,  // LDA $0100
            0xC9, 0x11,             // CMP #$11      ; normal level load only
            0xD0, 0x20,             // BNE orig
            0xAF, 0x1A, 0x14, ram,  // LDA $141A     ; sublevels entered since the map
            0xD0, 0x1A,             // BNE orig      ; pipes/doors keep their destination
            0xA9, level as u8,      // LDA #lo
            0x85, 0x0E,             // STA $0E
            0xA9, (level >> 8) as u8, // LDA #hi
            0x85, 0x0F,             // STA $0F
            0xA9, tile,             // LDA #tile
            0x8F, 0xBF, 0x13, ram,  // STA $13BF
            0xA9, (level >> 8) as u8, // LDA #hi       ; the original game takes the half from
            0x8F, 0x11, 0x1F, ram,  // STA $1F11     ; the submap (main map = 000-0FF)
            0xA9, 0x00,             // LDA #0
            0x8F, 0x09, 0x01, ram,  // STA $0109     ; no intro message
            // orig:
            0xC2, 0x30,             // REP #$30
            0xA5, 0x0E,             // LDA $0E
            0x6B,                   // RTL
        ];
        let at = copy.alloc_rats(&hook)?;
        copy.write_snes(site, &[0x22, at as u8, (at >> 8) as u8, (at >> 16) as u8]);
        copy.fix_checksum();
        Ok(copy.bytes)
    }

    /// Level numbers the game doesn't use: they all point at the same placeholder level data
    /// (in the original game 277 slots share $06:8000). Filling one replaces nothing.
    pub fn free_levels(&self) -> Vec<u16> {
        let ptrs: Vec<Option<u32>> = (0..0x200u32).map(|l| self.read_ptr(LAYER1_PTRS + l * 3)).collect();
        let mut counts: HashMap<u32, usize> = HashMap::new();
        for p in ptrs.iter().flatten() {
            *counts.entry(*p).or_default() += 1;
        }
        let Some((&placeholder, &n)) = counts.iter().max_by_key(|(_, &n)| n) else { return vec![] };
        if n < 20 {
            return vec![];
        }
        (0..0x200u16).filter(|&l| ptrs[l as usize] == Some(placeholder) && !self.edits.get(&l).is_some_and(|e| e.dirty)).collect()
    }

    /// The song each level asks for as it loads ($1DFB), as the game decides it (header
    /// music setting, Lunar Magic's or SMW Studio's per-level song). Runs the level loader.
    pub fn level_music(&self, level: u16) -> u8 {
        let mut cpu = Cpu::new(emu::CheckedMem::new(self.rom.clone()));
        emu::load_level(&mut cpu, level);
        cpu.mem.game_u8(0x1DFB)
    }

    /// The songs AddmusicK put in this ROM (None if it has no AddmusicK).
    pub fn amk_songs(&self) -> Option<Vec<crate::amk::RomSong>> {
        crate::amk::songs(&self.bytes, &|a| self.read_snes(a))
    }

    /// The ROM's own level names where they differ from the original game's.
    pub fn level_names(&self) -> Vec<(u16, String)> {
        crate::names::custom_names(&|a| self.read_snes(a))
    }

    pub(crate) fn read_snes(&self, addr: u32) -> Option<u8> {
        self.offset(addr).and_then(|o| self.bytes.get(o).copied())
    }

    /// Write every dirty level into `self.bytes` and fix the checksum.
    fn apply_edits(&mut self, report: &mut Vec<String>) -> Result<(), String> {
        let mut levels: Vec<u16> = self.edits.iter().filter(|(_, e)| e.dirty).map(|(l, _)| *l).collect();
        levels.sort();
        for level in levels {
            let (old_ptr, old_len, bytes) = {
                let e = &self.edits[&level];
                (e.ptr, e.len, e.data.encode())
            };
            let shared = (0..0x200u32)
                .filter(|l| self.read_ptr(LAYER1_PTRS + l * 3) == Some(old_ptr))
                .count()
                > 1;
            let new_ptr = if bytes.len() <= old_len && !shared {
                self.write_snes(old_ptr, &bytes);
                report.push(format!("Level {level:03X}: saved in place ({} bytes)", bytes.len()));
                old_ptr
            } else {
                self.free_rats(old_ptr);
                let p = self.alloc_rats(&bytes)?;
                self.write_snes(LAYER1_PTRS + level as u32 * 3, &p.to_le_bytes()[..3]);
                report.push(format!("Level {level:03X}: moved to ${p:06X} ({} bytes)", bytes.len()));
                p
            };
            let e = self.edits.get_mut(&level).unwrap();
            e.ptr = new_ptr;
            e.len = bytes.len();
            e.dirty = false;
            self.save_sprites(level, report)?;
            self.save_entrances(level);
        }
        self.save_songs()?;

        self.fix_checksum();
        Ok(())
    }

    // ---- level songs ----
    //
    // The header's music setting only picks one of 8 songs. To let a level play any song
    // (custom AddmusicK songs are 29 and up), `LDA $0584DB,X` at $05:8551 (where the game
    // turns the header setting into a song) becomes a JSL to a routine that looks the level
    // up in a 512-byte table first. The level number is Lunar Magic's $010B. 0 = no change.

    const SONG_HOOK: u32 = 0x058551;
    #[rustfmt::skip]
    const SONG_ROUTINE: [u8; 22] = [
        0xDA,                   // PHX           (X = header setting, 8-bit)
        0xC2, 0x10,             // REP #$10
        0xAE, 0x0B, 0x01,       // LDX $010B     ; level number
        0xBF, 0, 0, 0,          // LDA table,X   (filled in)
        0xE2, 0x10,             // SEP #$10
        0xFA,                   // PLX
        0xC9, 0x00,             // CMP #$00
        0xD0, 0x04,             // BNE done
        0xBF, 0xDB, 0x84, 0x05, // LDA $0584DB,X ; the header's song
        0x6B,                   // done: RTL
    ];

    /// The per-level song table, when the hook is installed.
    fn song_table(&self) -> Option<u32> {
        if self.read_snes(Self::SONG_HOOK)? != 0x22 {
            return None;
        }
        let at = u32::from_le_bytes([self.read_snes(Self::SONG_HOOK + 1)?, self.read_snes(Self::SONG_HOOK + 2)?, self.read_snes(Self::SONG_HOOK + 3)?, 0]);
        let head: Vec<u8> = (0..6).map(|i| self.read_snes(at + i).unwrap_or(0)).collect();
        (head == Self::SONG_ROUTINE[..6]).then_some(at + Self::SONG_ROUTINE.len() as u32)
    }

    /// Whether levels can play any song: needs Lunar Magic's level code (it keeps the level
    /// number in $010B), i.e. a ROM saved by Lunar Magic.
    pub fn can_pick_any_song(&self) -> bool {
        self.song_table().is_some() || (0..4).map(|i| self.read_snes(0x05D8E2 + i)).collect::<Vec<_>>() == [Some(0x22), Some(0x50), Some(0xF5), Some(0x0E)]
    }

    /// The song a level plays: an AddmusicK song number, or 0 for the header's music setting.
    pub fn level_song(&mut self, level: u16) -> Result<u8, String> {
        Ok(self.edit(level)?.song)
    }

    pub fn set_level_song(&mut self, level: u16, song: u8) -> Result<(), String> {
        if song != 0 && !self.can_pick_any_song() {
            return Err("Picking any song needs a ROM saved with Lunar Magic at least once. Until then, use the music setting.".into());
        }
        let ed = self.edit(level)?;
        let snap = ed.snapshot();
        ed.undo.push(snap);
        ed.redo.clear();
        ed.song = song;
        ed.dirty = true;
        Ok(())
    }

    fn save_songs(&mut self) -> Result<(), String> {
        let wanted: Vec<(u16, u8)> = self.edits.iter().map(|(&l, e)| (l, e.song)).collect();
        let table = match self.song_table() {
            Some(t) => t,
            None if wanted.iter().all(|&(_, s)| s == 0) => return Ok(()),
            None => {
                let mut block = Self::SONG_ROUTINE.to_vec();
                block.extend_from_slice(&[0u8; 0x200]);
                let at = self.alloc_rats(&block)?;
                let table = at + Self::SONG_ROUTINE.len() as u32;
                self.write_snes(at + 7, &table.to_le_bytes()[..3]);
                self.write_snes(Self::SONG_HOOK, &[0x22, at as u8, (at >> 8) as u8, (at >> 16) as u8]);
                table
            }
        };
        for (level, song) in wanted {
            self.write_snes(table + level as u32, &[song]);
        }
        Ok(())
    }

    // ---- sprite lists ----
    //
    // Sprite pointers are 16-bit at $05:EC00 (2 bytes per level). The original game keeps
    // every list in bank $07; Lunar Magic adds a bank byte per level at $0E:F100, read by
    // a routine at $0E:F300 that replaces `LDA #$07 / STA $D0` at $05:D8F5.

    const SPRITE_PTRS: u32 = 0x05EC00;
    const SPRITE_BANKS: u32 = 0x0EF100;
    const SPRITE_BANK_HOOK: u32 = 0x05D8F5;
    const SPRITE_BANK_CODE: u32 = 0x0EF300;
    /// Lunar Magic's routine, byte for byte: PHB PHK PLB LDY $0E LDA $F100,Y STA $D0 PLB RTL
    const LM_SPRITE_BANK_ROUTINE: [u8; 12] = [0x8B, 0x4B, 0xAB, 0xA4, 0x0E, 0xB9, 0x00, 0xF1, 0x85, 0xD0, 0xAB, 0x6B];

    fn has_sprite_banks(&self) -> bool {
        (0..4).map(|i| self.read_snes(Self::SPRITE_BANK_HOOK + i)).collect::<Vec<_>>() == [Some(0x22), Some(0x00), Some(0xF3), Some(0x0E)]
    }

    /// Where this level's sprite list starts (SNES address)
    pub fn sprite_ptr(&self, level: u16) -> Option<u32> {
        let a = Self::SPRITE_PTRS + level as u32 * 2;
        let lo = self.read_snes(a)? as u32 | (self.read_snes(a + 1)? as u32) << 8;
        let bank = if self.has_sprite_banks() { self.read_snes(Self::SPRITE_BANKS + level as u32)? } else { 0x07 };
        Some((bank as u32) << 16 | lo)
    }

    /// Give an original ROM Lunar Magic's sprite bank table, so sprite lists can grow and move
    /// anywhere. Identical to what Lunar Magic installs, so the ROM stays compatible with it.
    fn install_sprite_banks(&mut self) -> Result<(), String> {
        let free = (0..0x20Cu32).all(|i| self.read_snes(Self::SPRITE_BANKS + i) == Some(0xFF));
        if !free {
            return Err("This ROM uses the space Lunar Magic's sprite table needs, so its sprite lists can't grow".into());
        }
        self.write_snes(Self::SPRITE_BANKS, &[0x07; 0x200]);
        self.write_snes(Self::SPRITE_BANK_CODE, &Self::LM_SPRITE_BANK_ROUTINE);
        self.write_snes(Self::SPRITE_BANK_HOOK, &[0x22, 0x00, 0xF3, 0x0E]);
        Ok(())
    }

    fn save_sprites(&mut self, level: u16, report: &mut Vec<String>) -> Result<(), String> {
        let (old_ptr, old_len, bytes) = {
            let e = &self.edits[&level];
            (e.sprite_ptr, e.sprite_len, e.sprites.encode())
        };
        if old_len == 0 {
            return Ok(()); // unreadable list: never written
        }
        let original: Vec<u8> = (0..old_len as u32).filter_map(|i| self.read_snes(old_ptr + i)).collect();
        if original == bytes {
            return Ok(());
        }
        let shared = (0..0x200u16).filter(|&l| self.sprite_ptr(l) == Some(old_ptr)).count() > 1;
        let new_ptr = if bytes.len() <= old_len && !shared {
            self.write_snes(old_ptr, &bytes);
            report.push(format!("Level {level:03X}: sprites saved in place ({} sprites)", (bytes.len() - 2) / 3));
            old_ptr
        } else {
            if !self.has_sprite_banks() {
                self.install_sprite_banks()?;
                report.push("Added Lunar Magic's sprite bank table so sprite lists can grow".into());
            }
            self.free_rats(old_ptr);
            let p = self.alloc_rats(&bytes)?;
            self.write_snes(Self::SPRITE_PTRS + level as u32 * 2, &[p as u8, (p >> 8) as u8]);
            self.write_snes(Self::SPRITE_BANKS + level as u32, &[(p >> 16) as u8]);
            report.push(format!("Level {level:03X}: sprites moved to ${p:06X} ({} sprites)", (bytes.len() - 2) / 3));
            p
        };
        let e = self.edits.get_mut(&level).unwrap();
        e.sprite_ptr = new_ptr;
        e.sprite_len = bytes.len();
        Ok(())
    }

    fn offset(&self, addr: u32) -> Option<usize> {
        self.rom.mapper().map_to_file(addr as usize)
    }

    /// SA-1 hacks open for looking (and playing); saving them isn't supported yet.
    pub fn is_sa1(&self) -> bool {
        crate::sa1::is_sa1(&self.bytes)
    }

    /// Where this level's layer 1 data starts (SNES address)
    pub fn layer1_ptr(&self, level: u16) -> Option<u32> {
        self.read_ptr(LAYER1_PTRS + level as u32 * 3)
    }

    fn read_ptr(&self, addr: u32) -> Option<u32> {
        let o = self.offset(addr)?;
        Some(u32::from_le_bytes([*self.bytes.get(o)?, *self.bytes.get(o + 1)?, *self.bytes.get(o + 2)?, 0]))
    }

    fn write_snes(&mut self, addr: u32, data: &[u8]) {
        for (i, b) in data.iter().enumerate() {
            if let Some(o) = self.offset(addr + i as u32) {
                self.bytes[o] = *b;
            }
        }
    }

    /// Valid RATS tags in the ROM as (file offset of tag, total protected length incl. tag).
    fn rats_blocks(&self) -> Vec<(usize, usize)> {
        let mut out = Vec::new();
        let b = &self.bytes;
        let mut i = 0x80000.min(b.len());
        while i + 8 <= b.len() {
            if &b[i..i + 4] == b"STAR" {
                let size = u16::from_le_bytes([b[i + 4], b[i + 5]]);
                let inv = u16::from_le_bytes([b[i + 6], b[i + 7]]);
                if size ^ inv == 0xFFFF {
                    let total = size as usize + 1 + 8;
                    out.push((i, total));
                    i += total;
                    continue;
                }
            }
            i += 1;
        }
        out
    }

    /// If `ptr` points just after one of our RATS tags, clear the tag and its data.
    pub(crate) fn free_rats(&mut self, ptr: u32) {
        let Some(o) = self.offset(ptr) else { return };
        if o < 8 || o < 0x80000 {
            return;
        }
        if let Some(&(tag, total)) = self.rats_blocks().iter().find(|(t, _)| t + 8 == o) {
            self.bytes[tag..tag + total].fill(0);
        }
    }

    /// Find free space (zeros, not RATS-protected, inside one bank) for a tagged block;
    /// expand the ROM to 1 MB, then 2 MB, if needed. Returns the SNES address of the data.
    fn alloc_rats(&mut self, data: &[u8]) -> Result<u32, String> {
        let need = data.len() + 8;
        loop {
            let blocks = self.rats_blocks();
            let protected = |o: usize| blocks.iter().any(|&(t, n)| o >= t && o < t + n);
            let mut start = 0x80000;
            let mut run = 0;
            let mut o = 0x80000;
            let mapper = self.rom.mapper();
            // Only space the game can see (SA-1 cartridges show some 1 MB blocks only)
            let reachable = |o: usize| {
                let a = mapper.map_to_addr(o);
                a != usize::MAX && mapper.map_to_file(a) == Some(o)
            };
            // SA-1 hacks: take space from the end backwards (where Lunar Magic keeps its free
            // space), never the first megabyte, where the game's and patches' code lives and a
            // run of zeros isn't necessarily free
            let sa1 = self.is_sa1();
            let mut found = None;
            while o < self.bytes.len() {
                if o % 0x8000 == 0 {
                    run = 0;
                }
                if self.bytes[o] == 0 && !protected(o) && reachable(o) && !(sa1 && o < 0x100000) {
                    if run == 0 {
                        start = o;
                    }
                    run += 1;
                    // Leave one zero byte before a new tag so tags never touch
                    if run >= need + 1 {
                        found = Some(start);
                        if !sa1 {
                            break;
                        }
                        run = 0; // keep looking for a later one
                    }
                } else {
                    run = 0;
                }
                o += 1;
            }
            if let Some(start) = found {
                {
                        let tag = start + 1;
                        let size = (data.len() - 1) as u16;
                        self.bytes[tag..tag + 4].copy_from_slice(b"STAR");
                        self.bytes[tag + 4..tag + 6].copy_from_slice(&size.to_le_bytes());
                        self.bytes[tag + 6..tag + 8].copy_from_slice(&(size ^ 0xFFFF).to_le_bytes());
                        self.bytes[tag + 8..tag + 8 + data.len()].copy_from_slice(data);
                        return Ok(self.rom.mapper().map_to_addr(tag + 8) as u32);
                }
            }
            self.expand()?;
        }
    }

    fn expand(&mut self) -> Result<(), String> {
        if self.is_sa1() {
            return Err("This SA-1 hack has no free space left where the game can reach it".into());
        }
        let (new_size, code) = match self.bytes.len() {
            0x80000 => (0x100000, 0x0A),
            0x100000 => (0x200000, 0x0B),
            n => return Err(format!("Out of free space (ROM is {} KB)", n / 1024)),
        };
        self.bytes.resize(new_size, 0);
        self.bytes[0x7FD7] = code;
        Ok(())
    }

    fn fix_checksum(&mut self) {
        self.bytes[0x7FDC..0x7FE0].copy_from_slice(&[0xFF, 0xFF, 0x00, 0x00]);
        let sum = checksum(&self.bytes);
        self.bytes[0x7FDC..0x7FDE].copy_from_slice(&(sum ^ 0xFFFF).to_le_bytes());
        self.bytes[0x7FDE..0x7FE0].copy_from_slice(&sum.to_le_bytes());
    }

    pub fn entry_kind_name(kind: Kind) -> &'static str {
        match kind {
            Kind::Standard => "Object",
            Kind::Extended => "Extended object",
            Kind::Exit => "Screen exit",
        }
    }
}
