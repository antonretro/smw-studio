import { useMemo, useState, type ReactNode } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";

// The guide lives in docs/guide/ (the same pages people read on GitHub)
const files = import.meta.glob("../../docs/guide/*.md", { query: "?raw", import: "default", eager: true }) as Record<string, string>;

type Page = { file: string; title: string; text: string };

export const PAGES: Page[] = Object.entries(files)
  .sort(([a], [b]) => a.localeCompare(b))
  .map(([path, text]) => ({ file: path.split("/").pop()!, title: text.match(/^# (.+)$/m)?.[1] ?? path, text }));

/** Inline Markdown: **bold**, `code`, [links](...). */
function inline(text: string, go: (file: string) => void): ReactNode[] {
  const out: ReactNode[] = [];
  const re = /\*\*(.+?)\*\*|`([^`]+)`|\[([^\]]+)\]\(([^)]+)\)/g;
  let last = 0;
  let m: RegExpExecArray | null;
  while ((m = re.exec(text))) {
    if (m.index > last) out.push(text.slice(last, m.index));
    const k = out.length;
    if (m[1] !== undefined) out.push(<b key={k}>{inline(m[1], go)}</b>);
    else if (m[2] !== undefined) out.push(<code key={k}>{m[2]}</code>);
    else {
      const href = m[4];
      out.push(
        <a key={k} href="#" onClick={(e) => { e.preventDefault(); if (/^https?:/.test(href)) openUrl(href); else go(href); }}>
          {m[3]}
        </a>,
      );
    }
    last = m.index + m[0].length;
  }
  if (last < text.length) out.push(text.slice(last));
  return out;
}

/** The small part of Markdown the guide uses: headings, paragraphs, lists (nested by indent). */
function render(md: string, go: (file: string) => void): ReactNode[] {
  const out: ReactNode[] = [];
  const lines = md.split(/\r?\n/);
  let i = 0;
  while (i < lines.length) {
    const line = lines[i];
    const h = line.match(/^(#{1,3}) (.+)$/);
    if (h) {
      const level = h[1].length;
      const content = inline(h[2], go);
      out.push(level === 1 ? <h1 key={i}>{content}</h1> : level === 2 ? <h2 key={i}>{content}</h2> : <h3 key={i}>{content}</h3>);
      i++;
    } else if (/^\s*(- |\d+\. )/.test(line)) {
      // A list: collect it with its indented sub-lists
      const items: { depth: number; ordered: boolean; text: string }[] = [];
      while (i < lines.length && /^\s*(- |\d+\. )/.test(lines[i])) {
        const m = lines[i].match(/^(\s*)(- |\d+\. )(.*)$/)!;
        items.push({ depth: Math.floor(m[1].length / 3), ordered: m[2] !== "- ", text: m[3] });
        i++;
      }
      // Sub-lists go inside the item before them (so numbering doesn't skip)
      const build = (from: number, depth: number): [ReactNode, number] => {
        const lis: { text: string; sub: ReactNode[] }[] = [];
        let j = from;
        const ordered = items[from].ordered;
        while (j < items.length && items[j].depth >= depth) {
          if (items[j].depth > depth && lis.length) {
            const [sub, next] = build(j, items[j].depth);
            lis[lis.length - 1].sub.push(sub);
            j = next;
          } else {
            lis.push({ text: items[j].text, sub: [] });
            j++;
          }
        }
        const nodes = lis.map((li, k) => <li key={k}>{inline(li.text, go)}{li.sub}</li>);
        return [ordered ? <ol key={from}>{nodes}</ol> : <ul key={from}>{nodes}</ul>, j];
      };
      out.push(build(0, items[0].depth)[0]);
    } else if (line.trim() === "") {
      i++;
    } else {
      const para: string[] = [];
      while (i < lines.length && lines[i].trim() !== "" && !/^(#{1,3} |\s*- |\s*\d+\. )/.test(lines[i])) para.push(lines[i++]);
      out.push(<p key={i}>{inline(para.join(" "), go)}</p>);
    }
  }
  return out;
}

let openPage: ((file: string) => void) | null = null;

/** Show a page of the guide (from elsewhere in the app). */
export function showDoc(file: string) {
  openPage?.(file);
}

/** Docs tab: the user guide, with tutorials for everything. */
export default function DocsPanel() {
  const [file, setFile] = useState(PAGES[0]?.file ?? "");
  const [q, setQ] = useState("");
  openPage = setFile;
  const page = PAGES.find((p) => p.file === file) ?? PAGES[0];
  const shown = useMemo(() => {
    const n = q.trim().toLowerCase();
    return n ? PAGES.filter((p) => p.text.toLowerCase().includes(n)) : PAGES;
  }, [q]);
  return (
    <div className="panel docs">
      <nav className="docs-nav">
        <input className="search" placeholder="Search the docs…" value={q} onChange={(e) => setQ(e.target.value)} />
        {shown.map((p) => (
          <button key={p.file} className={"docs-link" + (p.file === page?.file ? " on" : "")} onClick={() => setFile(p.file)}>
            {p.title}
          </button>
        ))}
        {shown.length === 0 && <p className="muted">Nothing found.</p>}
      </nav>
      <article className="docs-page" key={page?.file}>
        {page ? render(page.text, (f) => setFile(f.replace(/^.*\//, ""))) : <p>No docs found.</p>}
      </article>
    </div>
  );
}
