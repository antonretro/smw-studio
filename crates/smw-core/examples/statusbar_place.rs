//! cargo run --release -p smw-core --example statusbar_place -- <rom> <out dir>
//! On a copy: drop Coins at row 3 column 6 and Lives at row 2 column 22, try Time on top of
//! the score (refused), save.
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let copy = std::path::PathBuf::from(&a[2]).join("place_test.sfc");
    std::fs::copy(&a[1], &copy).unwrap();
    let mut p = smw_core::project::Project::open(copy).unwrap();
    use smw_core::statusbar::place_element;
    place_element(&mut p, "coins", 3, 6).unwrap();
    place_element(&mut p, "lives", 2, 22).unwrap();
    println!("time on score: {:?}", place_element(&mut p, "time", 3, 20).err());
    println!("score on top line: {:?}", place_element(&mut p, "score", 2, 3).err());
    println!("counters: {:?}", smw_core::statusbar::counters(&p));
    p.save().unwrap();
}
