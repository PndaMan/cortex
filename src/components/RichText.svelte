<script lang="ts">
  // Lightweight Markdown renderer for assistant answers — headings, lists, code
  // blocks, `---` dividers, bold/italic/inline-code, and ⟦source · loc⟧ citation
  // chips that are clickable (open the cited source). Rendered as real elements
  // (no innerHTML) so it's safe and the citations stay interactive.
  import { app } from "../lib/store.svelte";

  let { text }: { text: string } = $props();

  type Inline = { t: "text" | "b" | "i" | "code" | "cite"; v: string };
  type Block =
    | { type: "h2" | "h3" | "p"; text: string }
    | { type: "hr" }
    | { type: "code"; text: string }
    | { type: "ul" | "ol"; items: string[] };

  const INLINE_RE = /⟦([^⟧]+)⟧|\*\*([^*]+)\*\*|`([^`]+)`|\*([^*\n]+)\*/g;

  function inlineTokens(s: string): Inline[] {
    const out: Inline[] = [];
    let last = 0;
    let m: RegExpExecArray | null;
    INLINE_RE.lastIndex = 0;
    while ((m = INLINE_RE.exec(s)) !== null) {
      if (m.index > last) out.push({ t: "text", v: s.slice(last, m.index) });
      if (m[1] !== undefined) out.push({ t: "cite", v: m[1].trim() });
      else if (m[2] !== undefined) out.push({ t: "b", v: m[2] });
      else if (m[3] !== undefined) out.push({ t: "code", v: m[3] });
      else if (m[4] !== undefined) out.push({ t: "i", v: m[4] });
      last = m.index + m[0].length;
    }
    if (last < s.length) out.push({ t: "text", v: s.slice(last) });
    return out;
  }

  function parse(src: string): Block[] {
    const lines = (src ?? "").replace(/\r\n/g, "\n").split("\n");
    const blocks: Block[] = [];
    let para: string[] = [];
    let list: { type: "ul" | "ol"; items: string[] } | null = null;
    let code: string[] | null = null;

    const flushPara = () => {
      if (para.length) { blocks.push({ type: "p", text: para.join(" ") }); para = []; }
    };
    const flushList = () => { if (list) { blocks.push(list); list = null; } };

    for (const raw of lines) {
      const line = raw;
      const t = line.trim();
      if (code !== null) {
        if (t.startsWith("```")) { blocks.push({ type: "code", text: code.join("\n") }); code = null; }
        else code.push(line);
        continue;
      }
      if (t.startsWith("```")) { flushPara(); flushList(); code = []; continue; }
      if (/^(-{3,}|\*{3,}|_{3,})$/.test(t)) { flushPara(); flushList(); blocks.push({ type: "hr" }); continue; }
      const h = /^(#{2,3})\s+(.*)$/.exec(t);
      if (h) { flushPara(); flushList(); blocks.push({ type: h[1].length === 2 ? "h2" : "h3", text: h[2] }); continue; }
      const li = /^\s*([-*]|\d+\.)\s+(.*)$/.exec(line);
      if (li) {
        flushPara();
        const kind = /\d+\./.test(li[1]) ? "ol" : "ul";
        if (!list || list.type !== kind) { flushList(); list = { type: kind, items: [] }; }
        list.items.push(li[2]);
        continue;
      }
      if (t === "") { flushPara(); flushList(); continue; }
      flushList();
      para.push(t);
    }
    if (code !== null) blocks.push({ type: "code", text: code.join("\n") });
    flushPara(); flushList();
    return blocks;
  }

  const blocks = $derived(parse(text));

  function openCite(v: string) {
    // v looks like "source-name · p.14"; match the source by name.
    const name = v.split("·")[0].trim().toLowerCase();
    if (!name) return;
    const src =
      app.activeSources().find((s) => s.name.toLowerCase() === name) ??
      app.activeSources().find((s) => s.name.toLowerCase().includes(name) || name.includes(s.name.toLowerCase()));
    if (src) app.openSource(src);
    else app.pushToast({ kind: "info", title: "Source", body: v });
  }
</script>

{#snippet inline(s: string)}
  {#each inlineTokens(s) as tok}
    {#if tok.t === "b"}<strong>{tok.v}</strong>
    {:else if tok.t === "i"}<em>{tok.v}</em>
    {:else if tok.t === "code"}<code class="rt-code">{tok.v}</code>
    {:else if tok.t === "cite"}<button type="button" class="cite rt-cite" onclick={() => openCite(tok.v)}>{tok.v}</button>
    {:else}{tok.v}{/if}
  {/each}
{/snippet}

<div class="richtext">
  {#each blocks as b}
    {#if b.type === "hr"}
      <hr class="rt-hr" />
    {:else if b.type === "h2"}
      <h2 class="rt-h2">{@render inline(b.text)}</h2>
    {:else if b.type === "h3"}
      <h3 class="rt-h3">{@render inline(b.text)}</h3>
    {:else if b.type === "code"}
      <pre class="rt-pre"><code>{b.text}</code></pre>
    {:else if b.type === "ul"}
      <ul class="rt-ul">{#each b.items as it}<li>{@render inline(it)}</li>{/each}</ul>
    {:else if b.type === "ol"}
      <ol class="rt-ol">{#each b.items as it}<li>{@render inline(it)}</li>{/each}</ol>
    {:else if b.type === "p"}
      <p class="rt-p">{@render inline(b.text)}</p>
    {/if}
  {/each}
</div>

<style>
  .richtext { line-height: 1.6; }
  .rt-p { margin: 0 0 10px; }
  .rt-p:last-child { margin-bottom: 0; }
  .rt-h2 {
    margin: 16px 0 8px; font-size: 14.5px; font-weight: 600;
    color: var(--fg-bright); letter-spacing: 0.01em;
  }
  .rt-h3 { margin: 14px 0 6px; font-size: 13px; font-weight: 600; color: var(--fg-bright); }
  .rt-h2:first-child, .rt-h3:first-child { margin-top: 0; }
  .rt-hr { border: none; border-top: 1px solid var(--border-strong); margin: 14px 0; }
  .rt-ul, .rt-ol { margin: 0 0 10px; padding-left: 20px; }
  .rt-ul li, .rt-ol li { margin: 3px 0; }
  .rt-ul { list-style: disc; }
  .rt-ol { list-style: decimal; }
  .rt-pre {
    margin: 0 0 10px; padding: 10px 12px; border-radius: 8px;
    background: var(--bg-sunken, var(--surface-2)); border: 1px solid var(--border);
    overflow-x: auto; font-size: 12px; line-height: 1.45;
  }
  .rt-pre code { font-family: var(--font-mono); color: var(--fg); }
  .rt-code {
    font-family: var(--font-mono); font-size: 0.88em; padding: 1px 5px;
    border-radius: 5px; background: color-mix(in oklab, var(--fg) 10%, transparent);
  }
  strong { color: var(--fg-bright); font-weight: 600; }
  /* clickable citation chip */
  .rt-cite {
    font: inherit; cursor: pointer; border: none;
    transition: background 0.12s ease, color 0.12s ease;
  }
  .rt-cite:hover { filter: brightness(1.15); text-decoration: underline; }
</style>
