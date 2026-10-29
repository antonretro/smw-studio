//! cargo run --release -p smw-core --example rom_songs -- <rom>
//! Each AddmusicK song in the ROM: original, edited original or custom, and where it plays.
use smw_core::{music, names, project::Project};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let p = Project::open(a[1].clone().into()).unwrap();
    let t = std::time::Instant::now();
    let originals = music::original_songs().unwrap();
    println!("{} original songs ({:?})", originals.len(), t.elapsed());
    let t = std::time::Instant::now();
    let used: Vec<(u16, u8)> = (0..0x60u16).map(names::level_of).map(|l| (l, p.level_music(l))).collect();
    println!("level music read in {:?}", t.elapsed());
    for o in music::classify(&p.amk_songs().unwrap(), &originals) {
        let levels: Vec<String> = used.iter().filter(|(_, s)| *s == o.number).map(|(l, _)| format!("{l:03X}")).collect();
        println!("{:02X} {:8} {:3}% {:28} {}", o.number, o.kind, o.match_percent, o.like.unwrap_or_default(), levels.join(" "));
    }
}
