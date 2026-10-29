# SMW Studio plan

## Stack

| Layer | Tech | Why |
|---|---|---|
| Core (ROM read/write, compression, levels, graphics, patching) | Rust crates, starting from [smw-editor](https://github.com/SMW-Editor/smw-editor) (MIT) | Fast, safe, one codebase for desktop and web |
| Desktop app | Tauri 2 | Small installer (~10-20 MB), Windows / macOS / Linux |
| Web app | Same Rust core compiled to WebAssembly | Same features in the browser, ROM never leaves the user's machine |
| UI | React + TypeScript, Dockview for docking panels | Pro editor look (Adobe / Roblox Studio style) |
| Level canvas | WebGL | Smooth zoom/scroll on big levels |

## Layout

- Top: ribbon toolbar (Select, Draw, Fill, Sprites, Test Play)
- Left: Explorer tree (levels, overworld, graphics, music, patches)
- Center: tabbed level canvas
- Right: Properties + Asset Library / toolbar parts
- Bottom: Output, Undo History, Patch Log
- All panels dock/float/hide; saved layouts; dark + light theme

## Milestones

1. Project setup: Rust workspace + Tauri + React shell with docking layout
2. Level viewer: load a vanilla SMW ROM, draw level 105 identical to Lunar Magic
3. Level editing: objects, sprites, exits; save; reopen in Lunar Magic to verify
4. Map16, graphics (ExGFX), palettes
5. Overworld editor
6. Asset library: export/import parts (gfx + palette + Map16 + behaviour ASM) as packages; pull parts from any hack
7. Built-in patching: Asar, PIXI, GPS, UberASM
8. Music: AddmusicK integration, MIDI import
9. Test Play: launch in an emulator at the chosen level
10. Web build

## Beyond Lunar Magic (things no SMW tool does yet)

SMW Studio runs the real game code (emulated 65816), which makes these possible:

1. Custom sprites/blocks drawn exactly in the editor by running their own ASM
2. Play inside the editor (a "Game" panel with a full emulator core, e.g. snes9x via libretro;
   same core as WebAssembly for the web version), then edit while playing:
   pause, change the level, resume from the same moment
3. Rewind in Test Play to retry a section after an edit
4. Automatic level checks: a bot plays the level and reports softlocks, unbeatable parts, stuck enemies
5. Portable assets: pull blocks/sprites/songs from any hack into the library; auto-fit into the ROM without clashes
6. Patch clash detection: show which patches hook the same code before applying
7. Player graphics import (PNG or another ROM) with a preview of every animation frame
8. Export: native .exe (bundled emulator), widescreen (spawn/load-range patches), shareable .bps patch
9. Web version (same Rust core as WebAssembly)
10. Full version history of the ROM (every save restorable)
11. MCP server: AI agents can look (render levels, read objects/RAM), build (place/move objects,
    import graphics, apply patches), test (Test Play, press buttons, check goal/death) and save,
    with changes appearing live in the open window
12. No-code behaviour builder for sprites/blocks, compiled to 65816
13. Platformer templates: physics presets, HUD and camera styles; one-click SA-1 / widescreen

## Studio features (queued)

- My Projects home screen: every hack with thumbnail, badges (clean / LM version / SA-1 / header),
  reopen at the last level; clean ROM set once and protected ("start a new hack from this")
- Feature tracker inside the studio: Lunar Magic parity checklist (done / partial / not yet)
  plus the "beyond Lunar Magic" list

## Rules

- Every format feature is checked against Lunar Magic 3.71 output (`reference/lunar-magic-3.71`).
- Never commit ROMs or Lunar Magic binaries.
