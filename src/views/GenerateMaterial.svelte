<script lang="ts">
  import { app } from "../lib/store.svelte";
  import * as api from "../lib/api";
  import Icon from "../components/Icon.svelte";
  import { jobs, type JobKind } from "../lib/jobs.svelte";
  import { t } from "../lib/i18n.svelte";

  // ── Material type definitions ──────────────────────────────
  const GEN_TYPES = [
    { id: "flashcards", label: t("Flashcards"),      ico: "cards", desc: t("Spaced-repetition deck"),    color: "var(--accent)"      },
    { id: "quiz",       label: t("Quiz"),             ico: "check", desc: t("MCQ · short answer · cloze"), color: "var(--info)"        },
    { id: "audio",      label: t("Audio overview"),   ico: "music", desc: t("Two-host podcast"),           color: "var(--mode-select)" },
    { id: "slideshow",  label: t("Slides"),           ico: "grid",  desc: t("Presentation slide deck"),     color: "var(--warn)"        },
    { id: "infographic",label: t("Infographic"),      ico: "grid",  desc: t("One-poster summary"),         color: "var(--ok)"          },
    { id: "mindmap",    label: t("Mind map"),          ico: "link",  desc: t("Concept map of the topic"),    color: "var(--info)"        },
  ] as const;

  const srcLabel: Record<string, string> = {
    pdf: "PDF", pptx: "PPTX", docx: "DOCX", web: "WEB", yt: "YT", audio: "AUD", image: "IMG",
  };

  // Count-based formats and their clamps; other formats have no "how many".
  const COUNT_LIMITS: Record<string, { min: number; max: number; def: number }> = {
    flashcards: { min: 4, max: 40, def: 14 },
    quiz: { min: 3, max: 30, def: 10 },
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
  let customPrompt = $state("");
  // Per-type item count (flashcards / quiz). Seeded from the defaults.
  let cardCount = $state(COUNT_LIMITS.flashcards.def);
  let quizCount = $state(COUNT_LIMITS.quiz.def);

  // ── Derived ───────────────────────────────────────────────────
  const selSources = $derived(allSources.filter(s => sel.includes(s.id)));
  const countLimit = $derived(COUNT_LIMITS[type] ?? null);
  const countValue = $derived(type === "flashcards" ? cardCount : type === "quiz" ? quizCount : null);

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
  const suffixFor = (kind: string) =>
    ({ flashcards: t(" — flashcards"), quiz: t(" — quiz"), audio: t(" — deep dive"), slideshow: t(" — slides"), infographic: t(" — infographic"), mindmap: t(" — mind map") } as Record<string, string>)[kind] ?? "";
  const suggested = $derived.by(() => {
    const suffix = suffixFor(type);
    if (selSources.length === 1) {
      const base = selSources[0].name.replace(/\.[^.]+$/, "").trim();
      if (base) return base + suffix;
    }
    return autoTopic ? autoTopic + suffix : "";
  });
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

  function setCount(n: number) {
    if (!countLimit) return;
    const v = Math.max(countLimit.min, Math.min(countLimit.max, Math.round(n) || countLimit.def));
    if (type === "flashcards") cardCount = v;
    else if (type === "quiz") quizCount = v;
  }

  function generate() {
    const sub = app.activeSubject;
    if (!sub) {
      app.pushToast({ kind: "error", title: t("No active subject"), body: t("Select a subject first.") });
      return;
    }

    const kind = type as JobKind;
    const subjectId = sub.id;
    const topicId = dominantTopicId;
    const matTitle = finalTitle || undefined;
    const sourceIds = [...sel];
    const count = countValue ?? undefined;

    jobs.start({
      kind,
      label: finalTitle || tm.label,
      subjectId,
      topicId,
      run: () =>
        api.generateMaterial(
          subjectId,
          kind as "flashcards" | "quiz" | "audio" | "infographic" | "slideshow" | "mindmap",
          topicId,
          matTitle,
          customPrompt.trim() || undefined,
          sourceIds,
          count,
        ),
    });

    app.setView("subject");
    app.setTab("materials");
  }

  function cancel() {
    app.setView("subject");
    app.setTab("materials");
  }
</script>

<!-- ── Side-by-side launcher (mirrors AddSource: header · two-column · sticky foot,
     no page scroll — only the long source list scrolls inside its panel). ── -->
<div class="genmat2">
  <!-- Header -->
  <div class="gm2-head">
    <button class="btn btn--icon btn--sm btn--ghost" onclick={cancel} title={t("Back")}>
      <span style="display:inline-flex;transform:rotate(180deg)"><Icon name="chevron" size={14} /></span>
    </button>
    <div>
      <div class="eyebrow">{t("Generate material")}</div>
      <h1 class="addpage-title">{t("New study material")}</h1>
      <div class="mono faint" style="font-size: var(--t-xs)">
        {t("from {name} · pick a format and sources", { name: app.activeSubject?.name ?? t("your subject") })}
      </div>
    </div>
  </div>

  <div class="gm2-grid">
    <!-- LEFT — format, count, details -->
    <div class="gm2-left">
      <div class="gm2-block">
        <div class="onb-label mono">{t("FORMAT")}</div>
        <div class="gm2-formats">
          {#each GEN_TYPES as t (t.id)}
            <button class="gm2-format{type === t.id ? ' on' : ''}" onclick={() => (type = t.id)}>
              <span class="gm2-format-ico" style:color={t.color}><Icon name={t.ico} size={16} /></span>
              <span class="gm2-format-txt">
                <span class="gm2-format-label">{t.label}</span>
                <span class="gm2-format-desc mono">{t.desc}</span>
              </span>
              {#if type === t.id}<Icon name="check" size={13} color="var(--accent)" />{/if}
            </button>
          {/each}
        </div>
      </div>

      {#if countLimit && countValue !== null}
        <div class="gm2-block">
          <div class="onb-label mono">{type === "quiz" ? t("QUESTIONS") : t("CARDS")}</div>
          <div class="gm2-count">
            <div class="gm2-step">
              <button class="btn btn--icon btn--sm" onclick={() => setCount(countValue - 1)} aria-label={t("fewer")} disabled={countValue <= countLimit.min}>−</button>
              <span class="mono gm2-step-v">{countValue}</span>
              <button class="btn btn--icon btn--sm" onclick={() => setCount(countValue + 1)} aria-label={t("more")} disabled={countValue >= countLimit.max}>+</button>
            </div>
            <div class="seg gm2-count-presets">
              {#each [Math.round(countLimit.def / 2), countLimit.def, Math.min(countLimit.max, countLimit.def * 2)] as p}
                <button class="seg-opt{countValue === p ? ' on' : ''}" onclick={() => setCount(p)}>{p}</button>
              {/each}
            </div>
          </div>
        </div>
      {/if}

      <div class="gm2-block">
        <div class="field">
          <!-- svelte-ignore a11y_label_has_associated_control -->
          <label class="onb-label mono">{t("TITLE")} <span class="gm2-label-hint">{t("auto-suggested")}</span></label>
          <input class="input" bind:value={title} placeholder={suggested || t("Select sources first…")} />
        </div>
        <div class="field">
          <!-- svelte-ignore a11y_label_has_associated_control -->
          <label class="onb-label mono">{t("CUSTOM INSTRUCTIONS")} <span class="gm2-label-hint">{t("optional")}</span></label>
          <textarea
            class="input set-textarea"
            bind:value={customPrompt}
            rows="3"
            placeholder={t("e.g. “Focus on exam-likely topics”, “Explain like I'm new to {name}”, “Emphasise dates and order”…", { name: app.activeSubject?.name ?? t("this") })}
          ></textarea>
        </div>
        <div class="gm-autotag">
          <div class="gm-autotag-l">
            <Icon name="lock" size={13} color="var(--fg-faint)" />
            <span class="mono">{t("Auto-filed under topic")}</span>
          </div>
          {#if autoTopic}
            <div class="gm-autotag-r">
              <span class="topic-tag mono"><Icon name="chevron" size={9} /> {autoTopic}</span>
              {#if multi}<span class="mono faint">{t("spans {n} topics", { n: topicNames.length })}</span>{/if}
            </div>
          {:else}
            <span class="mono faint">{t("select sources to assign a topic")}</span>
          {/if}
        </div>
      </div>
    </div>

    <!-- RIGHT — source selection (scrolls within the panel) -->
    <div class="gm2-right">
      <div class="gm2-right-head">
        <span class="onb-label mono" style="margin:0">{t("SOURCES")}</span>
        <span class="faint mono">{t("{n} selected", { n: sel.length })}</span>
      </div>
      <div class="gm2-panel">
        {#if subjectTopics.length === 0}
          <p class="mono faint" style="font-size: var(--t-sm); padding: 8px;">{t("No sources found for this subject. Add sources first.")}</p>
        {:else}
          <div class="gm-sources">
            {#each subjectTopics as topic (topic.id)}
              {@const ids    = topic.sources.map(s => s.id)}
              {@const allOn  = ids.length > 0 && ids.every(i => sel.includes(i))}
              {@const someOn = ids.some(i => sel.includes(i))}
              <div class="gm-topic">
                <div class="gm-topic-h">
                  <button class="gm-check{allOn ? ' on' : someOn ? ' some' : ''}" onclick={() => toggleTopic(topic.id)}>
                    {#if allOn}<Icon name="check" size={11} />{:else if someOn}<span class="gm-dash"></span>{/if}
                  </button>
                  <span class="gm-topic-name mono">{topic.name}</span>
                  <span class="faint mono">{topic.sources.length}</span>
                </div>
                <div class="gm-src-list">
                  {#each topic.sources as s (s.id)}
                    {@const on = sel.includes(s.id)}
                    <button class="gm-src{on ? ' on' : ''}" onclick={() => toggle(s.id)}>
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
    </div>
  </div>

  <!-- Sticky footer — always visible, no scrolling to reach Generate -->
  <div class="add-foot gm2-foot">
    <button class="btn btn--ghost" onclick={cancel}>{t("Cancel")}</button>
    <button class="btn btn--primary" disabled={!ready} onclick={generate}>
      <Icon name="bolt" size={13} /> {t("Generate {x}", { x: tm.label.toLowerCase() })}
    </button>
  </div>
</div>

<style>
  .genmat2 {
    height: 100%;
    display: flex;
    flex-direction: column;
    padding: clamp(16px, 3vh, 30px) clamp(20px, 4vw, 52px);
    gap: 16px;
    overflow: hidden;
  }
  .gm2-head { display: flex; align-items: center; gap: 12px; flex: 0 0 auto; }

  .gm2-grid {
    flex: 1 1 auto;
    min-height: 0;
    display: grid;
    grid-template-columns: minmax(300px, 380px) 1fr;
    gap: clamp(18px, 3vw, 38px);
    align-items: stretch;
  }
  .gm2-left { display: flex; flex-direction: column; gap: 16px; min-height: 0; overflow-y: auto; padding-right: 4px; }
  .gm2-block { display: flex; flex-direction: column; gap: 8px; }

  /* format picker — vertical rows */
  .gm2-formats { display: flex; flex-direction: column; gap: 6px; }
  .gm2-format {
    display: flex; align-items: center; gap: 11px;
    padding: 9px 11px; text-align: left; cursor: pointer;
    background: var(--surface); border: 1px solid var(--border); border-radius: var(--rad-3);
    color: var(--fg-muted); transition: border-color var(--dur-fast), background var(--dur-fast);
  }
  .gm2-format:hover { border-color: var(--border-strong); color: var(--fg-bright); }
  .gm2-format.on { border-color: var(--accent-dim); background: color-mix(in oklab, var(--accent) 8%, var(--surface)); color: var(--fg-bright); }
  .gm2-format-ico { display: grid; place-items: center; width: 20px; flex: none; }
  .gm2-format-txt { display: flex; flex-direction: column; gap: 1px; flex: 1; min-width: 0; }
  .gm2-format-label { font-size: var(--t-sm); font-weight: 600; }
  .gm2-format-desc { font-size: var(--t-2xs); color: var(--fg-faint); }

  /* count stepper — mirrors the ExamView / Settings pomodoro stepper, with the
     7/14/28 quick-picks as a .seg segmented control. */
  .gm2-count { display: flex; align-items: center; gap: 10px; flex-wrap: wrap; }
  .gm2-step { display: flex; align-items: center; gap: 8px; }
  .gm2-step-v { min-width: 36px; text-align: center; color: var(--fg-bright); font-variant-numeric: tabular-nums; }
  .gm2-count-presets { margin-left: auto; }

  /* label qualifier — quietly subordinate to the uppercase mono eyebrow label */
  .gm2-label-hint { text-transform: none; letter-spacing: normal; color: var(--fg-faint); font-weight: 400; }

  /* right column / source panel */
  .gm2-right { display: flex; flex-direction: column; min-height: 0; gap: 8px; }
  .gm2-right-head { display: flex; align-items: center; justify-content: space-between; flex: 0 0 auto; }
  .gm2-panel {
    flex: 1 1 auto; min-height: 0; overflow: auto;
    border: 1px solid var(--border); border-radius: var(--rad-3);
    background: var(--surface); padding: 12px;
  }

  .gm2-foot { flex: 0 0 auto; margin: 0; }

  @media (max-width: 860px) {
    .genmat2 { overflow: auto; }
    .gm2-grid { grid-template-columns: 1fr; }
    .gm2-left { overflow: visible; }
    .gm2-panel { max-height: 50vh; }
  }
</style>
