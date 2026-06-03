<script lang="ts">
  import { app } from "../lib/store.svelte";
  import Icon from "./Icon.svelte";
  import RichText from "./RichText.svelte";
  import Picker from "./Picker.svelte";
  import type { Source } from "../lib/api";
  import * as api from "../lib/api";

  let { compact = false, onClose, onFullscreen }: { compact?: boolean; onClose?: () => void; onFullscreen?: () => void } = $props();

  // ── vertical resize ────────────────────────────────────────────────────────
  // A thin grab bar on the panel's top edge drags to change its height. Dragging
  // up (negative dy) grows the panel; clamped between 240px and 90vh. Applied to
  // the root so both the docked instance and the pop-out resize.
  let panelHeight = $state<number | null>(null);
  function startResize(e: PointerEvent) {
    e.preventDefault();
    const startY = e.clientY;
    const startH = panelHeight ?? (e.currentTarget instanceof HTMLElement
      ? e.currentTarget.parentElement?.getBoundingClientRect().height ?? 480
      : 480);
    const maxH = window.innerHeight * 0.9;
    const onMove = (ev: PointerEvent) => {
      const next = startH + (startY - ev.clientY);
      panelHeight = Math.max(240, Math.min(maxH, next));
    };
    const onUp = () => {
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", onUp);
    };
    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", onUp);
  }

  // ── scope state ──────────────────────────────────────────────────────────
  type Level = "subject" | "topic" | "source";

  interface ChatMessage { role: "system" | "user" | "assistant"; text: string }

  let level = $state<Level>("subject");
  let srcId = $state<string | null>(null);
  let topicId = $state<string | null>(null);
  let messages = $state<ChatMessage[]>([]);
  let draft = $state("");
  let streaming = $state<string | null>(null);
  let suggestions = $state<string[]>([]); // next-step prompts under the composer
  let queued = $state<string[]>([]); // messages sent while a response is streaming

  // In-chat model picker (writes the same model_chat setting Settings uses).
  const CHAT_MODELS = [
    "openrouter:openai/gpt-4o-mini",
    "openrouter:openai/gpt-4o",
    "openrouter:anthropic/claude-3.5-sonnet",
    "openrouter:google/gemini-2.0-flash-001",
    "openrouter:deepseek/deepseek-chat",
    "gemini:gemini-2.5-flash",
    "gemini:gemini-2.5-pro",
    "openai:gpt-4o-mini",
    "claude:claude-3-5-sonnet-20241022",
  ];
  let chatModel = $state("");
  let chatModelLoaded = false;
  $effect(() => {
    if (chatModelLoaded) return;
    chatModelLoaded = true;
    api.getSetting("model_chat").then((v) => { if (v) chatModel = v; }).catch(() => {});
  });
  const modelOptions = $derived(
    Array.from(new Set([...(chatModel ? [chatModel] : []), ...CHAT_MODELS])).map((spec) => ({
      id: spec,
      label: spec.includes(":") ? spec.split(":").slice(1).join(":") : spec,
    }))
  );
  function setChatModel(spec: string) {
    chatModel = spec;
    api.setSetting("model_chat", spec).catch(() => {});
  }

  // ── persisted chat history: load the subject's conversation on open ────────
  let loadedSubject: string | null = null;
  $effect(() => {
    const sid = app.activeSubjectId;
    if (sid === loadedSubject) return;
    loadedSubject = sid;
    messages = [];
    suggestions = [];
    queued = [];
    if (!sid) return;
    api.listChatMessages(sid)
      .then((ms) => {
        if (app.activeSubjectId === sid)
          messages = ms.map((m) => ({ role: m.role as ChatMessage["role"], text: m.text }));
      })
      .catch(() => {});
  });
  let scrollEl = $state<HTMLElement | null>(null);
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

  // Effective level: downgrade gracefully when the chosen scope has no target
  // (e.g. "source" scope but the subject has no sources → fall back to topic/subject).
  const effLevel = $derived<Level>(
    level === "source" && !curSrcObj ? (curTopic ? "topic" : "subject")
      : level === "topic" && !curTopic ? "subject"
      : level
  );
  // Plain display name of the current scope (no "Source:" prefix), used in the
  // breadcrumb selector and the empty state.
  const scopeName = $derived(
    effLevel === "source" ? (curSrcObj?.name ?? "")
      : effLevel === "topic" ? (curTopic?.name ?? "")
      : (app.activeSubject?.name ?? "")
  );

  // ── status-bar PWD sync ─────────────────────────────────────────────────────
  // Keep app.chatScope in lock-step with the chat's scope so the status bar
  // reflects subject › topic › source.
  $effect(() => {
    if (!app.activeSubject) {
      app.chatScope = null;
      return;
    }
    if (effLevel === "source" && curSrcObj) {
      app.chatScope = { topicName: curTopic?.name, sourceName: curSrcObj.name };
    } else if (effLevel === "topic") {
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


  // ── source-switcher overlay actions ─────────────────────────────────────────
  function openSwitcher() {
    if (!app.activeSubject) return;
    switcherSel = Math.max(0, currentOptionIndex);
    switcherOpen = true;
  }

  function applyOption(o: ScopeOption) {
    // Silent scope change — no system message in the thread.
    if (o.kind === "subject") {
      level = "subject";
    } else if (o.kind === "topic") {
      topicId = o.topicId;
      level = "topic";
    } else {
      srcId = o.src.id;
      topicId = o.src.topic_id ?? topicId;
      level = "source";
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

  // ── streaming send (with queue, stop, and next-step suggestions) ───────────
  let typeIv: ReturnType<typeof setInterval> | null = null;
  let cancelled = false;

  // Split the trailing "SUGGESTIONS: a | b | c" line off the answer. Harden the
  // parse so placeholder/garbage chips (empty, single chars, a/b/c labels, the
  // literal word "prompt") never render as suggestions.
  function splitSuggestions(full: string): { body: string; sugg: string[] } {
    const m = full.match(/\n?\s*SUGGESTIONS:\s*(.+?)\s*$/i);
    if (!m) return { body: full, sugg: [] };
    const reject = new Set(["prompt", "a", "b", "c"]);
    const sugg = m[1]
      .split("|")
      .map((s) => s.trim())
      .filter((s) => s.length > 1 && !/^[a-c]$/i.test(s) && !reject.has(s.toLowerCase()))
      .slice(0, 3);
    return { body: full.slice(0, m.index).trimEnd(), sugg };
  }

  function send(textArg?: string) {
    const text = (textArg ?? draft).trim();
    if (!text || !app.activeSubject) return;
    // While a response is in-flight, queue the message instead of dropping it.
    if (streaming !== null) {
      queued = [...queued, text];
      if (textArg === undefined) draft = "";
      return;
    }
    if (textArg === undefined) draft = "";
    const sid = app.activeSubject.id;
    messages = [...messages, { role: "user", text }];
    api.addChatMessage(sid, "user", text).catch(() => {}); // persist
    suggestions = [];
    streaming = "";
    cancelled = false;

    const sourceId = effLevel === "source" && curSrcObj ? curSrcObj.id : undefined;
    api
      .chatAnswer(sid, effLevel, text, sourceId)
      .then((result) => {
        if (cancelled) { streaming = null; dequeue(); return; }
        const { body, sugg } = splitSuggestions(result.text);
        let i = 0;
        typeIv = setInterval(() => {
          if (cancelled) {
            if (typeIv) clearInterval(typeIv);
            typeIv = null;
            const partial = body.slice(0, i) || body;
            messages = [...messages, { role: "assistant", text: partial }];
            api.addChatMessage(sid, "assistant", partial).catch(() => {}); // persist
            streaming = null;
            suggestions = sugg;
            dequeue();
            return;
          }
          i += 3;
          streaming = body.slice(0, i);
          if (i >= body.length) {
            if (typeIv) clearInterval(typeIv);
            typeIv = null;
            messages = [...messages, { role: "assistant", text: body }];
            api.addChatMessage(sid, "assistant", body).catch(() => {}); // persist
            streaming = null;
            suggestions = sugg;
            dequeue();
          }
        }, 16);
      })
      .catch((err) => {
        streaming = null;
        const msg = err instanceof Error ? err.message : String(err);
        messages = [...messages, { role: "system", text: msg }];
        dequeue();
      });
  }

  // ── chat sessions / history ────────────────────────────────────────────
  let historyOpen = $state(false);
  let threads = $state<api.ThreadInfo[]>([]);

  async function startNewConversation() {
    const sid = app.activeSubject?.id;
    if (!sid) return;
    await api.newChat(sid).catch(() => {});
    messages = [];
    suggestions = [];
    queued = [];
    composeEl?.focus();
  }

  async function openHistory() {
    const sid = app.activeSubject?.id;
    if (!sid) return;
    threads = await api.listChatThreads(sid).catch(() => [] as api.ThreadInfo[]);
    historyOpen = true;
  }

  async function pickThread(id: string) {
    const sid = app.activeSubject?.id;
    if (!sid) return;
    await api.openChatThread(sid, id).catch(() => {});
    const ms = await api.listChatMessages(sid).catch(() => []);
    messages = ms.map((m) => ({ role: m.role as ChatMessage["role"], text: m.text }));
    suggestions = [];
    historyOpen = false;
  }

  // Send the next queued message (if any) once the current one finishes.
  function dequeue() {
    if (queued.length === 0) return;
    const [next, ...rest] = queued;
    queued = rest;
    send(next);
  }

  // Stop the current generation: abort the typewriter (and discard a pending
  // result), keeping whatever has streamed so far.
  function stop() {
    cancelled = true;
    if (typeIv) { clearInterval(typeIv); typeIv = null; }
    if (streaming) messages = [...messages, { role: "assistant", text: streaming }];
    streaming = null;
  }

  // ── save an assistant answer as a note ─────────────────────────────────────
  // Title = first ~6 words of the message; body = the full markdown.
  async function saveToNote(text: string) {
    const title =
      text.replace(/\s+/g, " ").trim().split(" ").slice(0, 6).join(" ") || "Note";
    try {
      await api.createNote(title, text, app.activeSubjectId ?? null);
      app.pushToast({ kind: "success", title: "Saved to notes" });
    } catch (e) {
      app.pushToast({ kind: "error", title: "Save failed", body: String(e) });
    }
  }

  function handleKey(e: KeyboardEvent) {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      send();
      return;
    }
    // When the composer is EMPTY, Escape or "c" closes the chat — so the same
    // key that opens it also closes it, even though the dock pages land focus
    // here. With text typed, "c" types normally and Esc just blurs.
    if (!draft.trim() && !e.metaKey && !e.ctrlKey && !e.altKey && (e.key === "c" || e.key === "Escape")) {
      e.preventDefault();
      (e.target as HTMLElement | null)?.blur();
      app.setMode("NOR");
      if (onClose) onClose();
      else app.chatOpen = false;
    }
  }

  // Root-level keybind for the whole panel: while the chat is focused and not
  // typing in the composer, "s" opens the source switcher.
  function panelKey(e: KeyboardEvent) {
    if (switcherOpen) return; // overlay owns the keys while open
    const typing =
      app.mode === "INS" ||
      (e.target instanceof HTMLElement &&
        (e.target.tagName === "TEXTAREA" || e.target.tagName === "INPUT"));
    if (e.key === "s" && !typing && !e.metaKey && !e.ctrlKey && !e.altKey) {
      e.preventDefault();
      openSwitcher();
    }
  }

  // ── auto-scroll (stick to bottom only when the user is already there, so
  // they can freely scroll up to read history mid-stream) ────────────────────
  let stick = $state(true);
  function onScroll() {
    if (!scrollEl) return;
    stick = scrollEl.scrollHeight - scrollEl.scrollTop - scrollEl.clientHeight < 120;
  }
  $effect(() => {
    const _ = messages.length;
    const __ = streaming;
    if (scrollEl && stick) scrollEl.scrollTop = scrollEl.scrollHeight;
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
<!-- Window-level so "s" works without the panel div holding focus (a plain div
     never receives keydown unless focused). panelKey ignores it while typing. -->
<svelte:window onkeydown={panelKey} />

<div class="chatdock-inner" style:height={panelHeight ? panelHeight + "px" : null}>
  <!-- ── top drag handle: vertical resize of the whole panel ──────────────── -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="chat-resize"
    role="separator"
    aria-orientation="horizontal"
    aria-label="Resize chat panel"
    title="Drag to resize"
    onpointerdown={startResize}
  ></div>

  <!-- ── header ─────────────────────────────────────────────────────────── -->
  <div class="chat-head">
    {#if app.activeSubject}
      <!-- One clean clickable scope selector (opens the switcher). Shows the
           current scope path; no redundant chevrons or banner. -->
      <button class="scope-pick" type="button" title="Change scope (s)" onclick={openSwitcher}>
        <span class="sp-ico">{app.activeSubject.glyph || "◆"}</span>
        <span class="sp-name">{app.activeSubject.name}</span>
        {#if effLevel !== "subject" && curTopic}
          <span class="sp-sep">›</span>
          <span class="sp-name sp-dim">{curTopic.name}</span>
        {/if}
        {#if effLevel === "source" && curSrcObj}
          <span class="sp-sep">›</span>
          <span class="sp-name">{shortName(curSrcObj.name)}</span>
        {/if}
        <Icon name="chevron" size={11} style="transform:rotate(90deg);opacity:.55;margin-left:2px" />
      </button>
    {:else}
      <span class="faint" style="font-size:12px">No subject open</span>
    {/if}

    <div class="grow"></div>
    <div class="chat-model-pick" title="Chat model">
      <Picker value={chatModel} onChange={setChatModel} options={modelOptions} icon="bolt" placeholder="Model" />
    </div>
    <button class="btn btn--icon btn--sm btn--ghost" title="Chat history" onclick={openHistory}>
      <Icon name="book" size={13} />
    </button>
    <button class="btn btn--icon btn--sm btn--ghost" title="New conversation" onclick={startNewConversation}>
      <Icon name="plus" size={13} />
    </button>
    {#if onFullscreen}
      <button class="btn btn--icon btn--sm btn--ghost" onclick={onFullscreen} title="Fullscreen chat">
        <Icon name="external" size={12} />
      </button>
    {/if}
    {#if onClose}
      <button class="btn btn--icon btn--sm btn--ghost" onclick={onClose} title="Close chat">
        <Icon name="x" size={12} />
      </button>
    {/if}
  </div>

  <!-- ── message list ───────────────────────────────────────────────────── -->
  <div class="chat-scroll" bind:this={scrollEl} onscroll={onScroll}>
    {#if !app.activeSubject}
      <div class="chat-empty-state">
        <div class="ces-ico">
          <Icon name="diamond" size={22} color="var(--fg3)" />
        </div>
        <div class="ces-title">Open a subject to start chatting</div>
        <div class="ces-sub">Ask questions grounded in your sources.</div>
      </div>
    {:else}
      {#if messages.length === 0 && streaming === null}
        <div class="chat-empty-state" style="min-height:48vh;display:flex;flex-direction:column;align-items:center;justify-content:center;gap:8px;text-align:center;">
          <div class="ces-ico"><Icon name="chat" size={22} color="var(--fg3)" /></div>
          <div class="ces-title">Ask anything about {scopeName}</div>
          <div class="ces-sub">Press <span class="kbd">i</span> to start · <span class="kbd">s</span> to change scope</div>
        </div>
      {/if}

      {#each messages as m, i (i)}
        {#if m.role === "system"}
          <div class="bubble system">— {m.text} —</div>
        {:else if m.role === "user"}
          <div class="bubble user">{m.text}</div>
        {:else}
          <div class="bubble assistant">
            <RichText text={m.text} />
            <div class="bubble-actions">
              <button
                type="button"
                class="msg-action"
                title="Save to notes"
                aria-label="Save to notes"
                onclick={() => saveToNote(m.text)}
              >
                <Icon name="plus" size={12} />
              </button>
            </div>
          </div>
        {/if}
      {/each}

      {#if streaming !== null}
        <div class="bubble assistant">
          <RichText text={streaming} /><span class="cursor-blink">▋</span>
        </div>
      {/if}
    {/if}
  </div>

  <!-- ── compose ───────────────────────────────────────────────────────── -->
  <div class="chat-compose">
    {#if suggestions.length && streaming === null}
      <div class="chat-suggest">
        <span class="cs-label mono">Next</span>
        {#each suggestions as s}
          <button type="button" class="suggest-chip" title={s} onclick={() => send(s)}>{s}</button>
        {/each}
      </div>
    {/if}
    {#if queued.length}
      <div class="chat-queued mono faint">{queued.length} message{queued.length === 1 ? "" : "s"} queued…</div>
    {/if}
    <div class="compose-box{app.mode === 'INS' ? ' is-insert' : ''}">
      <textarea
        bind:this={composeEl}
        rows={1}
        placeholder={!app.activeSubject
          ? "Open a subject first…"
          : app.mode === "INS"
          ? "Ask about " + scopeName + "…"
          : "Press i to ask…"}
        bind:value={draft}
        disabled={!app.activeSubject}
        onfocus={() => app.setMode("INS")}
        onblur={() => app.setMode("NOR")}
        onkeydown={handleKey}
      ></textarea>
      {#if streaming !== null}
        <button class="btn btn--icon btn--sm chat-stop" onclick={stop} title="Stop generating">
          <span class="stop-sq"></span>
        </button>
      {:else}
        <button
          class="btn btn--icon btn--sm btn--primary"
          onclick={() => send()}
          disabled={!draft.trim() || !app.activeSubject}
          title="Send"
        >
          <Icon name="arrowR" size={13} />
        </button>
      {/if}
    </div>
    <div class="compose-hint">
      <span><span class="kbd">i</span> insert</span>
      <span><span class="kbd">⏎</span> send</span>
      <span><span class="kbd">⎋</span> normal</span>
      <span><span class="kbd">s</span> scope</span>
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
              tabindex="-1"
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

  <!-- ── chat history (past conversation sessions) ─────────────────────────── -->
  {#if historyOpen}
    <div class="hist-overlay" role="presentation" onmousedown={() => (historyOpen = false)}>
      <div class="hist-panel" role="dialog" aria-modal="true" tabindex="-1" onmousedown={(e) => e.stopPropagation()}>
        <div class="hist-head">
          <span class="hist-title">Chat history</span>
          <div class="grow"></div>
          <button class="btn btn--sm btn--ghost" onclick={startNewConversation} title="New conversation">
            <Icon name="plus" size={12} /> New
          </button>
          <button class="btn btn--icon btn--sm btn--ghost" onclick={() => (historyOpen = false)} title="Close">
            <Icon name="x" size={12} />
          </button>
        </div>
        {#if threads.length === 0}
          <div class="hist-empty mono faint">No past conversations yet.</div>
        {:else}
          <div class="hist-list">
            {#each threads as th}
              <button type="button" class="hist-item" onclick={() => pickThread(th.id)}>
                <Icon name="chat" size={13} color="var(--fg-faint)" />
                <span class="hist-item-title">{th.title || "New conversation"}</span>
                <span class="hist-item-meta mono faint">{th.count} msg</span>
              </button>
            {/each}
          </div>
        {/if}
      </div>
    </div>
  {/if}
</div>

<style>
  /* ── chat history panel ────────────────────────────────────────────────── */
  .hist-overlay {
    position: absolute; inset: 0; z-index: 80; display: flex;
    align-items: flex-start; justify-content: center; padding-top: 56px;
    background: color-mix(in oklab, var(--bg) 55%, transparent); backdrop-filter: blur(2px);
  }
  .hist-panel {
    width: min(440px, calc(100% - 32px)); max-height: 70%;
    display: flex; flex-direction: column;
    background: var(--surface); border: 1px solid var(--border-strong);
    border-radius: 12px; box-shadow: 0 18px 50px rgba(0,0,0,0.45); padding: 12px;
  }
  .hist-head { display: flex; align-items: center; gap: 8px; margin-bottom: 8px; }
  .hist-title { font-family: var(--font-mono); font-weight: 600; color: var(--fg-bright); font-size: 13px; }
  .hist-empty { padding: 24px; text-align: center; font-size: 12px; }
  .hist-list { overflow-y: auto; display: flex; flex-direction: column; gap: 2px; }
  .hist-item {
    display: flex; align-items: center; gap: 9px; width: 100%; text-align: left;
    padding: 9px 10px; border-radius: 8px; border: 1px solid transparent;
    background: none; color: var(--fg); font: inherit; cursor: pointer;
  }
  .hist-item:hover { background: var(--surface-2); border-color: var(--border); }
  .hist-item-title { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; color: var(--fg-bright); font-size: 12.5px; }
  .hist-item-meta { flex: none; font-size: 10.5px; }

  /* ── next-step suggestion chips + queue + stop + model picker ──────────── */
  .chat-suggest {
    display: flex; flex-wrap: wrap; align-items: center; gap: 6px; margin-bottom: 8px;
  }
  .chat-suggest .cs-label {
    font-size: var(--t-2xs, 10.5px); letter-spacing: 0.12em; text-transform: uppercase;
    color: var(--fg-faint); margin-right: 2px;
  }
  .suggest-chip {
    font: inherit; font-size: 12px; cursor: pointer;
    padding: 5px 10px; border-radius: 999px;
    border: 1px solid var(--border-strong); background: var(--surface-2); color: var(--fg);
    transition: background 0.12s ease, border-color 0.12s ease, color 0.12s ease;
    max-width: 220px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis;
  }
  .suggest-chip:hover { background: var(--surface-3); border-color: var(--accent-dim, var(--accent)); color: var(--fg-bright); }
  .chat-queued { font-size: 11px; margin-bottom: 6px; }

  /* per-message save-to-note action (appears on hover of an assistant bubble) */
  .bubble-actions {
    display: flex;
    justify-content: flex-end;
    gap: 4px;
    margin-top: 6px;
    opacity: 0;
    transition: opacity 0.12s ease;
  }
  .bubble.assistant:hover .bubble-actions {
    opacity: 1;
  }
  .msg-action {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 22px;
    height: 22px;
    border-radius: var(--r-lg, 8px);
    border: 1px solid var(--border);
    background: var(--surface-2);
    color: var(--fg-muted);
    cursor: pointer;
    transition: background 0.12s ease, color 0.12s ease, border-color 0.12s ease;
  }
  .msg-action:hover {
    background: var(--surface-3);
    color: var(--fg-bright);
    border-color: var(--border-strong);
  }
  .chat-stop .stop-sq { width: 10px; height: 10px; border-radius: 2px; background: var(--err); display: block; }
  .chat-stop { border-color: var(--border-strong); }
  .chat-model-pick { max-width: 200px; font-size: 11px; }

  /* ── scope selector (the reworked top bar: one clean clickable control) ─── */
  .scope-pick {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    max-width: 100%;
    min-width: 0;
    padding: 5px 10px;
    border: 1px solid var(--border-strong);
    border-radius: var(--r-md, 8px);
    background: var(--surface-2);
    color: var(--fg);
    font: inherit;
    font-size: var(--t-sm, 12.5px);
    cursor: pointer;
    transition: background 0.12s ease, border-color 0.12s ease;
  }
  .scope-pick:hover {
    background: var(--surface-3);
    border-color: var(--accent-dim, var(--accent));
  }
  .scope-pick .sp-ico { flex: none; font-size: 13px; line-height: 1; }
  .scope-pick .sp-name {
    color: var(--fg-bright);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    min-width: 0;
  }
  .scope-pick .sp-name.sp-dim { color: var(--fg-muted); }
  .scope-pick .sp-sep { flex: none; color: var(--fg-faint); }

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
  /* top drag handle for vertical resize */
  .chat-resize {
    flex: none;
    height: 7px;
    cursor: ns-resize;
    display: flex;
    align-items: center;
    justify-content: center;
    touch-action: none;
  }
  .chat-resize::before {
    content: "";
    width: 34px;
    height: 3px;
    border-radius: 999px;
    background: var(--border-strong);
    transition: background 0.12s ease;
  }
  .chat-resize:hover::before {
    background: var(--accent);
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
