//! cargo run -p smw-play --example sa1_compare -- <sa1 rom> <out dir>
//! Play an SA-1 hack from power-on in Snes9x until it's in a level, then load that level in
//! SMW Studio's emulator and compare VRAM (per 4 KB) and BW-RAM, and save both screens.
use std::path::{Path, PathBuf};

use smw_core::{emu, romfile, Cpu};
use smw_play::libretro::Core;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let out = PathBuf::from(&a[2]);
    std::fs::create_dir_all(&out).unwrap();
    let bytes = std::fs::read(&a[1]).unwrap();
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
        let mode = core.save_ram()[0x100];
        match entered {
            None if mode == 0x14 => entered = Some(f),
            Some(at) if f > at + 2 => break,
            _ => {}
        }
    }
    let bw = core.save_ram().to_vec();
    let level = u16::from_le_bytes([bw[0x10B], bw[0x10C]]);
    let real = core.vram().to_vec();
    println!("Snes9x: level {level:03X}");
    let rom = romfile::load(bytes).0;
    let mut cpu = Cpu::new(emu::CheckedMem::new(rom));
    emu::load_level(&mut cpu, level);
    let ours = cpu.mem.vram.clone();
    for slot in 0..16 {
        let r = slot * 0x1000..(slot + 1) * 0x1000;
        let diff = r.clone().filter(|&i| real.get(i) != ours.get(i)).count();
        // Is our slot the real one shifted somewhere else in VRAM?
        let ours_slot = &ours[r.clone()];
        let elsewhere = (0..16).find(|&o| o != slot && real[o * 0x1000..(o + 1) * 0x1000] == *ours_slot && ours_slot.iter().any(|&b| b != 0));
        println!("  VRAM ${:04X}: {diff:5} of 4096 differ{}", r.start, elsewhere.map(|o| format!("  (ours = real at ${:04X})", o * 0x1000)).unwrap_or_default());
    }
    let our_bw = &cpu.mem.sa1.as_ref().unwrap().cpu.mem.bwram;
    for (name, r) in [("$40:0000-1FFF", 0..0x2000usize), ("$40:2000-C7FF", 0x2000..0xC800), ("$40:C800-FFFF", 0xC800..0x10000), ("$41:0000-C7FF", 0x10000..0x1C800), ("$41:C800-FFFF", 0x1C800..0x20000)] {
        let d = r.clone().filter(|&i| bw.get(i) != our_bw.get(i)).count();
        println!("  BW-RAM {name}: {d} bytes differ");
    }
    std::fs::write(out.join("real_vram.bin"), &real).unwrap();
    std::fs::write(out.join("our_vram.bin"), &ours).unwrap();
    std::fs::write(out.join("real_bwram.bin"), &bw).unwrap();
    std::fs::write(out.join("our_bwram.bin"), our_bw).unwrap();
}
