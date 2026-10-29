//! cargo run -p smw-core --example sa1_probe -- <sa1 rom> [instructions]
//! Run an SA-1 ROM from its reset vector and report what the SA-1 does.
use smw_core::{emu, romfile, Cpu};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let rom = romfile::load(std::fs::read(&a[1]).unwrap()).0;
    let n: u64 = a.get(2).map(|s| s.parse().unwrap()).unwrap_or(2_000_000);
    let mut cpu = Cpu::new(emu::CheckedMem::new(rom));
    println!("SA-1 chip: {}", cpu.mem.sa1.is_some());
    println!("reset at ${:02X}:{:04X}", cpu.pbr, cpu.pc);
    let mut seen = std::collections::HashMap::<u32, u64>::new();
    let mut running_at = None;
    for i in 0..n {
        let pc = (cpu.pbr as u32) << 16 | cpu.pc as u32;
        *seen.entry(pc).or_default() += 1;
        cpu.dispatch();
        cpu.mem.process_dma();
        if cpu.ill {
            println!("main CPU stopped (illegal) at ${pc:06X} after {i}");
            break;
        }
        let s = cpu.mem.sa1.as_ref().unwrap();
        if running_at.is_none() && s.cpu.mem.r.running {
            running_at = Some(i);
            println!("SA-1 started at instruction {i}: reset vector ${:04X}, IRQ ${:04X}, NMI ${:04X}", s.cpu.mem.r.crv, s.cpu.mem.r.civ, s.cpu.mem.r.cnv);
        }
    }
    let mode = cpu.mem.wram[0x100];
    let s = cpu.mem.sa1.as_ref().unwrap();
    println!("main CPU now at ${:02X}:{:04X}; game mode ${mode:02X}", cpu.pbr, cpu.pc);
    println!("SA-1 at ${:02X}:{:04X}, running {}, unsupported: {:?}, ROM blocks (MMC) {:02X?}", s.cpu.pbr, s.cpu.pc, s.cpu.mem.r.running, s.cpu.mem.r.unsupported, s.cpu.mem.r.mmc);
    let mut hot: Vec<_> = seen.into_iter().collect();
    hot.sort_by(|a, b| b.1.cmp(&a.1));
    println!("main CPU busiest spots: {}", hot.iter().take(6).map(|(pc, c)| format!("${pc:06X}x{c}")).collect::<Vec<_>>().join(" "));
}
