//! Draw a loaded level (emulated WRAM/VRAM/CGRAM) into an RGBA image.
//! Layer layout logic adapted from smw-editor's level_renderer.rs (MIT).

use crate::Cpu;

#[derive(Clone)]
pub struct Image {
    pub width:  u32,
    pub height: u32,
    pub rgba:   Vec<u8>,
}

impl Image {
    /// PNG bytes of this image.
    pub fn png(&self) -> Vec<u8> {
        let mut out = Vec::new();
        {
            let mut e = png::Encoder::new(&mut out, self.width, self.height);
            e.set_color(png::ColorType::Rgba);
            if let Ok(mut w) = e.write_header() {
                let _ = w.write_image_data(&self.rgba);
            }
        }
        out
    }

    pub fn new(width: u32, height: u32, fill: [u8; 4]) -> Self {
        let rgba = fill.iter().copied().cycle().take((width * height * 4) as usize).collect();
        Self { width, height, rgba }
    }
}

/// SNES BGR555 color -> RGBA
fn color(cgram: &[u8], index: usize) -> [u8; 4] {
    let c = u16::from_le_bytes([cgram[index * 2], cgram[index * 2 + 1]]);
    let x = |v: u16| ((v & 0x1F) << 3 | (v & 0x1F) >> 2) as u8;
    [x(c), x(c >> 5), x(c >> 10), 255]
}

/// Draw one 8x8 4bpp tile. `t` is a SNES tilemap entry: tile 0-9, palette 10-12, flips 14/15.
pub(crate) fn draw_tile(img: &mut Image, vram: &[u8], cgram: &[u8], x: i32, y: i32, tile: u32, pal: u32, t: u16) {
    let base = (tile as usize * 32) & 0xFFFF;
    let (flip_x, flip_y) = (t & 0x4000 != 0, t & 0x8000 != 0);
    for row in 0..8 {
        let r = if flip_y { 7 - row } else { row };
        let p = [vram[base + r * 2], vram[base + r * 2 + 1], vram[base + 16 + r * 2], vram[base + 17 + r * 2]];
        for col in 0..8 {
            let bit = 7 - col;
            let v = (p[0] >> bit & 1) | (p[1] >> bit & 1) << 1 | (p[2] >> bit & 1) << 2 | (p[3] >> bit & 1) << 3;
            if v == 0 {
                continue; // transparent
            }
            let px = x + if flip_x { 7 - col as i32 } else { col as i32 };
            let py = y + row as i32;
            if px < 0 || py < 0 || px >= img.width as i32 || py >= img.height as i32 {
                continue;
            }
            let i = ((py as u32 * img.width + px as u32) * 4) as usize;
            img.rgba[i..i + 4].copy_from_slice(&color(cgram, (pal * 16 + v as u32) as usize));
        }
    }
}

/// Layer 1 only, on a transparent background (for object previews).
pub fn render_layer1_transparent(cpu: &mut Cpu) -> Image {
    let info = level_info(cpu);
    let (w, h) = if info.vertical { (512, 256 * info.screens) } else { (256 * info.screens, 432) };
    let mut img = Image::new(w, h, [0, 0, 0, 0]);
    draw_layer(cpu, &mut img, false);
    img
}

/// Draw every level sprite by running its own code. Sprites that draw nothing
/// (generators, invisible helpers) get an outlined 16x16 marker instead.
pub fn draw_sprites(cpu: &mut Cpu, img: &mut Image) -> Vec<crate::sprites::LevelSprite> {
    let list = crate::sprites::parse_sprites(cpu);
    draw_sprite_list(cpu, img, &list);
    list
}

/// Draw `list` (e.g. the editor's edited sprites) and return each sprite's box on the image
/// as [x, y, w, h], for clicking on it.
pub fn draw_sprite_list(cpu: &mut Cpu, img: &mut Image, list: &[crate::sprites::LevelSprite]) -> Vec<[i32; 4]> {
    let cgram = cpu.mem.cgram.clone();
    let base = cpu.clone();
    let mut boxes = Vec::with_capacity(list.len());
    for s in list {
        let tiles = crate::sprites::draw_sprite(&base, s);
        // Tiles far from the sprite's own spot belong to something it spawned or a
        // stray; only count those near it for the clickable box
        let near: Vec<_> = tiles.iter().filter(|t| (t.x - s.x as i32).abs() < 48 && (t.y - s.y as i32).abs() < 64).collect();
        if near.is_empty() {
            draw_marker(img, s.x as i32, s.y as i32);
            boxes.push([s.x as i32, s.y as i32, 16, 16]);
            continue;
        }
        // OAM: earlier slots are in front, so paint back to front
        for t in tiles.iter().rev() {
            let flips = ((t.attr as u16) & 0xC0) << 8;
            let pal = ((t.attr as u32 >> 1) & 7) + 8;
            draw_tile(img, &base.mem.vram, &cgram, t.x, t.y, t.tile as u32 + 0x600, pal, flips);
        }
        let x0 = near.iter().map(|t| t.x).min().unwrap();
        let y0 = near.iter().map(|t| t.y).min().unwrap();
        let x1 = near.iter().map(|t| t.x + 8).max().unwrap();
        let y1 = near.iter().map(|t| t.y + 8).max().unwrap();
        boxes.push([x0, y0, x1 - x0, y1 - y0]);
    }
    boxes
}

/// Draw sprite `id` on its own (as the game would in this level) and crop it. None if it
/// draws nothing on its own (generators, shooters, invisible helpers).
pub fn sprite_preview(base: &Cpu, id: u8) -> Option<Image> {
    sprite_preview_at(base, id).map(|(img, _, _)| img)
}

/// Like sprite_preview, with where the image's top-left is relative to the sprite's own
/// position (most sprites draw up or left of it: a Koopa is 16 pixels taller).
pub fn sprite_preview_at(base: &Cpu, id: u8) -> Option<(Image, i32, i32)> {
    let s = crate::sprites::LevelSprite { id, x: 0x100, y: 0x100, extra: 0 };
    let tiles = crate::sprites::draw_sprite(base, &s);
    let near: Vec<_> = tiles.iter().filter(|t| (t.x - 0x100).abs() < 48 && (t.y - 0x100).abs() < 64).collect();
    if near.is_empty() {
        return None;
    }
    let x0 = near.iter().map(|t| t.x).min().unwrap();
    let y0 = near.iter().map(|t| t.y).min().unwrap();
    let x1 = near.iter().map(|t| t.x + 8).max().unwrap();
    let y1 = near.iter().map(|t| t.y + 8).max().unwrap();
    let mut img = Image { width: (x1 - x0) as u32, height: (y1 - y0) as u32, rgba: vec![0; ((x1 - x0) * (y1 - y0) * 4) as usize] };
    for t in near.iter().rev() {
        let flips = ((t.attr as u16) & 0xC0) << 8;
        let pal = ((t.attr as u32 >> 1) & 7) + 8;
        draw_tile(&mut img, &base.mem.vram, &base.mem.cgram, t.x - x0, t.y - y0, t.tile as u32 + 0x600, pal, flips);
    }
    Some((img, x0 - 0x100, y0 - 0x100))
}

fn draw_marker(img: &mut Image, x: i32, y: i32) {
    for i in 0..16 {
        for (px, py) in [(x + i, y), (x + i, y + 15), (x, y + i), (x + 15, y + i)] {
            if px >= 0 && py >= 0 && (px as u32) < img.width && (py as u32) < img.height {
                let o = ((py as u32 * img.width + px as u32) * 4) as usize;
                img.rgba[o..o + 4].copy_from_slice(&[255, 64, 200, 255]);
            }
        }
    }
}

pub struct LevelInfo {
    pub vertical: bool,
    pub screens:  u32,
    pub mode:     u8,
}

pub fn level_info(cpu: &mut Cpu) -> LevelInfo {
    let vertical = cpu.mem.game_u8(0x5B) & 1 != 0;
    // $5D: screens across (horizontal levels); $5F: screens down (vertical levels)
    let screens = (cpu.mem.game_u8(if vertical { 0x5F } else { 0x5D }) as u32).clamp(1, 0x20);
    LevelInfo { vertical, screens, mode: cpu.mem.game_u8(0x1925) }
}

/// Render layer 2 then layer 1 of the currently loaded level.
pub fn render_level(cpu: &mut Cpu) -> Image {
    let info = level_info(cpu);
    let (w, h) = if info.vertical { (512, 256 * info.screens) } else { (256 * info.screens, 432) };
    // Back area color lives in WRAM $0701 (BGR555); CGRAM 0 isn't set yet at this point
    let back = [cpu.mem.game_u8(0x0701), cpu.mem.game_u8(0x0702)];
    let bg = color(&back, 0);
    let mut img = Image::new(w, h, bg);
    draw_layer(cpu, &mut img, true);
    draw_layer(cpu, &mut img, false);
    img
}

/// The level as two images: the back (back area colour plus the background, or layer 2 in
/// levels that have one) and layer 1 on a transparent background.
pub fn render_level_layers(cpu: &mut Cpu) -> (Image, Image) {
    let info = level_info(cpu);
    let (w, h) = if info.vertical { (512, 256 * info.screens) } else { (256 * info.screens, 432) };
    let back_colour = [cpu.mem.game_u8(0x0701), cpu.mem.game_u8(0x0702)];
    let mut back = Image::new(w, h, color(&back_colour, 0));
    draw_layer(cpu, &mut back, true);
    let mut front = Image::new(w, h, [0, 0, 0, 0]);
    draw_layer(cpu, &mut front, false);
    (back, front)
}

impl Image {
    /// Draw `top` over this image (pixels with alpha 0 are see-through).
    pub fn overlay(&mut self, top: &Image) {
        for (d, s) in self.rgba.chunks_exact_mut(4).zip(top.rgba.chunks_exact(4)) {
            if s[3] != 0 {
                d.copy_from_slice(s);
            }
        }
    }
}

/// Does this level mode use a real layer 2 (instead of a background)?
pub fn has_layer2(cpu: &mut Cpu) -> bool {
    let mode = cpu.mem.game_u8(0x1925);
    let renderer_table = cpu.mem.cart.resolve("CODE_058955").unwrap() + 9;
    let renderer = cpu.mem.game_u24(renderer_table + (mode as u32) * 3);
    let l2 = [cpu.mem.cart.resolve("CODE_058B8D"), cpu.mem.cart.resolve("CODE_058C71")];
    l2.contains(&Some(renderer))
}

/// Block position of layer 1 tile `idx` (offset into the $C800 tables).
pub fn layer1_block(idx: u32, vertical: bool, has_layer2: bool) -> Option<(u32, u32)> {
    let len = if has_layer2 { 256 * 27 } else { 512 * 27 };
    if idx >= len {
        return None;
    }
    Some(if vertical {
        let (screen, s) = (idx / 256, idx % 256);
        (s % 16 + (screen % 2) * 16, s / 16 + (screen / 2) * 16)
    } else {
        let (screen, s) = (idx / (16 * 27), idx % (16 * 27));
        (s % 16 + screen * 16, s / 16)
    })
}

/// Where a Map16 block's four tile words live. Vanilla uses the $0FBE table (foreground)
/// and $0D9100 (background). Lunar Magic replaces both lookups with its own code
/// (JSL at $058A65 for foreground, at $058DA4 for the background base); we run that code
/// on a scratch copy of the machine so any LM version gives the game's own answer.
#[derive(Default)]
struct Map16Lookup {
    scratch: Option<Box<Cpu>>,
    fg_cache: std::collections::HashMap<u32, u32>,
    bg: Option<u32>,
}

impl Map16Lookup {
    fn hook(cpu: &Cpu, at: u32) -> Option<u32> {
        let c = &cpu.mem.cart;
        (c.read(at)? == 0x22).then(|| u32::from_le_bytes([c.read(at + 1).unwrap(), c.read(at + 2).unwrap(), c.read(at + 3).unwrap(), 0]))
    }

    /// Run `REP #$30 / LDA #a / JSL target` (plus `STA $0A` when store) and return the long pointer at $0A.
    fn call(&mut self, cpu: &Cpu, target: u32, a: u16, bank: u8, store: bool) -> u32 {
        let s = self.scratch.get_or_insert_with(|| Box::new(cpu.clone()));
        s.mem.game_store(0x0C, bank);
        let mut code = vec![0xC2, 0x30, 0xA9, a as u8, (a >> 8) as u8, 0x22, target as u8, (target >> 8) as u8, (target >> 16) as u8];
        if store {
            code.extend_from_slice(&[0x85, 0x0A]);
        }
        for (i, b) in code.iter().enumerate() {
            s.mem.store_u8(0x1F00 + i as u32, *b); // code, not a game variable
        }
        let end = 0x1F00 + code.len() as u16;
        s.emulation = false;
        s.pc = 0x1F00;
        s.pbr = 0;
        s.dbr = 0;
        s.d = if s.mem.sa1.is_some() { 0x3000 } else { 0 };
        s.s = 0x1EF;
        let mut n = 0;
        while s.pc != end && !s.ill && n < 100_000 {
            s.dispatch();
            n += 1;
        }
        s.mem.game_u24(0x0A)
    }

    fn fg(&mut self, cpu: &mut Cpu, block: u32, map16_bank: u32) -> u32 {
        if let Some(&p) = self.fg_cache.get(&block) {
            return p;
        }
        let p = match Self::hook(cpu, 0x058A65) {
            Some(target) => self.call(cpu, target, (block * 2) as u16, (map16_bank >> 16) as u8, true),
            None => cpu.mem.game_u16(0x0FBE + block * 2) as u32 + map16_bank,
        };
        self.fg_cache.insert(block, p);
        p
    }

    fn bg_base(&mut self, cpu: &mut Cpu, vanilla: u32) -> u32 {
        if let Some(b) = self.bg {
            return b;
        }
        let b = match Self::hook(cpu, 0x058DA4) {
            Some(target) => self.call(cpu, target, vanilla as u16, (vanilla >> 16) as u8, false),
            None => vanilla,
        };
        self.bg = Some(b);
        if std::env::var("SMW_DEBUG").is_ok() {
            eprintln!("bg base: hook={:X?} vanilla={vanilla:06X} -> {b:06X}", Self::hook(cpu, 0x058DA4));
        }
        b
    }
}

/// Tile words for the 32 background columns (2 screens x 16), 27 rows each, in Map16
/// order per block (TL, BL, TR, BR), made by the game's BG column routine CODE_058D7A.
/// It takes the column in $1928 and leaves the left 8x8 column at $1CE8 and the right one
/// at $1D68 (4 bytes per block row).
/// The BG2 screen map the game uploaded to VRAM during the level load, addressed in level
/// coordinates (8x8 tiles). It only covers 64x32 tiles around the start camera; outside
/// that the hardware wraps, which is also how the game would show it.
struct Bg2Map {
    vram: Vec<u8>,
    written: Vec<bool>,
    base: usize,
    wide: bool,
    /// Layer 2 position minus layer 1 position at the start, in tiles
    dx: i32,
    dy: i32,
    /// Rows of the map that were uploaded (around the start camera)
    rows: std::ops::Range<u32>,
}

impl Bg2Map {
    fn new(cpu: &mut Cpu) -> Bg2Map {
        let sc = cpu.mem.regs[0x108] as usize;
        let l1 = (cpu.mem.game_u16(0x1A) as i32, cpu.mem.game_u16(0x1C) as i32);
        let l2 = (cpu.mem.game_u16(0x1E) as i32, cpu.mem.game_u16(0x20) as i32);
        let top = (l1.1 / 8).max(0) as u32;
        Bg2Map {
            vram: cpu.mem.vram.clone(),
            written: cpu.mem.vram_written.clone(),
            base: (sc & 0xFC) << 9,
            wide: sc & 1 != 0,
            dx: (l2.0 - l1.0) / 8,
            dy: (l2.1 - l1.1) / 8,
            rows: top..top + 28,
        }
    }

    /// VRAM byte offset of level tile (tx, ty).
    fn offset(&self, tx: u32, ty: u32) -> usize {
        let x = (tx as i32 + self.dx).rem_euclid(if self.wide { 64 } else { 32 }) as usize;
        let y = (ty as i32 + self.dy).rem_euclid(32) as usize;
        (self.base + (x / 32) * 0x800 + (y * 32 + x % 32) * 2) & 0xFFFF
    }

    /// Tile word at level tile (tx, ty).
    fn at(&self, tx: u32, ty: u32) -> u16 {
        let o = self.offset(tx, ty);
        u16::from_le_bytes([self.vram[o], self.vram[(o + 1) & 0xFFFF]])
    }

    /// Whether the load actually wrote most of the map (rather than it being leftover data).
    fn filled(&self) -> bool {
        let words = (0..32).flat_map(|y| (0..64).map(move |x| (x, y)));
        let total = 32 * 64;
        let written = words.filter(|&(x, y)| self.written[self.offset(x, y)]).count();
        written * 4 >= total * 3
    }
    /// Whether the column routine's tiles match this map on the rows the game uploaded.
    fn agrees(&self, cols: &[[u16; 27 * 4]]) -> bool {
        let (mut same, mut total) = (0, 0);
        for ty in self.rows.clone().filter(|&y| y < 54) {
            for tx in 0..64u32 {
                // (the original game only uploads part of the map while loading and streams
                // the rest in during play: compare just what was written)
                if !self.written[self.offset(tx, ty)] {
                    continue;
                }
                let c = &cols[(tx / 2) as usize % 32];
                let want = c[(ty / 2) as usize * 4 + (tx % 2 * 2 + ty % 2) as usize];
                total += 1;
                same += (want == self.at(tx, ty)) as u32;
            }
        }
        total == 0 || same * 10 >= total * 9
    }
}

pub fn bg_columns(cpu: &Cpu) -> Option<Vec<[u16; 27 * 4]>> {
    let target = cpu.mem.cart.resolve("CODE_058D7A")?;
    let mut s = cpu.clone();
    let code = [0x22, target as u8, (target >> 8) as u8, (target >> 16) as u8];
    let mut out = Vec::with_capacity(32);
    for col in 0..32u8 {
        s.mem.game_store(0x1928, col);
        for (i, b) in code.iter().enumerate() {
            s.mem.store_u8(0x1F00 + i as u32, *b); // code, not a game variable
        }
        s.emulation = false;
        s.pc = 0x1F00;
        s.pbr = 0;
        // The game calls it from bank $05 code with the data bank set to $05 (LM's hook
        // reads its tables through the data bank)
        s.dbr = (target >> 16) as u8;
        s.d = if s.mem.sa1.is_some() { 0x3000 } else { 0 };
        s.s = 0x1EF;
        let mut n = 0;
        while s.pc != 0x1F04 && !s.ill && n < 200_000 {
            s.dispatch();
            n += 1;
        }
        if s.pc != 0x1F04 {
            return None;
        }
        let mut c = [0u16; 27 * 4];
        for row in 0..27u32 {
            let x = row * 4;
            c[row as usize * 4] = s.mem.game_u16(0x1CE8 + x);
            c[row as usize * 4 + 1] = s.mem.game_u16(0x1CEA + x);
            c[row as usize * 4 + 2] = s.mem.game_u16(0x1D68 + x);
            c[row as usize * 4 + 3] = s.mem.game_u16(0x1D6A + x);
        }
        out.push(c);
    }
    Some(out)
}

fn draw_layer(cpu: &mut Cpu, img: &mut Image, bg: bool) {
    let map16_bank = cpu.mem.cart.resolve("Map16Common").unwrap() & 0xFF0000;
    let map16_bg = cpu.mem.cart.resolve("Map16BGTiles").unwrap();
    let vertical = cpu.mem.game_u8(0x5B) & if bg { 2 } else { 1 } != 0;
    let has_layer2 = {
        let mode = cpu.mem.game_u8(0x1925);
        let renderer_table = cpu.mem.cart.resolve("CODE_058955").unwrap() + 9;
        let renderer = cpu.mem.game_u24(renderer_table + (mode as u32) * 3);
        let l2 = [cpu.mem.cart.resolve("CODE_058B8D"), cpu.mem.cart.resolve("CODE_058C71")];
        l2.contains(&Some(renderer))
    };
    let scr_len = match (vertical, has_layer2) {
        (false, false) => 0x20,
        (true, false) => 0x1C,
        (false, true) => 0x10,
        (true, true) => 0x0E,
    };
    let scr_size = if vertical { 16 * 32 } else { 16 * 27 };
    let (lo, hi) = match (bg, has_layer2) {
        (true, true) => {
            let off = scr_len * scr_size;
            (0x7EC800 + off, 0x7FC800 + off)
        }
        (true, false) => (0x7EB900, 0x7EBD00),
        (false, _) => (0x7EC800, 0x7FC800),
    };
    let len = if has_layer2 { 256 * 27 } else { 512 * 27 };
    let mut lookup = Map16Lookup::default();
    let vram = cpu.mem.vram.clone();
    let cgram = cpu.mem.cgram.clone();
    // Backgrounds (2 screens, repeated): let the game draw each column with its own routine,
    // so whatever Lunar Magic or a patch changed about BG tiles is exactly what we draw
    if bg && !has_layer2 && !vertical {
        let cols = bg_columns(cpu);
        // Lunar Magic custom backgrounds are uploaded to VRAM by LM's own code and the column
        // routine's output doesn't match them. The load already left the real BG2 screen map
        // in VRAM (64x32 tiles around the camera), so when the routine disagrees with it,
        // draw from that map instead.
        let map = Bg2Map::new(cpu);
        // (The original game streams BG columns into VRAM during play, so after loading its
        // map is still empty: only trust the map when the load actually filled it.)
        // (Only Lunar Magic ROMs, which hook the BG lookup at $058DA4: the original game's
        // own column routine is always right, while its half-streamed map isn't.)
        let lm_bg = Map16Lookup::hook(cpu, 0x058DA4).is_some();
        if lm_bg && map.filled() && cols.as_ref().is_none_or(|c| !map.agrees(c)) {
            // Only the 32 rows around the start camera are really in VRAM; above and below them
            // the hardware would wrap, so leave the backdrop rather than draw a repeated copy
            let rows = map.rows.start.saturating_sub(4)..map.rows.start.saturating_sub(4) + 32;
            let max_row = img.height / 8;
            for ty in rows.filter(|&y| y < max_row) {
                for tx in 0..img.width / 8 {
                    let t = map.at(tx, ty);
                    draw_tile(img, &vram, &cgram, (tx * 8) as i32, (ty * 8) as i32, (t & 0x3FF) as u32, ((t >> 10) & 7) as u32, t);
                }
            }
            return;
        }
        if let Some(cols) = cols {
            for bx in (0..img.width).step_by(16) {
                let c = &cols[((bx / 256) % 2 * 16 + (bx % 256) / 16) as usize];
                for row in 0..27u32 {
                    let by = row * 16;
                    if by >= img.height {
                        break;
                    }
                    for (i, (ox, oy)) in [(0, 0), (0, 8), (8, 0), (8, 8)].into_iter().enumerate() {
                        let t = c[row as usize * 4 + i];
                        draw_tile(img, &vram, &cgram, (bx + ox) as i32, (by + oy) as i32, (t & 0x3FF) as u32, ((t >> 10) & 7) as u32, t);
                    }
                }
            }
            return;
        }
    }
    for idx in 0..len {
        let (bx, by) = if vertical {
            let (screen, s) = (idx / 256, idx % 256);
            ((s % 16) * 16 + (screen % 2) * 256, (s / 16) * 16 + (screen / 2) * 256)
        } else {
            let (screen, s) = (idx / (16 * 27), idx % (16 * 27));
            ((s % 16) * 16 + screen * 256, (s / 16) * 16)
        };
        if bx >= img.width || by >= img.height {
            continue;
        }
        let src = if bg && !has_layer2 { idx % (16 * 27 * 2) } else { idx };
        let block = cpu.mem.game_u8(lo + src) as u32 | (cpu.mem.game_u8(hi + src) as u32) << 8;
        let ptr = if bg && !has_layer2 {
            block * 8 + lookup.bg_base(cpu, map16_bg)
        } else {
            lookup.fg(cpu, block, map16_bank)
        };
        for (i, (ox, oy)) in [(0, 0), (0, 8), (8, 0), (8, 8)].into_iter().enumerate() {
            let t = cpu.mem.game_u16(ptr + i as u32 * 2);
            draw_tile(img, &vram, &cgram, (bx + ox) as i32, (by + oy) as i32, (t & 0x3FF) as u32, ((t >> 10) & 7) as u32, t);
        }
    }
}
