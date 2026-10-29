//! Toolbox commands: the asset library and using packs in the open ROM.

use base64::Engine;
use serde::Serialize;
use smw_core::{
    lunar::{self, Action, Lunar},
    project::Project,
    toolbox::{self, FileKind, Pack},
};
use tauri::State;

use crate::Studio;

#[derive(Serialize)]
pub struct PackView {
    id:      String,
    name:    String,
    creator: String,
    kind:    String,
    notes:   Vec<String>,
    links:   Vec<String>,
    files:   usize,
    /// Sample levels (.mwl) in the pack, for "Add as level"
    levels:  Vec<String>,
    /// data: URL of the preview ("" if none)
    preview: String,
    /// What the app can do with it right now: "add_level", "replace_graphics"
    actions: Vec<&'static str>,
}

fn view(p: &Pack) -> PackView {
    let png = std::fs::read(toolbox::default_dir().join(&p.id).join("preview.png")).unwrap_or_default();
    let levels: Vec<String> = p.files_of(|k| *k == FileKind::Level).map(|f| f.path.clone()).collect();
    let mut actions = Vec::new();
    if !levels.is_empty() {
        actions.push("add_level");
    }
    if p.kind == "Graphics replacement" {
        actions.push("replace_graphics");
    }
    if p.files_of(|k| *k == FileKind::Music).next().is_some() {
        actions.push("add_song");
    }
    PackView {
        id: p.id.clone(),
        name: toolbox::tidy_name(&p.name),
        creator: p.creator.clone(),
        kind: p.kind.clone(),
        notes: p.notes.clone(),
        links: p.links.clone(),
        files: p.files.len(),
        levels,
        preview: if png.is_empty() { String::new() } else { format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(png)) },
        actions,
    }
}

#[tauri::command]
pub fn toolbox_list() -> Vec<PackView> {
    toolbox::list(&toolbox::default_dir()).iter().map(view).collect()
}

#[tauri::command]
pub async fn toolbox_import(paths: Vec<String>) -> Result<Vec<PackView>, String> {
    let lib = toolbox::default_dir();
    std::fs::create_dir_all(&lib).map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for p in paths {
        out.push(view(&toolbox::import_zip(&lib, std::path::Path::new(&p))?));
    }
    Ok(out)
}

/// Import a zip dropped onto the window (the page gets the file's bytes, not its path).
#[tauri::command]
pub async fn toolbox_import_bytes(name: String, data: Vec<u8>) -> Result<PackView, String> {
    let lib = toolbox::default_dir();
    std::fs::create_dir_all(&lib).map_err(|e| e.to_string())?;
    let safe: String = name.chars().filter(|c| !matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|')).collect();
    let tmp = std::env::temp_dir().join("smw-studio-drop");
    std::fs::create_dir_all(&tmp).map_err(|e| e.to_string())?;
    let path = tmp.join(if safe.is_empty() { "pack.zip".into() } else { safe });
    std::fs::write(&path, data).map_err(|e| e.to_string())?;
    let pack = toolbox::import_zip(&lib, &path);
    let _ = std::fs::remove_file(&path);
    Ok(view(&pack?))
}

/// Turn MIDI files (one per instrument, or one with several tracks) into a music pack in
/// the Toolbox. Returns the pack and which parts were left out (the SNES has 8 voices).
#[tauri::command]
pub async fn toolbox_import_midi(paths: Vec<String>) -> Result<(PackView, Vec<String>), String> {
    use std::io::Write;
    let lib = toolbox::default_dir();
    std::fs::create_dir_all(&lib).map_err(|e| e.to_string())?;
    let files: Vec<&std::path::Path> = paths.iter().map(std::path::Path::new).collect();
    let song = smw_core::midi::read(&files)?;
    let (text, dropped) = smw_core::midi::to_amk(&song, "");
    let safe: String = song.title.chars().filter(|c| !matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|')).collect();
    let safe = if safe.trim().is_empty() { "MIDI song".to_string() } else { safe.trim().to_string() };
    let tmp = std::env::temp_dir().join("smw-studio-midi");
    std::fs::create_dir_all(&tmp).map_err(|e| e.to_string())?;
    let zip_path = tmp.join(format!("{safe}.zip"));
    {
        // The song, plus the MIDI files and the arrangement so the Music tab can rebuild it
        let f = std::fs::File::create(&zip_path).map_err(|e| e.to_string())?;
        let mut z = zip::ZipWriter::new(f);
        let opts = zip::write::SimpleFileOptions::default();
        z.start_file(format!("{safe}.txt"), opts).map_err(|e| e.to_string())?;
        z.write_all(text.as_bytes()).map_err(|e| e.to_string())?;
        for p in &files {
            let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("part.mid");
            z.start_file(format!("{MIDI_DIR}/{name}"), opts).map_err(|e| e.to_string())?;
            z.write_all(&std::fs::read(p).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
        }
        z.start_file(ARRANGEMENT, opts).map_err(|e| e.to_string())?;
        z.write_all(&serde_json::to_vec_pretty(&smw_core::midi::default_arrangement(&song)).unwrap()).map_err(|e| e.to_string())?;
        z.finish().map_err(|e| e.to_string())?;
    }
    let pack = toolbox::import_zip(&lib, &zip_path);
    let _ = std::fs::remove_file(&zip_path);
    Ok((view(&pack?), dropped))
}

/// Inside a music pack made from MIDI: the MIDI files and the arrangement
const MIDI_DIR: &str = "midi";
const ARRANGEMENT: &str = "arrangement.smwmusic";

/// What the Music tab shows for a pack made from MIDI.
#[derive(serde::Serialize)]
pub struct MusicProject {
    title:       String,
    bpm:         f64,
    /// Per part: name, drums, notes as [start, length, key, velocity] (48 ticks a beat)
    parts:       Vec<(String, bool, Vec<(u64, u64, u8, u8)>)>,
    settings:    Vec<smw_core::midi::Setting>,
    instruments: Vec<(u8, String)>,
    channels:    usize,
    /// The song number it has in the open ROM, if it's been added
    in_rom:      Option<u8>,
}

fn project_song(id: &str) -> Result<(std::path::PathBuf, smw_core::midi::Song), String> {
    let dir = toolbox::files_dir(&toolbox::default_dir(), id);
    let mut mids: Vec<std::path::PathBuf> = std::fs::read_dir(dir.join(MIDI_DIR))
        .map_err(|_| "This song wasn't made from MIDI files in SMW Studio, so there's nothing to arrange".to_string())?
        .flatten()
        .map(|e| e.path())
        .collect();
    mids.sort();
    let refs: Vec<&std::path::Path> = mids.iter().map(|p| p.as_path()).collect();
    Ok((dir.clone(), smw_core::midi::read(&refs)?))
}

#[tauri::command]
pub fn music_project(id: String, studio: State<'_, Studio>) -> Result<MusicProject, String> {
    let (dir, song) = project_song(&id)?;
    let settings = std::fs::read(dir.join(ARRANGEMENT))
        .ok()
        .and_then(|b| serde_json::from_slice::<Vec<smw_core::midi::Setting>>(&b).ok())
        .filter(|s| s.len() == song.parts.len())
        .unwrap_or_else(|| smw_core::midi::default_arrangement(&song));
    let in_rom = studio.project.lock().unwrap().as_ref().and_then(|p| smw_core::music::songs(&p.path).into_iter().find(|s| s.pack.as_deref() == Some(id.as_str())).map(|s| s.number));
    Ok(MusicProject {
        title: song.title.clone(),
        bpm: song.bpm,
        parts: song.parts.iter().map(|p| (p.name.clone(), p.drums, p.notes.clone())).collect(),
        settings,
        instruments: smw_core::midi::INSTRUMENTS.iter().map(|(n, s)| (*n, s.to_string())).collect(),
        channels: smw_core::midi::CHANNELS,
        in_rom,
    })
}

/// Save an arrangement and rebuild the pack's song from it.
#[tauri::command]
pub fn music_project_save(id: String, settings: Vec<smw_core::midi::Setting>) -> Result<(), String> {
    let (dir, song) = project_song(&id)?;
    if settings.len() != song.parts.len() {
        return Err("The arrangement doesn't match the song's parts".into());
    }
    if settings.iter().filter(|s| s.keep).count() > smw_core::midi::CHANNELS {
        return Err(format!("The SNES can only play {} parts at once: turn one off first", smw_core::midi::CHANNELS));
    }
    let pack = toolbox::get(&toolbox::default_dir(), &id)?;
    let txt = pack.files_of(|k| *k == FileKind::Music).next().ok_or("This pack has no song file")?;
    let text = smw_core::midi::to_amk_with(&song, &settings, &pack.creator);
    std::fs::write(dir.join(&txt.path), text).map_err(|e| e.to_string())?;
    std::fs::write(dir.join(ARRANGEMENT), serde_json::to_vec_pretty(&settings).unwrap()).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn toolbox_remove(id: String) -> Result<(), String> {
    toolbox::remove(&toolbox::default_dir(), &id)
}

#[tauri::command]
pub fn toolbox_set_creator(id: String, creator: String) -> Result<(), String> {
    let lib = toolbox::default_dir();
    let mut p = toolbox::get(&lib, &id)?;
    p.creator = creator.trim().to_string();
    toolbox::save(&lib, &p)
}

#[tauri::command]
pub fn has_lunar_magic() -> bool {
    Lunar::find().is_some()
}

/// Use a pack in the open ROM. Unsaved edits are saved first (Lunar Magic works on the
/// file), then the ROM is reopened so the editor shows the result.
#[tauri::command]
pub async fn toolbox_apply(id: String, action: Action, studio: State<'_, Studio>) -> Result<Vec<String>, String> {
    let lm = Lunar::find().ok_or("Lunar Magic wasn't found (SMW Studio uses it to insert graphics and levels)")?;
    let lib = toolbox::default_dir();
    let pack = toolbox::get(&lib, &id)?;
    let path = {
        let mut g = studio.project.lock().unwrap();
        let p = g.as_mut().ok_or("Open a ROM first")?;
        let mut report = Vec::new();
        if p.edits.values().any(|e| e.dirty) {
            report.extend(p.save()?);
        }
        p.path.clone()
    };
    let report = lunar::apply(&lm, &path, &lib, &pack, &action)?;
    *studio.project.lock().unwrap() = Some(Project::open(path)?);
    Ok(report)
}

/// Songs of the open ROM (the game's own, replacements and new ones), whether levels can
/// pick any of them, and whether AddmusicK is available.
#[derive(serde::Serialize)]
pub struct MusicView {
    songs:     Vec<smw_core::music::Song>,
    any_song:  bool,
    available: bool,
}

#[tauri::command]
pub fn music_songs(studio: State<'_, Studio>) -> Result<MusicView, String> {
    let g = studio.project.lock().unwrap();
    let p = g.as_ref().ok_or("Open a ROM first")?;
    Ok(MusicView {
        songs:     smw_core::music::songs(&p.path),
        any_song:  p.can_pick_any_song(),
        available: smw_core::music::amk_source().is_some(),
    })
}

/// Put a music pack's song in the ROM: as a new song, or replacing song `replace`.
/// Unsaved edits are saved first (AddmusicK works on the file), then the ROM is reopened.
#[tauri::command]
pub async fn music_add(id: String, file: Option<String>, replace: Option<u8>, studio: State<'_, Studio>) -> Result<(u8, Vec<String>), String> {
    let lib = toolbox::default_dir();
    let pack = toolbox::get(&lib, &id)?;
    let path = save_first(&studio)?;
    let out = smw_core::music::add_from_pack(&path, &lib, &pack, file.as_deref(), replace)?;
    *studio.project.lock().unwrap() = Some(Project::open(path)?);
    Ok(out)
}

/// Remove a new song, or put back the game's own song where one was replaced.
#[tauri::command]
pub async fn music_remove(number: u8, studio: State<'_, Studio>) -> Result<Vec<String>, String> {
    let path = save_first(&studio)?;
    let out = smw_core::music::remove(&path, number)?;
    *studio.project.lock().unwrap() = Some(Project::open(path)?);
    Ok(out)
}

fn save_first(studio: &State<'_, Studio>) -> Result<std::path::PathBuf, String> {
    let mut g = studio.project.lock().unwrap();
    let p = g.as_mut().ok_or("Open a ROM first")?;
    if p.edits.values().any(|e| e.dirty) {
        p.save()?;
    }
    Ok(p.path.clone())
}

/// Hear a music pack's song: plays `level` from a scratch copy of the ROM with the song in
/// it (sound only matters; frames go to the channel like normal play).
#[tauri::command]
pub async fn music_preview(
    id: String,
    level: u16,
    frames: tauri::ipc::Channel<tauri::ipc::InvokeResponseBody>,
    events: tauri::ipc::Channel<String>,
    studio: State<'_, Studio>,
    player: State<'_, crate::play::Player>,
) -> Result<(), String> {
    let lib = toolbox::default_dir();
    let pack = toolbox::get(&lib, &id)?;
    let path = studio.project.lock().unwrap().as_ref().ok_or("Open a ROM first")?.path.clone();
    let rom = smw_core::music::preview_rom(&path, &lib, &pack, level)?;
    let options = smw_play::Options { infinite_lives: true, invincible: true, infinite_time: true, powerup: -1, midway: false, song: None };
    player.start(rom, options, frames, events);
    Ok(())
}

/// Hear one of the ROM's own songs: play a level that uses it (or level 105) and ask the game
/// for that song. Only the sound is used; the ROM isn't changed.
#[tauri::command]
pub async fn rom_song_play(
    number: u8,
    level: Option<u16>,
    frames: tauri::ipc::Channel<tauri::ipc::InvokeResponseBody>,
    events: tauri::ipc::Channel<String>,
    studio: State<'_, Studio>,
    player: State<'_, crate::play::Player>,
) -> Result<(), String> {
    let rom = studio.project.lock().unwrap().as_ref().ok_or("Open a ROM first")?.build_play_rom(level.unwrap_or(0x105))?;
    let options = smw_play::Options { infinite_lives: true, invincible: true, infinite_time: true, powerup: -1, midway: false, song: Some(number) };
    player.start(rom, options, frames, events);
    Ok(())
}

/// A hack opened for taking parts from: its name and the levels it has.
#[derive(serde::Serialize)]
pub struct HackView {
    name:   String,
    levels: Vec<u16>,
}

/// Open a hack (exports it the first time; quick after that).
#[tauri::command]
pub async fn hack_open(path: String) -> Result<HackView, String> {
    let lm = Lunar::find().ok_or("Lunar Magic wasn't found (SMW Studio uses it to take hacks apart)")?;
    let h = smw_core::hackparts::open(&lm, std::path::Path::new(&path))?;
    Ok(HackView { name: h.name, levels: h.levels })
}

/// A preview picture of one of the hack's levels (data: URL).
#[tauri::command]
pub async fn hack_preview(path: String, level: u16) -> Result<String, String> {
    let lm = Lunar::find().ok_or("Lunar Magic wasn't found")?;
    let h = smw_core::hackparts::open(&lm, std::path::Path::new(&path))?;
    let png = smw_core::hackparts::preview(&lm, &h, level)?;
    Ok(format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(png)))
}

/// Copy one of the hack's levels into the open ROM as `as_level`. Saves unsaved edits first,
/// then reopens the ROM.
#[tauri::command]
pub async fn hack_bring(path: String, level: u16, as_level: u16, studio: State<'_, Studio>) -> Result<Vec<String>, String> {
    let lm = Lunar::find().ok_or("Lunar Magic wasn't found")?;
    let h = smw_core::hackparts::open(&lm, std::path::Path::new(&path))?;
    let rom = save_first(&studio)?;
    let report = smw_core::hackparts::bring_level(&lm, &h, level, &rom, as_level)?;
    *studio.project.lock().unwrap() = Some(Project::open(rom)?);
    Ok(report)
}

#[derive(Serialize)]
pub struct RomSongView {
    #[serde(flatten)]
    origin: smw_core::music::SongOrigin,
    /// A name to show: the original's, or "<game> – <first level using it>"
    name:   String,
    /// Levels (with names on the map) that play it
    levels: Vec<u16>,
}

/// The songs in the open ROM (AddmusicK ROMs), each compared with the original game's
/// songs, with the levels that play it. Takes a few seconds: it loads every map level.
#[tauri::command]
pub async fn rom_songs(studio: State<'_, Studio>) -> Result<Vec<RomSongView>, String> {
    let p = studio.project.lock().unwrap().as_ref().ok_or("Open a ROM first")?.clone();
    let songs = p.amk_songs().ok_or("This ROM has no AddmusicK music: its songs are the original game's.")?;
    let originals = smw_core::music::original_songs()?;
    let names: std::collections::HashMap<u16, String> = p.level_names().into_iter().collect();
    let game = p.path.file_stem().and_then(|s| s.to_str()).unwrap_or("This ROM");
    // "My Hack v1.2" -> "My Hack"
    let game = match game.rsplit_once(' ') {
        Some((head, v)) if v.starts_with(['v', 'V']) && v[1..].chars().all(|c| c.is_ascii_digit() || c == '.') => head,
        _ => game,
    };
    let map_levels: Vec<u16> = (0..0x60u16).map(smw_core::names::level_of).collect();
    let used: Vec<(u16, u8)> = map_levels.iter().map(|&l| (l, p.level_music(l))).collect();
    Ok(smw_core::music::classify(&songs, &originals)
        .into_iter()
        .map(|origin| {
            let levels: Vec<u16> = used.iter().filter(|(_, s)| *s == origin.number).map(|(l, _)| *l).collect();
            let place = levels.first().map(|l| names.get(l).cloned().unwrap_or_else(|| format!("level {l:03X}")));
            let name = match (&origin.like, origin.kind, place) {
                (Some(like), "original", _) => like.clone(),
                (Some(like), _, Some(at)) => format!("{game} – {at} ({like} remix)"),
                (Some(like), _, None) => format!("{game} – {like} remix"),
                (None, _, Some(at)) => format!("{game} – {at}"),
                (None, _, None) => format!("{game} – song {:02X}", origin.number),
            };
            RomSongView { origin, name, levels }
        })
        .collect())
}

#[derive(Serialize)]
pub struct PatchView {
    id:       String,
    name:     String,
    on:       bool,
    settings: Vec<smw_core::patches::Setting>,
    /// Files it came with that aren't applied (e.g. UberASM code)
    other:    Vec<String>,
}

/// Every imported patch, whether it's in the open ROM, and its settings (as applied, or the
/// patch's own with free RAM filled in).
#[tauri::command]
pub fn patches_list(studio: State<'_, Studio>) -> Result<Vec<PatchView>, String> {
    let rom = studio.project.lock().unwrap().as_ref().ok_or("Open a ROM first")?.path.clone();
    let applied = smw_core::patches::applied(&rom);
    Ok(smw_core::patches::library()
        .into_iter()
        .map(|p| {
            let a = applied.iter().find(|a| a.name == p.name);
            let settings = match a {
                Some(a) => a.settings.clone(),
                None => {
                    let asm = std::fs::read_to_string(smw_core::patches::main_path(&p)).unwrap_or_default();
                    let mut s = smw_core::patches::settings(&asm);
                    smw_core::patches::assign_free_ram(&mut s, &rom, smw_core::patches::is_frame_code(&asm));
                    s
                }
            };
            PatchView { id: p.id, name: p.name, on: a.is_some(), settings, other: p.other }
        })
        .collect())
}

/// Remove a patch from the library (not from ROMs that have it).
#[tauri::command]
pub fn patch_forget(id: String, studio: State<'_, Studio>) -> Result<(), String> {
    let p = smw_core::patches::library().into_iter().find(|p| p.id == id).ok_or("No such patch")?;
    if let Some(rom) = studio.project.lock().unwrap().as_ref().map(|p| p.path.clone()) {
        if smw_core::patches::applied(&rom).iter().any(|a| a.name == p.name) {
            return Err(format!("{} is on in this ROM: turn it off first", p.name));
        }
    }
    std::fs::remove_dir_all(smw_core::patches::library_dir().join(&p.id)).map_err(|e| e.to_string())
}

/// Add a patch (.asm or .zip) to the library.
#[tauri::command]
pub fn patch_import(path: String) -> Result<String, String> {
    Ok(smw_core::patches::import(std::path::Path::new(&path))?.name)
}

/// Put a patch in the open ROM (with these settings) or take it out. Unsaved edits are saved
/// first (Asar works on the file), then the ROM is reopened.
#[tauri::command]
pub async fn patch_set(id: String, on: bool, settings: Vec<smw_core::patches::Setting>, studio: State<'_, Studio>) -> Result<Vec<String>, String> {
    let p = smw_core::patches::library().into_iter().find(|p| p.id == id).ok_or("No such patch")?;
    let path = save_first(&studio)?;
    let frame_of = |path: &std::path::Path| smw_core::patches::applied(path).iter().find(|a| a.name == p.name).and_then(|a| a.frame);
    let out = if on {
        let out = smw_core::patches::apply(&path, &p.name, &smw_core::patches::main_path(&p), &settings)?;
        // Per-frame code: call it every frame in levels
        if let Some(frame) = frame_of(&path) {
            let hooked = (|| {
                let mut proj = Project::open(path.clone())?;
                let mut calls = smw_core::framehook::frame_calls(&proj);
                calls.push(frame);
                smw_core::framehook::set_frame_calls(&mut proj, &calls)?;
                proj.save().map(|_| ())
            })();
            if let Err(e) = hooked {
                // Couldn't hook it in: take the code back out
                let _ = smw_core::patches::remove(&path, &p.name);
                return Err(e);
            }
        }
        out
    } else {
        if let Some(frame) = frame_of(&path) {
            let mut proj = Project::open(path.clone())?;
            let calls: Vec<u32> = smw_core::framehook::frame_calls(&proj).into_iter().filter(|&c| c != frame).collect();
            smw_core::framehook::set_frame_calls(&mut proj, &calls)?;
            proj.save()?;
        }
        smw_core::patches::remove(&path, &p.name)?;
        vec![]
    };
    *studio.project.lock().unwrap() = Some(Project::open(path)?);
    Ok(out)
}
