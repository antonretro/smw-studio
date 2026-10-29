//! cargo run -p smw-core --example palette_place_check -- <rom> [level hex]
//! For every palette object: place it the way the editor does (preview top-left under the
//! mouse at block 40,12, object position = that minus the preview offset), render, and check
//! the object's top-left drawn block is exactly 40,12.
use smw_core::{level_data::Kind, project::Project};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let level = a.get(2).map(|s| u16::from_str_radix(s, 16).unwrap()).unwrap_or(0x105);
    let base = Project::open(a[1].clone().into()).unwrap();
    let items = base.clone().palette(level).unwrap();
    let (vx, vy) = (40i32, 12i32);
    let (mut pass, mut fail, mut skipped) = (0, 0, 0);
    for it in &items {
        if it.png.is_empty() {
            skipped += 1;
            continue;
        }
        let mut p = base.clone();
        let kind = if it.kind == "object" { Kind::Standard } else { Kind::Extended };
        let (x, y) = (vx - it.ox, vy - it.oy);
        let i = p.add_entry(level, kind, it.id, it.settings, x as u32, y as u32).unwrap();
        let r = p.render(level, false).unwrap();
        let blocks: Vec<(i32, i32)> = r.owner.iter().enumerate().filter(|(_, &o)| o as usize == i + 1).map(|(n, _)| ((n as u32 % r.cols) as i32, (n as u32 / r.cols) as i32)).collect();
        let top_left = (blocks.iter().map(|b| b.0).min().unwrap_or(-1), blocks.iter().map(|b| b.1).min().unwrap_or(-1));
        if top_left == (vx, vy) {
            pass += 1;
        } else {
            fail += 1;
            println!("FAIL {} {:02X} {:<28} offset ({},{}) landed at {:?}", it.kind, it.id, it.name, it.ox, it.oy, top_left);
        }
    }
    let moved = items.iter().filter(|i| i.ox != 0 || i.oy != 0).count();
    println!("{pass} pass, {fail} fail, {skipped} invisible; {moved} objects draw away from their position (were offset before this fix)");
}
