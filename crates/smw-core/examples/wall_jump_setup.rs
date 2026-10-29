//! cargo run --release -p smw-core --example wall_jump_setup -- <rom> <wall jump .asm> <out.sfc>
//! On a copy: a cement block column in level 105 next to Mario's start, then the wall jump
//! code applied the way the app does it (Asar + a per-frame call). Prints its settings.
use smw_core::{framehook, level_data::Kind, patches, project::Project};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let out = std::path::PathBuf::from(&a[3]);
    std::fs::copy(&a[1], &out).unwrap();
    let _ = std::fs::remove_file(format!("{}.patches.json", out.display()));
    let mut p = Project::open(out.clone()).unwrap();
    // Cement blocks (0D), 1 wide and 8 tall, at x 8, rows 16-23, on the ground
    p.add_entries(0x105, &[(Kind::Standard, 0x0D, 0x70, 8, 16)]).unwrap();
    p.save().unwrap();
    let asm = std::fs::read_to_string(&a[2]).unwrap();
    let mut s = patches::settings(&asm);
    patches::assign_free_ram(&mut s, &out, true);
    for x in s.iter().filter(|x| x.free_ram || x.value == "0") {
        println!("  !{} = {}", x.name, x.value);
    }
    let lines = patches::apply(&out, "Wall jump", std::path::Path::new(&a[2]), &s).unwrap();
    let frame = patches::applied(&out)[0].frame.unwrap();
    println!("code at ${frame:06X} ({} changed runs); asar: {:?}", patches::applied(&out)[0].changes.len(), lines.iter().filter(|l| !l.contains("SMWSTUDIO")).collect::<Vec<_>>());
    let mut p = Project::open(out.clone()).unwrap();
    framehook::set_frame_calls(&mut p, &[frame]).unwrap();
    p.save().unwrap();
    println!("per-frame calls now: {:X?}", framehook::frame_calls(&Project::open(out).unwrap()));
}
