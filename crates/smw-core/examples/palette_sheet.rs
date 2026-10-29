//! cargo run -p smw-core --example palette_sheet -- <rom> <level hex> <out.png>
//! Draw every object palette preview onto one contact sheet.
use smw_core::project::Project;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let mut p = Project::open(a[1].clone().into()).unwrap();
    let level = u16::from_str_radix(&a[2], 16).unwrap();
    let t = std::time::Instant::now();
    let items = p.palette(level).unwrap();
    let drawn = items.iter().filter(|i| !i.png.is_empty()).count();
    println!("{} items, {} with previews, in {:?}", items.len(), drawn, t.elapsed());
    for i in items.iter().filter(|i| i.png.is_empty()) {
        println!("  no preview: {} {:02X} {}", i.kind, i.id, i.name);
    }

    // 12 cells per row, 72x72 each, previews scaled down to fit
    let (cell, cols) = (72u32, 12u32);
    let rows = (items.len() as u32).div_ceil(cols);
    let (w, h) = (cell * cols, cell * rows);
    let mut sheet = vec![40u8; (w * h * 4) as usize];
    for (n, it) in items.iter().enumerate() {
        if it.png.is_empty() {
            continue;
        }
        let dec = png::Decoder::new(std::io::Cursor::new(&it.png));
        let mut r = dec.read_info().unwrap();
        let mut buf = vec![0; r.output_buffer_size()];
        r.next_frame(&mut buf).unwrap();
        let scale = ((cell - 8) as f32 / it.w.max(it.h) as f32).min(2.0);
        let (cx, cy) = ((n as u32 % cols) * cell + 4, (n as u32 / cols) * cell + 4);
        for y in 0..(it.h as f32 * scale) as u32 {
            for x in 0..(it.w as f32 * scale) as u32 {
                let (sx, sy) = ((x as f32 / scale) as u32, (y as f32 / scale) as u32);
                let s = ((sy * it.w + sx) * 4) as usize;
                if buf[s + 3] == 0 {
                    continue;
                }
                let d = (((cy + y) * w + cx + x) * 4) as usize;
                sheet[d..d + 4].copy_from_slice(&buf[s..s + 4]);
            }
        }
    }
    let f = std::fs::File::create(&a[3]).unwrap();
    let mut e = png::Encoder::new(std::io::BufWriter::new(f), w, h);
    e.set_color(png::ColorType::Rgba);
    e.write_header().unwrap().write_image_data(&sheet).unwrap();
}
