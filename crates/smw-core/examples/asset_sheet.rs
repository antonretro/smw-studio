//! cargo run -p smw-core --example asset_sheet -- <out.png> <file.bin[:palette.pal[:row]]>...
//! Draw 4bpp SNES graphics files (Lunar Magic ExGFX .bin) 16 tiles wide, one under another,
//! coloured with a Lunar Magic .pal (256 colours x RGB). Without a row, the palette row with
//! the most colours in use (per the .palmask next to it) is picked; without a .pal, grey.
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let mut sheets: Vec<(u32, u32, Vec<u8>)> = Vec::new();
    for spec in &a[2..] {
        let parts: Vec<&str> = spec.split('|').collect();
        let gfx = std::fs::read(parts[0]).unwrap();
        let pal = parts.get(1).filter(|p| !p.is_empty()).map(|p| std::fs::read(p).unwrap());
        let row = match (parts.get(2), &pal, parts.get(1)) {
            (Some(r), _, _) => r.parse::<usize>().unwrap(),
            (None, Some(_), Some(p)) => {
                // Busiest row in the mask (mask byte per colour, non-zero = used)
                let mask = std::fs::read(p.replace(".pal", ".palmask")).unwrap_or_default();
                (0..16).max_by_key(|r| (1..16).filter(|c| mask.get(r * 16 + c).is_some_and(|&m| m != 0)).count()).unwrap()
            }
            _ => 0,
        };
        let colour = |i: usize| -> [u8; 4] {
            match &pal {
                Some(p) => [p[(row * 16 + i) * 3], p[(row * 16 + i) * 3 + 1], p[(row * 16 + i) * 3 + 2], 255],
                None => [(i * 17) as u8, (i * 17) as u8, (i * 17) as u8, 255],
            }
        };
        let tiles = gfx.len() / 32;
        let (w, h) = (16 * 8u32, (tiles.div_ceil(16) * 8) as u32);
        let mut img = vec![0u8; (w * h * 4) as usize];
        for t in 0..tiles {
            for y in 0..8 {
                let b = t * 32;
                let p = [gfx[b + y * 2], gfx[b + y * 2 + 1], gfx[b + 16 + y * 2], gfx[b + 17 + y * 2]];
                for x in 0..8 {
                    let bit = 7 - x;
                    let v = (p[0] >> bit & 1) | (p[1] >> bit & 1) << 1 | (p[2] >> bit & 1) << 2 | (p[3] >> bit & 1) << 3;
                    let o = ((((t / 16) * 8 + y) as u32 * w + ((t % 16) * 8 + x) as u32) * 4) as usize;
                    img[o..o + 4].copy_from_slice(&if v == 0 { [40, 40, 48, 255] } else { colour(v as usize) });
                }
            }
        }
        println!("{} -> {} tiles, palette row {row}", parts[0].rsplit(['\\', '/']).next().unwrap(), tiles);
        sheets.push((w, h, img));
    }
    // Stack with a 4 px gap, 2x scale
    let gap = 4;
    let w = 128;
    let h: u32 = sheets.iter().map(|s| s.1 + gap).sum();
    let mut out = vec![255u8; (w * 2 * h * 2 * 4) as usize];
    let mut y0 = 0;
    for (sw, sh, img) in &sheets {
        for y in 0..*sh {
            for x in 0..*sw {
                let s = ((y * sw + x) * 4) as usize;
                for (dy, dx) in [(0, 0), (0, 1), (1, 0), (1, 1)] {
                    let o = ((((y0 + y) * 2 + dy) * w * 2 + x * 2 + dx) * 4) as usize;
                    out[o..o + 4].copy_from_slice(&img[s..s + 4]);
                }
            }
        }
        y0 += sh + gap;
    }
    let f = std::fs::File::create(&a[1]).unwrap();
    let mut e = png::Encoder::new(std::io::BufWriter::new(f), w * 2, h * 2);
    e.set_color(png::ColorType::Rgba);
    e.write_header().unwrap().write_image_data(&out).unwrap();
}
