import { useEffect, useState, type ReactNode } from "react";
import { invoke } from "@tauri-apps/api/core";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import {
  closeRom,
  deleteEntry,
  importPatch,
  setPatch,
  studioState,
  type PatchSetting,
  type PatchView,
  loadPrefs,
  openRom,
  openRomAt,
  recentRoms,
  redo,
  refreshDirty,
  save,
  saveCopy,
  savePrefs,
  select,
  toggleSprites,
  undo,
  useStudio,
  type Prefs,
} from "./store";

export type MenuItem = { label: string; keys?: string; disabled?: boolean; onClick: () => void } | "-";

/** A top-bar menu: a button that opens a list of commands. */
export function Menu({ title, items }: { title: string; items: () => MenuItem[] }) {
  const [open, setOpen] = useState(false);
  return (
    <div className="tp">
      <button className={"menu" + (open ? " open" : "")} onClick={() => setOpen(!open)}>{title}</button>
      {open && (
        <>
          <div className="ctx-backdrop" onMouseDown={() => setOpen(false)} />
          <div className="tp-menu win-menu">
            {items().map((it, i) =>
              it === "-" ? (
                <div key={i} className="menu-sep" />
              ) : (
                <button key={i} className="tp-row tp-action menu-item" disabled={it.disabled} onClick={() => { setOpen(false); it.onClick(); }}>
                  <span>{it.label}</span>
                  {it.keys && <kbd>{it.keys}</kbd>}
                </button>
              ),
            )}
          </div>
        </>
      )}
    </div>
  );
}

const fileName = (p: string) => p.split(/[\\/]/).pop() ?? p;

export function FileMenu() {
  const { rom, loading, dirty } = useStudio();
  return (
    <Menu
      title="File"
      items={() => {
        const recent = recentRoms().filter((p) => p !== rom?.path);
        return [
          { label: "Open ROM…", keys: "Ctrl+O", onClick: openRom },
          ...recent.slice(0, 5).map((p) => ({ label: `↺ ${fileName(p)}`, onClick: () => { if (!dirty || confirm("You have unsaved changes. Open another ROM anyway?")) openRomAt(p); } })),
          "-",
          { label: "Save", keys: "Ctrl+S", disabled: !rom || loading, onClick: save },
          { label: "Save a copy as…", disabled: !rom || loading, onClick: saveCopy },
          "-",
          { label: "Show ROM in folder", disabled: !rom, onClick: () => { if (rom) revealItemInDir(rom.path).catch(() => {}); } },
          { label: "Close ROM", disabled: !rom, onClick: closeRom },
        ];
      }}
    />
  );
}

export function EditMenu({ onPrefs }: { onPrefs: () => void }) {
  const { rom, level, selected, showSprites } = useStudio();
  return (
    <Menu
      title="Edit"
      items={() => [
        { label: "Undo", keys: "Ctrl+Z", disabled: !rom || !level, onClick: undo },
        { label: "Redo", keys: "Ctrl+Y", disabled: !rom || !level, onClick: redo },
        "-",
        { label: "Delete selected", keys: "Del", disabled: selected === null, onClick: () => { if (selected !== null) deleteEntry(selected); } },
        { label: "Deselect", keys: "Esc", disabled: selected === null, onClick: () => select(null) },
        "-",
        { label: showSprites ? "Hide sprites" : "Show sprites", disabled: !level, onClick: toggleSprites },
        "-",
        { label: "Preferences…", keys: "Ctrl+,", onClick: onPrefs },
      ]}
    />
  );
}

function Row({ label, hint, children }: { label: string; hint?: string; children: ReactNode }) {
  return (
    <label className="pref-row">
      <span>
        <b>{label}</b>
        {hint && <small>{hint}</small>}
      </span>
      {children}
    </label>
  );
}

/** Settings for this computer. */
export function PreferencesDialog({ onClose, onResetLayout }: { onClose: () => void; onResetLayout: () => void }) {
  const [p, setP] = useState<Prefs>(loadPrefs);
  const [tools, setTools] = useState<{ lunar_magic: string | null; addmusick: string | null } | null>(null);
  useEffect(() => { invoke<typeof tools>("tools_status").then(setTools).catch(() => {}); }, []);
  const change = (patch: Partial<Prefs>) => {
    const next = { ...p, ...patch };
    setP(next);
    savePrefs(next);
  };
  return (
    <>
      <div className="modal-backdrop" onMouseDown={onClose} />
      <div className="modal prefs">
        <h3>Preferences</h3>
        <div className="modal-label">Starting up</div>
        <Row label="Reopen my last ROM" hint="When SMW Studio starts, open the ROM you had open">
          <input type="checkbox" checked={p.reopenLastRom} onChange={(e) => change({ reopenLastRom: e.target.checked })} />
        </Row>
        <div className="modal-label">Level view</div>
        <Row label="Zoom" hint="How big levels show when opened">
          <select value={p.zoom} onChange={(e) => change({ zoom: Number(e.target.value) })}>
            {[1, 2, 3, 4].map((z) => <option key={z} value={z}>{z * 100}%</option>)}
          </select>
        </Row>
        <Row label="Show sprites" hint="Draw enemies and items on levels">
          <input type="checkbox" checked={p.showSprites} onChange={(e) => change({ showSprites: e.target.checked })} />
        </Row>
        <Row label="Window layout" hint="Put every panel back where it started">
          <button className="menu" onClick={() => { onResetLayout(); onClose(); }}>Reset layout</button>
        </Row>
        <div className="modal-label">Helper tools</div>
        <Row label="Lunar Magic" hint={tools?.lunar_magic ?? "Not found: adding Toolbox levels and graphics needs it"}>
          <span className={tools?.lunar_magic ? "pref-ok" : "pref-missing"}>{tools?.lunar_magic ? "Found" : "Missing"}</span>
        </Row>
        <Row label="AddmusicK" hint={tools?.addmusick ?? "Not found: custom music needs it"}>
          <span className={tools?.addmusick ? "pref-ok" : "pref-missing"}>{tools?.addmusick ? "Found" : "Missing"}</span>
        </Row>
        <div className="modal-buttons">
          <button className="menu" onClick={onClose}>Done</button>
        </div>
      </div>
    </>
  );
}

type UpgradeView = { id: string; installed: boolean; blocked: string };

const UPGRADES: Record<string, { name: string; what: string }> = {
  checkpoints: {
    name: "Multiple checkpoints",
    what: "Every midway tape in a level becomes its own checkpoint: after dying, Mario restarts at the last one he touched (like Mario Maker), not always the level's one midway point. Place more midway tapes from the Objects panel.",
  },
};

/** Patches that lift limits of the original game, as plain switches. */
export function UpgradesDialog({ onClose }: { onClose: () => void }) {
  const [list, setList] = useState<UpgradeView[] | null>(null);
  const [error, setError] = useState("");
  const load = () => invoke<UpgradeView[]>("upgrades").then(setList).catch((e) => setError(String(e)));
  useEffect(() => { load(); }, []);
  const add = async (id: string) => {
    try {
      await invoke("add_upgrade", { id });
      await refreshDirty();
      await load();
    } catch (e) {
      setError(String(e));
    }
  };
  return (
    <>
      <div className="modal-backdrop" onMouseDown={onClose} />
      <div className="modal prefs">
        <h3>Engine upgrades</h3>
        <p className="hint">Changes to how the game itself works, beyond what the original allows. Each one checks your ROM first and is saved with your next Save.</p>
        {error && <p className="pref-missing">{error}</p>}
        {list?.map((u) => (
          <div key={u.id} className="pref-row">
            <span>
              <b>{UPGRADES[u.id]?.name ?? u.id}</b>
              <small style={{ whiteSpace: "normal" }}>{u.blocked || UPGRADES[u.id]?.what}</small>
            </span>
            <Toggle on={u.installed} disabled={u.installed || !!u.blocked} label={u.installed ? "On (stays in this ROM)" : "Turn on"} onChange={() => add(u.id)} />
          </div>
        ))}
        <p className="hint">Coming: more enemies on screen, stackable springboards and items, more exits per screen. Community patches (ground pound, wall jump…) are in <b>Tools → Patches</b>.</p>
        <div className="modal-buttons">
          <button className="menu" onClick={onClose}>Done</button>
        </div>
      </div>
    </>
  );
}

/** An on/off switch (green when on). */
export function Toggle({ on, disabled, onChange, label }: { on: boolean; disabled?: boolean; onChange: (on: boolean) => void; label: string }) {
  return (
    <button type="button" role="switch" aria-checked={on} aria-label={label} title={label}
      className={"toggle-switch" + (on ? " on" : "")} disabled={disabled} onClick={() => onChange(!on)}>
      <span className="knob" />
    </button>
  );
}

/** Tools → Patches: community patches and per-frame code, switched on and off per ROM. */
export function PatchesDialog({ onClose }: { onClose: () => void }) {
  const { rom, loading } = useStudio();
  const [list, setList] = useState<PatchView[]>([]);
  const [open, setOpen] = useState<string | null>(null);
  const [filter, setFilter] = useState("");
  // The result of the last thing done here, shown in the window (good or bad)
  const [result, setResult] = useState<{ ok: boolean; text: string } | null>(null);
  const lastPatchLog = () => [...studioState().log].reverse().find((l) => l.startsWith("Patches: ")) ?? "";
  const toggle = async (p: PatchView, on: boolean) => {
    setResult({ ok: true, text: `${on ? "Putting in" : "Taking out"} ${p.name}… (your work is saved first)` });
    const ok = await setPatch(p, on, settingsOf(p));
    setResult({ ok, text: lastPatchLog().replace(/^Patches: /, "") });
    load();
  };
  const [edits, setEdits] = useState<Record<string, PatchSetting[]>>({});
  const [error, setError] = useState("");
  const load = () =>
    invoke<PatchView[]>("patches_list")
      .then((l) => { setList(l); setError(""); })
      .catch((e) => setError(String(e)));
  // Again when the ROM changes or a job finishes (the list can't be read mid-job)
  useEffect(() => { load(); }, [rom?.path, loading]);
  const settingsOf = (p: PatchView) => edits[p.id] ?? p.settings;
  const change = (p: PatchView, i: number, value: string) =>
    setEdits({ ...edits, [p.id]: settingsOf(p).map((s, j) => (j === i ? { ...s, value } : s)) });
  const shown = list.filter((p) => p.name.toLowerCase().includes(filter.toLowerCase()));
  const forget = async (p: PatchView) => {
    try {
      await invoke("patch_forget", { id: p.id });
      load();
    } catch (e) {
      alert(String(e));
    }
  };
  return (
    <>
      <div className="modal-backdrop" onMouseDown={onClose} />
      <div className="modal prefs patches">
      <h3>Patches</h3>
      <p className="hint">Community patches and per-frame code (Asar .asm and UberASM files, like a ground pound or wall jump): add one once, then switch it on or off in any ROM. SMW Studio saves your work first, remembers every byte a patch changes, and can take it out again.</p>
      <div className="patch-bar">
        <input className="search" placeholder="Search patches…" value={filter} onChange={(e) => setFilter(e.target.value)} />
        <button className="menu" onClick={async () => {
          const name = await importPatch();
          if (name) setResult({ ok: true, text: `Added ${name}. Switch it on below.` });
          else { const l = lastPatchLog(); if (l) setResult({ ok: false, text: l.replace(/^Patches: /, "") }); }
          load();
        }}>Add patch (.asm or .zip)…</button>
      </div>
      {result && <div className={"patch-result " + (result.ok ? "ok" : "bad")}>{result.text}</div>}
      {error && <p className="pref-missing">{error}</p>}
      {!error && list.length === 0 && <p className="hint">No patches yet. Add one: most come as a .zip from SMW Central.</p>}
      <div className="patch-list">
      {shown.map((p) => (
        <div key={p.id}>
          <div className="pref-row">
            <span>
              <b>{p.name}</b>
              <small style={{ whiteSpace: "normal" }}>
                {p.on ? "In this ROM" : "Not in this ROM"}
                {p.other.length > 0 && ` · also came with ${p.other.join(", ")} (not applied)`}
              </small>
            </span>
            <span style={{ display: "flex", gap: 4 }}>
              {!p.on && <button className="menu" onClick={() => setOpen(open === p.id ? null : p.id)}>Settings</button>}
              {!p.on && <button className="menu" title="Remove it from SMW Studio's library" onClick={() => forget(p)}>Remove</button>}
              <Toggle on={p.on} disabled={loading || !rom} label={p.on ? `Turn ${p.name} off` : `Turn ${p.name} on`}
                onChange={(on) => toggle(p, on)} />
            </span>
          </div>
          {open === p.id && !p.on && (
            <div className="patch-settings">
              {settingsOf(p).map((s, i) => (
                <label key={s.name} title={s.comment}>
                  <code>!{s.name}</code>
                  <input value={s.value} onChange={(e) => change(p, i, e.target.value)} />
                  <small>{s.comment}</small>
                </label>
              ))}
            </div>
          )}
        </div>
      ))}
      </div>
      <div className="modal-buttons">
        <button className="menu" onClick={onClose}>Done</button>
      </div>
      </div>
    </>
  );
}

export function ToolsMenu({ onUpgrades, onPatches }: { onUpgrades: () => void; onPatches: () => void }) {
  const { rom } = useStudio();
  return (
    <Menu title="Tools" items={() => [
      { label: "Engine upgrades…", disabled: !rom, onClick: onUpgrades },
      { label: "Patches…", disabled: !rom, onClick: onPatches },
    ]} />
  );
}
