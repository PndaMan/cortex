<script lang="ts">
  import { app } from "../lib/store.svelte";
  import * as api from "../lib/api";
  import type { CsSection as ApiCsSection } from "../lib/api";
  import Icon from "../components/Icon.svelte";

  // ── reactive data ────────────────────────────────────────────
  let sections = $state<ApiCsSection[]>([]);
  let cheatTopic = $state<string>("");
  let cheatSub = $state<string>("");
  let sourceCount = $state<number>(0);
  let hasCheatsheet = $state(false);  // true = real cheatsheet loaded
  let regenerating = $state(false);

  // Track sections that were just approved (for flash animation).
  let recentlyApproved = $state<Record<string, boolean>>({});

  // ── load cheatsheet whenever activeSubject changes ────────────
  $effect(() => {
    const sub = app.activeSubject;
    if (!sub) {
      sections = [];
      cheatTopic = "";
      cheatSub = "";
      sourceCount = 0;
      hasCheatsheet = false;
      return;
    }

    const topicId = sub.topics[0]?.id;

    api.getCheatsheet(sub.id, topicId).then((data) => {
      if (data) {
        sections = data.sections;
        cheatTopic = data.topic;
        cheatSub = data.subject;
        sourceCount = data.sources;
        hasCheatsheet = true;
        app.pending = data.sections.filter((s) => s.state === "draft-pending").length;
      } else {
        sections = [];
        cheatTopic = sub.topics[0]?.name ?? "";
        cheatSub = sub.name;
        sourceCount = sub.sourceCount;
        hasCheatsheet = false;
        app.pending = 0;
      }
    }).catch(() => {
      sections = [];
      cheatTopic = sub.topics[0]?.name ?? "";
      cheatSub = sub.name;
      sourceCount = sub.sourceCount;
      hasCheatsheet = false;
      app.pending = 0;
    });
  });

  const sectionCount = $derived(sections.length);

  // ── generate / regenerate action ─────────────────────────────
  async function generate() {
    const sub = app.activeSubject;
    if (!sub || regenerating) return;
    regenerating = true;
    try {
      const topicId = sub.topics[0]?.id;
      const result = await api.generateCheatsheet(sub.id, topicId);
      sections = result.sections;
      cheatTopic = result.topic;
      cheatSub = result.subject;
      sourceCount = result.sources;
      hasCheatsheet = true;
      app.pending = result.sections.filter((s) => s.state === "draft-pending").length;
      app.pushToast({
        kind: "success",
        title: "Cheatsheet synthesized",
        body: "from " + result.sources + " sources · " + result.model,
      });
    } catch (e) {
      app.pushToast({ kind: "error", title: "Couldn't generate", body: String(e) });
    } finally {
      regenerating = false;
    }
  }
</script>

<div class="workspace-scroll">
  <div class="cs-doc">
    {#if !app.activeSubject}
      <!-- No subject open -->
      <div class="cs-empty-state">
        <Icon name="diamond" size={28} color="var(--fg3)" />
        <div class="ces-title">No subject open</div>
        <div class="ces-sub">Open a subject from the sidebar to view its cheatsheet.</div>
      </div>
    {:else if !hasCheatsheet}
      <!-- Subject open but no cheatsheet generated yet -->
      {@const noSources = app.activeSubject.sourceCount === 0}
      <div class="cs-working">
        <div class="cs-working-ico">
          <Icon name="diamond" size={26} color="var(--fg3)" />
        </div>
        <h1 class="cs-working-title read">No cheatsheet yet</h1>
        <p class="cs-working-sub mono muted">
          {#if noSources}
            Add sources to this subject first — your cheatsheet is synthesized from them.
          {:else}
            A completeness-checked cheatsheet will be generated from this subject's {app.activeSubject.sourceCount} source{app.activeSubject.sourceCount !== 1 ? "s" : ""}.
          {/if}
        </p>
        <button class="btn btn--primary btn--sm" onclick={generate} disabled={regenerating}>
          <Icon name="refresh" size={13} /> {regenerating ? "Synthesizing…" : "Generate cheatsheet"}
        </button>
      </div>
    {:else}
      <!-- Document header -->
      <div class="cs-doc-head">
        <div>
          <div class="eyebrow">Cheatsheet</div>
          <h1 class="cs-title">{cheatTopic}</h1>
          <div class="cs-sub mono">
            {app.activeSubject.name}{app.activeSubject.code ? " · " + app.activeSubject.code : ""} ·
            synthesized from {sourceCount} source{sourceCount !== 1 ? "s" : ""} · {sectionCount} enforced sections
          </div>
        </div>
        <div class="cs-doc-actions">
          <button class="btn btn--sm" onclick={generate} disabled={regenerating}>
            <Icon name="refresh" size={13} /> {regenerating ? "Synthesizing…" : "Regenerate"}
          </button>
        </div>
      </div>

      <!-- Sections -->
      <div class="cs-sections">
        {#each sections as sec (sec.id)}
          {@const isRecent = !!recentlyApproved[sec.id]}
          <section
            class="cs-section{isRecent ? ' just-approved' : ''}{sec.state === 'draft-pending' ? ' is-pending' : ''}"
          >
            <header class="cs-sec-head">
              <h2 class="cs-sec-title">{sec.title}</h2>

              {#if sec.state === "draft-pending"}
                <span class="status-pill status-pill--draft">
                  <span class="dot"></span>pending
                </span>
              {/if}

              {#if isRecent}
                <span class="status-pill status-pill--ready">
                  <span class="dot"></span>just approved
                </span>
              {/if}

              {#if (sec.state === "approved" || sec.state === "idle") && !isRecent}
                <span class="cs-sec-count mono">{sec.items.length}</span>
              {/if}
            </header>

            <dl class="cs-list">
              {#each sec.items as item, i (i)}
                <div class="cs-item{(item as any).flag === 'changed' ? ' item-changed' : ''}">
                  <dt>{item.t}</dt>
                  <dd>{item.d}</dd>
                </div>
              {/each}
            </dl>
          </section>
        {/each}
      </div>
    {/if}
  </div>
</div>

<style>
  /* Centered empty / working states — mirrors GenerateMaterial's .genmat--working */
  .cs-empty-state,
  .cs-working {
    min-height: 60vh;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    text-align: center;
    gap: 12px;
  }
  .cs-working-ico {
    width: 54px;
    height: 54px;
    border-radius: 50%;
    border: 1px solid var(--border-strong);
    background: var(--surface);
    display: grid;
    place-items: center;
    margin-bottom: 4px;
  }
  .cs-working-title {
    font-size: var(--r-xl);
    color: var(--fg-bright);
    font-weight: 500;
  }
  .cs-working-sub,
  .cs-empty-state .ces-sub {
    max-width: 380px;
    font-size: var(--t-sm);
    line-height: 1.55;
  }
  .cs-empty-state .ces-title {
    font-size: var(--r-lg);
    color: var(--fg-bright);
    font-weight: 500;
  }
  .cs-working .btn {
    margin-top: 4px;
  }
</style>
