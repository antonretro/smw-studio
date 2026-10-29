//! cargo run -p smw-core --example ppu_regs -- <rom> <level hex>
//! After loading a level, print the PPU registers that say where BG tiles and maps live.
use smw_core::{emu, romfile, Cpu};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let rom = romfile::load(std::fs::read(&a[1]).unwrap()).0;
    let mut cpu = Cpu::new(emu::CheckedMem::new(rom));
    emu::load_level(&mut cpu, u16::from_str_radix(&a[2], 16).unwrap());
    let r = |x: usize| cpu.mem.regs[x];
    println!("BGMODE(2105)={:02X} BG1SC(2107)={:02X} BG2SC(2108)={:02X} BG3SC(2109)={:02X} BG12NBA(210B)={:02X} BG34NBA(210C)={:02X} OBSEL(2101)={:02X}",
        r(0x105), r(0x107), r(0x108), r(0x109), r(0x10B), r(0x10C), r(0x101));
    // Mirror copies the game keeps in RAM for the NMI to write
    println!("game mode {:02X}", cpu.mem.load_u8(0x100));
}
