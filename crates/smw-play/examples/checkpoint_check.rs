//! cargo run -p smw-play --example checkpoint_check -- <rom> <work dir> [level hex, default 105]
//! Multiple checkpoints, in Snes9x without sound: add the upgrade and a second midway tape
//! to level 105, touch that tape, die, re-enter from the map: Mario should restart there.
use std::path::{Path, PathBuf};

use smw_core::{project::Project, upgrades};
use smw_play::{Options, Session};


fn tapes(s: &mut Session) -> Vec<(u32, u32)> {
    // Layer 1 blocks: $7EC800 (low) / $7FC800 (high), 16x27 per screen (horizontal level)
    let w = s.wram();
    (0..0x3800u32)
        .filter(|&i| w[0xC800 + i as usize] == 0x38 && w[0x1C800 + i as usize] == 0)
        .map(|i| ((i / (16 * 27)) * 16 + i % 16, (i % (16 * 27)) / 16))
        .collect()
}

fn boot(dll: &Path, p: &Project, level: u16) -> Session {
    Session::boot(dll, p.build_play_rom(level).unwrap(), &std::env::temp_dir(), Options::default()).unwrap()
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let level = a.get(3).map(|l| u16::from_str_radix(l, 16).unwrap()).unwrap_or(0x105);
    let work = PathBuf::from(&a[2]);
    std::fs::create_dir_all(&work).unwrap();
    let rom = work.join("checkpoints.sfc");
    std::fs::copy(&a[1], &rom).unwrap();
    let dll = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../vendor/snes9x/snes9x_libretro.dll");

    let mut p = Project::open(rom.clone()).unwrap();
    println!("can add: {:?}", upgrades::can_add_checkpoints(&p));
    upgrades::add_checkpoints(&mut p).unwrap();
    // A second tape: copy the level's tape object 4 screens earlier
    let mut s = boot(&dll, &p, level);
    let first = tapes(&mut s);
    println!("tape blocks in {level:03X}: {:?}", first.iter().take(3).collect::<Vec<_>>());
    drop(s);
    let (tx, ty) = first[0];
    let r = p.render(level, false).unwrap();
    let owner = r.owner[(ty * r.cols + tx) as usize] as usize - 1;
    let e = p.level_data(level).unwrap().entries[owner];
    println!("tape object: {:?}", e);
    p.add_entries(level, &[(e.kind, e.id, e.settings, e.x - 64, e.y)]).unwrap();
    p.save().unwrap();

    let q = Project::open(rom.clone()).unwrap();
    println!("checkpoints in ROM after reopen: {}", upgrades::has_checkpoints(&q));
    let mut s = boot(&dll, &q, level);
    let all = tapes(&mut s);
    let (cx, cy) = *all.iter().min().unwrap();
    println!("tapes now at blocks: {:?}; touching the one at {cx},{cy}", all.iter().map(|t| (t.0, t.1)).collect::<std::collections::BTreeSet<_>>().into_iter().take(4).collect::<Vec<_>>());
    // Put Mario on it
    for _ in 0..30 {
        let w = s.wram();
        let (x, y) = (cx * 16, cy * 16 - 16);
        w[0x94..0x96].copy_from_slice(&(x as u16).to_le_bytes());
        w[0x96..0x98].copy_from_slice(&(y as u16).to_le_bytes());
        s.run_frame(0);
        if s.wram()[0x13CE] != 0 {
            break;
        }
    }
    let w = s.wram();
    let cp = &w[0x19F80..0x19F85];
    println!("midway flag {:02X}; saved checkpoint: level+1 {:02X}, X {:#06X}, Y {:#06X}", w[0x13CE], cp[0], u16::from_le_bytes([cp[1], cp[2]]), u16::from_le_bytes([cp[3], cp[4]]));
    let want_x = u16::from_le_bytes([cp[1], cp[2]]);
    // Die, then tap A through the map until the level is running again. (The test boots on
    // another level's map tile, so mark this level's tile "midway passed" as the map would.)
    s.wram()[0x71] = 9;
    let tile = if level >= 0x100 { level as usize - 0xDC } else { level as usize };
    let mut left = false;
    for f in 0..3000 {
        s.wram()[0x1EA2 + tile] |= 0x40;
        s.run_frame(if f % 30 < 5 { 1 << 8 } else { 0 });
        let m = s.wram()[0x100];
        if m != 0x14 {
            left = true;
        } else if left {
            break;
        }
    }
    for _ in 0..5 {
        s.run_frame(0);
    }
    let w = s.wram();
    let mx = u16::from_le_bytes([w[0x94], w[0x95]]);
    println!("after dying: Mario starts at X {mx:#06X} (checkpoint {want_x:#06X}) -> {}", if (mx as i32 - want_x as i32).abs() <= 2 { "PASS" } else { "FAIL" });
}
