//! cargo run --release -p smw-core --example song_compare -- <reference rom> <rom>
//! For each song in <rom>, the most alike song in <reference rom> and how alike.
use smw_core::{amk, project::Project};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let r = Project::open(a[1].clone().into()).unwrap().amk_songs().unwrap();
    let s = Project::open(a[2].clone().into()).unwrap().amk_songs().unwrap();
    for song in &s {
        let best = r.iter().map(|o| (o.number, amk::similarity(song, o))).max_by(|x, y| x.1.total_cmp(&y.1));
        if let Some((n, sim)) = best {
            println!("{:02X} ({:5} bytes @{:04X}): best {n:02X} {:.0}%", song.number, song.data.len(), song.aram, sim * 100.0);
        }
    }
}
