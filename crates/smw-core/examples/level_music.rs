//! cargo run --release -p smw-core --example level_music -- <rom> <level hex>...
//! The song each level asks for while loading ($1DFB), as the game decides it.
use smw_core::{emu, Cpu};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let rom = smw_core::romfile::load(std::fs::read(&a[1]).unwrap()).0;
    for l in &a[2..] {
        let level = u16::from_str_radix(l, 16).unwrap();
        let mut cpu = Cpu::new(emu::CheckedMem::new(rom.clone()));
        let t = std::time::Instant::now();
        emu::load_level(&mut cpu, level);
        println!("{level:03X}: $1DFB={:02X} $0DDA={:02X} ({:?})", cpu.mem.game_u8(0x1DFB), cpu.mem.game_u8(0x0DDA), t.elapsed());
    }
}
