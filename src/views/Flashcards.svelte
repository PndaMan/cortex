<script lang="ts">
  import * as mock from "../lib/mock";
  import Icon from "../components/Icon.svelte";

  let { onExit, deck: deckProp }: { onExit?: () => void; deck?: { q: string; a: string }[] } = $props();

  const deck = $derived(deckProp && deckProp.length > 0 ? deckProp : mock.flashcards);
  const RATINGS = [
    { id: "again", label: "Again", key: "1", cls: "again" },
    { id: "hard",  label: "Hard",  key: "2", cls: "hard"  },
    { id: "good",  label: "Good",  key: "3", cls: "good"  },
    { id: "easy",  label: "Easy",  key: "4", cls: "easy"  },
  ] as const;

  let i       = $state(0);
  let flipped = $state(false);
  let done    = $state(false);
  let rated   = $state(0);
  let glow    = $state<string | null>(null);

  function rate(cls: string) {
    if (glow) return;
    glow = cls;
    setTimeout(() => {
      glow = null;
      if (i + 1 >= deck.length) { done = true; return; }
      rated += 1;
      i += 1;
      flipped = false;
    }, 460);
  }

  function restart() {
    i = 0; done = false; flipped = false; rated = 0; glow = null;
  }

  // Claim the keyboard while the session is mounted so the global Helix engine
  // (space-leader, etc.) stays out of the way. Runs once on mount.
  $effect(() => {
    window.__cortexModalOpen = true;
    return () => { window.__cortexModalOpen = false; };
  });

  $effect(() => {
    // Read reactive values so this effect re-runs when they change.
    // Local snapshot used by the keydown closure to avoid stale state.
    const cur_flipped = flipped;
    const cur_glow = glow;
    const cur_done = done;
    void i; // read i so effect re-runs on card advance

    function onKey(e: KeyboardEvent) {
      if (cur_done) return;
      const el = document.activeElement as HTMLElement | null;
      if (el && (el.tagName === "INPUT" || el.tagName === "TEXTAREA")) return;
      if (e.key === " ") {
        e.preventDefault();
        flipped = !flipped;
        return;
      }
      if (cur_flipped && !cur_glow) {
        const r = RATINGS.find(x => x.key === e.key);
        if (r) { e.preventDefault(); rate(r.cls); }
      }
    }

    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });
</script>

<div class="fc-wrap">
  {#if done}
    <div class="fc-done">
      <div class="fc-done-glyph">
        <Icon name="check" size={22} color="var(--ok)" />
      </div>
      <h2 class="read">Deck complete</h2>
      <p class="mono muted">{deck.length} cards · come back tomorrow — 3 are due then.</p>
      <div class="row gap-2" style="justify-content: center">
        <button class="btn btn--primary" onclick={restart}>Study again</button>
        {#if onExit}
          <button class="btn" onclick={onExit}>
            <span style="display:inline-flex;transform:rotate(180deg)"><Icon name="chevron" size={12} /></span> Materials
          </button>
        {/if}
      </div>
    </div>
  {:else}
    <div class="fc-bar-row">
      {#if onExit}
        <button class="btn btn--icon btn--sm btn--ghost" onclick={onExit} title="Back to materials">
          <span style="display:inline-flex;transform:rotate(180deg)"><Icon name="chevron" size={13} /></span>
        </button>
      {/if}
      <div class="fc-progress">
        <div class="fc-bar" style:width="{(i / deck.length * 100)}%"></div>
      </div>
    </div>

    <div class="fc-meta mono">
      <span>Card {i + 1} / {deck.length}</span>
      <span>Recursion · SRS</span>
    </div>

    <div
      class="flashcard{flipped ? ' flipped' : ''}{glow ? ' glow-' + glow : ''}"
      onclick={() => { if (!glow) flipped = !flipped; }}
      role="button"
      tabindex="0"
      onkeydown={(e) => { if (e.key === "Enter" && !glow) flipped = !flipped; }}
    >
      <div class="fc-face fc-front">
        <div class="fc-side mono">QUESTION</div>
        <p class="read">{deck[i].q}</p>
        <div class="fc-hint mono">click or <span class="kbd">␣</span> to flip</div>
      </div>
      <div class="fc-face fc-back">
        <div class="fc-side mono">ANSWER</div>
        <p class="read">{deck[i].a}</p>
      </div>
    </div>

    <div class="fc-rate{flipped ? ' show' : ''}">
      {#each RATINGS as r (r.id)}
        <button
          class="btn rate-{r.cls}{glow === r.cls ? ' is-picked' : ''}"
          onclick={() => rate(r.cls)}
        >
          {r.label} <span class="kbd">{r.key}</span>
        </button>
      {/each}
    </div>
  {/if}
</div>
