import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { log, refreshDirty, useStudio } from "./store";

type View = {
  problem: string;
  parts: [string, string, boolean][];
  picture: string;
  cells: [string, [number, number, number][]][];
};

type Cell = { row: number; col: number; entry: number; editable: boolean; live: string | null };

const SCALE = 3;
const TILE_SCALE = 2;

type Sticker = { x: number; y: number; tile: number; pal: number; flip_x: boolean; flip_y: boolean; big: boolean };
const MAX_STICKERS = 6;

/** Sprites on the status bar: pick a sprite picture, click the bar to put it there. */
function StickerEditor({ picture, levelNo, locked, run, version }: {
  picture: string;
  levelNo: number;
  locked: boolean;
  run: (what: () => Promise<unknown>, done: string) => void;
  version: number;
}) {
  const [list, setList] = useState<Sticker[]>([]);
  const [sel, setSel] = useState<number | null>(null);
  const [pal, setPal] = useState(4);
  const [big, setBig] = useState(true);
  const [tile, setTile] = useState(0x24);
  const [tiles, setTiles] = useState("");
  useEffect(() => { invoke<Sticker[]>("statusbar_stickers").then(setList).catch(() => setList([])); }, [version]);
  useEffect(() => { invoke<string>("statusbar_sprite_tiles", { level: levelNo, palette: pal }).then(setTiles).catch(() => setTiles("")); }, [levelNo, pal]);

  const save = (next: Sticker[], what: string) => run(() => invoke("statusbar_set_stickers", { stickers: next }), what);
  const current = sel !== null ? list[sel] : null;
  const update = (patch: Partial<Sticker>, what: string) => {
    if (sel === null) return;
    save(list.map((s, i) => (i === sel ? { ...s, ...patch } : s)), what);
  };

  // Clicking the bar: move the picked sprite there, or add a new one (centred on the click)
  const clickBar = (e: React.MouseEvent<HTMLDivElement>) => {
    if (locked) return;
    const r = e.currentTarget.getBoundingClientRect();
    const size = (current ? current.big : big) ? 16 : 8;
    const x = Math.max(0, Math.min(255, Math.round((e.clientX - r.left) / SCALE - size / 2)));
    const y = Math.max(0, Math.min(239, Math.round((e.clientY - r.top) / SCALE - size / 2) + 8));
    if (current) {
      update({ x, y }, "Sprite moved");
    } else if (list.length < MAX_STICKERS) {
      const next = [...list, { x, y, tile, pal, flip_x: false, flip_y: false, big }];
      setSel(next.length - 1);
      save(next, "Sprite added");
    } else {
      log(`UI: at most ${MAX_STICKERS} sprites fit on the bar`);
    }
  };

  return (
    <div className="ui-stickers">
      <p className="muted">Sprites can go anywhere on the bar and use the game's sprite pictures (items, coins, enemies…), up to {MAX_STICKERS}. Pick a picture, then click the bar. Pick a sprite in the list to move it (click the bar) or change it.</p>
      <div className="ui-bar ui-sticker-bar" style={{ width: 256 * SCALE, height: 32 * SCALE }} onClick={clickBar}>
        <img src={picture} width={256 * SCALE} height={32 * SCALE} alt="Status bar" />
        {list.map((s, i) => (
          <div key={i} className={"ui-sticker" + (i === sel ? " sel" : "")}
            style={{ left: s.x * SCALE, top: (s.y - 8) * SCALE, width: (s.big ? 16 : 8) * SCALE, height: (s.big ? 16 : 8) * SCALE }} />
        ))}
      </div>
      <div className="ui-designer">
        <div className="ui-tools">
          <div className="ui-row">
            <button className={"menu" + (sel === null ? " on" : "")} onClick={() => setSel(null)}>+ New sprite</button>
            {list.map((_, i) => (
              <button key={i} className={"menu" + (i === sel ? " on" : "")} onClick={() => setSel(i)}>Sprite {i + 1}</button>
            ))}
          </div>
          {current ? (
            <>
              <b>Sprite {sel! + 1} at {current.x}, {current.y}</b>
              <div className="ui-row">
                <button className="menu" disabled={locked} onClick={() => update({ x: Math.max(0, current.x - 1) }, "Sprite moved")}>←</button>
                <button className="menu" disabled={locked} onClick={() => update({ x: Math.min(255, current.x + 1) }, "Sprite moved")}>→</button>
                <button className="menu" disabled={locked} onClick={() => update({ y: Math.max(0, current.y - 1) }, "Sprite moved")}>↑</button>
                <button className="menu" disabled={locked} onClick={() => update({ y: current.y + 1 }, "Sprite moved")}>↓</button>
                <button className="menu" disabled={locked} onClick={() => update({ flip_x: !current.flip_x }, "Sprite flipped")}>↔ Flip</button>
                <button className="menu" disabled={locked} onClick={() => update({ flip_y: !current.flip_y }, "Sprite flipped")}>↕ Flip</button>
                <button className="menu" disabled={locked} onClick={() => update({ big: !current.big }, "Sprite size changed")}>{current.big ? "16×16" : "8×8"}</button>
                <button className="menu" disabled={locked} onClick={() => { const next = list.filter((_, i) => i !== sel); setSel(null); save(next, "Sprite removed"); }}>Remove</button>
              </div>
            </>
          ) : (
            <b>{list.length < MAX_STICKERS ? "Pick a picture, then click the bar" : `All ${MAX_STICKERS} sprites used`}</b>
          )}
          <div className="ui-row">
            <span>Colours</span>
            {[0, 1, 2, 3, 4, 5, 6, 7].map((p) => (
              <button key={p} className={"menu" + ((current ? current.pal : pal) === p ? " on" : "")} disabled={locked}
                onClick={() => { setPal(p); if (current) update({ pal: p }, `Sprite colours ${p}`); }}>{p}</button>
            ))}
          </div>
          {!current && (
            <label><input type="checkbox" checked={big} onChange={(e) => setBig(e.target.checked)} /> 16×16 (else 8×8)</label>
          )}
        </div>
        <div className="ui-tiles">
          <span className="muted">Sprite pictures (colours {pal}): click one{current ? " to use it for this sprite" : ""}</span>
          {tiles && (
            <div className="ui-sprite-tiles">
              <img src={tiles} width={128 * TILE_SCALE} height={256 * TILE_SCALE} alt="Sprite pictures"
                onClick={(e) => {
                  const r = (e.target as HTMLImageElement).getBoundingClientRect();
                  const t = Math.floor((e.clientY - r.top) / (8 * TILE_SCALE)) * 16 + Math.floor((e.clientX - r.left) / (8 * TILE_SCALE));
                  setTile(t);
                  if (current) update({ tile: t, pal }, `Sprite picture ${t.toString(16).toUpperCase()}`);
                }} />
              <div className="ui-pick" style={{ left: (tile % 16) * 8 * TILE_SCALE, top: Math.floor(tile / 16) * 8 * TILE_SCALE, width: (big ? 16 : 8) * TILE_SCALE, height: (big ? 16 : 8) * TILE_SCALE }} />
            </div>
          )}
        </div>
      </div>
    </div>
  );
}

/** Status bar font: tile number for each character ("" = blank) */
function charTile(ch: string): number | null {
  const c = ch.toUpperCase();
  if (c >= "0" && c <= "9") return c.charCodeAt(0) - 48;
  if (c >= "A" && c <= "Z") return 0x0a + c.charCodeAt(0) - 65;
  return ({ " ": 0xfc, ".": 0x24, ",": 0x25, "×": 0x26, "*": 0x26, "-": 0x27, "!": 0x28 } as Record<string, number>)[c] ?? null;
}

/** UI tab: the in-game status bar. Show or hide parts, and design it cell by cell. */
export default function UiPanel() {
  const { rom, level, loading } = useStudio();
  const [view, setView] = useState<View | null>(null);
  const [cells, setCells] = useState<Cell[]>([]);
  const [hover, setHover] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  // Designer: selected cells (row*32+col), the palette shown in the tile picker, typed text
  const [sel, setSel] = useState<number[]>([]);
  const [anchor, setAnchor] = useState<number | null>(null);
  const [pal, setPal] = useState(6);
  const [tiles, setTiles] = useState("");
  const [text, setText] = useState("");
  // Moving a counter: its part id, until a square is clicked
  const [moving, setMoving] = useState<string | null>(null);
  const levelNo = level?.level ?? 0x105;

  // Bumped after every reload, so parts that keep their own copy (sprites) reload too
  const [version, setVersion] = useState(0);
  const load = async () => {
    try {
      setView(await invoke<View>("statusbar_view", { level: levelNo }));
      setCells(await invoke<Cell[]>("statusbar_cells"));
      setVersion((v) => v + 1);
    } catch (e) {
      setView(null);
      log(`UI: ${e}`);
    }
  };
  useEffect(() => { if (rom) load(); else setView(null); }, [rom?.path, levelNo]);
  useEffect(() => {
    if (!rom) return;
    invoke<string>("statusbar_tiles", { level: levelNo, palette: pal }).then(setTiles).catch(() => setTiles(""));
  }, [rom?.path, levelNo, pal]);
  // Ready-made bars, pictured in this ROM
  const [presets, setPresets] = useState<{ id: string; name: string; about: string; picture: string }[]>([]);
  useEffect(() => {
    if (!rom) return;
    invoke<typeof presets>("statusbar_presets", { level: levelNo }).then(setPresets).catch(() => setPresets([]));
  }, [rom?.path, levelNo]);
  // Pieces to drag onto the bar, and the one being dragged (with where it would land)
  const [elements, setElements] = useState<{ id: string; name: string; width: number; picture: string }[]>([]);
  const [dragging, setDragging] = useState<{ id: string; width: number } | null>(null);
  const [dropAt, setDropAt] = useState<number | null>(null);
  useEffect(() => {
    if (!rom) return;
    invoke<typeof elements>("statusbar_elements", { level: levelNo }).then(setElements).catch(() => setElements([]));
  }, [rom?.path, levelNo]);
  const drop = (i: number) => {
    if (!dragging) return;
    const el = elements.find((e) => e.id === dragging.id);
    setDragging(null);
    setDropAt(null);
    run(() => invoke("statusbar_place", { id: dragging.id, row: Math.floor(i / 32) + 1, col: i % 32 }), `${el?.name ?? "Element"} placed`);
  };

  const run = async (what: () => Promise<unknown>, done: string) => {
    if (busy) return;
    setBusy(true);
    try {
      await what();
      await refreshDirty();
      log(`UI: ${done} (saved with your next save)`);
      await load();
    } catch (e) {
      log(`UI: ${e}`);
    } finally {
      setBusy(false);
    }
  };

  const toggle = (id: string) => {
    if (!view) return;
    const hidden = view.parts.filter(([pid, , h]) => (pid === id ? !h : h)).map(([pid]) => pid);
    const name = view.parts.find(([pid]) => pid === id)?.[1];
    run(() => invoke("statusbar_set", { hidden }), `${name} ${hidden.includes(id) ? "hidden" : "shown"}`);
  };

  const at = (i: number) => cells.find((c) => (c.row - 1) * 32 + c.col === i);
  const selected = sel.map(at).filter((c): c is Cell => !!c && c.editable);
  const first = selected[0];

  /** Change the selected cells with `f` (live cells keep their tile: the game writes it) */
  const change = (f: (e: number, c: Cell) => number, what: string) => {
    const changes = selected.map((c) => {
      let e = f(c.entry, c);
      if (c.live) e = (e & ~0x3ff) | (c.entry & 0x3ff);
      return [c.row, c.col, e] as [number, number, number];
    });
    if (changes.length) run(() => invoke("statusbar_set_cells", { changes }), what);
  };

  const partName = (id: string) => view?.parts.find(([pid]) => pid === id)?.[1] ?? id;

  const clickCell = (i: number, shift: boolean) => {
    if (moving) {
      const id = moving;
      setMoving(null);
      run(() => invoke("statusbar_move", { id, row: Math.floor(i / 32) + 1, col: i % 32 }), `${partName(id)} moved`);
      setSel([i]);
      return;
    }
    if (shift && anchor !== null) {
      const [a, b] = [Math.min(anchor, i), Math.max(anchor, i)];
      const [r0, r1, c0, c1] = [Math.floor(a / 32), Math.floor(b / 32), Math.min(a % 32, b % 32), Math.max(a % 32, b % 32)];
      const out: number[] = [];
      for (let r = r0; r <= r1; r++) for (let c = c0; c <= c1; c++) out.push(r * 32 + c);
      setSel(out);
    } else {
      setSel([i]);
      setAnchor(i);
    }
  };

  const typeText = () => {
    if (!first || !text) return;
    const start = (first.row - 1) * 32 + first.col;
    const changes: [number, number, number][] = [];
    [...text].forEach((ch, k) => {
      const c = at(start + k);
      const t = charTile(ch);
      if (!c || !c.editable || c.live || t === null || c.row !== first.row) return;
      changes.push([c.row, c.col, (c.entry & ~0x3ff) | t]);
    });
    if (changes.length) run(() => invoke("statusbar_set_cells", { changes }), `Wrote "${text}"`);
  };

  if (!rom) return <div className="panel ui-panel"><p className="muted">Open a ROM to change its status bar.</p></div>;
  if (!view) return <div className="panel ui-panel"><p className="muted">Loading…</p></div>;
  const hoverCells = view.cells.find(([id]) => id === hover)?.[1] ?? [];
  const locked = !!view.problem || busy || loading;
  return (
    <div className="panel ui-panel">
      <h3>Status bar</h3>
      <p className="muted">The bar at the top of the screen in levels, as the game shows it. Click a square to change it (Shift+click for a block), or untick a part to hide it. Everything here works on a real SNES.</p>
      <div className="ui-bar" style={{ width: 256 * SCALE, height: 32 * SCALE }}>
        <img src={view.picture} width={256 * SCALE} height={32 * SCALE} alt="Status bar" />
        {hoverCells.map(([row, a, b], i) => (
          <div key={i} className="ui-cell" style={{ left: a * 8 * SCALE, top: row * 8 * SCALE, width: (b - a + 1) * 8 * SCALE, height: 8 * SCALE }} />
        ))}
        {cells.map((c) => {
          const i = (c.row - 1) * 32 + c.col;
          return (
            <div
              key={i}
              className={"ui-grid" + (moving ? " target" : "") + (sel.includes(i) ? " sel" : "") + (!c.editable ? " fixed" : c.live ? " live" : "")}
              style={{ left: c.col * 8 * SCALE, top: (c.row - 1) * 8 * SCALE, width: 8 * SCALE, height: 8 * SCALE }}
              title={!c.editable ? "The game doesn't draw here" : c.live ? `The game writes the ${c.live} counter here: you can change its colour and flips` : `Tile ${(c.entry & 0x3ff).toString(16).toUpperCase()}, colours ${(c.entry >> 10) & 7}`}
              onClick={(e) => !locked && clickCell(i, e.shiftKey)}
              onDragOver={(e) => { if (dragging && !locked) { e.preventDefault(); setDropAt(i); } }}
              onDrop={(e) => { e.preventDefault(); drop(i); }}
            />
          );
        })}
        {dragging && dropAt !== null && (
          <div className="ui-ghost" style={{ left: (dropAt % 32) * 8 * SCALE, top: Math.floor(dropAt / 32) * 8 * SCALE, width: dragging.width * 8 * SCALE, height: 8 * SCALE }} />
        )}
      </div>
      {view.problem && <p className="ui-problem">{view.problem}</p>}

      {!view.problem && elements.length > 0 && (
        <>
          <h4>Drag onto the bar</h4>
          <div className="ui-elements">
            {elements.map((el) => (
              <div key={el.id} className="ui-element" draggable={!locked}
                onDragStart={(e) => { e.dataTransfer.setData("text/plain", el.id); e.dataTransfer.effectAllowed = "copy"; setDragging({ id: el.id, width: el.width }); }}
                onDragEnd={() => { setDragging(null); setDropAt(null); }}
                title={`${el.name}: drag it onto the bar`}>
                <img src={el.picture} width={el.width * 8 * 2} height={16} alt="" />
                <span>{el.name}</span>
              </div>
            ))}
          </div>
        </>
      )}

      {!view.problem && presets.length > 0 && (
        <>
          <h4>Start from a preset</h4>
          <div className="ui-presets">
            {presets.map((pr) => (
              <button key={pr.id} className="ui-preset" disabled={locked} title={pr.about}
                onClick={() => run(() => invoke("statusbar_apply_preset", { id: pr.id }), `Status bar: ${pr.name}`)}>
                <img src={pr.picture} width={256} height={32} alt="" />
                <b>{pr.name}</b>
                <span className="muted">{pr.about}</span>
              </button>
            ))}
          </div>
          <h4>Design it</h4>
        </>
      )}

      {!view.problem && (
        <div className="ui-designer">
          <div className="ui-tools">
            <b>{selected.length ? `${selected.length} square${selected.length > 1 ? "s" : ""} picked` : "Pick a square on the bar"}</b>
            <div className="ui-row">
              <span>Colours</span>
              {[0, 1, 2, 3, 4, 5, 6, 7].map((p) => (
                <button key={p} className={"menu" + (first && ((first.entry >> 10) & 7) === p ? " on" : "")} disabled={!selected.length || locked}
                  onClick={() => { setPal(p); change((e) => (e & ~0x1c00) | (p << 10), `Colours ${p}`); }}>{p}</button>
              ))}
            </div>
            <div className="ui-row">
              <button className="menu" disabled={!selected.length || locked} onClick={() => change((e) => e ^ 0x4000, "Flipped")}>↔ Flip</button>
              <button className="menu" disabled={!selected.length || locked} onClick={() => change((e) => e ^ 0x8000, "Flipped")}>↕ Flip</button>
              <button className="menu" disabled={!selected.length || locked} onClick={() => change((e) => (e & ~0x3ff) | 0xfc, "Cleared")}>Clear</button>
            </div>
            {first?.live && first.live !== "stars" && (
              <div className="ui-row">
                {moving ? (
                  <>
                    <span>Click where the first digit of the {partName(moving).toLowerCase()} should go</span>
                    <button className="menu" onClick={() => setMoving(null)}>Cancel</button>
                  </>
                ) : (
                  <button className="menu" disabled={locked} onClick={() => setMoving(first.live)}>Move {partName(first.live).toLowerCase()}…</button>
                )}
              </div>
            )}
            <div className="ui-row">
              <input className="ui-text" placeholder="Type text, e.g. WORLD 1" value={text} maxLength={30} onChange={(e) => setText(e.target.value)}
                onKeyDown={(e) => e.key === "Enter" && typeText()} />
              <button className="menu" disabled={!first || !text || locked} onClick={typeText}>Write</button>
            </div>
            <p className="muted">Text starts at the first picked square: letters, numbers and . , - ! ×</p>
          </div>
          <div className="ui-tiles">
            <span className="muted">Click a tile to put it in the picked squares (colours {pal})</span>
            {tiles && (
              <img src={tiles} width={128 * TILE_SCALE} height={256 * TILE_SCALE} alt="Tiles"
                onClick={(e) => {
                  if (!selected.length || locked) return;
                  const r = (e.target as HTMLImageElement).getBoundingClientRect();
                  const t = Math.floor((e.clientY - r.top) / (8 * TILE_SCALE)) * 16 + Math.floor((e.clientX - r.left) / (8 * TILE_SCALE));
                  change((x) => (x & ~0x3ff) | t, `Tile ${t.toString(16).toUpperCase()}`);
                }} />
            )}
          </div>
        </div>
      )}

      {!view.problem && (
        <>
          <h4>Sprites on the bar</h4>
          <StickerEditor picture={view.picture} levelNo={levelNo} locked={locked} run={run} version={version} />
        </>
      )}

      <h4>Show or hide</h4>
      <div className="ui-parts">
        {view.parts.map(([id, name, hidden]) => (
          <label key={id} onMouseEnter={() => setHover(id)} onMouseLeave={() => setHover(null)}>
            <input type="checkbox" checked={!hidden} disabled={locked} onChange={() => toggle(id)} /> {name}
          </label>
        ))}
      </div>
    </div>
  );
}
