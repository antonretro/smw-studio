//! cargo run --release -p smw-core --example render_speed -- <rom> <level hex>
//! How long each part of redrawing a level after an edit takes.
use smw_core::{emu, project::Project, render, Cpu};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let level = u16::from_str_radix(&a[2], 16).unwrap();
    let mut p = Project::open(a[1].clone().into()).unwrap();
    p.render(level, true).unwrap(); // warm up
    let t = std::time::Instant::now();
    let r = p.render(level, true).unwrap();
    println!("whole render: {:?} ({} sprites, {}x{})", t.elapsed(), r.sprites, r.image.width, r.image.height);
    let t = std::time::Instant::now();
    p.render(level, false).unwrap();
    println!("without sprites: {:?}", t.elapsed());

    let rom = smw_core::romfile::load(p.build_rom().unwrap()).0;
    let mut cpu = Cpu::new(emu::CheckedMem::new(rom));
    let t = std::time::Instant::now();
    emu::load_level(&mut cpu, level);
    println!("  load level (game code): {:?}", t.elapsed());
    let t = std::time::Instant::now();
    let _ = render::render_level_layers(&mut cpu);
    println!("  draw layers: {:?}", t.elapsed());
    let list = smw_core::sprites::parse_sprites(&mut cpu);
    let t = std::time::Instant::now();
    for s in &list {
        smw_core::sprites::draw_sprite(&cpu, s);
    }
    println!("  {} sprites: {:?}", list.len(), t.elapsed());
    let t = std::time::Instant::now();
    for _ in 0..10 {
        let _ = cpu.clone();
    }
    println!("  one emulator copy: {:?}", t.elapsed() / 10);
}
