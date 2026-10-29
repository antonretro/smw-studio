//! cargo run -p smw-core --example sa1_where -- <sa1 rom> <level hex>
//! After loading a level in an SA-1 ROM: where did the level's data end up?
use smw_core::{emu, romfile, Cpu};

fn nz(v: &[u8]) -> usize {
    v.iter().filter(|&&b| b != 0).count()
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let rom = romfile::load(std::fs::read(&a[1]).unwrap()).0;
    let level = u16::from_str_radix(&a[2], 16).unwrap();
    let mut cpu = Cpu::new(emu::CheckedMem::new(rom));
    emu::load_level(&mut cpu, level);
    let w = cpu.mem.wram.clone();
    let s = cpu.mem.sa1.as_ref().unwrap();
    let bw = &s.cpu.mem.bwram;
    println!("WRAM  $7EC800-$7EFFFF nonzero: {}", nz(&w[0xC800..0x10000]));
    println!("WRAM  $7FC800-$7FFFFF nonzero: {}", nz(&w[0x1C800..0x20000]));
    println!("WRAM  $0000-$1FFF nonzero: {}", nz(&w[0..0x2000]));
    for (name, r) in [("BW-RAM $40:0000-1FFF", 0..0x2000), ("BW-RAM $40:2000-C7FF", 0x2000..0xC800), ("BW-RAM $40:C800-FFFF", 0xC800..0x10000), ("BW-RAM $41:0000-C7FF", 0x10000..0x1C800), ("BW-RAM $41:C800-FFFF", 0x1C800..0x20000), ("BW-RAM $42+", 0x20000..0x40000)] {
        println!("{name}: nonzero {}", nz(&bw[r]));
    }
    println!("I-RAM nonzero: {}", nz(&s.cpu.mem.iram));
    println!("SA-1 unsupported: {:?}", s.cpu.mem.r.unsupported);
    println!("$010B (BW): {:02X}{:02X}  $13BF: {:02X}  $0E/$0F (I-RAM $300E): {:02X?}", bw[0x10C], bw[0x10B], bw[0x13BF], &s.cpu.mem.iram[0x0E..0x10]);
    println!("VRAM nonzero per 4 KB: {:?}", cpu.mem.vram.chunks(0x1000).map(nz).collect::<Vec<_>>());
    println!("game mode (BW $40:0100): {:02X}, level mode $1925 (BW): {:02X}, WRAM $1925: {:02X}", bw[0x100], bw[0x1925], w[0x1925]);
    println!("first 32 layer-1 blocks in BW $40:C800: {:02X?}", &bw[0xC800 + 0x1B0..0xC800 + 0x1D0]);
    println!("same in WRAM $7EC800: {:02X?}", &w[0xC800 + 0x1B0..0xC800 + 0x1D0]);
    println!("palette $0703 in BW: {:02X?} / WRAM: {:02X?} / CGRAM: {:02X?}", &bw[0x703..0x70B], &w[0x703..0x70B], &cpu.mem.cgram[0..8]);
}
