<script lang="ts">
  import { app } from "../lib/store.svelte";
  import Icon from "./Icon.svelte";
  import { stations } from "../lib/mock";

  const stationName = $derived(
    stations.find((s) => s.id === app.music.current)?.name ?? "Study sound"
  );

  // Local state: which subject is expanded in the tree
  let expanded = $state<string | null>(null);

  function toggleExpand(id: string, e: MouseEvent) {
    e.stopPropagation();
    expanded = expanded === id ? null : id;
  }

  function openSubject(id: string) {
    expanded = id;
    app.openSubject(id);
  }

  function renameSubject(s: { id: string; name: string; code?: string | null }, e: Event) {
    e.stopPropagation();
    const next = window.prompt("Rename subject", s.name);
    if (next && next.trim() && next.trim() !== s.name) {
      app.updateSubject(s.id, next.trim(), s.code ?? undefined);
    }
  }

  function removeSubject(s: { id: string; name: string }, e: Event) {
    e.stopPropagation();
    if (window.confirm(`Delete subject “${s.name}”? This can’t be undone.`)) {
      app.deleteSubject(s.id);
    }
  }
</script>

<div class="sidebar">
  <!-- Brand row -->
  <div class="sb-brand">
    <span class="glyph">C</span>
    <span class="b-name">Cortex</span>
    <span class="b-spacer"></span>
    <span class="b-cmd" title="Command palette ( : )" onclick={() => (app.cmdkOpen = true)}>
      <Icon name="cmd" size={15} />
    </span>
  </div>

  <div class="sb-scroll">
    <!-- Nav items -->
    <div
      class="sb-nav-item{app.view === 'dashboard' ? ' on' : ''}"
      onclick={() => app.setView("dashboard")}
      role="button"
      tabindex="0"
      onkeydown={(e) => e.key === "Enter" && app.setView("dashboard")}
    >
      <Icon name="home" size={14} /> Dashboard <span class="nav-k">␣ g</span>
    </div>
    <div
      class="sb-nav-item{app.view === 'recorder' ? ' on' : ''}"
      onclick={() => app.setView("recorder")}
      role="button"
      tabindex="0"
      onkeydown={(e) => e.key === "Enter" && app.setView("recorder")}
    >
      <Icon name="record" size={13} /> Record lecture <span class="nav-k">␣ r</span>
    </div>
    <div
      class="sb-nav-item{app.view === 'websearch' ? ' on' : ''}"
      onclick={() => app.setView("websearch")}
      role="button"
      tabindex="0"
      onkeydown={(e) => e.key === "Enter" && app.setView("websearch")}
    >
      <Icon name="search" size={14} /> Web search <span class="nav-k">␣ w</span>
    </div>
    <div
      class="sb-nav-item{app.view === 'add-source' ? ' on' : ''}"
      onclick={() => app.setView("add-source")}
      role="button"
      tabindex="0"
      onkeydown={(e) => e.key === "Enter" && app.setView("add-source")}
    >
      <Icon name="plus" size={14} /> Add source <span class="nav-k">␣ s</span>
    </div>

    <!-- Subjects section header -->
    <div class="sb-section-l">
      <span class="label">Subjects</span>
      <span
        class="add"
        title="New subject"
        role="button"
        tabindex="0"
        onclick={() => app.setView("add-subject")}
        onkeydown={(e) => e.key === "Enter" && app.setView("add-subject")}
      >
        <Icon name="plus" size={13} />
      </span>
    </div>

    <!-- Subjects tree -->
    {#each app.subjects as s (s.id)}
      <div class="sb-subj">
        <div
          class="sb-subj-row{app.activeSubjectId === s.id && app.view !== 'dashboard' ? ' on' : ''}"
          onclick={() => openSubject(s.id)}
          role="button"
          tabindex="0"
          onkeydown={(e) => e.key === "Enter" && openSubject(s.id)}
        >
          <span
            class="twisty{expanded === s.id ? ' open' : ''}"
            onclick={(e) => toggleExpand(s.id, e)}
            role="button"
            tabindex="-1"
            aria-label="Expand {s.name}"
          >
            <Icon name="chevron" size={11} />
          </span>
          <span class="s-name">{s.name}</span>
          <span class="s-actions">
            <button
              class="s-act"
              type="button"
              title="Rename subject"
              aria-label="Rename {s.name}"
              onclick={(e) => renameSubject(s, e)}
            >
              <Icon name="pencil" size={12} />
            </button>
            <button
              class="s-act s-act--danger"
              type="button"
              title="Delete subject"
              aria-label="Delete {s.name}"
              onclick={(e) => removeSubject(s, e)}
            >
              <Icon name="x" size={12} />
            </button>
          </span>
          <span class="s-count">{s.sourceCount}</span>
          <span
            class="s-dot"
            style:background={s.status === "ready" ? "var(--ok)" : "var(--warn)"}
          ></span>
        </div>

        {#if expanded === s.id}
          <div class="sb-children">
            {#each s.topics as t (t.id)}
              <div
                class="sb-topic"
                onclick={() => openSubject(s.id)}
                role="button"
                tabindex="0"
                onkeydown={(e) => e.key === "Enter" && openSubject(s.id)}
              >
                <Icon name="chevron" size={10} /> {t.name}
              </div>
              {#each t.sources as src (src.id)}
                <div
                  class="sb-src"
                  role="button"
                  tabindex="0"
                  onclick={() => app.openSource(src)}
                  onkeydown={(e) => (e.key === 'Enter' || e.key === ' ') && app.openSource(src)}
                >
                  <span
                    class="badge badge--{src.kind === 'audio' ? 'audio' : src.kind}"
                    style:height="14px"
                    style:padding="0 4px"
                    style:font-size="9px"
                  >{src.kind.toUpperCase().slice(0, 3)}</span>
                  <span style:overflow="hidden" style:text-overflow="ellipsis" style:white-space="nowrap">{src.name}</span>
                </div>
              {/each}
            {/each}
          </div>
        {/if}
      </div>
    {/each}
  </div>

  <button
    class="sb-settings{app.view === 'settings' ? ' on' : ''}"
    onclick={() => app.setView("settings")}
    title="Settings"
  >
    <Icon name="settings" size={14} /> Settings
  </button>

  <!-- Music mini row — opens the study-sound panel -->
  <div
    class="sb-music"
    role="button"
    tabindex="0"
    title="Study sound (m)"
    onclick={() => (app.musicOpen = true)}
    onkeydown={(e) => e.key === "Enter" && (app.musicOpen = true)}
  >
    <div class="m-art">
      {#if app.music.playing}
        <span class="eq"><i></i><i></i><i></i><i></i></span>
      {:else}
        <Icon name="music" size={14} color="var(--accent-fg)" />
      {/if}
    </div>
    <div style:flex="1" style:min-width="0">
      <div class="m-name">{stationName}</div>
      <div class="m-sub">lo-fi · ad-free study</div>
    </div>
    <button
      class="m-play"
      title={app.music.playing ? "Pause" : "Play"}
      onclick={(e) => { e.stopPropagation(); app.toggleMusic(); }}
    >
      <Icon name={app.music.playing ? "pause" : "play"} size={14} />
    </button>
  </div>
</div>

<style>
  /* Per-subject inline rename / delete controls — faint, brighten on row hover */
  .sb-subj-row .s-actions {
    display: flex;
    align-items: center;
    gap: 2px;
    margin-left: auto;
    opacity: 0;
    transition: opacity var(--dur-fast);
  }
  .sb-subj-row:hover .s-actions,
  .sb-subj-row:focus-within .s-actions {
    opacity: 1;
  }
  .sb-subj-row .s-act {
    display: grid;
    place-items: center;
    width: 20px;
    height: 20px;
    padding: 0;
    border: none;
    background: none;
    border-radius: var(--rad-2);
    color: var(--fg-faint);
    cursor: pointer;
    transition: color var(--dur-fast), background var(--dur-fast);
  }
  .sb-subj-row .s-act:hover {
    color: var(--fg-bright);
    background: var(--surface-3);
  }
  .sb-subj-row .s-act--danger:hover {
    color: var(--err);
  }
  /* Count still hugs the dot; cancel the row's default margin-auto on name */
  .sb-subj-row .s-actions + .s-count {
    margin-left: 0;
  }
</style>
