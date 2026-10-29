//! cargo run --release -p smw-core --example ow_pictures -- <rom> <out dir>
//! Picture every overworld map (ow_0.png ... ow_6.png).
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let p = smw_core::project::Project::open(a[1].clone().into()).unwrap();
    for s in 0..7u8 {
        let t = std::time::Instant::now();
        let img = smw_core::overworld::submap_picture(&p, s);
        std::fs::write(std::path::Path::new(&a[2]).join(format!("ow_{s}.png")), img.png()).unwrap();
        println!("{s} {}: {:?}", smw_core::overworld::SUBMAPS[s as usize], t.elapsed());
    }
}
