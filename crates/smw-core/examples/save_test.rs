//! cargo run -p smw-core --example save_test -- <clean rom> <work dir>
//! Edit levels on a copy, save, reopen, and check that the game's own loader
//! (reading the saved ROM, no override) draws exactly what the editor previewed.
use smw_core::{emu, project::Project, render, romfile, Cpu};

fn plain_render(path: &std::path::Path, level: u16) -> Vec<u8> {
    let rom = romfile::load(std::fs::read(path).unwrap()).0;
    let mut cpu = Cpu::new(emu::CheckedMem::new(rom));
    emu::load_level(&mut cpu, level);
    render::render_level(&mut cpu).rgba
}

fn main() {
    let src = std::env::args().nth(1).unwrap();
    let dir = std::path::PathBuf::from(std::env::args().nth(2).unwrap());
    std::fs::create_dir_all(&dir).unwrap();
    let work = dir.join("save_test.sfc");
    std::fs::copy(&src, &work).unwrap();
    let _ = std::fs::remove_file(dir.join("save_test.sfc.bak"));

    let mut p = Project::open(work.clone()).unwrap();
    // 1) Small edit in place: move the first object of level 105 right by 2 blocks, delete the 3rd
    let first = p.level_data(0x105).unwrap().entries.iter().position(|e| e.kind != smw_core::level_data::Kind::Exit).unwrap();
    let e = p.level_data(0x105).unwrap().entries[first];
    p.change(0x105, |d| { d.move_entry(first, e.x + 2, e.y); d.entries.remove(first + 2); }).unwrap();
    // 2) Grow level 106: copy its first 40 objects one screen later -> must relocate (and expand ROM)
    p.change(0x106, |d| {
        let objs: Vec<_> = d.entries.iter().filter(|e| e.kind != smw_core::level_data::Kind::Exit).take(40).copied().collect();
        for o in objs {
            d.entries.push(o);
            let n = d.entries.len() - 1;
            d.move_entry(n, (o.x + 16).min(0x13F), o.y);
        }
    }).unwrap();
    let preview_105 = p.render(0x105, false).unwrap().image.rgba;
    let preview_106 = p.render(0x106, false).unwrap().image.rgba;
    for line in p.save().unwrap() {
        println!("{line}");
    }

    let size = std::fs::metadata(&work).unwrap().len();
    println!("saved ROM size: {} KB, backup exists: {}", size / 1024, dir.join("save_test.sfc.bak").exists());
    let checks = [
        ("105 saved == preview", plain_render(&work, 0x105) == preview_105),
        ("106 saved == preview", plain_render(&work, 0x106) == preview_106),
        ("105 changed vs clean", plain_render(&work, 0x105) != plain_render(std::path::Path::new(&src), 0x105)),
        ("104 untouched", plain_render(&work, 0x104) == plain_render(std::path::Path::new(&src), 0x104)),
    ];
    let mut reopened = Project::open(work.clone()).unwrap();
    let n106 = reopened.level_data(0x106).unwrap().entries.len();
    println!("reopened 106 has {n106} entries");
    let info = romfile::identify(&std::fs::read(&work).unwrap(), false);
    println!("reopened kind: {:?}", info.kind);
    for (name, ok) in checks {
        println!("{} {name}", if ok { "PASS" } else { "FAIL" });
    }
}
