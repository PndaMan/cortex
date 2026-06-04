<script lang="ts">
  import * as mock from "../lib/mock";
  import * as api from "../lib/api";
  import { app } from "../lib/store.svelte";
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

  // Review mode: null = normal deck, string[] = only fronts in this list
  let reviewKeys  = $state<string[] | null>(null);
  // Active deck: review subset or full deck
  let activeDeck = $derived(
    reviewKeys
      ? reviewKeys.map((key) => deck.find((c) => c.q === key) ?? { q: key, a: "" })
      : deck
  );

  function rate(cls: string) {
    if (glow) return;
    // "good" / "easy" = got it; "again" / "hard" = missed
    const gotIt = cls === "good" || cls === "easy";
    const sid = app.activeSubjectId;
    if (sid) {
      api.recordAttempt(sid, "flashcard", i, activeDeck[i].q, gotIt).catch((e: unknown) => {
        app.pushToast({ kind: "error", title: "Record failed", body: String(e) });
      });
    }
    glow = cls;
    setTimeout(() => {
      glow = null;
      if (i + 1 >= activeDeck.length) { done = true; return; }
      rated += 1;
      i += 1;
      flipped = false;
    }, 460);
  }

  function restart() {
    i = 0; done = false; flipped = false; rated = 0; glow = null; reviewKeys = null;
  }

  async function startReview() {
    const sid = app.activeSubjectId;
    if (!sid) { app.pushToast({ kind: "warning", title: "No subject selected" }); return; }
    try {
      const wrong = await api.reviewSet(sid, "flashcard");
      if (wrong.length === 0) {
        app.pushToast({ kind: "success", title: "No missed cards to review 🎉" });
        return;
      }
      reviewKeys = wrong.map((w) => w.item_key);
      i = 0; done = false; flipped = false; rated = 0; glow = null;
    } catch (e) {
      app.pushToast({ kind: "error", title: "Review load failed", body: String(e) });
    }
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
      const el = document.activeElement as HTMLElement | null;
      if (el && (el.tagName === "INPUT" || el.tagName === "TEXTAREA")) return;
      // Esc always exits the session (back to materials), even on the done screen.
      if (e.key === "Escape") { e.preventDefault(); onExit?.(); return; }
      if (cur_done) return;
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
      <p class="mono muted">{activeDeck.length} cards · come back tomorrow — 3 are due then.</p>
      <div class="row gap-2" style="justify-content: center">
        <button class="btn btn--primary" onclick={restart}>Study again</button>
        <button class="btn" onclick={startReview}>Review missed</button>
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
        <div class="fc-bar" style:width="{(i / activeDeck.length * 100)}%"></div>
      </div>
      {#if !reviewKeys}
        <button class="btn btn--sm" onclick={startReview} title="Review previously missed cards">
          Review missed
        </button>
      {/if}
    </div>

    <div class="fc-meta mono">
      <span>{reviewKeys ? "Review" : "Card"} {i + 1} / {activeDeck.length}</span>
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
        <p class="read">{activeDeck[i].q}</p>
        <div class="fc-hint mono">click or <span class="kbd">␣</span> to flip</div>
      </div>
      <div class="fc-face fc-back">
        <div class="fc-side mono">ANSWER</div>
        <p class="read">{activeDeck[i].a}</p>
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
