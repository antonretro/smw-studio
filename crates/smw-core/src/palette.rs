//! Object palette: names and previews drawn by
//! the game itself, using a given level's tileset.

use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::{
    emu,
    level_data::{Entry, Kind, LevelData},
    render,
    rom::Rom,
    Cpu,
};

#[derive(Deserialize)]
struct NameEntry {
    name: String,
    desc: String,
}

#[derive(Deserialize)]
struct NameFile {
    standard: std::collections::HashMap<String, NameEntry>,
    extended: std::collections::HashMap<String, NameEntry>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PaletteItem {
    pub kind:     &'static str,
    pub id:       u8,
    pub settings: u8,
    pub name:     String,
    pub desc:     String,
    /// Preview size in pixels and PNG bytes (empty if the object drew nothing)
    pub w:        u32,
    pub h:        u32,
    /// Where the preview's top-left block sits relative to the object's own position, in
    /// blocks (negative for objects that draw up or left of where they're placed)
    pub ox:       i32,
    pub oy:       i32,
    #[serde(skip)]
    pub png:      Vec<u8>,
}

const UNUSED: &str = "null or unused";
const INTERNAL: &str = "Internal ";
const RESERVED: &str = "Reserved for";

/// Every placeable object: standard 01-3F and extended 02-97, minus unused/internal ones.
pub fn catalog() -> Vec<PaletteItem> {
    let names: NameFile = serde_json::from_str(include_str!("../data/object_names.json").trim_start_matches('\u{feff}')).expect("object names");
    let usable = |d: &str| !d.contains(UNUSED) && !d.starts_with(INTERNAL) && !d.starts_with(RESERVED);
    let mut out = Vec::new();
    for id in 1..=0x3Fu8 {
        let (name, desc) = match names.standard.get(&format!("{id:02X}")) {
            Some(n) if usable(&n.desc) => (n.name.clone(), n.desc.clone()),
            Some(_) => continue,
            None if id >= 0x30 => (format!("Tileset object {id:02X}"), "Depends on the level's tileset.".into()),
            None => continue,
        };
        // Size nibbles: most objects read settings as height/width; 0x11 gives a small 2x2 sample
        out.push(PaletteItem { kind: "object", id, settings: 0x11, name, desc, w: 0, h: 0, ox: 0, oy: 0, png: vec![] });
    }
    for id in 0x02..=0x97u8 {
        if let Some(n) = names.extended.get(&format!("{id:02X}")) {
            if usable(&n.desc) && id >= 0x10 {
                out.push(PaletteItem { kind: "extended", id, settings: 0, name: n.name.clone(), desc: n.desc.clone(), w: 0, h: 0, ox: 0, oy: 0, png: vec![] });
            }
        }
    }
    out
}

/// Draw previews for `items` using level `level`'s header (tileset, palette, graphics).
pub fn render_previews(rom: Arc<Rom>, level: u16, header: [u8; 5], items: &mut [PaletteItem]) {
    // Horizontal, no layer 2, 32 screens: one object per screen
    let mut h = header;
    h[0] = (h[0] & 0xE0) | 0x1F;
    h[1] &= 0xE0;
    const PER: usize = 32;
    std::thread::scope(|s| {
        for chunk in items.chunks_mut(PER) {
            let rom = rom.clone();
            s.spawn(move || render_chunk(rom, level, h, chunk));
        }
    });
}

fn render_chunk(rom: Arc<Rom>, level: u16, header: [u8; 5], chunk: &mut [PaletteItem]) {
    let entries: Vec<Entry> = chunk
        .iter()
        .enumerate()
        .map(|(i, it)| Entry {
            kind:     if it.kind == "object" { Kind::Standard } else { Kind::Extended },
            id:       it.id,
            settings: it.settings,
            x:        i as u32 * 16 + 4,
            y:        10,
        })
        .collect();
    let data = LevelData { header, entries, vertical: false };
    let (bytes, starts) = data.encode_with_offsets();

    let mut cpu = Cpu::new(emu::CheckedMem::new(rom));
    emu::load_level_with(&mut cpu, level, Some(&bytes));
    let has_layer2 = render::has_layer2(&mut cpu);
    let full = render::render_layer1_transparent(&mut cpu);

    // Which blocks each entry owns
    let mut owned: Vec<Vec<(u32, u32)>> = vec![vec![]; chunk.len()];
    for (idx, &off) in cpu.mem.owner.iter().enumerate() {
        if off == u32::MAX || (off as usize) < 5 {
            continue;
        }
        let e = match starts.binary_search(&(off as usize)) {
            Ok(i) => i,
            Err(0) => continue,
            Err(i) => i - 1,
        };
        if let Some(b) = render::layer1_block(idx as u32, false, has_layer2) {
            owned[e].push(b);
        }
    }

    for (i, (item, blocks)) in chunk.iter_mut().zip(owned).enumerate() {
        if blocks.is_empty() {
            continue;
        }
        let x0 = blocks.iter().map(|b| b.0).min().unwrap();
        let y0 = blocks.iter().map(|b| b.1).min().unwrap();
        let x1 = blocks.iter().map(|b| b.0).max().unwrap();
        let y1 = blocks.iter().map(|b| b.1).max().unwrap();
        let (w, h) = ((x1 - x0 + 1) * 16, (y1 - y0 + 1) * 16);
        let mut rgba = vec![0u8; (w * h * 4) as usize];
        for &(bx, by) in &blocks {
            for py in 0..16 {
                let sy = by * 16 + py;
                let dy = (by - y0) * 16 + py;
                let s = ((sy * full.width + bx * 16) * 4) as usize;
                let d = ((dy * w + (bx - x0) * 16) * 4) as usize;
                rgba[d..d + 64].copy_from_slice(&full.rgba[s..s + 64]);
            }
        }
        item.w = w;
        item.h = h;
        item.ox = x0 as i32 - (i as i32 * 16 + 4);
        item.oy = y0 as i32 - 10;
        item.png = encode_png(w, h, &rgba);
    }
}

fn encode_png(w: u32, h: u32, rgba: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut out, w, h);
        enc.set_color(png::ColorType::Rgba);
        enc.write_header().unwrap().write_image_data(rgba).unwrap();
    }
    out
}
