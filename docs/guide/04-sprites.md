# Sprites

Sprites are everything that moves or can be picked up: enemies, items, springboards, platforms.

## Tutorial: add enemies

1. Open the **Sprites** panel. Every sprite is listed by name and drawn the way it looks in this level.
2. Click one (or search, for example "koopa"). The tool switches to **Sprites**.
3. A see-through copy follows the mouse, exactly where it will land.
4. Click to place one, or drag to place several.

**Shortcut:** with the Sprites tool and nothing picked, click a sprite in the level to pick it.

## Moving, copying, changing

- With **Select**, drag a sprite to move it, `Ctrl+drag` to copy, arrow keys to nudge, `Delete` to remove.
- With a sprite selected, **Properties** lets you change which sprite it is and its extra setting (for example a Koopa's colour or which way something faces).

## Why does an enemy look garbled?

Each level loads graphics for one group of enemies (**Level settings → Enemy graphics**: Forest, Castle, Mushroom...). Enemies from other groups show the wrong graphics in the game too. Pick the group that has the enemies you want.

## Limits of the original game

- About **12 sprites** can be active on screen at once. Place more and some won't appear until others are gone.
- Carryable items (springboards, shells, blocks) can't stand on each other.

Some of these limits can be lifted with [Engine upgrades](10-engine-upgrades.md).

Next: [Entrances, midway points and exits](05-entrances-and-exits.md)
