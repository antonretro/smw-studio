//! cargo run -p smw-core --example sa1_project -- <sa1 rom> <out.png> <level hex>...
//! Open an SA-1 hack as a project: read levels' objects, entrances and sprites, and draw one.
use smw_core::project::Project;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let t = std::time::Instant::now();
    let mut p = Project::open(a[1].clone().into()).unwrap();
    println!("opened in {:.1} s, SA-1: {}", t.elapsed().as_secs_f32(), p.is_sa1());
    for l in &a[3..] {
        let level = u16::from_str_radix(l, 16).unwrap();
        let ptr = p.layer1_ptr(level).unwrap_or(0);
        match p.level_data(level) {
            Ok(d) => println!("level {level:03X} at ${ptr:06X}: {} objects, {} screens", d.entries.len(), d.header[0] & 0x1F) ,
            Err(e) => println!("level {level:03X}: {e}"),
        }
        println!("  main entrance {:?}, sprites {}", p.main_entrance(level).ok(), p.sprites(level).map(|s| s.sprites.len()).unwrap_or(0));
    }
    let level = u16::from_str_radix(&a[3], 16).unwrap();
    let r = p.render(level, true).unwrap();
    std::fs::write(&a[2], r.image.png()).unwrap();
}
