//! cargo run -p smw-core --example save_noop -- <rom> <copy path> [level hex to open first]
//! Save a ROM without changing anything and list every byte range that differs.
use smw_core::project::Project;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    std::fs::copy(&a[1], &a[2]).unwrap();
    let mut p = Project::open(a[2].clone().into()).unwrap();
    if let Some(l) = a.get(3) {
        let level = u16::from_str_radix(l, 16).unwrap();
        p.level_data(level).unwrap();
        p.edits.get_mut(&level).unwrap().dirty = true; // save it as if edited
    }
    for l in p.save().unwrap() {
        println!("  {l}");
    }
    let x = std::fs::read(&a[1]).unwrap();
    let y = std::fs::read(&a[2]).unwrap();
    let mut i = 0;
    let mut n = 0;
    while i < x.len().min(y.len()) {
        if x[i] != y[i] {
            let start = i;
            while i < x.len() && x[i] != y[i] {
                i += 1;
            }
            if n < 20 {
                println!("differs at file ${start:06X}-${i:06X} ({} bytes): {:02X?} -> {:02X?}", i - start, &x[start..i.min(start + 8)], &y[start..i.min(start + 8)]);
            }
            n += 1;
        }
        i += 1;
    }
    println!("{n} ranges differ; sizes {} / {}", x.len(), y.len());
}
