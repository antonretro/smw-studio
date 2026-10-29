//! cargo run --release -p smw-core --example ow_probe -- <rom> [submap]
//! Run the game's overworld loader and report what it left: PPU registers, where VRAM was
//! written, and RAM layer buffers.
use smw_core::{emu, Cpu};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let rom = smw_core::romfile::load(std::fs::read(&a[1]).unwrap()).0;
    let mut cpu = Cpu::new(emu::CheckedMem::new(rom));
    let sub = a.get(2).map(|s| s.parse().unwrap()).unwrap_or(0);
    let t = std::time::Instant::now();
    let cy = emu::load_overworld(&mut cpu, sub);
    println!("cycles {cy}, ill {}, at {:02X}:{:04X}, {:?}", cpu.ill, cpu.pbr, cpu.pc, t.elapsed());
    let r = |a: usize| cpu.mem.regs[a - 0x2000];
    println!("BGMODE {:02X} BG1SC {:02X} BG2SC {:02X} BG3SC {:02X} BG12NBA {:02X} BG34NBA {:02X} OBSEL {:02X} TM {:02X}", r(0x2105), r(0x2107), r(0x2108), r(0x2109), r(0x210B), r(0x210C), r(0x2101), r(0x212C));
    let mut runs = Vec::new();
    let mut start = None;
    for i in (0..0x10000).step_by(0x400) {
        let w = cpu.mem.vram_written[i..i + 0x400].iter().any(|&b| b);
        match (w, start) {
            (true, None) => start = Some(i),
            (false, Some(s)) => { runs.push((s, i)); start = None; }
            _ => {}
        }
    }
    if let Some(s) = start { runs.push((s, 0x10000)); }
    println!("VRAM written (byte ranges): {:X?}", runs);
    println!("submap now {:02X}, $13C3 {:02X}", cpu.mem.game_u8(0x1F11), cpu.mem.game_u8(0x13C3));
    let bg2: usize = (0x6000..0x8000).filter(|&i| cpu.mem.vram[i] != 0).count();
    println!("VRAM BG2 tilemap bytes non-zero: {bg2} / 0x2000");
    let mut nz = |base: u32, n: u32| (0..n).filter(|&i| cpu.mem.load_u8(base + i) != 0).count();
    println!("non-zero: $7F4000 {} / 0x2000, $7EC800 {} / 0x800, $7ED000 {}", nz(0x7F4000, 0x2000), nz(0x7EC800, 0x800), nz(0x7ED000, 0x800));
}
