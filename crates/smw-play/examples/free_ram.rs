//! cargo run -p smw-play --example free_ram -- <rom> <level hex>
//! Fill bank $7F with a marker after boot, then play the level, die, go to the map and back
//! in Snes9x (no sound out), and list the longest runs of $7F that were never written.
use std::path::Path;

use smw_core::project::Project;
use smw_play::{Options, Session};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let level = u16::from_str_radix(&a[2], 16).unwrap();
    let p = Project::open(a[1].clone().into()).unwrap();
    let dll = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../vendor/snes9x/snes9x_libretro.dll");
    let mut s = Session::boot(&dll, p.build_play_rom(level).unwrap(), &std::env::temp_dir(), Options::default()).unwrap();
    // (leave $7F8000-$7F8FFF: the game keeps an OAM-clearing routine there)
    for i in (0x10000..0x18000).chain(0x19000..0x20000) {
        s.wram()[i] = 0xA5;
    }
    let mut modes = vec![];
    // Walk right a while, then die (falling / timer), through the map and back into a level
    for f in 0..3000 {
        let w = s.wram();
        if f == 200 {
            w[0x71] = 9; // dying
        }
        let buttons = if f > 400 && f % 30 < 5 { 1 << 8 } else { 0 }; // after dying: tap A on the map
        s.run_frame(buttons);
        let m = s.wram()[0x100];
        if modes.last() != Some(&m) {
            modes.push(m);
        }
    }
    println!("game modes seen: {modes:02X?}");
    let w = s.wram().to_vec();
    let mut runs = vec![];
    let mut start = None;
    for i in 0x10000..=0x20000 {
        let untouched = i < 0x20000 && !(0x18000..0x19000).contains(&i) && w[i] == 0xA5;
        match (untouched, start) {
            (true, None) => start = Some(i),
            (false, Some(st)) => {
                runs.push((i - st, st));
                start = None;
            }
            _ => {}
        }
    }
    runs.sort_by(|a, b| b.cmp(a));
    for (len, st) in runs.iter().take(6) {
        println!("  untouched: $7E{:04X}+ ... ${:06X}, {len} bytes", 0, 0x7E0000 + st);
    }
}
