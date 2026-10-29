import { LEVEL_NAMES } from "./levelNames";
import { useSyncExternalStore } from "react";
import { invoke, Channel } from "@tauri-apps/api/core";
import { open, save as saveDialog } from "@tauri-apps/plugin-dialog";

export type RomSummary = {
  path: string;
  kind: string;
  title: string;
  sha1: string;
  size: number;
  had_header: boolean;
  /** The ROM's own level names, where they differ from the original game's */
  level_names: Record<number, string>;
};

export type Entry = {
  index: number;
  kind: "object" | "extended" | "exit";
  id: number;
  settings: number;
  x: number;
  y: number;
};

export type LevelView = {
  level: number;
  width: number;
  height: number;
  screens: number;
  vertical: boolean;
  mode: number;
  sprites: number;
  /** The layers as images (same size): back area colour + background or layer 2, layer 1, sprites */
  back: Uint8ClampedArray;
  front: Uint8ClampedArray;
  spriteLayer: Uint8ClampedArray;
  /** Per block: entry index + 1, 0 = empty */
  owner: Uint16Array;
  /** Each sprite's box on the image [x, y, w, h], in sprite list order */
  spriteBoxes: Int32Array;
  cols: number;
  rows: number;
  ms: number;
};

export type PaletteItem = {
  kind: "object" | "extended";
  id: number;
  settings: number;
  name: string;
  desc: string;
  w: number;
  h: number;
  /** Preview's top-left block relative to the object's position (objects can draw up/left) */
  ox: number;
  oy: number;
  img: string;
};

type State = {
  rom: RomSummary | null;
  palette: PaletteItem[];
  paletteFor: number | null;
  paletteLoading: boolean;
  brush: PaletteItem | null;
  /** Palette item being dragged onto the level (shown as a ghost where it will land) */
  dragItem: PaletteItem | null;
  level: LevelView | null;
  entries: Entry[];
  selected: number | null;
  /** The level's sprites, and the selected one (separate from object selection) */
  sprites: SpriteItem[];
  selectedSprite: number | null;
  /** Screen exits (where pipes/doors on each screen lead) */
  exits: ScreenExit[];
  exitsPerScreen: boolean;
  /** Where Mario appears in this level */
  entrances: Entrances | null;
  /** Every secondary entrance in the ROM: [number, destination level] */
  allSecondaries: [number, number][];
  /** Sprite chosen in the Sprites palette, placed with each click on the level */
  spriteBrush: SpritePaletteItem | null;
  spritePalette: SpritePaletteItem[];
  spritePaletteFor: number | null;
  loading: boolean;
  dirty: boolean;
  showSprites: boolean;
  layers: Layers;
  tool: Tool;
  /** In-editor play: off, booting the game, or running */
  play: "off" | "loading" | "playing";
  /** Short message shown over the game (died, cleared...) */
  playNote: string;
  log: string[];
};

let state: State = {
  rom: null,
  palette: [],
  paletteFor: null,
  paletteLoading: false,
  brush: null,
  dragItem: null,
  level: null,
  entries: [],
  selected: null,
  sprites: [],
  selectedSprite: null,
  exits: [],
  exitsPerScreen: true,
  entrances: null,
  allSecondaries: [],
  spriteBrush: null,
  spritePalette: [],
  spritePaletteFor: null,
  loading: false,
  dirty: false,
  showSprites: loadPrefs().showSprites,
  layers: defaultLayers(),
  tool: "select",
  play: "off",
  playNote: "",
  log: ["[SMW Studio] Ready."],
};
const listeners = new Set<() => void>();

function set(patch: Partial<State>) {
  state = { ...state, ...patch };
  listeners.forEach((l) => l());
}

export function log(line: string) {
  set({ log: [...state.log.slice(-200), line] });
}

/** The state right now (outside React rendering). */
export const studioState = () => state;

export function useStudio() {
  return useSyncExternalStore(
    (l) => (listeners.add(l), () => listeners.delete(l)),
    () => state,
  );
}

/** A level's name: the ROM's own, else the original game's (a hint: hacks reuse numbers). */
export function levelNameOf(n: number): string | undefined {
  return state.rom?.level_names?.[n] ?? LEVEL_NAMES[n];
}

/** Whether levelNameOf(n) comes from the ROM itself. */
export const nameFromRom = (n: number) => state.rom?.level_names?.[n] !== undefined;

export const hex = (n: number, w = 2) => n.toString(16).toUpperCase().padStart(w, "0");
export const hex3 = (n: number) => hex(n, 3);

export function entryLabel(e: Entry) {
  if (e.kind === "exit") return `Screen exit → ${hex3(((e.settings & 1) << 8) | e.id)}`;
  if (e.kind === "extended") return `Extended object ${hex(e.id)}`;
  return `Object ${hex(e.id)} · size ${hex(e.settings)}`;
}

/** Bounding box in blocks of everything an entry drew, or null if it drew nothing visible. */
export function entryBox(view: LevelView, index: number) {
  let x0 = Infinity, y0 = Infinity, x1 = -1, y1 = -1;
  const want = index + 1;
  for (let i = 0; i < view.owner.length; i++) {
    if (view.owner[i] !== want) continue;
    const x = i % view.cols, y = Math.floor(i / view.cols);
    if (x < x0) x0 = x;
    if (y < y0) y0 = y;
    if (x > x1) x1 = x;
    if (y > y1) y1 = y;
  }
  return x1 < 0 ? null : { x: x0, y: y0, w: x1 - x0 + 1, h: y1 - y0 + 1 };
}

export async function openRom() {
  if (state.dirty && !confirm("You have unsaved changes. Open another ROM anyway?")) return;
  const path = await open({
    title: "Open Super Mario World ROM",
    filters: [{ name: "SNES ROM", extensions: ["sfc", "smc"] }],
  });
  if (typeof path !== "string") return;
  await openRomAt(path);
}

// ---- Preferences (this computer only) ----

export type Prefs = { reopenLastRom: boolean; zoom: number; showSprites: boolean };
// (a function, not a constant: the store's starting state reads it before this point)
function defaultPrefs(): Prefs {
  return { reopenLastRom: true, zoom: 2, showSprites: true };
}

export function loadPrefs(): Prefs {
  try {
    return { ...defaultPrefs(), ...JSON.parse(localStorage.getItem("smw-studio.prefs") ?? "{}") };
  } catch {
    return defaultPrefs();
  }
}

export function savePrefs(p: Prefs) {
  try { localStorage.setItem("smw-studio.prefs", JSON.stringify(p)); } catch {}
}

/** ROMs opened lately, newest first. */
export function recentRoms(): string[] {
  try {
    return JSON.parse(localStorage.getItem("smw-studio.recent") ?? "[]");
  } catch {
    return [];
  }
}

export async function saveCopy() {
  if (!state.rom) return;
  const path = await saveDialog({ title: "Save a copy of the ROM", filters: [{ name: "SNES ROM", extensions: ["sfc", "smc"] }] });
  if (!path) return;
  try {
    await invoke("save_rom_copy", { path });
    log(`Saved a copy with your changes to ${path}`);
  } catch (e) {
    log(`Save a copy failed: ${e}`);
  }
}

/** Re-read whether there are unsaved changes (after a change outside the level editor). */
export async function refreshDirty() {
  set({ dirty: await invoke<boolean>("is_dirty") });
}

export function closeRom() {
  if (state.dirty && !confirm("You have unsaved changes. Close the ROM anyway?")) return;
  try { localStorage.removeItem("smw-studio.lastRom"); } catch {}
  set({ rom: null, level: null, entries: [], selected: null, dirty: false });
  log("Closed the ROM");
}

export async function openRomAt(path: string) {
  try {
    const rom = await invoke<RomSummary>("open_rom", { path });
    try {
      localStorage.setItem("smw-studio.lastRom", path);
      localStorage.setItem("smw-studio.recent", JSON.stringify([path, ...recentRoms().filter((p) => p !== path)].slice(0, 6)));
    } catch {}
    set({ rom, level: null, entries: [], selected: null, dirty: false, palette: [], paletteFor: null, brush: null });
    log(`Opened ${path}`);
    log(`  ${rom.kind} · ${rom.size / 1024} KB${rom.had_header ? " · copier header removed" : ""}`);
    await loadLevel(0x105);
  } catch (e) {
    log(`Error: ${e}`);
    alert(String(e));
  }
}

/** Reopen the ROM from last time (after a restart or reload), if it's still there. */
export async function reopenLastRom() {
  let path: string | null = null;
  try { path = localStorage.getItem("smw-studio.lastRom"); } catch {}
  if (path && !state.rom && loadPrefs().reopenLastRom) await openRomAt(path);
}

async function refresh(level: number, keepSelection: number | null) {
  const t = performance.now();
  const buf = await invoke<ArrayBuffer>("render_level", { level, sprites: state.showSprites });
  const entries = await invoke<Entry[]>("level_entries", { level });
  const sprites = await invoke<SpriteItem[]>("level_sprites", { level });
  const ex = await invoke<{ per_screen: boolean; exits: ScreenExit[] }>("level_exits", { level });
  const entrances = await invoke<Entrances>("level_entrances", { level });
  const allSecondaries = await invoke<[number, number][]>("all_secondary_entrances");
  const dirty = await invoke<boolean>("is_dirty");
  const dv = new DataView(buf);
  const width = dv.getUint32(0, true);
  const height = dv.getUint32(4, true);
  const cols = dv.getUint32(12, true);
  const rows = dv.getUint32(16, true);
  const layerBytes = width * height * 4;
  const pixEnd = 20 + layerBytes * 3;
  const view: LevelView = {
    level,
    width,
    height,
    screens: dv.getUint8(8),
    vertical: dv.getUint8(9) === 1,
    mode: dv.getUint8(10),
    sprites: dv.getUint8(11),
    back: new Uint8ClampedArray(buf, 20, layerBytes),
    front: new Uint8ClampedArray(buf, 20 + layerBytes, layerBytes),
    spriteLayer: new Uint8ClampedArray(buf, 20 + layerBytes * 2, layerBytes),
    owner: new Uint16Array(buf.slice(pixEnd, pixEnd + cols * rows * 2)),
    spriteBoxes: (() => {
      const at = pixEnd + cols * rows * 2;
      const n = buf.byteLength >= at + 4 ? dv.getUint32(at, true) : 0;
      return new Int32Array(buf.slice(at + 4, at + 4 + n * 16));
    })(),
    cols,
    rows,
    ms: Math.round(performance.now() - t),
  };
  set({ level: view, entries, sprites, exits: ex.exits, exitsPerScreen: ex.per_screen, entrances, allSecondaries, dirty, selected: keepSelection });
  return view;
}

export async function loadLevel(level: number) {
  if (!state.rom || state.loading) return;
  set({ loading: true });
  try {
    const view = await refresh(level, null);
    log(`Level ${hex3(level)} loaded in ${view.ms} ms (${view.screens} screens, ${view.sprites} sprites)`);
  } catch (e) {
    log(`Error loading level ${hex3(level)}: ${e}`);
  } finally {
    set({ loading: false });
  }
}

export function select(index: number | null) {
  set({ selected: index, selectedSprite: index === null ? state.selectedSprite : null });
}

export function selectSprite(index: number | null) {
  set({ selectedSprite: index, selected: index === null ? state.selected : null });
}

/** Run an edit command, then re-render the level. */
async function edit(cmd: string, args: Record<string, unknown>, describe: string) {
  const lv = state.level;
  if (!lv || state.loading) return;
  set({ loading: true });
  try {
    const result = await invoke<number | null>(cmd, { level: lv.level, ...args });
    const sel = typeof result === "number" ? result : cmd === "delete_entry" ? null : state.selected;
    await refresh(lv.level, sel);
    log(describe);
  } catch (e) {
    log(`Error: ${e}`);
  } finally {
    set({ loading: false });
  }
}

export function moveEntry(index: number, x: number, y: number) {
  return edit("move_entry", { index, x, y }, `Moved entry ${index} to ${x},${y}`);
}

export function duplicateEntry(index: number, x: number, y: number) {
  return edit("duplicate_entry", { index, x, y }, `Copied entry ${index} to ${x},${y}`);
}

/** Make the level longer or shorter: 1-32 screens (1-14 for vertical levels). */
export function setScreens(screens: number) {
  return edit("set_screens", { screens }, `Level is now ${screens} screen${screens === 1 ? "" : "s"} long`);
}

export function deleteEntry(index: number) {
  return edit("delete_entry", { index }, `Deleted entry ${index}`);
}

export function setEntry(index: number, id: number, settings: number) {
  return edit("set_entry", { index, id, settings }, `Changed entry ${index}`);
}

export async function undo() {
  const lv = state.level;
  if (!lv || state.loading) return;
  if (await invoke<boolean>("undo", { level: lv.level })) {
    await refresh(lv.level, null);
    set({ selectedSprite: null });
    log("Undo");
  }
}

export async function redo() {
  const lv = state.level;
  if (!lv || state.loading) return;
  if (await invoke<boolean>("redo", { level: lv.level })) {
    await refresh(lv.level, null);
    set({ selectedSprite: null });
    log("Redo");
  }
}

export async function save() {
  if (!state.rom || state.loading) return;
  set({ loading: true });
  try {
    const report = await invoke<string[]>("save_rom");
    report.forEach((l) => log(l));
    set({ dirty: false });
  } catch (e) {
    log(`Save failed: ${e}`);
  } finally {
    set({ loading: false });
  }
}

export function toggleSprites() {
  set({ showSprites: !state.showSprites });
  if (state.level) loadLevel(state.level.level);
}

/** Load the object palette (previews in the current level's tileset). */
export async function loadPalette() {
  const lv = state.level;
  if (!lv || state.paletteLoading || state.paletteFor === lv.level) return;
  set({ paletteLoading: true });
  try {
    const t = performance.now();
    const palette = await invoke<PaletteItem[]>("object_palette", { level: lv.level });
    set({ palette, paletteFor: lv.level });
    log(`Object palette: ${palette.length} objects (${Math.round(performance.now() - t)} ms)`);
  } catch (e) {
    log(`Error loading object palette: ${e}`);
  } finally {
    set({ paletteLoading: false });
  }
}

export function setBrush(item: PaletteItem | null) {
  // Picking an object to place switches to Draw (and back to Select when it's put down)
  set({ brush: item, spriteBrush: null, tool: item ? "draw" : state.tool === "draw" || state.tool === "fill" ? "select" : state.tool });
}

// ---- Tools ----

export type Tool = "select" | "draw" | "fill" | "erase" | "sprites";

export function setTool(tool: Tool) {
  set({ tool });
}

/** Whether the game is running in the editor (keys go to the game then). */
export function isPlaying() {
  return state.play !== "off";
}

/** Size of an object brush in blocks (its preview's size). */
export function brushBlocks(b: { w: number; h: number }): [number, number] {
  return [Math.max(1, Math.ceil(b.w / 16)), Math.max(1, Math.ceil(b.h / 16))];
}

/** Place the brush object at each [x, y] (one undo step). */
export function paintObjects(item: PaletteItem, cells: [number, number][]) {
  if (!cells.length) return;
  const items = cells.map(([x, y]) => ({ kind: item.kind, id: item.id, settings: item.settings, x, y }));
  return edit("add_entries", { items }, `Placed ${cells.length} × ${item.name}`);
}

/** Place a sprite at each [x, y] (one undo step). */
export function paintSprites(id: number, cells: [number, number][]) {
  if (!cells.length) return;
  return spriteEdit("add_sprites", { items: cells.map(([x, y]) => [id, x, y, 0]) }, `Placed ${cells.length} sprite${cells.length > 1 ? "s" : ""}`, null);
}

/** Delete objects and sprites wiped by the Erase tool. */
export async function eraseThings(objects: number[], spriteIdx: number[]) {
  if (spriteIdx.length) await spriteEdit("delete_sprites", { indices: spriteIdx }, `Erased ${spriteIdx.length} sprite${spriteIdx.length > 1 ? "s" : ""}`, null);
  if (objects.length) await edit("delete_entries", { indices: objects }, `Erased ${objects.length} object${objects.length > 1 ? "s" : ""}`);
}

export function setDragItem(item: PaletteItem | null) {
  set({ dragItem: item });
}

export function placeObject(item: PaletteItem, x: number, y: number) {
  return edit("add_entry", { kind: item.kind, id: item.id, settings: item.settings, x, y }, `Placed ${item.name} at ${x},${y}`);
}
// ---- Test Play ----

export type TestOptions = {
  lock_level: boolean;
  infinite_lives: boolean;
  invincible: boolean;
  infinite_time: boolean;
  /** -1 keep, 0 small, 1 big, 2 cape, 3 fire */
  powerup: number;
  /** Start as if the midway gate was already touched */
  midway: boolean;
};

const DEFAULT_TEST: TestOptions = { lock_level: true, infinite_lives: true, invincible: false, infinite_time: false, powerup: -1, midway: false };

export function loadTestOptions(): TestOptions {
  try {
    return { ...DEFAULT_TEST, ...JSON.parse(localStorage.getItem("smw-studio.test") ?? "{}") };
  } catch {
    return DEFAULT_TEST;
  }
}

export function saveTestOptions(o: TestOptions) {
  try {
    localStorage.setItem("smw-studio.test", JSON.stringify(o));
  } catch {
    /* storage unavailable: options just won't be remembered */
  }
}

export async function testPlay(options: TestOptions) {
  const lv = state.level;
  if (!lv) return;
  try {
    log(await invoke<string>("test_play", { level: lv.level, options }));
    if (state.dirty) log("  (includes your unsaved changes; your ROM file wasn't changed)");
  } catch (e) {
    log(`Test Play failed: ${e}`);
  }
}
// ---- In-editor play ----

/** Receives every game frame (u16 width, u16 height, RGBA). Set by the Game view. */
let frameSink: ((buf: ArrayBuffer) => void) | null = null;
export function setFrameSink(f: ((buf: ArrayBuffer) => void) | null) {
  frameSink = f;
}

let noteTimer: number | undefined;
function note(text: string) {
  set({ playNote: text });
  clearTimeout(noteTimer);
  noteTimer = window.setTimeout(() => set({ playNote: "" }), 1500);
}

let playSession = 0;
let framesReceived = 0;

export async function startPlay() {
  const lv = state.level;
  if (!lv || state.play !== "off") return;
  const me = ++playSession;
  set({ play: "loading", playNote: "" });
  const frames = new Channel<ArrayBuffer>();
  frames.onmessage = (buf) => {
    framesReceived++;
    frameSink?.(buf);
  };
  const events = new Channel<string>();
  events.onmessage = (ev) => {
    if (me !== playSession) return; // a game that was already stopped
    if (ev === "ready") set({ play: "playing" });
    else if (ev === "died") note("Restarting");
    else if (ev === "finished") note("Course clear! Restarting");
    else if (ev === "stopped") set({ play: "off" });
    else if (ev.startsWith("error")) {
      log(`Play failed: ${ev.slice(7)}`);
      set({ play: "off" });
    }
  };
  const o = loadTestOptions();
  try {
    await invoke("play_start", {
      level: lv.level,
      options: { infinite_lives: o.infinite_lives, invincible: o.invincible, infinite_time: o.infinite_time, powerup: o.powerup, midway: o.midway },
      frames,
      events,
    });
  } catch (e) {
    log(`Play failed: ${e}`);
    set({ play: "off" });
  }
}

export function stopPlay() {
  playSession++;
  lastButtons = -1;
  invoke("play_stop");
  set({ play: "off", playNote: "" });
}

export function restartPlay() {
  invoke("play_restart");
}

let lastButtons = -1;
/** Buttons held by a script (automated checks, MCP), merged with keyboard/gamepad */
let scriptedButtons = 0;
export function scriptButtons(b: number) {
  scriptedButtons = b;
  playInput(0);
}
/** libretro joypad bits: B=0 Y=1 Select=2 Start=3 Up=4 Down=5 Left=6 Right=7 A=8 X=9 L=10 R=11 */
export function playInput(held: number) {
  const buttons = held | scriptedButtons;
  if (buttons === lastButtons) return;
  lastButtons = buttons;
  invoke("play_input", { buttons });
}
// Dev builds only: lets scripts drive the studio (automated checks; later the MCP server)
if (import.meta.env.DEV) {
  (window as unknown as { __smw: object }).__smw = {
    state: () => state,
    frames: () => framesReceived,
    buttons: () => lastButtons,
    pads: () => [...(navigator.getGamepads?.() ?? [])].filter(Boolean).map((g) => ({ id: g!.id, axes: g!.axes.map((a) => +a.toFixed(2)), pressed: g!.buttons.map((b, i) => (b.pressed ? i : -1)).filter((i) => i >= 0) })),
    openRomAt,
    loadLevel,
    setPatch: (p: PatchView, on: boolean, settings: PatchSetting[]) => setPatch(p, on, settings),
    startPlay,
    stopPlay,
    restartPlay,
    playInput,
    scriptButtons,
    selectSprite,
    setExit,
  };
}

// ---- Toolbox (asset library) ----

export type Pack = {
  id: string;
  name: string;
  creator: string;
  kind: string;
  notes: string[];
  links: string[];
  files: number;
  levels: string[];
  preview: string;
  actions: ("add_level" | "replace_graphics" | "add_song")[];
};

export async function listPacks(): Promise<Pack[]> {
  return invoke<Pack[]>("toolbox_list");
}

export async function importPacks(paths?: string[]): Promise<Pack[]> {
  if (!paths) {
    const picked = await open({ title: "Import asset packs", multiple: true, filters: [{ name: "Zip", extensions: ["zip"] }] });
    if (!picked) return [];
    paths = Array.isArray(picked) ? picked : [picked];
  }
  const added = await invoke<Pack[]>("toolbox_import", { paths });
  for (const p of added) log(`Toolbox: added "${p.name}" (${p.kind}${p.creator ? `, by ${p.creator}` : ""})`);
  return added;
}

/** Turn MIDI files into a music pack (pick all the parts of one song together). */
export async function importMidi(): Promise<Pack | null> {
  const picked = await open({ title: "Import a song from MIDI (pick all its parts together)", multiple: true, filters: [{ name: "MIDI", extensions: ["mid", "midi"] }] });
  if (!picked) return null;
  const paths = Array.isArray(picked) ? picked : [picked];
  const [pack, dropped] = await invoke<[Pack, string[]]>("toolbox_import_midi", { paths });
  log(`Toolbox: made song "${pack.name}" from ${paths.length} MIDI file${paths.length > 1 ? "s" : ""}.`);
  if (dropped.length) log(`  The SNES plays 8 parts at once, so these were left out: ${dropped.join(", ")}`);
  log(`  Press "Add song" on it to put it in your ROM.`);
  return pack;
}

/** Import zips dropped from Explorer (the page only gets their contents). */
export async function importDroppedFiles(files: File[]): Promise<Pack[]> {
  const out: Pack[] = [];
  for (const f of files.filter((f) => f.name.toLowerCase().endsWith(".zip"))) {
    const data = Array.from(new Uint8Array(await f.arrayBuffer()));
    const p = await invoke<Pack>("toolbox_import_bytes", { name: f.name, data });
    log(`Toolbox: added "${p.name}" (${p.kind}${p.creator ? `, by ${p.creator}` : ""})`);
    out.push(p);
  }
  return out;
}

export async function removePack(id: string) {
  await invoke("toolbox_remove", { id });
}

export async function setPackCreator(id: string, creator: string) {
  await invoke("toolbox_set_creator", { id, creator });
}

/** Use a pack in the open ROM (saves unsaved edits first; the ROM file is backed up). */
export async function applyPack(pack: Pack, action: { type: "add_level"; level: number; mwl: string | null } | { type: "replace_graphics" }) {
  const rom = state.rom;
  if (!rom) return;
  set({ loading: true });
  try {
    log(`Toolbox: using "${pack.name}"…`);
    const report = await invoke<string[]>("toolbox_apply", { id: pack.id, action });
    report.forEach((l) => log(`  ${l}`));
    // (not busy any more, or loading the level is skipped)
    set({ loading: false });
    await openRomAt(rom.path);
    if (action.type === "add_level") await loadLevel(action.level);
    log(`Toolbox: "${pack.name}" is in. Press Play to try it.`);
  } catch (e) {
    log(`Toolbox: couldn't use "${pack.name}": ${e}`);
  } finally {
    set({ loading: false });
  }
}
// ---- Music (AddmusicK) ----

export type Song = { number: number; title: string; author: string; file: string; pack: string | null };
export type MusicView = { songs: Song[]; any_song: boolean; available: boolean };

export async function loadMusic(): Promise<MusicView | null> {
  try {
    return await invoke<MusicView>("music_songs");
  } catch {
    return null;
  }
}

/** Put a music pack's song in the ROM, as a new song or replacing song `replace`. */
export async function addSong(pack: Pack, replace: number | null) {
  const rom = state.rom;
  if (!rom) return null;
  const lv = state.level?.level ?? null;
  set({ loading: true });
  try {
    log(`Music: adding "${pack.name}"${replace !== null ? ` in place of song ${hex(replace)}` : ""}…`);
    const [n, report] = await invoke<[number, string[]]>("music_add", { id: pack.id, file: null, replace });
    report.forEach((l) => log(`  ${l}`));
    set({ loading: false });
    await openRomAt(rom.path);
    if (lv !== null) await loadLevel(lv);
    log(`Music: "${pack.name}" is song ${hex(n)}. Pick it for a level in Level settings → Song.`);
    return n;
  } catch (e) {
    log(`Music: couldn't add "${pack.name}": ${e}`);
    return null;
  } finally {
    set({ loading: false });
  }
}

export async function removeSong(number: number) {
  const rom = state.rom;
  if (!rom) return;
  const lv = state.level?.level ?? null;
  set({ loading: true });
  try {
    await invoke("music_remove", { number });
    set({ loading: false });
    await openRomAt(rom.path);
    if (lv !== null) await loadLevel(lv);
    log(`Music: song ${hex(number)} removed (the game's own song is back if it was a replacement)`);
  } catch (e) {
    log(`Music: ${e}`);
  } finally {
    set({ loading: false });
  }
}

export function setLevelSong(song: number, title: string) {
  return edit("set_level_song", { song }, song ? `Song: ${hex(song)} ${title}` : "Song: from the music setting");
}

// ---- Level settings (header) ----

export type HeaderView = { bytes: number[]; music: number[]; colours: string[] };

export async function loadHeader(level: number): Promise<HeaderView | null> {
  try {
    return await invoke<HeaderView>("level_header", { level });
  } catch {
    return null;
  }
}

/** Change bits `mask` of header byte `byte` to `value` (already shifted into place). */
export async function setHeaderBits(byte: number, mask: number, value: number, what: string) {
  await edit("set_header_bits", { byte, mask, value }, what);
  // Tileset/palettes changed: object previews must be redrawn
  set({ paletteFor: null, spritePaletteFor: null });
}
// ---- Sprites ----

export type SpriteItem = { index: number; id: number; name: string; x: number; y: number; extra: number };
/** ox/oy: where the picture's top-left is relative to the sprite's position (pixels) */
export type SpritePaletteItem = { id: number; name: string; w: number; h: number; img: string; ox: number; oy: number };

/** Run a sprite edit, re-render, and select the sprite the command returns (if any). */
async function spriteEdit(cmd: string, args: Record<string, unknown>, describe: string, keep: number | null) {
  const lv = state.level;
  if (!lv || state.loading) return;
  set({ loading: true });
  try {
    const result = await invoke<number | null>(cmd, { level: lv.level, ...args });
    await refresh(lv.level, null);
    set({ selectedSprite: typeof result === "number" ? result : keep });
    log(describe);
  } catch (e) {
    log(`Error: ${e}`);
  } finally {
    set({ loading: false });
  }
}

export function addSprite(id: number, x: number, y: number, extra = 0) {
  return spriteEdit("add_sprite", { id, x, y, extra }, `Placed sprite ${hex(id)} at ${x},${y}`, null);
}
export function moveSprite(index: number, x: number, y: number) {
  return spriteEdit("move_sprite", { index, x, y }, `Moved sprite to ${x},${y}`, null);
}
export function setSprite(index: number, id: number, extra: number) {
  return spriteEdit("set_sprite", { index, id, extra }, `Changed sprite to ${hex(id)}`, index);
}
export function deleteSprite(index: number) {
  return spriteEdit("delete_sprite", { index }, "Deleted sprite", null);
}

export function setSpriteBrush(item: SpritePaletteItem | null) {
  set({ spriteBrush: item, brush: null, tool: item ? "sprites" : state.tool === "sprites" ? "select" : state.tool });
}

export async function loadSpritePalette() {
  const lv = state.level;
  if (!lv || state.spritePaletteFor === lv.level) return;
  set({ spritePaletteFor: lv.level });
  try {
    const t = performance.now();
    const spritePalette = await invoke<SpritePaletteItem[]>("sprite_palette", { level: lv.level });
    set({ spritePalette });
    log(`Sprite palette: ${spritePalette.length} sprites (${Math.round(performance.now() - t)} ms)`);
  } catch (e) {
    log(`Error loading sprites: ${e}`);
    set({ spritePaletteFor: null });
  }
}

/** Which sprite (index) is at level pixel (px, py), topmost first. */
// ---- Layers ----

export type LayerId = "sprites" | "front" | "back" | "marks";
export type LayerState = { show: boolean; lock: boolean };
export type Layers = Record<LayerId, LayerState> & { active: LayerId | null; fade: boolean };

function defaultLayers(): Layers {
  const l = { show: true, lock: false };
  return { sprites: { ...l }, front: { ...l }, back: { ...l }, marks: { ...l }, active: null, fade: false };
}

export function setLayer(id: LayerId, patch: Partial<LayerState>) {
  set({ layers: { ...state.layers, [id]: { ...state.layers[id], ...patch } } });
}

export function setActiveLayer(id: LayerId | null) {
  set({ layers: { ...state.layers, active: id } });
}

export function setLayerFade(fade: boolean) {
  set({ layers: { ...state.layers, fade } });
}

/** Whether clicks can pick things on a layer (shown, not locked, and the active one if any). */
export function canPick(id: LayerId) {
  const l = state.layers;
  return l[id].show && !l[id].lock && (l.active === null || l.active === id);
}

export function spriteAt(px: number, py: number): number | null {
  const b = state.level?.spriteBoxes;
  if (!b || !state.showSprites || !canPick("sprites")) return null;
  for (let i = b.length / 4 - 1; i >= 0; i--) {
    const [x, y, w, h] = [b[i * 4], b[i * 4 + 1], b[i * 4 + 2], b[i * 4 + 3]];
    if (px >= x && px < x + w && py >= y && py < y + h) return i;
  }
  return null;
}
// ---- Screen exits ----

export type ScreenExit = { screen: number; dest: number; secondary: boolean; water: boolean };

export function setExit(exit: ScreenExit) {
  const where = exit.secondary ? `secondary entrance ${hex3(exit.dest)}` : `level ${hex3(exit.dest)}`;
  return edit("set_exit", { exit }, `Screen ${hex(exit.screen)} exit now leads to ${where}`);
}

export function removeExit(screen: number) {
  return edit("remove_exit", { screen }, `Removed the exit on screen ${hex(screen)}`);
}
// ---- Entrances ----

export type Entrance = { screen: number; x: number; y: number; action: number };
/** For the main entrance, `exact` means x/y are 16x16 tiles (newer Lunar Magic) instead of spot/height indexes. */
export type Entrances = { main: Entrance; secondaries: { number: number; entrance: Entrance }[]; xs: number[]; ys: number[]; exact: boolean; midway: number };

export function setMainEntrance(entrance: Entrance) {
  return edit("set_main_entrance", { entrance }, `Main entrance: screen ${hex(entrance.screen)}`);
}

export function setSecondaryEntrance(number: number, entrance: Entrance) {
  return edit("set_secondary_entrance", { number, entrance }, `Secondary entrance ${hex3(number)} moved`);
}

export function setMidwayScreen(screen: number) {
  return edit("set_midway_screen", { screen }, `Midway point: screen ${hex(screen)}`);
}

/** Make a new secondary entrance into the current level (at the main entrance's spot). */
export async function addSecondaryEntrance() {
  const lv = state.level;
  if (!lv || state.loading) return null;
  set({ loading: true });
  try {
    const n = await invoke<number>("add_secondary_entrance", { level: lv.level });
    await refresh(lv.level, state.selected);
    log(`New secondary entrance ${hex3(n)} into this level. Point a screen exit at it to use it.`);
    return n;
  } catch (e) {
    log(`Error: ${e}`);
    return null;
  } finally {
    set({ loading: false });
  }
}

/** Level pixel position where Mario appears for an entrance (`exact`: x/y are 16x16 tiles). */
export function entrancePixel(e: Entrance, en: Entrances, vertical: boolean, exact = false) {
  const x = exact ? e.x * 16 : en.xs[e.x], y = exact ? e.y * 16 : en.ys[e.y];
  return vertical ? { x: (e.screen % 2) * 256 + x, y: Math.floor(e.screen / 2) * 256 + y } : { x: e.screen * 256 + x, y };
}

/** The entrance spot nearest to level pixel (px, py). */
export function nearestEntrance(px: number, py: number, en: Entrances, vertical: boolean, action: number, exact = false): Entrance {
  const screen = vertical ? Math.floor(py / 256) * 2 + Math.floor(px / 256) : Math.floor(px / 256);
  const lx = vertical ? px % 256 : px - screen * 256;
  const ly = vertical ? py % 256 : py;
  const near = (list: number[], v: number) => list.reduce((best, cur, i) => (Math.abs(cur - v) < Math.abs(list[best] - v) ? i : best), 0);
  const clamp = (v: number, hi: number) => Math.max(0, Math.min(hi, v));
  if (exact) return { screen: clamp(screen, 31), x: clamp(Math.floor(lx / 16), 15), y: clamp(Math.floor(ly / 16) - 1, 255), action };
  return { screen: clamp(screen, 31), x: near(en.xs, lx - 8), y: near(en.ys, ly - 16), action };
}
export type PatchSetting = { name: string; value: string; comment: string };
export type PatchView = { id: string; name: string; on: boolean; settings: PatchSetting[]; other: string[] };

/** Put a patch in the open ROM or take it out (saves first), then reload. */
export async function setPatch(p: PatchView, on: boolean, settings: PatchSetting[]) {
  const rom = state.rom;
  if (!rom || state.loading) return false;
  const lv = state.level?.level ?? null;
  set({ loading: true });
  try {
    log(`Patches: ${on ? "putting in" : "taking out"} "${p.name}"…`);
    await invoke<string[]>("patch_set", { id: p.id, on, settings });
    set({ loading: false });
    await openRomAt(rom.path);
    if (lv !== null) await loadLevel(lv);
    log(`Patches: "${p.name}" is ${on ? "on" : "off"}. Press Play to try it.`);
    return true;
  } catch (e) {
    log(`Patches: ${e}`);
    return false;
  } finally {
    set({ loading: false });
  }
}

/** Add a patch (.asm or .zip) to the library. */
export async function importPatch() {
  const path = await open({ multiple: false, filters: [{ name: "Patch (.asm or .zip)", extensions: ["asm", "zip"] }] });
  if (typeof path !== "string") return null;
  try {
    const name = await invoke<string>("patch_import", { path });
    log(`Patches: added "${name}"`);
    return name;
  } catch (e) {
    log(`Patches: ${e}`);
    return null;
  }
}
