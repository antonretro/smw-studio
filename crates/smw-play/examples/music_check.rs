//! cargo run -p smw-play --example music_check -- <rom folder> <rom file name> <work dir>
//! Quiet end-to-end check of custom music (nothing is played out loud):
//! 1. make a music pack from one of AddmusicK's own songs (retitled), import it into a
//!    scratch Toolbox
//! 2. add it to a copy of the ROM as a new song, and as a replacement for song 18
//! 3. pick the new song for level 105 and save
//! 4. boot level 105 in Snes9x and read which song the game chose ($0DDA)
use std::io::Write;
use std::path::{Path, PathBuf};

use smw_core::{music, project::Project, toolbox};
use smw_play::{Options, Session};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let (src, name, work) = (PathBuf::from(&a[1]), &a[2], PathBuf::from(&a[3]));
    let _ = std::fs::remove_dir_all(&work);
    let romdir = work.join("rom");
    std::fs::create_dir_all(&romdir).unwrap();
    for e in std::fs::read_dir(&src).unwrap().flatten().filter(|e| e.path().is_file()) {
        std::fs::copy(e.path(), romdir.join(e.file_name())).unwrap();
    }
    let rom = romdir.join(name);

    // 1. A pack: "Test Song.txt" = the Forest of Illusion song, retitled
    let amk = music::amk_source().expect("AddmusicK not found");
    let text = std::fs::read_to_string(amk.join("music/originals/27 Forest of Illusion.txt")).unwrap();
    let text = text.replace("#title \"Forest of Illusion\"", "#title \"Test Song\"");
    let zip_path = work.join("Test Song.zip");
    {
        let mut z = zip::ZipWriter::new(std::fs::File::create(&zip_path).unwrap());
        z.start_file("Test Song.txt", zip::write::SimpleFileOptions::default()).unwrap();
        z.write_all(text.as_bytes()).unwrap();
        z.finish().unwrap();
    }
    let library = work.join("library");
    std::fs::create_dir_all(&library).unwrap();
    let pack = toolbox::import_zip(&library, &zip_path).unwrap();
    println!("pack: {} [{}]", pack.name, pack.kind);

    // 2. New song, and a replacement for 18 (Yoshi's Island)
    let t = std::time::Instant::now();
    match music::add_from_pack(&rom, &library, &pack, None, None) {
        Ok((n, report)) => println!("added as song {n:02X} in {:.1} s: {}", t.elapsed().as_secs_f32(), report.join(" / ")),
        Err(e) => {
            println!("ADD FAILED: {e}");
            return;
        }
    }
    match music::add_from_pack(&rom, &library, &pack, None, Some(0x18)) {
        Ok((n, _)) => println!("replaced song {n:02X}"),
        Err(e) => println!("REPLACE FAILED: {e}"),
    }
    let songs = music::songs(&rom);
    for s in songs.iter().filter(|s| s.pack.is_some() || s.number >= music::FIRST_CUSTOM || s.number == 0x18) {
        println!("  song {:02X}: {} by {:?} ({})", s.number, s.title, s.author, s.file);
    }

    // 3. Level 105 plays song 29
    let mut p = Project::open(rom.clone()).unwrap();
    println!("can pick any song: {}", p.can_pick_any_song());
    p.set_level_song(0x105, 0x29).unwrap();
    p.save().unwrap();
    let mut q = Project::open(rom.clone()).unwrap();
    println!("level 105 song after reopen: {:02X} (want 29)", q.level_song(0x105).unwrap());

    // 4. In the game (no sound output: the session only collects samples)
    let dll = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../vendor/snes9x/snes9x_libretro.dll");
    for (level, want) in [(0x105u16, 0x29u8), (0x106, 0)] {
        let mut s = Session::boot(&dll, q.build_play_rom(level).unwrap(), &std::env::temp_dir(), Options::default()).unwrap();
        for _ in 0..30 {
            s.step(0);
        }
        let samples = s.take_audio();
        let loud = samples.iter().map(|v| v.unsigned_abs() as u32).max().unwrap_or(0);
        let w = s.wram();
        let song = w[0x0DDA];
        let ok = if want == 0 { song != 0x29 } else { song == want };
        println!("game, level {level:03X}: song ${song:02X} {} (sound level {loud}) -> {}", if want == 0 { "(header's own)".to_string() } else { format!("want {want:02X}") }, if ok { "PASS" } else { "FAIL" });
    }
}
