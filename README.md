# SMW Studio

A modern, all-in-one editor for Super Mario World hacks. **Alpha.**

**Website:** https://antonretro.github.io/smw-studio/ · **Download:** [Releases](https://github.com/antonretro/smw-studio/releases)

What works now (Windows):

- Level editing, drawn by running the game's own code (Lunar Magic and SA-1 hacks included).
- Play any level straight from the editor, with test cheats.
- Music: MIDI to song, AddmusicK insertion, original vs custom song detection, play any song.
- Patches (Asar) and per-frame code (UberASM) switched on and off, with exact undo and safety checks.
- Status bar designer, Toolbox packs, taking levels and songs from other hacks, overworld view.
- Saves in Lunar Magic's ROM format, so hacks open in both editors.

Not yet: overworld editing, Map16 and graphics editors, custom sprites/blocks (PIXI, GPS), macOS/Linux, a browser version. Some jobs still use your own installed copy of Lunar Magic. You need your own copy of Super Mario World (USA).

## Guide

The same guide is in the app's **Docs** tab (Help → Docs, or F1):

1. [Getting started](docs/guide/01-getting-started.md)
2. [Editing levels](docs/guide/02-editing-levels.md)
3. [Layers](docs/guide/03-layers.md)
4. [Sprites](docs/guide/04-sprites.md)
5. [Entrances, midway points and exits](docs/guide/05-entrances-and-exits.md)
6. [Level settings](docs/guide/06-level-settings.md)
7. [Test Play](docs/guide/07-test-play.md)
8. [The Toolbox](docs/guide/08-toolbox.md)
9. [Music](docs/guide/09-music.md)
10. [Engine upgrades](docs/guide/10-engine-upgrades.md)
11. [Saving and sharing](docs/guide/11-saving-and-sharing.md)
12. [Keyboard shortcuts](docs/guide/12-shortcuts.md)

See [docs/PLAN.md](docs/PLAN.md) for the architecture and milestones.

Lunar Magic is by FuSoYa and is not part of SMW Studio. Some features run your own installed copy of Lunar Magic; SMW Studio includes none of its code.
