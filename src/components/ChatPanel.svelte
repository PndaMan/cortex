<script lang="ts">
  import { app } from "../lib/store.svelte";
  import Icon from "./Icon.svelte";
  import CiteText from "./CiteText.svelte";
  import type { Source } from "../lib/api";
  import * as api from "../lib/api";

  let { compact = false, onClose }: { compact?: boolean; onClose?: () => void } = $props();

  // ── scope state ──────────────────────────────────────────────────────────
  type Level = "subject" | "topic" | "source";
  const ORDER: Level[] = ["subject", "topic", "source"];

  interface ChatMessage { role: "system" | "user" | "assistant"; text: string }

  let level = $state<Level>("source");
  let srcId = $state<string | null>(null);
  let topicId = $state<string | null>(null);
  let messages = $state<ChatMessage[]>([]);
  let draft = $state("");
  let streaming = $state<string | null>(null);
  let scrollEl = $state<HTMLElement | null>(null);
  let modelLabel = $state<string | null>(null);
  let composeEl = $state<HTMLTextAreaElement | null>(null);

  // ── source-switcher overlay ────────────────────────────────────────────────
  let switcherOpen = $state(false);
  let switcherSel = $state(0); // highlighted row in the flat option list

  // All sources for the active subject, flattened across topics
  const topicSources = $derived<Source[]>(
    app.activeSubject?.topics?.flatMap((t) => t.sources) ?? []
  );

  // Resolve the active source object
  const curSrcObj = $derived<Source | null>(
    topicSources.find((s) => s.id === srcId) ?? topicSources[0] ?? null
  );

  // Resolve the active topic object (explicit topicId, else the source's topic, else first)
  const curTopic = $derived(
    app.activeSubject?.topics?.find((t) => t.id === topicId) ??
      app.activeSubject?.topics?.find((t) => t.id === curSrcObj?.topic_id) ??
      app.activeSubject?.topics?.[0] ??
      null
  );

  // Short display name for the source segment
  function shortName(name: string) {
    return name.replace(/\.[^.]+$/, "").replace(/^lecture-0?/, "lec-");
  }

  // Scope label derived from real data
  const activeScopeLabel = $derived<Record<Level, string>>({
    subject: "Subject: " + (app.activeSubject?.name ?? "—"),
    topic: "Topic: " + (curTopic?.name ?? "—"),
    source: curSrcObj ? "Source: " + curSrcObj.name : "Source",
  });

  // ── status-bar PWD sync ─────────────────────────────────────────────────────
  // Keep app.chatScope in lock-step with the chat's scope so the status bar
  // reflects subject › topic › source.
  $effect(() => {
    if (!app.activeSubject) {
      app.chatScope = null;
      return;
    }
    if (level === "source" && curSrcObj) {
      app.chatScope = { topicName: curTopic?.name, sourceName: curSrcObj.name };
    } else if (level === "topic") {
      app.chatScope = { topicName: curTopic?.name };
    } else {
      app.chatScope = null; // whole-subject
    }
  });

  // ── flat option list for the switcher (subject / topics / sources) ──────────
  type ScopeOption =
    | { kind: "subject"; label: string }
    | { kind: "topic"; label: string; topicId: string }
    | { kind: "source"; label: string; src: Source };

  const switcherOptions = $derived.by<ScopeOption[]>(() => {
    const out: ScopeOption[] = [
      { kind: "subject", label: app.activeSubject?.name ?? "Whole subject" },
    ];
    for (const t of app.activeSubject?.topics ?? []) {
      out.push({ kind: "topic", label: t.name, topicId: t.id });
      for (const s of t.sources) {
        out.push({ kind: "source", label: s.name, src: s });
      }
    }
    return out;
  });

  // Index of the option matching the current scope (for highlight on open)
  const currentOptionIndex = $derived.by(() => {
    const opts = switcherOptions;
    if (level === "subject") return 0;
    if (level === "source" && curSrcObj)
      return opts.findIndex((o) => o.kind === "source" && o.src.id === curSrcObj.id);
    if (level === "topic" && curTopic)
      return opts.findIndex((o) => o.kind === "topic" && o.topicId === curTopic.id);
    return 0;
  });

  // ── scope actions ─────────────────────────────────────────────────────────
  function changeScope(k: Level) {
    if (k === level) return;
    const widening = ORDER.indexOf(k) < ORDER.indexOf(level);
    level = k;
    messages = [
      ...messages,
      {
        role: "system",
        text: (widening ? "scope widened to " : "scope narrowed to ") + activeScopeLabel[k],
      },
    ];
  }

  // ── source-switcher overlay actions ─────────────────────────────────────────
  function openSwitcher() {
    if (!app.activeSubject) return;
    switcherSel = Math.max(0, currentOptionIndex);
    switcherOpen = true;
  }

  function applyOption(o: ScopeOption) {
    if (o.kind === "subject") {
      level = "subject";
      messages = [...messages, { role: "system", text: "scope set to whole subject" }];
    } else if (o.kind === "topic") {
      topicId = o.topicId;
      level = "topic";
      messages = [...messages, { role: "system", text: "scope set to Topic: " + o.label }];
    } else {
      srcId = o.src.id;
      topicId = o.src.topic_id ?? topicId;
      level = "source";
      messages = [...messages, { role: "system", text: "scope set to Source: " + o.label }];
    }
    switcherOpen = false;
    // Return focus to the composer so typing keeps working.
    composeEl?.focus();
  }

  // Focus the overlay so ArrowUp/Down/Enter/Esc are captured immediately.
  function autofocus(node: HTMLElement) {
    node.focus();
  }

  function switcherKey(e: KeyboardEvent) {
    const opts = switcherOptions;
    if (e.key === "ArrowDown") {
      e.preventDefault();
      switcherSel = (switcherSel + 1) % opts.length;
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      switcherSel = (switcherSel - 1 + opts.length) % opts.length;
    } else if (e.key === "Enter") {
      e.preventDefault();
      const o = opts[switcherSel];
      if (o) applyOption(o);
    } else if (e.key === "Escape") {
      e.preventDefault();
      switcherOpen = false;
      composeEl?.focus();
    }
  }

  // ── streaming send ────────────────────────────────────────────────────────
  async function send() {
    const text = draft.trim();
    if (!text || streaming !== null) return;
    if (!app.activeSubject) return;

    messages = [...messages, { role: "user", text }];
    draft = "";

    streaming = "";
    try {
      const sourceId =
        level === "source" && curSrcObj ? curSrcObj.id : undefined;
      const result = await api.chatAnswer(
        app.activeSubject.id,
        level,
        text,
        sourceId
      );
      // Typewriter effect over result.text
      const full = result.text;
      modelLabel = result.model || null;
      let i = 0;
      const iv = setInterval(() => {
        i += 2;
        streaming = full.slice(0, i);
        if (i >= full.length) {
          clearInterval(iv);
          streaming = null;
          messages = [...messages, { role: "assistant", text: full }];
        }
      }, 16);
    } catch (err) {
      streaming = null;
      const msg = err instanceof Error ? err.message : String(err);
      messages = [...messages, { role: "system", text: msg }];
    }
  }

  function handleKey(e: KeyboardEvent) {
    // Open the source switcher with Cmd/Ctrl+J even while typing.
    if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "j") {
      e.preventDefault();
      openSwitcher();
      return;
    }
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      send();
    }
  }

  // Root-level keybind for the whole panel: while the chat is focused, "s"
  // (in normal / non-typing mode) or Cmd/Ctrl+J opens the source switcher.
  function panelKey(e: KeyboardEvent) {
    if (switcherOpen) return; // overlay owns the keys while open
    if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "j") {
      e.preventDefault();
      openSwitcher();
      return;
    }
    // "s" opens the switcher only when not typing in the composer.
    const typing =
      app.mode === "INS" ||
      (e.target instanceof HTMLElement &&
        (e.target.tagName === "TEXTAREA" || e.target.tagName === "INPUT"));
    if (e.key === "s" && !typing && !e.metaKey && !e.ctrlKey && !e.altKey) {
      e.preventDefault();
      openSwitcher();
    }
  }

  // ── auto-scroll ───────────────────────────────────────────────────────────
  $effect(() => {
    // reactive on messages and streaming
    const _ = messages.length;
    const __ = streaming;
    if (scrollEl) scrollEl.scrollTop = scrollEl.scrollHeight;
  });

  // ── badge label map ───────────────────────────────────────────────────────
  const kindLabel: Record<string, string> = {
    pdf: "PDF",
    pptx: "PPTX",
    docx: "DOCX",
    web: "WEB",
    yt: "YT",
    audio: "AUD",
    image: "IMG",
  };
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="chatdock-inner" onkeydown={panelKey}>
  <!-- ── header ─────────────────────────────────────────────────────────── -->
  <div class="chat-head">
    {#if app.activeSubject}
      <!-- Scope breadcrumb (inlined) -->
      <div class="scope" role="group" aria-label="chat scope">
        <!-- Subject segment -->
        <span
          class="scope-seg{level === 'subject' ? ' is-active' : ''}"
          role="button"
          tabindex="0"
          title="Scope chat to {activeScopeLabel.subject}"
          onclick={() => changeScope("subject")}
          onkeydown={(e) => e.key === "Enter" && changeScope("subject")}
        >
          <span class="seg-ico"><Icon name="diamond" size={11} /></span>{app.activeSubject.name}
        </span>

        {#if curTopic}
          <span class="scope-sep">›</span>

          <!-- Topic segment -->
          <span
            class="scope-seg{level === 'topic' ? ' is-active' : ''}"
            role="button"
            tabindex="0"
            title="Scope chat to {activeScopeLabel.topic}"
            onclick={() => changeScope("topic")}
            onkeydown={(e) => e.key === "Enter" && changeScope("topic")}
          >
            <span class="seg-ico"><Icon name="chevron" size={11} /></span>{curTopic.name}
          </span>
        {/if}

        <!-- Source segment (only when a source is available) -->
        {#if curSrcObj}
          <span class="scope-sep">›</span>
          <div class="scope-srcwrap">
            <span
              class="scope-seg scope-seg--src{level === 'source' ? ' is-active' : ''}"
              role="button"
              tabindex="0"
              title="Switch scope — {curSrcObj.name} (s or ⌘J)"
              onclick={openSwitcher}
              onkeydown={(e) => { if (e.key === "Enter") openSwitcher(); }}
            >
              <span class="seg-ico"><Icon name="doc" size={11} /></span>
              <span class="src-seg-label">{shortName(curSrcObj.name)}</span>
              <Icon name="chevron" size={10} />
            </span>
          </div>
        {/if}
      </div>
    {:else}
      <span class="faint" style="font-size:12px">No subject open</span>
    {/if}

    <div class="grow"></div>
    <button class="btn btn--icon btn--sm btn--ghost" title="History">
      <Icon name="refresh" size={13} />
    </button>
    {#if onClose}
      <button class="btn btn--icon btn--sm btn--ghost" onclick={onClose} title="Close chat">
        <Icon name="x" size={12} />
      </button>
    {/if}
  </div>

  <!-- ── message list ───────────────────────────────────────────────────── -->
  <div class="chat-scroll" bind:this={scrollEl}>
    {#if !app.activeSubject}
      <div class="chat-empty-state">
        <div class="ces-ico">
          <Icon name="diamond" size={22} color="var(--fg3)" />
        </div>
        <div class="ces-title">Open a subject to start chatting</div>
        <div class="ces-sub">Ask questions grounded in your sources.</div>
      </div>
    {:else}
      <div class="chat-scope-note">
        <Icon name="diamond" size={9} color="var(--accent)" />
        Answers limited to <b>{activeScopeLabel[level]}</b>
      </div>

      {#each messages as m, i (i)}
        {#if m.role === "system"}
          <div class="bubble system">— {m.text} —</div>
        {:else if m.role === "user"}
          <div class="bubble user">{m.text}</div>
        {:else}
          <div class="bubble assistant"><CiteText text={m.text} /></div>
        {/if}
      {/each}

      {#if streaming !== null}
        <div class="bubble assistant">
          <CiteText text={streaming} /><span class="cursor-blink">▋</span>
        </div>
      {/if}
    {/if}
  </div>

  <!-- ── compose ───────────────────────────────────────────────────────── -->
  <div class="chat-compose">
    <div class="compose-box{app.mode === 'INS' ? ' is-insert' : ''}">
      <textarea
        bind:this={composeEl}
        rows={1}
        placeholder={!app.activeSubject
          ? "Open a subject first…"
          : app.mode === "INS"
          ? "Ask about " + activeScopeLabel[level] + "…"
          : "Press i to ask…"}
        bind:value={draft}
        disabled={!app.activeSubject}
        onfocus={() => app.setMode("INS")}
        onblur={() => app.setMode("NOR")}
        onkeydown={handleKey}
      ></textarea>
      <button
        class="btn btn--icon btn--sm btn--primary"
        onclick={send}
        disabled={!draft.trim() || !app.activeSubject}
        title="Send"
      >
        <Icon name="arrowR" size={13} />
      </button>
    </div>
    <div class="compose-hint">
      <span><span class="kbd">i</span> insert</span>
      <span><span class="kbd">⏎</span> send</span>
      <span><span class="kbd">⎋</span> normal</span>
      <span><span class="kbd">s</span> / <span class="kbd">⌘J</span> scope</span>
      <span style="margin-left:auto" class="faint">
        {#if modelLabel}
          <span class="model-tag">{modelLabel}</span> ·
        {/if}
        ▾ scope to switch · ◆ to widen
      </span>
    </div>
  </div>

  <!-- ── source / scope switcher overlay ──────────────────────────────────── -->
  {#if switcherOpen}
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div
      class="scopesw-overlay"
      onmousedown={() => { switcherOpen = false; composeEl?.focus(); }}
    >
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div
        class="scopesw"
        onmousedown={(e) => e.stopPropagation()}
        onkeydown={switcherKey}
        tabindex="-1"
        use:autofocus
      >
        <div class="scopesw-head">
          <span class="scopesw-title mono">Switch chat scope</span>
          <span class="kbd">esc</span>
        </div>
        <div class="scopesw-list" role="listbox" aria-label="chat scope options">
          {#each switcherOptions as o, i (i)}
            {@const sel = i === switcherSel}
            {@const isCur = i === currentOptionIndex}
            <!-- svelte-ignore a11y_click_events_have_key_events -->
            <div
              class="scopesw-item scopesw-item--{o.kind}{sel ? ' sel' : ''}"
              role="option"
              aria-selected={sel}
              onmouseenter={() => (switcherSel = i)}
              onclick={() => applyOption(o)}
            >
              {#if o.kind === "subject"}
                <Icon name="diamond" size={12} color="var(--accent)" />
                <span class="scopesw-label">Whole subject — {o.label}</span>
              {:else if o.kind === "topic"}
                <Icon name="chevron" size={11} color="var(--fg-faint)" />
                <span class="scopesw-label">{o.label}</span>
                <span class="scopesw-kindtag mono">TOPIC</span>
              {:else}
                <span class="badge badge--{o.src.kind === 'audio' ? 'audio' : o.src.kind}" style="height:15px;padding:0 5px">{kindLabel[o.src.kind] ?? o.src.kind.toUpperCase()}</span>
                <span class="scopesw-label mono">{o.label}</span>
              {/if}
              {#if isCur}
                <Icon name="check" size={13} color="var(--accent)" />
              {/if}
            </div>
          {/each}
        </div>
        <div class="scopesw-foot mono">
          <span><span class="kbd">↑</span><span class="kbd">↓</span> move</span>
          <span><span class="kbd">⏎</span> select</span>
          <span><span class="kbd">esc</span> close</span>
        </div>
      </div>
    </div>
  {/if}
</div>

<style>
  /* ── fit-to-page: the panel always fills its container as a flex column.
     header (fixed) · messages (flex:1, scroll) · composer (fixed). This makes
     the messages region grow to fill the page in the full "Chats" tab while
     the docked variant keeps working (both render .chatdock-inner). ───────── */
  .chatdock-inner {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
    position: relative; /* anchor the scope-switcher overlay */
  }
  .chatdock-inner :global(.chat-head) {
    flex: none;
  }
  .chatdock-inner :global(.chat-scroll) {
    flex: 1 1 auto;
    min-height: 0;
    overflow-y: auto;
  }
  .chatdock-inner :global(.chat-compose) {
    flex: none;
  }

  /* ── scope switcher overlay (themed like CommandPalette / Picker) ───────── */
  .scopesw-overlay {
    position: absolute;
    inset: 0;
    z-index: 70;
    display: flex;
    align-items: flex-start;
    justify-content: center;
    padding-top: 52px;
    background: color-mix(in oklab, var(--bg) 52%, transparent);
    backdrop-filter: blur(2px);
    outline: none;
  }
  .scopesw {
    width: min(340px, calc(100% - 28px));
    max-height: calc(100% - 80px);
    display: flex;
    flex-direction: column;
    background: var(--surface-2);
    border: 1px solid var(--border-strong);
    border-radius: var(--rad-3);
    box-shadow: var(--shadow-pop);
    overflow: hidden;
    outline: none;
    animation: popIn var(--dur-fast) var(--ease);
  }
  .scopesw-head {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 9px 11px;
    border-bottom: 1px solid var(--border);
  }
  .scopesw-title {
    flex: 1;
    font-size: var(--t-xs);
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--fg-faint);
  }
  .scopesw-list {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 6px;
  }
  .scopesw-item {
    display: flex;
    align-items: center;
    gap: 9px;
    width: 100%;
    padding: 7px 9px;
    border-radius: var(--rad-2);
    cursor: pointer;
    color: var(--fg-muted);
    user-select: none;
  }
  .scopesw-item--source {
    padding-left: 22px; /* indent sources under their topic */
  }
  .scopesw-item.sel {
    background: var(--surface-3);
    color: var(--fg-bright);
  }
  .scopesw-label {
    flex: 1;
    font-size: var(--t-sm);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .scopesw-kindtag {
    font-size: var(--t-2xs);
    letter-spacing: 0.08em;
    color: var(--fg-faint);
  }
  .scopesw-foot {
    display: flex;
    gap: 12px;
    align-items: center;
    padding: 8px 11px;
    border-top: 1px solid var(--border);
    font-size: var(--t-2xs);
    color: var(--fg-faint);
  }

  /* Centered "no subject" empty state — mirrors GenerateMaterial's .genmat--working */
  .chat-empty-state {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    text-align: center;
    gap: 10px;
  }
  .chat-empty-state .ces-ico {
    width: 48px;
    height: 48px;
    border-radius: 50%;
    border: 1px solid var(--border-strong);
    background: var(--surface);
    display: grid;
    place-items: center;
    margin-bottom: 2px;
  }
  .chat-empty-state .ces-title {
    font-size: var(--r-md);
    color: var(--fg-bright);
    font-weight: 500;
  }
  .chat-empty-state .ces-sub {
    max-width: 260px;
    font-size: var(--t-sm);
    color: var(--fg-muted);
    line-height: 1.5;
  }
</style>
