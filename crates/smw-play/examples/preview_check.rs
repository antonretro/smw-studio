//! cargo run -p smw-play --example preview_check -- <rom> <pack id>
//! Build the Music tab's preview ROM for a Toolbox music pack and run it without sound:
//! which song the game plays and how loud the sound it makes is.
use std::path::Path;

use smw_core::{music, toolbox};
use smw_play::{Options, Session};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let lib = toolbox::default_dir();
    let pack = toolbox::get(&lib, &a[2]).unwrap();
    let t = std::time::Instant::now();
    let rom = music::preview_rom(Path::new(&a[1]), &lib, &pack, 0x105).unwrap();
    println!("preview ROM built in {:.1} s", t.elapsed().as_secs_f32());
    let dll = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../vendor/snes9x/snes9x_libretro.dll");
    let mut s = Session::boot(&dll, rom, &std::env::temp_dir(), Options { infinite_time: true, invincible: true, ..Options::default() }).unwrap();
    let mut peak = 0u32;
    for _ in 0..600 {
        s.step(0);
        peak = peak.max(s.take_audio().iter().map(|v| v.unsigned_abs() as u32).max().unwrap_or(0));
    }
    println!("song ${:02X}, loudest sample in 10 s: {peak}", s.wram()[0x0DDA]);
}
