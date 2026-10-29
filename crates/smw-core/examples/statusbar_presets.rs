//! cargo run --release -p smw-core --example statusbar_presets -- <rom> <out dir>
//! Picture every status bar preset (stacked, out/presets.png) and save a copy of the ROM
//! with each one (out/preset_<id>.sfc) for checking in the game.
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let out = std::path::PathBuf::from(&a[2]);
    let p = smw_core::project::Project::open(a[1].clone().into()).unwrap();
    let pics = smw_core::statusbar::preset_pictures(&p, 0x105);
    let mut sheet = smw_core::render::Image::new(256, pics.len() as u32 * 40, [40, 40, 40, 255]);
    for (i, (id, name, _, img)) in pics.iter().enumerate() {
        println!("{id}: {name}");
        for y in 0..img.height {
            let s = (y * 256 * 4) as usize;
            let d = ((i as u32 * 40 + y) * 256 * 4) as usize;
            sheet.rgba[d..d + 1024].copy_from_slice(&img.rgba[s..s + 1024]);
        }
        let copy = out.join(format!("preset_{id}.sfc"));
        std::fs::copy(&a[1], &copy).unwrap();
        let mut q = smw_core::project::Project::open(copy).unwrap();
        smw_core::statusbar::apply_preset(&mut q, id).unwrap();
        q.save().unwrap();
    }
    std::fs::write(out.join("presets.png"), sheet.png()).unwrap();
}
