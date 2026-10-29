//! The status bar (layer 3, top of the screen): what it shows and hiding parts of it.
//!
//! The game draws it from four tables of tilemap entries (tile, YXPCCCTT) in bank 0 when a
//! level starts, and every frame copies two rows of tile numbers from RAM ($0EF9 and $0F15,
//! where the counters are written) to the screen at $00:8DAC. Hiding a part blanks its tiles
//! in that RAM right before the copy (so the counters keep working underneath), and for the
//! item box's top and bottom edges, which aren't copied each frame, blanks them in the tables.

use serde::Serialize;

use crate::project::Project;

/// The rows 2 and 3 tables: ROM address, screen row, first column, tiles. Their tiles are
/// also copied into the RAM buffer when a level starts.
const MIDDLE: [(u32, u32, u32, u32); 2] = [(0x008C89, 2, 2, 28), (0x008CC1, 3, 3, 27)];

/// Rows 1 and 4 (the item box's top and bottom edges in the original game, 4 tiles) are
/// copied once by a DMA job each: (job parameters, `LDA #` operand of the VRAM address low
/// byte, row). The designer widens them to the full 32 tiles from a table in free space.
const EDGE_JOBS: [(u32, u32, u32); 2] = [(0x008D90, 0x008D05, 1), (0x008DA5, 0x008D62, 4)];

/// Tables drawn when a level starts: (ROM address, row, first column, tiles).
fn tables(p: &Project) -> Vec<(u32, u32, u32, u32)> {
    let mut out = Vec::new();
    for &(job, _, row) in &EDGE_JOBS {
        let b = |i: u32| p.read_snes(job + i).unwrap_or(0) as u32;
        let (src, size) = (b(2) | b(3) << 8 | b(4) << 16, b(5) | b(6) << 8);
        let col = if size >= 0x40 { 0 } else { 14 };
        out.push((src, row, col, (size / 2).min(32)));
    }
    out.extend_from_slice(&MIDDLE);
    out.sort_by_key(|t| t.1);
    out
}

/// Give rows 1 and 4 a full-width table (copying what they show now).
fn widen_edges(p: &mut Project) -> Result<(), String> {
    for &(job, dest, row) in &EDGE_JOBS {
        let t = tables(p).into_iter().find(|t| t.1 == row).unwrap();
        if t.3 == 32 {
            continue;
        }
        let mut table = Vec::with_capacity(64);
        for col in 0..32 {
            if col >= t.2 && col < t.2 + t.3 {
                let a = t.0 + (col - t.2) * 2;
                table.extend_from_slice(&[p.read_snes(a).unwrap_or(0xFC), p.read_snes(a + 1).unwrap_or(0x38)]);
            } else {
                table.extend_from_slice(&[0xFC, 0x38]);
            }
        }
        let at = p.alloc_code(&table)?;
        p.patch(job + 2, &[at as u8, (at >> 8) as u8, (at >> 16) as u8, 0x40, 0x00]);
        p.patch(dest, &[(row * 32) as u8]);
    }
    Ok(())
}

/// Rows of the status bar shown in previews (rows 1-4 of layer 3)
pub const ROWS: u32 = 4;

/// The per-frame copy: `STZ $2115 / LDA #$42`, replaced by a JSL to the hiding routine.
const COPY_HOOK: u32 = 0x008DAC;
const COPY_ORIG: [u8; 5] = [0x9C, 0x15, 0x21, 0xA9, 0x42];

/// Drawing the reserve item (a sprite in the item box): `JSR $9079`.
const ITEM_DRAW: u32 = 0x008FC5;
const ITEM_DRAW_ORIG: [u8; 6] = [0x20, 0x79, 0x90, 0xAD, 0xB3, 0x0D]; // and LDA $0DB3

/// A part of the status bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Part {
    pub id:   &'static str,
    pub name: &'static str,
    /// Its fixed squares (labels, icons) as (row 1-4, first column, last column); its counter,
    /// if it has one, is wherever the code puts it (see `part_cells`)
    pub cells: &'static [(u32, u32, u32)],
}

pub const PARTS: [Part; 8] = [
    Part { id: "name", name: "Player name", cells: &[(2, 2, 6)] },
    Part { id: "lives", name: "Lives", cells: &[(3, 3, 3)] },
    Part { id: "dragon", name: "Dragon coins", cells: &[] },
    Part { id: "stars", name: "Bonus stars", cells: &[(3, 9, 10), (2, 12, 13), (3, 12, 13)] },
    Part { id: "item", name: "Item box", cells: &[(1, 14, 17), (2, 14, 17), (3, 14, 17), (4, 14, 17)] },
    Part { id: "time", name: "Time", cells: &[(2, 19, 21)] },
    Part { id: "coins", name: "Coins", cells: &[(2, 25, 26)] },
    Part { id: "score", name: "Score", cells: &[(3, 29, 29)] },
];

/// Offset into the $0EF9 buffer of a cell on rows 2-3 (None on rows 1 and 4).
fn buffer_offset(row: u32, col: u32) -> Option<u8> {
    match row {
        2 if (2..30).contains(&col) => Some((col - 2) as u8),
        3 if (3..30).contains(&col) => Some((0x1C + col - 3) as u8),
        _ => None,
    }
}

/// Address of the tilemap entry for a cell in the ROM tables.
fn table_entry(p: &Project, row: u32, col: u32) -> Option<u32> {
    tables(p).into_iter().find(|t| t.1 == row && col >= t.2 && col < t.2 + t.3).map(|t| t.0 + (col - t.2) * 2)
}

/// Whether this ROM's status bar code is the one SMW Studio knows (else nothing can be hidden).
pub fn supported(p: &Project) -> Result<(), String> {
    if p.is_sa1() {
        return Err("SA-1 hacks keep the status bar elsewhere; editing it isn't supported for them yet.".into());
    }
    let at = |a: u32, n: usize| (0..n as u32).map(|i| p.read_snes(a + i)).collect::<Option<Vec<u8>>>();
    let copy = at(COPY_HOOK, 5).ok_or("bad ROM")?;
    if copy != COPY_ORIG && copy[0] != 0x22 {
        return Err("This ROM's status bar code was changed by another patch, so SMW Studio leaves it alone.".into());
    }
    Ok(())
}

/// Length of the routine's code; its table of offsets follows
const CODE_LEN: u32 = 29;

/// The routine the per-frame copy jumps to: blank `offsets` in the RAM buffer, then do the
/// two instructions it replaced. Runs in the NMI with 8-bit A and X.
fn hide_routine(offsets: &[u8], at: u32) -> Vec<u8> {
    let table = at + CODE_LEN;
    let mut c = vec![
        0xDA, //                         PHX
        0xA2, offsets.len() as u8 - 1, // LDX #n-1
        // loop:
        0xDA, //                         PHX
        0xBF, table as u8, (table >> 8) as u8, (table >> 16) as u8, // LDA.l offsets,X
        0xAA, //                         TAX
        0xA9, 0xFC, //                   LDA #$FC (blank tile)
        0x9F, 0xF9, 0x0E, 0x7E, //       STA.l $7E0EF9,X
        0xFA, //                         PLX
        0xCA, //                         DEX
        0x10, 0xF0, //                   BPL loop
        0xFA, //                         PLX
        0xA9, 0x00, 0x8F, 0x15, 0x21, 0x00, // LDA #0 / STA.l $002115 (was STZ $2115)
        0xA9, 0x42, //                   LDA #$42
        0x6B, //                         RTL
    ];
    debug_assert_eq!(c.len() as u32, CODE_LEN);
    c.extend_from_slice(offsets);
    c
}

/// Where the per-frame copy's JSL goes.
fn hook_target(p: &Project) -> u32 {
    (1..4).map(|i| p.read_snes(COPY_HOOK + i).unwrap_or(0) as u32).enumerate().fold(0, |a, (i, b)| a | b << (8 * i))
}

/// The buffer offsets the ROM currently blanks every frame (empty if none).
fn hidden_offsets(p: &Project) -> Vec<u8> {
    if p.read_snes(COPY_HOOK) != Some(0x22) {
        return vec![];
    }
    let at = hook_target(p);
    let n = p.read_snes(at + 2).map_or(0, |n| n as u32 + 1);
    (0..n).filter_map(|i| p.read_snes(at + CODE_LEN + i)).collect()
}

/// Fixed squares of each part in the original game: (part id, row, column, tile). Hiding a
/// part clears them (if they still show that tile); showing it again puts them back.
const LABELS: [(&str, u32, u32, u8); 21] = [
    ("lives", 3, 3, 0x26),
    ("stars", 3, 9, 0x64),
    ("stars", 3, 10, 0x26),
    ("item", 1, 14, 0x3A),
    ("item", 1, 15, 0x3B),
    ("item", 1, 16, 0x3B),
    ("item", 1, 17, 0x3A),
    ("item", 2, 14, 0x4A),
    ("item", 2, 17, 0x4A),
    ("item", 3, 14, 0x4A),
    ("item", 3, 17, 0x4A),
    ("item", 4, 14, 0x3A),
    ("item", 4, 15, 0x3B),
    ("item", 4, 16, 0x3B),
    ("item", 4, 17, 0x3A),
    ("time", 2, 19, 0x3D),
    ("time", 2, 20, 0x3E),
    ("time", 2, 21, 0x3F),
    ("coins", 2, 25, 0x2E),
    ("coins", 2, 26, 0x26),
    ("score", 3, 29, 0x00),
];

/// Squares the game rewrites every frame for a part (so hiding blanks them every frame):
/// its counter, the bonus star digits, and the player name (Luigi's is written there).
fn frame_cells(p: &Project, id: &str) -> Vec<(u32, u32, u32)> {
    if id == "name" {
        return vec![(2, 2, 6)];
    }
    live(p).into_iter().filter(|l| l.0 == id).map(|l| (l.1, l.2, l.3)).collect()
}

/// Which parts are hidden in this ROM.
pub fn hidden(p: &Project) -> Vec<&'static str> {
    let offs = hidden_offsets(p);
    let item_off = !item_drawn(p);
    PARTS
        .iter()
        .filter(|part| {
            if part.id == "item" {
                return item_off;
            }
            let first = frame_cells(p, part.id).iter().find_map(|&(r, c, _)| buffer_offset(r, c));
            first.is_some_and(|o| offs.contains(&o))
        })
        .map(|part| part.id)
        .collect()
}

/// Hide exactly `ids` (and show the rest).
pub fn set_hidden(p: &mut Project, ids: &[String]) -> Result<(), String> {
    supported(p)?;
    let before = hidden(p);
    let parts: Vec<&Part> = PARTS.iter().filter(|part| ids.iter().any(|i| i == part.id)).collect();
    // Fixed squares: cleared when a part gets hidden, put back when it's shown again
    for &(id, row, col, tile) in &LABELS {
        let (was, now) = (before.contains(&id), parts.iter().any(|x| x.id == id));
        if was == now {
            continue;
        }
        if let Some(e) = table_entry(p, row, col) {
            let cur = p.read_snes(e);
            if now && cur == Some(tile) {
                p.patch(e, &[0xFC]);
            } else if !now && cur == Some(0xFC) {
                p.patch(e, &[tile]);
            }
        }
    }
    let mut offsets: Vec<u8> = parts.iter().flat_map(|part| frame_cells(p, part.id)).flat_map(|(r, a, b)| (a..=b).filter_map(move |c| buffer_offset(r, c))).collect();
    offsets.sort();
    offsets.dedup();
    // Remove the old routine, then put in the new one (or the original bytes)
    if p.read_snes(COPY_HOOK) == Some(0x22) {
        p.free_rats(hook_target(p));
    }
    if offsets.is_empty() {
        p.patch(COPY_HOOK, &COPY_ORIG);
    } else {
        // The routine refers to its own table, so place it, then write it with the address
        let at = p.alloc_code(&vec![0xFF; CODE_LEN as usize + offsets.len()])?;
        p.patch(at, &hide_routine(&offsets, at));
        p.patch(COPY_HOOK, &[0x22, at as u8, (at >> 8) as u8, (at >> 16) as u8, 0xEA]);
    }
    // The item inside the box is a sprite
    let item = parts.iter().any(|part| part.id == "item");
    let st = stickers(p);
    write_sprites(p, !item, &st)
}

/// The status bar's tilemap entries as the game starts a level (rows 1-4, 32 columns;
/// 0x00FC where nothing is drawn), with hidden parts blanked.
pub fn tilemap(p: &Project) -> Vec<u16> {
    let mut out = vec![0x38FCu16; (ROWS * 32) as usize];
    for (addr, row, col, n) in tables(p) {
        for i in 0..n {
            let t = p.read_snes(addr + i * 2).unwrap_or(0xFC) as u16;
            let prop = p.read_snes(addr + i * 2 + 1).unwrap_or(0x38) as u16;
            out[((row - 1) * 32 + col + i) as usize] = prop << 8 | t;
        }
    }
    let offs = hidden_offsets(p);
    for row in 2..=3 {
        for col in 0..32 {
            if buffer_offset(row, col).is_some_and(|o| offs.contains(&o)) {
                let e = &mut out[((row - 1) * 32 + col) as usize];
                *e = (*e & 0xFF00) | 0xFC;
            }
        }
    }
    out
}

/// A picture of the status bar (256 x 32), drawn with the graphics and colours level
/// `level` loads, on black. Hidden parts are left out.
pub fn picture(p: &Project, level: u16) -> crate::render::Image {
    let mut cpu = crate::Cpu::new(crate::emu::CheckedMem::new(p.rom_arc()));
    crate::emu::load_level(&mut cpu, level);
    let (vram, cgram) = (&cpu.mem.vram, &cpu.mem.cgram);
    let mut img = crate::render::Image::new(256, ROWS * 8, [0, 0, 0, 255]);
    for (i, &e) in tilemap(p).iter().enumerate() {
        draw_l3(&mut img, vram, cgram, (i as u32 % 32) * 8, (i as u32 / 32) * 8, e);
    }
    draw_stickers(&mut img, vram, cgram, &stickers(p));
    img
}

/// Draw one layer 3 tilemap entry (2 bits per pixel; graphics at VRAM $4000) at (x, y).
fn draw_l3(img: &mut crate::render::Image, vram: &[u8], cgram: &[u8], cx: u32, cy: u32, e: u16) {
    let tile = (e & 0x3FF) as usize;
    let pal = ((e >> 10) & 7) as usize;
    let (fx, fy) = (e & 0x4000 != 0, e & 0x8000 != 0);
    let base = 0x8000 + tile * 16;
    for y in 0..8usize {
        let row = if fy { 7 - y } else { y };
        let (lo, hi) = (vram.get(base + row * 2).copied().unwrap_or(0), vram.get(base + row * 2 + 1).copied().unwrap_or(0));
        for x in 0..8usize {
            let bit = if fx { x } else { 7 - x };
            let c = ((lo >> bit) & 1) | (((hi >> bit) & 1) << 1);
            if c == 0 {
                continue;
            }
            let ci = (pal * 4 + c as usize) * 2;
            let v = u16::from_le_bytes([cgram[ci], cgram[ci + 1]]);
            let ch = |s: u16| (((v >> s) & 0x1F) << 3 | ((v >> s) & 0x1F) >> 2) as u8;
            let px = ((cy as usize + y) * img.width as usize + cx as usize + x) * 4;
            if px + 4 <= img.rgba.len() {
                img.rgba[px..px + 4].copy_from_slice(&[ch(0), ch(5), ch(10), 255]);
            }
        }
    }
}

/// A counter the game writes into the RAM buffer every frame, and where its code says so:
/// each site is (address of an instruction's operand, operand size, offset from the first
/// digit's RAM address). An 8-bit site holds an index from $0F15 (row 3) instead.
struct Counter {
    id:     &'static str,
    digits: u32,
    sites:  &'static [(u32, u32, i32)],
    /// Only row 3 (the score's digit routine indexes from $0F15)
    row3_only: bool,
}

const COUNTERS: [Counter; 5] = [
    // STX $0F16 / STA $0F17
    Counter { id: "lives", digits: 2, sites: &[(0x008F56, 2, 0), (0x008F59, 2, 1)], row3_only: false },
    // STA $0EFF,X (4 icons)
    Counter { id: "dragon", digits: 4, sites: &[(0x008FF0, 2, 0)], row3_only: false },
    // STA $0F25 / $0F26 / $0F27, and STA $0F15,X blanking leading zeros with X from $10
    Counter { id: "time", digits: 3, sites: &[(0x008E73, 2, 0), (0x008E79, 2, 1), (0x008E7F, 2, 2), (0x008E8D, 2, -0x10)], row3_only: false },
    // STX $0F13 / STA $0F14
    Counter { id: "coins", digits: 2, sites: &[(0x008F82, 2, 0), (0x008F7F, 2, 1)], row3_only: false },
    // LDX #$14 before the digit routine (twice: Mario, Luigi), LDA/STA $0F29,X blanking zeros
    Counter { id: "score", digits: 6, sites: &[(0x008ED8, 1, 0), (0x008F06, 1, 0), (0x008EE1, 2, 0), (0x008EE8, 2, 0), (0x008F0F, 2, 0), (0x008F16, 2, 0)], row3_only: true },
];

/// RAM buffer address of a cell (rows 2-3).
fn buffer_addr(row: u32, col: u32) -> Option<u32> {
    buffer_offset(row, col).map(|o| 0x0EF9 + o as u32)
}

/// The cell of a RAM buffer address.
fn buffer_cell(addr: u32) -> Option<(u32, u32)> {
    match addr {
        0x0EF9..=0x0F14 => Some((2, addr - 0x0EF9 + 2)),
        0x0F15..=0x0F2F => Some((3, addr - 0x0F15 + 3)),
        _ => None,
    }
}

/// Where a counter's first digit is now (row, column), if its code is as SMW Studio knows it.
fn counter_at(p: &Project, c: &Counter) -> Option<(u32, u32)> {
    let read = |&(a, size, delta): &(u32, u32, i32)| -> Option<i32> {
        let v = if size == 1 { p.read_snes(a)? as i32 + 0x0F15 } else { p.read_snes(a)? as i32 | (p.read_snes(a + 1)? as i32) << 8 };
        Some(v - delta)
    };
    let base = read(&c.sites[0])?;
    if c.sites.iter().any(|s| read(s) != Some(base)) {
        return None;
    }
    let (row, col) = buffer_cell(base as u32)?;
    (buffer_cell(base as u32 + c.digits - 1) == Some((row, col + c.digits - 1))).then_some((row, col))
}

/// Move a counter so its first digit is at (row, col). Its old squares are blanked and the
/// new ones take the old ones' colours.
pub fn move_counter(p: &mut Project, id: &str, row: u32, col: u32) -> Result<(), String> {
    supported(p)?;
    let c = COUNTERS.iter().find(|c| c.id == id).ok_or("That part can't be moved")?;
    let (old_row, old_col) = counter_at(p, c).ok_or("This ROM's code for that counter was changed by another patch, so SMW Studio leaves it alone.")?;
    let last = col + c.digits - 1;
    let fits = buffer_addr(row, col).is_some() && buffer_addr(row, last).is_some() && (!c.row3_only || row == 3);
    if !fits {
        return Err(if c.row3_only {
            format!("The score needs {} squares in a row on the lower line (columns 3-29).", c.digits)
        } else {
            format!("It needs {} squares in a row on the middle lines (upper: columns 2-29, lower: 3-29).", c.digits)
        });
    }
    // Not on top of another counter (both would write the same squares)
    if let Some(other) = live(p).into_iter().find(|l| l.0 != id && l.1 == row && l.2 <= last && col <= l.3) {
        let name = PARTS.iter().find(|x| x.id == other.0).map_or(other.0, |x| x.name);
        return Err(format!("That's on top of the {} counter. Move it somewhere else first.", name.to_lowercase()));
    }
    let hidden: Vec<String> = hidden(p).into_iter().map(String::from).collect();
    let base = buffer_addr(row, col).unwrap();
    for &(a, size, delta) in c.sites {
        let v = base as i32 + delta;
        if size == 1 {
            p.patch(a, &[(v - 0x0F15) as u8]);
        } else {
            p.patch(a, &(v as u16).to_le_bytes());
        }
    }
    // Squares: old ones blank (keeping colours for whatever goes there next), new ones take
    // the old ones' colours and flips
    let props: Vec<u8> = (0..c.digits).map(|i| table_entry(p, old_row, old_col + i).and_then(|e| p.read_snes(e + 1)).unwrap_or(0x38)).collect();
    for i in 0..c.digits {
        if let Some(e) = table_entry(p, old_row, old_col + i) {
            p.patch(e, &[0xFC]);
        }
    }
    for i in 0..c.digits {
        if let Some(e) = table_entry(p, row, col + i) {
            p.patch(e, &[0x00, props[i as usize]]);
        }
    }
    // Hidden parts follow the counter
    set_hidden(p, &hidden)
}

/// Cells whose tile the game rewrites every frame: (part id, row, first column, last column).
fn live(p: &Project) -> Vec<(&'static str, u32, u32, u32)> {
    let mut out: Vec<_> = COUNTERS.iter().filter_map(|c| counter_at(p, c).map(|(r, col)| (c.id, r, col, col + c.digits - 1))).collect();
    // Bonus stars: big digits, not movable
    out.extend([("stars", 2, 12, 13), ("stars", 3, 12, 13)]);
    out
}

/// A part's squares now: its labels plus its counter wherever it is.
fn part_cells(p: &Project, part: &Part) -> Vec<(u32, u32, u32)> {
    let mut out = part.cells.to_vec();
    out.extend(live(p).into_iter().filter(|l| l.0 == part.id).map(|l| (l.1, l.2, l.3)));
    out.dedup();
    out
}

/// Every part's squares now, for showing where it is: (id, [(row, first column, last column)]).
pub fn layout(p: &Project) -> Vec<(&'static str, Vec<(u32, u32, u32)>)> {
    PARTS.iter().map(|part| (part.id, part_cells(p, part))).collect()
}

/// Which counters can be moved, and where each is: (id, row, column, digits).
pub fn counters(p: &Project) -> Vec<(&'static str, u32, u32, u32)> {
    COUNTERS.iter().filter_map(|c| counter_at(p, c).map(|(r, col)| (c.id, r, col, c.digits))).collect()
}

/// One cell of the status bar, for the designer.
#[derive(Debug, Clone, Serialize)]
pub struct Cell {
    pub row:   u32,
    pub col:   u32,
    /// Tilemap entry: tile (bits 0-9), palette (10-12), priority (13), flips (14 X, 15 Y)
    pub entry: u16,
    /// Can be changed: it's in one of the tables (rows 1 and 4 can be widened to all of it)
    pub editable: bool,
    /// The part whose counter the game writes here every frame (its tile can't be changed)
    pub live: Option<&'static str>,
}

/// Every cell of rows 1-4.
pub fn cells(p: &Project) -> Vec<Cell> {
    let map = tilemap(p);
    let t = tables(p);
    let live_cells = live(p);
    (0..ROWS * 32)
        .map(|i| {
            let (row, col) = (i / 32 + 1, i % 32);
            let in_table = t.iter().any(|t| t.1 == row && col >= t.2 && col < t.2 + t.3);
            let live = live_cells.iter().find(|l| l.1 == row && col >= l.2 && col <= l.3).map(|l| l.0);
            Cell { row, col, entry: map[i as usize], editable: in_table || row == 1 || row == 4, live }
        })
        .collect()
}

/// Change cells: (row 1-4, column, tilemap entry). Rows 1 and 4 are widened first if needed.
pub fn set_cells(p: &mut Project, changes: &[(u32, u32, u16)]) -> Result<(), String> {
    supported(p)?;
    if changes.iter().any(|&(r, c, _)| (r == 1 || r == 4) && table_entry(p, r, c).is_none()) {
        widen_edges(p)?;
    }
    for &(row, col, entry) in changes {
        let a = table_entry(p, row, col).ok_or(format!("Row {row}, column {col} can't be changed"))?;
        p.patch(a, &entry.to_le_bytes());
    }
    Ok(())
}

/// The layer 3 tiles 0-511 in palette `pal` (16 per row: 128 x 256), drawn with the graphics
/// level `level` loads: for picking tiles.
pub fn tiles_picture(p: &Project, level: u16, pal: u16) -> crate::render::Image {
    let mut cpu = crate::Cpu::new(crate::emu::CheckedMem::new(p.rom_arc()));
    crate::emu::load_level(&mut cpu, level);
    let mut img = crate::render::Image::new(128, 256, [0, 0, 0, 255]);
    for t in 0..512u32 {
        draw_l3(&mut img, &cpu.mem.vram, &cpu.mem.cgram, (t % 16) * 8, (t / 16) * 8, (pal & 7) << 10 | t as u16);
    }
    img
}

/// A whole status bar: what's drawn in every square, where each counter goes and what's hidden.
#[derive(Debug, Clone, Default)]
pub struct Layout {
    pub hidden:   Vec<&'static str>,
    /// (counter id, row, column of its first digit); every counter needs a place
    pub counters: Vec<(&'static str, u32, u32)>,
    /// (row, column, tilemap entry); every other square is left blank
    pub cells:    Vec<(u32, u32, u16)>,
}

/// Put a whole layout in: all squares blank, then the layout's squares, counters and hidden
/// parts.
pub fn apply_layout(p: &mut Project, l: &Layout) -> Result<(), String> {
    supported(p)?;
    // Check the counters first: each fits, none on another (or on the bonus star digits)
    let mut taken: Vec<(u32, u32, u32)> = vec![(2, 12, 13), (3, 12, 13)];
    for &(id, row, col) in &l.counters {
        let c = COUNTERS.iter().find(|c| c.id == id).ok_or(format!("No counter {id}"))?;
        counter_at(p, c).ok_or("This ROM's counter code was changed by another patch, so SMW Studio leaves it alone.")?;
        let last = col + c.digits - 1;
        if buffer_addr(row, col).is_none() || buffer_addr(row, last).is_none() || (c.row3_only && row != 3) {
            return Err(format!("The {id} counter doesn't fit at row {row}, column {col}"));
        }
        if taken.iter().any(|t| t.0 == row && t.1 <= last && col <= t.2) {
            return Err(format!("The {id} counter overlaps another one"));
        }
        taken.push((row, col, last));
    }
    if COUNTERS.iter().any(|c| !l.counters.iter().any(|x| x.0 == c.id)) {
        return Err("Every counter needs a place (hide the ones you don't want)".into());
    }
    if l.cells.iter().any(|&(r, c, _)| (r == 1 || r == 4) && !(14..18).contains(&c)) {
        widen_edges(p)?;
    }
    // Start from nothing hidden, then blank every square
    set_hidden(p, &[])?;
    for (addr, _, _, n) in tables(p) {
        for i in 0..n {
            p.patch(addr + i * 2, &[0xFC, 0x38]);
        }
    }
    for &(id, row, col) in &l.counters {
        let c = COUNTERS.iter().find(|c| c.id == id).unwrap();
        let base = buffer_addr(row, col).unwrap();
        for &(a, size, delta) in c.sites {
            let v = base as i32 + delta;
            if size == 1 {
                p.patch(a, &[(v - 0x0F15) as u8]);
            } else {
                p.patch(a, &(v as u16).to_le_bytes());
            }
        }
    }
    for &(row, col, entry) in &l.cells {
        let a = table_entry(p, row, col).ok_or(format!("Row {row}, column {col} can't be drawn on"))?;
        p.patch(a, &entry.to_le_bytes());
    }
    set_hidden(p, &l.hidden.iter().map(|s| s.to_string()).collect::<Vec<_>>())
}

// Colours of the original bar (palette and priority bits of a tilemap entry)
const WHITE: u16 = 0x3800; // palette 6
const GOLD: u16 = 0x3C00; // palette 7
const RED: u16 = 0x2800; // palette 2

/// Squares for `s` from (row, col), in colour `prop`: letters, digits and . , - ! ×
fn text(row: u32, col: u32, s: &str, prop: u16) -> Vec<(u32, u32, u16)> {
    s.chars()
        .enumerate()
        .filter_map(|(i, ch)| {
            let t = match ch.to_ascii_uppercase() {
                d @ '0'..='9' => d as u16 - '0' as u16,
                c @ 'A'..='Z' => 0x0A + c as u16 - 'A' as u16,
                '.' => 0x24,
                ',' => 0x25,
                '×' => 0x26,
                '-' => 0x27,
                '!' => 0x28,
                _ => return None,
            };
            Some((row, col + i as u32, prop | t))
        })
        .collect()
}

/// Squares from (row, col) to the right.
fn raw(row: u32, col: u32, entries: &[u16]) -> Vec<(u32, u32, u16)> {
    entries.iter().enumerate().map(|(i, &e)| (row, col + i as u32, e)).collect()
}

// The game's own pictures used by the presets
const COIN: u16 = GOLD | 0x2E;
const TIMES: u16 = WHITE | 0x26;
const STAR: u16 = RED | 0x64;
const MARIO: [u16; 5] = [RED | 0x30, RED | 0x31, RED | 0x32, RED | 0x33, RED | 0x34];
const TIME_LABEL: [u16; 3] = [GOLD | 0x3D, GOLD | 0x3E, GOLD | 0x3F];

/// The item box (columns 14-17, where the game draws the item): top edge, sides, bottom edge.
fn item_box() -> Vec<(u32, u32, u16)> {
    let mut v = raw(1, 14, &[WHITE | 0x3A, WHITE | 0x3B, WHITE | 0x3B, 0x7800 | 0x3A]);
    for row in [2, 3] {
        v.push((row, 14, WHITE | 0x4A));
        v.push((row, 17, 0x7800 | 0x4A));
    }
    v.extend(raw(4, 14, &[0xB800 | 0x3A, 0xB800 | 0x3B, 0xB800 | 0x3B, 0xF800 | 0x3A]));
    v
}

/// Ready-made status bars: (id, name, what it is, layout).
pub fn presets() -> Vec<(&'static str, &'static str, &'static str, Layout)> {
    // A counter as the game first shows it: blanks, then 0
    let digits = |row: u32, col: u32, n: usize, prop: u16| {
        let mut v = vec![prop | 0xFC; n];
        v[n - 1] = prop;
        raw(row, col, &v)
    };
    let mut out = Vec::new();

    // The original game's bar
    let mut c = Vec::new();
    c.extend(raw(2, 2, &MARIO));
    c.extend(raw(2, 8, &[GOLD | 0xFC; 4]));
    c.extend(raw(2, 19, &TIME_LABEL));
    c.extend(raw(2, 25, &[COIN, TIMES]));
    c.extend(digits(2, 28, 2, WHITE));
    c.extend(raw(3, 3, &[TIMES]));
    c.extend(digits(3, 4, 2, WHITE));
    c.extend(raw(3, 9, &[STAR, TIMES]));
    c.extend(digits(3, 19, 3, GOLD));
    c.extend(digits(3, 23, 7, WHITE));
    c.extend(item_box());
    out.push(("classic", "Classic", "The original game's status bar", Layout {
        hidden: vec![],
        counters: vec![("lives", 3, 4), ("dragon", 2, 8), ("time", 3, 19), ("coins", 2, 28), ("score", 3, 23)],
        cells: c,
    }));

    // Just coins and lives, in the corners
    let mut c = Vec::new();
    c.extend(raw(2, 2, &[COIN, TIMES]));
    c.extend(digits(2, 4, 2, WHITE));
    c.extend(raw(2, 24, &MARIO));
    c.extend(raw(3, 26, &[TIMES]));
    c.extend(digits(3, 27, 2, WHITE));
    out.push(("minimal", "Minimal", "Only coins and lives, tucked into the corners", Layout {
        hidden: vec!["dragon", "stars", "item", "time", "score"],
        counters: vec![("coins", 2, 4), ("lives", 3, 27), ("time", 3, 19), ("dragon", 2, 8), ("score", 3, 3)],
        cells: c,
    }));

    // Everything on one line
    let mut c = Vec::new();
    c.extend(raw(2, 2, &MARIO));
    c.extend(raw(2, 7, &[TIMES]));
    c.extend(digits(2, 8, 2, WHITE));
    c.extend(raw(2, 15, &TIME_LABEL));
    c.extend(digits(2, 19, 3, GOLD));
    c.extend(raw(2, 25, &[COIN, TIMES]));
    c.extend(digits(2, 27, 2, WHITE));
    out.push(("one-line", "One line", "Lives, time and coins across a single line", Layout {
        hidden: vec!["dragon", "stars", "item", "score"],
        counters: vec![("lives", 2, 8), ("time", 2, 19), ("coins", 2, 27), ("dragon", 3, 4), ("score", 3, 20)],
        cells: c,
    }));

    // Score first, arcade style
    let mut c = Vec::new();
    c.extend(text(2, 3, "SCORE", GOLD));
    c.extend(digits(3, 3, 7, WHITE));
    // (columns 12-13 are the bonus star digits, which the game draws every frame)
    c.extend(raw(2, 14, &[COIN, TIMES]));
    c.extend(digits(2, 16, 2, WHITE));
    c.extend(text(3, 14, "UP", GOLD));
    c.extend(digits(3, 16, 2, WHITE));
    c.extend(raw(2, 24, &TIME_LABEL));
    c.extend(digits(3, 24, 3, GOLD));
    out.push(("arcade", "Arcade", "Big score on the left, like an arcade game", Layout {
        hidden: vec!["dragon", "stars", "item"],
        counters: vec![("score", 3, 3), ("coins", 2, 16), ("lives", 3, 16), ("time", 3, 24), ("dragon", 2, 20)],
        cells: c,
    }));

    // The item box in the middle, counters around it
    let mut c = Vec::new();
    c.extend(raw(2, 2, &MARIO));
    c.extend(raw(3, 3, &[TIMES]));
    c.extend(digits(3, 4, 2, WHITE));
    c.extend(raw(2, 8, &[GOLD | 0xFC; 4]));
    c.extend(item_box());
    c.extend(raw(2, 20, &TIME_LABEL));
    c.extend(digits(3, 20, 3, GOLD));
    c.extend(raw(2, 25, &[COIN, TIMES]));
    c.extend(digits(2, 27, 2, WHITE));
    out.push(("adventure", "Adventure", "Item box in the middle and no score: for hacks about exploring", Layout {
        hidden: vec!["stars", "score"],
        counters: vec![("lives", 3, 4), ("dragon", 2, 8), ("time", 3, 20), ("coins", 2, 27), ("score", 3, 23)],
        cells: c,
    }));

    // Nothing at all
    out.push(("none", "No status bar", "A clean screen: everything hidden (it all still counts)", Layout {
        hidden: vec!["name", "lives", "dragon", "stars", "item", "time", "coins", "score"],
        counters: vec![("lives", 3, 4), ("dragon", 2, 8), ("time", 3, 19), ("coins", 2, 28), ("score", 3, 23)],
        cells: vec![],
    }));
    out
}

/// Put preset `id` in.
pub fn apply_preset(p: &mut Project, id: &str) -> Result<(), String> {
    let l = presets().into_iter().find(|x| x.0 == id).ok_or("No such preset")?.3;
    apply_layout(p, &l)
}

/// Each preset with a picture of it in this ROM (drawn with level `level`'s graphics):
/// (id, name, description, picture). The ROM isn't changed.
pub fn preset_pictures(p: &Project, level: u16) -> Vec<(&'static str, &'static str, &'static str, crate::render::Image)> {
    presets()
        .into_iter()
        .filter_map(|(id, name, about, l)| {
            let mut copy = p.clone();
            if let Err(e) = apply_layout(&mut copy, &l) {
                eprintln!("status bar preset {id}: {e}");
                return None;
            }
            Some((id, name, about, picture(&copy, level)))
        })
        .collect()
}

/// A piece to drop onto the bar: its squares from the drop point (column offset, tilemap
/// entry), and for counters, which part it is and where its first digit goes.
pub struct Element {
    pub id:      &'static str,
    pub name:    &'static str,
    /// The part this shows (it's un-hidden when placed)
    pub part:    Option<&'static str>,
    /// The counter's first digit, as a column offset
    pub counter: Option<u32>,
    pub squares: Vec<(u32, u16)>,
}

impl Element {
    /// Squares wide.
    pub fn width(&self) -> u32 {
        let digits = self.counter.map_or(0, |c| c + COUNTERS.iter().find(|x| Some(x.id) == self.part).map_or(0, |x| x.digits));
        self.squares.iter().map(|s| s.0 + 1).max().unwrap_or(0).max(digits)
    }
}

/// Everything that can be dropped onto the bar.
pub fn elements() -> Vec<Element> {
    let blanks = |n: u32, prop: u16| (0..n).map(move |i| (i, prop | if i + 1 == n { 0x00 } else { 0xFC }));
    let shift = |v: Vec<(u32, u16)>, by: u32| v.into_iter().map(|(c, e)| (c + by, e)).collect::<Vec<_>>();
    vec![
        Element { id: "coins", name: "Coins", part: Some("coins"), counter: Some(2), squares: [vec![(0, COIN), (1, TIMES)], shift(blanks(2, WHITE).collect(), 2)].concat() },
        Element { id: "lives", name: "Lives", part: Some("lives"), counter: Some(1), squares: [vec![(0, TIMES)], shift(blanks(2, WHITE).collect(), 1)].concat() },
        Element { id: "time", name: "Time", part: Some("time"), counter: Some(4), squares: [TIME_LABEL.iter().enumerate().map(|(i, &e)| (i as u32, e)).collect(), shift(blanks(3, GOLD).collect(), 4)].concat() },
        Element { id: "score", name: "Score (lower line)", part: Some("score"), counter: Some(0), squares: blanks(7, WHITE).collect() },
        Element { id: "dragon", name: "Dragon coins", part: Some("dragon"), counter: Some(0), squares: (0..4).map(|i| (i, GOLD | 0xFC)).collect() },
        Element { id: "name", name: "MARIO", part: Some("name"), counter: None, squares: MARIO.iter().enumerate().map(|(i, &e)| (i as u32, e)).collect() },
        Element { id: "coin-icon", name: "Coin icon", part: None, counter: None, squares: vec![(0, COIN)] },
        Element { id: "star-icon", name: "Star icon", part: None, counter: None, squares: vec![(0, STAR)] },
        Element { id: "times", name: "×", part: None, counter: None, squares: vec![(0, TIMES)] },
        Element { id: "dash", name: "-", part: None, counter: None, squares: vec![(0, WHITE | 0x27)] },
    ]
}

/// Drop element `id` with its first square at (row, col). A counter moves there (its old
/// label squares are cleared if they still show its label) and its part is shown.
pub fn place_element(p: &mut Project, id: &str, row: u32, col: u32) -> Result<(), String> {
    supported(p)?;
    let el = elements().into_iter().find(|e| e.id == id).ok_or("No such element")?;
    if col + el.width() > 32 || (1..=ROWS).all(|r| r != row) {
        return Err(format!("{} needs {} squares from there", el.name, el.width()));
    }
    // Every square must be one the game draws (rows 1 and 4 can be widened)
    let drawable = |p: &Project, c: u32| table_entry(p, row, c).is_some() || row == 1 || row == 4;
    if (col..col + el.width()).any(|c| !drawable(p, c)) {
        return Err(format!("{} doesn't fit there: the game doesn't draw the squares at the edges of that line", el.name));
    }
    if let (Some(off), Some(part)) = (el.counter, el.part) {
        let c = COUNTERS.iter().find(|c| c.id == part).unwrap();
        // Move the counter (this checks it fits), then clear its old label squares where
        // they still show the label: around the counter, and where the original game has it
        if let Some((orow, ocol)) = counter_at(p, c) {
            move_counter(p, part, row, col + off)?;
            for &(_, r, cl, tile) in LABELS.iter().filter(|l| l.0 == part) {
                if let Some(a) = table_entry(p, r, cl) {
                    if p.read_snes(a) == Some(tile) {
                        p.patch(a, &[0xFC]);
                    }
                }
            }
            if (orow, ocol) != (row, col + off) {
                for &(dc, e) in &el.squares {
                    let (dc, oc) = (dc as i32 - off as i32, ocol as i32);
                    if (0..c.digits as i32).contains(&dc) || oc + dc < 0 {
                        continue;
                    }
                    if let Some(a) = table_entry(p, orow, (oc + dc) as u32) {
                        if p.read_snes(a) == Some(e as u8) {
                            p.patch(a, &[0xFC]);
                        }
                    }
                }
            }
        }
    }
    let changes: Vec<(u32, u32, u16)> = el.squares.iter().map(|&(dc, e)| (row, col + dc, e)).collect();
    set_cells(p, &changes)?;
    if let Some(part) = el.part {
        let hidden: Vec<String> = hidden(p).into_iter().filter(|h| *h != part).map(String::from).collect();
        set_hidden(p, &hidden)?;
    }
    Ok(())
}

/// Each element as a small picture with level `level`'s graphics: (id, name, picture).
pub fn element_pictures(p: &Project, level: u16) -> Vec<(&'static str, &'static str, crate::render::Image)> {
    let mut cpu = crate::Cpu::new(crate::emu::CheckedMem::new(p.rom_arc()));
    crate::emu::load_level(&mut cpu, level);
    elements()
        .into_iter()
        .map(|el| {
            let mut img = crate::render::Image::new(el.width() * 8, 8, [0, 0, 0, 255]);
            for &(dc, e) in &el.squares {
                draw_l3(&mut img, &cpu.mem.vram, &cpu.mem.cgram, dc * 8, 0, e);
            }
            (el.id, el.name, img)
        })
        .collect()
}

/// A sprite drawn on top of the status bar every frame in levels (from the level's sprite
/// graphics: the item box's own picture works the same way).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, serde::Deserialize)]
pub struct Sticker {
    /// Screen position in pixels (the bar is y 8-39)
    pub x:      u8,
    pub y:      u8,
    /// Sprite tile 0-511 (the top-left one for 16x16)
    pub tile:   u16,
    /// Sprite colours 0-7 (CGRAM palettes 8-15)
    pub pal:    u8,
    pub flip_x: bool,
    pub flip_y: bool,
    /// 16x16 (else 8x8)
    pub big:    bool,
}

/// Stickers use sprite slots 1-6 (OAM $0204-$021B): free in levels (slot 0 is the item box's
/// place in the Bowser fight)
pub const MAX_STICKERS: usize = 6;

/// The routine `JSR $9079 / LDA $0DB3` at $00:8FC5 is replaced with (via JSL), run every frame
/// in levels: draw the item box's item (unless hidden) by calling $9079 in bank 0 through an
/// RTL there, copy the stickers into their sprite slots, call each per-frame routine (code
/// from patches, see `frame_calls`), then do the `LDA $0DB3` it replaced. The sticker table
/// follows the code.
fn sprite_routine(p: &Project, at: u32, item: bool, st: &[Sticker], calls: &[u32]) -> Result<Vec<u8>, String> {
    // An RTL byte in bank 0, for returning here from $9079's RTS
    let rtl0 = (0x8000..0xFFFFu32).find(|&a| p.read_snes(a) == Some(0x6B)).ok_or("bad ROM")?;
    let ret = at + 15;
    let loops = if st.is_empty() { 0 } else { 26 };
    let table = at + 15 + loops + calls.len() as u32 * 4 + 5;
    let sizes = table + st.len() as u32 * 4;
    let n = st.len() as u8;
    let mut c = vec![
        0xA9, item as u8, //                     LDA #item
        0xF0, 0x0B, //                           BEQ skip
        0x4B, //                                 PHK                (for $9079's way back)
        0xF4, (ret - 1) as u8, ((ret - 1) >> 8) as u8, // PEA ret-1
        0xF4, (rtl0 - 1) as u8, ((rtl0 - 1) >> 8) as u8, // PEA rtl0-1 (its RTS goes to an RTL)
        0x5C, 0x79, 0x90, 0x00, //               JML $009079
        // ret / skip:
    ];
    if !st.is_empty() {
        c.extend_from_slice(&[
            0xA2, n * 4 - 1, //                      LDX #n*4-1
            0xBF, table as u8, (table >> 8) as u8, (table >> 16) as u8, // LDA.l table,X
            0x9F, 0x04, 0x02, 0x7E, //               STA.l $7E0204,X    (slots 1-6)
            0xCA, //                                 DEX
            0x10, 0xF5, //                           BPL
            0xA2, n - 1, //                          LDX #n-1
            0xBF, sizes as u8, (sizes >> 8) as u8, (sizes >> 16) as u8, // LDA.l sizes,X
            0x9F, 0x21, 0x04, 0x7E, //               STA.l $7E0421,X    (size bits)
            0xCA, //                                 DEX
            0x10, 0xF5, //                           BPL
        ]);
    }
    for &a in calls {
        c.extend_from_slice(&[0x22, a as u8, (a >> 8) as u8, (a >> 16) as u8]); // JSL routine
    }
    c.extend_from_slice(&[0xAF, 0xB3, 0x0D, 0x7E, 0x6B]); // LDA.l $7E0DB3 / RTL
    debug_assert_eq!(at + c.len() as u32, table);
    for s in st {
        let attr = 0x30 | (s.flip_y as u8) << 7 | (s.flip_x as u8) << 6 | (s.pal & 7) << 1 | (s.tile >> 8) as u8 & 1;
        c.extend_from_slice(&[s.x, s.y, s.tile as u8, attr]);
    }
    c.extend(st.iter().map(|s| if s.big { 2 } else { 0 }));
    Ok(c)
}

/// The sprite routine's address, if it's in.
fn sprite_hook(p: &Project) -> Option<u32> {
    if p.read_snes(ITEM_DRAW) != Some(0x22) {
        return None;
    }
    Some((1..4).map(|i| p.read_snes(ITEM_DRAW + i).unwrap_or(0) as u32).enumerate().fold(0, |a, (i, b)| a | b << (8 * i)))
}

/// Whether the item box's item is drawn.
fn item_drawn(p: &Project) -> bool {
    match sprite_hook(p) {
        Some(at) => p.read_snes(at + 1) != Some(0),
        None => p.read_snes(ITEM_DRAW) != Some(0xEA),
    }
}

/// What the sprite routine holds: (sticker count, per-frame calls, where the table starts).
fn sprite_layout(p: &Project, at: u32) -> (usize, Vec<u32>, u32) {
    let mut a = at + 15;
    let n = if p.read_snes(a) == Some(0xA2) {
        let n = p.read_snes(a + 1).map_or(0, |v| (v as usize + 1) / 4);
        a += 26;
        n
    } else {
        0
    };
    let mut calls = Vec::new();
    while p.read_snes(a) == Some(0x22) && calls.len() < 32 {
        calls.push((1..4).map(|i| p.read_snes(a + i).unwrap_or(0) as u32).enumerate().fold(0, |x, (i, b)| x | b << (8 * i)));
        a += 4;
    }
    (n, calls, a + 5)
}

/// The stickers in this ROM.
pub fn stickers(p: &Project) -> Vec<Sticker> {
    let Some(at) = sprite_hook(p) else { return vec![] };
    let (n, _, table) = sprite_layout(p, at);
    let sizes = table + n as u32 * 4;
    (0..n as u32)
        .filter_map(|i| {
            let b = |k: u32| p.read_snes(table + i * 4 + k);
            let (x, y, t, attr) = (b(0)?, b(1)?, b(2)?, b(3)?);
            Some(Sticker {
                x,
                y,
                tile: (attr as u16 & 1) << 8 | t as u16,
                pal: (attr >> 1) & 7,
                flip_x: attr & 0x40 != 0,
                flip_y: attr & 0x80 != 0,
                big: p.read_snes(sizes + i) == Some(2),
            })
        })
        .collect()
}

/// Routines called every frame in levels (from patches with per-frame code).
pub fn frame_calls(p: &Project) -> Vec<u32> {
    sprite_hook(p).map(|at| sprite_layout(p, at).1).unwrap_or_default()
}

/// Put in the item box's item (shown or not), the stickers and the per-frame calls.
fn write_sprites_and_calls(p: &mut Project, item: bool, st: &[Sticker], calls: &[u32]) -> Result<(), String> {
    let cur = (0..6).map(|i| p.read_snes(ITEM_DRAW + i)).collect::<Option<Vec<u8>>>().ok_or("bad ROM")?;
    let ours = cur[0] == 0x22 || cur == ITEM_DRAW_ORIG || cur[..3] == [0xEA; 3] && cur[3..] == ITEM_DRAW_ORIG[3..];
    if !ours {
        return Err("This ROM's item box code was changed by another patch, so SMW Studio leaves it alone.".into());
    }
    if let Some(at) = sprite_hook(p) {
        p.free_rats(at);
    }
    if st.is_empty() && calls.is_empty() {
        let mut b = ITEM_DRAW_ORIG;
        if !item {
            b[..3].copy_from_slice(&[0xEA; 3]);
        }
        p.patch(ITEM_DRAW, &b);
    } else {
        let len = 15 + 26 + calls.len() * 4 + 5 + st.len() * 5;
        let at = p.alloc_code(&vec![0xFF; len])?;
        let code = sprite_routine(p, at, item, st, calls)?;
        p.patch(at, &code);
        p.patch(ITEM_DRAW, &[0x22, at as u8, (at >> 8) as u8, (at >> 16) as u8, 0xEA, 0xEA]);
    }
    Ok(())
}

fn write_sprites(p: &mut Project, item: bool, st: &[Sticker]) -> Result<(), String> {
    let calls = frame_calls(p);
    write_sprites_and_calls(p, item, st, &calls)
}

/// Replace the per-frame calls (code patches add theirs here).
pub fn set_frame_calls(p: &mut Project, calls: &[u32]) -> Result<(), String> {
    supported(p)?;
    let (item, st) = (item_drawn(p), stickers(p));
    write_sprites_and_calls(p, item, &st, calls)
}

/// Replace the stickers (at most six).
pub fn set_stickers(p: &mut Project, st: &[Sticker]) -> Result<(), String> {
    supported(p)?;
    if st.len() > MAX_STICKERS {
        return Err(format!("At most {MAX_STICKERS} sprites fit on the bar"));
    }
    let item = item_drawn(p);
    write_sprites(p, item, st)
}

/// The level's sprite tiles 0-511 in sprite colours `pal` (16 per row: 128 x 256), for picking.
pub fn sprite_tiles_picture(p: &Project, level: u16, pal: u8) -> crate::render::Image {
    let mut cpu = crate::Cpu::new(crate::emu::CheckedMem::new(p.rom_arc()));
    crate::emu::load_level(&mut cpu, level);
    let mut img = crate::render::Image::new(128, 256, [0, 0, 0, 255]);
    for t in 0..512u32 {
        crate::render::draw_tile(&mut img, &cpu.mem.vram, &cpu.mem.cgram, ((t % 16) * 8) as i32, ((t / 16) * 8) as i32, t + 0x600, 8 + (pal as u32 & 7), 0);
    }
    img
}

/// Draw the stickers onto a status bar picture (which starts at screen y 8).
fn draw_stickers(img: &mut crate::render::Image, vram: &[u8], cgram: &[u8], st: &[Sticker]) {
    for s in st {
        let flips = (s.flip_x as u16) << 14 | (s.flip_y as u16) << 15;
        let parts: &[(i32, i32, u16)] = if s.big { &[(0, 0, 0), (8, 0, 1), (0, 8, 16), (8, 8, 17)] } else { &[(0, 0, 0)] };
        for &(dx, dy, add) in parts {
            let (dx, dy) = (if s.flip_x && s.big { 8 - dx } else { dx }, if s.flip_y && s.big { 8 - dy } else { dy });
            let tile = ((s.tile + add) & 0x1FF) as u32;
            crate::render::draw_tile(img, vram, cgram, s.x as i32 + dx, s.y as i32 - 8 + dy, tile + 0x600, 8 + (s.pal as u32 & 7), flips);
        }
    }
}
