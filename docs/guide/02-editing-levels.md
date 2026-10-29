# Editing levels

Levels are made of **objects**: ground, pipes, blocks, coins, slopes. Each object has a number, a position and a size. Pick a tool in the top bar, then work in the Level panel.

## The tools

- **Select** (`V`): click something to select it. Drag to move it, `Ctrl+drag` to copy it, arrow keys nudge it, `Delete` removes it, `Esc` deselects.
- **Draw** (`B`): paints the object you picked. Click, or drag to paint a row.
- **Fill** (`G`): drag a box; it fills with copies of the object you picked.
- **Erase** (`E`): click or drag over objects and sprites to remove them.
- **Sprites** (`S`): paints the enemy or item you picked.

While you drag, a preview shows what will happen (blue for placing, red for erasing). It's done when you let go, and **each drag is one undo step**.

## Tutorial: paint a row of coins

1. Open the **Objects** panel (next to Toolbox) and click a coin. The tool switches to **Draw**.
2. Move over the level: a see-through coin follows the mouse.
3. Drag across the sky. Coins appear along your drag when you let go.
4. Didn't like it? **Undo** (`Ctrl+Z`) removes the whole row.

**Shortcut:** with Draw or Fill and nothing picked, click something already in the level to pick it ("eyedropper").

## Tutorial: fill a wall of blocks

1. Pick a block in the **Objects** panel (or click one in the level with Draw).
2. Press `G` for **Fill**.
3. Drag a box. The outline shows the area, copies show where blocks go.
4. Let go.

## Changing an object

Select it, then in **Properties**:

- **Object number** changes what it is.
- **Size / settings** changes how big it is (for most objects: width and height).

## Right-click menu

Right-click an object for **Duplicate** and **Delete**, plus things for that spot: **Mario starts here**, **Midway restart on this screen**, and moving secondary entrances.

## Level length

In **Properties → Level**, **Length (screens)** has **−** and **+**. A horizontal level can be 1 to 32 screens long.

## Undo and redo

`Ctrl+Z` / `Ctrl+Y`, or **Edit → Undo / Redo**, or the ↶ ↷ buttons. Each level has its own history.

Next: [Layers](03-layers.md)
