//! Using Toolbox packs in a ROM by driving Lunar Magic's command line (it's the reference
//! implementation of ExGFX, Map16 pages, custom palettes and .mwl levels, and keeps the ROM
//! compatible with every other SMW tool). Everything here works on the ROM file on disk.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::romfile;
use crate::toolbox::{self, FileKind, Pack};

pub struct Lunar {
    exe: PathBuf,
}

/// What to do with a pack.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Action {
    /// Insert the pack's graphics, then its sample level as `level` (with its Map16 tiles)
    AddLevel { level: u16, mwl: Option<String> },
    /// Replace the game's own GFX files with the pack's
    ReplaceGraphics,
}

impl Lunar {
    /// SMW_LUNAR_MAGIC, next to SMW Studio, or the copy in reference/ during development.
    pub fn find() -> Option<Lunar> {
        let mut candidates = Vec::new();
        if let Some(p) = std::env::var_os("SMW_LUNAR_MAGIC") {
            candidates.push(PathBuf::from(p));
        }
        if let Some(dir) = std::env::current_exe().ok().and_then(|e| e.parent().map(Path::to_path_buf)) {
            candidates.push(dir.join("Lunar Magic").join("Lunar Magic.exe"));
        }
        candidates.push(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../reference/lunar-magic-3.71/Lunar Magic.exe"));
        candidates.into_iter().find(|p| p.exists()).map(|exe| Lunar { exe })
    }

    pub fn path(&self) -> &Path {
        &self.exe
    }

    /// Run one command; Lunar Magic prints its messages (the ones its dialogs would show).
    pub fn run(&self, args: &[&str]) -> Result<Vec<String>, String> {
        // Every command takes a few seconds at most. If one runs much longer, Lunar Magic is
        // sitting on a window waiting for someone to click: close it rather than hang.
        let mut child = Command::new(&self.exe)
            .args(args)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .spawn()
            .map_err(|e| format!("Couldn't run Lunar Magic: {e}"))?;
        let start = std::time::Instant::now();
        let status = loop {
            if let Some(s) = child.try_wait().map_err(|e| e.to_string())? {
                break s;
            }
            if start.elapsed() > std::time::Duration::from_secs(60) {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!(
                    "Lunar Magic stopped responding during {} (it was probably waiting on a window). Nothing after that step was done; your ROM backup is in \"SMW Studio Backups\".",
                    args[0]
                ));
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        };
        let mut stdout = Vec::new();
        if let Some(mut s) = child.stdout.take() {
            use std::io::Read;
            let _ = s.read_to_end(&mut stdout);
        }
        let out = std::process::Output { status, stdout, stderr: Vec::new() };
        let text = String::from_utf8_lossy(&out.stdout);
        let lines: Vec<String> = text.lines().map(str::trim).filter(|l| !l.is_empty()).map(String::from).collect();
        let failed = lines.iter().any(|l| l.contains("Failed") || l.contains("failed") || l.contains("not found in ROM"));
        if failed || !out.status.success() {
            return Err(format!("Lunar Magic {}: {}", args[0], lines.join(" / ")));
        }
        Ok(lines)
    }

    /// One-time setup Lunar Magic needs before it can add graphics to a ROM: convert the
    /// game's graphics to its format (export + import). Skipped once the ROM has it.
    pub fn prepare(&self, rom: &Path, report: &mut Vec<String>) -> Result<(), String> {
        let r = rom.to_string_lossy().to_string();
        let bytes = std::fs::read(rom).map_err(|e| e.to_string())?;
        if find(&bytes, b"Lunar Magic Version").is_none() || !rom.with_file_name("Graphics").exists() {
            self.run(&["-ExportGFX", &r])?;
            self.run(&["-ImportGFX", &r])?;
            report.push("Set up the ROM for extra graphics (one time)".into());
        }
        // Lunar Magic installs its level code (which, among other things, keeps the 4 KB
        // graphics it just converted from overwriting the background map) the first time a
        // level is saved. Without it, backgrounds break when the screen scrolls. Save one
        // level through it (export + import back) if that code isn't there yet.
        let bytes = std::fs::read(rom).map_err(|e| e.to_string())?;
        let header = if bytes.len() % 0x8000 == 0x200 { 0x200 } else { 0 };
        let hook = header + 0x2803B; // $05:803B, where LM's level loader hook goes
        if bytes.get(hook) != Some(&0x5C) {
            let tmp = std::env::temp_dir().join("smw-studio-lm-level.mwl");
            let t = tmp.to_string_lossy().to_string();
            self.run(&["-ExportLevel", &r, &t, "105"])?;
            self.run(&["-ImportLevel", &r, &t, "105"])?;
            let _ = std::fs::remove_file(&tmp);
            report.push("Installed Lunar Magic's level code (one time)".into());
        }
        Ok(())
    }
}

fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}

/// Copy the ROM to "<rom folder>/SMW Studio Backups/<name> before <pack>.<ext>".
pub fn backup(rom: &Path, why: &str) -> Result<PathBuf, String> {
    let dir = rom.with_file_name("SMW Studio Backups");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let stem = rom.file_stem().and_then(|s| s.to_str()).unwrap_or("rom");
    let ext = rom.extension().and_then(|s| s.to_str()).unwrap_or("smc");
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let to = dir.join(format!("{stem} before {why} {stamp}.{ext}"));
    std::fs::copy(rom, &to).map_err(|e| format!("Backup failed: {e}"))?;
    Ok(to)
}

/// Where SMW Studio keeps a copy of the clean game once it has seen one.
pub fn remembered_clean_rom() -> Option<PathBuf> {
    std::env::var_os("APPDATA").map(|d| PathBuf::from(d).join("SMW Studio").join("clean").join("Super Mario World (USA).sfc"))
}

/// The bytes of `path` if it is the clean, unmodified game (with or without a header).
fn clean_rom_at(path: &Path) -> Option<Vec<u8>> {
    let len = std::fs::metadata(path).ok()?.len();
    if len != 0x80000 && len != 0x80200 {
        return None;
    }
    let (bytes, _) = romfile::strip_header(std::fs::read(path).ok()?);
    (romfile::identify(&bytes, false).kind == romfile::RomKind::CleanUsa).then_some(bytes)
}

/// Remember a clean ROM (headerless bytes) for later.
pub fn remember_clean_rom(bytes: &[u8]) {
    if let Some(to) = remembered_clean_rom() {
        if !to.exists() {
            let _ = to.parent().map(std::fs::create_dir_all);
            let _ = std::fs::write(to, bytes);
        }
    }
}

/// Lunar Magic's restore system keeps a clean copy of the game in "sysLMRestore\smwOrig.smc"
/// next to the ROM. When it's missing, Lunar Magic stops and shows an Open window asking for
/// one, even from the command line. Put one there first: from SMW Studio's remembered copy,
/// or any clean copy in the ROM's folder (Lunar Magic leaves one as "<name>.bak").
pub fn ensure_restore_original(rom: &Path) -> Result<(), String> {
    let dir = rom.parent().ok_or("Bad ROM path")?;
    let target = dir.join("sysLMRestore").join("smwOrig.smc");
    if clean_rom_at(&target).is_some() {
        return Ok(());
    }
    let mut candidates: Vec<PathBuf> = remembered_clean_rom().into_iter().collect();
    for d in [dir.to_path_buf(), dir.join("SMW Studio Backups")] {
        if let Ok(rd) = std::fs::read_dir(d) {
            candidates.extend(rd.flatten().map(|e| e.path()).filter(|p| p.is_file()));
        }
    }
    let clean = candidates.iter().find_map(|p| clean_rom_at(p)).ok_or(
        "Lunar Magic needs a clean, unmodified Super Mario World (USA) ROM for its restore points. \
         Open one in SMW Studio once (it will remember it), or put a copy in this ROM's folder, then try again.",
    )?;
    remember_clean_rom(&clean);
    std::fs::create_dir_all(target.parent().unwrap()).map_err(|e| e.to_string())?;
    let mut with_header = vec![0u8; 0x200];
    with_header.extend_from_slice(&clean);
    std::fs::write(&target, with_header).map_err(|e| e.to_string())
}

/// Use `pack` (from the Toolbox at `library`) in the ROM file `rom`. Returns a report.
pub fn apply(lunar: &Lunar, rom: &Path, library: &Path, pack: &Pack, action: &Action) -> Result<Vec<String>, String> {
    let src = toolbox::files_dir(library, &pack.id);
    let mut report = Vec::new();
    ensure_restore_original(rom)?;
    let b = backup(rom, &pack.id)?;
    report.push(format!("Backup: {}", b.display()));
    lunar.prepare(rom, &mut report)?;
    let r = rom.to_string_lossy().to_string();

    match action {
        Action::AddLevel { level, mwl } => {
            // Extra graphics go in the ROM's ExGraphics folder, then into the ROM
            let ex_dir = rom.with_file_name("ExGraphics");
            std::fs::create_dir_all(&ex_dir).map_err(|e| e.to_string())?;
            let mut any = false;
            for f in pack.files_of(|k| matches!(k, FileKind::ExGfx { .. })) {
                let FileKind::ExGfx { number } = f.kind else { continue };
                let data = std::fs::read(src.join(&f.path)).map_err(|e| e.to_string())?;
                let to = ex_dir.join(format!("ExGFX{number:X}.bin"));
                if let Ok(existing) = std::fs::read(&to) {
                    if existing != data {
                        return Err(format!(
                            "ExGFX {number:X} is already used by different graphics in this ROM's ExGraphics folder, so \"{}\" would overwrite them. (Automatic renumbering is coming.)",
                            pack.name
                        ));
                    }
                }
                std::fs::write(&to, &data).map_err(|e| e.to_string())?;
                any = true;
            }
            if any {
                report.extend(lunar.run(&["-ImportExGFX", &r])?);
            }
            let level_hex = format!("{level:X}");
            let mwl_file = pack
                .files_of(|k| *k == FileKind::Level)
                .find(|f| mwl.as_ref().is_none_or(|m| &f.path == m))
                .ok_or("This pack has no sample level (.mwl) to add")?;
            let mwl_path = src.join(&mwl_file.path).to_string_lossy().to_string();
            report.extend(lunar.run(&["-ImportLevel", &r, &mwl_path, &level_hex])?);
            for m in pack.files_of(|k| *k == FileKind::Map16) {
                let p = src.join(&m.path).to_string_lossy().to_string();
                report.extend(lunar.run(&["-ImportMap16", &r, &p, &level_hex])?);
            }
        }
        Action::ReplaceGraphics => {
            // Export the game's graphics, swap in the pack's files, import them all back
            report.extend(lunar.run(&["-ExportGFX", &r])?);
            let gfx_dir = rom.with_file_name("Graphics");
            for f in pack.files_of(|k| matches!(k, FileKind::Gfx { .. })) {
                let FileKind::Gfx { number } = f.kind else { continue };
                std::fs::copy(src.join(&f.path), gfx_dir.join(format!("GFX{number:02X}.bin"))).map_err(|e| e.to_string())?;
                report.push(format!("Replaced GFX{number:02X}"));
            }
            report.extend(lunar.run(&["-ImportGFX", &r])?);
        }
    }
    report.retain(|l| !l.starts_with("Imported Map16 file") && !l.contains("(OK / CANCEL)"));
    Ok(report)
}
