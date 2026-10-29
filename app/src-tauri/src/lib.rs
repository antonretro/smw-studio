mod play;
mod testplay;
mod toolbox_cmds;

use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use smw_core::{
    level_data::{Entry, Kind},
    project::Project,
    romfile::RomKind,
};
use tauri::{ipc::Response, State};

#[derive(Default)]
pub(crate) struct Studio {
    pub(crate) project: Mutex<Option<Project>>,
}

fn with<T>(studio: &State<Studio>, f: impl FnOnce(&mut Project) -> Result<T, String>) -> Result<T, String> {
    let mut guard = studio.project.lock().unwrap();
    f(guard.as_mut().ok_or("No ROM loaded")?)
}

#[derive(Serialize)]
struct RomSummary {
    path:       String,
    kind:       String,
    title:      String,
    sha1:       String,
    size:       usize,
    had_header: bool,
    /// The ROM's own level names (where they differ from the original game's), by level
    level_names: std::collections::HashMap<u16, String>,
}

#[derive(Serialize)]
struct EntryView {
    index:    usize,
    kind:     &'static str,
    id:       u8,
    settings: u8,
    x:        u32,
    y:        u32,
}

fn entry_view(index: usize, e: &Entry) -> EntryView {
    let kind = match e.kind {
        Kind::Standard => "object",
        Kind::Extended => "extended",
        Kind::Exit => "exit",
    };
    EntryView { index, kind, id: e.id, settings: e.settings, x: e.x, y: e.y }
}

#[tauri::command]
fn open_rom(path: String, studio: State<Studio>) -> Result<RomSummary, String> {
    let project = Project::open(path.clone().into())?;
    let i = &project.info;
    // Lunar Magic needs a clean copy of the game later on (see lunar::ensure_restore_original)
    if i.kind == RomKind::CleanUsa {
        if let Ok(bytes) = std::fs::read(&path) {
            smw_core::lunar::remember_clean_rom(&smw_core::romfile::strip_header(bytes).0);
        }
    }
    let kind = match i.kind {
        RomKind::CleanUsa => "Clean Super Mario World (USA)",
        RomKind::Modified { lunar_magic: true } if project.is_sa1() => "SA-1 hack (edited with Lunar Magic)",
        RomKind::Modified { lunar_magic: true } => "Modified SMW (edited with Lunar Magic)",
        RomKind::Modified { lunar_magic: false } => "Modified SMW",
        RomKind::NotSmw => "Not SMW",
    }
    .to_string();
    let summary =
        RomSummary { path, kind, title: i.title.clone(), sha1: i.sha1.clone(), size: i.size, had_header: i.had_header, level_names: project.level_names().into_iter().collect() };
    *studio.project.lock().unwrap() = Some(project);
    Ok(summary)
}

/// Raw bytes: [w u32][h u32][screens u8][vertical u8][mode u8][sprites u8][cols u32][rows u32]
/// then RGBA (w*h*4), then per-block owner entry index + 1 (cols*rows u16 LE).
#[tauri::command]
fn render_level(level: u16, sprites: bool, studio: State<Studio>) -> Result<Response, String> {
    with(&studio, |p| {
        let r = p.render(level, sprites)?;
        let img = &r.image;
        let mut out = Vec::with_capacity(20 + img.rgba.len() * 3 + r.owner.len() * 2);
        out.extend_from_slice(&img.width.to_le_bytes());
        out.extend_from_slice(&img.height.to_le_bytes());
        out.extend_from_slice(&[r.info.screens as u8, r.info.vertical as u8, r.info.mode, r.sprites.min(255) as u8]);
        out.extend_from_slice(&r.cols.to_le_bytes());
        out.extend_from_slice(&r.rows.to_le_bytes());
        // Three layers, same size: back (background / layer 2), layer 1, sprites
        out.extend_from_slice(&r.back.rgba);
        out.extend_from_slice(&r.front.rgba);
        out.extend_from_slice(&r.sprite_layer.rgba);
        for o in &r.owner {
            out.extend_from_slice(&o.to_le_bytes());
        }
        // Sprite boxes (x, y, w, h as i32), in sprite list order
        out.extend_from_slice(&(r.sprite_boxes.len() as u32).to_le_bytes());
        for b in &r.sprite_boxes {
            for v in b {
                out.extend_from_slice(&v.to_le_bytes());
            }
        }
        Ok(Response::new(out))
    })
}

#[derive(Serialize)]
struct StatusBarView {
    /// Why it can't be changed in this ROM ("" if it can)
    problem: String,
    parts:   Vec<(&'static str, &'static str, bool)>,
    /// The status bar as the game shows it (PNG data URL, 256 x 32)
    picture: String,
    /// Where each part is, as (id, [row, first column, last column]) in 8x8 cells
    cells:   Vec<(&'static str, Vec<(u32, u32, u32)>)>,
}

/// The status bar: a picture (with level `level`'s graphics) and which parts are hidden.
#[tauri::command]
fn statusbar_view(level: u16, studio: State<Studio>) -> Result<StatusBarView, String> {
    use base64::Engine;
    with(&studio, |p| {
        use smw_core::statusbar as sb;
        let hidden = sb::hidden(p);
        let png = sb::picture(p, level).png();
        Ok(StatusBarView {
            problem: sb::supported(p).err().unwrap_or_default(),
            parts:   sb::PARTS.iter().map(|x| (x.id, x.name, hidden.contains(&x.id))).collect(),
            picture: format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(png)),
            cells:   sb::layout(p).into_iter().map(|(id, c)| (id, c.into_iter().map(|(r, a, b)| (r - 1, a, b)).collect())).collect(),
        })
    })
}

/// Hide exactly these parts of the status bar (saved with the next save).
#[tauri::command]
fn statusbar_set(hidden: Vec<String>, studio: State<Studio>) -> Result<(), String> {
    with(&studio, |p| smw_core::statusbar::set_hidden(p, &hidden))
}

#[derive(Serialize)]
struct PresetView {
    id:      &'static str,
    name:    &'static str,
    about:   &'static str,
    picture: String,
}

/// The ready-made status bars, each pictured in this ROM (nothing is changed).
#[tauri::command]
async fn statusbar_presets(level: u16, studio: State<'_, Studio>) -> Result<Vec<PresetView>, String> {
    use base64::Engine;
    let p = studio.project.lock().unwrap().as_ref().ok_or("No ROM loaded")?.clone();
    Ok(smw_core::statusbar::preset_pictures(&p, level)
        .into_iter()
        .map(|(id, name, about, img)| PresetView { id, name, about, picture: format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(img.png())) })
        .collect())
}

#[derive(Serialize)]
struct ElementView {
    id:      &'static str,
    name:    &'static str,
    width:   u32,
    picture: String,
}

/// The pieces that can be dropped onto the status bar, pictured in this ROM.
#[tauri::command]
async fn statusbar_elements(level: u16, studio: State<'_, Studio>) -> Result<Vec<ElementView>, String> {
    use base64::Engine;
    let p = studio.project.lock().unwrap().as_ref().ok_or("No ROM loaded")?.clone();
    Ok(smw_core::statusbar::element_pictures(&p, level)
        .into_iter()
        .map(|(id, name, img)| ElementView { id, name, width: img.width / 8, picture: format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(img.png())) })
        .collect())
}

/// Drop an element onto the status bar with its first square at (row, column).
#[tauri::command]
fn statusbar_place(id: String, row: u32, col: u32, studio: State<Studio>) -> Result<(), String> {
    with(&studio, |p| smw_core::statusbar::place_element(p, &id, row, col))
}

/// A picture of overworld map `submap` (0 main map, 1-6 the submaps), as the game draws it.
#[tauri::command]
async fn overworld_picture(submap: u8, studio: State<'_, Studio>) -> Result<String, String> {
    use base64::Engine;
    let p = studio.project.lock().unwrap().as_ref().ok_or("No ROM loaded")?.clone();
    if p.is_sa1() {
        return Err("SA-1 hacks' overworlds can't be shown yet".into());
    }
    let png = smw_core::overworld::submap_picture(&p, submap).png();
    Ok(format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(png)))
}

/// The sprites drawn on the status bar.
#[tauri::command]
fn statusbar_stickers(studio: State<Studio>) -> Result<Vec<smw_core::statusbar::Sticker>, String> {
    with(&studio, |p| Ok(smw_core::statusbar::stickers(p)))
}

/// Replace the sprites drawn on the status bar.
#[tauri::command]
fn statusbar_set_stickers(stickers: Vec<smw_core::statusbar::Sticker>, studio: State<Studio>) -> Result<(), String> {
    with(&studio, |p| smw_core::statusbar::set_stickers(p, &stickers))
}

/// The level's sprite tiles to pick from, in one set of sprite colours (PNG data URL).
#[tauri::command]
fn statusbar_sprite_tiles(level: u16, palette: u8, studio: State<Studio>) -> Result<String, String> {
    use base64::Engine;
    with(&studio, |p| {
        let png = smw_core::statusbar::sprite_tiles_picture(p, level, palette).png();
        Ok(format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(png)))
    })
}

/// Replace the whole status bar with a preset.
#[tauri::command]
fn statusbar_apply_preset(id: String, studio: State<Studio>) -> Result<(), String> {
    with(&studio, |p| smw_core::statusbar::apply_preset(p, &id))
}

/// Move a counter's first digit to (row, column).
#[tauri::command]
fn statusbar_move(id: String, row: u32, col: u32, studio: State<Studio>) -> Result<(), String> {
    with(&studio, |p| smw_core::statusbar::move_counter(p, &id, row, col))
}

/// Every cell of the status bar (rows 1-4) for the designer.
#[tauri::command]
fn statusbar_cells(studio: State<Studio>) -> Result<Vec<smw_core::statusbar::Cell>, String> {
    with(&studio, |p| Ok(smw_core::statusbar::cells(p)))
}

/// Change status bar cells: (row, column, tilemap entry) each.
#[tauri::command]
fn statusbar_set_cells(changes: Vec<(u32, u32, u16)>, studio: State<Studio>) -> Result<(), String> {
    with(&studio, |p| smw_core::statusbar::set_cells(p, &changes))
}

/// The layer 3 tiles to pick from, in one palette (PNG data URL, 128 x 256).
#[tauri::command]
fn statusbar_tiles(level: u16, palette: u16, studio: State<Studio>) -> Result<String, String> {
    use base64::Engine;
    with(&studio, |p| {
        let png = smw_core::statusbar::tiles_picture(p, level, palette).png();
        Ok(format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(png)))
    })
}

#[derive(Serialize)]
struct SpriteView {
    index: usize,
    id:    u8,
    name:  String,
    x:     u32,
    y:     u32,
    extra: u8,
}

#[derive(Serialize)]
struct EntrancesView {
    main:        smw_core::project::Entrance,
    secondaries: Vec<smw_core::project::SecondaryEntrance>,
    /// Pixel X within the screen for each of the 8 spots, pixel Y for each of the 16 heights
    xs:          [u16; 8],
    ys:          [u16; 16],
    /// The main entrance is on an exact 16x16 tile (newer Lunar Magic): x/y are tiles
    exact:       bool,
    /// Screen Mario restarts on after the midway gate
    midway:      u8,
}

#[tauri::command]
fn level_entrances(level: u16, studio: State<Studio>) -> Result<EntrancesView, String> {
    with(&studio, |p| {
        let (xs, ys) = p.entrance_positions();
        Ok(EntrancesView {
            main: p.main_entrance(level)?,
            secondaries: p.secondary_entrances(level)?,
            xs,
            ys,
            exact: p.main_entrance_exact(level)?,
            midway: p.midway_screen(level)?,
        })
    })
}

#[tauri::command]
fn level_song(level: u16, studio: State<Studio>) -> Result<u8, String> {
    with(&studio, |p| p.level_song(level))
}

#[tauri::command]
fn set_level_song(level: u16, song: u8, studio: State<Studio>) -> Result<(), String> {
    with(&studio, |p| p.set_level_song(level, song))
}

#[tauri::command]
fn set_midway_screen(level: u16, screen: u8, studio: State<Studio>) -> Result<(), String> {
    with(&studio, |p| p.set_midway_screen(level, screen))
}

/// Every secondary entrance in the ROM: [number, destination level]
#[tauri::command]
fn all_secondary_entrances(studio: State<Studio>) -> Result<Vec<(u16, u16)>, String> {
    with(&studio, |p| Ok(p.all_secondary_entrances()))
}

#[tauri::command]
fn add_secondary_entrance(level: u16, studio: State<Studio>) -> Result<u16, String> {
    with(&studio, |p| p.add_secondary_entrance(level))
}

#[tauri::command]
fn set_main_entrance(level: u16, entrance: smw_core::project::Entrance, studio: State<Studio>) -> Result<(), String> {
    with(&studio, |p| p.set_main_entrance(level, entrance))
}

#[tauri::command]
fn set_secondary_entrance(level: u16, number: u16, entrance: smw_core::project::Entrance, studio: State<Studio>) -> Result<(), String> {
    with(&studio, |p| p.set_secondary_entrance(level, number, entrance))
}

#[derive(Serialize)]
struct ExitsView {
    /// Lunar Magic ROM: each exit has its own secondary/water flags
    per_screen: bool,
    exits:      Vec<smw_core::project::ScreenExit>,
}

#[tauri::command]
fn level_exits(level: u16, studio: State<Studio>) -> Result<ExitsView, String> {
    with(&studio, |p| Ok(ExitsView { per_screen: p.per_screen_exit_flags(), exits: p.exits(level)? }))
}

#[tauri::command]
fn set_exit(level: u16, exit: smw_core::project::ScreenExit, studio: State<Studio>) -> Result<(), String> {
    with(&studio, |p| p.set_exit(level, exit))
}

#[tauri::command]
fn remove_exit(level: u16, screen: u8, studio: State<Studio>) -> Result<(), String> {
    with(&studio, |p| p.remove_exit(level, screen))
}

#[tauri::command]
fn level_sprites(level: u16, studio: State<Studio>) -> Result<Vec<SpriteView>, String> {
    with(&studio, |p| {
        Ok(p.sprites(level)?
            .sprites
            .iter()
            .enumerate()
            .map(|(index, s)| SpriteView { index, id: s.id, name: smw_core::sprite_data::name(s.id), x: s.x, y: s.y, extra: s.extra })
            .collect())
    })
}

#[tauri::command]
fn add_sprite(level: u16, id: u8, x: u32, y: u32, extra: u8, studio: State<Studio>) -> Result<usize, String> {
    with(&studio, |p| p.add_sprite(level, id, x, y, extra))
}

#[tauri::command]
fn move_sprite(level: u16, index: usize, x: u32, y: u32, studio: State<Studio>) -> Result<usize, String> {
    with(&studio, |p| p.move_sprite(level, index, x, y))
}

#[tauri::command]
fn set_sprite(level: u16, index: usize, id: u8, extra: u8, studio: State<Studio>) -> Result<(), String> {
    with(&studio, |p| p.set_sprite(level, index, id, extra))
}

#[tauri::command]
fn delete_sprite(level: u16, index: usize, studio: State<Studio>) -> Result<(), String> {
    with(&studio, |p| p.delete_sprite(level, index))
}

#[derive(Serialize)]
struct SpritePaletteView {
    id:   u8,
    name: String,
    w:    u32,
    h:    u32,
    /// data: URL ("" when the sprite draws nothing by itself)
    img:  String,
    /// Where the picture's top-left is relative to the sprite's position, in pixels
    ox:   i32,
    oy:   i32,
}

#[tauri::command]
async fn sprite_palette(level: u16, studio: State<'_, Studio>) -> Result<Vec<SpritePaletteView>, String> {
    use base64::Engine;
    with(&studio, |p| {
        Ok(p.sprite_palette(level)?
            .into_iter()
            .map(|i| {
                let (w, h, img) = match &i.preview {
                    Some(im) => (im.width, im.height, format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(im.png()))),
                    None => (16, 16, String::new()),
                };
                SpritePaletteView { id: i.id, name: i.name, w, h, img, ox: i.offset.0, oy: i.offset.1 }
            })
            .collect())
    })
}

#[tauri::command]
fn level_entries(level: u16, studio: State<Studio>) -> Result<Vec<EntryView>, String> {
    with(&studio, |p| Ok(p.level_data(level)?.entries.iter().enumerate().map(|(i, e)| entry_view(i, e)).collect()))
}

#[tauri::command]
fn move_entry(level: u16, index: usize, x: u32, y: u32, studio: State<Studio>) -> Result<usize, String> {
    with(&studio, |p| {
        let mut new_index = index;
        p.change(level, |d| new_index = d.move_entry(index, x, y))?;
        Ok(new_index)
    })
}

#[tauri::command]
fn delete_entry(level: u16, index: usize, studio: State<Studio>) -> Result<(), String> {
    with(&studio, |p| {
        p.change(level, |d| {
            if index < d.entries.len() {
                d.entries.remove(index);
            }
        })
    })
}

#[tauri::command]
fn duplicate_entry(level: u16, index: usize, x: u32, y: u32, studio: State<Studio>) -> Result<usize, String> {
    with(&studio, |p| {
        let mut new_index = index;
        p.change(level, |d| {
            let e = d.entries[index];
            d.entries.push(e);
            new_index = d.move_entry(d.entries.len() - 1, x, y);
        })?;
        Ok(new_index)
    })
}

#[derive(Serialize)]
struct HeaderView {
    bytes:   [u8; 5],
    /// Song number for each of the 8 music choices (from this ROM)
    music:   [u8; 8],
    /// Back area colours as #rrggbb
    colours: Vec<String>,
}

#[tauri::command]
fn level_header(level: u16, studio: State<Studio>) -> Result<HeaderView, String> {
    with(&studio, |p| {
        let bytes = p.header(level)?;
        let (music, cols) = p.header_tables();
        let colours = cols
            .iter()
            .map(|&c| {
                let ch = |v: u16| ((v & 31) << 3 | (v & 31) >> 2) as u8;
                format!("#{:02x}{:02x}{:02x}", ch(c), ch(c >> 5), ch(c >> 10))
            })
            .collect();
        Ok(HeaderView { bytes, music, colours })
    })
}

/// Change a level setting (bits `mask` of header byte `byte`).
#[tauri::command]
fn set_header_bits(level: u16, byte: usize, mask: u8, value: u8, studio: State<Studio>) -> Result<(), String> {
    with(&studio, |p| p.set_header_bits(level, byte, mask, value))
}

/// Make the level longer or shorter (number of screens).
#[tauri::command]
fn set_screens(level: u16, screens: u8, studio: State<Studio>) -> Result<(), String> {
    with(&studio, |p| p.set_screens(level, screens).map(|_| ()))
}

#[tauri::command]
fn set_entry(level: u16, index: usize, id: u8, settings: u8, studio: State<Studio>) -> Result<(), String> {
    with(&studio, |p| {
        p.change(level, |d| {
            if let Some(e) = d.entries.get_mut(index) {
                e.id = id;
                e.settings = settings;
            }
        })
    })
}

#[derive(Serialize)]
struct PaletteView {
    kind:     &'static str,
    id:       u8,
    settings: u8,
    name:     String,
    desc:     String,
    w:        u32,
    h:        u32,
    ox:       i32,
    oy:       i32,
    /// data: URL of the preview PNG ("" when the object draws nothing)
    img:      String,
}

#[tauri::command]
async fn object_palette(level: u16, studio: State<'_, Studio>) -> Result<Vec<PaletteView>, String> {
    use base64::Engine;
    with(&studio, |p| {
        Ok(p.palette(level)?
            .into_iter()
            .map(|i| PaletteView {
                kind:     i.kind,
                id:       i.id,
                settings: i.settings,
                name:     i.name,
                desc:     i.desc,
                w:        i.w,
                h:        i.h,
                ox:       i.ox,
                oy:       i.oy,
                img:      if i.png.is_empty() {
                    String::new()
                } else {
                    format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(&i.png))
                },
            })
            .collect())
    })
}

#[tauri::command]
fn add_entry(
    level: u16,
    kind: String,
    id: u8,
    settings: u8,
    x: u32,
    y: u32,
    studio: State<Studio>,
) -> Result<usize, String> {
    let kind = if kind == "extended" { Kind::Extended } else { Kind::Standard };
    with(&studio, |p| p.add_entry(level, kind, id, settings, x, y))
}

#[derive(Deserialize)]
struct NewEntry {
    kind:     String,
    id:       u8,
    settings: u8,
    x:        u32,
    y:        u32,
}

/// Place several objects at once (one undo step): Draw and Fill strokes.
#[tauri::command]
fn add_entries(level: u16, items: Vec<NewEntry>, studio: State<Studio>) -> Result<(), String> {
    let items: Vec<_> = items
        .iter()
        .map(|e| (if e.kind == "extended" { Kind::Extended } else { Kind::Standard }, e.id, e.settings, e.x, e.y))
        .collect();
    with(&studio, |p| p.add_entries(level, &items))
}

#[tauri::command]
fn delete_entries(level: u16, indices: Vec<usize>, studio: State<Studio>) -> Result<(), String> {
    with(&studio, |p| p.delete_entries(level, &indices))
}

/// Place several sprites at once: [id, x, y, extra] in blocks.
#[tauri::command]
fn add_sprites(level: u16, items: Vec<(u8, u32, u32, u8)>, studio: State<Studio>) -> Result<(), String> {
    with(&studio, |p| p.add_sprites(level, &items))
}

#[tauri::command]
fn delete_sprites(level: u16, indices: Vec<usize>, studio: State<Studio>) -> Result<(), String> {
    with(&studio, |p| p.delete_sprites(level, &indices))
}

#[tauri::command]
fn test_play(level: u16, options: testplay::TestOptions, studio: State<Studio>) -> Result<String, String> {
    let rom = with(&studio, |p| p.build_rom())?;
    testplay::launch(&rom, level, &options)
}

#[tauri::command]
fn play_start(
    level: u16,
    options: play::PlayOptions,
    frames: tauri::ipc::Channel<tauri::ipc::InvokeResponseBody>,
    events: tauri::ipc::Channel<String>,
    studio: State<Studio>,
    player: State<play::Player>,
) -> Result<(), String> {
    let rom = with(&studio, |p| p.build_play_rom(level))?;
    player.start(rom, options.into(), frames, events);
    Ok(())
}

#[tauri::command]
fn play_input(buttons: u16, player: State<play::Player>) {
    player.send(play::Cmd::Input(buttons));
}

#[tauri::command]
fn play_restart(player: State<play::Player>) {
    player.send(play::Cmd::Restart);
}

#[tauri::command]
fn play_stop(player: State<play::Player>) {
    player.stop();
}

/// Unused level numbers (safe to fill with a Toolbox level).
#[tauri::command]
fn free_levels(studio: State<Studio>) -> Result<Vec<u16>, String> {
    with(&studio, |p| Ok(p.free_levels()))
}

#[tauri::command]
fn has_emulator() -> bool {
    testplay::find_mesen().is_some()
}

#[tauri::command]
fn undo(level: u16, studio: State<Studio>) -> Result<bool, String> {
    with(&studio, |p| p.undo(level))
}

#[tauri::command]
fn redo(level: u16, studio: State<Studio>) -> Result<bool, String> {
    with(&studio, |p| p.redo(level))
}

#[tauri::command]
fn save_rom(studio: State<Studio>) -> Result<Vec<String>, String> {
    with(&studio, |p| p.save())
}

/// Write the ROM with all changes to another file (the open ROM stays as it is).
#[tauri::command]
fn save_rom_copy(path: String, studio: State<Studio>) -> Result<(), String> {
    with(&studio, |p| {
        let mut bytes = p.build_rom()?;
        if p.info.had_header {
            let mut h = vec![0u8; 0x200];
            h.append(&mut bytes);
            bytes = h;
        }
        std::fs::write(&path, bytes).map_err(|e| format!("Couldn't write {path}: {e}"))
    })
}

/// Engine upgrades: whether each is in the ROM, or why it can't be added.
#[derive(Serialize)]
struct UpgradeView {
    id:        &'static str,
    installed: bool,
    /// Why it can't be added to this ROM ("" if it can)
    blocked:   String,
}

#[tauri::command]
fn upgrades(studio: State<Studio>) -> Result<Vec<UpgradeView>, String> {
    with(&studio, |p| {
        Ok(vec![UpgradeView {
            id:        "checkpoints",
            installed: smw_core::upgrades::has_checkpoints(p),
            blocked:   smw_core::upgrades::can_add_checkpoints(p).err().unwrap_or_default(),
        }])
    })
}

#[tauri::command]
fn add_upgrade(id: String, studio: State<Studio>) -> Result<(), String> {
    with(&studio, |p| match id.as_str() {
        "checkpoints" => smw_core::upgrades::add_checkpoints(p),
        _ => Err(format!("Unknown upgrade {id}")),
    })
}

/// Which helper tools SMW Studio found (for Preferences).
#[derive(Serialize)]
struct ToolsStatus {
    lunar_magic: Option<String>,
    addmusick:   Option<String>,
}

#[tauri::command]
fn tools_status() -> ToolsStatus {
    ToolsStatus {
        lunar_magic: smw_core::lunar::Lunar::find().map(|l| l.path().display().to_string()),
        addmusick:   smw_core::music::amk_source().map(|p| p.display().to_string()),
    }
}

#[tauri::command]
fn is_dirty(studio: State<Studio>) -> Result<bool, String> {
    with(&studio, |p| Ok(p.is_dirty()))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(Studio::default())
        .manage(play::Player::default())
        .invoke_handler(tauri::generate_handler![
            open_rom,
            render_level,
            level_entries,
            move_entry,
            delete_entry,
            duplicate_entry,
            set_entry,
            object_palette,
            add_entry,
            test_play,
            play_start,
            free_levels,
            set_screens,
            level_sprites,
            level_exits,
            level_entrances,
            set_main_entrance,
            set_secondary_entrance,
            set_midway_screen,
            save_rom_copy,
            add_entries,
            delete_entries,
            add_sprites,
            delete_sprites,
            tools_status,
            upgrades,
            add_upgrade,
            level_song,
            set_level_song,
            all_secondary_entrances,
            add_secondary_entrance,
            set_exit,
            remove_exit,
            add_sprite,
            move_sprite,
            set_sprite,
            delete_sprite,
            sprite_palette,
            level_header,
            set_header_bits,
            toolbox_cmds::toolbox_list,
            toolbox_cmds::toolbox_import,
            toolbox_cmds::toolbox_import_bytes,
            toolbox_cmds::toolbox_remove,
            toolbox_cmds::toolbox_set_creator,
            toolbox_cmds::toolbox_apply,
            toolbox_cmds::has_lunar_magic,
            toolbox_cmds::music_songs,
            toolbox_cmds::toolbox_import_midi,
            toolbox_cmds::hack_open,
            toolbox_cmds::hack_preview,
            toolbox_cmds::hack_bring,
            toolbox_cmds::music_project,
            toolbox_cmds::rom_songs,
            toolbox_cmds::patches_list,
            toolbox_cmds::patch_import,
            toolbox_cmds::patch_forget,
            toolbox_cmds::patch_set,
            statusbar_view,
            statusbar_set,
            statusbar_cells,
            statusbar_set_cells,
            statusbar_move,
            statusbar_presets,
            statusbar_apply_preset,
            statusbar_elements,
            statusbar_place,
            statusbar_stickers,
            overworld_picture,
            statusbar_set_stickers,
            statusbar_sprite_tiles,
            statusbar_tiles,
            toolbox_cmds::rom_song_play,
            toolbox_cmds::music_project_save,
            toolbox_cmds::music_preview,
            toolbox_cmds::music_add,
            toolbox_cmds::music_remove,
            play_input,
            play_restart,
            play_stop,
            has_emulator,
            undo,
            redo,
            save_rom,
            is_dirty
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
