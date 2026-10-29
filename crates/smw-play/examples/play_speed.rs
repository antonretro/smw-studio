//! cargo run --release -p smw-play --example play_speed -- <rom> <level hex>
//! How long one frame of in-editor play takes (no sound, nothing shown).
use std::path::Path;

use smw_core::project::Project;
use smw_play::{Options, Session};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let p = Project::open(a[1].clone().into()).unwrap();
    let rom = p.build_play_rom(u16::from_str_radix(&a[2], 16).unwrap()).unwrap();
    let dll = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../vendor/snes9x/snes9x_libretro.dll");
    let mut s = Session::boot(&dll, rom, &std::env::temp_dir(), Options { infinite_lives: true, ..Default::default() }).unwrap();
    let t = std::time::Instant::now();
    for _ in 0..600 {
        s.step(0);
        s.take_audio();
    }
    println!("{:.2} ms per frame (16.7 ms is real time)", t.elapsed().as_secs_f64() * 1000.0 / 600.0);
}
