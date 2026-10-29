//! cargo run -p smw-core --example place_test -- <rom>
//! Place a few new objects on different screens and check each one draws tiles where expected.
use smw_core::{level_data::Kind, project::Project};

fn main() {
    let mut p = Project::open(std::env::args().nth(1).unwrap().into()).unwrap();
    let tests = [(Kind::Standard, 0x0A, 0x11, 20, 14), (Kind::Extended, 0x2D, 0, 3, 12), (Kind::Standard, 0x0D, 0x22, 150, 8)];
    for (kind, id, settings, x, y) in tests {
        let i = p.add_entry(0x105, kind, id, settings, x, y).unwrap();
        let r = p.render(0x105, false).unwrap();
        let blocks: Vec<(u32, u32)> = r
            .owner
            .iter()
            .enumerate()
            .filter(|(_, &o)| o as usize == i + 1)
            .map(|(n, _)| (n as u32 % r.cols, n as u32 / r.cols))
            .collect();
        let hit = blocks.contains(&(x, y));
        println!("{:?} {id:02X} at {x},{y} -> entry {i}, {} blocks, covers its spot: {}", kind, blocks.len(), if hit { "PASS" } else { "FAIL" });
    }
}
