//! cargo run -p smw-core --example level_gfx -- <rom> <level hex> <ExGraphics dir>
//! Which ExGFX files a level uses: load it, then find each file's (decompressed) contents
//! in VRAM.
use smw_core::{emu, romfile, Cpu};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let rom = romfile::load(std::fs::read(&a[1]).unwrap()).0;
    let level = u16::from_str_radix(&a[2], 16).unwrap();
    let mut cpu = Cpu::new(emu::CheckedMem::new(rom));
    emu::load_level(&mut cpu, level);
    let vram = cpu.mem.vram.clone();
    let mut found = vec![];
    for e in std::fs::read_dir(&a[3]).unwrap().flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        let Some(num) = name.strip_prefix("ExGFX").and_then(|n| n.strip_suffix(".bin")).and_then(|n| u16::from_str_radix(n, 16).ok()) else { continue };
        let data = std::fs::read(e.path()).unwrap();
        if data.len() < 0x400 || data.iter().all(|&b| b == 0) {
            continue;
        }
        // Compare the first 1 KB at every 1 KB step of VRAM
        let probe = &data[..0x400];
        for off in (0..vram.len() - 0x400).step_by(0x400) {
            if &vram[off..off + 0x400] == probe {
                found.push((num, off, data.len()));
            }
        }
    }
    found.sort();
    for (n, off, len) in &found {
        println!("ExGFX {n:X} ({len} bytes) at VRAM ${:04X} (word ${:04X})", off, off / 2);
    }
}
