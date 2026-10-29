//! cargo run -p smw-play --example screens_check -- <rom> <level hex> <screens>
//! Change a level's length, then check the editor render and the running game agree.
use std::path::Path;

use smw_core::project::Project;
use smw_play::{Options, Session};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let level = u16::from_str_radix(&a[2], 16).unwrap();
    let want: u8 = a[3].parse().unwrap();
    let mut p = Project::open(a[1].clone().into()).unwrap();
    let before = p.render(level, false).unwrap().info.screens;
    let n = p.set_screens(level, want).unwrap();
    let r = p.render(level, false).unwrap();
    println!("editor: {before} -> {} screens ({}x{} px), asked {want}, got {n}", r.info.screens, r.image.width, r.image.height);
    let dll = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../vendor/snes9x/snes9x_libretro.dll");
    let mut s = Session::boot(&dll, p.build_play_rom(level).unwrap(), &std::env::temp_dir(), Options::default()).unwrap();
    for _ in 0..30 {
        s.step(0);
    }
    let game = s.wram()[0x5D];
    println!("game: $5D = {game} screens -> {}", if game == n { "PASS" } else { "FAIL" });
}
