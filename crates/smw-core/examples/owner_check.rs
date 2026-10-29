//! cargo run --release -p smw-core --example owner_check -- <rom> <level hex>
//! How many blocks of the level SMW Studio can tell the object of (needed for picking and
//! erasing).
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let mut p = smw_core::project::Project::open(a[1].clone().into()).unwrap();
    let level = u16::from_str_radix(&a[2], 16).unwrap();
    let r = p.render(level, false).unwrap();
    let known = r.owner.iter().filter(|&&o| o != 0).count();
    println!("level {level:03X}: {} objects, {known} of {} blocks have an owner", p.level_data(level).unwrap().entries.len(), r.owner.len());
}
