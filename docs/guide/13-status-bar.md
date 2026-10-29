# The status bar (UI tab)

The **UI** tab (**Window → UI**) shows the status bar at the top of the screen in levels, exactly as the game draws it with the open level's graphics and colours.

## Tutorial: build a bar from pieces

Under **Drag onto the bar** are ready-made pieces, drawn with your level's graphics:

- **Coins**, **Lives**, **Time**, **Score**, **Dragon coins**: a counter with its icon or label. Dropping one moves that counter there (its old icon is cleared) and shows it if it was hidden.
- **MARIO**, **Coin icon**, **Star icon**, **×**, **-**: decorations.

Drag a piece onto the bar: a green outline shows where it lands. Counters go on the two middle lines, the score on the lower one; SMW Studio tells you if a piece doesn't fit or would cover another counter.

## Presets

Under **Start from a preset**, pick a ready-made bar, each shown with your level's graphics:

- **Classic**: the original game's bar.
- **Minimal**: only coins and lives, in the corners.
- **One line**: lives, time and coins across one line.
- **Arcade**: a big score on the left.
- **Adventure**: the item box in the middle and no score.
- **No status bar**: a clean screen (everything still counts).

A preset replaces the whole bar; change anything after with the designer below it.

## Tutorial: hide parts of the status bar

1. Open **Window → UI**.
2. Hover a part in the list (*Lives*, *Coins*, *Time*, *Score*…) to see where it is on the bar.
3. Untick it to hide it. Tick it again to bring it back.
4. Press **Save**. Press **Play** to see it in the game.

Hidden parts only disappear from the screen: the counters keep counting (coins still give 1-ups, the timer still runs out). Hiding the **Item box** also hides the item inside it, but Select still drops the reserve item.

This works on a real SNES: the game blanks the hidden parts right before it draws the bar each frame.

## Tutorial: design your own status bar

1. Open **Window → UI**.
2. Click a square on the bar. **Shift+click** another square to pick the whole block between them.
3. Change what's picked:
   - **Click a tile** in the tile picker to put it there (the picker shows the tiles in the colours you chose last).
   - **Colours 0–7** pick one of the 8 colour sets.
   - **↔ Flip / ↕ Flip** mirror tiles; **Clear** empties them.
   - **Type text** (letters, numbers and . , - ! ×) and press **Write**: it starts at the first picked square.
4. Press **Save**, then **Play** to see it in the game.

Squares with a yellow line are counters (lives, coins, time, score, dragon coins, bonus stars): the game writes their numbers every frame, so you can change their colours and flips but not their tiles. The greyed squares at the far left and right are never drawn by the game.

## Tutorial: move a counter

1. Click one of the counter's squares (the yellow-lined ones): coins, lives, time, score or dragon coins.
2. Press **Move coins…** (or lives, time…).
3. Click the square where its first digit should go. Its old squares are cleared and the new ones take its colours.

Counters go on the two middle lines; the score stays on the lower line (the game's score code only reaches there). A counter can't go on top of another one. Move the labels that go with it (like the coin icon) with the designer.

The top and bottom rows (the item box's edges in the original game) become full width the first time you change them outside the box.

## Tutorial: put sprites on the bar

Sprites can go anywhere on the bar (not just on the 8×8 squares) and use the game's sprite pictures: items, coins, enemies and more. Up to 6.

1. Under **Sprites on the bar**, pick **Colours** and click a picture in **Sprite pictures** (16×16 by default; untick it for 8×8).
2. Click the bar where it should go.
3. To change one, pick it in the list (**Sprite 1**, **Sprite 2**…): click the bar to move it, use the arrows to nudge it a pixel, flip it, change its size or colours, click another picture, or **Remove** it.

The pictures come from the level's sprite graphics: the ones every level has (items, coins, the star…) always look right; enemy pictures depend on the level's enemy graphics. The item box still shows its item.

## Limits

- SA-1 hacks keep their status bar elsewhere; the UI tab can't change them yet.
- If another patch already changed the status bar code, SMW Studio leaves it alone and says so.

Next: [Shortcuts](12-shortcuts.md)
