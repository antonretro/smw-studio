//! cargo run -p smw-play --example exit_check -- <rom> <work dir>
//! Set / replace / remove screen exits on level 105, save a copy, reopen it, and check the
//! running game's own per-screen exit tables ($19B8 low byte, $19D8 flags) match.
use std::path::{Path, PathBuf};

use smw_core::project::{Project, ScreenExit};
use smw_play::{Options, Session};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let work = PathBuf::from(&a[2]);
    std::fs::create_dir_all(&work).unwrap();
    let copy = work.join("exits.sfc");
    std::fs::copy(&a[1], &copy).unwrap();
    let level = 0x105;

    let mut p = Project::open(copy.clone()).unwrap();
    let before = p.exits(level).unwrap();
    println!("before: {:?}", before.iter().map(|x| format!("scr {:02X} -> {}{:03X}", x.screen, if x.secondary { "sec " } else { "" }, x.dest)).collect::<Vec<_>>());
    p.set_exit(level, ScreenExit { screen: 2, dest: 0x1C8, secondary: false, water: false }).unwrap();
    p.set_exit(level, ScreenExit { screen: 5, dest: 0x1C9, secondary: true, water: true }).unwrap();
    if let Some(first) = before.first() {
        p.remove_exit(level, first.screen).unwrap();
    }
    let want = p.exits(level).unwrap();
    p.save().unwrap();
    let mut q = Project::open(copy.clone()).unwrap();
    let back = q.exits(level).unwrap();
    println!("after save + reopen: {}", if back == want { "MATCHES" } else { "DIFFERS" });

    let dll = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../vendor/snes9x/snes9x_libretro.dll");
    let mut s = Session::boot(&dll, q.build_play_rom(level).unwrap(), &std::env::temp_dir(), Options::default()).unwrap();
    for _ in 0..10 {
        s.step(0);
    }
    let per_screen = q.per_screen_exit_flags();
    println!("per-screen exit flags (Lunar Magic): {per_screen}");
    let w = s.wram();
    for x in &want {
        let (lo, hi) = (w[0x19B8 + x.screen as usize], w[0x19D8 + x.screen as usize]);
        // The high bit counts only when Lunar Magic's "own flags" bit is set; otherwise
        // it's the half the level was entered in
        let high = if per_screen && hi & 4 != 0 { hi as u16 & 1 } else { level >> 8 };
        let dest = high << 8 | lo as u16;
        // Original game: one level-wide secondary flag at $1B93
        let sec = if per_screen { hi & 2 != 0 } else { w[0x1B93] != 0 };
        let ok = dest == x.dest && sec == x.secondary;
        println!("screen {:02X}: game has {}{dest:03X} (flags {hi:02X}) want {}{:03X}  {}", x.screen, if sec { "sec " } else { "" }, if x.secondary { "sec " } else { "" }, x.dest, if ok { "PASS" } else { "FAIL" });
    }
    if let Some(first) = before.first() {
        if !want.iter().any(|x| x.screen == first.screen) {
            let (lo, hi) = (w[0x19B8 + first.screen as usize], w[0x19D8 + first.screen as usize]);
            println!("removed screen {:02X}: game has lo {lo:02X} flags {hi:02X} (was {:03X})", first.screen, first.dest);
        }
    }
}
