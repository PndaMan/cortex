<script lang="ts">
  import { app } from "../lib/store.svelte";
  import * as api from "../lib/api";
  import Icon from "../components/Icon.svelte";

  // ── Material type definitions ──────────────────────────────
  const GEN_TYPES = [
    { id: "flashcards", label: "Flashcards",      ico: "cards", desc: "Spaced-repetition deck",    color: "var(--accent)"      },
    { id: "quiz",       label: "Quiz",             ico: "check", desc: "MCQ · short answer · cloze", color: "var(--info)"        },
    { id: "audio",      label: "Audio overview",   ico: "music", desc: "Two-host podcast",           color: "var(--mode-select)" },
    { id: "slideshow",  label: "Slideshow video",  ico: "play",  desc: "Narrated slides",            color: "var(--warn)"        },
    { id: "infographic",label: "Infographic",      ico: "grid",  desc: "One-poster summary",         color: "var(--ok)"          },
  ] as const;

  const srcLabel: Record<string, string> = {
    pdf: "PDF", pptx: "PPTX", docx: "DOCX", web: "WEB", yt: "YT", audio: "AUD", image: "IMG",
  };

  // ── Derived from real active subject ─────────────────────────
  const subjectTopics = $derived(app.activeSubject?.topics ?? []);
  const allSources = $derived(
    subjectTopics.flatMap(t =>
      t.sources.map(s => ({ ...s, topicId: t.id, topicName: t.name }))
    )
  );

  // ── State ─────────────────────────────────────────────────────
  let type  = $state<string>("flashcards");
  let sel   = $state<string[]>([]);
  let title = $state("");
  let gen   = $state<null | "working" | "done">(null);

  // ── Derived ───────────────────────────────────────────────────
  const selSources = $derived(allSources.filter(s => sel.includes(s.id)));

  const counts = $derived.by(() => {
    const c: Record<string, number> = {};
    for (const s of selSources) c[s.topicName] = (c[s.topicName] ?? 0) + 1;
    return c;
  });
  const topicNames = $derived(Object.keys(counts));
  const autoTopic  = $derived(
    topicNames.length === 0
      ? null
      : [...topicNames].sort((a, b) => counts[b] - counts[a])[0]
  );
  const dominantTopicId = $derived.by(() => {
    if (!autoTopic) return app.activeSubject?.topics[0]?.id;
    const t = subjectTopics.find(t => t.name === autoTopic);
    return t?.id ?? app.activeSubject?.topics[0]?.id;
  });
  const multi = $derived(topicNames.length > 1);

  const tm = $derived(GEN_TYPES.find(t => t.id === type)!);
  const suggested = $derived(
    autoTopic
      ? autoTopic + ({ flashcards: " — flashcards", quiz: " — quiz", audio: " — deep dive", slideshow: " visualized", infographic: " — infographic" } as Record<string, string>)[type]
      : ""
  );
  const finalTitle = $derived(title.trim() || suggested);
  const ready = $derived(sel.length > 0 && !!app.activeSubject);

  // ── Actions ───────────────────────────────────────────────────
  function toggle(id: string) {
    sel = sel.includes(id) ? sel.filter(y => y !== id) : [...sel, id];
  }

  function toggleTopic(topicId: string) {
    const topic = subjectTopics.find(t => t.id === topicId);
    if (!topic) return;
    const ids = topic.sources.map(s => s.id);
    const allOn = ids.every(i => sel.includes(i));
    sel = allOn ? sel.filter(i => !ids.includes(i)) : Array.from(new Set([...sel, ...ids]));
  }

  async function generate() {
    const sub = app.activeSubject;
    if (!sub) {
      app.pushToast({ kind: "error", title: "No active subject", body: "Select a subject first." });
      return;
    }

    gen = "working";

    try {
      const result = await api.generateMaterial(
        sub.id,
        type as "flashcards" | "quiz" | "audio" | "infographic" | "slideshow",
        dominantTopicId,
        finalTitle || undefined,
      );
      gen = "done";
      setTimeout(() => {
        app.pushToast({
          kind: "success",
          title: "Material ready",
          body: `${result.title} filed under ${result.topic}.`,
        });
        app.setView("subject");
        app.setTab("materials");
      }, 800);
    } catch (e: unknown) {
      gen = null;
      const msg = e instanceof Error ? e.message : String(e);
      app.pushToast({ kind: "error", title: "Generation failed", body: msg });
    }
  }

  function cancel() {
    app.setView("subject");
    app.setTab("materials");
  }
</script>

{#if gen}
  <!-- ── Generating / done state ───────────────────────────── -->
  <div class="workspace-scroll">
    <div class="genmat genmat--working">
      <div class="gm-working">
        <div class="gm-spin-wrap">
          {#if gen === "done"}
            <Icon name="check" size={26} color="var(--ok)" />
          {:else}
            <span class="gm-spin"></span>
          {/if}
        </div>
        <h1 class="read">
          {gen === "done" ? "Material ready" : `Generating ${tm.label.toLowerCase()}…`}
        </h1>
        <p class="mono muted">
          {gen === "done"
            ? `Filed under ${autoTopic ?? "your subject"}`
            : `Synthesizing from ${sel.length} source${sel.length > 1 ? "s" : ""} · ${autoTopic ?? ""}`}
        </p>
        <div class="gm-prog">
          <div class="gm-prog-bar" style:width={gen === "done" ? "100%" : "70%"}></div>
        </div>
      </div>
    </div>
  </div>

{:else}
  <!-- ── Form ─────────────────────────────────────────────── -->
  <div class="workspace-scroll">
    <div class="genmat">
      <!-- Header -->
      <div class="addpage-head">
        <button class="btn btn--icon btn--sm btn--ghost" onclick={cancel} title="Back">
          <span style="display:inline-flex;transform:rotate(180deg)"><Icon name="chevron" size={14} /></span>
        </button>
        <div>
          <div class="eyebrow">Generate material</div>
          <h1 class="addpage-title read">New study material</h1>
          <div class="mono faint" style="font-size: var(--t-xs)">
            from {app.activeSubject?.name ?? "your subject"} · pick a format and sources
          </div>
        </div>
      </div>

      <!-- Step 1: format -->
      <div class="gm-section">
        <div class="gm-step mono">
          <span class="gm-step-n">1</span> Choose a format
        </div>
        <div class="gm-types">
          {#each GEN_TYPES as t (t.id)}
            <button
              class="gm-type{type === t.id ? ' on' : ''}"
              onclick={() => (type = t.id)}
            >
              <span class="gm-type-ico" style:color={t.color}>
                <Icon name={t.ico} size={18} />
              </span>
              <span class="gm-type-label">{t.label}</span>
              <span class="gm-type-desc mono">{t.desc}</span>
            </button>
          {/each}
        </div>
      </div>

      <!-- Step 2: sources -->
      <div class="gm-section">
        <div class="gm-step mono">
          <span class="gm-step-n">2</span> Select sources
          <span class="faint">· {sel.length} selected</span>
        </div>
        {#if subjectTopics.length === 0}
          <p class="mono faint" style="font-size: var(--t-sm)">No sources found for this subject. Add sources first.</p>
        {:else}
          <div class="gm-sources">
            {#each subjectTopics as topic (topic.id)}
              {@const ids    = topic.sources.map(s => s.id)}
              {@const allOn  = ids.length > 0 && ids.every(i => sel.includes(i))}
              {@const someOn = ids.some(i => sel.includes(i))}
              <div class="gm-topic">
                <div class="gm-topic-h">
                  <button
                    class="gm-check{allOn ? ' on' : someOn ? ' some' : ''}"
                    onclick={() => toggleTopic(topic.id)}
                  >
                    {#if allOn}
                      <Icon name="check" size={11} />
                    {:else if someOn}
                      <span class="gm-dash"></span>
                    {/if}
                  </button>
                  <span class="gm-topic-name mono">{topic.name}</span>
                  <span class="faint mono">{topic.sources.length}</span>
                </div>
                <div class="gm-src-list">
                  {#each topic.sources as s (s.id)}
                    {@const on = sel.includes(s.id)}
                    <button
                      class="gm-src{on ? ' on' : ''}"
                      onclick={() => toggle(s.id)}
                    >
                      <span class="gm-check{on ? ' on' : ''}">
                        {#if on}<Icon name="check" size={11} />{/if}
                      </span>
                      <span class="badge badge--{s.kind === 'audio' ? 'audio' : s.kind}">
                        <span class="dot"></span>{srcLabel[s.kind] ?? s.kind.toUpperCase()}
                      </span>
                      <span class="gm-src-name mono">{s.name}</span>
                      <span class="gm-src-meta mono faint">{s.meta ?? ""}</span>
                    </button>
                  {/each}
                </div>
              </div>
            {/each}
          </div>
        {/if}
      </div>

      <!-- Step 3: details -->
      <div class="gm-section">
        <div class="gm-step mono"><span class="gm-step-n">3</span> Details</div>
        <div class="field">
          <!-- svelte-ignore a11y_label_has_associated_control -->
          <label class="onb-label mono">TITLE <span class="faint">auto-suggested</span></label>
          <input
            class="input"
            bind:value={title}
            placeholder={suggested || "Select sources first…"}
          />
        </div>
        <div class="gm-autotag">
          <div class="gm-autotag-l">
            <Icon name="lock" size={13} color="var(--fg-faint)" />
            <span class="mono">Auto-filed under topic</span>
          </div>
          {#if autoTopic}
            <div class="gm-autotag-r">
              <span class="topic-tag mono"><Icon name="chevron" size={9} /> {autoTopic}</span>
              {#if multi}
                <span class="mono faint">spans {topicNames.length} topics · filed under the dominant one</span>
              {/if}
            </div>
          {:else}
            <span class="mono faint">select sources to assign a topic</span>
          {/if}
        </div>
      </div>

      <!-- Footer -->
      <div class="add-foot">
        <button class="btn btn--ghost" onclick={cancel}>Cancel</button>
        <button
          class="btn btn--primary"
          disabled={!ready}
          onclick={generate}
        >
          <Icon name="bolt" size={13} /> Generate {tm.label.toLowerCase()}
        </button>
      </div>
    </div>
  </div>
{/if}
