//! cargo run -p smw-play --example vram_compare -- <rom> <level hex>
//! Compare SMW Studio's emulated level load (VRAM tile area) against Snes9x running the
//! real game, per 4 KB graphics slot, to see which graphics our loader gets wrong.
use std::path::Path;

use smw_core::{emu, project::Project, romfile, Cpu};
use smw_play::{Options, Session};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let level = u16::from_str_radix(&a[2], 16).unwrap();
    let p = Project::open(a[1].clone().into()).unwrap();
    let dll = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../vendor/snes9x/snes9x_libretro.dll");
    let mut s = Session::boot(&dll, p.build_play_rom(level).unwrap(), &std::env::temp_dir(), Options::default()).unwrap();
    let frames: u32 = a.get(3).map(|s| s.parse().unwrap()).unwrap_or(30);
    for _ in 0..frames {
        s.step(0);
    }
    let real = s.vram().to_vec();
    let real_wram = s.wram().to_vec();
    drop(s);

    let mut cpu = Cpu::new(emu::CheckedMem::new(romfile::load(p.build_rom().unwrap()).0));
    if std::env::var("SEED_7FC0").is_ok() {
        // Diagnosis: start from the game's values in LM's work area
        cpu.mem.wram[0x1C000..0x1C100].copy_from_slice(&real_wram[0x1C000..0x1C100]);
    }
    emu::load_level(&mut cpu, level);
    let ours = cpu.mem.vram.clone();
    println!("VRAM sizes: snes9x {} ours {}", real.len(), ours.len());
    for slot in 0..16 {
        let r = slot * 0x1000..(slot + 1) * 0x1000;
        let diff = r.clone().filter(|&i| real.get(i) != ours.get(i)).count();
        println!("  VRAM ${:04X} (word ${:04X}): {:5} of 4096 bytes differ{}", r.start, r.start / 2, diff, if diff == 0 { "" } else { "  <--" });
    }
    // Background screen map (BG2SC base, 64 tiles wide): column 0, first 24 rows
    let base = 0x7000;
    let col = |v: &[u8]| (0..24).map(|r| format!("{:04X}", u16::from_le_bytes([v[base + r * 64], v[base + r * 64 + 1]]))).collect::<Vec<_>>().join(" ");
    println!("BG2 map col 0 game: {}", col(&real));
    println!("BG2 map col 0 ours: {}", col(&ours));
    if let Some(cols) = smw_core::render::bg_columns(&cpu) {
        println!("BG routine col 0  : {}", (0..24).map(|r| format!("{:04X}", cols[0][(r / 2) * 4 + r % 2])).collect::<Vec<_>>().join(" "));
    }
    println!("camera L1=({},{}) L2=({},{}) game", u16::from_le_bytes([real_wram[0x1A], real_wram[0x1B]]), u16::from_le_bytes([real_wram[0x1C], real_wram[0x1D]]), u16::from_le_bytes([real_wram[0x1E], real_wram[0x1F]]), u16::from_le_bytes([real_wram[0x20], real_wram[0x21]]));
    let mut cmp = |name: &str, start: usize, len: usize| {
        let d: Vec<usize> = (start..start + len).filter(|&i| real_wram[i] != cpu.mem.load_u8(0x7E0000 + i as u32)).collect();
        println!("{name:34} ${:06X}+{len:#06X}: {:5} bytes differ{}", 0x7E0000 + start, d.len(), match d.first() {
            Some(&f) => format!("  (first at ${:06X}: game {:02X} ours {:02X})", 0x7E0000 + f, real_wram[f], cpu.mem.load_u8(0x7E0000 + f as u32)),
            None => String::new(),
        });
    };
    cmp("LM level number $010B", 0x010B, 2);
    cmp("LM BG flags $7FC00B", 0x1C00B, 1);
    cmp("Map16 pointer table", 0x0FBE, 0x400);
    cmp("Palette buffer (all 256 colours)", 0x0703, 0x200);
    cmp("Background map (lo bytes)", 0xB900, 0x400);
    cmp("Background map (hi bytes)", 0xBD00, 0x400);
    cmp("Level blocks (lo)", 0xC800, 0x3800);
    cmp("Level blocks (hi)", 0x1C800, 0x3800);
}
