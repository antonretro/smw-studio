//! Custom music through AddmusicK, the standard SMW music inserter.
//!
//! Each ROM gets its own AddmusicK folder next to it ("AddmusicK", the usual layout for
//! SMW projects), copied from the one SMW Studio ships. Songs added from the Toolbox go into
//! its music/ and samples/ folders, SMW Studio keeps the song list in "smw-studio-songs.json"
//! there, writes Addmusic_list.txt from it and runs AddmusicK on the ROM. A song can be new
//! (numbers 29 and up) or replace one of the game's own (0A-28).

use std::path::{Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};

use crate::toolbox::{self, FileKind, Pack};

/// The first song number free for new level songs (0A-28 are the original game's)
pub const FIRST_CUSTOM: u8 = 0x29;

/// One local song slot.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Song {
    pub number: u8,
    pub title:  String,
    /// Composer / porter, from the song's #author (or the pack's credit)
    pub author: String,
    /// Path inside AddmusicK's music folder, '/' separated
    pub file:   String,
    /// Toolbox pack it came from (None for the original game's songs)
    pub pack:   Option<String>,
}

/// The bundled AddmusicK: next to SMW Studio, or vendor/ during development.
pub fn amk_source() -> Option<PathBuf> {
    let mut c = Vec::new();
    if let Some(dir) = std::env::current_exe().ok().and_then(|e| e.parent().map(Path::to_path_buf)) {
        c.push(dir.join("AddmusicK"));
    }
    c.push(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../vendor/addmusick/AddmusicK_1.0.11"));
    c.into_iter().find(|p| p.join("AddmusicK.exe").exists())
}

/// This ROM's AddmusicK folder (it may not exist yet).
pub fn workspace(rom: &Path) -> PathBuf {
    rom.with_file_name("AddmusicK")
}

fn copy_dir(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for e in std::fs::read_dir(from)? {
        let e = e?;
        let t = to.join(e.file_name());
        if e.file_type()?.is_dir() {
            copy_dir(&e.path(), &t)?;
        } else {
            std::fs::copy(e.path(), t)?;
        }
    }
    Ok(())
}

fn ensure_workspace(rom: &Path) -> Result<PathBuf, String> {
    let ws = workspace(rom);
    if !ws.join("AddmusicK.exe").exists() {
        let src = amk_source().ok_or("AddmusicK isn't installed with SMW Studio")?;
        copy_dir(&src, &ws).map_err(|e| format!("Couldn't set up AddmusicK: {e}"))?;
    }
    Ok(ws)
}

/// `#title` / `#author` from a song file.
fn song_info(text: &str) -> (Option<String>, Option<String>) {
    let get = |key: &str| {
        text.lines().find_map(|l| {
            let l = l.trim();
            let rest = l.strip_prefix(key)?;
            let v = rest.trim().trim_matches('"').trim();
            (!v.is_empty()).then(|| v.to_string())
        })
    };
    (get("#title"), get("#author"))
}

/// The original game's level songs, from AddmusicK's default list ("0A  originals/10 Piano.txt")
/// in AddmusicK folder `amk`.
fn originals(amk: &Path) -> Vec<Song> {
    // The workspace keeps the untouched list as .default (its own list gets rewritten)
    let list = std::fs::read_to_string(amk.join("Addmusic_list.default.txt"))
        .or_else(|_| std::fs::read_to_string(amk.join("Addmusic_list.txt")))
        .unwrap_or_default();
    let mut out = Vec::new();
    let mut locals = false;
    for line in list.lines() {
        let l = line.trim();
        if l.starts_with("Locals:") {
            locals = true;
            continue;
        }
        if !locals || l.is_empty() {
            continue;
        }
        let Some((num, file)) = l.split_once(char::is_whitespace) else { continue };
        let Ok(number) = u8::from_str_radix(num, 16) else { continue };
        let file = file.trim().to_string();
        let text = std::fs::read_to_string(amk.join("music").join(&file)).unwrap_or_default();
        let (title, author) = song_info(&text);
        let stem = Path::new(&file).file_stem().and_then(|s| s.to_str()).unwrap_or("").to_string();
        // "24 Yoshi's Island" -> "Yoshi's Island"
        let fallback = stem.split_once(' ').map(|(_, t)| t.to_string()).unwrap_or(stem);
        out.push(Song { number, title: title.unwrap_or(fallback), author: author.unwrap_or_default(), file, pack: None });
    }
    out
}

fn manifest_path(ws: &Path) -> PathBuf {
    ws.join("smw-studio-songs.json")
}

/// Songs SMW Studio added (new ones and replacements), by number.
fn added(ws: &Path) -> Vec<Song> {
    std::fs::read(manifest_path(ws)).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

/// Every level song this ROM has (or will have once AddmusicK runs): the original game's,
/// with SMW Studio's replacements and new songs.
pub fn songs(rom: &Path) -> Vec<Song> {
    let ws = workspace(rom);
    let base = if ws.join("AddmusicK.exe").exists() { Some(ws.clone()) } else { amk_source() };
    let mut out = base.map(|b| originals(&b)).unwrap_or_default();
    for s in added(&ws) {
        match out.iter_mut().find(|o| o.number == s.number) {
            Some(o) => *o = s,
            None => out.push(s),
        }
    }
    out.sort_by_key(|s| s.number);
    out
}

/// Write Addmusic_list.txt: the default globals, then every local song by number.
fn write_list(ws: &Path, songs: &[Song]) -> Result<(), String> {
    let default = std::fs::read_to_string(ws.join("Addmusic_list.default.txt")).map_err(|e| e.to_string())?;
    let globals: String = default.lines().take_while(|l| !l.trim().starts_with("Locals:")).map(|l| format!("{l}\n")).collect();
    let mut out = globals;
    out.push_str("Locals:\n");
    for s in songs {
        out.push_str(&format!("{:02X}  {}\n", s.number, s.file));
    }
    std::fs::write(ws.join("Addmusic_list.txt"), out).map_err(|e| e.to_string())
}

/// Run AddmusicK on the ROM with the current song list. AddmusicK wants the ROM in its own
/// folder, so it works on a copy that then replaces the ROM (after a backup).
pub fn insert(rom: &Path) -> Result<Vec<String>, String> {
    let ws = ensure_workspace(rom)?;
    if !ws.join("Addmusic_list.default.txt").exists() {
        std::fs::copy(ws.join("Addmusic_list.txt"), ws.join("Addmusic_list.default.txt")).map_err(|e| e.to_string())?;
    }
    write_list(&ws, &songs(rom))?;
    let work = ws.join("rom.smc");
    std::fs::copy(rom, &work).map_err(|e| format!("Couldn't copy the ROM for AddmusicK: {e}"))?;
    let mut child = Command::new(ws.join("AddmusicK.exe"))
        .args(["-noblock", "rom.smc"])
        .current_dir(&ws)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("Couldn't run AddmusicK: {e}"))?;
    let start = std::time::Instant::now();
    let status = loop {
        if let Some(s) = child.try_wait().map_err(|e| e.to_string())? {
            break s;
        }
        if start.elapsed() > std::time::Duration::from_secs(60) {
            let _ = child.kill();
            let _ = child.wait();
            return Err("AddmusicK took too long and was stopped. Your ROM wasn't changed.".into());
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    };
    let mut text = String::new();
    use std::io::Read;
    if let Some(mut o) = child.stdout.take() {
        let _ = o.read_to_string(&mut text);
    }
    if let Some(mut e) = child.stderr.take() {
        let _ = e.read_to_string(&mut text);
    }
    let lines: Vec<String> = text.lines().map(str::trim).filter(|l| !l.is_empty()).map(String::from).collect();
    // It ends with "Success!" when everything went in (its welcome text mentions errors too)
    if !status.success() || !lines.iter().any(|l| l == "Success!") {
        let errors: Vec<&String> = lines.iter().filter(|l| l.contains("rror") && !l.contains("Protip")).collect();
        let shown = if errors.is_empty() { lines.iter().rev().take(5).collect() } else { errors };
        return Err(format!("AddmusicK couldn't insert the music; your ROM wasn't changed. {}", shown.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(" / ")));
    }
    crate::lunar::backup(rom, "music")?;
    std::fs::copy(&work, rom).map_err(|e| format!("Couldn't save the ROM: {e}"))?;
    // Song names for Lunar Magic's music list
    if let Ok(msc) = std::fs::read(ws.join("rom.msc")) {
        let _ = std::fs::write(rom.with_extension("msc"), msc);
    }
    let _ = std::fs::remove_file(&work);
    // The lines for SMW Studio's songs ("test-song/Test Song.txt: ..... 0:15, 0x0149 bytes")
    Ok(lines.into_iter().filter(|l| !l.starts_with("originals/") && l.contains(".txt:")).map(|l| l.replace(" .", "").replace("..", "")).collect())
}

/// Copy a music pack's song (`txt`, a path inside the pack) and its samples into the ROM's
/// AddmusicK folder, as song `replace` or the first free new number; then run AddmusicK.
/// Returns the song number.
pub fn add_from_pack(rom: &Path, library: &Path, pack: &Pack, txt: Option<&str>, replace: Option<u8>) -> Result<(u8, Vec<String>), String> {
    let ws = ensure_workspace(rom)?;
    if !ws.join("Addmusic_list.default.txt").exists() {
        std::fs::copy(ws.join("Addmusic_list.txt"), ws.join("Addmusic_list.default.txt")).map_err(|e| e.to_string())?;
    }
    let src = toolbox::files_dir(library, &pack.id);
    let song_file = pack
        .files_of(|k| *k == FileKind::Music)
        .find(|f| txt.is_none_or(|t| f.path == t))
        .ok_or("This pack has no song file")?;
    let text = std::fs::read_to_string(src.join(&song_file.path)).map_err(|e| e.to_string())?;

    // The song: music/<pack id>/<file name>
    let name = Path::new(&song_file.path).file_name().and_then(|s| s.to_str()).unwrap_or("song.txt").to_string();
    let dest = ws.join("music").join(&pack.id);
    std::fs::create_dir_all(&dest).map_err(|e| e.to_string())?;
    std::fs::write(dest.join(&name), &text).map_err(|e| e.to_string())?;

    // Samples: the song names their folder with #path "X"; AddmusicK looks in samples/X.
    // Copy every folder of .brr files (and loose .brr files) from the pack there.
    let path_dir = text.lines().find_map(|l| l.trim().strip_prefix("#path").map(|r| r.trim().trim_matches('"').to_string()));
    let mut brr_dirs: Vec<String> = pack
        .files
        .iter()
        .filter(|f| f.path.to_lowercase().ends_with(".brr"))
        .map(|f| f.path.rsplit_once('/').map(|(d, _)| d.to_string()).unwrap_or_default())
        .collect();
    brr_dirs.sort();
    brr_dirs.dedup();
    for d in &brr_dirs {
        // Folder name under samples/: the song's #path when there's one folder, else the
        // folder's own name
        let last = d.rsplit('/').next().unwrap_or("").to_string();
        let target = match (&path_dir, brr_dirs.len()) {
            (Some(p), 1) => p.clone(),
            _ if !last.is_empty() => last,
            _ => path_dir.clone().unwrap_or_else(|| pack.id.clone()),
        };
        let to = ws.join("samples").join(&target);
        std::fs::create_dir_all(&to).map_err(|e| e.to_string())?;
        for f in pack.files.iter().filter(|f| f.path.to_lowercase().ends_with(".brr")) {
            let dir = f.path.rsplit_once('/').map(|(d, _)| d).unwrap_or("");
            if dir == d {
                let file = f.path.rsplit('/').next().unwrap();
                std::fs::copy(src.join(&f.path), to.join(file)).map_err(|e| e.to_string())?;
            }
        }
    }

    let (title, author) = song_info(&text);
    let mut list = added(&ws);
    let current = songs(rom);
    // Adding a song that's already in keeps its number (it's updated, not added twice)
    let existing = list.iter().find(|s| s.pack.as_deref() == Some(pack.id.as_str()) && s.number >= FIRST_CUSTOM).map(|s| s.number);
    let number = match replace.or(existing) {
        Some(n) => n,
        None => (FIRST_CUSTOM..=0xFF).find(|n| !current.iter().any(|s| s.number == *n)).ok_or("No free song numbers left")?,
    };
    // Replacing one of the game's songs keeps the original in the pool: it moves to the
    // next free number (once), so it can still be picked for levels
    if let Some(old) = current.iter().find(|s| s.number == number && s.pack.is_none()) {
        if !current.iter().chain(list.iter()).any(|s| s.number != number && s.file == old.file) {
            let free = (FIRST_CUSTOM..=0xFF).find(|n| !current.iter().any(|s| s.number == *n) && !list.iter().any(|s| s.number == *n)).ok_or("No free song numbers left")?;
            list.push(Song { number: free, ..old.clone() });
        }
    }
    list.retain(|s| s.number != number);
    list.push(Song {
        number,
        title: title.unwrap_or_else(|| pack.name.clone()),
        author: author.unwrap_or_else(|| pack.creator.clone()),
        file: format!("{}/{}", pack.id, name),
        pack: Some(pack.id.clone()),
    });
    list.sort_by_key(|s| s.number);
    std::fs::write(manifest_path(&ws), serde_json::to_vec_pretty(&list).unwrap()).map_err(|e| e.to_string())?;
    match insert(rom) {
        Ok(report) => Ok((number, report)),
        Err(e) => {
            // Leave the list as it was so a broken song doesn't block the next insert
            list.retain(|s| s.number != number);
            let _ = std::fs::write(manifest_path(&ws), serde_json::to_vec_pretty(&list).unwrap());
            Err(e)
        }
    }
}

/// Undo a replacement or remove a new song, then reinsert.
pub fn remove(rom: &Path, number: u8) -> Result<Vec<String>, String> {
    let ws = workspace(rom);
    let mut list = added(&ws);
    list.retain(|s| s.number != number);
    std::fs::write(manifest_path(&ws), serde_json::to_vec_pretty(&list).unwrap()).map_err(|e| e.to_string())?;
    insert(rom)
}

/// A copy of `rom` with just this pack's song inserted and `level` playing it, for hearing
/// a song before it goes in the real ROM. Works in a scratch folder; `rom` isn't touched.
pub fn preview_rom(rom: &Path, library: &Path, pack: &Pack, level: u16) -> Result<Vec<u8>, String> {
    let dir = std::env::temp_dir().join("smw-studio-preview");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let copy = dir.join("preview.sfc");
    std::fs::copy(rom, &copy).map_err(|e| e.to_string())?;
    // Only this song: forget the previous preview's
    let _ = std::fs::remove_file(manifest_path(&workspace(&copy)));
    let (n, _) = add_from_pack(&copy, library, pack, None, None)?;
    let mut p = crate::project::Project::open(copy)?;
    p.set_level_song(level, n)?;
    p.build_play_rom(level)
}

/// The original game's level songs as AddmusicK builds them, with their names: AddmusicK
/// runs once on SMW Studio's clean copy of the game, and the result is kept.
pub fn original_songs() -> Result<Vec<(Song, crate::amk::RomSong)>, String> {
    let clean = crate::lunar::remembered_clean_rom().filter(|p| p.exists()).ok_or("Open a clean Super Mario World ROM once first, so SMW Studio has the original songs to compare with.")?;
    let dir = clean.parent().ok_or("bad path")?.join("amk-originals");
    let built = dir.join("originals.smc");
    let read = |p: &Path| crate::project::Project::open(p.to_path_buf()).ok().and_then(|p| p.amk_songs());
    let songs_in = match read(&built) {
        Some(s) => s,
        None => {
            std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
            std::fs::copy(&clean, &built).map_err(|e| e.to_string())?;
            // AddmusicK needs room: expand the copy with Lunar Magic first
            let lm = crate::lunar::Lunar::find().ok_or("Lunar Magic is needed to build the original songs once")?;
            crate::lunar::ensure_restore_original(&built)?;
            lm.run(&["-ExpandROM", &built.to_string_lossy(), "2MB"])?;
            insert(&built)?;
            read(&built).ok_or("AddmusicK didn't put the original songs in")?
        }
    };
    let names = songs(&built);
    Ok(songs_in
        .into_iter()
        .filter_map(|s| names.iter().find(|n| n.number == s.number && n.pack.is_none()).map(|n| (n.clone(), s)))
        .collect())
}

/// Where one of a ROM's songs comes from.
#[derive(Debug, Clone, Serialize)]
pub struct SongOrigin {
    pub number: u8,
    pub bytes:  usize,
    /// "original", "edited" (an original with changes) or "custom"
    pub kind:   &'static str,
    /// The original song it is or is based on
    pub like:   Option<String>,
    /// How alike, 0-100
    pub match_percent: u8,
}

/// Compare each of a ROM's songs with the original game's.
pub fn classify(rom: &[crate::amk::RomSong], originals: &[(Song, crate::amk::RomSong)]) -> Vec<SongOrigin> {
    rom.iter()
        .map(|s| {
            let best = originals.iter().map(|(n, o)| (n, crate::amk::similarity(s, o))).max_by(|a, b| a.1.total_cmp(&b.1));
            let (like, sim) = best.map(|(n, v)| (Some(n.title.clone()), v)).unwrap_or((None, 0.0));
            // Unrelated songs share under 10% of their runs; edits of one song keep most
            let kind = if sim >= 0.97 { "original" } else if sim >= 0.4 { "edited" } else { "custom" };
            SongOrigin { number: s.number, bytes: s.data.len(), kind, like: if kind == "custom" { None } else { like }, match_percent: (sim * 100.0).round() as u8 }
        })
        .collect()
}
