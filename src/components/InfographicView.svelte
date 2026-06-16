<script lang="ts">
  import Icon from "./Icon.svelte";

  // Structured infographic payload (NotebookLM-style poster). Older materials may
  // still carry a raw { svg } payload — we fall back to rendering that.
  interface Stat { value?: string; label?: string }
  interface Section { emoji?: string; heading?: string; points?: string[]; stat?: Stat }
  interface TimelineEvent { date?: string; title?: string; detail?: string }
  interface InfoData {
    title?: string; subtitle?: string; sections?: Section[];
    timeline?: TimelineEvent[]; takeaway?: string;
    svg?: string; image?: string;
  }

  let { data, onExit }: { data?: InfoData; onExit?: () => void } = $props();

  const image = $derived(typeof data?.image === "string" ? data!.image! : "");
  const sections = $derived(Array.isArray(data?.sections) ? data!.sections! : []);
  const timeline = $derived(
    (Array.isArray(data?.timeline) ? data!.timeline! : []).filter(
      (e) => e && (e.date || e.title || e.detail)
    )
  );
  const takeaway = $derived(typeof data?.takeaway === "string" ? data!.takeaway!.trim() : "");
  const hasPoster = $derived(sections.length > 0 || timeline.length > 0 || !!takeaway);
  // AI-generated SVG is untrusted — strip scripts, foreignObject (HTML embed),
  // event-handler attributes, and javascript:/html-data: URLs before {@html}.
  // (The app CSP's script-src 'self' is the second line of defence.)
  function sanitizeSvg(svg: string): string {
    if (!svg.trim()) return "";
    try {
      const doc = new DOMParser().parseFromString(svg, "image/svg+xml");
      if (doc.querySelector("parsererror")) return "";
      doc.querySelectorAll("script, foreignObject, iframe, object, embed").forEach((n) => n.remove());
      doc.querySelectorAll("*").forEach((el) => {
        for (const attr of [...el.attributes]) {
          const name = attr.name.toLowerCase();
          const val = attr.value.replace(/\s+/g, "").toLowerCase();
          if (name.startsWith("on")) el.removeAttribute(attr.name);
          else if (/^(href|xlink:href|src)$/.test(name) && (val.startsWith("javascript:") || val.startsWith("data:text/html"))) el.removeAttribute(attr.name);
        }
      });
      const root = doc.documentElement;
      return root?.tagName?.toLowerCase() === "svg" ? root.outerHTML : "";
    } catch {
      return "";
    }
  }
  const legacySvg = $derived(sanitizeSvg(typeof data?.svg === "string" ? data!.svg! : ""));
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
        {#if takeaway}
          <div class="poster-takeaway">
            <span class="ptk-label mono">KEY TAKEAWAY</span>
            <p class="ptk-text">{takeaway}</p>
          </div>
        {/if}

        <div class="poster-body" class:poster-body--split={sections.length > 0 && timeline.length > 0}>
          {#if sections.length}
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
          {/if}

          {#if timeline.length}
            <section class="poster-timeline">
              <h2 class="ptl-heading"><span aria-hidden="true">🕑</span> Timeline</h2>
              <ol class="ptl-track">
                {#each timeline as ev, i (i)}
                  <li class="ptl-item">
                    <span class="ptl-node" aria-hidden="true"></span>
                    <div class="ptl-body">
                      {#if ev.date}<span class="ptl-date mono">{ev.date}</span>{/if}
                      {#if ev.title}<span class="ptl-title">{ev.title}</span>{/if}
                      {#if ev.detail}<p class="ptl-detail">{ev.detail}</p>{/if}
                    </div>
                  </li>
                {/each}
              </ol>
            </section>
          {/if}
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
  /* Single dense "poster" that aims to fit a 1080p viewport. We use a
     container query (poster is the query root) so the sections/timeline
     split reacts to the poster's own width, not the global viewport. */
  .poster { max-width: 1280px; margin: 0 auto; width: 100%; container-type: inline-size; }
  .poster-head { text-align: center; margin-bottom: var(--sp-4); padding-bottom: var(--sp-3); border-bottom: 2px solid color-mix(in oklab, var(--accent) 50%, transparent); }
  .poster-title { font-size: clamp(24px, 3.4vw, 38px); font-weight: 700; color: var(--fg-bright); margin: 0 0 4px; line-height: 1.05; }
  .poster-sub { font-size: var(--t-sm); color: var(--accent); letter-spacing: 0.04em; }

  /* On wide posters, lay sections and timeline side-by-side. Sections get
     the larger share; the timeline becomes a tight rail on the right. */
  .poster-body { display: flex; flex-direction: column; gap: var(--sp-5); }
  @container (min-width: 760px) {
    .poster-body--split {
      display: grid;
      grid-template-columns: minmax(0, 1.55fr) minmax(280px, 1fr);
      gap: var(--sp-5);
      align-items: start;
    }
  }

  .poster-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(220px, 1fr));
    gap: 12px;
    align-items: start;
  }
  .poster-card {
    background: color-mix(in oklab, var(--accent) 7%, var(--surface));
    border: 1px solid color-mix(in oklab, var(--accent) 28%, var(--border));
    border-radius: var(--rad-4);
    padding: 12px 13px 11px;
    break-inside: avoid;
  }
  .pc-head { display: flex; align-items: center; gap: 8px; margin-bottom: 8px; }
  .pc-emoji { font-size: 19px; line-height: 1; flex: none; }
  .pc-heading { font-size: 15px; font-weight: 650; color: var(--fg-bright); margin: 0; line-height: 1.2; }
  .pc-stat { display: flex; align-items: baseline; gap: 7px; margin: 0 0 8px; }
  .pc-stat-val { font-size: 26px; font-weight: 750; color: var(--accent); line-height: 1; letter-spacing: -0.01em; }
  .pc-stat-label { font-size: var(--t-xs); color: var(--fg-muted); }
  .pc-points { margin: 0; padding-left: 16px; display: flex; flex-direction: column; gap: 4px; }
  .pc-points li { font-size: 13px; line-height: 1.4; color: var(--fg); }
  .pc-points li::marker { color: var(--accent); }

  /* ── key takeaway banner ─────────────────────────────────── */
  .poster-takeaway {
    margin: 0 0 var(--sp-4);
    padding: 11px 16px;
    border-radius: var(--rad-4);
    background: color-mix(in oklab, var(--accent) 12%, var(--surface));
    border: 1px solid color-mix(in oklab, var(--accent) 34%, var(--border));
    border-left: 3px solid var(--accent);
  }
  .ptk-label { display: block; font-size: var(--t-xs); letter-spacing: 0.08em; color: var(--accent); margin-bottom: 2px; }
  .ptk-text { margin: 0; font-size: 14.5px; line-height: 1.45; color: var(--fg-bright); font-weight: 500; }

  /* ── timeline (compact, horizontal-wrapping rail) ─────────── */
  .poster-timeline { margin: 0; min-width: 0; }
  .ptl-heading {
    font-size: 16px; font-weight: 700; color: var(--fg-bright);
    margin: 0 0 var(--sp-3); display: flex; align-items: center; gap: 7px;
  }
  /* Wrapping grid of uniform event chips — reads left-to-right, top-to-bottom,
     so most/all events are visible without a long vertical scroll. */
  .ptl-track {
    list-style: none; margin: 0; padding: 0;
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(180px, 1fr));
    gap: 10px;
  }
  /* In the split (side-by-side) layout the timeline column is narrow, so keep
     events stacked in a single tidy column there. */
  @container (min-width: 760px) {
    .poster-body--split .ptl-track { grid-template-columns: 1fr; gap: 8px; }
  }
  .ptl-item {
    position: relative;
    padding: 9px 11px 9px 22px;
    border: 1px solid color-mix(in oklab, var(--accent) 22%, var(--border));
    border-radius: var(--rad-3);
    background: color-mix(in oklab, var(--accent) 5%, var(--surface));
    min-width: 0;
  }
  .ptl-node {
    position: absolute; left: 9px; top: 13px; width: 9px; height: 9px;
    border-radius: 50%; background: var(--accent);
    box-shadow: 0 0 0 3px color-mix(in oklab, var(--accent) 16%, transparent);
  }
  .ptl-body { display: flex; flex-direction: column; gap: 1px; min-width: 0; }
  .ptl-date { font-size: var(--t-2xs); color: var(--accent); letter-spacing: 0.03em; }
  .ptl-title {
    font-size: 13.5px; font-weight: 650; color: var(--fg-bright); line-height: 1.25;
    overflow: hidden; text-overflow: ellipsis;
    display: -webkit-box; -webkit-line-clamp: 2; line-clamp: 2; -webkit-box-orient: vertical;
  }
  .ptl-detail {
    margin: 1px 0 0; font-size: 12px; line-height: 1.4; color: var(--fg-muted);
    overflow: hidden; text-overflow: ellipsis;
    display: -webkit-box; -webkit-line-clamp: 2; line-clamp: 2; -webkit-box-orient: vertical;
  }

  @container (max-width: 759px) {
    .poster-grid { grid-template-columns: repeat(auto-fill, minmax(200px, 1fr)); }
  }

  @media (max-width: 600px) {
    .infographic-page { padding: var(--sp-5) var(--sp-4); }
    .poster-grid { grid-template-columns: 1fr; }
    .ptl-track { grid-template-columns: 1fr; }
  }
</style>
