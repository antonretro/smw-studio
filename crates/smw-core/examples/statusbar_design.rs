//! cargo run --release -p smw-core --example statusbar_design -- <rom> <out dir>
//! On a copy: write "WORLD 1-1" in row 1 (widens it), "HERO " over MARIO, recolour the coin
//! digits, save, reopen, and save a picture.
fn tile(c: char) -> u16 {
    match c {
        '0'..='9' => c as u16 - '0' as u16,
        'A'..='Z' => 0x0A + c as u16 - 'A' as u16,
        '-' => 0x27,
        _ => 0xFC,
    }
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let out = std::path::PathBuf::from(&a[2]);
    let copy = out.join("design_test.sfc");
    std::fs::copy(&a[1], &copy).unwrap();
    let mut p = smw_core::project::Project::open(copy.clone()).unwrap();
    let mut changes: Vec<(u32, u32, u16)> = "WORLD 1-1".chars().enumerate().map(|(i, c)| (1, 2 + i as u32, 0x3800 | tile(c))).collect();
    changes.extend("HERO ".chars().enumerate().map(|(i, c)| (2, 2 + i as u32, 0x2800 | tile(c))));
    changes.extend([(2, 28, 0x3C00 | 0xFC), (2, 29, 0x3000)]); // coin digits: palette 4
    smw_core::statusbar::set_cells(&mut p, &changes).unwrap();
    p.save().unwrap();
    let p = smw_core::project::Project::open(copy).unwrap();
    let row = |r: u32| smw_core::statusbar::cells(&p).iter().filter(|c| c.row == r).map(|c| format!("{:04X}", c.entry)).collect::<Vec<_>>().join(" ");
    println!("row 1: {}\nrow 2: {}", row(1), row(2));
    std::fs::write(out.join("design.png"), smw_core::statusbar::picture(&p, 0x105).png()).unwrap();
}
