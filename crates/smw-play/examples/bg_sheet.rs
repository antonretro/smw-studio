//! cargo run -p smw-play --example bg_sheet -- <rom> <out.png> <level hex>...
//! Side by side for each level: the editor's render (no sprites) at the game's camera, and
//! the real game in Snes9x one second after the start. Two levels per row.
use std::path::Path;

use smw_core::project::Project;
use smw_play::{Options, Session};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let mut p = Project::open(a[1].clone().into()).unwrap();
    let dll = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../vendor/snes9x/snes9x_libretro.dll");
    let levels: Vec<u16> = a[3..].iter().map(|s| u16::from_str_radix(s, 16).unwrap()).collect();
    let (cw, ch) = (256usize, 224usize);
    let rows = levels.len().div_ceil(2);
    let sheet_w = cw * 4 + 24;
    let mut sheet = vec![40u8; sheet_w * rows * (ch + 8) * 4];
    for (i, &level) in levels.iter().enumerate() {
        let mut s = match Session::boot(&dll, p.build_play_rom(level).unwrap(), &std::env::temp_dir(), Options::default()) {
            Ok(s) => s,
            Err(e) => {
                println!("{level:03X}: {e}");
                continue;
            }
        };
        for _ in 0..60 {
            s.step(0);
        }
        let w = s.wram().to_vec();
        let (camx, camy) = (u16::from_le_bytes([w[0x1A], w[0x1B]]) as usize, u16::from_le_bytes([w[0x1C], w[0x1D]]) as usize);
        let (gw, gh, game) = s.frame();
        drop(s);
        let r = match p.render(level, false) {
            Ok(r) => r,
            Err(e) => {
                println!("{level:03X}: render: {e}");
                continue;
            }
        };
        let (ox, oy) = ((i % 2) * (cw * 2 + 16), (i / 2) * (ch + 8));
        let mut put = |x: usize, y: usize, px: &[u8]| {
            let o = ((oy + y) * sheet_w + ox + x) * 4;
            sheet[o..o + 4].copy_from_slice(px);
        };
        for y in 0..ch {
            for x in 0..cw {
                let (ex, ey) = (camx + x, camy + y);
                if ex < r.image.width as usize && ey < r.image.height as usize {
                    let o = (ey * r.image.width as usize + ex) * 4;
                    put(x, y, &r.image.rgba[o..o + 4]);
                }
                if x < gw as usize && y < gh as usize {
                    let o = (y * gw as usize + x) * 4;
                    put(cw + 4 + x, y, &game[o..o + 4]);
                }
            }
        }
        println!("{level:03X}: camera ({camx}, {camy})");
    }
    let f = std::fs::File::create(&a[2]).unwrap();
    let mut e = png::Encoder::new(std::io::BufWriter::new(f), sheet_w as u32, (rows * (ch + 8)) as u32);
    e.set_color(png::ColorType::Rgba);
    e.write_header().unwrap().write_image_data(&sheet).unwrap();
}
