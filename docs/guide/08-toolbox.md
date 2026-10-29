# The Toolbox

The Toolbox keeps **packs**: tilesets, backgrounds, graphics and songs you've downloaded (for example from SMW Central). They stay in your Toolbox for every hack, with credit to whoever made them.

## Tutorial: add a tileset or background as a new level

1. Download a pack (`.zip`).
2. In the **Toolbox** panel, click **+ Import packs…**, or drop the `.zip` onto the window.
3. The pack shows a preview, what kind it is, and who made it (click **add creator** to fill that in).
4. Click **Add as level…**, pick a free level number, and confirm.
5. The level opens with the pack's graphics. Press **▶ Play** to try it.

## Other kinds of packs

- **Graphics replacement**: **Use in whole game** swaps the game's own graphics files.
- **Music**: **Add song** / **Replace a song…** (see [Music](09-music.md)).

## Tutorial: take a level from another hack

1. In the **Toolbox**, click **⇩ From a hack…** and pick the hack's ROM file. Your copy of the hack is never changed.
2. The first time, SMW Studio takes the hack apart with Lunar Magic (about a minute). After that it opens straight away.
3. Its levels appear with pictures. Click one.
4. Pick one of your free level numbers and click **Bring it in**. The level opens.

What comes with it:

- **The level**, its layout, settings and colours.
- **The graphics it uses.** If a number is already taken in your ROM by different graphics, it gets a new number, so nothing of yours is replaced. Identical files are shared.
- **The blocks it uses** (Map16 pages), only pages the original game leaves empty. Pages you've filled with your own blocks, and the game's standard block pages, are left alone; the Output panel says which.

What doesn't come: the hack's own enemies (custom sprites), blocks with their own code, and patches such as a custom status bar. Those are separate code.

Please credit the hack's makers if you share something that uses their work.

## How it works

Adding levels and graphics uses **Lunar Magic** in the background, so the ROM stays compatible with every other tool. Lunar Magic keeps a clean copy of the game in a hidden **sysLMRestore** folder next to your ROM. SMW Studio puts it there itself (from a clean ROM you've opened before, or the `.bak` next to your ROM), so Lunar Magic never stops to ask for it.

**Preferences** shows whether Lunar Magic was found.

Next: [Music](09-music.md)
