<script lang="ts">
  // Markdown editor with a formatting toolbar and a Write|Preview toggle.
  // Wraps/prefixes the current textarea selection for each format, then calls
  // onChange. Preview reuses the app's RichText renderer. Fully themed.
  import Icon from "./Icon.svelte";
  import RichText from "./RichText.svelte";

  let { value, onChange }: { value: string; onChange: (v: string) => void } = $props();

  let ta = $state<HTMLTextAreaElement | null>(null);
  let tab = $state<"write" | "preview">("write");

  // Apply a transform to the current selection and restore the caret/selection.
  function applySelection(transform: (sel: string) => { text: string; selStart: number; selEnd: number }) {
    const el = ta;
    if (!el) return;
    const start = el.selectionStart;
    const end = el.selectionEnd;
    const before = value.slice(0, start);
    const sel = value.slice(start, end);
    const after = value.slice(end);
    const { text, selStart, selEnd } = transform(sel);
    const next = before + text + after;
    onChange(next);
    // Restore focus + selection after Svelte updates the bound value.
    queueMicrotask(() => {
      el.focus();
      el.setSelectionRange(start + selStart, start + selEnd);
    });
  }

  // Wrap selection in a marker (bold/italic/inline code). If empty, drops in a
  // placeholder and selects it so the user can type over it.
  function wrap(marker: string, placeholder: string) {
    applySelection((sel) => {
      const inner = sel || placeholder;
      const text = marker + inner + marker;
      return { text, selStart: marker.length, selEnd: marker.length + inner.length };
    });
  }

  // Prefix each selected line (lists, quote, headings).
  function prefixLines(make: (line: string, i: number) => string, placeholder: string) {
    applySelection((sel) => {
      const body = sel || placeholder;
      const lines = body.split("\n");
      const text = lines.map((l, i) => make(l, i)).join("\n");
      return { text, selStart: 0, selEnd: text.length };
    });
  }

  function codeBlock() {
    applySelection((sel) => {
      const inner = sel || "code";
      const text = "```\n" + inner + "\n```";
      return { text, selStart: 4, selEnd: 4 + inner.length };
    });
  }

  function link() {
    applySelection((sel) => {
      const label = sel || "text";
      const text = "[" + label + "](url)";
      // Select the "url" portion so it's easy to replace.
      const urlStart = label.length + 3;
      return { text, selStart: urlStart, selEnd: urlStart + 3 };
    });
  }

  type Tool = { id: string; label: string; icon?: string; glyph?: string; run: () => void };
  const tools: Tool[] = [
    { id: "bold", label: "Bold", glyph: "B", run: () => wrap("**", "bold") },
    { id: "italic", label: "Italic", glyph: "I", run: () => wrap("*", "italic") },
    { id: "h2", label: "Heading", glyph: "H2", run: () => prefixLines((l) => "## " + l, "Heading") },
    { id: "ul", label: "Bullet list", glyph: "•", run: () => prefixLines((l) => "- " + l, "List item") },
    { id: "ol", label: "Numbered list", glyph: "1.", run: () => prefixLines((l, i) => `${i + 1}. ` + l, "List item") },
    { id: "code", label: "Inline code", glyph: "`", run: () => wrap("`", "code") },
    { id: "codeblock", label: "Code block", glyph: "{ }", run: codeBlock },
    { id: "link", label: "Link", icon: "link", run: link },
    { id: "quote", label: "Quote", glyph: "❝", run: () => prefixLines((l) => "> " + l, "Quote") },
  ];
</script>

<div class="md">
  <div class="md-bar">
    <div class="md-tools" role="toolbar" aria-label="Formatting">
      {#each tools as t}
        <button
          type="button"
          class="md-tool"
          title={t.label}
          aria-label={t.label}
          disabled={tab !== "write"}
          onclick={t.run}
        >
          {#if t.icon}<Icon name={t.icon} size={14} />{:else}<span class="md-glyph">{t.glyph}</span>{/if}
        </button>
      {/each}
    </div>
    <div class="md-seg" role="tablist" aria-label="Editor view">
      <button
        type="button"
        role="tab"
        aria-selected={tab === "write"}
        class={"md-segbtn" + (tab === "write" ? " on" : "")}
        onclick={() => (tab = "write")}
      >Write</button>
      <button
        type="button"
        role="tab"
        aria-selected={tab === "preview"}
        class={"md-segbtn" + (tab === "preview" ? " on" : "")}
        onclick={() => (tab = "preview")}
      >Preview</button>
    </div>
  </div>

  {#if tab === "write"}
    <textarea
      bind:this={ta}
      class="md-area"
      spellcheck="true"
      placeholder="Write your notes in Markdown…"
      value={value}
      oninput={(e) => onChange((e.target as HTMLTextAreaElement).value)}
    ></textarea>
  {:else}
    <div class="md-preview">
      {#if value.trim()}
        <RichText text={value} />
      {:else}
        <div class="md-empty">Nothing to preview yet.</div>
      {/if}
    </div>
  {/if}
</div>

<style>
  .md {
    display: flex;
    flex-direction: column;
    border: 1px solid var(--border-strong);
    border-radius: var(--r-lg, 12px);
    background: var(--surface);
    overflow: hidden;
  }
  .md-bar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    flex-wrap: wrap;
    padding: 6px 8px;
    border-bottom: 1px solid var(--border);
    background: var(--surface-2);
  }
  .md-tools { display: flex; align-items: center; gap: 2px; flex-wrap: wrap; }
  .md-tool {
    display: inline-flex; align-items: center; justify-content: center;
    min-width: 28px; height: 28px; padding: 0 6px;
    font: inherit; cursor: pointer; border-radius: 6px;
    background: none; border: 1px solid transparent; color: var(--fg-muted);
    transition: background 0.1s ease, color 0.1s ease, border-color 0.1s ease;
  }
  .md-tool:hover:not(:disabled) { color: var(--fg-bright); background: var(--surface-3); }
  .md-tool:disabled { opacity: 0.4; cursor: default; }
  .md-glyph {
    font-family: var(--font-mono); font-size: var(--t-xs, 12px); font-weight: 700; line-height: 1;
  }
  .md-seg {
    display: inline-flex; gap: 2px; padding: 2px;
    border-radius: 999px; background: var(--surface-3); border: 1px solid var(--border);
  }
  .md-segbtn {
    font: inherit; font-size: var(--t-2xs, 10.5px); font-weight: 600; letter-spacing: 0.04em;
    cursor: pointer; padding: 3px 12px; border-radius: 999px;
    background: none; border: none; color: var(--fg-muted);
    transition: background 0.1s ease, color 0.1s ease;
  }
  .md-segbtn:hover { color: var(--fg-bright); }
  .md-segbtn.on { color: var(--accent-fg); background: var(--accent); }
  .md-area {
    width: 100%; box-sizing: border-box; resize: vertical;
    min-height: 280px; padding: 14px 16px;
    font-family: var(--font-mono); font-size: var(--t-xs, 12.5px); line-height: 1.6;
    color: var(--fg-bright); background: var(--surface); border: none; outline: none;
    tab-size: 2;
  }
  .md-area::placeholder { color: var(--fg-faint); }
  .md-preview {
    min-height: 280px; padding: 14px 16px;
    color: var(--fg); font-size: var(--t-xs, 12.5px); overflow-y: auto;
  }
  .md-empty {
    color: var(--fg-faint); font-style: italic; font-size: var(--t-xs, 12px);
  }
</style>
