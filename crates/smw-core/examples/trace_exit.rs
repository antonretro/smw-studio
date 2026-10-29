//! cargo run -p smw-core --example trace_exit -- <rom> <current level hex> <exit dest hex> <flags hex>
//! Load a level the way a screen exit does: Mario on screen 0 of the current level, whose
//! exit table entry for screen 0 is (dest low byte, flags). Use SMW_TRACE / SMW_BREAK=05D8B7.
use smw_core::{emu, romfile, Cpu};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let rom = romfile::load(std::fs::read(&a[1]).unwrap()).0;
    let cur = u16::from_str_radix(&a[2], 16).unwrap();
    let dest = u16::from_str_radix(&a[3], 16).unwrap();
    let flags = u8::from_str_radix(&a[4], 16).unwrap();
    let mut cpu = Cpu::new(emu::CheckedMem::new(rom));
    cpu.mem.store_u8(0x13BF, if cur >= 0x100 { cur - 0xDC } else { cur } as u8);
    cpu.mem.store_u8(0x95, 0);
    cpu.mem.store_u8(0x19B8, dest as u8);
    cpu.mem.store_u8(0x19D8, flags);
    cpu.mem.store_u8(0x1B93, (flags >> 1) & 1);
    emu::load_level(&mut cpu, cur);
    let m = &mut cpu.mem;
    println!("Mario X ${:04X} Y ${:04X}, sprite list ${:06X}", m.load_u16(0x94), m.load_u16(0x96), m.load_u24(0xCE));
}
