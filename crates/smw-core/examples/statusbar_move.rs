//! cargo run --release -p smw-core --example statusbar_move -- <rom> <out dir>
//! On a copy: move the coin counter to row 3 column 7 and the time to row 3 column 24,
//! hide the score, save and reopen.
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let out = std::path::PathBuf::from(&a[2]);
    let copy = out.join("move_test.sfc");
    std::fs::copy(&a[1], &copy).unwrap();
    let mut p = smw_core::project::Project::open(copy.clone()).unwrap();
    println!("before: {:?}", smw_core::statusbar::counters(&p));
    smw_core::statusbar::set_hidden(&mut p, &["score".to_string()]).unwrap();
    smw_core::statusbar::move_counter(&mut p, "coins", 3, 7).unwrap();
    println!("time over score: {:?}", smw_core::statusbar::move_counter(&mut p, "time", 3, 24).err());
    smw_core::statusbar::move_counter(&mut p, "time", 2, 22).unwrap();
    println!("score on row 2: {:?}", smw_core::statusbar::move_counter(&mut p, "score", 2, 5).err());
    p.save().unwrap();
    let p = smw_core::project::Project::open(copy).unwrap();
    println!("after: {:?}, hidden {:?}", smw_core::statusbar::counters(&p), smw_core::statusbar::hidden(&p));
}
