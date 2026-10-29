# Music

The SNES plays music on its sound chip: up to **8 parts** at once, each one note at a time, with short recorded instruments called samples. SMW Studio uses **AddmusicK**, the standard tool for Super Mario World music.

## Tutorial: add a song from SMW Central

1. Download a song (`.zip`) from SMW Central's music section.
2. Drop it on the **Toolbox** (or **+ Import packs…**).
3. On its card, click **Add song**. It becomes song **29** (or the next free number).
4. Open a level and pick it in **Level settings → Music → Your songs ★**.
5. **▶ Play** to hear it.

## Replace one of the game's songs

**Replace a song…** on a song pack, then choose the song. It plays everywhere the old one did. The old song isn't lost: it moves to the next free number, so you can still pick it.

## Tutorial: make a song from MIDI files

1. Open **Window → Music** (or the Toolbox) and click **♪ Import MIDI…**.
2. Pick **all the parts** of the song together: one `.mid` per instrument works best (Bass, Drums, Keyboard...), or one `.mid` with several tracks.
3. The song opens in the **Music** tab as a piano roll, one colour per part.
4. Press **▶ Play** to hear it. This uses a scratch copy of your ROM, so your ROM isn't changed.
5. Adjust each part:
   - the tick box plays it or not (at most 8);
   - pick its **instrument**;
   - **− / +** moves it an octave down or up;
   - **Vol** sets its loudness (left = automatic).
6. Click **Put in ROM** (or **Update song 29 in ROM** after changes), then pick it in a level's Music setting.

### How MIDI becomes SNES music

- The melody, bass and drums are always kept first; then the parts that play the most.
- Chords become their top note (the lowest for bass).
- Drums use the game's kick, snare, hi-hat, cymbal and toms.
- Notes too high or low for the samples move by octaves.
- The result is a normal AddmusicK song, so it can be fine-tuned by hand.

## Songs in this ROM

In the **Music** tab, open **Songs in this ROM** and click **Check songs** to see every song a ROM has (any ROM with AddmusicK music, including other people's hacks):

- **Original**: the original game's song, unchanged.
- **Edited original**: an original song with changes.
- **Custom**: a new song. Custom songs are named after the game and the first level that plays them, like *My Hack – 1-1*.

Each row lists the levels that play it. Click **▶** to hear a song (any of them, even ones no level uses); your ROM isn't changed.

## Where files go

Each ROM gets an **AddmusicK** folder next to it with the songs and their instruments. SMW Studio keeps the song list there and backs up the ROM before every insert.

Next: [Engine upgrades](10-engine-upgrades.md)
