//! cargo run --release -p smw-play --example oam_usage -- <rom> <level hex>...
//! Play each level for 20 s holding right and jumping (no sound) and list which of the
//! 128 OAM slots were ever on screen (Y below $E0).
use std::path::Path;

use smw_core::project::Project;
use smw_play::{Options, Session};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let p = Project::open(a[1].clone().into()).unwrap();
    let dll = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../vendor/snes9x/snes9x_libretro.dll");
    let mut used = [0u32; 128];
    for l in &a[2..] {
        let level = u16::from_str_radix(l, 16).unwrap();
        let mut s = Session::boot(&dll, p.build_play_rom(level).unwrap(), &std::env::temp_dir(), Options { invincible: true, infinite_time: true, ..Options::default() }).unwrap();
        for f in 0..1200u32 {
            // Right + run (Y), jump (B) on and off, now and then a spin (A) and fire (X)
            let b = (1u16 << 7) | (1 << 1) | if f % 40 < 20 { 1 } else { 0 } | if f % 200 < 10 { 1 << 8 } else { 0 } | if f % 30 == 0 { 1 << 9 } else { 0 };
            s.step(b);
            for i in 0..128 {
                if s.peek(0x201 + i * 4) < 0xE0 {
                    used[i] += 1;
                }
            }
        }
    }
    let free: Vec<usize> = (0..128).filter(|&i| used[i] == 0).collect();
    println!("never used: {:?}", free);
    println!("used (slot: frames): {:?}", (0..128).filter(|&i| used[i] > 0).map(|i| format!("{i}:{}", used[i])).collect::<Vec<_>>());
}
