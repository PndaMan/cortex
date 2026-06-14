<script lang="ts">
  import * as mock from "../lib/mock";
  import * as api from "../lib/api";
  import { app } from "../lib/store.svelte";
  import Icon from "../components/Icon.svelte";
  import RichText from "../components/RichText.svelte";

  let { onExit, deck: deckProp }: { onExit?: () => void; deck?: { q: string; a: string }[] } = $props();

  const deck = $derived(deckProp && deckProp.length > 0 ? deckProp : mock.flashcards);
  // `q` is the SM-2 quality grade (0-5) sent to the scheduler.
  const RATINGS = [
    { id: "again", label: "Again", key: "1", cls: "again", q: 1 },
    { id: "hard",  label: "Hard",  key: "2", cls: "hard",  q: 3 },
    { id: "good",  label: "Good",  key: "3", cls: "good",  q: 4 },
    { id: "easy",  label: "Easy",  key: "4", cls: "easy",  q: 5 },
  ] as const;

  let i       = $state(0);
  let flipped = $state(false);
  let done    = $state(false);
  let rated   = $state(0);
  let glow    = $state<string | null>(null);

  // Indices (into the active deck) the user rated "Again" this session.
  // Reset whenever a new session starts so the review list stays per-session.
  let missed  = $state<number[]>([]);
  const missedCards = $derived(
    missed.map((idx) => activeDeck[idx]).filter((c): c is { q: string; a: string } => !!c)
  );

  // Review mode: null = normal deck, string[] = only fronts in this list
  let reviewKeys  = $state<string[] | null>(null);
  // Cards due now per the SM-2 schedule (for the "Study due" affordance).
  let dueCount    = $state(0);
  // Active deck: review subset or full deck
  let activeDeck = $derived(
    reviewKeys
      ? reviewKeys.map((key) => deck.find((c) => c.q === key) ?? { q: key, a: "" })
      : deck
  );

  // Next-interval preview per grade [again, hard, good, easy] for the current
  // card, so each rating button shows what it'll schedule (Anki-style). Fetched
  // whenever the card changes; null while loading / when no subject is active.
  let intervals = $state<number[] | null>(null);
  $effect(() => {
    const sid = app.activeSubjectId;
    const card = activeDeck[i];
    if (!sid || !card) { intervals = null; return; }
    let cancelled = false;
    api.srsPreview(sid, "flashcard", card.q)
      .then((v) => { if (!cancelled) intervals = v; })
      .catch(() => { if (!cancelled) intervals = null; });
    return () => { cancelled = true; };
  });
  function fmtInterval(d: number): string {
    if (d <= 0) return "<1d";
    if (d < 7) return `${d}d`;
    if (d < 30) return `${Math.round(d / 7)}w`;
    if (d < 365) return `${Math.round(d / 30)}mo`;
    return `${(d / 365).toFixed(d < 730 ? 1 : 0)}y`;
  }

  function rate(cls: string) {
    if (glow) return;
    const quality = RATINGS.find((r) => r.cls === cls)?.q ?? 4;
    const sid = app.activeSubjectId;
    if (sid) {
      // SM-2 grade (also logs the attempt for the "review missed" set).
      api.srsGrade(sid, "flashcard", i, activeDeck[i].q, quality).catch((e: unknown) => {
        app.pushToast({ kind: "error", title: "Record failed", body: String(e) });
      });
    }
    // "Again" (the first rate button) counts as missed for this session.
    if (cls === "again" && !missed.includes(i)) missed = [...missed, i];
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
    i = 0; done = false; flipped = false; rated = 0; glow = null; reviewKeys = null; missed = [];
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
      i = 0; done = false; flipped = false; rated = 0; glow = null; missed = [];
    } catch (e) {
      app.pushToast({ kind: "error", title: "Review load failed", body: String(e) });
    }
  }

  // SM-2: study only the cards that are due now (by their scheduled due date).
  async function startDue() {
    const sid = app.activeSubjectId;
    if (!sid) { app.pushToast({ kind: "warning", title: "No subject selected" }); return; }
    try {
      const due = await api.srsDue(sid, "flashcard");
      if (due.length === 0) {
        app.pushToast({ kind: "success", title: "Nothing due — you're all caught up 🎉" });
        return;
      }
      reviewKeys = due.map((d) => d.item_key);
      i = 0; done = false; flipped = false; rated = 0; glow = null; missed = [];
    } catch (e) {
      app.pushToast({ kind: "error", title: "Due load failed", body: String(e) });
    }
  }

  async function refreshDue() {
    const sid = app.activeSubjectId;
    if (!sid) return;
    try {
      dueCount = (await api.srsStats(sid, "flashcard")).due;
    } catch { /* non-fatal */ }
  }
  // Re-pull the due count on mount, when the subject changes, and whenever a
  // session ends or its mode flips (grading shifts cards' due dates).
  $effect(() => { void done; void reviewKeys; void app.activeSubjectId; refreshDue(); });

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
      <p class="mono muted">
        {activeDeck.length} cards graded · spaced-repetition schedule updated.
      </p>
      <div class="row gap-2" style="justify-content: center">
        <button class="btn btn--primary" onclick={restart}>Study again</button>
        {#if dueCount > 0}
          <button class="btn" onclick={startDue}>Study due · {dueCount}</button>
        {/if}
        <button class="btn" onclick={startReview}>Review missed</button>
        {#if onExit}
          <button class="btn" onclick={onExit}>
            <span style="display:inline-flex;transform:rotate(180deg)"><Icon name="chevron" size={12} /></span> Materials
          </button>
        {/if}
      </div>

      <div class="fc-review">
        <div class="fc-review-head mono">
          <span>Cards you missed</span>
          {#if missedCards.length > 0}
            <span class="badge">{missedCards.length}</span>
          {/if}
        </div>
        {#if missedCards.length === 0}
          <p class="mono muted fc-review-empty">No cards missed this session 🎉</p>
        {:else}
          <ul class="fc-review-list">
            {#each missedCards as card, idx (idx)}
              <li class="fc-review-item">
                <div class="fc-review-q read"><RichText text={card.q} /></div>
                <div class="fc-review-a-label mono">ANSWER</div>
                <div class="fc-review-a read"><RichText text={card.a} /></div>
              </li>
            {/each}
          </ul>
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
        {#if dueCount > 0}
          <button class="btn btn--sm btn--primary" onclick={startDue} title="Study the cards scheduled as due today">
            Study due · {dueCount}
          </button>
        {/if}
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
        <div class="read fc-text"><RichText text={activeDeck[i].q} /></div>
        <div class="fc-hint mono">click or <span class="kbd">␣</span> to flip</div>
      </div>
      <div class="fc-face fc-back">
        <div class="fc-side mono">ANSWER</div>
        <div class="read fc-text"><RichText text={activeDeck[i].a} /></div>
      </div>
    </div>

    <div class="fc-rate{flipped ? ' show' : ''}">
      {#each RATINGS as r, idx (r.id)}
        <button
          class="btn rate-{r.cls}{glow === r.cls ? ' is-picked' : ''}"
          onclick={() => rate(r.cls)}
        >
          <span class="rate-top">{r.label} <span class="kbd">{r.key}</span></span>
          {#if intervals && intervals[idx] != null}<span class="rate-ivl">{fmtInterval(intervals[idx])}</span>{/if}
        </button>
      {/each}
    </div>
  {/if}
</div>

<style>
  /* In-session "missed" review list on the completion screen. Tokens only. */
  .fc-review {
    width: 100%;
    max-width: 520px;
    margin-top: var(--sp-4);
    text-align: left;
  }
  .fc-review-head {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    font-size: var(--t-2xs);
    letter-spacing: 0.16em;
    text-transform: uppercase;
    color: var(--fg-faint);
    margin-bottom: var(--sp-3);
  }
  .fc-review-empty {
    margin: 0;
    font-size: var(--t-xs);
  }
  .fc-review-list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
    max-height: 320px;
    overflow-y: auto;
  }
  .fc-review-item {
    background: var(--surface);
    border: 1px solid var(--border);
    border-left: 2px solid var(--err);
    border-radius: var(--rad-3);
    padding: var(--sp-3) var(--sp-4);
  }
  .fc-review-q {
    font-size: var(--r-md);
    line-height: 1.4;
    color: var(--fg-bright);
    font-weight: 500;
  }
  .fc-review-a-label {
    font-size: var(--t-2xs);
    letter-spacing: 0.16em;
    color: var(--fg-faint);
    margin: var(--sp-3) 0 var(--sp-1);
  }
  .fc-review-a {
    font-size: var(--r-sm);
    line-height: 1.45;
    color: var(--ok);
  }
  /* RichText emits .rt-* nodes; keep spacing tight inside review cells. */
  .fc-review-q :global(.rt-p),
  .fc-review-a :global(.rt-p) { margin: 0 0 var(--sp-1); }
  .fc-review-q :global(.rt-p:last-child),
  .fc-review-a :global(.rt-p:last-child) { margin-bottom: 0; }
</style>
