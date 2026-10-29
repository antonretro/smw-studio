//! cargo run -p smw-play --example header_check -- <rom>
//! Change level 105's settings (tileset, enemy graphics, back colour, music, timer) through
//! Project::set_header_bits, boot it in Snes9x and read what the running game uses.
use std::path::Path;

use smw_core::project::Project;
use smw_play::{Options, Session};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let mut p = Project::open(a[1].clone().into()).unwrap();
    let level = 0x105;
    // tileset 3 (Underground 1; a castle tileset would play the castle entrance scene first),
    // sprite set 4 (Water), back colour 3, music choice 4, timer 400
    p.set_header_bits(level, 4, 0x0F, 0x03).unwrap();
    p.set_header_bits(level, 2, 0x0F, 0x04).unwrap();
    p.set_header_bits(level, 1, 0xE0, 3 << 5).unwrap();
    p.set_header_bits(level, 2, 0x70, 4 << 4).unwrap();
    p.set_header_bits(level, 3, 0xC0, 3 << 6).unwrap();
    let (music, _) = p.header_tables();
    println!("header after edits: {:02X?}", p.header(level).unwrap());
    let rom = p.build_play_rom(level).unwrap();
    let ptr = |a: usize| -> usize {
        let lo = a % 0x8000 + (a >> 16) * 0x8000;
        lo
    };
    let l1 = 0x05E000 + level as usize * 3;
    let o = ptr(l1);
    let snes = rom[o] as usize | (rom[o + 1] as usize) << 8 | (rom[o + 2] as usize) << 16;
    let so = ptr(snes);
    println!("play ROM level {level:03X} data at ${snes:06X}, header {:02X?}", &rom[so..so + 5]);
    let dll = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../vendor/snes9x/snes9x_libretro.dll");
    let mut s = Session::boot(&dll, rom.clone(), &std::env::temp_dir(), Options::default()).unwrap();
    for _ in 0..60 {
        s.step(0);
    }
    let w = s.wram();
    println!("game: translevel $13BF={:02X} L1 data ptr $65={:06X}", w[0x13BF], w[0x65] as u32 | (w[0x66] as u32) << 8 | (w[0x67] as u32) << 16);
    let checks = [
        ("tileset $1931", w[0x1931] as u32, 3),
        ("enemy graphics $192B", w[0x192B] as u32, 4),
        ("back area colour $192F", w[0x192F] as u32, 3),
        ("music $0DDA", w[0x0DDA] as u32 & 0x7F, music[4] as u32),
        // 400 has already ticked down to 399 one second in (a 300 start would read 2)
        ("timer hundreds $0F31", w[0x0F31] as u32, if w[0x0F32] == 9 { 3 } else { 4 }),
    ];
    for (name, got, want) in checks {
        println!("{name:26} got {got:#04X} want {want:#04X}  {}", if got == want { "PASS" } else { "FAIL" });
    }
    // The editor's render must follow too (tileset changes the level's look)
    let r = p.render(level, false).unwrap();
    println!("editor render: {}x{} with mode {:02X}", r.image.width, r.image.height, r.info.mode);
}
