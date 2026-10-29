//! cargo run --release -p smw-core --example patch_check -- <rom> <patch.asm> <work dir>
//! On a copy: read the patch's settings, give it free RAM, apply it, then take it out and
//! check the ROM is back to exactly what it was.
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let work = std::path::PathBuf::from(&a[3]);
    std::fs::create_dir_all(&work).unwrap();
    let copy = work.join("patched.sfc");
    std::fs::copy(&a[1], &copy).unwrap();
    let _ = std::fs::remove_file(work.join("patched.sfc.patches.json"));
    let asm = std::fs::read_to_string(&a[2]).unwrap();
    let mut s = smw_core::patches::settings(&asm);
    smw_core::patches::assign_free_ram(&mut s, &copy, smw_core::patches::is_frame_code(&asm));
    for x in &s {
        println!("  !{} = {}   ({})", x.name, x.value, x.comment);
    }
    let before = std::fs::read(&copy).unwrap();
    match smw_core::patches::apply(&copy, "Ground pound", std::path::Path::new(&a[2]), &s) {
        Ok(lines) => println!("applied: {:?}", lines.iter().rev().take(2).collect::<Vec<_>>()),
        Err(e) => { println!("apply failed: {e}"); return; }
    }
    let applied = smw_core::patches::applied(&copy);
    println!("changes: {} runs, {} bytes", applied[0].changes.len(), applied[0].changes.iter().map(|c| c.after.len()).sum::<usize>());
    std::fs::copy(&copy, work.join("ground_pound.sfc")).unwrap();
    smw_core::patches::remove(&copy, "Ground pound").unwrap();
    let back = std::fs::read(&copy).unwrap();
    let diff = before.iter().zip(&back).filter(|(x, y)| x != y).count();
    println!("after taking it out: {} bytes differ from before ({})", diff, if diff == 0 { "identical" } else { "check: checksum?" });
}
