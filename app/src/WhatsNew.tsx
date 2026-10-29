/** Release notes: shown once when a new version is first opened, and from Help → What's New. */

export const VERSION = "0.1.0";
export const STAGE = "Alpha";

type Release = { version: string; title: string; notes: string[] };

/** Newest first. Add one for every release. */
export const RELEASES: Release[] = [
  {
    version: "0.1.0",
    title: "First alpha",
    notes: [
      "Levels drawn by running the game itself: what you see is what the game does, Lunar Magic hacks included.",
      "Play any level straight from the editor, with cheats for testing.",
      "SA-1 hacks open, edit, save and play from the inside.",
      "Music: import MIDI, insert songs with AddmusicK, see which songs in any hack are original or custom, and hear them.",
      "Take levels and songs from other hacks.",
      "Status bar designer: presets, drag-in pieces, move counters, sprites on the bar.",
      "Patches you switch on and off, like a ground pound or a wall jump; switching off puts the ROM back exactly.",
      "The overworld, shown as the game draws it.",
      "Docs, tutorials and a first-run tour.",
    ],
  },
];

const SEEN_KEY = "smw-studio.seenVersion";

/** Whether this version's notes haven't been shown yet (and remembers that they now have). */
export function whatsNewDue(): boolean {
  try {
    if (localStorage.getItem(SEEN_KEY) === VERSION) return false;
    localStorage.setItem(SEEN_KEY, VERSION);
    return true;
  } catch {
    return false;
  }
}

export default function WhatsNew({ onClose }: { onClose: () => void }) {
  return (
    <>
      <div className="modal-backdrop" onMouseDown={onClose} />
      <div className="modal prefs whats-new">
        <h3>What's new in SMW Studio {VERSION} {STAGE}</h3>
        <p className="hint">This is an alpha: the editor works, and more is coming before the first full release (see Window → Features).</p>
        <div className="wn-list">
          {RELEASES.map((r) => (
            <div key={r.version}>
              <h4>{r.version} · {r.title}</h4>
              <ul>{r.notes.map((n) => <li key={n}>{n}</li>)}</ul>
            </div>
          ))}
        </div>
        <div className="modal-buttons">
          <button className="menu" onClick={onClose}>Got it</button>
        </div>
      </div>
    </>
  );
}
