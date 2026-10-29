//! cargo run -p smw-core --example trace_midway -- <rom> <level hex> [midway 0/1]
//! Load a level as a first entry from the map, with or without its midway point passed
//! (use SMW_TRACE to print the loader). Prints where Mario starts.
use smw_core::{emu, romfile, Cpu};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let rom = romfile::load(std::fs::read(&a[1]).unwrap()).0;
    let level = u16::from_str_radix(&a[2], 16).unwrap();
    let midway = a.get(3).map(|s| s == "1").unwrap_or(true);
    let translevel = if level >= 0x100 { level - 0xDC } else { level } as u8;
    let mut cpu = Cpu::new(emu::CheckedMem::new(rom));
    cpu.mem.store_u8(0x13BF, translevel);
    if midway {
        // The loader works out which map tile it is itself: mark them all "midway passed"
        for t in 0..0x60u32 {
            cpu.mem.store_u8(0x1EA2 + t, 0x40);
        }
    }
    // Run the loader with $141A = 0 (load_level sets it to 1 first; patch it back in the
    // middle of the load via the break hook is overkill: use the env switch instead)
    std::env::set_var("SMW_FIRST_ENTRY", "1");
    emu::load_level(&mut cpu, level);
    let m = &mut cpu.mem;
    println!("level {level:03X} midway={midway}: Mario X ${:04X} Y ${:04X}, $192A={:02X}, cam Y ${:04X}",
        m.load_u16(0x94), m.load_u16(0x96), m.load_u8(0x192A), m.load_u16(0x1C));
}
