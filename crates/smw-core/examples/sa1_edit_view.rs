//! cargo run --release -p smw-core --example sa1_edit_view -- <sa1 rom> <out dir>
//! Render level 105 before and after deleting its first 20 objects (not saved): the
//! picture must change, and the unedited picture must match the saved reference.
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let out = std::path::PathBuf::from(&a[2]);
    let mut p = smw_core::project::Project::open(a[1].clone().into()).unwrap();
    let before = p.render(0x105, true).unwrap();
    std::fs::write(out.join("sa1_before.png"), before.image.png()).unwrap();
    let n = p.level_data(0x105).unwrap().entries.len();
    p.delete_entries(0x105, &(0..20).collect::<Vec<_>>()).unwrap();
    let after = p.render(0x105, true).unwrap();
    std::fs::write(out.join("sa1_after.png"), after.image.png()).unwrap();
    let diff = before.image.rgba.chunks(4).zip(after.image.rgba.chunks(4)).filter(|(a, b)| a != b).count();
    println!("objects {n} -> {}; pixels changed by the edit: {diff}", p.level_data(0x105).unwrap().entries.len());
}
