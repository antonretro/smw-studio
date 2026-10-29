//! cargo run -p smw-core --example testplay_expect -- <rom> <out.sfc> [--edit] <level hex>...
//! Write the ROM exactly as Test Play builds it, and print, per level, a hash of the level's
//! block map ($7EC800-$7EFFFF lo, $7FC800-$7FFFFF hi) after our loader runs. A Lua checker
//! hashes the same RAM inside Mesen so the two can be compared.
//! --edit places a 2x2 group of ? blocks at (20,14) in each listed level first.
use smw_core::{emu, level_data::Kind, project::Project, romfile, Cpu};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let mut p = Project::open(a[1].clone().into()).unwrap();
    let edit = a.iter().any(|s| s == "--edit");
    let levels: Vec<u16> = a[3..].iter().filter(|s| !s.starts_with("--")).map(|s| u16::from_str_radix(s, 16).unwrap()).collect();
    if edit {
        for &l in &levels {
            p.add_entry(l, Kind::Standard, 0x0A, 0x11, 20, 14).unwrap();
        }
    }
    let bytes = p.build_rom().unwrap();
    std::fs::write(&a[2], &bytes).unwrap();
    for &l in &levels {
        let rom = romfile::load(bytes.clone()).0;
        let mut cpu = Cpu::new(emu::CheckedMem::new(rom));
        emu::load_level(&mut cpu, l);
        // Only the level's own screens: $5D = screen count, $5B bit 0 = vertical
        let len = if cpu.mem.load_u8(0x5B) & 1 != 0 { 0x3800 } else { (cpu.mem.load_u8(0x5D) as u32 * 432).min(0x3800) };
        let mut h: u64 = 0;
        let mut raw = Vec::with_capacity(0x7000);
        for base in [0x7EC800u32, 0x7FC800] {
            for i in 0..len {
                let b = cpu.mem.load_u8(base + i);
                raw.push(b);
                h = (h * 31 + b as u64) % (1 << 32);
            }
        }
        // Raw expected block map next to the ROM, for byte-by-byte diffs against a game dump
        std::fs::write(format!("{}.{l:03X}.bin", a[2]), &raw).unwrap();
        println!("{l:03X} {h}");
    }
}
