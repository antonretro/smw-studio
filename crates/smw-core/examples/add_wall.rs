//! cargo run --release -p smw-core --example add_wall -- <rom> <out.sfc>
//! A copy with a cement block column in level 105 (as wall_jump_setup, without the patch).
fn main() {
    let a: Vec<String> = std::env::args().collect();
    std::fs::copy(&a[1], &a[2]).unwrap();
    let mut p = smw_core::project::Project::open(a[2].clone().into()).unwrap();
    p.add_entries(0x105, &[(smw_core::level_data::Kind::Standard, 0x0D, 0x70, 8, 16)]).unwrap();
    p.save().unwrap();
}
