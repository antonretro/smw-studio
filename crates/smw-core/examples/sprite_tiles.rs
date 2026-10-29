//! cargo run --release -p smw-core --example sprite_tiles -- <rom> <out.png> [colours 0-7]
//! The level's sprite tiles as the sticker picker shows them.
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let p = smw_core::project::Project::open(a[1].clone().into()).unwrap();
    let pal = a.get(3).map(|s| s.parse().unwrap()).unwrap_or(0);
    std::fs::write(&a[2], smw_core::statusbar::sprite_tiles_picture(&p, 0x105, pal).png()).unwrap();
}
