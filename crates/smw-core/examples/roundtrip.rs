//! cargo run -p smw-core --example roundtrip -- <rom>
//! For every vanilla level: parse layer 1 -> encode -> parse must match, and the
//! level rendered from our re-encoded data must equal the level rendered from the ROM.
use smw_core::{emu, project::Project, render, Cpu};

fn main() {
    let path = std::env::args().nth(1).expect("rom path");
    let mut project = Project::open(path.into()).unwrap();
    let rom = smw_core::romfile::load(std::fs::read(std::env::args().nth(1).unwrap()).unwrap()).0;
    let levels: Vec<u16> = (0..0x25).chain(0x101..0x13C).collect();
    let (mut ok, mut same_bytes, mut bad) = (0, 0, Vec::new());
    for level in levels {
        let data = project.level_data(level).unwrap().clone();
        let (bytes, _) = data.encode_with_offsets();
        let (again, _) = smw_core::level_data::LevelData::parse(&bytes, data.vertical).unwrap();
        if again.entries != data.entries {
            bad.push(format!("{level:03X}: parse/encode mismatch"));
            continue;
        }
        let e = &project.edits[&level];
        let orig: Vec<u8> = (0..e.len as u32).map(|i| rom.read(e.ptr + i).unwrap()).collect();
        if orig == bytes {
            same_bytes += 1;
        }

        let mut a = Cpu::new(emu::CheckedMem::new(rom.clone()));
        emu::load_level(&mut a, level);
        let img_a = render::render_level(&mut a);
        let r = project.render(level, false).unwrap();
        if img_a.rgba != r.image.rgba {
            bad.push(format!("{level:03X}: render differs"));
            continue;
        }
        let owned = r.owner.iter().filter(|&&o| o != 0).count();
        if owned == 0 {
            bad.push(format!("{level:03X}: no tile ownership recorded"));
            continue;
        }
        ok += 1;
    }
    println!("{ok} levels OK, {same_bytes} byte-identical re-encodes");
    for b in bad {
        println!("  {b}");
    }
}
