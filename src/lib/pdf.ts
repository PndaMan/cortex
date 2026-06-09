// PDF export. The webview's `window.print()` is unreliable on Linux WebKitGTK,
// so instead we serialize the already-rendered content to a self-contained HTML
// document (print styles inlined here, independent of the live theme) and hand
// it to the Rust `export_pdf` command, which renders it with headless Chromium.
//
// Callers pass the inner body HTML (typically `node.innerHTML` of the rendered
// cheatsheet/notes, so RichText's markup — tables, callouts, code — is preserved
// verbatim). We supply the page chrome + a clean light-on-white stylesheet.

import { save } from "@tauri-apps/plugin-dialog";
import * as api from "./api";
import { app } from "./store.svelte";

// Light, print-friendly theme. Defines the CSS variables RichText/cheatsheet
// markup reference (so any leftover var() resolves) AND concrete rules for the
// class names, since Svelte's scoped component styles don't travel with
// innerHTML.
const EXPORT_CSS = `
:root{
  --fg:#1f2328; --fg-bright:#0b0d0f; --fg-muted:#454c54; --fg-faint:#6b7280;
  --accent:#0f7f6b; --info:#2563eb; --ok:#15803d; --warn:#b45309; --err:#b91c1c;
  --border:#dcdfe4; --border-strong:#c2c7cf; --surface:#fff; --surface-2:#f4f6f8;
  --bg-sunken:#f4f6f8;
  --font-mono:"SFMono-Regular",ui-monospace,"JetBrains Mono",Menlo,Consolas,monospace;
}
*{box-sizing:border-box;}
@page{size:A4;margin:16mm 15mm;}
html,body{margin:0;padding:0;background:#fff;color:var(--fg);
  font-family:-apple-system,"Inter",Segoe UI,Roboto,Helvetica,Arial,sans-serif;
  font-size:13px;line-height:1.6;-webkit-print-color-adjust:exact;print-color-adjust:exact;
  orphans:3;widows:3;}
.cs-doc{max-width:none;margin:0;padding:0;}
.cs-doc + .cs-doc{margin-top:0;}
.cs-export-sheet{break-after:page;}
.cs-export-sheet:last-child{break-after:auto;}

/* document header */
.eyebrow{font-family:var(--font-mono);font-size:10.5px;letter-spacing:.08em;
  text-transform:uppercase;color:var(--fg-faint);margin-bottom:4px;}
.cs-title{font-size:30px;font-weight:650;color:var(--fg-bright);margin:0 0 6px;line-height:1.1;}
.cs-sub{font-family:var(--font-mono);font-size:11px;color:var(--fg-muted);margin:0 0 18px;}
.cs-doc-actions,.cs-tabs,.status-pill .dot{display:none !important;}

/* sections — note: NO break-inside:avoid here. Sections routinely run taller
   than a page; avoiding breaks inside them makes Chromium shove each one onto a
   fresh page (then break it anyway), stranding the prior page near-empty. We let
   sections flow and instead keep the smaller units (items, tables, callouts)
   together, and keep headings glued to what follows them. */
.cs-sections{display:block;}
.cs-section{margin:0 0 22px;padding:0 0 4px;border-bottom:1px solid var(--border);}
.cs-section:last-child{border-bottom:none;}
.cs-sec-head{display:flex;align-items:baseline;gap:10px;margin:0 0 12px;
  break-inside:avoid;break-after:avoid;}
/* Section titles are the dominant headings — big mono uppercase accent, matching
   the on-screen cheatsheet identity. */
.cs-sec-title{font-family:var(--font-mono);font-size:27px;font-weight:700;line-height:1.1;
  letter-spacing:.05em;text-transform:uppercase;color:var(--accent);margin:0;}
.cs-sec-count{font-family:var(--font-mono);font-size:11px;color:var(--fg-faint);}

/* images (section figure + inline markdown images) — bounded so they never blow
   past the page; print-color-adjust already set on body. */
.cs-sec-img{display:block;margin:0 0 12px;max-width:360px;border:1px solid var(--border);
  border-radius:7px;overflow:hidden;}
.cs-sec-img img{display:block;width:100%;height:auto;}
.rt-img{display:block;max-width:100%;height:auto;margin:8px 0;border:1px solid var(--border);
  border-radius:7px;}
.status-pill{font-family:var(--font-mono);font-size:10px;text-transform:uppercase;
  letter-spacing:.04em;color:var(--warn);}

/* topic dividers — present in the composed whole-subject sheet. Without these
   rules the labels fell back to plain body text; mirror the on-screen accent
   identity and keep each label glued to the topic it introduces. */
.cs-topic-divider{display:flex;align-items:center;gap:12px;margin:26px 0 6px;
  break-after:avoid;break-inside:avoid;}
.cs-topic-divider:first-child{margin-top:0;}
.cs-topic-divider::before,.cs-topic-divider::after{content:"";height:1px;flex:1;
  background:var(--border-strong);}
.cs-topic-divider-label{font-size:14px;font-weight:700;letter-spacing:.04em;
  text-transform:uppercase;color:var(--accent);white-space:nowrap;}
.cs-topic-subdiv{margin:24px 0 12px;padding-bottom:7px;
  border-bottom:2px solid var(--accent);break-after:avoid;break-inside:avoid;}
.cs-topic-subdiv:first-child{margin-top:4px;}
.cs-topic-subdiv-label{display:block;font-family:var(--font-mono);font-size:20px;
  font-weight:800;line-height:1.15;letter-spacing:.01em;color:var(--accent);
  overflow-wrap:anywhere;}

/* items */
.cs-list{margin:0;}
.cs-item{break-inside:avoid;margin:0 0 13px;}
.cs-item dt{font-weight:650;color:var(--fg-bright);font-size:13.5px;margin:0 0 3px;
  break-after:avoid;}
.cs-item dd{margin:0 0 0 0;color:var(--fg);}

/* ── RichText markup ───────────────────────────────────── */
.richtext{line-height:1.6;}
.rt-p{margin:0 0 9px;}.rt-p:last-child{margin-bottom:0;}
.rt-h2{margin:14px 0 7px;font-size:14.5px;font-weight:600;color:var(--fg-bright);break-after:avoid;}
.rt-h3{margin:12px 0 5px;font-size:13px;font-weight:600;color:var(--fg-bright);break-after:avoid;}
.rt-h2:first-child,.rt-h3:first-child{margin-top:0;}
.rt-hr{border:none;border-top:1px solid var(--border-strong);margin:13px 0;}
.rt-ul,.rt-ol{margin:0 0 9px;padding-left:20px;}
.rt-ul li,.rt-ol li{margin:3px 0;}
.rt-ul{list-style:disc;}.rt-ol{list-style:decimal;}
.rt-pre{margin:0 0 9px;padding:10px 12px;border-radius:7px;background:var(--bg-sunken);
  border:1px solid var(--border);overflow-x:auto;font-size:11.5px;line-height:1.45;
  white-space:pre-wrap;word-break:break-word;}
.rt-pre code{font-family:var(--font-mono);color:var(--fg);}
.rt-code{font-family:var(--font-mono);font-size:.88em;padding:1px 5px;border-radius:5px;
  background:#eceff2;}
strong{color:var(--fg-bright);font-weight:650;}
em{font-style:italic;}

/* tables */
.rt-table-wrap{margin:0 0 11px;border:1px solid var(--border);border-radius:7px;
  overflow:hidden;break-inside:avoid;}
.rt-table{border-collapse:collapse;width:100%;font-size:12px;}
.rt-table th,.rt-table td{padding:6px 10px;text-align:left;vertical-align:top;
  border-bottom:1px solid var(--border);border-right:1px solid var(--border);}
.rt-table th:last-child,.rt-table td:last-child{border-right:none;}
.rt-table tbody tr:last-child td{border-bottom:none;}
.rt-table thead th{background:var(--surface-2);color:var(--fg-bright);font-weight:600;}
.rt-table tbody tr:nth-child(even) td{background:#fafbfc;}

/* callouts */
.rt-callout{--co:var(--accent);margin:0 0 11px;padding:8px 13px;border-left:3px solid var(--co);
  border-radius:0 7px 7px 0;background:#f3f6f5;break-inside:avoid;}
.rt-callout--note{--co:var(--info);background:#eef3fd;}
.rt-callout--tip{--co:var(--ok);background:#eef7f0;}
.rt-callout--warning{--co:var(--warn);background:#fbf3e8;}
.rt-callout--important{--co:var(--err);background:#fbecec;}
.rt-callout--example{--co:var(--fg-faint);background:#f4f6f8;}
.rt-callout-label{display:flex;align-items:center;gap:6px;font-size:10.5px;font-weight:700;
  letter-spacing:.04em;text-transform:uppercase;color:var(--co);margin-bottom:4px;}
.rt-callout-ico{width:7px;height:7px;border-radius:50%;background:var(--co);flex:none;}
.rt-callout-body .rt-p:last-child{margin-bottom:0;}
.rt-quote{margin:0 0 11px;padding:3px 0 3px 13px;border-left:3px solid var(--border-strong);
  color:var(--fg-muted);font-style:italic;}
.rt-cite{font:inherit;background:none;border:none;color:var(--info);padding:0;}

/* bar chart */
.rt-chart{margin:0 0 12px;display:flex;flex-direction:column;gap:6px;break-inside:avoid;}
.rt-chart-row{display:grid;grid-template-columns:minmax(70px,30%) 1fr auto;align-items:center;gap:10px;font-size:12px;}
.rt-chart-label{color:var(--fg);overflow:hidden;text-overflow:ellipsis;white-space:nowrap;}
.rt-chart-track{height:11px;border-radius:6px;background:#e9edf1;overflow:hidden;}
.rt-chart-bar{display:block;height:100%;border-radius:6px;background:var(--accent);}
.rt-chart-val{color:var(--fg-muted);font-size:11px;text-align:right;}

/* notes export */
.note-export h1{font-size:26px;color:var(--fg-bright);margin:0 0 4px;}
.note-export .note-meta{font-family:var(--font-mono);font-size:11px;color:var(--fg-faint);
  margin:0 0 18px;}
`;

function docHtml(bodyHtml: string): string {
  return `<!doctype html><html><head><meta charset="utf-8">
<style>${EXPORT_CSS}</style></head><body>${bodyHtml}</body></html>`;
}

/**
 * Build a full HTML document from rendered body markup and render it to a PDF
 * the user picks via the save dialog. Returns true if a file was written.
 */
export async function savePdf(bodyHtml: string, defaultName: string): Promise<boolean> {
  const safe = defaultName.replace(/[^\w.\- ]+/g, "").trim() || "cortex-export";
  const dest = await save({
    defaultPath: safe.endsWith(".pdf") ? safe : `${safe}.pdf`,
    filters: [{ name: "PDF", extensions: ["pdf"] }],
  });
  if (!dest) return false; // user cancelled
  try {
    await api.exportPdf(docHtml(bodyHtml), dest);
    app.pushToast({ kind: "success", title: "Saved PDF", body: dest });
    return true;
  } catch (e) {
    app.pushToast({ kind: "error", title: "PDF export failed", body: String(e) });
    return false;
  }
}
