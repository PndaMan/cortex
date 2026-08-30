<script lang="ts">
  // Markdown editor with a formatting toolbar and a Write|Preview toggle.
  // Wraps/prefixes the current textarea selection for each format, then calls
  // onChange. Preview reuses the app's RichText renderer. Fully themed.
  // Supports keyboard shortcuts: Ctrl/Cmd+B/I/K/E/1/2.
  import Icon from "./Icon.svelte";
  import RichText from "./RichText.svelte";
  import { t } from "../lib/i18n.svelte";

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

  // Insert an image from a local file as a data: URL (renders in the webview AND in
  // the headless-Chromium PDF export). Placed on its own line so RichText renders it
  // as a block image.
  let imgInput = $state<HTMLInputElement | null>(null);
  function pickImage() {
    imgInput?.click();
  }
  function onImageFile(e: Event) {
    const f = (e.target as HTMLInputElement).files?.[0];
    if (imgInput) imgInput.value = ""; // allow re-picking the same file
    if (!f) return;
    const reader = new FileReader();
    reader.onload = () => {
      const url = reader.result as string;
      applySelection((sel) => {
        const alt = sel || "image";
        const text = `\n![${alt}](${url})\n`;
        return { text, selStart: 3, selEnd: 3 + alt.length };
      });
    };
    reader.readAsDataURL(f);
  }

  function bold()   { wrap("**", "bold"); }
  function italic() { wrap("*", "italic"); }
  function code()   { wrap("`", "code"); }
  function h1()     { prefixLines((l) => "# " + l, "Heading 1"); }
  function h2()     { prefixLines((l) => "## " + l, "Heading 2"); }

  // Keyboard shortcut handler for the textarea.
  function handleKeydown(e: KeyboardEvent) {
    const mod = e.ctrlKey || e.metaKey;
    if (!mod) return;
    switch (e.key) {
      case "b": case "B": e.preventDefault(); bold(); break;
      case "i": case "I": e.preventDefault(); italic(); break;
      case "k": case "K": e.preventDefault(); link(); break;
      case "e": case "E": e.preventDefault(); code(); break;
      case "1":           e.preventDefault(); h1(); break;
      case "2":           e.preventDefault(); h2(); break;
    }
  }

  type Tool = { id: string; label: string; icon?: string; glyph?: string; run: () => void };
  const tools: Tool[] = [
    { id: "bold",      label: t("Bold (⌘B)"),         glyph: "B",   run: bold },
    { id: "italic",    label: t("Italic (⌘I)"),        glyph: "I",   run: italic },
    { id: "h1",        label: t("Heading 1 (⌘1)"),     glyph: "H1",  run: h1 },
    { id: "h2",        label: t("Heading 2 (⌘2)"),     glyph: "H2",  run: h2 },
    { id: "ul",        label: t("Bullet list"),         glyph: "•",   run: () => prefixLines((l) => "- " + l, "List item") },
    { id: "ol",        label: t("Numbered list"),       glyph: "1.",  run: () => prefixLines((l, i) => `${i + 1}. ` + l, "List item") },
    { id: "code",      label: t("Inline code (⌘E)"),   glyph: "`",   run: code },
    { id: "codeblock", label: t("Code block"),          glyph: "{ }", run: codeBlock },
    { id: "link",      label: t("Link (⌘K)"),           icon: "link", run: link },
    { id: "image",     label: t("Insert image"),        icon: "camera", run: pickImage },
    { id: "quote",     label: t("Quote"),               glyph: "❝",  run: () => prefixLines((l) => "> " + l, "Quote") },
  ];
</script>

<div class="md">
  <input
    bind:this={imgInput}
    type="file"
    accept="image/*"
    style="display:none"
    onchange={onImageFile}
  />
  <div class="md-bar">
    <div class="md-tools" role="toolbar" aria-label={t("Formatting")}>
      {#each tools as tool}
        <button
          type="button"
          class="md-tool"
          title={tool.label}
          aria-label={tool.label}
          disabled={tab !== "write"}
          onclick={tool.run}
        >
          {#if tool.icon}<Icon name={tool.icon} size={14} />{:else}<span class="md-glyph">{tool.glyph}</span>{/if}
        </button>
      {/each}
    </div>
    <div class="md-seg" role="tablist" aria-label={t("Editor view")}>
      <button
        type="button"
        role="tab"
        aria-selected={tab === "write"}
        class={"md-segbtn" + (tab === "write" ? " on" : "")}
        onclick={() => (tab = "write")}
      >{t("Write")}</button>
      <button
        type="button"
        role="tab"
        aria-selected={tab === "preview"}
        class={"md-segbtn" + (tab === "preview" ? " on" : "")}
        onclick={() => (tab = "preview")}
      >{t("Preview")}</button>
    </div>
  </div>

  {#if tab === "write"}
    <textarea
      bind:this={ta}
      class="md-area"
      spellcheck="true"
      placeholder={t("Write your notes in Markdown…")}
      value={value}
      oninput={(e) => onChange((e.target as HTMLTextAreaElement).value)}
      onkeydown={handleKeydown}
    ></textarea>
  {:else}
    <div class="md-preview">
      <div class="md-preview-inner">
        {#if value.trim()}
          <RichText text={value} />
        {:else}
          <div class="md-empty">{t("Nothing to preview yet.")}</div>
        {/if}
      </div>
    </div>
  {/if}
</div>

<style>
  /* Fill whatever flex/grid cell the parent assigns — the parent (notes-detail)
     controls height; we just stretch to consume it. */
  .md {
    display: flex;
    flex-direction: column;
    flex: 1 1 0;
    min-height: 0;
    border: 1px solid var(--border-strong);
    border-radius: var(--r-lg, 12px);
    background: var(--surface);
    overflow: hidden;
  }
  /* Fixed-height toolbar row — never grows. */
  .md-bar {
    flex: none;
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
  /* Textarea stretches to fill all remaining editor height. No resize handle —
     the pane itself is already full-height, so manual resize adds no value. */
  .md-area {
    flex: 1 1 0;
    min-height: 0;
    width: 100%;
    box-sizing: border-box;
    resize: none;
    padding: 16px 20px;
    font-family: var(--font-mono); font-size: var(--t-xs, 12.5px); line-height: 1.7;
    color: var(--fg-bright); background: var(--surface); border: none; outline: none;
    tab-size: 2;
  }
  .md-area::placeholder { color: var(--fg-faint); }
  /* Preview fills height and scrolls; inner prose gets a comfortable line-length
     cap while the scroll container stays full-width. */
  .md-preview {
    flex: 1 1 0;
    min-height: 0;
    overflow-y: auto;
    padding: 0;
    color: var(--fg); font-size: var(--t-xs, 12.5px);
  }
  .md-preview-inner {
    max-width: 72ch;
    margin: 0 auto;
    padding: 16px 20px;
  }
  .md-empty {
    color: var(--fg-faint); font-style: italic; font-size: var(--t-xs, 12px);
  }
</style>
