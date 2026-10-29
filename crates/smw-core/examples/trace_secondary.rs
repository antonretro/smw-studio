//! cargo run -p smw-core --example trace_secondary -- <rom> <secondary entrance hex>
//! Load "level" N with the secondary-exit flag set, so the loader takes its secondary
//! entrance path (use SMW_TRACE to print it). Prints where Mario starts.
use smw_core::{emu, romfile, Cpu};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let rom = romfile::load(std::fs::read(&a[1]).unwrap()).0;
    let n = u16::from_str_radix(&a[2], 16).unwrap();
    let mut cpu = Cpu::new(emu::CheckedMem::new(rom));
    cpu.mem.store_u8(0x1B93, 1);
    emu::load_level(&mut cpu, n);
    let m = &mut cpu.mem;
    println!("Mario X ${:04X} Y ${:04X}, entrance type $192A={:02X}, layer1 cam Y ${:04X}, $5B={:02X}, $0E={:04X}, level data ptr ${:06X}",
        m.load_u16(0x94), m.load_u16(0x96), m.load_u8(0x192A), m.load_u16(0x1C), m.load_u8(0x5B), m.load_u16(0x0E), m.load_u24(0x65));
}
