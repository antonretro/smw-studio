# Entrances, midway points and exits

## Where Mario starts

The red **M** marker is the **main entrance**: where Mario appears when he enters the level from the map, or from a pipe that leads to this level.

**Tutorial: move the start**

1. Right-click an empty spot (or anything) in the level.
2. Choose **Mario starts here**.

Or set it in **Properties → Entrances**: screen, across, height, and how Mario enters (walking in, or coming out of a pipe going left / right / up / down...).

## Midway point

The blue **½** marker is where Mario restarts after touching the midway tape and dying. It's on the **midway screen**, at the same spot and height as the main entrance.

- Right-click → **Midway restart on this screen**, or
- **Properties → Entrances → Midway point → Screen**.

To have several checkpoints, see [Engine upgrades](10-engine-upgrades.md).

## Screen exits: where pipes and doors go

Every pipe and door on a screen uses that **screen's exit**. Each screen with an exit shows a label like **⇢ level 106** in the level.

**Tutorial: make a pipe lead somewhere**

1. In **Properties → Screen exits**, choose **Add an exit on screen…** and pick the screen the pipe is on.
2. Choose **goes to a level** and pick the level (by number and name).
3. Mario arrives at that level's main entrance.

## Secondary entrances: arrive somewhere else

Use a **secondary entrance** when Mario should come out somewhere other than the level's start, for example a pipe that leads back out of a bonus room.

**Tutorial: a pipe to a bonus room and back**

1. Open the bonus room level. In **Properties → Entrances**, click **+ New secondary entrance into this level**.
2. Move the new entrance where Mario should appear (right-click → **Move entrance … here**).
3. Open the level with the pipe. In **Screen exits**, set the pipe's screen to **goes to a secondary entrance** and pick the one you made. The list says which level each one leads to.

**Note:** in a ROM that hasn't been saved by Lunar Magic yet, exits can only lead to levels in the same half (000-0FF or 100-1FF), and "secondary" is one setting for the whole level. SMW Studio tells you when this applies.

Next: [Level settings](06-level-settings.md)
