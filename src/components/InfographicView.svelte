<script lang="ts">
  import Icon from "./Icon.svelte";

  // Structured infographic payload (NotebookLM-style poster). Older materials may
  // still carry a raw { svg } payload — we fall back to rendering that.
  interface Stat { value?: string; label?: string }
  interface Section { emoji?: string; heading?: string; points?: string[]; stat?: Stat }
  interface InfoData { title?: string; subtitle?: string; sections?: Section[]; svg?: string; image?: string }

  let { data, onExit }: { data?: InfoData; onExit?: () => void } = $props();

  const image = $derived(typeof data?.image === "string" ? data!.image! : "");
  const sections = $derived(Array.isArray(data?.sections) ? data!.sections! : []);
  const hasPoster = $derived(sections.length > 0);
  const legacySvg = $derived(typeof data?.svg === "string" ? data!.svg! : "");
</script>

<div class="workspace-scroll">
  <div class="infographic-page">
    {#if onExit}
      <div class="infographic-toolbar">
        <button class="btn btn--icon btn--sm btn--ghost" onclick={onExit} title="Back to materials">
          <span style="display:inline-flex;transform:rotate(180deg)"><Icon name="chevron" size={14} /></span>
        </button>
        <span class="mono faint" style="font-size: var(--t-xs);">Infographic</span>
      </div>
    {/if}

    {#if image}
      <div class="infographic-canvas">
        <img class="poster-img" src={image} alt={data?.title ?? "infographic"} />
      </div>
    {:else if hasPoster}
      <div class="poster">
        <header class="poster-head">
          {#if data?.title}<h1 class="poster-title read">{data.title}</h1>{/if}
          {#if data?.subtitle}<p class="poster-sub mono">{data.subtitle}</p>{/if}
        </header>
        <div class="poster-grid">
          {#each sections as sec, i (i)}
            <section class="poster-card">
              <div class="pc-head">
                {#if sec.emoji}<span class="pc-emoji" aria-hidden="true">{sec.emoji}</span>{/if}
                <h2 class="pc-heading">{sec.heading ?? ""}</h2>
              </div>
              {#if sec.stat?.value}
                <div class="pc-stat">
                  <span class="pc-stat-val">{sec.stat.value}</span>
                  {#if sec.stat.label}<span class="pc-stat-label">{sec.stat.label}</span>{/if}
                </div>
              {/if}
              {#if sec.points?.length}
                <ul class="pc-points">
                  {#each sec.points as p (p)}<li>{p}</li>{/each}
                </ul>
              {/if}
            </section>
          {/each}
        </div>
      </div>
    {:else if legacySvg}
      <div class="infographic-canvas">{@html legacySvg}</div>
    {:else}
      <div class="infographic-empty">
        <Icon name="grid" size={26} color="var(--fg-faint)" />
        <p class="mono faint" style="margin-top: 12px;">No infographic content.</p>
      </div>
    {/if}
  </div>
</div>

<style>
  .infographic-page { display: flex; flex-direction: column; min-height: 100%; padding: var(--sp-6) var(--sp-8); gap: var(--sp-5); }
  .infographic-toolbar { display: flex; align-items: center; gap: 10px; flex: none; }
  .infographic-empty { flex: 1; display: flex; flex-direction: column; align-items: center; justify-content: center; padding: var(--sp-10); }
  .infographic-canvas { flex: 1; display: flex; justify-content: center; align-items: flex-start; padding: var(--sp-4); }
  .infographic-canvas :global(svg) { max-width: 100%; height: auto; border-radius: var(--rad-3); box-shadow: var(--shadow-2); }
  .poster-img { max-width: 760px; width: 100%; height: auto; border-radius: var(--rad-3); box-shadow: var(--shadow-2); }

  /* ── structured poster ───────────────────────────────────── */
  .poster { max-width: 1100px; margin: 0 auto; width: 100%; }
  .poster-head { text-align: center; margin-bottom: var(--sp-6); padding-bottom: var(--sp-4); border-bottom: 2px solid color-mix(in oklab, var(--accent) 50%, transparent); }
  .poster-title { font-size: clamp(26px, 4vw, 44px); font-weight: 700; color: var(--fg-bright); margin: 0 0 6px; line-height: 1.05; }
  .poster-sub { font-size: var(--t-sm); color: var(--accent); letter-spacing: 0.04em; }

  .poster-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(260px, 1fr));
    gap: 16px;
    align-items: start;
  }
  .poster-card {
    background: color-mix(in oklab, var(--accent) 7%, var(--surface));
    border: 1px solid color-mix(in oklab, var(--accent) 28%, var(--border));
    border-radius: var(--rad-4);
    padding: 16px 16px 14px;
    break-inside: avoid;
  }
  .pc-head { display: flex; align-items: center; gap: 9px; margin-bottom: 10px; }
  .pc-emoji { font-size: 22px; line-height: 1; flex: none; }
  .pc-heading { font-size: 16px; font-weight: 650; color: var(--fg-bright); margin: 0; }
  .pc-stat { display: flex; align-items: baseline; gap: 8px; margin: 0 0 10px; }
  .pc-stat-val { font-size: 30px; font-weight: 750; color: var(--accent); line-height: 1; letter-spacing: -0.01em; }
  .pc-stat-label { font-size: var(--t-xs); color: var(--fg-muted); }
  .pc-points { margin: 0; padding-left: 18px; display: flex; flex-direction: column; gap: 6px; }
  .pc-points li { font-size: 13.5px; line-height: 1.45; color: var(--fg); }
  .pc-points li::marker { color: var(--accent); }
</style>
