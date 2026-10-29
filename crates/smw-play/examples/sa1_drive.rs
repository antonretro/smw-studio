//! cargo run -p smw-play --example sa1_drive -- <sa1 rom> <out dir> [frames]
//! Play an SA-1 hack from power-on in Snes9x, pressing Start / A, and save a screenshot at
//! every game mode change (the SA-1 Pack keeps the game mode in BW-RAM $40:0100).
use std::path::{Path, PathBuf};

use smw_play::libretro::Core;

fn png(path: &Path, w: u32, h: u32, rgba: &[u8]) {
    let f = std::fs::File::create(path).unwrap();
    let mut e = png::Encoder::new(std::io::BufWriter::new(f), w, h);
    e.set_color(png::ColorType::Rgba);
    e.write_header().unwrap().write_image_data(rgba).unwrap();
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let out = PathBuf::from(&a[2]);
    std::fs::create_dir_all(&out).unwrap();
    let frames: u32 = a.get(3).map(|s| s.parse().unwrap()).unwrap_or(9000);
    let dll = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../vendor/snes9x/snes9x_libretro.dll");
    let mut core = Core::new(&dll, std::fs::read(&a[1]).unwrap(), &std::env::temp_dir()).unwrap();
    let mut last = 0xFFu8;
    let mut n = 0;
    for f in 0..frames {
        // Start (bit 3) and A (bit 8) taps, alternating
        let buttons = match f % 60 {
            0..=3 => 1 << 3,
            30..=33 => 1 << 8,
            _ => 0,
        };
        core.run_frame(buttons);
        let mode = core.save_ram().get(0x100).copied().unwrap_or(0);
        if mode != last {
            let (w, h, img) = core.frame();
            png(&out.join(format!("{n:02}_f{f}_mode{mode:02X}.png")), w, h, &img);
            println!("frame {f}: game mode {mode:02X}");
            last = mode;
            n += 1;
            if mode == 0x14 {
                let lv = u16::from_le_bytes([core.save_ram()[0x10B], core.save_ram()[0x10C]]);
                println!("  in a level: {lv:03X}");
                break;
            }
        }
    }
}
