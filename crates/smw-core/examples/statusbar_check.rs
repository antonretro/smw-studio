//! cargo run --release -p smw-core --example statusbar_check -- <rom> <out dir> [part ids to hide...]
//! Save the status bar picture before and after hiding parts (on a copy of the ROM).
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let out = std::path::PathBuf::from(&a[2]);
    std::fs::create_dir_all(&out).unwrap();
    let copy = out.join("statusbar_test.sfc");
    std::fs::copy(&a[1], &copy).unwrap();
    let mut p = smw_core::project::Project::open(copy.clone()).unwrap();
    println!("supported: {:?}, hidden now: {:?}", smw_core::statusbar::supported(&p), smw_core::statusbar::hidden(&p));
    std::fs::write(out.join("bar_before.png"), smw_core::statusbar::picture(&p, 0x105).png()).unwrap();
    smw_core::statusbar::set_hidden(&mut p, &a[3..]).unwrap();
    println!("hidden after: {:?}", smw_core::statusbar::hidden(&p));
    std::fs::write(out.join("bar_after.png"), smw_core::statusbar::picture(&p, 0x105).png()).unwrap();
    p.save().unwrap();
    let p = smw_core::project::Project::open(copy).unwrap();
    println!("after save and reopen: {:?}", smw_core::statusbar::hidden(&p));
}
