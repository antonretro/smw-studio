//! cargo run -p smw-core --example gm12_calls -- <rom>
//! List the subroutines GM12PrepLevel calls directly (JSR/JSL from its own code), in order.
use smw_core::{emu, romfile, Cpu};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let rom = romfile::load(std::fs::read(&a[1]).unwrap()).0;
    let mut cpu = Cpu::new(emu::CheckedMem::new(rom));
    let name = a.get(2).cloned().unwrap_or("GM12PrepLevel".into());
    let gm12 = cpu.mem.cart.resolve(&name).unwrap();
    // Code: JSR GM12 ; STP-like end marker at $2003
    cpu.mem.store_u8(0x2000, 0x20);
    cpu.mem.store_u16(0x2001, gm12 as u16);
    cpu.mem.wram[0x18000] = 0x6B; // boot-built OAM clear routine: RTL
    cpu.emulation = false;
    cpu.mem.store_u8(0x0100, if name.starts_with("GM11") { 0x11 } else { 0x12 });
    cpu.s = 0x1FF;
    cpu.pc = 0x2000;
    cpu.pbr = 0;
    cpu.dbr = 0;
    let base_s = cpu.s;
    let mut n = 0u64;
    while cpu.pc != 0x2003 && !cpu.ill && n < 30_000_000 {
        // Direct calls made from GM12's own frame (stack two bytes below the start)
        let op = cpu.mem.load_u8((cpu.pbr as u32) << 16 | cpu.pc as u32);
        if cpu.s == base_s - 2 && (op == 0x20 || op == 0x22) {
            let at = (cpu.pbr as u32) << 16 | cpu.pc as u32;
            let lo = cpu.mem.load_u16(at + 1) as u32;
            let target = if op == 0x22 { lo | (cpu.mem.load_u8(at + 3) as u32) << 16 } else { (cpu.pbr as u32) << 16 | lo };
            let name = cpu.mem.cart.symbol_at(target).unwrap_or_default();
            println!("{at:06X}: {} ${target:06X} {name}", if op == 0x22 { "JSL" } else { "JSR" });
        }
        n += cpu.dispatch() as u64;
        cpu.mem.process_dma();
    }
    println!("done after {n} cycles, ill={}", cpu.ill);
}
