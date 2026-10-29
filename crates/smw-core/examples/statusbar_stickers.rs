//! cargo run --release -p smw-core --example statusbar_stickers -- <rom> <out dir>
//! On copies: (a) a mushroom, star and coin sticker; (b) the same with the item box hidden
//! and shown again. Saves sticker_a.sfc / sticker_b.sfc and a preview picture.
use smw_core::statusbar::{self, Sticker};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let out = std::path::PathBuf::from(&a[2]);
    let st = [
        Sticker { x: 24, y: 16, tile: 0x24, pal: 4, flip_x: false, flip_y: false, big: true },
        Sticker { x: 48, y: 16, tile: 0x48, pal: 2, flip_x: false, flip_y: false, big: true },
        Sticker { x: 196, y: 20, tile: 0xE8, pal: 2, flip_x: false, flip_y: false, big: true },
    ];
    for (name, toggle) in [("a", false), ("b", true)] {
        let copy = out.join(format!("sticker_{name}.sfc"));
        std::fs::copy(&a[1], &copy).unwrap();
        let mut p = smw_core::project::Project::open(copy.clone()).unwrap();
        statusbar::set_stickers(&mut p, &st).unwrap();
        if toggle {
            statusbar::set_hidden(&mut p, &["item".to_string()]).unwrap();
            println!("item hidden: {:?}, stickers kept: {}", statusbar::hidden(&p), statusbar::stickers(&p).len());
            statusbar::set_hidden(&mut p, &[]).unwrap();
        }
        println!("{name}: stickers {:?}", statusbar::stickers(&p) == st);
        std::fs::write(out.join(format!("sticker_{name}.png")), statusbar::picture(&p, 0x105).png()).unwrap();
        p.save().unwrap();
    }
}
