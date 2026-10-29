// Feature tracker: what Lunar Magic can do, and where SMW Studio stands on each.
// Keep this up to date as features land: it drives the Features panel and its percentage.

export type Status = "done" | "partial" | "todo";
export type Feature = { name: string; status: Status; note?: string };
export type Area = { area: string; features: Feature[] };

export const LUNAR_MAGIC: Area[] = [
  {
    area: "Levels",
    features: [
      { name: "Open any level and see it exactly as the game draws it", status: "done", note: "Rendered by running the game's own code, Lunar Magic hacks included" },
      { name: "Place objects from a palette", status: "done", note: "184 objects with Lunar Magic's names and previews" },
      { name: "Select, move, copy, delete, resize objects", status: "done" },
      { name: "Undo / redo", status: "done" },
      { name: "Level settings (music, tileset, palettes, timer, scroll...)", status: "done" },
      { name: "Level length (screens)", status: "done" },
      { name: "Edit sprites (enemies, items)", status: "done", note: "Place from a palette drawn by the game, move, copy, delete, change number/extra bits" },
      { name: "Screen exits (where pipes and doors lead)", status: "done", note: "Add, change, remove; per-screen flags on Lunar Magic ROMs" },
      { name: "Main / midway entrances", status: "done", note: "Right-click to place; exact-tile entrances from newer Lunar Magic; midway screen" },
      { name: "Secondary entrances", status: "done", note: "Create, move, and pick them from screen exits by destination" },
      { name: "Layer 2 editing", status: "todo", note: "Shown; not editable yet" },
      { name: "Layer 3 settings (tides, etc.)", status: "todo" },
      { name: "Level dimensions (tall or extra-long levels)", status: "todo" },
      { name: "Change level mode", status: "todo" },
    ],
  },
  {
    area: "Graphics",
    features: [
      { name: "Map16 editor (16x16 tiles and their behaviour)", status: "todo" },
      { name: "Import ExGFX (extra graphics)", status: "partial", note: "Through Toolbox packs" },
      { name: "Per-level graphics slots (Super GFX bypass)", status: "partial", note: "Set by Toolbox sample levels; no editor yet" },
      { name: "Palette editor", status: "todo" },
      { name: "Custom palettes per level", status: "partial", note: "Through Toolbox packs" },
      { name: "Replace the game's graphics (GFX)", status: "done", note: "Toolbox: Use in whole game" },
      { name: "ExAnimation (animated tiles)", status: "todo" },
    ],
  },
  {
    area: "World and game",
    features: [
      { name: "Overworld map editor", status: "todo" },
      { name: "Level names", status: "todo" },
      { name: "Message box text", status: "todo" },
      { name: "Title screen", status: "todo" },
      { name: "Credits", status: "todo" },
    ],
  },
  {
    area: "Files and testing",
    features: [
      { name: "Open and save ROMs (headered or not)", status: "done", note: "Backup made on first save" },
      { name: "Expand the ROM", status: "done", note: "Automatic when space runs out" },
      { name: "Test a level in an emulator", status: "done", note: "Plays inside the editor, starts in under a second" },
      { name: "Import levels (.mwl)", status: "partial", note: "Through Toolbox packs" },
      { name: "Export levels (.mwl)", status: "todo" },
      { name: "Import Map16 / palettes files", status: "partial", note: "Through Toolbox packs" },
      { name: "Restore points / history", status: "partial", note: "Backups only; full history is planned" },
    ],
  },
];

export const BEYOND: Feature[] = [
  { name: "Play inside the editor, Mario Maker style", status: "done" },
  { name: "Cheats while testing (lives, invincible, time, power-up)", status: "done" },
  { name: "Toolbox: import SMW Central packs once, use them anywhere", status: "done" },
  { name: "Add packs in plain words (no level numbers)", status: "done" },
  { name: "Game-accurate display of Lunar Magic hacks", status: "done" },
  { name: "SA-1 hacks: drawn, edited, saved and played from the inside", status: "done" },
  { name: "Dockable panels with a Window menu", status: "done" },
  { name: "Music import (AddmusicK) and MIDI to song", status: "done" },
  { name: "Original vs custom songs in any hack, and hear any song", status: "done" },
  { name: "Take levels and songs from other hacks", status: "done" },
  { name: "Layers panel (show/hide each layer)", status: "done" },
  { name: "Status bar designer: presets, drag-in pieces, moving counters, sprites", status: "done" },
  { name: "Patches and per-frame code switched on and off (ground pound, wall jump)", status: "done" },
  { name: "Docs, tutorials and first-run tour", status: "done" },
  { name: "Overworld shown as the game draws it", status: "done" },
];

/** What's needed before the first public release (this is the alpha). */
export const BEFORE_RELEASE: Feature[] = [
  { name: "Overworld editing: level tiles, paths, Mario's start, level names", status: "partial", note: "Shown, not editable yet" },
  { name: "Map16 block editor", status: "todo" },
  { name: "Toolbox blocks in Objects (like a brick block): added as new data in free space, never replacing anything", status: "todo", note: "Needs Lunar Magic's direct-Map16 objects read and written" },
  { name: "Objects start at their natural size, not 2x2", status: "todo" },
  { name: "Graphics and palette editing, with per-object colours and smart palette packing", status: "todo" },
  { name: "Custom sprites and blocks built in (the jobs of PIXI and GPS)", status: "todo" },
  { name: "Stand alone: no Lunar Magic needed for any job", status: "todo" },
  { name: "Clear messages where you are, not just in the Output log", status: "partial" },
  { name: "Per-frame code (UberASM) in SA-1 hacks", status: "todo" },
  { name: "Tabbed preferences: emulator, controls, light and dark mode", status: "todo" },
  { name: "Share your hack as a BPS patch", status: "todo" },
  { name: "Stability pass on popular hacks, crash-safe autosave", status: "todo" },
  { name: "Installer, GitHub page, release notes", status: "partial", note: "What's New window done" },
  { name: "Update check: tells you when a new version is out (downloaded version)", status: "todo", note: "Needs the GitHub releases page" },
];

/** Great follow-ups after release (not needed for it). */
export const FUTURE: Feature[] = [
  { name: ".smw project files: features older ROMs can't have, exported to a normal ROM to play and share", status: "todo" },
  { name: "Effects toolbox: Mode 7 and HDMA for pseudo-3D floors, waves and depth", status: "todo" },
  { name: "New game from empty (blank levels and map)", status: "todo" },
  { name: "Engine rewrites for less slowdown, proved identical side by side", status: "todo" },
  { name: "More engine upgrades: more sprites, auto-run, stackable items", status: "todo" },
  { name: "Scene builder: cutscenes with pictures and typewriter text, title and splash screens", status: "todo" },
  { name: "Translations", status: "todo" },
  { name: "What's New panel", status: "todo" },
  { name: "Plugins", status: "todo" },
  { name: "Japanese and European game versions", status: "todo" },
  { name: "Mac and Linux", status: "todo" },
  { name: "Widescreen / PC version", status: "todo" },
  { name: "AI assistant connection (MCP)", status: "todo" },
  { name: "Pause, edit and keep playing; rewind", status: "todo" },
  { name: "Bot that checks levels can be beaten", status: "todo" },
  { name: "A platformer engine of our own: any game, original graphics, no ROM needed", status: "todo", note: "A separate product sharing this editor" },
];

/** Done counts 1, partial counts half. */
export function percent(features: Feature[]) {
  const score = features.reduce((s, f) => s + (f.status === "done" ? 1 : f.status === "partial" ? 0.5 : 0), 0);
  return Math.round((score / features.length) * 100);
}
