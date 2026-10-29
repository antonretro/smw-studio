//! cargo run -p smw-core --example object_probe -- <rom> <level hex> <ext|obj> <id hex> <settings hex> <out.png>
//! Place one object at several x/y positions in a level and save a crop of the result.
use smw_core::{level_data::Kind, project::Project};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let mut p = Project::open(a[1].clone().into()).unwrap();
    let level = u16::from_str_radix(&a[2], 16).unwrap();
    let kind = if a[3] == "ext" { Kind::Extended } else { Kind::Standard };
    let id = u8::from_str_radix(&a[4], 16).unwrap();
    let settings = u8::from_str_radix(&a[5], 16).unwrap();
    // Screens 2-3, positions with different x parity and y
    let spots = [(34, 8), (39, 8), (44, 12), (49, 14), (55, 6)];
    for (x, y) in spots {
        p.add_entry(level, kind, id, settings, x, y).unwrap();
    }
    let r = p.render(level, false).unwrap();
    let (x0, w, h) = (32 * 16, 32 * 16, r.image.height);
    let mut crop = vec![0u8; (w * h * 4) as usize];
    for y in 0..h {
        let s = ((y * r.image.width + x0) * 4) as usize;
        let d = (y * w * 4) as usize;
        crop[d..d + (w * 4) as usize].copy_from_slice(&r.image.rgba[s..s + (w * 4) as usize]);
    }
    let f = std::fs::File::create(&a[6]).unwrap();
    let mut e = png::Encoder::new(std::io::BufWriter::new(f), w, h);
    e.set_color(png::ColorType::Rgba);
    e.write_header().unwrap().write_image_data(&crop).unwrap();
}
