# Getting started

SMW Studio edits Super Mario World ROMs: the levels, enemies, music and more. You see the game as it really looks, and you can play your changes inside the editor.

## What you need

- A **Super Mario World (USA)** ROM file (`.sfc` or `.smc`) that you own.
- That's it. SMW Studio works on clean ROMs and on hacks made with Lunar Magic.
- **SA-1 hacks** (hacks that use the cartridge's extra SA-1 chip, like *The Twelve Magic Orbs Powered-Up*) work too: their levels draw exactly like the game, you can edit and save them, and **Play** jumps straight into any level. ROM expanding isn't available for them.

## Tutorial: open your first ROM

1. Click **File → Open ROM…** (or press `Ctrl+O`).
2. Pick your `.sfc` / `.smc` file.
3. The **Output** panel at the bottom says what kind of ROM it is: *Clean Super Mario World (USA)*, *Modified SMW* or *Modified SMW (edited with Lunar Magic)*.
4. Level **105** (Yoshi's Island 1) opens. Pick any other level in the **Explorer** on the left. Each level shows its number and its name from the original game.

SMW Studio remembers your ROM: next time it starts (or reloads), it reopens it. You can turn that off in **Edit → Preferences**.

## The window

Every part of the window is a panel you can drag, dock, close and reopen from the **Window** menu. **Window → Reset layout** (or the button in Preferences) puts everything back.

- **Explorer**: the list of levels.
- **Level**: the level itself. Scroll to move around, `Ctrl+scroll` or **− / +** to zoom.
- **Properties**: what's selected, the level's settings, screen exits and entrances.
- **Toolbox / Objects / Sprites**: things to place, and packs you've collected.
- **Layers**: show, hide and lock parts of the level.
- **Music**: songs you made from MIDI files.
- **Output**: what SMW Studio did, step by step.
- **Docs**: this guide.

## Your work is safe

- The first time you save, SMW Studio copies your ROM to `<name>.sfc.bak` next to it.
- Before anything big (adding a Toolbox level, inserting music), the ROM is copied into a **SMW Studio Backups** folder next to it.
- Nothing changes on disk until you **Save** (`Ctrl+S`). The bottom bar says *Unsaved changes* or *All changes saved*.

Next: [Editing levels](02-editing-levels.md)
