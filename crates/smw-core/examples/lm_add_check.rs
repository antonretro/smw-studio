//! cargo run -p smw-core --example lm_add_check -- <rom folder> <rom file name> <work dir>
//! Add every Toolbox pack that has a level to its own copy of the ROM folder (ROM plus the
//! files next to it), the way the app does, and time each one. A Lunar Magic window would
//! show up here as a 60 s "stopped responding" error.
use std::path::PathBuf;

use smw_core::{lunar, project::Project, toolbox};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let (src, name, work) = (PathBuf::from(&a[1]), &a[2], PathBuf::from(&a[3]));
    let lm = lunar::Lunar::find().expect("Lunar Magic not found");
    let library = toolbox::default_dir();
    for pack in toolbox::list(&library) {
        if pack.files_of(|k| *k == toolbox::FileKind::Level).next().is_none() {
            continue;
        }
        let dir = work.join(&pack.id);
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        // Only the files beside the ROM (like the .bak Lunar Magic leaves), not its folders
        for e in std::fs::read_dir(&src).unwrap().flatten().filter(|e| e.path().is_file()) {
            std::fs::copy(e.path(), dir.join(e.file_name())).unwrap();
        }
        let t = std::time::Instant::now();
        let r = lunar::apply(&lm, &dir.join(name), &library, &pack, &lunar::Action::AddLevel { level: 0x10A, mwl: None });
        let loads = Project::open(dir.join(name)).and_then(|mut q| q.level_data(0x10A).map(|d| d.entries.len()));
        println!("{} ({:.1} s): {} / level 10A: {:?}", pack.id, t.elapsed().as_secs_f32(), match &r { Ok(_) => "OK".to_string(), Err(e) => format!("ERR {e}") }, loads);
    }
}
