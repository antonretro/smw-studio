//! cargo run -p smw-core --example vram_dump -- <rom> <level hex> <out.png>
//! Draw all 4bpp tiles in VRAM ($0000-$FFFF) as a 32-tile-wide sheet with palette row 2.
use smw_core::{emu, romfile, Cpu};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let rom = romfile::load(std::fs::read(&a[1]).unwrap()).0;
    let mut cpu = Cpu::new(emu::CheckedMem::new(rom));
    emu::load_level(&mut cpu, u16::from_str_radix(&a[2], 16).unwrap());
    let (vram, cg) = (&cpu.mem.vram, &cpu.mem.cgram);
    let col = |i: usize| {
        let c = u16::from_le_bytes([cg[i * 2], cg[i * 2 + 1]]);
        let x = |v: u16| ((v & 31) << 3) as u8;
        [x(c), x(c >> 5), x(c >> 10), 255]
    };
    let tiles = 0x800; // 64 KB / 32 bytes
    let (w, h) = (32 * 8, (tiles / 32) * 8);
    let mut img = vec![0u8; w * h * 4];
    for t in 0..tiles {
        for row in 0..8 {
            let b = t * 32;
            let p = [vram[b + row * 2], vram[b + row * 2 + 1], vram[b + 16 + row * 2], vram[b + 17 + row * 2]];
            for c in 0..8 {
                let bit = 7 - c;
                let v = (p[0] >> bit & 1) | (p[1] >> bit & 1) << 1 | (p[2] >> bit & 1) << 2 | (p[3] >> bit & 1) << 3;
                let (x, y) = ((t % 32) * 8 + c, (t / 32) * 8 + row);
                let o = (y * w + x) * 4;
                img[o..o + 4].copy_from_slice(&col(2 * 16 + v as usize));
            }
        }
    }
    let f = std::fs::File::create(&a[3]).unwrap();
    let mut e = png::Encoder::new(std::io::BufWriter::new(f), w as u32, h as u32);
    e.set_color(png::ColorType::Rgba);
    e.write_header().unwrap().write_image_data(&img).unwrap();

}
