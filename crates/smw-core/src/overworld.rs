//! The overworld: loaded by running the game's own "load overworld" game mode, then drawn
//! from what it leaves in video memory (layer 2: the land, layer 1: paths and level tiles).

use crate::{emu, project::Project, render::Image, Cpu};

/// The maps the player can be on, by `$1F11` value.
pub const SUBMAPS: [&str; 7] = ["Main map", "Yoshi's Island", "Vanilla Dome", "Forest of Illusion", "Valley of Bowser", "Special World", "Star World"];

/// A loaded overworld: the emulated machine right after the game loaded `submap`, with the
/// whole of layer 2 (the land) decompressed into $7F4000 (the game streams it to the screen
/// as the map scrolls, so it isn't all in video memory).
pub fn load(p: &Project, submap: u8) -> Cpu {
    let mut cpu = Cpu::new(emu::CheckedMem::new(p.rom_arc()));
    emu::load_overworld(&mut cpu, submap);
    let decompress = cpu.mem.cart.resolve("DecompressOverworldL2").unwrap();
    emu::call(&mut cpu, decompress);
    cpu
}

/// Draw layer 2 from its RAM copy ($7F4000: two maps of 64x64 tilemap entries, the main map
/// then the sheet all the submaps share).
fn draw_layer2(img: &mut Image, cpu: &mut Cpu, chars: usize, submaps: bool) {
    let base = if submaps { 0x7F6000 } else { 0x7F4000 };
    for i in 0..0x1000u32 {
        let e = cpu.mem.load_u8(base + i * 2) as u16 | (cpu.mem.load_u8(base + 1 + i * 2) as u16) << 8;
        // Four 32x32 screens: top left, top right, bottom left, bottom right
        let (screen, j) = (i / 0x400, i % 0x400);
        let (tx, ty) = ((screen % 2) * 32 + j % 32, (screen / 2) * 32 + j / 32);
        let tile = (e & 0x3FF) as u32 + (chars as u32 * 2) / 32;
        crate::render::draw_tile(img, &cpu.mem.vram, &cpu.mem.cgram, tx as i32 * 8, ty as i32 * 8, tile, ((e >> 10) & 7) as u32, e);
    }
}

/// Draw a 64x64 tilemap at VRAM word `map` (four 32x32 screens) with 4bpp graphics from word
/// `chars`, onto a 512x512 picture. `front`: only tiles with the priority bit (else only without).
fn draw_map(img: &mut Image, cpu: &Cpu, map: usize, chars: usize, front: Option<bool>) {
    let vram = &cpu.mem.vram;
    let cgram = &cpu.mem.cgram;
    for ty in 0..64usize {
        for tx in 0..64usize {
            let screen = (ty / 32) * 2 + tx / 32;
            let w = map + screen * 0x400 + (ty % 32) * 32 + tx % 32;
            let e = u16::from_le_bytes([vram[w * 2 & 0xFFFF], vram[(w * 2 + 1) & 0xFFFF]]);
            if front.is_some_and(|f| (e & 0x2000 != 0) != f) {
                continue;
            }
            let tile = (e & 0x3FF) as u32 + (chars as u32 * 2) / 32;
            let pal = ((e >> 10) & 7) as u32;
            crate::render::draw_tile(img, vram, cgram, tx as i32 * 8, ty as i32 * 8, tile, pal, e);
        }
    }
}

/// A picture of a loaded overworld (512 x 512): backdrop colour, layer 2, then layer 1.
pub fn picture(cpu: &mut Cpu) -> Image {
    let r = |a: usize| cpu.mem.regs[a - 0x2000] as usize;
    let back = u16::from_le_bytes([cpu.mem.cgram[0], cpu.mem.cgram[1]]);
    let ch = |s: u16| (((back >> s) & 0x1F) << 3) as u8;
    let mut img = Image::new(512, 512, [ch(0), ch(5), ch(10), 255]);
    // Tilemaps (in 1K-word steps) and graphics (4K-word steps), as word addresses
    let (bg1, bg2) = ((r(0x2107) & 0xFC) << 8, (r(0x2108) & 0xFC) << 8);
    let (c1, c2) = ((r(0x210B) & 0x0F) << 12, (r(0x210B) >> 4) << 12);
    let _ = bg2;
    let submaps = cpu.mem.game_u8(0x1F11) != 0;
    draw_layer2(&mut img, cpu, c2, submaps);
    draw_map(&mut img, cpu, bg1, c1, None);
    img
}

/// A picture of overworld map `submap` in this ROM (512 x 512), in its own colours. The
/// submaps all share one sheet (hacks can arrange it freely), so each shows the whole sheet.
pub fn submap_picture(p: &Project, submap: u8) -> Image {
    picture(&mut load(p, submap))
}
