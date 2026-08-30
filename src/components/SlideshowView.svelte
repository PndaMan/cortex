<script lang="ts">
  import Icon from "./Icon.svelte";
  import { t } from "../lib/i18n.svelte";

  export interface Slide {
    title: string;
    bullets: string[];
    notes?: string;
  }

  let {
    slides,
    title = t("Slideshow"),
    onExit,
  }: { slides: Slide[]; title?: string; onExit?: () => void } = $props();

  let idx = $state(0);
  let showNotes = $state(false);

  const current = $derived(slides[idx] ?? null);
  const hasPrev = $derived(idx > 0);
  const hasNext = $derived(idx < slides.length - 1);
  const hasNotes = $derived(!!current?.notes);

  function prev() { if (hasPrev) { idx -= 1; } }
  function next() { if (hasNext) { idx += 1; } }

  $effect(() => {
    function onKey(e: KeyboardEvent) {
      const el = document.activeElement as HTMLElement | null;
      if (el && (el.tagName === "INPUT" || el.tagName === "TEXTAREA")) return;
      if (e.key === "ArrowRight" || e.key === "ArrowDown") { e.preventDefault(); next(); }
      else if (e.key === "ArrowLeft" || e.key === "ArrowUp") { e.preventDefault(); prev(); }
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });
</script>

<div class="ss-wrap">
  <!-- ── Header bar ── -->
  <div class="ss-bar">
    {#if onExit}
      <button class="btn btn--icon btn--sm btn--ghost" onclick={onExit} title={t("Back to materials")}>
        <span style="display:inline-flex;transform:rotate(180deg)"><Icon name="chevron" size={14} /></span>
      </button>
    {/if}
    <span class="eyebrow" style="flex:1">{title}</span>
    {#if hasNotes}
      <button
        class="btn btn--sm btn--ghost{showNotes ? ' on' : ''}"
        onclick={() => (showNotes = !showNotes)}
        title={t("Toggle speaker notes")}
      >
        <Icon name="grid" size={12} /> {t("Notes")}
      </button>
    {/if}
    <span class="mono faint" style="font-size: var(--t-xs);">{idx + 1} / {slides.length}</span>
  </div>

  {#if slides.length === 0}
    <div class="ss-empty">
      <Icon name="play" size={26} color="var(--fg-faint)" />
      <p class="mono faint" style="margin-top: 12px;">{t("No slides available.")}</p>
    </div>
  {:else if current}
    <!-- ── Slide content ── -->
    <div class="ss-stage">
      <div class="ss-slide">
        <div class="ss-slide-n mono">{idx + 1}</div>
        <h2 class="ss-slide-title read">{current.title}</h2>
        {#if current.bullets.length > 0}
          <ul class="ss-bullets">
            {#each current.bullets as bullet (bullet)}
              <li class="ss-bullet read">{bullet}</li>
            {/each}
          </ul>
        {/if}
      </div>

      {#if showNotes && current.notes}
        <div class="ss-notes">
          <div class="ss-notes-h mono">{t("Speaker notes")}</div>
          <p class="ss-notes-body read">{current.notes}</p>
        </div>
      {/if}
    </div>

    <!-- ── Navigation ── -->
    <div class="ss-nav">
      <button
        class="btn btn--ghost"
        onclick={prev}
        disabled={!hasPrev}
      >
        <span style="display:inline-flex;transform:rotate(180deg)"><Icon name="chevron" size={13} /></span>
        {t("Prev")}
      </button>

      <!-- Slide pips -->
      <div class="ss-pips">
        {#each slides as _, n (n)}
          <button
            class="ss-pip{n === idx ? ' on' : ''}"
            onclick={() => (idx = n)}
            title={t("Slide {n}", { n: n + 1 })}
          ></button>
        {/each}
      </div>

      <button
        class="btn btn--ghost"
        onclick={next}
        disabled={!hasNext}
      >
        {t("Next")}
        <Icon name="chevron" size={13} />
      </button>
    </div>
  {/if}
</div>

<style>
  .ss-wrap {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-height: 0;
    overflow: hidden;
  }

  .ss-bar {
    display: flex;
    align-items: center;
    gap: 10px;
    height: 48px;
    padding: 0 16px;
    border-bottom: 1px solid var(--border);
    flex: none;
  }

  .ss-bar .btn.on {
    background: color-mix(in oklab, var(--accent) 10%, var(--surface));
    border-color: var(--accent);
    color: var(--accent);
  }

  .ss-empty {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
  }

  .ss-stage {
    flex: 1;
    display: flex;
    flex-direction: column;
    overflow-y: auto;
    min-height: 0;
    padding: var(--sp-8) var(--sp-10);
    gap: var(--sp-6);
  }

  .ss-slide {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: var(--sp-6);
    background: var(--surface);
    border: 1px solid var(--border-strong);
    border-radius: var(--rad-4);
    padding: var(--sp-10);
    min-height: 340px;
    position: relative;
    box-shadow: var(--shadow-2);
  }

  .ss-slide-n {
    position: absolute;
    top: 14px;
    right: 16px;
    font-size: var(--t-2xs);
    color: var(--fg-faint);
  }

  .ss-slide-title {
    font-size: var(--r-xl);
    font-weight: 600;
    color: var(--fg-bright);
    text-align: center;
    max-width: 640px;
    margin: 0;
  }

  .ss-bullets {
    list-style: none;
    padding: 0;
    margin: 0;
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
    max-width: 600px;
    width: 100%;
  }

  .ss-bullet {
    display: flex;
    align-items: flex-start;
    gap: 10px;
    font-size: var(--r-md);
    color: var(--fg);
    line-height: 1.45;
  }

  .ss-bullet::before {
    content: "·";
    color: var(--accent);
    font-weight: 700;
    flex: none;
    margin-top: 1px;
  }

  .ss-notes {
    background: var(--bg-sunken);
    border: 1px solid var(--border);
    border-radius: var(--rad-3);
    padding: var(--sp-4) var(--sp-5);
  }

  .ss-notes-h {
    font-size: var(--t-2xs);
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--fg-faint);
    margin-bottom: var(--sp-2);
  }

  .ss-notes-body {
    font-size: var(--r-sm);
    color: var(--fg-muted);
    line-height: 1.55;
    margin: 0;
  }

  .ss-nav {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 12px 20px;
    border-top: 1px solid var(--border);
    flex: none;
    background: var(--bg-sunken);
  }

  .ss-pips {
    display: flex;
    align-items: center;
    gap: 6px;
    flex-wrap: wrap;
    justify-content: center;
    max-width: 400px;
  }

  .ss-pip {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    border: none;
    background: var(--surface-3);
    cursor: pointer;
    padding: 0;
    transition: background var(--dur-fast), transform var(--dur-fast);
  }

  .ss-pip:hover { background: var(--fg-faint); transform: scale(1.2); }
  .ss-pip.on { background: var(--accent); transform: scale(1.25); }
</style>
