//! cargo run --release -p smw-core --example statusbar_tiles -- <rom> <out.png> [palette]
//! The layer 3 tiles (0-511) as the status bar designer shows them.
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let p = smw_core::project::Project::open(a[1].clone().into()).unwrap();
    let pal = a.get(3).map(|s| s.parse().unwrap()).unwrap_or(6);
    std::fs::write(&a[2], smw_core::statusbar::tiles_picture(&p, 0x105, pal).png()).unwrap();
}
