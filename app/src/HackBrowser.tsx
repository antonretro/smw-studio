import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { hex3, loadLevel, log, openRomAt, useStudio } from "./store";

type HackView = { name: string; levels: number[] };

/** Take levels from another hack: pick its ROM, browse its levels, bring one into yours. */
export default function HackBrowser({ onClose }: { onClose: () => void }) {
  const { rom } = useStudio();
  const [path, setPath] = useState<string | null>(null);
  const [hack, setHack] = useState<HackView | null>(null);
  const [status, setStatus] = useState("");
  const [previews, setPreviews] = useState<Record<number, string>>({});
  const [picked, setPicked] = useState<number | null>(null);
  const [free, setFree] = useState<number[]>([]);
  const [target, setTarget] = useState<number | null>(null);
  const [busy, setBusy] = useState(false);

  const choose = async () => {
    const p = await open({ title: "Pick a hack to take levels from", filters: [{ name: "SNES ROM", extensions: ["sfc", "smc"] }] });
    if (typeof p !== "string") return;
    setPath(p);
    setHack(null);
    setPreviews({});
    setPicked(null);
    setStatus("Taking the hack apart with Lunar Magic (only the first time, about a minute)…");
    try {
      const h = await invoke<HackView>("hack_open", { path: p });
      setHack(h);
      setStatus(`${h.levels.length} levels. Pictures appear as they're drawn.`);
    } catch (e) {
      setStatus(`Couldn't open it: ${e}`);
    }
  };
  useEffect(() => { choose(); }, []);
  useEffect(() => { invoke<number[]>("free_levels").then((f) => setFree(f.filter((l) => l !== 0))).catch(() => setFree([])); }, [rom?.path]);

  // Draw previews one at a time (each puts the level in a scratch ROM and draws it)
  useEffect(() => {
    if (!hack || !path) return;
    let stop = false;
    (async () => {
      for (const l of hack.levels) {
        if (stop) return;
        if (previews[l]) continue;
        try {
          const img = await invoke<string>("hack_preview", { path, level: l });
          if (!stop) setPreviews((p) => ({ ...p, [l]: img }));
        } catch {
          if (!stop) setPreviews((p) => ({ ...p, [l]: "" }));
        }
      }
    })();
    return () => { stop = true; };
  }, [hack, path]);

  const bring = async () => {
    if (!path || picked === null || target === null || !rom) return;
    setBusy(true);
    setStatus(`Bringing level ${hex3(picked)} in as level ${hex3(target)}…`);
    try {
      const report = await invoke<string[]>("hack_bring", { path, level: picked, asLevel: target });
      log(`From ${hack?.name}: level ${hex3(picked)} is now your level ${hex3(target)}`);
      report.filter((l) => !l.startsWith("Backup")).forEach((l) => log(`  ${l}`));
      await openRomAt(rom.path);
      await loadLevel(target);
      onClose();
    } catch (e) {
      setStatus(`Couldn't bring it in: ${e}`);
    } finally {
      setBusy(false);
    }
  };

  return (
    <>
      <div className="modal-backdrop" onMouseDown={() => !busy && onClose()} />
      <div className="modal hack-browser">
        <h3>Take a level from a hack {hack && <span className="muted">· {hack.name}</span>}</h3>
        <p className="hint">{status}</p>
        <div className="hb-grid">
          {hack?.levels.map((l) => (
            <button key={l} className={"hb-level" + (picked === l ? " on" : "")} onClick={() => setPicked(l)}>
              {previews[l] ? <img src={previews[l]} alt="" /> : <span className="hb-wait">{previews[l] === "" ? "no picture" : "…"}</span>}
              <b>Level {hex3(l)}</b>
            </button>
          ))}
        </div>
        <div className="hb-bar">
          <button className="menu" onClick={choose} disabled={busy}>Pick another hack…</button>
          <span className="mu-gap" />
          {picked !== null && (
            <>
              <span>Level {hex3(picked)} →</span>
              <select value={target ?? ""} onChange={(e) => setTarget(e.target.value === "" ? null : Number(e.target.value))}>
                <option value="">your level…</option>
                {free.map((f) => <option key={f} value={f}>{hex3(f)} (free)</option>)}
              </select>
              <button className="menu welcome-next" disabled={busy || target === null || !rom} onClick={bring}>{busy ? "Working…" : "Bring it in"}</button>
            </>
          )}
          <button className="menu" onClick={onClose} disabled={busy}>Close</button>
        </div>
        <p className="hint">Brings the level with the graphics and blocks it uses (renumbered so nothing of yours is replaced). The hack's own enemies and code don't come with it. Your ROM is backed up first. Please credit the hack's makers.</p>
      </div>
    </>
  );
}
