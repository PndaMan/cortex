<script lang="ts">
  import { app } from "../lib/store.svelte";
  import Icon from "./Icon.svelte";
  import { stations } from "../lib/mock";
  import { moveItem, reorderable } from "../lib/dnd";

  // Group stations by category
  const cats = $derived.by(() => {
    const map: Record<string, typeof stations> = {};
    for (const s of stations) {
      (map[s.cat] ??= []).push(s);
    }
    return map;
  });

  // Resolve a station id (built-in or custom) to uniform display fields.
  type Row = { id: string; name: string; kind: string; ico: string };
  function resolve(id: string): Row | null {
    const b = stations.find((s) => s.id === id);
    if (b) return { id: b.id, name: b.name, kind: b.kind, ico: b.ico };
    const c = app.customStations.find((s) => s.id === id);
    if (c) return { id: c.id, name: c.name, kind: c.kind === "live" ? "live" : "youtube", ico: "music" };
    return null; // a favourited id whose station was removed
  }
  const favStations = $derived(app.stationFavs.map(resolve).filter(Boolean) as Row[]);

  const cur = $derived(
    stations.find(s => s.id === app.music.current)
    ?? app.customStations.find(s => s.id === app.music.current)
    ?? stations[0]
  );

  // ── add-your-own (YouTube video or livestream by URL) ──
  let adding = $state(false);
  let newName = $state("");
  let newUrl = $state("");
  let busy = $state(false);

  function looksLikeUrl(u: string): boolean {
    return /^https?:\/\/\S+$/i.test(u.trim());
  }

  async function submitStation() {
    const url = newUrl.trim();
    if (!looksLikeUrl(url) || busy) return;
    busy = true;
    // Default a friendly name from the URL if none given.
    const name = newName.trim() || "YouTube station";
    const kind = /[?&]list=|\/live\b|live$/i.test(url) ? "live" : "youtube";
    const st = await app.addCustomStation(name, url, kind);
    busy = false;
    if (st) { newName = ""; newUrl = ""; adding = false; }
  }

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

      <!-- Star toggle, reused on every row -->
      {#snippet favBtn(id: string)}
        <button
          class={"st-fav btn btn--icon btn--sm btn--ghost" + (app.stationFavs.includes(id) ? " on" : "")}
          title={app.stationFavs.includes(id) ? "Unfavourite" : "Favourite"}
          onclick={(e) => { e.stopPropagation(); app.toggleStationFav(id); }}
        >
          <Icon name="star" size={12} color={app.stationFavs.includes(id) ? "var(--accent)" : "var(--fg-faint)"} />
        </button>
      {/snippet}

      <!-- Station list -->
      <div class="music-list">
        <!-- Favourites (built-in or custom), pinned to the top -->
        {#if favStations.length}
          <div class="music-cat">★ Favourites</div>
          {#each favStations as s (s.id)}
            <div
              class={"station" + (s.id === app.music.current ? " on" : "")}
              role="button"
              tabindex="0"
              onclick={() => app.pickStation(s.id)}
              onkeydown={(e) => { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); app.pickStation(s.id); } }}
            >
              <span class="st-art">
                <Icon name={s.ico} size={14} color={s.id === app.music.current ? "var(--accent)" : "var(--fg-muted)"} />
              </span>
              <span class="st-name">{s.name}</span>
              {#if s.id === app.music.current && app.music.playing}
                <span class="st-eq"><i></i><i></i><i></i></span>
              {:else}
                <span class="st-kind mono">{s.kind}</span>
              {/if}
              {@render favBtn(s.id)}
            </div>
          {/each}
        {/if}

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
              {@render favBtn(s.id)}
            </div>
          {/each}
        {/each}

        <!-- User-added YouTube / URL stations — drag to reorder -->
        <div class="music-cat">My stations</div>
        {#each app.customStations as s, i (s.id)}
          <div
            class={"station" + (s.id === app.music.current ? " on" : "")}
            role="button"
            tabindex="0"
            onclick={() => app.pickStation(s.id)}
            onkeydown={(e) => { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); app.pickStation(s.id); } }}
            use:reorderable={{ index: i, group: "stations", onReorder: (from, to) => app.reorderCustomStations(moveItem(app.customStations.map((x) => x.id), from, to)) }}
          >
            <span class="st-art">
              <Icon name="music" size={14} color={s.id === app.music.current ? "var(--accent)" : "var(--fg-muted)"} />
            </span>
            <span class="st-name">{s.name}</span>
            {#if s.id === app.music.current && app.music.playing}
              <span class="st-eq"><i></i><i></i><i></i></span>
            {:else}
              <span class="st-kind mono">{s.kind === "live" ? "live" : "youtube"}</span>
            {/if}
            {@render favBtn(s.id)}
            <button
              class="st-del btn btn--icon btn--sm btn--ghost"
              title="Remove station"
              onclick={(e) => { e.stopPropagation(); app.removeCustomStation(s.id); }}
            >
              <Icon name="x" size={11} />
            </button>
          </div>
        {/each}

        {#if adding}
          <div class="music-add">
            <input class="input" bind:value={newName} placeholder="Name (e.g. Lofi 2hr mix)" />
            <input
              class="input mono"
              bind:value={newUrl}
              placeholder="Paste a YouTube video or live URL…"
              onkeydown={(e) => { if (e.key === "Enter") { e.preventDefault(); submitStation(); } }}
            />
            <div class="music-add-actions">
              <button class="btn btn--sm btn--ghost" onclick={() => { adding = false; newName = ""; newUrl = ""; }}>Cancel</button>
              <button class="btn btn--sm btn--primary" disabled={!looksLikeUrl(newUrl) || busy} onclick={submitStation}>
                {busy ? "Adding…" : "Add station"}
              </button>
            </div>
          </div>
        {:else}
          <button class="music-add-btn" onclick={() => (adding = true)}>
            <Icon name="plus" size={12} /> Add a YouTube station
          </button>
        {/if}
      </div>
    </div>
  </div>
{/if}
