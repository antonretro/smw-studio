//! cargo run -p smw-core --example bg_blocks -- <rom> <level hex>
//! List the distinct background block numbers in RAM ($7EB900/$7EBD00) after loading a level.
use smw_core::{emu, render, romfile, Cpu};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let rom = romfile::load(std::fs::read(&a[1]).unwrap()).0;
    let level = u16::from_str_radix(&a[2], 16).unwrap();
    let mut cpu = Cpu::new(emu::CheckedMem::new(rom));
    emu::load_level(&mut cpu, level);
    println!("has_layer2: {}", render::has_layer2(&mut cpu));
    let mut seen = std::collections::BTreeMap::new();
    for i in 0..16 * 27 * 2 {
        let b = cpu.mem.load_u8(0x7EB900 + i) as u16 | (cpu.mem.load_u8(0x7EBD00 + i) as u16) << 8;
        *seen.entry(b).or_insert(0) += 1;
    }
    let hook = cpu.mem.cart.read(0x058A65);
    println!("058A65 byte: {:02X?}  distinct BG blocks: {}", hook, seen.len());
    println!("{:X?}", seen.keys().take(40).collect::<Vec<_>>());
}
