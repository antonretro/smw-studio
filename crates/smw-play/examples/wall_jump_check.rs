//! cargo run --release -p smw-play --example wall_jump_check -- <rom>...
//! Level 105: run right to the wall, step back, jump and drift into it; while stuck, press B
//! again. Reports whether
//! Mario stuck to the wall (stick flag $0F5E set, no speed in the air) and jumped off it
//! (Y speed $B0 away from the wall). No sound.
use std::path::Path;

use smw_core::project::Project;
use smw_play::{Options, Session};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let dll = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../vendor/snes9x/snes9x_libretro.dll");
    for rom in &a[1..] {
        let p = Project::open(rom.into()).unwrap();
        let mut s = Session::boot(&dll, p.build_play_rom(0x105).unwrap(), &std::env::temp_dir(), Options { invincible: true, ..Options::default() }).unwrap();
        for _ in 0..30 {
            s.step(0);
        }
        let (mut stuck, mut jumped) = (false, false);
        let mut log = Vec::new();
        let mut stuck_at = None;
        let mut at_wall = None;
        for f in 0..300u32 {
            let right = 1u16 << 7;
            // Once against the wall (blocked on the right), jump (B held 15 frames); once
            // stuck, wait 20 frames and press B again
            if at_wall.is_none() && f > 60 && s.peek(0x7B) < 0x08 {
                at_wall = Some(f);
            }
            // At the wall: step back left, jump, then press right in the air into the wall
            let left = 1u16 << 6;
            let mut b = match at_wall.map(|w| f - w) {
                None => right,
                Some(d) if d < 12 => left,
                Some(d) if d < 16 => 1,
                Some(d) if d < 30 => 1 | right,
                Some(_) => right,
            };
            if let Some(t) = stuck_at {
                if f >= t + 20 && f < t + 24 {
                    b = 1;
                }
            }
            s.step(b);
            let (flag, xs, ys, air) = (s.peek(0x0F5E), s.peek(0x7B), s.peek(0x7D), s.peek(0x72));
            // Stuck: the flag is on in mid-air (the game adds a little gravity after the code)
            if flag != 0 && air != 0 && ys < 0x10 {
                stuck = true;
                stuck_at.get_or_insert(f);
            }
            // Jumped off: shot upwards (the code sets $B0) and pushed away from the wall
            if stuck_at.is_some_and(|t| f > t + 20) && (0xB0..0xC0).contains(&ys) && xs >= 0x80 {
                jumped = true;
            }
            if std::env::var_os("WJ_TRACE").is_some() && at_wall.is_some_and(|w| f >= w + 24 && f < w + 80) {
                println!("  {f}: in {b:04X} $15 {:02X} $16 {:02X} air {air:02X} y {ys:02X} x {xs:02X} blocked {:02X} stick {flag:02X}", s.peek(0x15), s.peek(0x16), s.peek(0x77));
            }
            if f % 10 == 0 {
                log.push(format!("{f}:f{flag:02X} x{xs:02X} y{ys:02X} a{air:02X} b{:02X}", s.peek(0x77)));
            }
        }
        println!("{}: stuck to the wall {stuck}, jumped off {jumped}\n  {}", Path::new(rom).file_name().unwrap().to_string_lossy(), log.join(" "));
    }
}
