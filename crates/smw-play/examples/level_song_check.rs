//! cargo run -p smw-play --example level_song_check -- <rom> <level hex>
//! Start a level in Snes9x without sound output: which song plays and how loud it is.
use std::path::Path;

use smw_core::project::Project;
use smw_play::{Options, Session};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let level = u16::from_str_radix(&a[2], 16).unwrap();
    let p = Project::open(a[1].clone().into()).unwrap();
    let dll = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../vendor/snes9x/snes9x_libretro.dll");
    let mut s = Session::boot(&dll, p.build_play_rom(level).unwrap(), &std::env::temp_dir(), Options { infinite_time: true, invincible: true, ..Options::default() }).unwrap();
    let mut peaks = Vec::new();
    for sec in 0..12 {
        let mut peak = 0u32;
        for _ in 0..60 {
            s.step(0);
            peak = peak.max(s.take_audio().iter().map(|v| v.unsigned_abs() as u32).max().unwrap_or(0));
        }
        peaks.push(format!("{sec}s:{peak}"));
    }
    println!("level {level:03X}: song ${:02X}; loudness per second: {}", s.wram()[0x0DDA], peaks.join(" "));
}
