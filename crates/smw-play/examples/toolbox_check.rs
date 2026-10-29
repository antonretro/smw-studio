//! cargo run -p smw-play --example toolbox_check -- <clean rom> <work dir> <zip>...
//! Import each zip into a Toolbox at <work dir>/library, then apply each usable pack to its
//! own copy of the clean ROM (sample level -> level 105, or graphics replacement), and save
//! an editor render and an in-game screenshot of level 105 for each.
use std::path::{Path, PathBuf};

use smw_core::{lunar, project::Project, toolbox};
use smw_play::{Options, Session};

fn png(path: &Path, w: u32, h: u32, rgba: &[u8]) {
    let f = std::fs::File::create(path).unwrap();
    let mut e = png::Encoder::new(std::io::BufWriter::new(f), w, h);
    e.set_color(png::ColorType::Rgba);
    e.write_header().unwrap().write_image_data(rgba).unwrap();
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let clean = PathBuf::from(&a[1]);
    let work = PathBuf::from(&a[2]);
    let library = work.join("library");
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&library).unwrap();
    let lm = lunar::Lunar::find().expect("Lunar Magic not found");
    let dll = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../vendor/snes9x/snes9x_libretro.dll");

    for zip in &a[3..] {
        let pack = match toolbox::import_zip(&library, Path::new(zip)) {
            Ok(p) => p,
            Err(e) => {
                println!("IMPORT FAIL {zip}: {e}");
                continue;
            }
        };
        println!("== {} [{}] by {:?}: {} files", pack.name, pack.kind, pack.creator, pack.files.len());
        for n in &pack.notes {
            println!("   note: {n}");
        }
        let action = match pack.kind.as_str() {
            "Tileset" | "Background" | "Level" => lunar::Action::AddLevel { level: 0x105, mwl: None },
            "Graphics replacement" => lunar::Action::ReplaceGraphics,
            _ => {
                println!("   (nothing to apply automatically)");
                continue;
            }
        };
        let dir = work.join("roms").join(&pack.id);
        std::fs::create_dir_all(&dir).unwrap();
        let rom = dir.join(std::env::var("ROM_NAME").unwrap_or("test.smc".into()));
        std::fs::copy(&clean, &rom).unwrap();
        match lunar::apply(&lm, &rom, &library, &pack, &action) {
            Ok(report) => println!("   applied: {}", report.iter().filter(|l| !l.starts_with("Backup")).cloned().collect::<Vec<_>>().join(" | ")),
            Err(e) => {
                println!("   APPLY FAIL: {e}");
                continue;
            }
        }
        // Editor view: first 2 screens
        let mut p = Project::open(rom.clone()).unwrap();
        let r = p.render(0x105, true).unwrap();
        let w = r.image.width.min(512);
        let crop: Vec<u8> = (0..r.image.height).flat_map(|y| r.image.rgba[(y * r.image.width * 4) as usize..((y * r.image.width + w) * 4) as usize].to_vec()).collect();
        png(&work.join(format!("{}_editor.png", pack.id)), w, r.image.height, &crop);
        // In game, 2 s after the start
        match Session::boot(&dll, p.build_play_rom(0x105).unwrap(), &work, Options::default()) {
            Ok(mut s) => {
                for _ in 0..120 {
                    s.step(0);
                }
                let (w, h, rgba) = s.frame();
                png(&work.join(format!("{}_game.png", pack.id)), w, h, &rgba);
                println!("   screenshots saved");
            }
            Err(e) => println!("   PLAY FAIL: {e}"),
        }
    }
}
