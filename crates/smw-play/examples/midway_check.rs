//! cargo run -p smw-play --example midway_check -- <rom> <work dir>
//! 1. Midway: set level 105's midway screen, play from the midway point in Snes9x and
//!    check where Mario starts.
//! 2. New secondary entrance into 105, used by a screen exit on 106: save, reopen, then
//!    take that exit in the game and check where Mario starts.
use std::path::{Path, PathBuf};

use smw_core::project::{Project, ScreenExit};
use smw_play::{Options, Session};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let work = PathBuf::from(&a[2]);
    std::fs::create_dir_all(&work).unwrap();
    let copy = work.join("midway.sfc");
    std::fs::copy(&a[1], &copy).unwrap();
    let level = 0x105;

    let mut p = Project::open(copy.clone()).unwrap();
    let main = p.main_entrance(level).unwrap();
    p.set_midway_screen(level, 5).unwrap();
    let n = p.add_secondary_entrance(level).unwrap();
    p.set_exit(0x106, ScreenExit { screen: 0, dest: n, secondary: true, water: false }).unwrap();
    p.save().unwrap();
    let mut q = Project::open(copy.clone()).unwrap();
    println!("midway screen after reopen: {:02X} (want 05)", q.midway_screen(level).unwrap());
    let sec = q.secondary_entrances(level).unwrap();
    println!("secondary entrance {n:03X} into 105 after reopen: {}", if sec.iter().any(|s| s.number == n) { "present" } else { "MISSING" });
    let (px, py) = q.main_entrance_pixel(level).unwrap();

    // 1. Play from the midway point
    let dll = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../vendor/snes9x/snes9x_libretro.dll");
    let opts = Options { midway: true, ..Options::default() };
    let mut s = Session::boot(&dll, q.build_play_rom(level).unwrap(), &std::env::temp_dir(), opts).unwrap();
    let w = s.wram();
    let (mx, my) = (u16::from_le_bytes([w[0x94], w[0x95]]), u16::from_le_bytes([w[0x96], w[0x97]]));
    let want = (5 * 256 + px, py);
    // (some hacks' own patches nudge the midway start a pixel)
    let ok = (mx as i32 - want.0 as i32).abs() <= 1 && my == want.1;
    println!("game, from midway: Mario at ({mx:#06X}, {my:#06X}), expected ({:#06X}, {:#06X}) -> {}", want.0, want.1, if ok { "PASS" } else { "FAIL" });
    drop(s);

    // 2. Through the new secondary entrance: start level 106, then take its screen 0 exit
    //    the way the game does for a pipe (Mario's "teleport" state 6), and see where he lands
    let mut s = Session::boot(&dll, q.build_play_rom(0x106).unwrap(), &std::env::temp_dir(), Options::default()).unwrap();
    {
        let w = s.wram();
        w[0x95] = 0; // screen 0
        w[0x71] = 6;
        w[0x88] = 0;
        w[0x89] = 0;
    }
    let mut left = false;
    for _ in 0..600 {
        s.step(0);
        let mode = s.wram()[0x100];
        if mode != 0x14 {
            left = true;
        } else if left {
            break;
        }
    }
    let w = s.wram();
    let (mx, my) = (u16::from_le_bytes([w[0x94], w[0x95]]), u16::from_le_bytes([w[0x96], w[0x97]]));
    // Which level got loaded: its sprite list pointer ($CE) against each level's
    let ptr = w[0xCE] as u32 | (w[0xCF] as u32) << 8 | (w[0xD0] as u32) << 16;
    let lv = (0..0x200u16).find(|&l| q.sprite_ptr(l) == Some(ptr)).unwrap_or(0xFFFF);
    println!("  left the level: {left}, sprite list ${ptr:06X}, exit flags {:02X} {:02X} {:02X}", w[0x19B8], w[0x19D8], w[0x1B93]);
    // A new secondary entrance copies the main entrance (nearest spot when that is exact)
    let e = q.secondary_entrances(level).unwrap().into_iter().find(|s| s.number == n).unwrap().entrance;
    let (xs, ys) = q.entrance_positions();
    let want = (e.screen as u16 * 256 + xs[e.x as usize], ys[e.y as usize]);
    println!("  main entrance starts at ({:#06X}, {py:#06X}); the new secondary at ({:#06X}, {:#06X})", main.screen as u16 * 256 + px, want.0, want.1);
    println!("secondary {n:03X} from 106: in level {lv:03X}, Mario at ({mx:#06X}, {my:#06X}), expected 105 at ({:#06X}, {:#06X}) -> {}",
        want.0, want.1, if lv == level && (mx, my) == want { "PASS" } else { "FAIL" });
}
