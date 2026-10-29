//! cargo run -p smw-core --example render_level -- <rom> <level hex> <out.png>
use smw_core::{emu, render, romfile, Cpu};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let (rom, info) = romfile::load(std::fs::read(&args[1]).expect("read rom"));
    println!("ROM: {:?} title={:?} sha1={}", info.kind, info.title, info.sha1);
    let level = u16::from_str_radix(&args[2], 16).expect("level hex");

    let t = std::time::Instant::now();
    let mut cpu = Cpu::new(emu::CheckedMem::new(rom));
    let cycles = emu::load_level(&mut cpu, level);
    let li = render::level_info(&mut cpu);
    let mut img = render::render_level(&mut cpu);
    let ts = std::time::Instant::now();
    let sprites = render::draw_sprites(&mut cpu, &mut img);
    println!("{} sprites in {:?}: {:?}", sprites.len(), ts.elapsed(),
        sprites.iter().map(|s| format!("{:02X}@{},{}", s.id, s.x / 16, s.y / 16)).collect::<Vec<_>>());
    println!(
        "level {level:03X}: mode={:02X} vertical={} screens={} -> {}x{} in {:?} ({cycles} cycles)",
        li.mode, li.vertical, li.screens, img.width, img.height, t.elapsed()
    );

    let file = std::fs::File::create(&args[3]).unwrap();
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), img.width, img.height);
    enc.set_color(png::ColorType::Rgba);
    enc.write_header().unwrap().write_image_data(&img.rgba).unwrap();
}
