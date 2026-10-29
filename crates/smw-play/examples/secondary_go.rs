//! cargo run -p smw-play --example secondary_go -- <rom> <work dir> <entrance hex>...
//! For each secondary entrance: point level 106's screen 0 exit at it, start 106 in Snes9x,
//! take the exit, and print which level loads (by sprite list pointer) and where Mario is.
use std::path::{Path, PathBuf};

use smw_core::project::{Project, ScreenExit};
use smw_play::{Options, Session};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let work = PathBuf::from(&a[2]);
    std::fs::create_dir_all(&work).unwrap();
    let dll = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../vendor/snes9x/snes9x_libretro.dll");
    let all = Project::open(PathBuf::from(&a[1])).unwrap().all_secondary_entrances();
    for e in &a[3..] {
        let n = u16::from_str_radix(e, 16).unwrap();
        let copy = work.join("go.sfc");
        std::fs::copy(&a[1], &copy).unwrap();
        let mut p = Project::open(copy.clone()).unwrap();
        p.set_exit(0x106, ScreenExit { screen: 0, dest: n, secondary: true, water: false }).unwrap();
        p.save().unwrap();
        let mut s = Session::boot(&dll, p.build_play_rom(0x106).unwrap(), &std::env::temp_dir(), Options::default()).unwrap();
        {
            let w = s.wram();
            w[0x95] = 0;
            w[0x71] = 6;
            w[0x88] = 0;
            w[0x89] = 0;
        }
        let mut left = false;
        let mut modes = vec![];
        for _ in 0..600 {
            s.run_frame(0);
            let mode = s.wram()[0x100];
            if modes.last() != Some(&mode) {
                modes.push(mode);
            }
            if mode != 0x14 {
                left = true;
            } else if left {
                break;
            }
        }
        print!("modes {modes:02X?}, $010B {:04X}; ", u16::from_le_bytes([s.wram()[0x10B], s.wram()[0x10C]]));
        let w = s.wram();
        let ptr = w[0xCE] as u32 | (w[0xCF] as u32) << 8 | (w[0xD0] as u32) << 16;
        let lv: Vec<String> = (0..0x200u16).filter(|&l| p.sprite_ptr(l) == Some(ptr)).map(|l| format!("{l:03X}")).collect();
        let want = all.iter().find(|x| x.0 == n).map(|x| format!("{:03X}", x.1)).unwrap_or("?".into());
        println!("{n:03X}: table says level {want}; game loaded {lv:?}, Mario ({:#06X}, {:#06X}), flags {:02X} {:02X} {:02X}",
            u16::from_le_bytes([w[0x94], w[0x95]]), u16::from_le_bytes([w[0x96], w[0x97]]), w[0x19B8], w[0x19D8], w[0x1B93]);
    }
}
