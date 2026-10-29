//! cargo run -p smw-play --example entrance_check -- <rom> <work dir>
//! Move level 105's main entrance, check undo, save + reopen, then check where Mario starts
//! in the running game (and with which entry style).
use std::path::{Path, PathBuf};

use smw_core::project::{Entrance, Project};
use smw_play::{Options, Session};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let work = PathBuf::from(&a[2]);
    std::fs::create_dir_all(&work).unwrap();
    let copy = work.join("entrance.sfc");
    std::fs::copy(&a[1], &copy).unwrap();
    let level = 0x105;

    let mut p = Project::open(copy.clone()).unwrap();
    let orig = p.main_entrance(level).unwrap();
    println!("original main entrance: {orig:?}");
    // Entry style from the command line (pipe styles nudge Mario a few pixels to line up)
    let action: u8 = a.get(3).map(|s| s.parse().unwrap()).unwrap_or(0);
    let want = Entrance { screen: 3, x: 1, y: 6, action };
    p.set_main_entrance(level, want).unwrap();
    p.undo(level).unwrap();
    println!("undo restores: {}", if p.main_entrance(level).unwrap() == orig { "PASS" } else { "FAIL" });
    p.redo(level).unwrap();
    p.save().unwrap();
    let q = Project::open(copy.clone());
    let mut q = q.unwrap();
    println!("after save + reopen: {}", if q.main_entrance(level).unwrap() == want { "PASS" } else { "FAIL" });
    println!("secondary entrances into 105: {:?}", q.secondary_entrances(level).unwrap().iter().map(|s| format!("{:03X}", s.number)).collect::<Vec<_>>());

    // (on levels in Lunar Magic's exact format, x/y are 16-pixel tiles)
    println!("exact-tile entrance: {}", q.main_entrance_exact(level).unwrap());
    let (px, py) = q.main_entrance_pixel(level).unwrap();
    let (expect_x, expect_y) = (want.screen as u16 * 256 + px, py);
    let dll = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../vendor/snes9x/snes9x_libretro.dll");
    let mut s = Session::boot(&dll, q.build_play_rom(level).unwrap(), &std::env::temp_dir(), Options::default()).unwrap();
    let w = s.wram();
    let (mx, my) = (u16::from_le_bytes([w[0x94], w[0x95]]), u16::from_le_bytes([w[0x96], w[0x97]]));
    println!("game: Mario at ({mx:#06X}, {my:#06X}), expected ({expect_x:#06X}, {expect_y:#06X}) -> {}", if (mx as i32 - expect_x as i32).abs() <= 1 && my == expect_y { "PASS" } else { "FAIL" });
    println!("game: entry style $192A = {} (want {}) -> {}", w[0x192A], want.action, if w[0x192A] == want.action { "PASS" } else { "FAIL" });
}
