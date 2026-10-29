//! cargo run --release -p smw-play --example bar_shots -- <out.png> <rom>...
//! Play each ROM into level 105 (no sound) and stack the top of the screen (the status
//! bar) from each into one picture.
use std::path::Path;

use smw_core::project::Project;
use smw_play::{Options, Session};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let dll = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../vendor/snes9x/snes9x_libretro.dll");
    let (band, w) = (48u32, 256u32);
    let mut sheet = vec![0u8; (w * band * (a.len() as u32 - 2) * 4) as usize];
    for (k, rom) in a[2..].iter().enumerate() {
        let p = Project::open(rom.into()).unwrap();
        let mut s = Session::boot(&dll, p.build_play_rom(0x105).unwrap(), &std::env::temp_dir(), Options::default()).unwrap();
        for _ in 0..120 {
            s.step(0);
        }
        let (fw, _, rgba) = s.frame();
        for y in 0..band {
            for x in 0..w.min(fw) {
                let src = ((y * fw + x) * 4) as usize;
                let dst = (((k as u32 * band + y) * w + x) * 4) as usize;
                sheet[dst..dst + 4].copy_from_slice(&rgba[src..src + 4]);
            }
        }
        println!("{rom}: ok");
    }
    let f = std::fs::File::create(&a[1]).unwrap();
    let mut e = png::Encoder::new(std::io::BufWriter::new(f), w, band * (a.len() as u32 - 2));
    e.set_color(png::ColorType::Rgba);
    e.write_header().unwrap().write_image_data(&sheet).unwrap();
}
