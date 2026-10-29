import MusicPanel from "./MusicPanel";
import UiPanel from "./UiPanel";
import OverworldPanel from "./OverworldPanel";
import { EditMenu, FileMenu, Menu, PatchesDialog, PreferencesDialog, ToolsMenu, UpgradesDialog } from "./Menus";
import DocsPanel from "./DocsPanel";
import HackBrowser from "./HackBrowser";
import Welcome, { welcomeSeen } from "./Welcome";
import WhatsNew, { STAGE, whatsNewDue } from "./WhatsNew";
import { useEffect, useRef, useState } from "react";
import {
  DockviewReact,
  DockviewReadyEvent,
  type DockviewApi,
  IDockviewPanelProps,
  themeAbyss,
} from "dockview-react";
import "dockview-react/dist/styles/dockview.css";
import "./App.css";
import { BEFORE_RELEASE, BEYOND, FUTURE, LUNAR_MAGIC, percent, type Feature } from "./features";
import { openUrl } from "@tauri-apps/plugin-opener";
import { invoke } from "@tauri-apps/api/core";
import {
  deleteEntry,
  duplicateEntry,
  entryBox,
  entryLabel,
  hex,
  hex3,
  loadLevel,
  loadPalette,
  moveEntry,
  type PaletteItem,
  placeObject,
  openRom,
  redo,
  save,
  select,
  setBrush,
  selectSprite,
  spriteAt,
  canPick,
  setTool,
  isPlaying,
  brushBlocks,
  paintObjects,
  paintSprites,
  eraseThings,
  type Tool,
  setLayer,
  setActiveLayer,
  setLayerFade,
  type LayerId,
  addSprite,
  moveSprite,
  deleteSprite,
  setSprite,
  setSpriteBrush,
  loadSpritePalette,
  type SpriteItem,
  type ScreenExit,
  setExit,
  removeExit,
  type Entrance,
  type Entrances as EntrancesData,
  setMainEntrance,
  setSecondaryEntrance,
  loadMusic,
  loadPrefs,
  reopenLastRom,
  addSong,
  importMidi,
  setLevelSong,
  type MusicView,
  entrancePixel,
  nearestEntrance,
  setMidwayScreen,
  addSecondaryEntrance,
  setDragItem,
  setEntry,
  setScreens,
  loadHeader,
  setHeaderBits,
  type HeaderView,
  toggleSprites,
  undo,
  useStudio,
  testPlay,
  startPlay,
  stopPlay,
  restartPlay,
  playInput,
  setFrameSink,
  loadTestOptions,
  listPacks,
  importPacks,
  importDroppedFiles,
  removePack,
  setPackCreator,
  applyPack,
  log,
  type Pack,
  saveTestOptions,
  type TestOptions,
  levelNameOf,
  nameFromRom,
} from "./store";

// Transparent 1x1 image used to hide the browser's own drag picture
const EMPTY_DRAG = new Image();
EMPTY_DRAG.src = "data:image/gif;base64,R0lGODlhAQABAIAAAAAAAP///yH5BAEAAAAALAAAAAABAAEAAAIBRAA7";


const TOOLS: { id: Tool; label: string; key: string; help: string }[] = [
  { id: "select", label: "Select", key: "V", help: "Pick, move (drag), copy (Ctrl+drag) and change things" },
  { id: "draw", label: "Draw", key: "B", help: "Paint the object you picked (Objects panel, or click one in the level)" },
  { id: "fill", label: "Fill", key: "G", help: "Drag a box to fill it with the object you picked" },
  { id: "erase", label: "Erase", key: "E", help: "Click or drag over objects and sprites to remove them" },
  { id: "sprites", label: "Sprites", key: "S", help: "Paint the sprite you picked (Sprites panel, or click one in the level)" },
];

// Vanilla SMW uses levels 000-024 and 101-13B. Names come later from the ROM's own name table.
const LEVELS = [
  ...Array.from({ length: 0x25 }, (_, i) => i),
  ...Array.from({ length: 0x3b }, (_, i) => 0x101 + i),
];

function Explorer() {
  const { rom, level, loading } = useStudio();
  const [filter, setFilter] = useState("");
  const shown = LEVELS.filter((l) =>
    `${hex3(l)} ${levelNameOf(l) ?? ""}`.toLowerCase().includes(filter.toLowerCase()),
  );
  return (
    <div className="panel explorer">
      <input className="search" placeholder="Search levels…" value={filter} onChange={(e) => setFilter(e.target.value)} />
      <details open>
        <summary>Levels</summary>
        {!rom && <div className="tree-item muted">No ROM loaded</div>}
        {rom &&
          shown.map((l) => (
            <div
              key={l}
              className={"tree-item" + (level?.level === l ? " selected" : "")}
              onClick={() => !loading && loadLevel(l)}
            >
              <span className="lvl-num">{hex3(l)}</span> <span className="lvl-name" title={nameFromRom(l) ? "Name in this ROM" : levelNameOf(l) ? "Name in the original game" : undefined}>{levelNameOf(l) ?? ""}</span>
            </div>
          ))}
      </details>
      {["Overworld", "Graphics", "Map16", "Palettes", "Music", "Patches"].map((g) => (
        <details key={g}>
          <summary>{g}</summary>
          <div className="tree-item muted">Coming soon</div>
        </details>
      ))}
    </div>
  );
}

/** Cells to fill box [x0, y0, x1, y1] (blocks) with copies of a brush w x h blocks big. */
function fillCells(box: [number, number, number, number], [w, h]: [number, number]): [number, number][] {
  const [x0, y0, x1, y1] = [Math.min(box[0], box[2]), Math.min(box[1], box[3]), Math.max(box[0], box[2]), Math.max(box[1], box[3])];
  const out: [number, number][] = [];
  for (let y = y0; y <= y1; y += h) for (let x = x0; x <= x1; x += w) out.push([x, y]);
  return out.slice(0, 400);
}

type Drag = { index: number; startX: number; startY: number; dx: number; dy: number; copy: boolean };

function LevelCanvas() {
  const { rom, level, entries, selected, loading, showSprites, brush, dragItem, sprites, selectedSprite, spriteBrush, exits, entrances, layers, tool, spritePalette } = useStudio();
  const [spotMenu, setSpotMenu] = useState<{ px: number; py: number; x: number; y: number } | null>(null);
  // Entrance choices for the spot under a right-click (empty space or on top of an object)
  const spotItems = (px: number, py: number, close: () => void) => {
    if (!entrances || !level) return null;
    const screen = Math.floor(px / 256);
    return (
      <>
        <button onClick={() => { setMainEntrance(nearestEntrance(px, py, entrances, level.vertical, entrances.main.action, entrances.exact)); close(); }}>
          Mario starts here <span>main entrance</span>
        </button>
        {!level.vertical && screen <= 0x0f && (
          <button onClick={() => { setMidwayScreen(screen); close(); }}>
            Midway restart on this screen <span>screen {hex(screen)}</span>
          </button>
        )}
        {entrances.secondaries.map((s) => (
          <button key={s.number} onClick={() => { setSecondaryEntrance(s.number, nearestEntrance(px, py, entrances, level.vertical, s.entrance.action)); close(); }}>
            Move entrance {hex3(s.number)} here
          </button>
        ))}
      </>
    );
  };
  // What's about to be placed: the click-to-place brush or something dragged from the palette
  const ghost = brush ?? dragItem;
  const [cursor, setCursor] = useState<{ bx: number; by: number } | null>(null);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const [zoom, setZoom] = useState(() => loadPrefs().zoom);
  const [hover, setHover] = useState<number | null>(null);
  const [drag, setDrag] = useState<Drag | null>(null);
  const [menu, setMenu] = useState<{ index: number; x: number; y: number; px: number; py: number } | null>(null);
  const [spriteDrag, setSpriteDrag] = useState<Drag | null>(null);
  // A Draw / Fill / Erase / Sprites stroke in progress: cells painted (blocks), the box
  // being filled, and what's being erased
  const [stroke, setStroke] = useState<{ cells: [number, number][]; box?: [number, number, number, number]; objects: number[]; spr: number[] } | null>(null);
  const [spriteMenu, setSpriteMenu] = useState<{ index: number; x: number; y: number } | null>(null);

  // The level is three stacked canvases (back, layer 1, sprites) so layers can be hidden
  // or faded without redrawing
  const frontRef = useRef<HTMLCanvasElement>(null);
  const spriteRef = useRef<HTMLCanvasElement>(null);
  useEffect(() => {
    if (!level) return;
    for (const [ref, px] of [[canvasRef, level.back], [frontRef, level.front], [spriteRef, level.spriteLayer]] as const) {
      const c = ref.current;
      if (!c) continue;
      c.width = level.width;
      c.height = level.height;
      c.getContext("2d")!.putImageData(new ImageData(new Uint8ClampedArray(px), level.width, level.height), 0, 0);
    }
  }, [level]);
  const layerStyle = (id: LayerId): React.CSSProperties => ({
    visibility: layers[id].show ? "visible" : "hidden",
    opacity: layers.fade && layers.active !== null && layers.active !== id ? 0.3 : 1,
  });

  // Keyboard: Delete, arrows, Ctrl+Z/Y/S
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.target as HTMLElement).tagName === "INPUT") return;
      const ctrl = e.ctrlKey || e.metaKey;
      if (ctrl && e.key.toLowerCase() === "s") { e.preventDefault(); save(); return; }
      if (ctrl && e.key.toLowerCase() === "z") { e.preventDefault(); undo(); return; }
      if (ctrl && e.key.toLowerCase() === "y") { e.preventDefault(); redo(); return; }
      if (e.key === "Escape" && brush) { setBrush(null); return; }
      if (e.key === "Escape" && spriteBrush) { setSpriteBrush(null); return; }
      if (selectedSprite !== null) {
        const s = sprites[selectedSprite];
        if (!s) return;
        if (e.key === "Delete" || e.key === "Backspace") { e.preventDefault(); deleteSprite(selectedSprite); return; }
        if (e.key === "Escape") { selectSprite(null); return; }
        const step: Record<string, [number, number]> = { ArrowLeft: [-1, 0], ArrowRight: [1, 0], ArrowUp: [0, -1], ArrowDown: [0, 1] };
        const d = step[e.key];
        if (d) {
          e.preventDefault();
          moveSprite(selectedSprite, Math.max(0, s.x + d[0]), Math.max(0, s.y + d[1]));
        }
        return;
      }
      if (selected === null) return;
      const ent = entries[selected];
      if (!ent) return;
      if (e.key === "Delete" || e.key === "Backspace") { e.preventDefault(); deleteEntry(selected); return; }
      if (e.key === "Escape") { select(null); return; }
      if (ent.kind === "exit") return;
      const step: Record<string, [number, number]> = { ArrowLeft: [-1, 0], ArrowRight: [1, 0], ArrowUp: [0, -1], ArrowDown: [0, 1] };
      const d = step[e.key];
      if (d) {
        e.preventDefault();
        const [nx, ny] = clampPos(ent.x + d[0], ent.y + d[1]);
        moveEntry(selected, nx, ny);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [selected, entries, level, brush, spriteBrush, selectedSprite, sprites]);

  const clampPos = (x: number, y: number): [number, number] => {
    if (!level) return [x, y];
    const maxX = level.vertical ? 31 : level.screens * 16 - 1;
    const maxY = level.vertical ? level.screens * 16 - 1 : 26;
    return [Math.max(0, Math.min(maxX, x)), Math.max(0, Math.min(maxY, y))];
  };

  const blockAt = (e: React.MouseEvent) => {
    const r = canvasRef.current!.getBoundingClientRect();
    return { bx: Math.floor((e.clientX - r.left) / zoom / 16), by: Math.floor((e.clientY - r.top) / zoom / 16) };
  };
  const pixelAt = (e: React.MouseEvent) => {
    const r = canvasRef.current!.getBoundingClientRect();
    return { x: (e.clientX - r.left) / zoom, y: (e.clientY - r.top) / zoom };
  };
  const ownerAt = (bx: number, by: number) => {
    if (!level || bx < 0 || by < 0 || bx >= level.cols || by >= level.rows || !canPick("front")) return null;
    const o = level.owner[by * level.cols + bx];
    return o ? o - 1 : null;
  };

  if (!rom) {
    return (
      <div className="panel canvas">
        <div className="canvas-empty">
          <div className="logo">SMW Studio</div>
          <p>Open a Super Mario World ROM to start.</p>
          <button className="primary" onClick={openRom}>Open ROM…</button>
        </div>
      </div>
    );
  }

  const box = (index: number | null) => (level && index !== null ? entryBox(level, index) : null);
  const selBox = box(selected);
  const hovBox = hover !== selected ? box(hover) : null;
  const px = 16 * zoom;

  return (
    <div className="panel level-view">
      <div className="view-bar">
        <span>{level ? `Level ${hex3(level.level)}` : "Pick a level"}</span>
        {loading && <span className="muted">Working…</span>}
        <span className="spacer" />
        <label className="toggle">
          <input type="checkbox" checked={showSprites} onChange={toggleSprites} disabled={loading} /> Sprites
        </label>
        <button onClick={() => setZoom((z) => Math.max(0.5, z / 2))}>−</button>
        <span className="zoom">{zoom * 100}%</span>
        <button onClick={() => setZoom((z) => Math.min(8, z * 2))}>+</button>
      </div>
      <div
        className="level-scroll"
        onWheel={(e) => {
          if (e.ctrlKey) {
            e.preventDefault();
            setZoom((z) => (e.deltaY < 0 ? Math.min(8, z * 2) : Math.max(0.5, z / 2)));
          }
        }}
      >
        <div
          className={"level-stage tool-" + tool}
          style={level ? { width: level.width * zoom, height: level.height * zoom } : undefined}
          onMouseDown={(e) => {
            if (e.button !== 0 || loading) return;
            const { bx, by } = blockAt(e);
            if (tool !== "select") {
              const pt = pixelAt(e);
              if (tool === "erase") {
                const o = ownerAt(bx, by), si = spriteAt(pt.x, pt.y);
                setStroke({ cells: [[bx, by]], objects: o !== null && entries[o]?.kind !== "exit" ? [o] : [], spr: si !== null ? [si] : [] });
                return;
              }
              if (tool === "sprites") {
                if (!spriteBrush) {
                  // Nothing picked yet: click a sprite in the level to paint with it
                  const si = spriteAt(pt.x, pt.y);
                  const s = si !== null ? sprites[si] : null;
                  if (s) setSpriteBrush(spritePalette.find((p) => p.id === s.id) ?? { id: s.id, name: s.name, w: 16, h: 16, img: "", ox: 0, oy: 0 });
                  else log("Sprites tool: pick a sprite in the Sprites panel, or click one in the level.");
                  return;
                }
                setStroke({ cells: [clampPos(bx, by)], objects: [], spr: [] });
                return;
              }
              // Draw / Fill
              if (!brush) {
                const o = ownerAt(bx, by);
                const ent = o !== null ? entries[o] : null;
                if (ent && ent.kind !== "exit") {
                  setBrush({ kind: ent.kind === "extended" ? "extended" : "object", id: ent.id, settings: ent.settings, name: entryLabel(ent), desc: "", w: 16, h: 16, ox: 0, oy: 0, img: "" });
                  setTool(tool);
                } else log(`${tool === "draw" ? "Draw" : "Fill"} tool: pick an object in the Objects panel, or click one in the level.`);
                return;
              }
              const cell = clampPos(bx - brush.ox, by - brush.oy);
              setStroke(tool === "fill" ? { cells: [], box: [cell[0], cell[1], cell[0], cell[1]], objects: [], spr: [] } : { cells: [cell], objects: [], spr: [] });
              return;
            }
            if (spriteBrush) {
              const [nx, ny] = clampPos(bx, by);
              addSprite(spriteBrush.id, nx, ny);
              return;
            }
            // Sprites are drawn over objects, so they're hit first
            const pt = pixelAt(e);
            const si = spriteAt(pt.x, pt.y);
            if (si !== null) {
              selectSprite(si);
              setSpriteDrag({ index: si, startX: bx, startY: by, dx: 0, dy: 0, copy: e.ctrlKey });
              return;
            }
            if (brush) {
              // The preview's top-left is under the mouse; the object's own position may differ
              const [nx, ny] = clampPos(bx - brush.ox, by - brush.oy);
              placeObject(brush, nx, ny);
              return;
            }
            const o = ownerAt(bx, by);
            select(o);
            if (o !== null && entries[o]?.kind !== "exit") {
              setDrag({ index: o, startX: bx, startY: by, dx: 0, dy: 0, copy: e.ctrlKey });
            }
          }}
          onMouseMove={(e) => {
            const { bx, by } = blockAt(e);
            setCursor({ bx, by });
            if (stroke && e.buttons & 1) {
              if (tool === "erase") {
                const pt = pixelAt(e);
                const o = ownerAt(bx, by), si = spriteAt(pt.x, pt.y);
                setStroke({
                  cells: stroke.cells.some(([x, y]) => x === bx && y === by) ? stroke.cells : [...stroke.cells, [bx, by]],
                  objects: o !== null && entries[o]?.kind !== "exit" && !stroke.objects.includes(o) ? [...stroke.objects, o] : stroke.objects,
                  spr: si !== null && !stroke.spr.includes(si) ? [...stroke.spr, si] : stroke.spr,
                });
              } else if (tool === "fill" && stroke.box && brush) {
                const [x, y] = clampPos(bx - brush.ox, by - brush.oy);
                setStroke({ ...stroke, box: [stroke.box[0], stroke.box[1], x, y] });
              } else if (tool === "draw" && brush) {
                // A new copy wherever it doesn't overlap the ones already painted
                const [w, h] = brushBlocks(brush);
                const cell = clampPos(bx - brush.ox, by - brush.oy);
                if (stroke.cells.every(([x, y]) => Math.abs(x - cell[0]) >= w || Math.abs(y - cell[1]) >= h)) setStroke({ ...stroke, cells: [...stroke.cells, cell] });
              } else if (tool === "sprites") {
                const cell = clampPos(bx, by);
                if (stroke.cells.every(([x, y]) => Math.abs(x - cell[0]) >= 2 || Math.abs(y - cell[1]) >= 2)) setStroke({ ...stroke, cells: [...stroke.cells, cell] });
              }
              return;
            }
            if (spriteDrag) setSpriteDrag({ ...spriteDrag, dx: bx - spriteDrag.startX, dy: by - spriteDrag.startY });
            else if (drag) setDrag({ ...drag, dx: bx - drag.startX, dy: by - drag.startY });
            else setHover(ownerAt(bx, by));
          }}
          onMouseUp={(e) => {
            if (stroke) {
              const s = stroke;
              setStroke(null);
              if (tool === "erase") {
                if (!s.objects.length && !s.spr.length) {
                  log(layers.front.lock || layers.sprites.lock || layers.active
                    ? "Erase: nothing to erase there on the layers you can pick (see the Layers panel)."
                    : "Erase: nothing to erase there. That's the background picture (layer 2) or empty space; Erase removes objects and sprites.");
                } else eraseThings(s.objects, s.spr);
              }
              else if (tool === "sprites" && spriteBrush) paintSprites(spriteBrush.id, s.cells);
              else if (tool === "draw" && brush) paintObjects(brush, s.cells);
              else if (tool === "fill" && brush && s.box) paintObjects(brush, fillCells(s.box, brushBlocks(brush)));
              return;
            }
            // Measure the drag from where the mouse was released (the last move may not have
            // been applied to state yet)
            const up = blockAt(e);
            if (spriteDrag) {
              const s = sprites[spriteDrag.index];
              setSpriteDrag(null);
              const dx = up.bx - spriteDrag.startX, dy = up.by - spriteDrag.startY;
              if (!s || (dx === 0 && dy === 0)) return;
              const [nx, ny] = clampPos(s.x + dx, s.y + dy);
              if (spriteDrag.copy) addSprite(s.id, nx, ny, s.extra);
              else moveSprite(spriteDrag.index, nx, ny);
              return;
            }
            if (!drag) return;
            const ent = entries[drag.index];
            setDrag(null);
            const dx = up.bx - drag.startX, dy = up.by - drag.startY;
            if (!ent || (dx === 0 && dy === 0)) return;
            const [nx, ny] = clampPos(ent.x + dx, ent.y + dy);
            if (drag.copy) duplicateEntry(drag.index, nx, ny);
            else moveEntry(drag.index, nx, ny);
          }}
          onMouseLeave={() => { setHover(null); setDrag(null); setSpriteDrag(null); setCursor(null); setStroke(null); }}
          onContextMenu={(e) => {
            e.preventDefault();
            if (loading) return;
            const pt = pixelAt(e);
            const si = spriteAt(pt.x, pt.y);
            if (si !== null) {
              selectSprite(si);
              setSpriteMenu({ index: si, x: e.clientX, y: e.clientY });
              return;
            }
            const { bx, by } = blockAt(e);
            const o = ownerAt(bx, by);
            select(o);
            setMenu(o === null ? null : { index: o, x: e.clientX, y: e.clientY, px: pt.x, py: pt.y });
            if (o === null) setSpotMenu({ px: pt.x, py: pt.y, x: e.clientX, y: e.clientY });
          }}
          onDragOver={(e) => {
            e.preventDefault();
            e.dataTransfer.dropEffect = "copy";
            setCursor(blockAt(e));
          }}
          onDragLeave={() => setCursor(null)}
          onDrop={(e) => {
            e.preventDefault();
            const raw = e.dataTransfer.getData("application/x-smw-object");
            if (!raw) return;
            const item = JSON.parse(raw) as PaletteItem;
            setDragItem(null);
            const { bx, by } = blockAt(e);
            const [nx, ny] = clampPos(bx - item.ox, by - item.oy);
            placeObject(item, nx, ny);
          }}
        >
          <canvas ref={canvasRef} className="level-canvas" style={layerStyle("back")} />
          <canvas ref={frontRef} className="level-canvas layer-over" style={layerStyle("front")} />
          <canvas ref={spriteRef} className="level-canvas layer-over" style={layerStyle("sprites")} />
          {stroke && (() => {
            // What the stroke will do when the mouse is let go
            // (sprite pictures are offset in pixels, object pictures in blocks)
            const b = tool === "sprites" && spriteBrush ? { w: spriteBrush.w, h: spriteBrush.h, img: spriteBrush.img, ox: spriteBrush.ox / 16, oy: spriteBrush.oy / 16 } : tool === "sprites" ? null : brush;
            const cells = tool === "fill" && stroke.box && brush ? fillCells(stroke.box, brushBlocks(brush)) : stroke.cells;
            return (
              <>
                {tool === "erase"
                  ? stroke.cells.map(([x, y], i) => <div key={i} className="stroke-erase" style={{ left: x * px, top: y * px, width: px, height: px }} />)
                  : b && cells.map(([x, y], i) => (
                      b.img
                        ? <img key={i} className="brush-ghost" src={b.img} style={{ left: (x + b.ox) * px, top: (y + b.oy) * px, width: b.w * zoom, height: b.h * zoom }} />
                        : <div key={i} className="stroke-cell" style={{ left: x * px, top: y * px, width: Math.max(16, b.w) * zoom, height: Math.max(16, b.h) * zoom }} />
                    ))}
                {tool === "fill" && stroke.box && (() => {
                  const [x0, y0, x1, y1] = stroke.box;
                  return <div className="stroke-box" style={{ left: Math.min(x0, x1) * px, top: Math.min(y0, y1) * px, width: (Math.abs(x1 - x0) + 1) * px, height: (Math.abs(y1 - y0) + 1) * px }} />;
                })()}
              </>
            );
          })()}
          {ghost && cursor && ghost.img && !stroke && (tool === "draw" || tool === "fill" || tool === "select") && (
            <img
              className="brush-ghost"
              src={ghost.img}
              style={{ left: cursor.bx * px, top: cursor.by * px, width: ghost.w * zoom, height: ghost.h * zoom }}
            />
          )}
          {spriteBrush && cursor && (
            spriteBrush.img
              ? <img className="brush-ghost" src={spriteBrush.img} style={{ left: (cursor.bx * 16 + spriteBrush.ox) * zoom, top: (cursor.by * 16 + spriteBrush.oy) * zoom, width: spriteBrush.w * zoom, height: spriteBrush.h * zoom }} />
              : <div className="obj-box ghost" style={{ left: cursor.bx * px, top: cursor.by * px, width: px, height: px }} />
          )}
          {level && selectedSprite !== null && level.spriteBoxes.length > selectedSprite * 4 + 3 && (() => {
            const b = level.spriteBoxes;
            const [x, y, w, h] = [b[selectedSprite * 4], b[selectedSprite * 4 + 1], b[selectedSprite * 4 + 2], b[selectedSprite * 4 + 3]];
            const d = spriteDrag && spriteDrag.index === selectedSprite ? spriteDrag : null;
            return (
              <>
                <div className="obj-box selected sprite" style={{ left: x * zoom, top: y * zoom, width: w * zoom, height: h * zoom }} />
                {d && (d.dx !== 0 || d.dy !== 0) && (
                  <div className={"obj-box ghost" + (d.copy ? " copy" : "")} style={{ left: (x + d.dx * 16) * zoom, top: (y + d.dy * 16) * zoom, width: w * zoom, height: h * zoom }} />
                )}
              </>
            );
          })()}
          {level && layers.marks.show && exits.map((x) => {
            // Label each screen that has an exit (vertical levels stack screens two by two)
            const [sx, sy] = level.vertical ? [(x.screen % 2) * 256, Math.floor(x.screen / 2) * 256] : [x.screen * 256, 0];
            return (
              <div key={x.screen} className="exit-badge" style={{ left: sx * zoom + 4, top: sy * zoom + 4 }} title="Pipes and doors on this screen lead here">
                ⇢ {x.secondary ? `entrance ${hex3(x.dest)}` : `level ${hex3(x.dest)}`}
              </div>
            );
          })}
          {level && layers.marks.show && entrances && (() => {
            // Where Mario appears (main entrance), and each secondary entrance into this level
            // The midway point restarts Mario at the main entrance's spot on the midway screen
            const marks = [
              { key: "main", label: "Mario starts here", e: entrances.main, exact: entrances.exact, text: "M" },
              ...(level.vertical ? [] : [{ key: "midway", label: "Restart after the midway gate", e: { ...entrances.main, screen: entrances.midway }, exact: entrances.exact, text: "½" }]),
              ...entrances.secondaries.map((s) => ({ key: `s${s.number}`, label: `Entrance ${hex3(s.number)}`, e: s.entrance, exact: false, text: hex3(s.number) })),
            ];
            return marks.map((m) => {
              const p = entrancePixel(m.e, entrances, level.vertical, m.exact);
              return (
                <div key={m.key} className={"entrance-mark" + (m.key === "main" ? " main" : m.key === "midway" ? " midway" : "")} style={{ left: p.x * zoom, top: p.y * zoom, width: 16 * zoom, height: 32 * zoom }} title={`${m.label} (${ENTRY_STYLES[m.e.action]})`}>
                  <span>{m.text}</span>
                </div>
              );
            });
          })()}
          {hovBox && <div className="obj-box hover" style={{ left: hovBox.x * px, top: hovBox.y * px, width: hovBox.w * px, height: hovBox.h * px }} />}
          {selBox && (
            <div className="obj-box selected" style={{ left: selBox.x * px, top: selBox.y * px, width: selBox.w * px, height: selBox.h * px }} />
          )}
          {selBox && drag && (drag.dx !== 0 || drag.dy !== 0) && (
            <div
              className={"obj-box ghost" + (drag.copy ? " copy" : "")}
              style={{ left: (selBox.x + drag.dx) * px, top: (selBox.y + drag.dy) * px, width: selBox.w * px, height: selBox.h * px }}
            />
          )}
        </div>
      </div>
      {spotMenu && entrances && level && (
        <>
          <div className="ctx-backdrop" onMouseDown={() => setSpotMenu(null)} onContextMenu={(e) => { e.preventDefault(); setSpotMenu(null); }} />
          <div className="ctx-menu" style={{ left: spotMenu.x, top: spotMenu.y }}>
            <div className="ctx-title">This spot</div>
            {spotItems(spotMenu.px, spotMenu.py, () => setSpotMenu(null))}
            <button onClick={() => setSpotMenu(null)}>Cancel <span>Esc</span></button>
          </div>
        </>
      )}
      {spriteMenu && sprites[spriteMenu.index] && (
        <>
          <div className="ctx-backdrop" onMouseDown={() => setSpriteMenu(null)} onContextMenu={(e) => { e.preventDefault(); setSpriteMenu(null); }} />
          <div className="ctx-menu" style={{ left: spriteMenu.x, top: spriteMenu.y }}>
            <div className="ctx-title">{sprites[spriteMenu.index].name} ({hex(sprites[spriteMenu.index].id)})</div>
            <button onClick={() => { const s = sprites[spriteMenu.index]; const [nx, ny] = clampPos(s.x + 1, s.y); addSprite(s.id, nx, ny, s.extra); setSpriteMenu(null); }}>
              Duplicate <span>Ctrl+drag</span>
            </button>
            <button className="danger" onClick={() => { deleteSprite(spriteMenu.index); setSpriteMenu(null); }}>Delete <span>Del</span></button>
            <button onClick={() => setSpriteMenu(null)}>Cancel <span>Esc</span></button>
          </div>
        </>
      )}
      {menu && entries[menu.index] && (
        <>
          <div className="ctx-backdrop" onMouseDown={() => setMenu(null)} onContextMenu={(e) => { e.preventDefault(); setMenu(null); }} />
          <div className="ctx-menu" style={{ left: menu.x, top: menu.y }}>
            <div className="ctx-title">{entryLabel(entries[menu.index])}</div>
            {entries[menu.index].kind !== "exit" && (
              <button
                onClick={() => {
                  const ent = entries[menu.index];
                  const [nx, ny] = clampPos(ent.x + 1, ent.y);
                  duplicateEntry(menu.index, nx, ny);
                  setMenu(null);
                }}
              >
                Duplicate <span>Ctrl+drag</span>
              </button>
            )}
            <button className="danger" onClick={() => { deleteEntry(menu.index); setMenu(null); }}>
              Delete <span>Del</span>
            </button>
            {entrances && <div className="ctx-title">This spot</div>}
            {spotItems(menu.px, menu.py, () => setMenu(null))}
            <button onClick={() => setMenu(null)}>Cancel <span>Esc</span></button>
          </div>
        </>
      )}
    </div>
  );
}

function HexField({ label, value, onCommit }: { label: string; value: number; onCommit: (v: number) => void }) {
  const [text, setText] = useState(hex(value));
  useEffect(() => setText(hex(value)), [value]);
  const commit = () => {
    const v = parseInt(text, 16);
    if (!isNaN(v) && v >= 0 && v <= 0xff && v !== value) onCommit(v);
    else setText(hex(value));
  };
  return (
    <div className="prop-row">
      <span>{label}</span>
      <input
        className="hex-input"
        value={text}
        onChange={(e) => setText(e.target.value)}
        onBlur={commit}
        onKeyDown={(e) => e.key === "Enter" && (e.target as HTMLInputElement).blur()}
      />
    </div>
  );
}

// Names as Lunar Magic shows them. Song names are by SPC song number (the level's music
// choice goes through a table in the ROM, so hacks with custom music still read right).
const SONGS: Record<number, string> = {
  0x00: "No music", 0x01: "Athletic (Piano)", 0x02: "Here We Go! (main level theme)", 0x03: "Water",
  0x04: "Bowser fight 1", 0x05: "Boss battle", 0x06: "Underground (Cave Drums)", 0x07: "Ghost House",
  0x08: "Castle", 0x0D: "Star power", 0x0E: "Direct coins (P-switch)", 0x11: "Switch Palace",
  0x12: "Welcome! (Yoshi's House)", 0x15: "Bowser fight 2", 0x18: "Bowser fight 3", 0x19: "Bowser fight 4",
};
const TILESETS = ["Normal 1", "Castle 1", "Rope 1", "Underground 1", "Switch Palace 1", "Ghost House 1", "Rope 2",
  "Normal 2", "Rope 3", "Underground 2", "Switch Palace 2", "Castle 2", "Cloud/Forest", "Ghost House 2", "Underground 3"];
const SPRITE_SETS = ["Forest", "Castle", "Mushroom", "Underground", "Water", "Pokey", "Switch Palace", "Ghost House",
  "Banzai Bill", "Yoshi's House", "Dino-Rhino", "Mecha-Koopa", "Wendy/Lemmy", "Ninji"];

type Setting = { label: string; help: string; byte: number; shift: number; mask: number; options: (h: HeaderView) => string[] };
const SETTINGS: Setting[] = [
  { label: "Music", help: "Song that plays in this level", byte: 2, shift: 4, mask: 0x70,
    options: (h) => h.music.map((s) => SONGS[s] ?? `Song ${hex(s)}`) },
  { label: "Tileset", help: "Ground and block graphics", byte: 4, shift: 0, mask: 0x0f,
    options: () => [...TILESETS, "(unused 0F)"] },
  { label: "Enemy graphics", help: "Which enemies can show correctly", byte: 2, shift: 0, mask: 0x0f,
    options: () => [...SPRITE_SETS, "(unused 0E)", "(unused 0F)"] },
  { label: "Background colour", help: "Colour behind everything", byte: 1, shift: 5, mask: 0xe0,
    options: (h) => h.colours.map((c, i) => `Colour ${i}  ${c}`) },
  { label: "Background palette", help: "Colours of the background layer", byte: 0, shift: 5, mask: 0xe0,
    options: () => Array.from({ length: 8 }, (_, i) => `Palette ${i}`) },
  { label: "Ground palette", help: "Colours of the ground and blocks", byte: 3, shift: 0, mask: 0x07,
    options: () => Array.from({ length: 8 }, (_, i) => `Palette ${i}`) },
  { label: "Enemy palette", help: "Colours of enemies and items", byte: 3, shift: 3, mask: 0x38,
    options: () => Array.from({ length: 8 }, (_, i) => `Palette ${i}`) },
  { label: "Time limit", help: "Starting time", byte: 3, shift: 6, mask: 0xc0,
    options: () => ["None", "200", "300", "400"] },
  { label: "Vertical scroll", help: "When the camera moves up and down", byte: 4, shift: 4, mask: 0x30,
    options: () => ["Never", "Whenever Mario moves", "Only when flying or climbing", "Camera locked (no scrolling at all)"] },
  { label: "Item memory", help: "Which collected coins/blocks stay collected", byte: 4, shift: 6, mask: 0xc0,
    options: () => ["Set 0", "Set 1", "Set 2", "Set 3"] },
  { label: "Layer 3 in front", help: "Water/tide layers drawn over everything", byte: 2, shift: 7, mask: 0x80,
    options: () => ["No", "Yes"] },
];

function LevelSettings() {
  const { level, loading } = useStudio();
  const [h, setH] = useState<HeaderView | null>(null);
  const [music, setMusic] = useState<MusicView | null>(null);
  const [song, setSong] = useState(0);
  useEffect(() => {
    if (!level) return;
    loadHeader(level.level).then(setH);
    loadMusic().then(setMusic);
    invoke<number>("level_song", { level: level.level }).then(setSong).catch(() => setSong(0));
  }, [level]);
  if (!level || !h) return null;
  return (
    <>
      <div className="prop-group" style={{ marginTop: 16 }}>Level settings</div>
      {SETTINGS.map((s) => {
        // With any-song support, Music is one list: the header's 8 choices and every song
        if (s.label === "Music" && music?.any_song) {
          const header = (h.bytes[s.byte] & s.mask) >> s.shift;
          const value = song ? `s${song}` : `h${header}`;
          return (
            <div key={s.label} className="prop-row" title={s.help}>
              <span>Music</span>
              <span className="setting">
                <select
                  value={value}
                  disabled={loading}
                  onChange={async (e) => {
                    const v = e.target.value;
                    if (v.startsWith("h")) {
                      const i = Number(v.slice(1));
                      if (song) await setLevelSong(0, "");
                      if (i !== header) await setHeaderBits(s.byte, s.mask, i << s.shift, `Music: ${s.options(h)[i]}`);
                    } else {
                      const n = Number(v.slice(1));
                      await setLevelSong(n, music.songs.find((x) => x.number === n)?.title ?? "");
                    }
                  }}
                >
                  <optgroup label="Level music">
                    {h.music.map((n, i) => <option key={i} value={`h${i}`}>{music.songs.find((x) => x.number === n)?.title ?? s.options(h)[i]}</option>)}
                  </optgroup>
                  {music.songs.some((x) => x.pack) && (
                    <optgroup label="Your songs ★">
                      {music.songs.filter((x) => x.pack).map((x) => <option key={x.number} value={`s${x.number}`}>★ {x.title}</option>)}
                    </optgroup>
                  )}
                  <optgroup label="All the game's songs">
                    {music.songs.filter((x) => !x.pack).map((x) => <option key={x.number} value={`s${x.number}`}>{x.title}</option>)}
                  </optgroup>
                </select>
              </span>
            </div>
          );
        }
        const value = (h.bytes[s.byte] & s.mask) >> s.shift;
        const opts = s.options(h);
        return (
          <div key={s.label} className="prop-row" title={s.help}>
            <span>{s.label}</span>
            <span className="setting">
              {s.label === "Background colour" && <i className="swatch" style={{ background: h.colours[value] }} />}
              <select
                value={value}
                disabled={loading}
                onChange={(e) => {
                  const v = Number(e.target.value);
                  setHeaderBits(s.byte, s.mask, v << s.shift, `${s.label}: ${opts[v]}`);
                }}
              >
                {opts.map((o, i) => <option key={i} value={i}>{o}</option>)}
              </select>
            </span>
          </div>
        );
      })}
      <div className="hint">Changes show here right away; press ▶ Play to hear the music. ★ = songs you added.</div>
    </>
  );
}

/** Sprite palette: every sprite by name, drawn by the game in this level's enemy graphics. */
function SpritesPanel() {
  const { level, spritePalette, spritePaletteFor, spriteBrush } = useStudio();
  const [q, setQ] = useState("");
  useEffect(() => { if (level && spritePaletteFor !== level.level) loadSpritePalette(); }, [level, spritePaletteFor]);
  if (!level) return <div className="panel"><p className="muted">Open a level to see its sprites.</p></div>;
  const needle = q.trim().toLowerCase();
  const shown = spritePalette.filter((s) => !needle || s.name.toLowerCase().includes(needle) || hex(s.id).toLowerCase() === needle);
  return (
    <div className="panel objects">
      <input className="search" placeholder="Search sprites…" value={q} onChange={(e) => setQ(e.target.value)} />
      {spriteBrush && (
        <div className="brush-bar">Placing <b>{spriteBrush.name}</b> · click the level · Esc to stop</div>
      )}
      {spritePalette.length === 0 && <p className="muted">Drawing sprites…</p>}
      <div className="obj-grid">
        {shown.map((s) => {
          const active = spriteBrush?.id === s.id;
          return (
            <button key={s.id} className={"obj-card" + (active ? " active" : "")} title={`${hex(s.id)}: ${s.name}`} onClick={() => setSpriteBrush(active ? null : s)}>
              <div className="obj-thumb">
                {s.img ? <img src={s.img} alt="" draggable={false} /> : <div className="obj-invisible">no picture</div>}
              </div>
              <div className="obj-name">{s.name}</div>
              <div className="obj-id">{hex(s.id)}</div>
            </button>
          );
        })}
      </div>
    </div>
  );
}

// Lunar Magic's names for how Mario enters (the game's $192A)
const ENTRY_STYLES = ["Do nothing", "Pipe exit, going left", "Pipe exit, going right", "Pipe exit, going up",
  "Pipe exit, going down", "Do nothing (slippery level)", "Shot from a slanted pipe", "Pipe exit down (water level)"];

function EntranceFields({ e, en, screens, exact, onChange }: { e: Entrance; en: EntrancesData; screens: number; exact?: boolean; onChange: (e: Entrance) => void }) {
  const { loading } = useStudio();
  const row = (label: string, value: number, options: string[], key: keyof Entrance) => (
    <div className="prop-row">
      <span>{label}</span>
      <span className="setting">
        <select value={value} disabled={loading} onChange={(ev) => onChange({ ...e, [key]: Number(ev.target.value) })}>
          {options.map((o, i) => <option key={i} value={i}>{o}</option>)}
        </select>
      </span>
    </div>
  );
  return (
    <>
      {row("Screen", e.screen, Array.from({ length: Math.max(screens, e.screen + 1) }, (_, i) => `Screen ${hex(i)}`), "screen")}
      {exact ? (
        <>
          {row("Across", e.x, Array.from({ length: 16 }, (_, i) => `Tile ${i} (x ${i * 16})`), "x")}
          {row("Height", e.y, Array.from({ length: Math.max(32, e.y + 1) }, (_, i) => `Tile ${i} (y ${i * 16})`), "y")}
        </>
      ) : (
        <>
          {row("Across", e.x, en.xs.map((x, i) => `Spot ${i} (x ${x})`), "x")}
          {row("Height", e.y, en.ys.map((y, i) => `Height ${i} (y ${y})`), "y")}
        </>
      )}
      {row("Enters by", e.action, ENTRY_STYLES, "action")}
    </>
  );
}

/** Where Mario appears: the level's start, and secondary entrances that lead here. */
function Entrances() {
  const { level, entrances, loading } = useStudio();
  if (!level || !entrances) return null;
  return (
    <>
      <div className="prop-group" style={{ marginTop: 16 }}>Entrances</div>
      <div className="hint" style={{ marginBottom: 6 }}>Tip: right-click the level and choose “Mario starts here”.</div>
      <div className="entrance-title">Main entrance <span className="muted">(level start, and screen exits that go to this level)</span></div>
      <EntranceFields e={entrances.main} en={entrances} screens={level.screens} exact={entrances.exact} onChange={setMainEntrance} />
      {!level.vertical && (
        <>
          <div className="entrance-title">Midway point <span className="muted">(where Mario restarts after touching the midway gate)</span></div>
          <div className="prop-row">
            <span>Screen</span>
            <span className="setting">
              <select value={entrances.midway} disabled={loading} onChange={(ev) => setMidwayScreen(Number(ev.target.value))}>
                {Array.from({ length: 16 }, (_, i) => <option key={i} value={i}>Screen {hex(i)}{i >= level.screens ? " (past the end)" : ""}</option>)}
              </select>
            </span>
          </div>
          <div className="hint">Same spot and height as the main entrance, on this screen.</div>
        </>
      )}
      {entrances.secondaries.map((s) => (
        <div key={s.number}>
          <div className="entrance-title">Secondary entrance {hex3(s.number)}</div>
          <EntranceFields e={s.entrance} en={entrances} screens={level.screens} onChange={(e) => setSecondaryEntrance(s.number, e)} />
        </div>
      ))}
      <button className="add-entrance" disabled={loading} onClick={() => addSecondaryEntrance()}>+ New secondary entrance into this level</button>
      <div className="hint">Secondary entrances let pipes and doors drop Mario somewhere other than the start.</div>
    </>
  );
}

/** Where pipes and doors on each screen lead. */
function ScreenExits() {
  const { level, exits, exitsPerScreen, allSecondaries, loading } = useStudio();
  const [adding, setAdding] = useState<number | "">("");
  if (!level) return null;
  const used = new Set(exits.map((x) => x.screen));
  const free = Array.from({ length: level.screens }, (_, i) => i).filter((s) => !used.has(s));
  const levelOption = (n: number) => `Level ${hex3(n)}${levelNameOf(n) ? ` – ${levelNameOf(n)}` : ""}`;
  const update = (x: ScreenExit, patch: Partial<ScreenExit>) => setExit({ ...x, ...patch });
  return (
    <>
      <div className="prop-group" style={{ marginTop: 16 }}>Screen exits</div>
      <div className="hint" style={{ marginBottom: 6 }}>Pipes and doors on a screen take Mario to that screen's exit.</div>
      {exits.length === 0 && <div className="muted" style={{ fontSize: 12 }}>No exits yet.</div>}
      {exits.map((x) => (
        <div key={x.screen} className="exit-row">
          <div className="exit-top">
            <b>Screen {hex(x.screen)}</b>
            <select value={x.secondary ? "sec" : "lvl"} disabled={loading} onChange={(e) => update(x, { secondary: e.target.value === "sec" })}>
              <option value="lvl">goes to a level</option>
              <option value="sec">goes to a secondary entrance</option>
            </select>
            <button className="exit-del" title="Remove this exit" disabled={loading} onClick={() => removeExit(x.screen)}>✕</button>
          </div>
          {x.secondary ? (
            <select className="exit-dest" value={x.dest} disabled={loading} onChange={(e) => update(x, { dest: Number(e.target.value) })}>
              {!allSecondaries.some(([n]) => n === x.dest) && <option value={x.dest}>Entrance {hex3(x.dest)} (unused)</option>}
              {allSecondaries
                .filter(([n]) => exitsPerScreen || n >> 8 === level.level >> 8)
                .map(([n, to]) => <option key={n} value={n}>Entrance {hex3(n)} → {levelOption(to)}</option>)}
            </select>
          ) : (
            <select className="exit-dest" value={x.dest} disabled={loading} onChange={(e) => update(x, { dest: Number(e.target.value) })}>
              {Array.from({ length: 0x200 }, (_, n) => n).map((n) => <option key={n} value={n}>{levelOption(n)}</option>)}
            </select>
          )}
          {exitsPerScreen && x.secondary && (
            <div className="exit-flags">
              <label><input type="checkbox" checked={x.water} disabled={loading} onChange={(e) => update(x, { water: e.target.checked })} /> water level</label>
            </div>
          )}
        </div>
      ))}
      {!exitsPerScreen && exits.length > 1 && (
        <div className="hint">In this ROM (not saved by Lunar Magic yet) “secondary” is one setting for the whole level, so it changes every exit at once.</div>
      )}
      {free.length > 0 && (
        <div className="exit-add">
          <select value={adding} disabled={loading} onChange={(e) => setAdding(e.target.value === "" ? "" : Number(e.target.value))}>
            <option value="">Add an exit on screen…</option>
            {free.map((s) => <option key={s} value={s}>Screen {hex(s)}</option>)}
          </select>
          <button
            className="menu"
            disabled={adding === "" || loading}
            onClick={() => {
              if (adding === "") return;
              setExit({ screen: adding, dest: level.level, secondary: false, water: false });
              setAdding("");
            }}
          >
            Add
          </button>
        </div>
      )}
    </>
  );
}

function SelectedSprite({ s }: { s: SpriteItem }) {
  return (
    <>
      <div className="prop-group">Selected sprite</div>
      <div className="prop-row"><span>Type</span><span>{s.name}</span></div>
      <div className="prop-row"><span>Position</span><span>{s.x}, {s.y}</span></div>
      <HexField label="Sprite number" value={s.id} onCommit={(v) => setSprite(s.index, v, s.extra)} />
      <div className="prop-row" title="Two extra settings bits; their meaning depends on the sprite">
        <span>Extra bits</span>
        <span className="setting">
          <select value={s.extra} onChange={(e) => setSprite(s.index, s.id, Number(e.target.value))}>
            {[0, 1, 2, 3].map((v) => <option key={v} value={v}>{v}</option>)}
          </select>
        </span>
      </div>
      <div className="hint">Drag to move · Ctrl+drag to copy · Arrows nudge · Del deletes</div>
    </>
  );
}

function Properties() {
  const { rom, level, entries, selected, loading, sprites, selectedSprite } = useStudio();
  const sprite = selectedSprite !== null ? sprites[selectedSprite] : null;
  const ent = selected !== null ? entries[selected] : null;
  const rows: [string, string][] = level
    ? [
        ["Number", hex3(level.level)],
        ["Screens", String(level.screens)],
        ["Objects", String(entries.length)],
        ["Sprites", String(level.sprites)],
        ["Layout", level.vertical ? "Vertical" : "Horizontal"],
        ["Level mode", "0x" + hex(level.mode)],
      ]
    : [["Number", "—"], ["Screens", "—"], ["Layout", "—"], ["Level mode", "—"]];
  return (
    <div className="panel props">
      {sprite && <SelectedSprite s={sprite} />}
      {ent && (
        <>
          <div className="prop-group">Selected</div>
          <div className="prop-row"><span>Type</span><span>{entryLabel(ent)}</span></div>
          {ent.kind !== "exit" && <div className="prop-row"><span>Position</span><span>{ent.x}, {ent.y}</span></div>}
          {ent.kind === "object" && (
            <>
              <HexField label="Object number" value={ent.id} onCommit={(v) => v >= 1 && v <= 0x3f && setEntry(ent.index, v, ent.settings)} />
              <HexField label="Size / settings" value={ent.settings} onCommit={(v) => setEntry(ent.index, ent.id, v)} />
            </>
          )}
          {ent.kind === "extended" && (
            <HexField label="Extended number" value={ent.id} onCommit={(v) => v >= 2 && setEntry(ent.index, v, ent.settings)} />
          )}
          <div className="hint">Drag to move · Ctrl+drag to copy · Arrows nudge · Del deletes</div>
        </>
      )}
      <div className="prop-group" style={{ marginTop: ent || sprite ? 16 : 0 }}>Level</div>
      {rows.map(([k, v]) =>
        k === "Screens" && level ? (
          <div key={k} className="prop-row" title={level.vertical ? "How tall the level is: 1-14 screens" : "How long the level is: 1-32 screens"}>
            <span>Length (screens)</span>
            <span className="stepper">
              <button disabled={loading || level.screens <= 1} onClick={() => setScreens(level.screens - 1)}>−</button>
              <span>{level.screens}</span>
              <button disabled={loading || level.screens >= (level.vertical ? 14 : 32)} onClick={() => setScreens(level.screens + 1)}>+</button>
            </span>
          </div>
        ) : (
          <div key={k} className="prop-row"><span>{k}</span><span>{v}</span></div>
        ),
      )}
      <LevelSettings />
      <ScreenExits />
      <Entrances />
      {rom && (
        <>
          <div className="prop-group" style={{ marginTop: 16 }}>ROM</div>
          <div className="prop-row"><span>Type</span><span>{rom.kind}</span></div>
          <div className="prop-row"><span>Size</span><span>{rom.size / 1024} KB</span></div>
          <div className="prop-row"><span>SHA-1</span><span title={rom.sha1}>{rom.sha1.slice(0, 10)}…</span></div>
        </>
      )}
    </div>
  );
}

function ObjectsPanel() {
  const { level, palette, paletteFor, paletteLoading, brush } = useStudio();
  const [filter, setFilter] = useState("");
  const [group, setGroup] = useState<"all" | "object" | "extended">("all");

  useEffect(() => {
    if (level && paletteFor !== level.level) loadPalette();
  }, [level?.level, paletteFor]);

  if (!level) return <div className="panel muted">Open a level to see its objects.</div>;
  const q = filter.toLowerCase();
  const shown = palette.filter(
    (p) =>
      (group === "all" || p.kind === group) &&
      `${p.name} ${hex(p.id)} ${p.desc}`.toLowerCase().includes(q),
  );
  return (
    <div className="panel objects">
      <input className="search" placeholder="Search objects…" value={filter} onChange={(e) => setFilter(e.target.value)} />
      <div className="seg">
        {(["all", "object", "extended"] as const).map((g) => (
          <button key={g} className={group === g ? "on" : ""} onClick={() => setGroup(g)}>
            {g === "all" ? "All" : g === "object" ? "Objects" : "Extended"}
          </button>
        ))}
      </div>
      {paletteLoading && <div className="muted">Drawing previews…</div>}
      {brush && (
        <div className="brush-bar">
          Placing <b>{brush.name}</b> · click the level · Esc to stop
        </div>
      )}
      <div className="obj-grid">
        {shown.map((p) => {
          const key = `${p.kind}-${p.id}`;
          const active = brush && brush.kind === p.kind && brush.id === p.id;
          return (
            <div
              key={key}
              className={"obj-cell" + (active ? " active" : "")}
              title={`${p.kind === "object" ? "Object" : "Extended"} ${hex(p.id)}: ${p.desc}`}
              draggable
              onDragStart={(e) => {
                e.dataTransfer.setData("application/x-smw-object", JSON.stringify(p));
                e.dataTransfer.effectAllowed = "copy";
                // No browser drag picture: the level shows the real-size ghost where it lands
                e.dataTransfer.setDragImage(EMPTY_DRAG, 0, 0);
                setDragItem(p);
              }}
              onDragEnd={() => setDragItem(null)}
              onClick={() => setBrush(active ? null : p)}
            >
              <div className="obj-thumb">
                {p.img ? <img src={p.img} alt="" draggable={false} /> : <div className="obj-invisible">invisible</div>}
              </div>
              <div className="obj-name">{p.name}</div>
              <div className="obj-id">{p.kind === "object" ? "" : "E"}{hex(p.id)}</div>
            </div>
          );
        })}
      </div>
    </div>
  );
}

/** "Add this pack to my hack": pick where it goes in plain words (no level numbers needed). */
function AddLevelDialog({ pack, current, onClose }: { pack: Pack; current: number | null; onClose: () => void }) {
  const [free, setFree] = useState<number[] | null>(null);
  const [where, setWhere] = useState<"new" | "current" | "pick">("new");
  const [picked, setPicked] = useState<number>(current ?? 0x105);
  const [version, setVersion] = useState(pack.levels[0] ?? "");
  useEffect(() => {
    invoke<number[]>("free_levels").then((f) => setFree(f.filter((l) => l !== 0))).catch(() => setFree([]));
  }, []);
  const newSlot = free && free.length ? free[0] : null;
  const target = where === "new" ? newSlot : where === "current" ? current : picked;
  const levelName = (n: number) => `Level ${hex3(n)}${levelNameOf(n) ? ` – ${levelNameOf(n)}` : ""}`;
  const friendly = (path: string) => path.split("/").pop()!.replace(/\.mwl$/i, "").replace(/[-_]+/g, " ");
  const go = () => {
    if (target === null) return;
    applyPack(pack, { type: "add_level", level: target, mwl: version || null });
    onClose();
  };
  return (
    <>
      <div className="modal-backdrop" onMouseDown={onClose} />
      <div className="modal">
        <h3>Add “{pack.name}” to your hack</h3>
        <p className="muted">Its graphics, tiles and colours come with it. Your ROM is backed up first.</p>
        {pack.levels.length > 1 && (
          <>
            <div className="modal-label">Which version?</div>
            <div className="modal-choices">
              {pack.levels.map((l) => (
                <button key={l} className={"chip" + (version === l ? " on" : "")} onClick={() => setVersion(l)}>{friendly(l)}</button>
              ))}
            </div>
          </>
        )}
        <div className="modal-label">Where should it go?</div>
        <label className={"modal-option" + (where === "new" ? " on" : "")}>
          <input type="radio" checked={where === "new"} onChange={() => setWhere("new")} disabled={newSlot === null} />
          <span>
            <b>A new empty level</b> <span className="muted">{newSlot === null ? (free ? "(no free level slots left)" : "(looking…)") : `→ ${levelName(newSlot)}`}</span>
            <br /><span className="hint">Nothing is replaced. Try it with ▶ Play; connect it to your game with a pipe or door later.</span>
          </span>
        </label>
        {current !== null && (
          <label className={"modal-option" + (where === "current" ? " on" : "")}>
            <input type="radio" checked={where === "current"} onChange={() => setWhere("current")} />
            <span>
              <b>Replace the level I'm editing</b> <span className="muted">→ {levelName(current)}</span>
              <br /><span className="hint">Its current design is replaced by the pack's sample level.</span>
            </span>
          </label>
        )}
        <label className={"modal-option" + (where === "pick" ? " on" : "")}>
          <input type="radio" checked={where === "pick"} onChange={() => setWhere("pick")} />
          <span>
            <b>Choose a level</b>{" "}
            <select value={picked} onChange={(e) => { setPicked(Number(e.target.value)); setWhere("pick"); }}>
              {Array.from({ length: 0x200 }, (_, n) => n).map((n) => (
                <option key={n} value={n}>{levelName(n)}{free?.includes(n) ? " (empty)" : ""}</option>
              ))}
            </select>
            <br /><span className="hint">If it isn't empty, its design is replaced.</span>
          </span>
        </label>
        <div className="modal-buttons">
          <button className="menu" onClick={onClose}>Cancel</button>
          <button className="play" disabled={target === null} onClick={go}>Add to {target === null ? "…" : `level ${hex3(target)}`}</button>
        </div>
      </div>
    </>
  );
}

/** Pick which of the ROM's songs a music pack replaces. */
function ReplaceSongDialog({ pack, onClose }: { pack: Pack; onClose: () => void }) {
  const [music, setMusic] = useState<MusicView | null>(null);
  const [target, setTarget] = useState<number | null>(null);
  useEffect(() => { loadMusic().then(setMusic); }, []);
  return (
    <>
      <div className="modal-backdrop" onMouseDown={onClose} />
      <div className="modal">
        <h3>Replace a song with “{pack.name}”</h3>
        <p className="hint">Everywhere the old song plays (levels, the map, ...), this one plays instead. You can put the old one back later.</p>
        <select size={12} style={{ width: "100%" }} value={target ?? ""} onChange={(e) => setTarget(Number(e.target.value))}>
          {music?.songs.map((s) => (
            <option key={s.number} value={s.number}>{hex(s.number)} {s.title}{s.pack ? " ★" : ""}</option>
          ))}
        </select>
        <div className="modal-buttons">
          <button className="menu" onClick={onClose}>Cancel</button>
          <button className="menu primary" disabled={target === null} onClick={() => { if (target !== null) addSong(pack, target); onClose(); }}>Replace</button>
        </div>
      </div>
    </>
  );
}

function Toolbox() {
  const { rom, level, loading } = useStudio();
  const [packs, setPacks] = useState<Pack[]>([]);
  const [busy, setBusy] = useState(false);
  const [dropping, setDropping] = useState(false);
  const refresh = () => listPacks().then(setPacks).catch((e) => log(`Toolbox: ${e}`));
  useEffect(() => { refresh(); }, []);
  // Drop .zip files from Explorer onto the Toolbox to add them
  const hasFiles = (e: React.DragEvent) => e.dataTransfer.types.includes("Files");
  const onDragOver = (e: React.DragEvent) => {
    if (!hasFiles(e)) return;
    e.preventDefault();
    e.dataTransfer.dropEffect = "copy";
    setDropping(true);
  };
  const onDrop = async (e: React.DragEvent) => {
    if (!hasFiles(e)) return;
    e.preventDefault();
    setDropping(false);
    const files = Array.from(e.dataTransfer.files);
    if (!files.some((f) => f.name.toLowerCase().endsWith(".zip"))) { log("Toolbox: drop .zip files (asset packs)"); return; }
    setBusy(true);
    try { await importDroppedFiles(files); } catch (err) { log(`Toolbox: ${err}`); }
    setBusy(false);
    refresh();
  };

  const doImport = async () => {
    setBusy(true);
    try { await importPacks(); } catch (e) { log(`Toolbox: ${e}`); }
    setBusy(false);
    refresh();
  };
  const doMidi = async () => {
    setBusy(true);
    try { await importMidi(); } catch (e) { log(`Toolbox: couldn't make a song from those MIDI files: ${e}`); }
    setBusy(false);
    refresh();
  };
  const [adding, setAdding] = useState<Pack | null>(null);
  const [replacing, setReplacing] = useState<Pack | null>(null);
  const [hackOpen, setHackOpen] = useState(false);
  const addLevel = (p: Pack) => setAdding(p);
  const replace = (p: Pack) => {
    if (confirm(`Replace the game's graphics with "${p.name}" everywhere?\nYour ROM is backed up first.`)) applyPack(p, { type: "replace_graphics" });
  };
  const editCreator = async (p: Pack) => {
    const who = prompt(`Who made "${p.name}"?`, p.creator);
    if (who === null) return;
    await setPackCreator(p.id, who);
    refresh();
  };
  const remove = async (p: Pack) => {
    if (!confirm(`Remove "${p.name}" from your Toolbox? (Hacks that already use it keep it.)`)) return;
    await removePack(p.id);
    refresh();
  };

  return (
    <div
      className={"panel toolbox" + (dropping ? " dropping" : "")}
      onDragOver={onDragOver}
      onDragLeave={() => setDropping(false)}
      onDrop={onDrop}
    >
      <div className="tb-head">
        <button className="menu" onClick={doImport} disabled={busy}>{busy ? "Adding…" : "+ Import packs…"}</button>
        <button className="menu" onClick={doMidi} disabled={busy} title="Make a song from MIDI files">♪ Import MIDI…</button>
        <button className="menu" onClick={() => setHackOpen(true)} disabled={busy || !rom} title={rom ? "Take levels (with their graphics and blocks) from another hack" : "Open a ROM first"}>⇩ From a hack…</button>
        <span className="hint">or drop .zip files here</span>
      </div>
      {packs.length === 0 && !busy && (
        <p className="muted">Your Toolbox is empty. Add packs from SMW Central (tilesets, backgrounds, graphics) and they stay here for every hack.</p>
      )}
      {adding && <AddLevelDialog pack={adding} current={level?.level ?? null} onClose={() => setAdding(null)} />}
      {replacing && <ReplaceSongDialog pack={replacing} onClose={() => setReplacing(null)} />}
      {hackOpen && <HackBrowser onClose={() => setHackOpen(false)} />}
      <div className="tb-list">
        {packs.map((p) => (
          <div key={p.id} className="tb-card">
            <div className="tb-preview">{p.preview ? <img src={p.preview} alt="" /> : <span>no preview</span>}</div>
            <div className="tb-body">
              <div className="tb-name" title={p.name}>{p.name}</div>
              <div className="tb-meta">
                <span className="tb-kind">{p.kind}</span>
                <button className="tb-credit" onClick={() => editCreator(p)} title="Edit credit">
                  {p.creator ? `by ${p.creator}` : "add creator"}
                </button>
              </div>
              {p.notes.map((n, i) => <div key={i} className="tb-note">{n}</div>)}
              {p.links.slice(0, 3).map((l) => (
                <button key={l} className="tb-link" onClick={() => openUrl(l)} title={l}>{l.replace(/^https?:\/\/(www\.)?/, "").slice(0, 46)}</button>
              ))}
              <div className="tb-actions">
                {p.actions.includes("add_level") && (
                  <button className="menu" disabled={!rom || loading} onClick={() => addLevel(p)} title={rom ? "" : "Open a ROM first"}>Add as level…</button>
                )}
                {p.actions.includes("add_song") && (
                  <>
                    <button className="menu" disabled={!rom || loading} onClick={() => addSong(p, null)} title={rom ? "Add as a new song you can pick for any level" : "Open a ROM first"}>Add song</button>
                    <button className="menu" disabled={!rom || loading} onClick={() => setReplacing(p)} title={rom ? "Use it instead of one of the game's songs, everywhere that song plays" : "Open a ROM first"}>Replace a song…</button>
                  </>
                )}
                {p.actions.includes("replace_graphics") && (
                  <button className="menu" disabled={!rom || loading} onClick={() => replace(p)} title={rom ? "" : "Open a ROM first"}>Use in whole game</button>
                )}
                <button className="menu tb-remove" onClick={() => remove(p)} title="Remove from Toolbox">✕</button>
              </div>
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}

function Output() {
  const { log } = useStudio();
  const endRef = useRef<HTMLDivElement>(null);
  useEffect(() => {
    endRef.current?.scrollIntoView();
  }, [log]);
  return (
    <div className="panel output">
      {log.map((l, i) => <div key={i}>{l}</div>)}
      <div ref={endRef} />
    </div>
  );
}

// libretro joypad bits
const BTN = { B: 0, Y: 1, SELECT: 2, START: 3, UP: 4, DOWN: 5, LEFT: 6, RIGHT: 7, A: 8, X: 9, L: 10, R: 11 };
const KEYMAP: Record<string, number> = {
  ArrowUp: BTN.UP, ArrowDown: BTN.DOWN, ArrowLeft: BTN.LEFT, ArrowRight: BTN.RIGHT,
  KeyX: BTN.B, KeyZ: BTN.Y, KeyC: BTN.A, KeyS: BTN.X,
  KeyQ: BTN.L, KeyW: BTN.R, Enter: BTN.START, ShiftRight: BTN.SELECT, ShiftLeft: BTN.SELECT,
};
// Standard gamepad layout (Xbox naming) -> SNES, by position: bottom=B, right=A, left=Y, top=X
const PADMAP: [number, number][] = [
  [0, BTN.B], [1, BTN.A], [2, BTN.Y], [3, BTN.X], [4, BTN.L], [5, BTN.R], [8, BTN.SELECT], [9, BTN.START],
  [12, BTN.UP], [13, BTN.DOWN], [14, BTN.LEFT], [15, BTN.RIGHT],
];

function GameView() {
  const { play, playNote, level } = useStudio();
  const canvasRef = useRef<HTMLCanvasElement>(null);

  // Frames from the game thread go straight to the canvas
  useEffect(() => {
    setFrameSink((buf) => {
      const c = canvasRef.current;
      if (!c) return;
      const v = new DataView(buf);
      const w = v.getUint16(0, true), h = v.getUint16(2, true);
      if (c.width !== w || c.height !== h) {
        c.width = w;
        c.height = h;
      }
      c.getContext("2d")!.putImageData(new ImageData(new Uint8ClampedArray(buf, 4, w * h * 4), w, h), 0, 0);
    });
    return () => setFrameSink(null);
  }, []);

  // Keyboard + gamepad -> buttons. Capture phase so editor shortcuts never see game keys.
  useEffect(() => {
    let keys = 0;
    let raf = 0;
    const onKey = (e: KeyboardEvent) => {
      if (e.code === "Escape" && e.type === "keydown") { stopPlay(); e.preventDefault(); e.stopImmediatePropagation(); return; }
      if (e.code === "KeyR" && e.type === "keydown" && !e.repeat) { restartPlay(); e.preventDefault(); e.stopImmediatePropagation(); return; }
      const bit = KEYMAP[e.code];
      if (bit === undefined) return;
      e.preventDefault();
      e.stopImmediatePropagation();
      keys = e.type === "keydown" ? keys | (1 << bit) : keys & ~(1 << bit);
    };
    const tick = () => {
      let pad = 0;
      for (const gp of navigator.getGamepads?.() ?? []) {
        if (!gp) continue;
        for (const [i, bit] of PADMAP) if (gp.buttons[i]?.pressed) pad |= 1 << bit;
        const [ax, ay] = [gp.axes[0] ?? 0, gp.axes[1] ?? 0];
        if (ax < -0.5) pad |= 1 << BTN.LEFT;
        if (ax > 0.5) pad |= 1 << BTN.RIGHT;
        if (ay < -0.5) pad |= 1 << BTN.UP;
        if (ay > 0.5) pad |= 1 << BTN.DOWN;
      }
      playInput(keys | pad);
      raf = requestAnimationFrame(tick);
    };
    const onBlur = () => (keys = 0);
    window.addEventListener("keydown", onKey, true);
    window.addEventListener("keyup", onKey, true);
    window.addEventListener("blur", onBlur);
    raf = requestAnimationFrame(tick);
    return () => {
      window.removeEventListener("keydown", onKey, true);
      window.removeEventListener("keyup", onKey, true);
      window.removeEventListener("blur", onBlur);
      cancelAnimationFrame(raf);
      playInput(0);
    };
  }, []);

  return (
    <div className="panel game">
      <div className="game-screen">
        <canvas ref={canvasRef} width={256} height={224} />
        {play === "loading" && <div className="game-overlay">Starting level {level ? hex3(level.level) : ""}…</div>}
        {playNote && <div className="game-note">{playNote}</div>}
      </div>
      <div className="game-bar">
        <span><kbd>←→↑↓</kbd> move</span>
        <span><kbd>X</kbd> jump</span>
        <span><kbd>Z</kbd> run / fire</span>
        <span><kbd>C</kbd> spin jump</span>
        <span><kbd>Enter</kbd> pause</span>
        <span><kbd>R</kbd> restart</span>
        <span><kbd>Esc</kbd> back to editing</span>
        <span className="hint">Controllers work too</span>
      </div>
    </div>
  );
}

/** The Level panel: the editor, with the running game laid over it while playing
 *  (the editor stays as it was underneath, scroll position included). */
function LevelPanel() {
  const { play } = useStudio();
  return (
    <div className="level-panel">
      <LevelCanvas />
      {play !== "off" && <GameView />}
    </div>
  );
}

/** Show, hide, lock and fade the parts of the level, like layers in a paint program. */
function LayersPanel() {
  const { level, layers, showSprites } = useStudio();
  if (!level) return <div className="panel"><p className="muted">Open a level to see its layers.</p></div>;
  const rows: { id: LayerId; name: string; hint: string }[] = [
    { id: "marks", name: "Markers", hint: "Where Mario starts, entrances, and where screen exits lead" },
    { id: "sprites", name: "Sprites", hint: "Enemies and items" },
    { id: "front", name: "Level (layer 1)", hint: "Ground, blocks, pipes: what Mario stands on" },
    { id: "back", name: "Background (layer 2)", hint: "The picture behind the level, or a second moving layer in some level modes" },
  ];
  return (
    <div className="panel layers">
      {rows.map((r) => {
        const l = layers[r.id];
        const active = layers.active === r.id;
        return (
          <div key={r.id} className={"layer-row" + (active ? " active" : "") + (l.show ? "" : " hidden")} title={r.hint}
            onClick={() => r.id !== "marks" && setActiveLayer(active ? null : r.id)}>
            <button className={"layer-eye" + (l.show ? " on" : "")} title={l.show ? "Hide" : "Show"}
              onClick={(e) => { e.stopPropagation(); setLayer(r.id, { show: !l.show }); }}>{l.show ? "👁" : "–"}</button>
            <span className="layer-name">
              <b>{r.name}</b>
              <small>{r.id === "sprites" && !showSprites ? "Sprites are turned off (Edit menu)" : r.hint}</small>
            </span>
            {(r.id === "sprites" || r.id === "front") && (
              <button className={"layer-lock" + (l.lock ? " on" : "")} title={l.lock ? "Locked: clicks go through it" : "Unlocked: you can pick things on it"}
                onClick={(e) => { e.stopPropagation(); setLayer(r.id, { lock: !l.lock }); }}>{l.lock ? "🔒" : "🔓"}</button>
            )}
          </div>
        );
      })}
      <label className="layer-fade">
        <input type="checkbox" checked={layers.fade} onChange={(e) => setLayerFade(e.target.checked)} />
        Fade the other layers while one is picked
      </label>
      <p className="hint">Click a layer to work on just that one (clicks only pick things on it). Click it again to work on everything.</p>
    </div>
  );
}

const components: Record<string, React.FC<IDockviewPanelProps>> = {
  layers: LayersPanel,
  docs: DocsPanel,
  explorer: Explorer,
  canvas: LevelPanel,
  properties: Properties,
  objects: ObjectsPanel,
  assets: Toolbox,
  sprites: SpritesPanel,
  features: FeaturesPanel,
  music: MusicPanel,
  ui: UiPanel,
  overworld: OverworldPanel,
  output: Output,
};

const STATUS_LABEL = { done: "Done", partial: "Partly", todo: "Not yet" } as const;

function FeaturesPanel() {
  const all = LUNAR_MAGIC.flatMap((a) => a.features);
  const pct = percent(all);
  const count = (s: string) => all.filter((f) => f.status === s).length;
  return (
    <div className="panel features">
      <div className="ft-head">
        <div>
          <div className="ft-title">Lunar Magic features in SMW Studio</div>
          <div className="muted">{count("done")} done · {count("partial")} partly · {count("todo")} not yet</div>
        </div>
        <div className="ft-pct">{pct}%</div>
      </div>
      <div className="ft-bar"><div style={{ width: `${pct}%` }} /></div>
      {LUNAR_MAGIC.map((a) => (
        <div key={a.area} className="ft-area">
          <div className="ft-area-head"><span>{a.area}</span><span className="muted">{percent(a.features)}%</span></div>
          {a.features.map((f) => (
            <div key={f.name} className="ft-row" title={f.note ?? ""}>
              <span className={`ft-dot ${f.status}`} />
              <span className="ft-name">{f.name}{f.note && <span className="hint"> — {f.note}</span>}</span>
              <span className={`ft-status ${f.status}`}>{STATUS_LABEL[f.status]}</span>
            </div>
          ))}
        </div>
      ))}
      <FeatureList title="Beyond Lunar Magic" list={BEYOND} />
      <FeatureList title="Before the first release (this is the alpha)" list={BEFORE_RELEASE} />
      <FeatureList title="Coming after release" list={FUTURE} />
    </div>
  );
}

function FeatureList({ title, list }: { title: string; list: Feature[] }) {
  return (
    <div className="ft-area">
      <div className="ft-area-head"><span>{title}</span><span className="muted">{list.filter((f) => f.status === "done").length} of {list.length}</span></div>
      {list.map((f) => (
        <div key={f.name} className="ft-row" title={f.note ?? ""}>
          <span className={`ft-dot ${f.status}`} />
          <span className="ft-name">{f.name}{f.note && <span className="hint"> — {f.note}</span>}</span>
          <span className={`ft-status ${f.status}`}>{STATUS_LABEL[f.status]}</span>
        </div>
      ))}
    </div>
  );
}

/** Every panel, with where it goes in the default layout (first one that exists wins). */
const PANELS: { id: string; title: string; places: Parameters<DockviewApi["addPanel"]>[0]["position"][] }[] = [
  { id: "canvas", title: "Level", places: [undefined] },
  { id: "explorer", title: "Explorer", places: [{ direction: "left" }] },
  { id: "properties", title: "Properties", places: [{ referencePanel: "canvas", direction: "right" }, { direction: "right" }] },
  { id: "assets", title: "Toolbox", places: [{ referencePanel: "objects", direction: "within" }, { referencePanel: "properties", direction: "below" }, { direction: "right" }] },
  { id: "objects", title: "Objects", places: [{ referencePanel: "assets", direction: "within" }, { referencePanel: "properties", direction: "below" }, { direction: "right" }] },
  { id: "sprites", title: "Sprites", places: [{ referencePanel: "objects", direction: "within" }, { referencePanel: "assets", direction: "within" }, { direction: "right" }] },
  { id: "output", title: "Output", places: [{ referencePanel: "canvas", direction: "below" }, { direction: "below" }] },
  { id: "features", title: "Features", places: [{ referencePanel: "canvas", direction: "within" }, { direction: "right" }] },
  { id: "layers", title: "Layers", places: [{ referencePanel: "explorer", direction: "below" }, { direction: "left" }] },
  { id: "docs", title: "Docs", places: [{ referencePanel: "canvas", direction: "within" }, { direction: "right" }] },
  { id: "music", title: "Music", places: [{ referencePanel: "canvas", direction: "within" }, { direction: "right" }] },
  { id: "ui", title: "UI", places: [{ referencePanel: "canvas", direction: "within" }, { direction: "right" }] },
  { id: "overworld", title: "Overworld", places: [{ referencePanel: "canvas", direction: "within" }, { direction: "right" }] },
];

// Kept on window so a live-reload of this module (during development) keeps the layout handle
const holder = window as unknown as { __smwDock?: DockviewApi };
let dock: DockviewApi | null = holder.__smwDock ?? null;

/** Open a panel (or bring it to the front if it's already open). */
function showPanel(id: string) {
  if (!dock) return;
  const open = dock.getPanel(id);
  if (open) { open.api.setActive(); return; }
  const p = PANELS.find((x) => x.id === id)!;
  const place = p.places.find((pl) => !pl || !("referencePanel" in pl) || dock!.getPanel(pl.referencePanel as string));
  dock.addPanel({ id: p.id, component: p.id === "assets" ? "assets" : p.id, title: p.title, position: place });
}

function defaultLayout(api: DockviewApi) {
  for (const p of ["canvas", "explorer", "properties", "assets", "objects", "sprites", "output", "layers"]) {
    dock = api;
    showPanel(p);
  }
  api.getPanel("explorer")?.group.api.setSize({ width: 240 });
  api.getPanel("properties")?.group.api.setSize({ width: 280 });
  api.getPanel("output")?.group.api.setSize({ height: 140 });
  api.getPanel("canvas")?.api.setActive();
}

function onReady(e: DockviewReadyEvent) {
  dock = holder.__smwDock = e.api;
  defaultLayout(e.api);
}

/** Window menu, like Adobe's: reopen any panel you closed, or reset the layout. */
function WindowMenu() {
  const [open, setOpen] = useState(false);
  const [, force] = useState(0);
  return (
    <div className="tp">
      <button className="menu" onClick={() => { setOpen(!open); force((n) => n + 1); }}>Window</button>
      {open && (
        <>
          <div className="ctx-backdrop" onMouseDown={() => setOpen(false)} />
          <div className="tp-menu win-menu">
            {PANELS.map((p) => (
              <button key={p.id} className="tp-row tp-action" onClick={() => { showPanel(p.id); setOpen(false); }}>
                <span className="win-check">{dock?.getPanel(p.id) ? "✓" : ""}</span> {p.title}
              </button>
            ))}
            <div className="ctx-title" />
            <button className="tp-row tp-action" onClick={() => { if (dock) { dock.clear(); defaultLayout(dock); } setOpen(false); }}>
              <span className="win-check" /> Reset layout
            </button>
          </div>
        </>
      )}
    </div>
  );
}

function TestPlayButton() {
  const { level, play } = useStudio();
  const [open, setOpen] = useState(false);
  const [opts, setOpts] = useState<TestOptions>(loadTestOptions);
  const update = (patch: Partial<TestOptions>) => {
    const next = { ...opts, ...patch };
    setOpts(next);
    saveTestOptions(next);
  };
  const check = (key: "lock_level" | "infinite_lives" | "invincible" | "infinite_time" | "midway", label: string, hint: string) => (
    <label className="tp-row" title={hint}>
      <input type="checkbox" checked={opts[key]} onChange={(e) => update({ [key]: e.target.checked })} /> {label}
    </label>
  );
  return (
    <div className="tp">
      {play === "off" ? (
        <button
          className="play"
          disabled={!level}
          title={level ? `Play level ${hex3(level.level)} right here (unsaved edits included)` : "Open a level first"}
          onClick={startPlay}
        >
          ▶ Play
        </button>
      ) : (
        <button className="play stop" onClick={stopPlay} title="Back to editing (Esc)">
          ■ Stop
        </button>
      )}
      <button className="play tp-more" disabled={!level} onClick={() => setOpen(!open)} title="Play options">▾</button>
      {open && (
        <>
          <div className="ctx-backdrop" onMouseDown={() => setOpen(false)} />
          <div className="tp-menu">
            <div className="ctx-title">Play options</div>
            {check("infinite_lives", "Infinite lives", "Lives stay at 99")}
            {check("invincible", "Invincible", "Enemies can't hurt you (falling in pits still counts)")}
            {check("infinite_time", "Infinite time", "Timer stays at 999")}
            {check("midway", "Start at the midway point", "As if the midway gate was already touched")}
            <label className="tp-row">
              Start as
              <select value={opts.powerup} onChange={(e) => update({ powerup: Number(e.target.value) })}>
                <option value={-1}>(normal)</option>
                <option value={0}>Small Mario</option>
                <option value={1}>Big Mario</option>
                <option value={2}>Cape Mario</option>
                <option value={3}>Fire Mario</option>
              </select>
            </label>
            <div className="hint">Changes apply the next time you press Play.</div>
            <div className="ctx-title" style={{ marginTop: 6 }}>Whole game</div>
            {check("lock_level", "Mesen: always return to this level", "Starting, dying or exiting brings you back here")}
            <button className="tp-row tp-action" onClick={() => { setOpen(false); testPlay(opts); }}>
              Open in Mesen…
            </button>
          </div>
        </>
      )}
    </div>
  );
}

export default function App() {
  const { rom, dirty, loading, tool } = useStudio();
  const [prefs, setPrefs] = useState(false);
  const [upgradesOpen, setUpgradesOpen] = useState(false);
  const [patchesOpen, setPatchesOpen] = useState(false);
  // Release notes, once per new version (not on top of the first-run tour)
  const [whatsNew, setWhatsNew] = useState(() => welcomeSeen() && whatsNewDue());
  const [welcome, setWelcome] = useState(() => !welcomeSeen());
  useEffect(() => { reopenLastRom(); }, []);
  // App-wide shortcuts: Ctrl+O open, Ctrl+, preferences
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const typing = ["INPUT", "SELECT", "TEXTAREA"].includes((e.target as HTMLElement).tagName);
      if (e.key === "F1") { e.preventDefault(); showPanel("docs"); return; }
      if (!(e.ctrlKey || e.metaKey)) {
        // Tool keys: V B G E S
        const t = TOOLS.find((x) => x.key.toLowerCase() === e.key.toLowerCase());
        if (t && !typing && !e.altKey && !isPlaying()) setTool(t.id);
        return;
      }
      if (e.key.toLowerCase() === "o") { e.preventDefault(); openRom(); }
      if (e.key === ",") { e.preventDefault(); setPrefs(true); }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);
  return (
    <div className="app">
      <header className="ribbon">
        <div className="brand"><img src="/logo.svg" alt="" className="brand-logo" />SMW Studio <span className="stage">{STAGE}</span></div>
        <nav className="menus">
          <FileMenu />
          <EditMenu onPrefs={() => setPrefs(true)} />
          <WindowMenu />
          <button className="menu" onClick={save} disabled={!rom || loading} title="Save (Ctrl+S)">
            Save{dirty ? " •" : ""}
          </button>
          <button className="menu" onClick={undo} disabled={!rom} title="Undo (Ctrl+Z)">↶</button>
          <button className="menu" onClick={redo} disabled={!rom} title="Redo (Ctrl+Y)">↷</button>
          <ToolsMenu onUpgrades={() => setUpgradesOpen(true)} onPatches={() => setPatchesOpen(true)} />
          <Menu title="Help" items={() => [
            { label: "Docs and tutorials", keys: "F1", onClick: () => showPanel("docs") },
            { label: "Welcome tour", onClick: () => setWelcome(true) },
            { label: "What's New", onClick: () => setWhatsNew(true) },
          ]} />
          <button className="menu" onClick={() => showPanel("canvas")} title="The level editor">Level</button>
          <button className="menu" onClick={() => showPanel("overworld")} title="The overworld maps">Overworld</button>
        </nav>
        <div className="tools">
          {TOOLS.map((t) => (
            <button
              key={t.id}
              className={"tool" + (tool === t.id ? " active" : "")}
              title={`${t.label} (${t.key}): ${t.help}`}
              onClick={() => setTool(t.id)}
            >
              {t.label}
            </button>
          ))}
        </div>
        <TestPlayButton />
      </header>
      <main className="dock">
        <DockviewReact components={components} onReady={onReady} theme={themeAbyss} />
      </main>
      {upgradesOpen && <UpgradesDialog onClose={() => setUpgradesOpen(false)} />}
      {patchesOpen && <PatchesDialog onClose={() => setPatchesOpen(false)} />}
      {welcome && <Welcome onClose={() => setWelcome(false)} onDocs={() => showPanel("docs")} />}
      {!welcome && whatsNew && <WhatsNew onClose={() => setWhatsNew(false)} />}
      {prefs && <PreferencesDialog onClose={() => setPrefs(false)} onResetLayout={() => { if (dock) { dock.clear(); defaultLayout(dock); } }} />}
      <footer className="status">
        <span>{rom ? rom.kind : "No ROM loaded"}</span>
        <span>{dirty ? "Unsaved changes" : rom ? "All changes saved" : ""}</span>
        <span>Tool: {TOOLS.find((t) => t.id === tool)?.label}</span>
      </footer>
    </div>
  );
}
