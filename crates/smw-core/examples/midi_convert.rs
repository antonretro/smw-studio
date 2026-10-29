//! cargo run -p smw-core --example midi_convert -- <out.txt> <file.mid>...
//! Convert MIDI parts to an AddmusicK song and print what was kept.
use std::path::Path;

use smw_core::midi;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let files: Vec<&Path> = a[2..].iter().map(Path::new).collect();
    let song = midi::read(&files).unwrap();
    println!("{} at {:.1} BPM", song.title, song.bpm);
    for p in &song.parts {
        println!("  {:10} {:4} notes, program {:3}{}", p.name, p.notes.len(), p.program, if p.drums { " (drums)" } else { "" });
    }
    let (text, dropped) = midi::to_amk(&song, "");
    println!("left out: {dropped:?}; {} bytes of text", text.len());
    std::fs::write(&a[1], text).unwrap();
}
