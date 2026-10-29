import { useState } from "react";
import { openRom, setTool, useStudio } from "./store";

const SEEN = "smw-studio.welcomed";

export function welcomeSeen() {
  try {
    return localStorage.getItem(SEEN) === "1";
  } catch {
    return true;
  }
}

/** First-time tour: a few cards that get someone from nothing to playing their own edit. */
export default function Welcome({ onClose, onDocs }: { onClose: () => void; onDocs: () => void }) {
  const { rom } = useStudio();
  const [step, setStep] = useState(0);
  const done = () => {
    try { localStorage.setItem(SEEN, "1"); } catch {}
    onClose();
  };
  const steps = [
    {
      icon: "🍄",
      title: "Welcome to SMW Studio",
      body: "Make your own Super Mario World: edit levels, enemies and music, and play your changes right here. This quick tour shows the basics.",
    },
    {
      icon: "📂",
      title: "Open your ROM",
      body: rom
        ? `You have ${rom.path.split(/[\\/]/).pop()} open. Next time, File → Open ROM… (Ctrl+O) opens another.`
        : "Pick your Super Mario World (USA) ROM file (.sfc or .smc). SMW Studio backs it up before saving anything.",
      action: rom ? null : { label: "Open ROM…", run: openRom },
    },
    {
      icon: "🗺️",
      title: "Pick a level",
      body: "The Explorer on the left lists every level with its name, like 105 Yoshi's Island 1. Click one to open it. Scroll to look around, Ctrl+scroll to zoom.",
    },
    {
      icon: "✏️",
      title: "Paint with the tools",
      body: "Pick a block in the Objects panel, then drag in the level to paint (Draw, B). Fill (G) fills a box, Erase (E) wipes, Sprites (S) places enemies. Every drag is one Undo (Ctrl+Z).",
      action: { label: "Use Draw", run: () => setTool("draw") },
    },
    {
      icon: "▶️",
      title: "Play it",
      body: "Press ▶ Play (top right) to play the level with your changes. Arrows to move, X to jump, Esc to stop. Your ROM file only changes when you Save (Ctrl+S).",
    },
    {
      icon: "📖",
      title: "Learn everything in Docs",
      body: "The Docs tab has a tutorial for everything: sprites, pipes and entrances, layers, the Toolbox, music and MIDI songs, engine upgrades and shortcuts. You can see this tour again from Help.",
      action: { label: "Open Docs", run: () => { done(); onDocs(); } },
    },
  ];
  const s = steps[step];
  const last = step === steps.length - 1;
  return (
    <>
      <div className="modal-backdrop" />
      <div className="modal welcome">
        <div className="welcome-icon">{s.icon}</div>
        <h3>{s.title}</h3>
        <p>{s.body}</p>
        {s.action && <button className="menu welcome-action" onClick={s.action.run}>{s.action.label}</button>}
        <div className="welcome-dots">
          {steps.map((_, i) => <i key={i} className={i === step ? "on" : ""} onClick={() => setStep(i)} />)}
        </div>
        <div className="modal-buttons">
          <button className="menu" onClick={done}>Skip</button>
          {step > 0 && <button className="menu" onClick={() => setStep(step - 1)}>Back</button>}
          <button className="menu welcome-next" onClick={() => (last ? done() : setStep(step + 1))}>{last ? "Let's go!" : "Next"}</button>
        </div>
      </div>
    </>
  );
}
