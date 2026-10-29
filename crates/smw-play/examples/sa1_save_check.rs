//! cargo run -p smw-play --example sa1_save_check -- <sa1 rom> <work dir>
//! Edit an SA-1 hack's intro level (0C5) so it has to move, add a sprite, save, reopen, then
//! play the copy in Snes9x into that level and compare its level blocks with SMW Studio's.
use std::path::{Path, PathBuf};

use smw_core::{emu, level_data::Kind, project::Project, romfile, Cpu};
use smw_play::libretro::Core;

const LEVEL: u16 = 0xC5;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let work = PathBuf::from(&a[2]);
    std::fs::create_dir_all(&work).unwrap();
    let copy = work.join("edited.sfc");
    std::fs::copy(&a[1], &copy).unwrap();

    let mut p = Project::open(copy.clone()).unwrap();
    let before_ptr = p.layer1_ptr(LEVEL).unwrap();
    let n_before = p.level_data(LEVEL).unwrap().entries.len();
    // 40 turn blocks (extended object 0x?? is fine: use standard object 0x01? -> use a copy of
    // the level's first standard object instead, along the top row)
    // Copies of a small, harmless object: an ordinary coin (standard object 05, size 1x1),
    // along an empty top row
    let entries = p.level_data(LEVEL).unwrap().entries.clone();
    println!("objects in {LEVEL:03X}: {:?}", entries.iter().map(|e| format!("{:?} {:02X}/{:02X} @{},{}", e.kind, e.id, e.settings, e.x, e.y)).collect::<Vec<_>>());
    let items: Vec<_> = (0..30u32).map(|i| (Kind::Standard, 0x05, 0x00, 1 + i, 3)).collect();
    p.add_entries(LEVEL, &items).unwrap();
    p.add_sprites(LEVEL, &[(0x0F, 10, 10, 0)]).unwrap();
    let report = p.save().unwrap();
    for l in &report {
        println!("  {l}");
    }
    let mut q = Project::open(copy.clone()).unwrap();
    let after_ptr = q.layer1_ptr(LEVEL).unwrap();
    let n_after = q.level_data(LEVEL).unwrap().entries.len();
    println!("level {LEVEL:03X}: {n_before} -> {n_after} objects, data ${before_ptr:06X} -> ${after_ptr:06X}, sprites {}", q.sprites(LEVEL).unwrap().sprites.len());

    // The real game, from power-on into its first level (the intro, 0C5)
    let bytes = std::fs::read(&copy).unwrap();
    let dll = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../vendor/snes9x/snes9x_libretro.dll");
    let mut core = Core::new(&dll, bytes.clone(), &std::env::temp_dir()).unwrap();
    let mut entered = None;
    for f in 0..12000u32 {
        let buttons = match f % 60 {
            0..=3 => 1 << 3,
            30..=33 => 1 << 8,
            _ => 0,
        };
        core.run_frame(if entered.is_some() { 0 } else { buttons });
        match entered {
            None if core.save_ram()[0x100] == 0x14 => entered = Some(f),
            Some(at) if f > at + 2 => break,
            _ => {}
        }
    }
    let bw = core.save_ram().to_vec();
    let level = u16::from_le_bytes([bw[0x10B], bw[0x10C]]);
    let rom = romfile::load(bytes).0;
    let mut cpu = Cpu::new(emu::CheckedMem::new(rom));
    emu::load_level(&mut cpu, level);
    let ours = &cpu.mem.sa1.as_ref().unwrap().cpu.mem.bwram;
    let d1 = (0xC800..0x10000).filter(|&i| bw[i] != ours[i]).count();
    let d2 = (0x1C800..0x20000).filter(|&i| bw[i] != ours[i]).count();
    // Count blocks that are the copied object's tile in the real game's table vs the original
    println!("Snes9x is in level {level:03X}; its level blocks vs SMW Studio's: {d1} + {d2} bytes differ -> {}", if level == LEVEL && d1 + d2 == 0 { "PASS" } else { "CHECK" });
}
