<script lang="ts">
  import { app } from "../lib/store.svelte";
  import Icon from "../components/Icon.svelte";

  // Derive a simple day/week string for the eyebrow
  const now = new Date();
  const dayName = now.toLocaleDateString(undefined, { weekday: "long" });
  // Simple week number calculation
  const startOfYear = new Date(now.getFullYear(), 0, 1);
  const weekNum = Math.ceil(((now.getTime() - startOfYear.getTime()) / 86400000 + startOfYear.getDay() + 1) / 7);
</script>

<div class="workspace-scroll">
  <div class="dash">
    <header class="dash-head">
      <div>
        <div class="eyebrow">{dayName} · week {weekNum}</div>
        <h1 class="dash-title read">Your subjects</h1>
      </div>
      <div class="row gap-2">
        <button class="btn btn--sm" onclick={() => app.setView("websearch")}>
          <Icon name="search" size={13} /> Search the web
        </button>
        <button class="btn btn--sm btn--primary" onclick={() => app.setView("add-subject")}>
          <Icon name="plus" size={13} /> New subject
        </button>
      </div>
    </header>

    <div class="subj-grid">
      {#if app.subjects.length === 0}
        <div style:grid-column="1 / -1" style:text-align="center" style:padding="48px 0" style:color="var(--fg-faint)">
          Add your first subject.
        </div>
      {/if}

      {#each app.subjects as s, i (s.id)}
        <button
          class="subj-card{i === app.dashFocus ? ' kb-focus' : ''}"
          onclick={() => app.openSubject(s.id)}
        >
          <div class="sc-top">
            <span class="subj-glyph"><Icon name="diamond" size={15} color="var(--accent)" /></span>
            <span class="subj-code mono">{s.code ?? ""}</span>
            <div class="grow"></div>
            {#if s.streak > 0}
              <span class="streak mono" title="study streak">
                <Icon name="bolt" size={11} color="var(--warn)" /> {s.streak}
              </span>
            {/if}
          </div>
          <h3 class="subj-name read">{s.name}</h3>
          <div class="subj-stat mono">{s.sourceCount} sources · {s.topics.length} topics</div>
          <div class="subj-foot">
            {#if s.status === "ready"}
              <span class="status-pill status-pill--ready"><span class="dot"></span>Cheatsheet ready</span>
            {:else}
              <span class="status-pill status-pill--review"><span class="dot"></span>Needs review</span>
            {/if}
          </div>
        </button>
      {/each}

      <button class="subj-card subj-card--add" onclick={() => app.setView("add-subject")}>
        <div class="add-inner">
          <Icon name="plus" size={20} color="var(--fg-faint)" />
          <span>Add subject</span>
        </div>
      </button>
    </div>
  </div>
</div>
