import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useStudio } from "./store";

const MAPS = ["Main map", "Yoshi's Island", "Vanilla Dome", "Forest of Illusion", "Valley of Bowser", "Special World", "Star World"];

/** Overworld tab: each map as the game draws it (every path and event shown). */
export default function OverworldPanel() {
  const { rom } = useStudio();
  const [map, setMap] = useState(0);
  const [picture, setPicture] = useState("");
  const [error, setError] = useState("");
  const [zoom, setZoom] = useState(1.5);
  useEffect(() => {
    if (!rom) return;
    setError("");
    invoke<string>("overworld_picture", { submap: map }).then(setPicture).catch((e) => { setPicture(""); setError(String(e)); });
  }, [rom?.path, map]);

  if (!rom) return <div className="panel ow-panel"><p className="muted">Open a ROM to see its overworld.</p></div>;
  return (
    <div className="panel ow-panel">
      <div className="ow-head">
        {MAPS.map((m, i) => (
          <button key={m} className={"menu" + (map === i ? " on" : "")} onClick={() => setMap(i)}>{m}</button>
        ))}
        <span className="mu-gap" />
        <button className="menu" onClick={() => setZoom((z) => Math.max(0.5, z - 0.5))} title="Zoom out">−</button>
        <button className="menu" onClick={() => setZoom((z) => Math.min(4, z + 0.5))} title="Zoom in">+</button>
      </div>
      <p className="muted">The map as the game draws it, with every path and event shown. {map > 0 ? "The submaps share one sheet; this is it in this submap's colours. " : ""}Editing comes next.</p>
      {error && <p className="ui-problem">{error}</p>}
      {picture && <div className="ow-view"><img src={picture} width={512 * zoom} height={512 * zoom} alt={MAPS[map]} /></div>}
    </div>
  );
}
