//! cargo run -p smw-core --example bytes_roundtrip -- <rom>
//! For all 512 levels: layer 1 data and sprite lists parsed then encoded again must give back
//! exactly the bytes in the ROM (what saving an unchanged level writes).
use smw_core::{project::Project, sprite_data::SpriteList};

fn main() {
    let path = std::env::args().nth(1).expect("rom path");
    let mut p = Project::open(path.clone().into()).unwrap();
    let rom = smw_core::romfile::load(std::fs::read(&path).unwrap()).0;
    let (mut same, mut differ, mut unreadable, mut spr_same, mut spr_differ) = (0, vec![], 0, 0, 0);
    let mut not_equivalent: Vec<String> = vec![];
    let mut spr_changed: Vec<String> = vec![];
    for level in 0..0x200u16 {
        let Ok(data) = p.level_data(level).cloned() else {
            if unreadable < 2 {
                let ptr = p.layer1_ptr(level).unwrap_or(0);
                let raw: Vec<u8> = (0..24u32).map(|i| rom.read(ptr + i).unwrap_or(0xEE)).collect();
                println!("{level:03X} unreadable at ${ptr:06X}: {raw:02X?}");
            }
            unreadable += 1;
            continue;
        };
        let (bytes, _) = data.encode_with_offsets();
        let e = &p.edits[&level];
        let orig: Vec<u8> = (0..e.len as u32).map(|i| rom.read(e.ptr + i).unwrap_or(0xEE)).collect();
        // Encoded differently but the same level? (decodes to the same objects)
        let equivalent = smw_core::level_data::LevelData::parse(&orig, data.vertical).map(|(d, _)| d.entries) == smw_core::level_data::LevelData::parse(&bytes, data.vertical).map(|(d, _)| d.entries);
        if !equivalent {
            not_equivalent.push(format!("{level:03X}"));
        }
        if orig == bytes {
            same += 1;
        } else {
            if differ.len() < 3 {
                let i = first_diff(&orig, &bytes).unwrap_or(0);
                let lo = i.saturating_sub(6);
                println!("{level:03X} at ${:06X}: header {:02X?}, {} vs {} bytes, first difference at {i}: rom {:02X?} ours {:02X?}", e.ptr, &orig[..5], orig.len(), bytes.len(), &orig[lo..(i + 8).min(orig.len())], &bytes[lo..(i + 8).min(bytes.len())]);
            }
            differ.push(format!("{level:03X}"));
        }
        let (sp, sl) = (e.sprite_ptr, e.sprite_len);
        if sl > 0 {
            let raw: Vec<u8> = (0..sl as u32).map(|i| rom.read(sp + i).unwrap_or(0xEE)).collect();
            let (list, _) = SpriteList::parse(&raw, data.vertical).unwrap();
            let enc = list.encode();
            if enc == raw {
                spr_same += 1
            } else {
                spr_differ += 1;
                // Same sprites, maybe in another order?
                let (again, _) = SpriteList::parse(&enc, data.vertical).unwrap();
                let mut a = list.sprites.clone();
                let mut b = again.sprites.clone();
                a.sort_by_key(|s| (s.x, s.y, s.id, s.extra));
                b.sort_by_key(|s| (s.x, s.y, s.id, s.extra));
                if a != b || list.header != again.header {
                    spr_changed.push(format!("{level:03X}"));
                }
            }
        }
    }
    println!("layer 1: {same} identical, {} different {:?}, {unreadable} unreadable", differ.len(), differ.iter().take(12).collect::<Vec<_>>());
    println!("layer 1 levels that decode differently: {} {:?}", not_equivalent.len(), not_equivalent.iter().take(12).collect::<Vec<_>>());
    println!("sprites: {spr_same} identical, {spr_differ} different, of which {} aren't just reordered {:?}", spr_changed.len(), spr_changed.iter().take(8).collect::<Vec<_>>());
}

#[allow(dead_code)]
pub fn first_diff(a: &[u8], b: &[u8]) -> Option<usize> {
    (0..a.len().min(b.len())).find(|&i| a[i] != b[i]).or((a.len() != b.len()).then(|| a.len().min(b.len())))
}
