//! Bring parts of another hack into your ROM: open a hack once (Lunar Magic exports its
//! levels, graphics, Map16 and palettes into a cache), preview its levels, and copy a level
//! into your ROM with only the graphics and Map16 pages it needs, renumbered or skipped so
//! nothing of yours gets overwritten.
//!
//! How the pieces are found:
//! - A level's graphics: section 7 of its .mwl is the level's 16 graphics slots, each a file
//!   number (low 12 bits; 80+ are ExGFX) plus flag bits. Renumbering = rewriting them.
//! - A level's Map16 pages: load it (in a scratch copy of the clean game with the hack's
//!   graphics, Map16 and the level put in) and read the block numbers the game built.
//! - Map16 pages are copied as single-page LM16 files cut from the hack's full export:
//!   tiles at +$B0 (2 KB a page), "acts like" at +$800B0 (512 bytes a page, pages 00-7F).

use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};

use crate::lunar::{self, Lunar};
use crate::project::Project;
use crate::render::Image;
use crate::{emu, romfile, Cpu};

/// A hack opened for taking parts from.
pub struct Hack {
    pub name:   String,
    pub dir:    PathBuf,
    /// Levels it has (that Lunar Magic exported: the ones the hack changed)
    pub levels: Vec<u16>,
}

fn cache_root() -> PathBuf {
    std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_else(std::env::temp_dir).join("SMW Studio").join("Hacks")
}

fn slug(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    out.trim_matches('-').chars().take(60).collect()
}

fn levels_in(dir: &Path) -> Vec<u16> {
    let mut v: Vec<u16> = std::fs::read_dir(dir.join("levels"))
        .map(|rd| {
            rd.flatten()
                .filter_map(|e| {
                    let n = e.file_name().to_string_lossy().to_string();
                    n.strip_prefix("level ").and_then(|r| r.strip_suffix(".mwl")).and_then(|h| u16::from_str_radix(h, 16).ok())
                })
                .collect()
        })
        .unwrap_or_default();
    v.sort();
    v
}

/// Open a hack: export everything once (cached by file name and size).
pub fn open(lm: &Lunar, hack_rom: &Path) -> Result<Hack, String> {
    let stem = hack_rom.file_stem().and_then(|s| s.to_str()).unwrap_or("hack").to_string();
    let size = std::fs::metadata(hack_rom).map_err(|e| e.to_string())?.len();
    let dir = cache_root().join(format!("{}-{size:x}", slug(&stem)));
    let done = dir.join("exported.ok");
    if !done.exists() {
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("levels")).map_err(|e| e.to_string())?;
        let copy = dir.join("hack.smc");
        std::fs::copy(hack_rom, &copy).map_err(|e| e.to_string())?;
        lunar::ensure_restore_original(&copy)?;
        let r = copy.to_string_lossy().to_string();
        lm.run(&["-ExportGFX", &r])?;
        lm.run(&["-ExportExGFX", &r])?;
        lm.run(&["-ExportAllMap16", &r, &dir.join("hack.map16").to_string_lossy()])?;
        lm.run(&["-ExportMultLevels", &r, &dir.join("levels").join("level").to_string_lossy()])?;
        std::fs::write(&done, stem.as_bytes()).map_err(|e| e.to_string())?;
    }
    let levels = levels_in(&dir);
    if levels.is_empty() {
        return Err("Lunar Magic didn't find any changed levels in that ROM".into());
    }
    Ok(Hack { name: stem, dir, levels })
}

/// A scratch copy of the clean game with the hack's graphics and Map16 in, to put the hack's
/// levels in one at a time for previews and for finding which Map16 pages they use.
fn preview_rom(lm: &Lunar, hack: &Hack) -> Result<PathBuf, String> {
    let dir = hack.dir.join("preview");
    let rom = dir.join("preview.smc");
    if dir.join("ready.ok").exists() {
        return Ok(rom);
    }
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let clean = lunar::remembered_clean_rom().filter(|p| p.exists()).ok_or("SMW Studio needs a clean Super Mario World (USA) ROM: open one once")?;
    let mut bytes = vec![0u8; 0x200];
    bytes.extend(std::fs::read(&clean).map_err(|e| e.to_string())?);
    std::fs::write(&rom, bytes).map_err(|e| e.to_string())?;
    lunar::ensure_restore_original(&rom)?;
    let r = rom.to_string_lossy().to_string();
    lm.run(&["-ExpandROM", &r, "4MB"])?;
    lm.run(&["-ExportGFX", &r])?;
    // Keep the clean game's graphics files (to tell which ones the hack changed)
    copy_dir_files(&dir.join("Graphics"), &dir.join("CleanGraphics"))?;
    copy_dir_files(&hack.dir.join("Graphics"), &dir.join("Graphics"))?;
    copy_dir_files(&hack.dir.join("ExGraphics"), &dir.join("ExGraphics"))?;
    lm.run(&["-ImportAllGraphics", &r])?;
    lm.run(&["-ImportAllMap16", &r, &hack.dir.join("hack.map16").to_string_lossy()])?;
    std::fs::write(dir.join("ready.ok"), b"").map_err(|e| e.to_string())?;
    Ok(rom)
}

fn copy_dir_files(from: &Path, to: &Path) -> Result<(), String> {
    std::fs::create_dir_all(to).map_err(|e| e.to_string())?;
    for e in std::fs::read_dir(from).map_err(|e| format!("{}: {e}", from.display()))?.flatten() {
        if e.path().is_file() {
            std::fs::copy(e.path(), to.join(e.file_name())).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

fn mwl_path(hack: &Hack, level: u16) -> PathBuf {
    hack.dir.join("levels").join(format!("level {level:03X}.mwl"))
}

/// Put one of the hack's levels in the preview ROM (once).
fn level_in_preview(lm: &Lunar, hack: &Hack, level: u16) -> Result<PathBuf, String> {
    let rom = preview_rom(lm, hack)?;
    let mark = hack.dir.join("preview").join(format!("{level:03X}.ok"));
    if !mark.exists() {
        lm.run(&["-ImportLevel", &rom.to_string_lossy(), &mwl_path(hack, level).to_string_lossy(), &format!("{level:X}")])?;
        std::fs::write(&mark, b"").map_err(|e| e.to_string())?;
    }
    Ok(rom)
}

/// A picture of one of the hack's levels (as SMW Studio draws it; cached).
pub fn preview(lm: &Lunar, hack: &Hack, level: u16) -> Result<Vec<u8>, String> {
    let png = hack.dir.join("preview").join(format!("{level:03X}.png"));
    if let Ok(b) = std::fs::read(&png) {
        return Ok(b);
    }
    let rom = level_in_preview(lm, hack, level)?;
    let mut p = Project::open(rom)?;
    let r = p.render(level, true)?;
    // The first two screens are enough for a preview
    let w = r.image.width.min(512);
    let mut img = Image::new(w, r.image.height, [0, 0, 0, 0]);
    for y in 0..r.image.height {
        let s = (y * r.image.width * 4) as usize;
        let d = (y * w * 4) as usize;
        img.rgba[d..d + (w * 4) as usize].copy_from_slice(&r.image.rgba[s..s + (w * 4) as usize]);
    }
    let bytes = img.png();
    std::fs::write(&png, &bytes).map_err(|e| e.to_string())?;
    Ok(bytes)
}

/// The Map16 pages a level's blocks come from (layer 1 and the background).
fn pages_used(rom: &Path, level: u16) -> Result<BTreeSet<u8>, String> {
    let rom = romfile::load(std::fs::read(rom).map_err(|e| e.to_string())?).0;
    let mut cpu = Cpu::new(emu::CheckedMem::new(rom));
    emu::load_level(&mut cpu, level);
    let mut pages = BTreeSet::new();
    for i in 0..0x3800u32 {
        let lo = cpu.mem.game_u8(0x7EC800 + i);
        let hi = cpu.mem.game_u8(0x7FC800 + i);
        if lo != 0 || hi != 0 {
            pages.insert(hi);
        }
    }
    // Background blocks ($7EB900 / $7EBD00): Lunar Magic's background Map16 is pages 80-FF
    for i in 0..0x400u32 {
        let hi = cpu.mem.game_u8(0x7EBD00 + i);
        pages.insert(hi | 0x80);
    }
    Ok(pages)
}

/// One page of a full LM16 export ("tiles" 2 KB, "acts like" 512 bytes; pages 80+ have none).
fn page_of(full: &[u8], page: u8) -> (Vec<u8>, Vec<u8>) {
    let p = page as usize;
    let tiles = full.get(0xB0 + p * 0x800..0xB0 + (p + 1) * 0x800).unwrap_or(&[]).to_vec();
    let acts = if p < 0x80 { full.get(0x800B0 + p * 0x200..0x800B0 + (p + 1) * 0x200).unwrap_or(&[]).to_vec() } else { vec![0; 0x200] };
    (tiles, acts)
}

/// A single-page LM16 file that Lunar Magic's -ImportMap16 puts at `page`.
fn page_file(full: &[u8], page: u8) -> Vec<u8> {
    let (tiles, acts) = page_of(full, page);
    let mut h = full[..0xB0].to_vec();
    let put = |h: &mut Vec<u8>, at: usize, v: u32| h[at..at + 4].copy_from_slice(&v.to_le_bytes());
    put(&mut h, 0x18, 16); // 16 tiles wide
    put(&mut h, 0x1C, 16); // 16 rows: one page
    put(&mut h, 0x20, 0);
    put(&mut h, 0x24, page as u32 * 16); // first row = page * 16
    put(&mut h, 0x28, 0x100); // (as in single-page files Lunar Magic writes)
    for b in h[0x70..0xB0].iter_mut() {
        *b = 0;
    }
    put(&mut h, 0x70, 0xB0);
    put(&mut h, 0x74, tiles.len() as u32);
    put(&mut h, 0x78, 0xB0 + tiles.len() as u32);
    put(&mut h, 0x7C, acts.len() as u32);
    h.extend(tiles);
    h.extend(acts);
    h
}

fn exgfx_files(dir: &Path) -> HashMap<u16, PathBuf> {
    std::fs::read_dir(dir)
        .map(|rd| {
            rd.flatten()
                .filter_map(|e| {
                    let n = e.file_name().to_string_lossy().to_string();
                    let num = n.strip_prefix("ExGFX").and_then(|r| r.strip_suffix(".bin")).and_then(|h| u16::from_str_radix(h, 16).ok())?;
                    Some((num, e.path()))
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Copy one of the hack's levels into `target` as `as_level`. Returns a report.
pub fn bring_level(lm: &Lunar, hack: &Hack, level: u16, target: &Path, as_level: u16) -> Result<Vec<String>, String> {
    let mut report = Vec::new();
    let preview = level_in_preview(lm, hack, level)?;
    lunar::ensure_restore_original(target)?;
    let b = lunar::backup(target, &format!("from {}", slug(&hack.name)))?;
    report.push(format!("Backup: {}", b.display()));
    lm.prepare(target, &mut report)?;
    let t = target.to_string_lossy().to_string();

    // ---- Graphics: the level's 16 slots (section 7 of the .mwl) ----
    let mut mwl = std::fs::read(mwl_path(hack, level)).map_err(|e| e.to_string())?;
    let table = u32::from_le_bytes(mwl[4..8].try_into().unwrap()) as usize;
    let sec = |i: usize| -> (usize, usize) {
        let o = table + i * 8;
        (u32::from_le_bytes(mwl[o..o + 4].try_into().unwrap()) as usize, u32::from_le_bytes(mwl[o + 4..o + 8].try_into().unwrap()) as usize)
    };
    let (gfx_at, gfx_len) = sec(7);
    let target_ex_dir = target.with_file_name("ExGraphics");
    std::fs::create_dir_all(&target_ex_dir).map_err(|e| e.to_string())?;
    let mut theirs = exgfx_files(&target_ex_dir);
    let hack_ex = exgfx_files(&hack.dir.join("ExGraphics"));
    // Graphics files the hack changed in the base game come along as ExGFX
    let clean_gfx = hack.dir.join("preview").join("CleanGraphics");
    let mut remap: HashMap<u16, u16> = HashMap::new();
    let mut next_free = 0x180u16;
    let mut copied = 0;
    let mut new_number = |theirs: &HashMap<u16, PathBuf>| -> Result<u16, String> {
        while next_free < 0xFFF && theirs.contains_key(&next_free) {
            next_free += 1;
        }
        if next_free >= 0xFFF {
            return Err("Your ROM has no free ExGFX numbers left".into());
        }
        next_free += 1;
        Ok(next_free - 1)
    };
    if gfx_len >= 32 {
        for slot in 0..16 {
            let at = gfx_at + slot * 2;
            let v = u16::from_le_bytes([mwl[at], mwl[at + 1]]);
            let (flags, n) = (v & 0xF000, v & 0x0FFF);
            if n == 0xFFF || flags & 0x8000 != 0 && n == 0 {
                continue;
            }
            if let Some(m) = remap.get(&n) {
                mwl[at..at + 2].copy_from_slice(&(flags | m).to_le_bytes());
                continue;
            }
            let data = if n >= 0x80 {
                hack_ex.get(&n).and_then(|p| std::fs::read(p).ok())
            } else {
                // A base-game file: only if the hack changed it
                let hack_file = std::fs::read(hack.dir.join("Graphics").join(format!("GFX{n:02X}.bin"))).ok();
                let clean_file = std::fs::read(clean_gfx.join(format!("GFX{n:02X}.bin"))).ok();
                if hack_file.is_some() && hack_file != clean_file { hack_file } else { None }
            };
            let Some(data) = data else { continue };
            // Already there with the same contents? Use that number
            let same = theirs.iter().find(|(_, p)| std::fs::read(p).ok().as_ref() == Some(&data)).map(|(k, _)| *k);
            let m = match same {
                Some(k) => k,
                None => {
                    let k = if n >= 0x80 && !theirs.contains_key(&n) { n } else { new_number(&theirs)? };
                    let to = target_ex_dir.join(format!("ExGFX{k:X}.bin"));
                    std::fs::write(&to, &data).map_err(|e| e.to_string())?;
                    theirs.insert(k, to);
                    copied += 1;
                    k
                }
            };
            remap.insert(n, m);
            mwl[at..at + 2].copy_from_slice(&(flags | m).to_le_bytes());
        }
    }
    let changed: Vec<String> = {
        let mut v: Vec<_> = remap.iter().filter(|(a, b)| a != b).map(|(a, b)| format!("{a:X}→{b:X}")).collect();
        v.sort();
        v
    };
    report.push(format!(
        "Graphics: {} file{} used, {copied} copied{}",
        remap.len(),
        if remap.len() == 1 { "" } else { "s" },
        if changed.is_empty() { String::new() } else { format!(", renumbered {}", changed.join(" ")) }
    ));
    if copied > 0 {
        report.extend(lm.run(&["-ImportExGFX", &t])?.into_iter().filter(|l| !l.contains("(OK / CANCEL)")));
    }

    // ---- The level ----
    let tmp = std::env::temp_dir().join("smw-studio-bring.mwl");
    std::fs::write(&tmp, &mwl).map_err(|e| e.to_string())?;
    report.extend(lm.run(&["-ImportLevel", &t, &tmp.to_string_lossy(), &format!("{as_level:X}")])?);
    let _ = std::fs::remove_file(&tmp);

    // ---- Map16 pages the level uses ----
    let hack_full = std::fs::read(hack.dir.join("hack.map16")).map_err(|e| e.to_string())?;
    let clean_full = {
        let p = hack.dir.join("preview").join("clean.map16");
        if !p.exists() {
            // The clean game's Map16 (what "unchanged" means)
            let c = hack.dir.join("preview").join("clean.smc");
            let mut bytes = vec![0u8; 0x200];
            bytes.extend(std::fs::read(lunar::remembered_clean_rom().ok_or("No clean ROM")?).map_err(|e| e.to_string())?);
            std::fs::write(&c, bytes).map_err(|e| e.to_string())?;
            lunar::ensure_restore_original(&c)?;
            lm.run(&["-ExportAllMap16", &c.to_string_lossy(), &p.to_string_lossy()])?;
        }
        std::fs::read(&p).map_err(|e| e.to_string())?
    };
    let target_full = {
        let p = std::env::temp_dir().join("smw-studio-target.map16");
        lm.run(&["-ExportAllMap16", &t, &p.to_string_lossy()])?;
        std::fs::read(&p).map_err(|e| e.to_string())?
    };
    let (mut added, mut kept, mut shared) = (vec![], vec![], vec![]);
    // A page the original game doesn't use is all one repeated tile
    let empty = |tiles: &[u8]| tiles.chunks(8).all(|t| t == &tiles[..8]);
    for page in pages_used(&preview, level)? {
        let ours = page_of(&hack_full, page);
        let clean = page_of(&clean_full, page);
        if ours == clean {
            continue; // the game's own blocks: nothing to copy
        }
        // (Pages 00-01 are always the standard blocks; Lunar Magic keeps them per tileset
        // elsewhere in the file, so they can look empty here)
        if page < 2 || !empty(&clean.0) {
            // The game's standard blocks, used by every level: changing them would change
            // all of your levels
            shared.push(format!("{page:02X}"));
            continue;
        }
        let yours = page_of(&target_full, page);
        if yours == ours {
            continue; // already there
        }
        if yours != page_of(&clean_full, page) {
            kept.push(format!("{page:02X}"));
            continue; // your own blocks: don't overwrite
        }
        let f = std::env::temp_dir().join(format!("smw-studio-page{page:02X}.map16"));
        std::fs::write(&f, page_file(&hack_full, page)).map_err(|e| e.to_string())?;
        lm.run(&["-ImportMap16", &t, &f.to_string_lossy(), &format!("{as_level:X}")])?;
        let _ = std::fs::remove_file(&f);
        added.push(format!("{page:02X}"));
    }
    if !added.is_empty() {
        report.push(format!("Map16 pages copied: {}", added.join(" ")));
    }
    if !shared.is_empty() {
        report.push(format!(
            "The hack changed the game's standard blocks on page{} {}; those weren't copied (they'd change all your levels), so a few blocks may look like the original game's",
            if shared.len() == 1 { "" } else { "s" },
            shared.join(" ")
        ));
    }
    if !kept.is_empty() {
        report.push(format!(
            "Map16 pages {} already have your own blocks, so they were left alone: some blocks in this level may look or act differently",
            kept.join(" ")
        ));
    }
    report.push("Custom enemies, blocks with their own code, and patches (like the status bar) don't come with a level".into());
    Ok(report)
}
