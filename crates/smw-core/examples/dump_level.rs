//! cargo run -p smw-core --example dump_level -- <rom> <level hex>
//! Show where re-encoding a level's layer 1 data first differs from the original bytes.
use smw_core::{level_data::LevelData, project::Project, romfile};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let mut p = Project::open(a[1].clone().into()).unwrap();
    let rom = romfile::load(std::fs::read(&a[1]).unwrap()).0;
    let level = u16::from_str_radix(&a[2], 16).unwrap();
    let d = p.level_data(level).unwrap().clone();
    let e = &p.edits[&level];
    let orig: Vec<u8> = (0..e.len as u32).map(|i| rom.read(e.ptr + i).unwrap()).collect();
    let (enc, starts) = d.encode_with_offsets();
    let (again, _) = LevelData::parse(&enc, d.vertical).unwrap();
    println!("ptr ${:06X} orig {} bytes, encoded {} bytes, entries {} -> {}", e.ptr, orig.len(), enc.len(), d.entries.len(), again.entries.len());
    if let Some(i) = d.entries.iter().zip(&again.entries).position(|(x, y)| x != y) {
        println!("first differing entry {i}: {:?} vs {:?}", d.entries[i], again.entries[i]);
        let s = starts[i];
        println!("encoded around: {:02X?}", &enc[s.saturating_sub(9)..(s + 12).min(enc.len())]);
    }
    // Print original bytes as 3-byte groups from the header on, to eyeball alignment
    for (n, c) in orig[5..].chunks(12).enumerate().take(60) {
        println!("{:4}: {:02X?}", 5 + n * 12, c);
    }
}
