//! cargo run --release -p smw-play --example status_bar_dump -- <rom> <level hex>
//! Play into a level (no sound) and print the status bar's layer 3 tiles (VRAM $5000-$50BF):
//! tile number and palette per cell, plus the RAM buffers the game copies there.
use std::path::Path;

use smw_core::project::Project;
use smw_play::{Options, Session};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let p = Project::open(a[1].clone().into()).unwrap();
    let rom = p.build_play_rom(u16::from_str_radix(&a[2], 16).unwrap()).unwrap();
    let dll = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../vendor/snes9x/snes9x_libretro.dll");
    let mut s = Session::boot(&dll, rom, &std::env::temp_dir(), Options::default()).unwrap();
    for _ in 0..60 {
        s.step(0);
    }
    let v = s.vram().to_vec();
    for row in 0..6 {
        let line: Vec<String> = (0..32).map(|c| {
            let w = 0x5000 + row * 32 + c;
            let (lo, hi) = (v[w * 2], v[w * 2 + 1]);
            format!("{lo:02X}{}", (hi >> 2) & 7)
        }).collect();
        println!("row {row}: {}", line.join(" "));
    }
    let buf: Vec<String> = (0x0EF9..0x0F30).map(|a| format!("{:02X}", s.peek(a))).collect();
    println!("$0EF9..: {}", buf.join(" "));
}
