//! cargo run -p smw-play --example play_check -- <rom> <out_dir> <level hex>...
//! For each level: build the play ROM, boot it in Snes9x (hidden), time it, save a
//! screenshot after 2 s of play, and compare the level's block map in the running game
//! with SMW Studio's own loader.
use std::path::Path;

use smw_core::{emu, project::Project, romfile, Cpu};
use smw_play::{Options, Session};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let p = Project::open(a[1].clone().into()).unwrap();
    let out = Path::new(&a[2]);
    let dll = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../vendor/snes9x/snes9x_libretro.dll");
    for l in &a[3..] {
        let level = u16::from_str_radix(l, 16).unwrap();
        let rom = p.build_play_rom(level).unwrap();

        // What SMW Studio expects in RAM for this level
        let mut cpu = Cpu::new(emu::CheckedMem::new(romfile::load(p.build_rom().unwrap()).0));
        emu::load_level(&mut cpu, level);
        let len = if cpu.mem.game_u8(0x5B) & 1 != 0 { 0x3800 } else { (cpu.mem.game_u8(0x5D) as u32 * 432).min(0x3800) };
        let want: Vec<u8> = [0x7EC800u32, 0x7FC800].iter().flat_map(|&b| (0..len).map(move |i| (b, i))).map(|(b, i)| cpu.mem.game_u8(b + i)).collect();
        // Where the running game keeps those tables: WRAM, or BW-RAM on SA-1 hacks
        let sa1 = cpu.mem.sa1.is_some();

        let t = std::time::Instant::now();
        let mut s = match Session::boot(&dll, rom, out, Options::default()) {
            Ok(s) => s,
            Err(e) => {
                println!("{l}: FAIL boot: {e}");
                continue;
            }
        };
        let boot = t.elapsed();
        // With no input Mario must just stand there: on the ground ($77 bit 2) after 1.5 s
        for _ in 0..90 {
            s.step(0);
        }
        let on_ground = s.peek(0x77) & 4 != 0;
        if !on_ground {
            println!("{l}: note - Mario is not standing on the ground 1.5 s in (player anim {:02X})", s.peek(0x71));
        }
        // Play 2 s with no input; castles/ghost houses play their entrance scene first, so
        // keep going until the level's blocks match or 20 s pass
        let mut matched = None;
        for f in 0..1200 {
            s.step(0);
            if f % 60 == 59 && s.peek(0x100) == 0x14 {
                let w = if sa1 { s.save_ram() } else { s.wram() };
                let got: Vec<u8> = [0xC800usize, 0x1C800].iter().flat_map(|&b| w[b..b + len as usize].to_vec()).collect();
                if got == want {
                    matched = Some(f + 1);
                    if f >= 119 {
                        break;
                    }
                }
            }
        }
        let (w, h, rgba) = s.frame();
        let f = std::fs::File::create(out.join(format!("play_{l}.png"))).unwrap();
        let mut e = png::Encoder::new(std::io::BufWriter::new(f), w, h);
        e.set_color(png::ColorType::Rgba);
        e.write_header().unwrap().write_image_data(&rgba).unwrap();
        match matched {
            Some(f) => println!("{l}: PASS  boot {boot:.2?}, level blocks match (frame {f})"),
            None => println!("{l}: FAIL  boot {boot:.2?}, level blocks never matched"),
        }
        drop(s);
    }
}
