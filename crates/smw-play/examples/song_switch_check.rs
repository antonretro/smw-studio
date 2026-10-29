//! cargo run --release -p smw-play --example song_switch_check -- <rom> <level hex> <song hex>...
//! Boot into the level asking for each song (none, then each given), record 3 s of the
//! game's sound (not played), and check the recordings differ and aren't silent.
use std::path::Path;

use smw_core::project::Project;
use smw_play::{Options, Session};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let p = Project::open(a[1].clone().into()).unwrap();
    let rom = p.build_play_rom(u16::from_str_radix(&a[2], 16).unwrap()).unwrap();
    let dll = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../vendor/snes9x/snes9x_libretro.dll");
    let songs: Vec<Option<u8>> = std::iter::once(None).chain(a[3..].iter().map(|s| Some(u8::from_str_radix(s, 16).unwrap()))).collect();
    let mut takes = Vec::new();
    for song in songs {
        let mut s = Session::boot(&dll, rom.clone(), &std::env::temp_dir(), Options { infinite_time: true, invincible: true, song, ..Options::default() }).unwrap();
        let mut audio = Vec::new();
        for _ in 0..180 {
            s.step(0);
            audio.extend(s.take_audio());
        }
        let loud = audio.iter().map(|&x| (x as i32).abs()).max().unwrap_or(0);
        println!("song {:?}: {} samples, loudest {loud}, ${:02X} now", song.map(|n| format!("{n:02X}")), audio.len(), s.peek(0x0DDA));
        takes.push(audio);
        drop(s);
    }
    for i in 1..takes.len() {
        println!("take {i} differs from the level's own song: {}", takes[i] != takes[0]);
    }
}
