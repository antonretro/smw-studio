//! cargo run -p smw-play --example hack_bring -- <hack rom> <target rom folder> <rom name> <work dir> <level hex> <as level hex>
//! Open a hack, make a preview of one of its levels, bring that level into a copy of the
//! target ROM, then save an editor-vs-game picture of it.
use std::path::{Path, PathBuf};

use smw_core::{hackparts, lunar::Lunar};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let (hack_rom, src, name, work) = (PathBuf::from(&a[1]), PathBuf::from(&a[2]), &a[3], PathBuf::from(&a[4]));
    let level = u16::from_str_radix(&a[5], 16).unwrap();
    let as_level = u16::from_str_radix(&a[6], 16).unwrap();
    let lm = Lunar::find().unwrap();
    let t = std::time::Instant::now();
    let hack = hackparts::open(&lm, &hack_rom).unwrap();
    println!("opened {} in {:.1} s: {} levels", hack.name, t.elapsed().as_secs_f32(), hack.levels.len());
    let t = std::time::Instant::now();
    let png = hackparts::preview(&lm, &hack, level).unwrap();
    std::fs::create_dir_all(&work).unwrap();
    std::fs::write(work.join(format!("preview_{level:03X}.png")), png).unwrap();
    println!("preview of {level:03X} in {:.1} s", t.elapsed().as_secs_f32());
    // A copy of the target folder's files (ROM, .bak, ExGraphics)
    let dir = work.join("target");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    for e in std::fs::read_dir(&src).unwrap().flatten() {
        if e.path().is_file() {
            std::fs::copy(e.path(), dir.join(e.file_name())).unwrap();
        } else if e.file_name() == "ExGraphics" || e.file_name() == "Graphics" {
            let to = dir.join(e.file_name());
            std::fs::create_dir_all(&to).unwrap();
            for f in std::fs::read_dir(e.path()).unwrap().flatten() {
                std::fs::copy(f.path(), to.join(f.file_name())).unwrap();
            }
        }
    }
    let target = dir.join(name);
    let t = std::time::Instant::now();
    match hackparts::bring_level(&lm, &hack, level, &target, as_level) {
        Ok(r) => {
            println!("brought in {level:03X} as {as_level:03X} in {:.1} s:", t.elapsed().as_secs_f32());
            for l in r.iter().filter(|l| !l.starts_with("Backup")) {
                println!("  {l}");
            }
        }
        Err(e) => println!("FAILED: {e}"),
    }
    let _ = Path::new("");
}
