//! cargo run --release -p smw-play --example item_box_check -- <rom>...
//! Put a mushroom in the item box and check the game draws it (sprite slot 56, $02E0) and
//! that slots 1-6 (stickers) are on screen.
use std::path::Path;

use smw_core::project::Project;
use smw_play::{Options, Session};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let dll = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../vendor/snes9x/snes9x_libretro.dll");
    for rom in &a[1..] {
        let p = Project::open(rom.into()).unwrap();
        let mut s = Session::boot(&dll, p.build_play_rom(0x105).unwrap(), &std::env::temp_dir(), Options::default()).unwrap();
        s.poke(0x0DC2, 1); // reserve item: mushroom
        for _ in 0..30 {
            s.step(0);
        }
        let slot = |i: usize, s: &mut Session| (s.peek(0x200 + i * 4), s.peek(0x201 + i * 4), s.peek(0x202 + i * 4));
        let stickers: Vec<_> = (1..7).map(|i| slot(i, &mut s)).filter(|t| t.1 < 0xE0).collect();
        println!("{}: item box sprite {:?}, stickers on screen {:?}", Path::new(rom).file_name().unwrap().to_string_lossy(), slot(56, &mut s), stickers);
    }
}
