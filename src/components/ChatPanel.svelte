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
  let messages = $state<ChatMessage[]>([]);
  let draft = $state("");
  let streaming = $state<string | null>(null);
  let menuOpen = $state(false);
  let scrollEl = $state<HTMLElement | null>(null);
  let modelLabel = $state<string | null>(null);

  // All sources for the active subject, flattened across topics
  const topicSources = $derived<Source[]>(
    app.activeSubject?.topics?.flatMap((t) => t.sources) ?? []
  );

  // Resolve the active source object
  const curSrcObj = $derived<Source | null>(
    topicSources.find((s) => s.id === srcId) ?? topicSources[0] ?? null
  );

  // Short display name for the source segment
  function shortName(name: string) {
    return name.replace(/\.[^.]+$/, "").replace(/^lecture-0?/, "lec-");
  }

  // Scope label derived from real data
  const activeScopeLabel = $derived<Record<Level, string>>({
    subject: "Subject: " + (app.activeSubject?.name ?? "—"),
    topic: "Topic: " + (app.activeSubject?.topics?.[0]?.name ?? "—"),
    source: curSrcObj ? "Source: " + curSrcObj.name : "Source",
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

  function pickSource(s: Source) {
    srcId = s.id;
    level = "source";
    menuOpen = false;
    messages = [
      ...messages,
      { role: "system", text: "scope set to Source: " + s.name },
    ];
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
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      send();
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

<div class="chatdock-inner">
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

        {#if app.activeSubject.topics?.[0]}
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
            <span class="seg-ico"><Icon name="chevron" size={11} /></span>{app.activeSubject.topics[0].name}
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
              title="Switch source — {curSrcObj.name}"
              onclick={() => { changeScope("source"); menuOpen = !menuOpen; }}
              onkeydown={(e) => { if (e.key === "Enter") { changeScope("source"); menuOpen = !menuOpen; } }}
            >
              <span class="seg-ico"><Icon name="doc" size={11} /></span>
              <span class="src-seg-label">{shortName(curSrcObj.name)}</span>
              <Icon name="chevron" size={10} />
            </span>

            {#if menuOpen}
              <!-- svelte-ignore a11y_click_events_have_key_events -->
              <!-- svelte-ignore a11y_no_static_element_interactions -->
              <div class="src-menu-backdrop" onclick={() => (menuOpen = false)}></div>
              <div class="src-menu">
                <div class="src-menu-h mono">Switch source in this chat</div>
                {#each topicSources as s (s.id)}
                  {@const label = kindLabel[s.kind] ?? s.kind.toUpperCase()}
                  <button
                    class="src-menu-item{s.id === curSrcObj?.id ? ' on' : ''}"
                    onclick={() => pickSource(s)}
                  >
                    <span class="badge badge--{s.kind === 'audio' ? 'audio' : s.kind}" style="height:15px;padding:0 5px">{label}</span>
                    <span class="smi-name mono">{s.name}</span>
                    {#if s.id === curSrcObj?.id}
                      <Icon name="check" size={13} color="var(--accent)" />
                    {/if}
                  </button>
                {/each}
              </div>
            {/if}
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
      <span style="margin-left:auto" class="faint">
        {#if modelLabel}
          <span class="model-tag">{modelLabel}</span> ·
        {/if}
        ▾ source to switch · ◆ to widen
      </span>
    </div>
  </div>
</div>

<style>
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
