//! cargo run --release -p smw-play --example ground_pound_check -- <rom>...
//! In level 105: jump, press down in the air, and report Mario's pose ($13E0), Y speed ($7D)
//! and the patch's timer ($0F5E) each few frames (no sound).
use std::path::Path;

use smw_core::project::Project;
use smw_play::{Options, Session};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let dll = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../vendor/snes9x/snes9x_libretro.dll");
    for rom in &a[1..] {
        let p = Project::open(rom.into()).unwrap();
        let mut s = Session::boot(&dll, p.build_play_rom(0x105).unwrap(), &std::env::temp_dir(), Options { invincible: true, ..Options::default() }).unwrap();
        for _ in 0..40 {
            s.step(0);
        }
        let mut log = Vec::new();
        let mut pounded = false;
        for f in 0..90u32 {
            // B (jump) for 12 frames, then Down from frame 16 for a few frames
            let b = if f < 12 { 1 } else if (16..20).contains(&f) { 1 << 5 } else { 0 };
            s.step(b);
            let (pose, yspd, timer) = (s.peek(0x13E0), s.peek(0x7D), s.peek(0x0F5E));
            if pose == 0x1C || yspd == 0x50 {
                pounded = true;
            }
            if f % 6 == 0 {
                log.push(format!("{f}:pose {pose:02X} y {yspd:02X} t {timer:02X}"));
            }
        }
        println!("{}: ground pound seen: {pounded}\n  {}", Path::new(rom).file_name().unwrap().to_string_lossy(), log.join("  "));
    }
}
