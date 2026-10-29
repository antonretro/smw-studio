import { useEffect, useMemo, useRef, useState } from "react";
import { Channel, invoke } from "@tauri-apps/api/core";
import { addSong, hex3, importMidi, levelNameOf, listPacks, log, useStudio, type Pack } from "./store";

type Setting = { name: string; keep: boolean; instrument: number; octave: number; volume: number };
type Project = {
  title: string;
  bpm: number;
  parts: [string, boolean, [number, number, number, number][]][];
  settings: Setting[];
  instruments: [number, string][];
  channels: number;
  in_rom: number | null;
};

// One colour per part (repeats after 10)
const COLOURS = ["#ff6b6b", "#ffd166", "#06d6a0", "#4cc9f0", "#b388ff", "#ff9f43", "#f78fb3", "#9ae66e", "#70a1ff", "#e0c3fc"];
const BEAT = 48; // ticks per beat
const KEY_H = 4; // pixels per semitone
const LOW = 24, HIGH = 108; // keys shown

type RomSong = { number: number; bytes: number; kind: "original" | "edited" | "custom"; like: string | null; match_percent: number; name: string; levels: number[] };

const KIND_LABEL = { original: "Original", edited: "Edited original", custom: "Custom" };

/** The ROM's own songs, each marked original / edited original / custom, with where it plays. */
function RomSongs() {
  const { rom } = useStudio();
  const [songs, setSongs] = useState<RomSong[] | null>(null);
  const [state, setState] = useState<"idle" | "busy" | string>("idle");
  // The song being heard: its number, or "loading" while the game starts
  const [hearing, setHearing] = useState<{ n: number; ready: boolean } | null>(null);
  useEffect(() => { setSongs(null); setState("idle"); }, [rom?.path]);
  useEffect(() => () => { invoke("play_stop").catch(() => {}); }, []);
  const hear = async (s: RomSong) => {
    if (hearing?.n === s.number) {
      invoke("play_stop").catch(() => {});
      setHearing(null);
      return;
    }
    setHearing({ n: s.number, ready: false });
    const frames = new Channel<ArrayBuffer>();
    frames.onmessage = () => {};
    const events = new Channel<string>();
    events.onmessage = (ev) => {
      if (ev === "ready") setHearing((h) => (h?.n === s.number ? { n: s.number, ready: true } : h));
      else if (ev === "stopped") setHearing((h) => (h?.n === s.number ? null : h));
      else if (ev.startsWith("error")) { log(`Music: couldn't play song ${s.number.toString(16).toUpperCase()}: ${ev.slice(7)}`); setHearing(null); }
    };
    try {
      await invoke("rom_song_play", { number: s.number, level: s.levels[0] ?? null, frames, events });
    } catch (e) {
      log(`Music: couldn't play it: ${e}`);
      setHearing(null);
    }
  };
  const check = async () => {
    setState("busy");
    try {
      setSongs(await invoke<RomSong[]>("rom_songs"));
      setState("idle");
    } catch (e) {
      setState(String(e));
    }
  };
  const where = (l: number) => levelNameOf(l) ?? `level ${hex3(l)}`;
  return (
    <details className="mu-romsongs">
      <summary>Songs in this ROM</summary>
      {!songs && (
        <p className="muted">
          {state === "busy" ? "Comparing with the original game's songs and checking every level…" : state !== "idle" ? state : "See which songs are the original game's, edited originals or new, and which levels play them."}{" "}
          {state !== "busy" && <button className="menu" disabled={!rom} onClick={check}>Check songs</button>}
        </p>
      )}
      {songs && (
        <table>
          <tbody>
            {songs.map((s) => (
              <tr key={s.number}>
                <td>
                  <button className="menu mu-hear" onClick={() => hear(s)} title={hearing?.n === s.number ? "Stop" : "Hear it (your ROM isn't changed)"}>
                    {hearing?.n !== s.number ? "▶" : hearing.ready ? "■" : "…"}
                  </button>
                </td>
                <td className="mu-num">{s.number.toString(16).toUpperCase().padStart(2, "0")}</td>
                <td><span className={`mu-kind ${s.kind}`} title={s.like ? `${s.match_percent}% like the original "${s.like}"` : "Not like any original song"}>{KIND_LABEL[s.kind]}</span></td>
                <td>{s.name}</td>
                <td className="muted">{s.levels.length ? s.levels.map(where).join(", ") : "no map level"}</td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </details>
  );
}

/** Music tab: a song made from MIDI as a piano roll, with each part's settings. */
export default function MusicPanel() {
  const { rom, loading } = useStudio();
  const [packs, setPacks] = useState<Pack[]>([]);
  const [id, setId] = useState<string>("");
  const [proj, setProj] = useState<Project | null>(null);
  const [settings, setSettings] = useState<Setting[]>([]);
  const [focus, setFocus] = useState<number | null>(null);
  const [zoom, setZoom] = useState(0.25); // pixels per tick
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  const canvas = useRef<HTMLCanvasElement>(null);
  const head = useRef<HTMLDivElement>(null);
  const roll = useRef<HTMLDivElement>(null);
  // Hearing the song: "off", "loading" (building a test ROM), "playing" since a time
  const [listen, setListen] = useState<"off" | "loading" | number>("off");

  const refreshPacks = async () => {
    const all = (await listPacks()).filter((p) => p.actions.includes("add_song"));
    setPacks(all);
    if (!id && all.length) setId(all[all.length - 1].id);
  };
  useEffect(() => { refreshPacks(); }, []);

  useEffect(() => {
    if (!id) return;
    setError("");
    invoke<Project>("music_project", { id })
      .then((p) => { setProj(p); setSettings(p.settings); setFocus(null); })
      .catch((e) => { setProj(null); setError(String(e)); });
  }, [id, rom?.path]);

  const playing = settings.filter((s) => s.keep).length;
  const end = useMemo(() => Math.max(BEAT * 4, ...(proj?.parts.flatMap((p) => p[2].map((n) => n[0] + n[1])) ?? [0])), [proj]);

  // Draw the piano roll
  useEffect(() => {
    const c = canvas.current;
    if (!c || !proj) return;
    const w = Math.ceil(end * zoom) + 40;
    const h = (HIGH - LOW) * KEY_H;
    c.width = w;
    c.height = h;
    const g = c.getContext("2d")!;
    g.fillStyle = "#11151c";
    g.fillRect(0, 0, w, h);
    // Octave lines (C notes) and bar lines
    for (let k = LOW; k <= HIGH; k++) {
      if (k % 12 === 0) {
        g.fillStyle = "#1e2530";
        g.fillRect(0, (HIGH - k) * KEY_H, w, 1);
        g.fillStyle = "#4a5568";
        g.font = "9px sans-serif";
        g.fillText(`C${k / 12 - 1}`, 2, (HIGH - k) * KEY_H - 1);
      }
    }
    for (let t = 0; t < end; t += BEAT * 4) {
      g.fillStyle = (t / (BEAT * 4)) % 4 === 0 ? "#2a3240" : "#1a2029";
      g.fillRect(Math.round(t * zoom), 0, 1, h);
    }
    proj.parts.forEach(([, drums, notes], i) => {
      const s = settings[i];
      if (!s) return;
      const on = s.keep && (focus === null || focus === i);
      g.globalAlpha = on ? 0.95 : 0.12;
      g.fillStyle = COLOURS[i % COLOURS.length];
      for (const [start, len, key] of notes) {
        const k = drums ? key : key + s.octave * 12;
        if (k < LOW || k > HIGH) continue;
        g.fillRect(Math.round(start * zoom), (HIGH - k) * KEY_H, Math.max(1, Math.round(len * zoom) - 1), KEY_H - 1);
      }
    });
    g.globalAlpha = 1;
  }, [proj, settings, focus, zoom, end]);

  const change = (i: number, patch: Partial<Setting>) => {
    const next = settings.map((s, j) => (j === i ? { ...s, ...patch } : s));
    if (patch.keep && next.filter((s) => s.keep).length > (proj?.channels ?? 8)) {
      log(`Music: the SNES plays ${proj?.channels ?? 8} parts at once. Turn one off first.`);
      return;
    }
    setSettings(next);
    invoke("music_project_save", { id, settings: next }).catch((e) => log(`Music: ${e}`));
  };

  const putInRom = async () => {
    const pack = packs.find((p) => p.id === id);
    if (!pack || !proj) return;
    setBusy(true);
    await invoke("music_project_save", { id, settings });
    const n = proj.in_rom !== null
      ? await invoke<[number, string[]]>("music_add", { id, file: null, replace: proj.in_rom }).then(([n]) => n).catch((e) => { log(`Music: ${e}`); return null; })
      : await addSong(pack, null);
    if (n !== null && proj.in_rom !== null) log(`Music: song ${n.toString(16).toUpperCase()} updated in your ROM. Press Play to hear it.`);
    setBusy(false);
    setProj(await invoke<Project>("music_project", { id }).catch(() => proj));
  };

  const doImport = async () => {
    try {
      const p = await importMidi();
      await refreshPacks();
      if (p) setId(p.id);
    } catch (e) {
      log(`Music: couldn't make a song from those MIDI files: ${e}`);
    }
  };

  // Playhead: follows the song's tempo from when the game started playing it
  useEffect(() => {
    if (typeof listen !== "number" || !proj) return;
    let raf = 0;
    const loop = () => {
      const ticks = (((performance.now() - listen) / 1000) * proj.bpm / 60) * BEAT;
      const x = Math.round((ticks % end) * zoom);
      if (head.current) head.current.style.transform = `translateX(${x}px)`;
      const r = roll.current;
      if (r && (x < r.scrollLeft || x > r.scrollLeft + r.clientWidth - 40)) r.scrollLeft = Math.max(0, x - 40);
      raf = requestAnimationFrame(loop);
    };
    raf = requestAnimationFrame(loop);
    return () => cancelAnimationFrame(raf);
  }, [listen, proj, zoom, end]);
  useEffect(() => () => { invoke("play_stop").catch(() => {}); }, []);

  const startListen = async () => {
    if (!proj) return;
    await invoke("music_project_save", { id, settings }).catch(() => {});
    setListen("loading");
    const frames = new Channel<ArrayBuffer>();
    frames.onmessage = () => {};
    const events = new Channel<string>();
    events.onmessage = (ev) => {
      if (ev === "ready") setListen(performance.now());
      else if (ev === "stopped") setListen("off");
      else if (ev.startsWith("error")) { log(`Music: couldn't play it: ${ev.slice(7)}`); setListen("off"); }
    };
    try {
      await invoke("music_preview", { id, level: 0x105, frames, events });
    } catch (e) {
      log(`Music: couldn't play it: ${e}`);
      setListen("off");
    }
  };
  const stopListen = () => { invoke("play_stop").catch(() => {}); setListen("off"); };

  const instrumentName = (n: number) => proj?.instruments.find(([m]) => m === n)?.[1] ?? `@${n}`;

  return (
    <div className="panel music">
      <div className="mu-head">
        <select value={id} onChange={(e) => setId(e.target.value)}>
          {packs.length === 0 && <option value="">No songs yet</option>}
          {packs.map((p) => <option key={p.id} value={p.id}>{p.name}</option>)}
        </select>
        <button className="menu" onClick={doImport}>♪ Import MIDI…</button>
        <span className="mu-gap" />
        {proj && <span className="mu-info">{Math.round(proj.bpm)} BPM · {playing}/{proj.channels} parts playing</span>}
        {listen === "off" ? (
          <button className="menu mu-play" disabled={!rom || !proj} onClick={startListen} title={rom ? "Hear it (your ROM isn't changed)" : "Open a ROM first"}>▶ Play</button>
        ) : (
          <button className="menu mu-play" onClick={stopListen}>{listen === "loading" ? "Getting ready…" : "■ Stop"}</button>
        )}
        <button className="menu" onClick={() => setZoom((z) => Math.max(0.05, z / 1.5))} title="Zoom out">−</button>
        <button className="menu" onClick={() => setZoom((z) => Math.min(2, z * 1.5))} title="Zoom in">+</button>
        <button className="menu" disabled={!rom || !proj || loading || busy} onClick={putInRom} title={rom ? "" : "Open a ROM first"}>
          {busy ? "Working…" : proj?.in_rom != null ? `Update song ${proj.in_rom.toString(16).toUpperCase()} in ROM` : "Put in ROM"}
        </button>
      </div>
      <RomSongs />
      {error && <p className="muted" style={{ padding: 12 }}>{error}</p>}
      {!id && !error && (
        <p className="muted" style={{ padding: 12 }}>Import a song from MIDI files to see and arrange it here. Pick all its parts together (one file per instrument works best).</p>
      )}
      {proj && (
        <div className="mu-body">
          <div className="mu-parts">
            {proj.parts.map(([name, drums, notes], i) => {
              const s = settings[i];
              return (
                <div key={i} className={"mu-part" + (focus === i ? " focus" : "") + (s.keep ? "" : " off")} onClick={() => setFocus(focus === i ? null : i)}>
                  <div className="mu-row">
                    <input type="checkbox" checked={s.keep} onClick={(e) => e.stopPropagation()} onChange={(e) => change(i, { keep: e.target.checked })} title="Play this part" />
                    <i className="mu-swatch" style={{ background: COLOURS[i % COLOURS.length] }} />
                    <b>{name}</b>
                    <span className="muted">{notes.length} notes</span>
                  </div>
                  <div className="mu-row" onClick={(e) => e.stopPropagation()}>
                    {drums ? (
                      <span className="muted">Drum kit</span>
                    ) : (
                      <select value={s.instrument} disabled={!s.keep} onChange={(e) => change(i, { instrument: Number(e.target.value) })} title={instrumentName(s.instrument)}>
                        {proj.instruments.map(([n, label]) => <option key={n} value={n}>{label}</option>)}
                      </select>
                    )}
                    {!drums && (
                      <span className="mu-oct" title="Octave">
                        <button disabled={!s.keep || s.octave <= -3} onClick={() => change(i, { octave: s.octave - 1 })}>−</button>
                        {s.octave > 0 ? `+${s.octave}` : s.octave}
                        <button disabled={!s.keep || s.octave >= 3} onClick={() => change(i, { octave: s.octave + 1 })}>+</button>
                      </span>
                    )}
                  </div>
                  <div className="mu-row" onClick={(e) => e.stopPropagation()} title="Volume (left = automatic)">
                    <span className="muted">Vol</span>
                    <input type="range" min={0} max={255} value={s.volume} disabled={!s.keep}
                      onChange={(e) => setSettings(settings.map((x, j) => (j === i ? { ...x, volume: Number(e.target.value) } : x)))}
                      onMouseUp={(e) => change(i, { volume: Number((e.target as HTMLInputElement).value) })} />
                    <span className="muted">{s.volume === 0 ? "auto" : s.volume}</span>
                  </div>
                </div>
              );
            })}
          </div>
          <div className="mu-roll" ref={roll}>
            <div className="mu-canvas">
              <canvas ref={canvas} />
              {typeof listen === "number" && <div className="mu-head-line" ref={head} />}
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
