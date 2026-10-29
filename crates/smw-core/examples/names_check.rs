//! cargo run -p smw-core --example names_check -- <rom>
//! Print the ROM's own level names (those that differ from the original game's).
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let p = smw_core::project::Project::open(a[1].clone().into()).unwrap();
    for (l, n) in p.level_names() {
        println!("{l:03X} {n}");
    }
}
