<script lang="ts">
  import { t } from "../lib/i18n.svelte";
  import { app } from "../lib/store.svelte";
  import Icon from "../components/Icon.svelte";
  import { moveItem, reorderable } from "../lib/dnd";

  // ── pointer-based card reordering (no native HTML5 drag — see lib/dnd.ts) ──
  function reorderSubject(from: number, to: number) {
    app.reorderSubjects(moveItem(app.subjects.map((s) => s.id), from, to));
  }

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
        <div class="eyebrow">{dayName} · {t("week {n}", { n: weekNum })}</div>
        <h1 class="dash-title">{t("Your subjects")}</h1>
      </div>
      <div class="row gap-2">
        <button class="btn btn--sm btn--primary" onclick={() => app.setView("add-subject")}>
          <Icon name="plus" size={13} /> {t("New subject")}
        </button>
      </div>
    </header>

    <div class="subj-grid">
      {#if app.subjects.length === 0}
        <div style:grid-column="1 / -1" style:text-align="center" style:padding="48px 0" style:color="var(--fg-faint)">
          {t("Add your first subject.")}
        </div>
      {/if}

      {#each app.subjects as s, i (s.id)}
        <button
          class="subj-card{i === app.dashFocus ? ' kb-focus' : ''}"
          onclick={() => app.openSubject(s.id)}
          style:box-shadow="inset 3px 0 0 {app.subjectColor(s)}"
          use:reorderable={{ index: i, group: "subjects", onReorder: reorderSubject }}
        >
          <div class="sc-top">
            <span class="subj-glyph" style="font-size:15px;line-height:1">{s.glyph || "◆"}</span>
            <span class="subj-code mono">{s.code ?? ""}</span>
            <div class="grow"></div>
            {#if s.streak > 0}
              <span class="streak mono" title={t("study streak")}>
                <Icon name="bolt" size={11} color="var(--warn)" /> {s.streak}
              </span>
            {/if}
          </div>
          <h3 class="subj-name read">{s.name}</h3>
          <div class="subj-stat mono">{t("{n} sources · {m} topics", { n: s.sourceCount, m: s.topics.length })}</div>
          <div class="subj-foot">
            {#if s.sourceCount === 0}
              <span class="status-pill status-pill--review"><span class="dot"></span>{t("No sources yet")}</span>
            {:else}
              <span class="status-pill status-pill--ready"><span class="dot"></span>{t("{n} sources", { n: s.sourceCount })}</span>
            {/if}
          </div>
        </button>
      {/each}

      <button class="subj-card subj-card--add" onclick={() => app.setView("add-subject")}>
        <div class="add-inner">
          <Icon name="plus" size={20} color="var(--fg-faint)" />
          <span>{t("Add subject")}</span>
        </div>
      </button>
    </div>
  </div>
</div>
