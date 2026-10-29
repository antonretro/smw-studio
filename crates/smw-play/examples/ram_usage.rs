//! cargo run --release -p smw-play --example ram_usage -- <rom> <level hex>...
//! Play each level for 20 s (running, jumping, spin jumps, fireballs; no sound) and list
//! the bytes of RAM $0000-$1FFF that never changed: candidates for free RAM.
use std::path::Path;

use smw_core::project::Project;
use smw_play::{Options, Session};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let p = Project::open(a[1].clone().into()).unwrap();
    let dll = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../vendor/snes9x/snes9x_libretro.dll");
    let mut changed = vec![false; 0x2000];
    for l in &a[2..] {
        let level = u16::from_str_radix(l, 16).unwrap();
        let mut s = Session::boot(&dll, p.build_play_rom(level).unwrap(), &std::env::temp_dir(), Options { invincible: true, infinite_time: true, powerup: 3, ..Options::default() }).unwrap();
        let mut prev: Vec<u8> = (0..0x2000).map(|i| s.peek(i)).collect();
        for f in 0..1200u32 {
            let b = (1u16 << 7) | (1 << 1) | if f % 40 < 20 { 1 } else { 0 } | if f % 200 < 10 { 1 << 8 } else { 0 } | if f % 30 == 0 { 1 << 9 } else { 0 } | if f % 300 > 280 { 1 << 5 } else { 0 };
            s.step(b);
            for i in 0..0x2000 {
                let v = s.peek(i);
                if v != prev[i] {
                    changed[i] = true;
                    prev[i] = v;
                }
            }
        }
    }
    // Runs of at least 4 untouched bytes
    let mut runs = Vec::new();
    let mut i = 0x100;
    while i < 0x2000 {
        if !changed[i] {
            let start = i;
            while i < 0x2000 && !changed[i] {
                i += 1;
            }
            if i - start >= 4 {
                runs.push(format!("${start:04X}-${:04X}", i - 1));
            }
        }
        i += 1;
    }
    println!("never changed (runs of 4+): {}", runs.join(" "));
    println!("ground pound's defaults: $1E00 changed {}, $1FFF changed {}", changed[0x1E00], changed[0x1FFF]);
}
