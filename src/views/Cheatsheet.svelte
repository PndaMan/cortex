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
      } else {
        sections = [];
        cheatTopic = sub.topics[0]?.name ?? "";
        cheatSub = sub.name;
        sourceCount = sub.sourceCount;
        hasCheatsheet = false;
      }
    }).catch(() => {
      sections = [];
      cheatTopic = sub.topics[0]?.name ?? "";
      cheatSub = sub.name;
      sourceCount = sub.sourceCount;
      hasCheatsheet = false;
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
      <div class="cs-doc-head">
        <div>
          <div class="eyebrow">Cheatsheet</div>
          <h1 class="cs-title">{app.activeSubject.topics[0]?.name ?? app.activeSubject.name}</h1>
          <div class="cs-sub mono">
            {app.activeSubject.name}{app.activeSubject.code ? " · " + app.activeSubject.code : ""}
          </div>
        </div>
      </div>

      <div class="cs-empty-state">
        <Icon name="diamond" size={28} color="var(--fg3)" />
        <div class="ces-title">No cheatsheet yet</div>
        <div class="ces-sub">
          Generate a completeness-checked cheatsheet from this subject's {app.activeSubject.sourceCount} source{app.activeSubject.sourceCount !== 1 ? "s" : ""}.
        </div>
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
