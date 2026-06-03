<script lang="ts">
  import { app } from "../lib/store.svelte";
  import Icon from "./Icon.svelte";
  import { stations } from "../lib/mock";

  // Group stations by category
  const cats = $derived.by(() => {
    const map: Record<string, typeof stations> = {};
    for (const s of stations) {
      (map[s.cat] ??= []).push(s);
    }
    return map;
  });

  const cur = $derived(stations.find(s => s.id === app.music.current) ?? stations[0]);

  function onKeyDown(e: KeyboardEvent) {
    if (e.key === "Escape") app.musicOpen = false;
  }
</script>

<svelte:window onkeydown={onKeyDown} />

{#if app.musicOpen}
  <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
  <div class="overlay music-overlay" role="presentation" onmousedown={() => (app.musicOpen = false)}>
    <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
    <div class="music-modal" role="presentation" onmousedown={e => e.stopPropagation()}>
      <header class="music-head">
        <div>
          <div class="eyebrow">Study sound</div>
          <div class="mh-title">Now playing</div>
        </div>
        <button class="btn btn--icon btn--sm btn--ghost" onclick={() => (app.musicOpen = false)}>
          <Icon name="x" size={12} />
        </button>
      </header>

      <!-- Now playing row -->
      <div class="music-now">
        <div class="mn-art">
          {#if app.music.playing}
            <span class="eq"><i></i><i></i><i></i><i></i></span>
          {:else}
            <Icon name="music" size={18} color="var(--accent-fg)" />
          {/if}
        </div>
        <div class="mn-info">
          <div class="mn-name">{cur.name}</div>
          <div class="mn-sub mono">{cur.kind} · ad-free</div>
        </div>
        <button class="mn-play" onclick={() => app.toggleMusic()} title="Play / pause">
          <Icon name={app.music.playing ? "pause" : "play"} size={16} />
        </button>
      </div>

      <!-- Volume -->
      <div class="music-vol">
        <Icon name="music" size={13} color="var(--fg-faint)" />
        <input
          type="range"
          min="0"
          max="100"
          value={app.music.volume}
          oninput={e => app.setVolume(Number((e.target as HTMLInputElement).value))}
        />
        <span class="mono faint" style="font-size:var(--t-2xs);width:30px;text-align:right">
          {app.music.volume}%
        </span>
      </div>

      <!-- Station list -->
      <div class="music-list">
        {#each Object.entries(cats) as [cat, items]}
          <div class="music-cat">{cat}</div>
          {#each items as s}
            <div
              class={"station" + (s.id === app.music.current ? " on" : "")}
              role="button"
              tabindex="0"
              onclick={() => app.pickStation(s.id)}
              onkeydown={(e) => { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); app.pickStation(s.id); } }}
            >
              <span class="st-art">
                <Icon
                  name={s.ico}
                  size={14}
                  color={s.id === app.music.current ? "var(--accent)" : "var(--fg-muted)"}
                />
              </span>
              <span class="st-name">{s.name}</span>
              {#if s.id === app.music.current && app.music.playing}
                <span class="st-eq"><i></i><i></i><i></i></span>
              {:else}
                <span class="st-kind mono">{s.kind}</span>
              {/if}
            </div>
          {/each}
        {/each}
      </div>
    </div>
  </div>
{/if}
