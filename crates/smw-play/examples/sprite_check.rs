//! cargo run -p smw-play --example sprite_check -- <rom> <work dir>
//! 1. Every level's sprite list re-encodes to the exact bytes in the ROM.
//! 2. Add / move / delete sprites, save to a copy, reopen: the list is what we made.
//! 3. Grow a list past its space (forces a move, and on an original ROM the Lunar Magic
//!    bank table), save, reopen, and check the running game reads the new list.
use std::path::{Path, PathBuf};

use smw_core::{project::Project, sprite_data::SpriteList};
use smw_play::{Options, Session};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let work = PathBuf::from(&a[2]);
    std::fs::create_dir_all(&work).unwrap();

    // 1. Round trip
    let mut p = Project::open(a[1].clone().into()).unwrap();
    let (mut ok, mut bad) = (0, 0);
    for level in 0..0x200u16 {
        let Ok(list) = p.sprites(level) else { continue };
        let e = &p.edits[&level];
        let rom = std::fs::read(&a[1]).unwrap();
        let off = if rom.len() % 0x8000 == 0x200 { 0x200 } else { 0 };
        let file = |addr: u32| off + ((addr >> 16) as usize) * 0x8000 + (addr as usize & 0x7FFF);
        let original = &rom[file(e.sprite_ptr)..file(e.sprite_ptr) + e.sprite_len];
        if list.encode() == original { ok += 1 } else { bad += 1; if bad <= 3 { println!("  level {level:03X} re-encodes differently") } }
    }
    println!("1. round trip: {ok} levels identical, {bad} different");

    // 2. Edit, save, reopen
    let copy = work.join("sprites.sfc");
    std::fs::copy(&a[1], &copy).unwrap();
    let mut p = Project::open(copy.clone()).unwrap();
    let before = p.sprites(0x105).unwrap();
    let i = p.add_sprite(0x105, 0x04, 30, 22, 0).unwrap(); // Green Koopa
    let j = p.add_sprite(0x105, 0x74, 2, 20, 0).unwrap(); // Mushroom
    let _ = (i, j);
    let first = p.sprites(0x105).unwrap().sprites[0];
    p.move_sprite(0x105, 0, first.x + 1, first.y).unwrap();
    let last = p.sprites(0x105).unwrap().sprites.len() - 1;
    p.delete_sprite(0x105, last).unwrap();
    let expect = p.sprites(0x105).unwrap();
    for l in p.save().unwrap() {
        println!("   {l}");
    }
    let mut q = Project::open(copy.clone()).unwrap();
    let got = q.sprites(0x105).unwrap();
    println!("2. edit/save/reopen: {} -> {} sprites, reopened list {}", before.sprites.len(), expect.sprites.len(), if got == expect { "MATCHES" } else { "DIFFERS" });

    // 3. Grow past the space
    for k in 0..60u32 {
        q.add_sprite(0x105, 0x0D, 40 + k * 4, 18 + (k % 6), 0).unwrap(); // Bob-ombs
    }
    let big = q.sprites(0x105).unwrap();
    for l in q.save().unwrap() {
        println!("   {l}");
    }
    let mut r = Project::open(copy.clone()).unwrap();
    let back = r.sprites(0x105).unwrap();
    println!("3. grow: {} sprites saved, reopened {}", big.sprites.len(), if back == big { "MATCHES" } else { "DIFFERS" });

    // The running game: read its sprite list through its own pointer ($CE)
    let dll = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../vendor/snes9x/snes9x_libretro.dll");
    let rom = r.build_play_rom(0x105).unwrap();
    let mut s = Session::boot(&dll, rom.clone(), &std::env::temp_dir(), Options::default()).unwrap();
    for _ in 0..10 {
        s.step(0);
    }
    let w = s.wram();
    let ptr = w[0xCE] as u32 | (w[0xCF] as u32) << 8 | (w[0xD0] as u32) << 16;
    let file = ((ptr >> 16) as usize) * 0x8000 + (ptr as usize & 0x7FFF);
    let (in_game, _) = SpriteList::parse(&rom[file..], false).unwrap();
    println!("   game reads sprites at ${ptr:06X}: {} sprites, {}", in_game.sprites.len(), if in_game.sprites == back.sprites { "SAME as edited list - PASS" } else { "DIFFERENT - FAIL" });
}
