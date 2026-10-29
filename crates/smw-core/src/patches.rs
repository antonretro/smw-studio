//! Patches (Asar .asm files, the way the SMW community shares engine changes) that can be
//! switched on and off. Asar can't take a patch out again, so applying one records every
//! byte it changed and what was there before; switching it off puts those bytes back (if
//! nothing else changed them since).

use std::path::{Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};

/// A patch's adjustable settings: its `!name = value ; comment` lines.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Setting {
    pub name:    String,
    pub value:   String,
    pub comment: String,
    /// Asks for free RAM (named so, or under a "FreeRAM" heading)
    #[serde(default)]
    pub free_ram: bool,
}

/// Bytes a patch changed: file offset (no copier header), before, after.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Change {
    pub offset: usize,
    pub before: Vec<u8>,
    pub after:  Vec<u8>,
}

/// A patch that's in a ROM.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Applied {
    pub name:     String,
    pub settings: Vec<Setting>,
    pub changes:  Vec<Change>,
    /// Per-frame code (UberASM): the routine SMW Studio calls every frame in levels
    #[serde(default)]
    pub frame: Option<u32>,
}

/// Free RAM SMW Studio hands to patches that ask for it ($0F5E-$0F6F: unused by the game,
/// long used by the community; SA-1 patches add their own base to it)
const FREE_RAM: std::ops::RangeInclusive<u32> = 0x0F5E..=0x0F6F;

/// Asar, bundled with AddmusicK.
fn asar() -> Option<PathBuf> {
    crate::music::amk_source().map(|d| d.join("asar.exe")).filter(|p| p.exists())
}

/// The record of patches in `rom` (next to it).
fn manifest_path(rom: &Path) -> PathBuf {
    let mut s = rom.as_os_str().to_owned();
    s.push(".patches.json");
    PathBuf::from(s)
}

pub fn applied(rom: &Path) -> Vec<Applied> {
    std::fs::read(manifest_path(rom)).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

fn save_manifest(rom: &Path, list: &[Applied]) -> Result<(), String> {
    std::fs::write(manifest_path(rom), serde_json::to_vec_pretty(list).map_err(|e| e.to_string())?).map_err(|e| e.to_string())
}

/// The `!name = value ; comment` lines before a patch's code (its settings).
pub fn settings(asm: &str) -> Vec<Setting> {
    let mut out = Vec::new();
    // The last comment-only line: a heading like ";FreeRAM" applies to the settings under it
    let mut heading = String::new();
    for line in asm.lines() {
        let t = line.trim();
        let lower = t.to_ascii_lowercase();
        if lower.starts_with("org ") || lower.starts_with("main:") || lower.starts_with("init:") {
            break;
        }
        if t.starts_with(';') {
            let h = t.trim_start_matches(';').trim();
            if !h.is_empty() {
                heading = h.to_ascii_lowercase();
            }
            continue;
        }
        let Some(rest) = t.strip_prefix('!') else { continue };
        let Some((name, rhs)) = rest.split_once('=') else { continue };
        let (value, comment) = rhs.split_once(';').unwrap_or((rhs, ""));
        let comment = comment.trim().trim_start_matches(['\\', '|', '/']).trim().to_string();
        if out.iter().any(|s: &Setting| s.name == name.trim()) {
            continue; // set again further down (e.g. for SA-1): the first is the user's
        }
        let free_ram = name.to_ascii_lowercase().contains("freeram") || heading.contains("freeram") || comment.to_ascii_lowercase().contains("freeram");
        // A setting for a companion patch that isn't applied with this one
        let value = if heading.contains("set to zero if not using") { "0".to_string() } else { value.trim().to_string() };
        out.push(Setting { name: name.trim().to_string(), value, comment, free_ram });
    }
    out
}

/// Fill in settings that ask for free RAM from SMW Studio's pool (skipping addresses patches
/// already in `rom` use), keeping anything added to them (like `|!base2` for SA-1).
pub fn assign_free_ram(list: &mut [Setting], rom: &Path, frame_code: bool) {
    let used: Vec<u32> = applied(rom).iter().flat_map(|a| a.settings.iter().filter(|s| s.free_ram).filter_map(|s| ram_value(&s.value)).collect::<Vec<_>>()).collect();
    let mut pool = FREE_RAM.filter(|a| !used.contains(a));
    for s in list.iter_mut() {
        if !s.free_ram {
            continue;
        }
        let Some(addr) = pool.next() else { break };
        let (num, rest) = match s.value.find(['|', '+']) {
            Some(i) => (&s.value[..i], &s.value[i..]),
            None => (s.value.as_str(), ""),
        };
        if num.trim().starts_with('$') {
            // Per-frame code's own define for SA-1 RAM; added where the address had none
            let rest = if rest.is_empty() && frame_code { "|!addr" } else { rest };
            s.value = format!("${addr:04X}{rest}");
        }
    }
}

/// The address in a setting like `$0F5E|!base2`.
fn ram_value(v: &str) -> Option<u32> {
    let num = v.split(['|', '+']).next()?.trim().strip_prefix('$')?;
    u32::from_str_radix(num, 16).ok()
}

/// `asm` with its settings set to `list`.
fn with_settings(asm: &str, list: &[Setting]) -> String {
    let mut done = std::collections::HashSet::new();
    let mut out = String::new();
    for line in asm.lines() {
        let t = line.trim_start();
        let replaced = t.strip_prefix('!').and_then(|rest| {
            let (name, _) = rest.split_once('=')?;
            let name = name.trim();
            let s = list.iter().find(|s| s.name == name)?;
            // Only the first assignment is the setting (later ones are SA-1 overrides)
            done.insert(name.to_string()).then(|| format!("!{name} = {}", s.value))
        });
        out.push_str(replaced.as_deref().unwrap_or(line));
        out.push('\n');
    }
    out
}

/// Apply the patch `main` (an .asm file; files it includes sit next to it) to the ROM file
/// with these settings. The ROM must be saved first. Returns Asar's messages.
pub fn apply(rom: &Path, name: &str, main: &Path, list: &[Setting]) -> Result<Vec<String>, String> {
    if applied(rom).iter().any(|a| a.name == name) {
        return Err(format!("{name} is already in this ROM"));
    }
    let asar = asar().ok_or("Asar (the patch tool) wasn't found; it comes with SMW Studio's AddmusicK")?;
    let work = std::env::temp_dir().join("smw-studio-patch");
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&work).map_err(|e| e.to_string())?;
    // The patch's folder (for incsrc / incbin), then the patch with its settings
    let dir = main.parent().ok_or("bad patch path")?;
    for e in std::fs::read_dir(dir).map_err(|e| e.to_string())?.flatten() {
        if e.path().is_file() {
            let _ = std::fs::copy(e.path(), work.join(e.file_name()));
        }
    }
    let asm = std::fs::read_to_string(main).map_err(|e| format!("Couldn't read the patch: {e}"))?;
    let frame_code = is_frame_code(&asm);
    if frame_code {
        // UberASM code: assembled in free space behind a wrapper that sets its bank, called
        // every frame in levels by SMW Studio (Asar prints the wrapper's address)
        std::fs::write(work.join("smw-studio-code.asm"), with_settings(&asm, list)).map_err(|e| e.to_string())?;
        std::fs::write(work.join("smw-studio-patch.asm"), FRAME_WRAPPER).map_err(|e| e.to_string())?;
    } else {
        std::fs::write(work.join("smw-studio-patch.asm"), with_settings(&asm, list)).map_err(|e| e.to_string())?;
    }
    let file = std::fs::read(rom).map_err(|e| e.to_string())?;
    let header = if file.len() % 0x8000 == 0x200 { 0x200 } else { 0 };
    std::fs::write(work.join("rom.smc"), &file).map_err(|e| e.to_string())?;
    let out = Command::new(&asar)
        // SMW Studio checks the ROM itself; hacks often have odd header titles
        .args(["--fix-checksum=on", "--no-title-check", "smw-studio-patch.asm", "rom.smc"])
        .current_dir(&work)
        .stdin(std::process::Stdio::null())
        .output()
        .map_err(|e| format!("Couldn't run Asar: {e}"))?;
    let text = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    let lines: Vec<String> = text.lines().map(str::trim).filter(|l| !l.is_empty()).map(String::from).collect();
    if !out.status.success() {
        let errors: Vec<&String> = lines.iter().filter(|l| l.contains("error")).collect();
        let shown = if errors.is_empty() { lines.iter().collect() } else { errors };
        return Err(format!("The patch couldn't go in; your ROM wasn't changed. {}", shown.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(" / ")));
    }
    let frame = if frame_code {
        let at = lines.iter().find_map(|l| l.split_once("SMWSTUDIO_FRAME=").and_then(|(_, h)| u32::from_str_radix(h.trim(), 16).ok()));
        Some(at.ok_or("Asar didn't say where the code went")?)
    } else {
        None
    };
    let after = std::fs::read(work.join("rom.smc")).map_err(|e| e.to_string())?;
    let (old, new) = (&file[header..], &after[header..]);
    // Every changed run (the checksum is recomputed on any change, so it isn't one)
    let mut changes = Vec::new();
    let mut i = 0;
    let n = old.len().max(new.len());
    while i < n {
        let differs = |i: usize| old.get(i) != new.get(i) && !(0x7FDC..0x7FE0).contains(&i);
        if differs(i) {
            let start = i;
            while i < n && differs(i) {
                i += 1;
            }
            changes.push(Change { offset: start, before: (start..i).map(|k| old.get(k).copied().unwrap_or(0)).collect(), after: new[start..i.min(new.len())].to_vec() });
        } else {
            i += 1;
        }
    }
    // The patch overwrites the game's own code (hooks) where those bytes were still the
    // original game's; if this ROM already changed them (a hack's own code or another
    // patch), putting it in would break that code
    if let Some(clean) = crate::lunar::remembered_clean_rom().and_then(|p| std::fs::read(p).ok()) {
        let clean = &clean[clean.len() % 0x8000..];
        let clash: Vec<String> = changes
            .iter()
            .filter(|c| c.offset < clean.len() && c.before != clean[c.offset..(c.offset + c.before.len()).min(clean.len())])
            // Free space the hack cleared (Asar put new code there): not the game's code
            .filter(|c| !c.before.iter().all(|&b| b == 0x00) && !c.before.iter().all(|&b| b == 0xFF))
            .map(|c| format!("${:02X}:{:04X}", c.offset / 0x8000, 0x8000 + c.offset % 0x8000))
            .collect();
        if !clash.is_empty() {
            return Err(format!(
                "{name} changes the game's code at {}, but this ROM already changed it there (the hack's own code or another patch). Putting it in would likely break the game, so your ROM wasn't changed.",
                clash.join(", ")
            ));
        }
    }
    crate::lunar::backup(rom, name)?;
    std::fs::write(rom, &after).map_err(|e| format!("Couldn't save the ROM: {e}"))?;
    let mut list_now = applied(rom);
    list_now.push(Applied { name: name.to_string(), settings: list.to_vec(), changes, frame });
    save_manifest(rom, &list_now)?;
    Ok(lines)
}

/// UberASM code: no `org` lines, a `main:` label.
pub fn is_frame_code(asm: &str) -> bool {
    let lines: Vec<String> = asm.lines().map(|l| l.trim().to_ascii_lowercase()).collect();
    !lines.iter().any(|l| l.starts_with("org ")) && lines.iter().any(|l| l.starts_with("main:"))
}

/// Wraps UberASM code: the defines it expects (SA-1 or not), then in free space a routine
/// that sets the code's bank and calls its `main` (with 8-bit registers, as UberASM does).
const FRAME_WRAPPER: &str = r#"if read1($00FFD5) == $23
	sa1rom
	!sa1 = 1
	!addr = $6000
	!dp = $3000
	!bank = $000000
else
	lorom
	!sa1 = 0
	!addr = $0000
	!dp = $0000
	!bank = $800000
endif
freecode
SMWStudioFrame:
	PHP
	PHB
	PHK
	PLB
	SEP #$30
	JSL code_main
	PLB
	PLP
	RTL
namespace code
incsrc "smw-studio-code.asm"
namespace off
print "SMWSTUDIO_FRAME=", hex(SMWStudioFrame)
"#;

/// Take patch `name` out: put back every byte it changed. Refuses if something else has
/// changed those bytes since (taking it out then could break the ROM).
pub fn remove(rom: &Path, name: &str) -> Result<(), String> {
    let mut list = applied(rom);
    let at = list.iter().position(|a| a.name == name).ok_or(format!("{name} isn't in this ROM"))?;
    let mut file = std::fs::read(rom).map_err(|e| e.to_string())?;
    let header = if file.len() % 0x8000 == 0x200 { 0x200 } else { 0 };
    let a = &list[at];
    let changed_since = a.changes.iter().any(|c| file.get(header + c.offset..header + c.offset + c.after.len()) != Some(&c.after[..]));
    if changed_since {
        return Err(format!("Something changed the same part of the ROM after {name} went in, so it can't be taken out safely. Restore a backup from before it instead."));
    }
    crate::lunar::backup(rom, &format!("removing {name}"))?;
    for c in &a.changes {
        file[header + c.offset..header + c.offset + c.before.len()].copy_from_slice(&c.before);
    }
    crate::project::fix_checksum_bytes(&mut file[header..]);
    std::fs::write(rom, &file).map_err(|e| e.to_string())?;
    list.remove(at);
    save_manifest(rom, &list)
}

/// A patch in SMW Studio's library (imported once, usable in any ROM).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibraryPatch {
    pub id:   String,
    pub name: String,
    /// The .asm to apply, inside the patch's folder
    pub main: String,
    /// Files it came with that aren't patches (e.g. UberASM code, readme): shown, not applied
    pub other: Vec<String>,
}

/// Where imported patches are kept.
pub fn library_dir() -> PathBuf {
    std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_else(std::env::temp_dir).join("SMW Studio").join("Patches")
}

/// Every imported patch.
pub fn library() -> Vec<LibraryPatch> {
    let mut out: Vec<LibraryPatch> = std::fs::read_dir(library_dir())
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| serde_json::from_slice(&std::fs::read(e.path().join("patch.json")).ok()?).ok())
        .collect();
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    out
}

/// The patch's main file on disk.
pub fn main_path(p: &LibraryPatch) -> PathBuf {
    library_dir().join(&p.id).join(&p.main)
}

/// Import a patch from an .asm file or a .zip with one. The patch is the .asm with `org`
/// lines (UberASM code has none: it runs from another tool, so it's kept but not applied).
pub fn import(path: &Path) -> Result<LibraryPatch, String> {
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("patch").to_string();
    let id: String = stem.to_lowercase().chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect();
    let dir = library_dir().join(&id);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let is_zip = path.extension().is_some_and(|e| e.eq_ignore_ascii_case("zip"));
    if is_zip {
        let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
        let mut zip = zip::ZipArchive::new(file).map_err(|e| format!("Not a readable zip: {e}"))?;
        for i in 0..zip.len() {
            let mut f = zip.by_index(i).map_err(|e| e.to_string())?;
            // Flatten folders, and never write outside the patch's folder
            let Some(name) = f.enclosed_name().and_then(|p| p.file_name().map(|n| n.to_owned())) else { continue };
            if f.is_dir() {
                continue;
            }
            let mut out = std::fs::File::create(dir.join(name)).map_err(|e| e.to_string())?;
            std::io::copy(&mut f, &mut out).map_err(|e| e.to_string())?;
        }
    } else {
        std::fs::copy(path, dir.join(path.file_name().ok_or("bad file")?)).map_err(|e| e.to_string())?;
    }
    let files: Vec<String> = std::fs::read_dir(&dir).map_err(|e| e.to_string())?.flatten().filter_map(|e| e.file_name().to_str().map(String::from)).collect();
    let is_patch = |f: &String| {
        f.to_lowercase().ends_with(".asm") && std::fs::read_to_string(dir.join(f)).is_ok_and(|t| t.lines().any(|l| l.trim_start().to_lowercase().starts_with("org ")))
    };
    let mut patches: Vec<&String> = files.iter().filter(|f| is_patch(f)).collect();
    // Per-frame (UberASM) code comes first: a patch shipped with it is an optional companion
    let frame: Vec<&String> = files.iter().filter(|f| f.to_lowercase().ends_with(".asm") && std::fs::read_to_string(dir.join(f)).is_ok_and(|t| is_frame_code(&t))).collect();
    if frame.len() == 1 {
        patches = frame;
    }
    let main = match patches.as_slice() {
        [one] => (*one).clone(),
        [] => {
            let _ = std::fs::remove_dir_all(&dir);
            return Err("That has no patch or per-frame code in it (no .asm file with `org` lines or a `main:` label).".into());
        }
        many => {
            let _ = std::fs::remove_dir_all(&dir);
            return Err(format!("That has more than one patch in it ({}); import them one at a time.", many.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", ")));
        }
    };
    let name = stem.replace(['_', '-'], " ");
    let other = files.iter().filter(|f| **f != main && *f != "patch.json").cloned().collect();
    let p = LibraryPatch { id, name, main, other };
    std::fs::write(dir.join("patch.json"), serde_json::to_vec_pretty(&p).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    Ok(p)
}
