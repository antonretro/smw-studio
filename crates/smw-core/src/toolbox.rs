//! The Toolbox: a library of community asset packs (SMW Central zips and the like).
//!
//! Importing a pack copies its files into `<library>/<id>/files/`, works out what each file
//! is, reads the creator from the readme, draws a preview and writes `pack.json`. Packs stay
//! in the library and can be used in any hack (see `lunar::apply`).

use std::io::Read;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// What one file in a pack is, by Lunar Magic's own file conventions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum FileKind {
    /// ExGFXnn.bin: an extra graphics file, number in hex as in the name
    ExGfx { number: u16 },
    /// GFXnn.bin: replaces one of the game's own graphics files
    Gfx { number: u16 },
    /// Graphics without a standard number in the name ("SMM2 Spike.bin"): the user picks
    /// which ExGFX number it becomes
    Graphics,
    /// .map16: Map16 tiles (16x16 blocks) with their behaviour
    Map16,
    /// .pal: a 256-colour palette (with an optional .palmask)
    Palette,
    /// .mwl: a whole level exported from Lunar Magic
    Level,
    /// .asm / .cfg / .json: code for a custom block or sprite (needs GPS / PIXI)
    Code,
    /// .txt that looks like an AddmusicK song
    Music,
    Readme,
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackFile {
    /// Path inside the pack's files folder, '/' separated
    pub path: String,
    pub kind: FileKind,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pack {
    pub id: String,
    pub name: String,
    /// Who made it ("" if the readme didn't say; the user can fill it in)
    pub creator: String,
    /// Short label: Tileset, Background, Graphics replacement, Sprite graphics, ...
    pub kind: String,
    /// What can be done with it, and what it still needs, in plain words
    pub notes: Vec<String>,
    /// Links mentioned in the readme (often the sprite/block the graphics are for)
    pub links: Vec<String>,
    pub files: Vec<PackFile>,
    pub imported_from: String,
}

impl Pack {
    pub fn files_of(&self, f: impl Fn(&FileKind) -> bool) -> impl Iterator<Item = &PackFile> {
        self.files.iter().filter(move |p| f(&p.kind))
    }
}

/// The library folder: %APPDATA%\SMW Studio\Toolbox
pub fn default_dir() -> PathBuf {
    let base = std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
    base.join("SMW Studio").join("Toolbox")
}

fn classify(path: &str, contents: &[u8]) -> FileKind {
    let name = path.rsplit('/').next().unwrap_or(path);
    let lower = name.to_ascii_lowercase();
    let hex_after = |prefix: &str| -> Option<u16> {
        let rest = lower.strip_prefix(prefix)?;
        let digits: String = rest.chars().take_while(|c| c.is_ascii_hexdigit()).collect();
        (!digits.is_empty()).then(|| u16::from_str_radix(&digits, 16).ok()).flatten()
    };
    if lower.ends_with(".bin") {
        if let Some(n) = hex_after("exgfx") {
            return FileKind::ExGfx { number: n };
        }
        if let Some(n) = hex_after("gfx") {
            return FileKind::Gfx { number: n };
        }
        // 4bpp graphics come in multiples of 2 KB
        return if !contents.is_empty() && contents.len() % 0x800 == 0 { FileKind::Graphics } else { FileKind::Other };
    }
    let ext = lower.rsplit('.').next().unwrap_or("");
    match ext {
        "map16" => FileKind::Map16,
        "pal" => FileKind::Palette,
        "mwl" => FileKind::Level,
        "asm" | "cfg" | "json" => FileKind::Code,
        "txt" => {
            let text = String::from_utf8_lossy(contents);
            if text.contains("#samples") || text.contains("#instruments") || text.contains("#amk") {
                FileKind::Music
            } else {
                FileKind::Readme
            }
        }
        _ => FileKind::Other,
    }
}

/// "By: ThunderRebooted", "By Weekly.", "Made by X" -> the name
fn find_creator(readme: &str) -> Option<String> {
    for line in readme.lines() {
        let l = line.trim().trim_matches(|c: char| c == ';' || c == '-' || c == '*' || c == '=').trim();
        let lower = l.to_ascii_lowercase();
        if let Some(i) = lower.find("by:").or_else(|| lower.find("by ")) {
            // Skip phrases like "used by", "made by the sprite": take the words right after "by"
            let before = &lower[..i];
            if !(before.is_empty() || before.ends_with("- ") || before.ends_with("made ") || before.ends_with("created ") || before.contains(" - ")) {
                continue;
            }
            let who = l[i + 3..].trim_start_matches(':').trim().trim_end_matches('.').trim();
            let who: String = who.chars().take_while(|&c| c != ',' && c != '(').collect();
            let who = who.trim();
            if !who.is_empty() && who.len() < 40 && !who.contains("http") {
                return Some(who.to_string());
            }
        }
    }
    None
}

fn find_links(readme: &str) -> Vec<String> {
    let mut out = Vec::new();
    for word in readme.split_whitespace() {
        if word.starts_with("http") && !out.contains(&word.to_string()) {
            out.push(word.trim_end_matches(['.', ',', ')']).to_string());
        }
    }
    out
}

fn describe(name: &str, files: &[PackFile], links: &[String]) -> (String, Vec<String>) {
    let has = |f: fn(&FileKind) -> bool| files.iter().any(|p| f(&p.kind));
    let exgfx = has(|k| matches!(k, FileKind::ExGfx { .. } | FileKind::Graphics));
    let gfx = has(|k| matches!(k, FileKind::Gfx { .. }));
    let map16 = has(|k| *k == FileKind::Map16);
    let level = has(|k| *k == FileKind::Level);
    let code = has(|k| *k == FileKind::Code);
    let music = has(|k| *k == FileKind::Music);
    let lower_name = name.to_ascii_lowercase();
    let bg_named = lower_name.starts_with("bg") || lower_name.contains(" bg")
        || files.iter().all(|p| !matches!(p.kind, FileKind::Map16) || p.path.to_ascii_lowercase().contains("bg"));
    let mut notes = Vec::new();
    let kind = if music {
        "Music"
    } else if code {
        notes.push("Contains block or sprite code (needs GPS/PIXI support, coming later).".into());
        "Custom code"
    } else if map16 && (exgfx || gfx) {
        if level {
            notes.push("Add it as a level: its graphics, tiles, colours and sample level go in together.".into());
        }
        if bg_named { "Background" } else { "Tileset" }
    } else if gfx && !exgfx {
        notes.push("Replaces the game's own graphics everywhere (the old look is kept in the backup).".into());
        "Graphics replacement"
    } else if exgfx && !map16 {
        if !links.is_empty() {
            notes.push("Graphics for a custom sprite: it also needs that sprite (see the links), and sprite insertion isn't built yet.".into());
        } else {
            notes.push("Sprite graphics: they need a sprite that uses them, and sprite insertion isn't built yet.".into());
        }
        "Sprite graphics"
    } else if level {
        notes.push("Add it as a level.".into());
        "Level"
    } else {
        "Other"
    };
    (kind.into(), notes)
}

/// "bg-mushrooms---2026" -> "bg mushrooms 2026", "SMW_Coin" -> "SMW Coin"
pub fn tidy_name(raw: &str) -> String {
    raw.replace(['_', '-'], " ").split_whitespace().collect::<Vec<_>>().join(" ")
}

fn slug(name: &str) -> String {
    let s: String = name.chars().map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '-' }).collect();
    let s = s.split('-').filter(|p| !p.is_empty()).collect::<Vec<_>>().join("-");
    if s.is_empty() { "pack".into() } else { s }
}

/// Import a .zip into the library. Returns the new pack (an existing one with the same
/// name gets a numbered id rather than being replaced).
pub fn import_zip(library: &Path, zip_path: &Path) -> Result<Pack, String> {
    let file = std::fs::File::open(zip_path).map_err(|e| format!("Can't open {}: {e}", zip_path.display()))?;
    let mut zip = zip::ZipArchive::new(file).map_err(|e| format!("Not a readable zip: {e}"))?;
    let name = tidy_name(zip_path.file_stem().and_then(|s| s.to_str()).unwrap_or("Pack"));
    let mut id = slug(&name);
    let mut n = 2;
    while library.join(&id).exists() {
        id = format!("{}-{n}", slug(&name));
        n += 1;
    }
    let dir = library.join(&id);
    let files_dir = dir.join("files");
    std::fs::create_dir_all(&files_dir).map_err(|e| e.to_string())?;

    let mut files = Vec::new();
    let mut readme = String::new();
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i).map_err(|e| e.to_string())?;
        if entry.is_dir() {
            continue;
        }
        // enclosed_name rejects paths that would escape the folder (../, absolute)
        let Some(rel) = entry.enclosed_name() else { continue };
        let rel_str = rel.to_string_lossy().replace('\\', "/");
        let mut data = Vec::new();
        entry.read_to_end(&mut data).map_err(|e| e.to_string())?;
        if data.len() > 16 << 20 {
            continue; // nothing in an SMW pack is this big
        }
        let out = files_dir.join(&rel);
        if let Some(parent) = out.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        std::fs::write(&out, &data).map_err(|e| e.to_string())?;
        let kind = classify(&rel_str, &data);
        if kind == FileKind::Readme {
            readme.push_str(&String::from_utf8_lossy(&data));
            readme.push('\n');
        }
        files.push(PackFile { path: rel_str, kind });
    }
    if files.is_empty() {
        let _ = std::fs::remove_dir_all(&dir);
        return Err("The zip is empty".into());
    }
    files.sort_by(|a, b| a.path.cmp(&b.path));
    let links = find_links(&readme);
    let (kind, notes) = describe(&name, &files, &links);
    let pack = Pack {
        id,
        name,
        creator: find_creator(&readme).unwrap_or_default(),
        kind,
        notes,
        links,
        files,
        imported_from: zip_path.display().to_string(),
    };
    save(library, &pack)?;
    if let Some(png) = preview(&files_dir, &pack) {
        let _ = std::fs::write(dir.join("preview.png"), png);
    }
    Ok(pack)
}

pub fn save(library: &Path, pack: &Pack) -> Result<(), String> {
    let json = serde_json::to_string_pretty(pack).map_err(|e| e.to_string())?;
    std::fs::write(library.join(&pack.id).join("pack.json"), json).map_err(|e| e.to_string())
}

pub fn list(library: &Path) -> Vec<Pack> {
    let mut out: Vec<Pack> = std::fs::read_dir(library)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| std::fs::read_to_string(e.path().join("pack.json")).ok())
        .filter_map(|s| serde_json::from_str(&s).ok())
        .collect();
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    out
}

pub fn get(library: &Path, id: &str) -> Result<Pack, String> {
    let s = std::fs::read_to_string(library.join(id).join("pack.json")).map_err(|_| format!("No pack \"{id}\" in the Toolbox"))?;
    serde_json::from_str(&s).map_err(|e| e.to_string())
}

pub fn remove(library: &Path, id: &str) -> Result<(), String> {
    if id.is_empty() || id.contains(['/', '\\', '.']) {
        return Err("Bad pack id".into());
    }
    std::fs::remove_dir_all(library.join(id)).map_err(|e| e.to_string())
}

pub fn files_dir(library: &Path, id: &str) -> PathBuf {
    library.join(id).join("files")
}

/// Preview: the pack's graphics files drawn as tiles (16 per row, 2x), coloured with its
/// first palette's busiest row; grey without a palette.
fn preview(files_dir: &Path, pack: &Pack) -> Option<Vec<u8>> {
    let pal_file = pack.files_of(|k| *k == FileKind::Palette).next();
    let pal = pal_file.and_then(|f| std::fs::read(files_dir.join(&f.path)).ok());
    let mask = pal_file.and_then(|f| std::fs::read(files_dir.join(f.path.replace(".pal", ".palmask"))).ok()).unwrap_or_default();
    let row = (0..16).max_by_key(|r| (1..16).filter(|c| mask.get(r * 16 + c).is_some_and(|&m| m != 0)).count()).unwrap_or(0);
    let colour = |i: usize| -> [u8; 4] {
        match &pal {
            Some(p) if p.len() >= 768 => [p[(row * 16 + i) * 3], p[(row * 16 + i) * 3 + 1], p[(row * 16 + i) * 3 + 2], 255],
            _ => [(i * 17) as u8, (i * 17) as u8, (i * 17) as u8, 255],
        }
    };
    let mut gfx = Vec::new();
    for f in pack.files_of(|k| matches!(k, FileKind::ExGfx { .. } | FileKind::Gfx { .. } | FileKind::Graphics)).take(2) {
        gfx.extend(std::fs::read(files_dir.join(&f.path)).ok()?);
    }
    if gfx.is_empty() {
        return None;
    }
    // Drop fully empty tile rows at the end so previews aren't mostly blank
    let tiles = gfx.len() / 32;
    let used = (0..tiles).rev().find(|t| gfx[t * 32..t * 32 + 32].iter().any(|&b| b != 0)).map_or(0, |t| t + 1);
    let rows = used.div_ceil(16).max(1);
    let (w, h) = (128u32, rows as u32 * 8);
    let mut img = vec![0u8; (w * h * 4) as usize];
    for t in 0..rows * 16 {
        for y in 0..8 {
            let b = t * 32;
            if b + 32 > gfx.len() {
                break;
            }
            let p = [gfx[b + y * 2], gfx[b + y * 2 + 1], gfx[b + 16 + y * 2], gfx[b + 17 + y * 2]];
            for x in 0..8 {
                let bit = 7 - x;
                let v = (p[0] >> bit & 1) | (p[1] >> bit & 1) << 1 | (p[2] >> bit & 1) << 2 | (p[3] >> bit & 1) << 3;
                let o = ((((t / 16) * 8 + y) as u32 * w + ((t % 16) * 8 + x) as u32) * 4) as usize;
                if v != 0 {
                    img[o..o + 4].copy_from_slice(&colour(v as usize));
                }
            }
        }
    }
    let mut out = Vec::new();
    {
        let mut e = png::Encoder::new(&mut out, w, h);
        e.set_color(png::ColorType::Rgba);
        e.write_header().ok()?.write_image_data(&img).ok()?;
    }
    Some(out)
}
