# Engine upgrades

**Tools → Engine upgrades…** has switches that change how the game itself works, beyond what the original allows. Each one checks your ROM's code first: if another patch already changed that part of the game, the switch says why it can't be turned on instead of risking a broken ROM. They're saved with your next **Save**.

## Multiple checkpoints

In the original game a level has one midway point. With this upgrade, **every midway tape is a checkpoint**: after dying, Mario restarts at the **last tape he touched**, like Mario Maker.

**Tutorial: three checkpoints in one level**

1. **Tools → Engine upgrades… → Multiple checkpoints → Turn on**, then **Save**.
2. In the **Objects** panel, find the **midway point** tape (or pick the level's own tape with **Draw** by clicking it).
3. Place it in two more spots.
4. **Save**, play the level, touch the second tape, lose a life: you restart at the second tape.

Works in the original game and in Lunar Magic hacks, and on real SNES hardware: it's plain SNES code with no special chip, and it guards against the random memory a real SNES starts with.

### Technical details

- Touching a tape (block `$38`, code at `$00:F2C9`) calls a routine that sets the midway flag as before, then records the level (`$13BF`+1), the tape's X (`$9A`, snapped to the block), Mario's Y (`$96`) and a check value (level XOR `$FF`).
- The midway restart (the level loader's `STA $95 / JMP $DA17`, found in `$05:D9C0`-`$05:DA17` because Lunar Magic moves it) jumps to a routine that uses the recorded spot when the level and check value match.
- Memory: `$7F9F80`-`$7F9F85`. The original game, Lunar Magic and AddmusicK hacks leave `$7F9000`-`$7FBFFF` alone (checked with the `free_ram` example).
- Code goes in free space protected with RATS tags.

## Coming

- More enemies on screen at once.
- Stackable springboards, throw blocks and shells.
- More than one exit per screen.

## Patches

Patches are how the SMW community shares engine changes (a ground pound, wall kicks, new physics…): Asar `.asm` files, often in a `.zip`.

1. **Tools → Engine upgrades**, then **Add patch (.asm or .zip)…** and pick the file. It's kept in SMW Studio's library, ready for any ROM.
2. Press **Settings** to see what the patch lets you change (its `!settings`, with the author's notes). Free RAM it asks for is filled in for you.
3. Press **Turn on**. SMW Studio saves your work, backs up the ROM, applies the patch, and remembers every byte it changed.
4. **Turn off** puts those bytes back exactly. If something else has changed the same bytes since, SMW Studio says so instead of breaking the ROM.

Code meant for UberASM (no `org` lines, like most wall jump code) can't be applied yet: SMW Studio says so when you add it.

To change the status bar, see [The status bar (UI tab)](13-status-bar.md).

Next: [Saving and sharing](11-saving-and-sharing.md)
