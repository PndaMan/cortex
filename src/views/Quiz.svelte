<script lang="ts">
  import * as mock from "../lib/mock";
  import * as api from "../lib/api";
  import { app } from "../lib/store.svelte";
  import Icon from "../components/Icon.svelte";

  let { onExit, questions: questionsProp }: { onExit?: () => void; questions?: { q: string; options: string[]; answer: number; explain: string }[] } = $props();

  const qs = $derived(questionsProp && questionsProp.length > 0 ? questionsProp : mock.quiz);

  let i           = $state(0);
  let picked      = $state<number | null>(null);
  let score       = $state(0);
  let done        = $state(false);
  // Record of the option index the user picked, keyed by question index in activeQs.
  let answers     = $state<Record<number, number>>({});

  // Review mode: null = normal quiz, string[] = only these question texts
  let reviewKeys  = $state<string[] | null>(null);
  // Active question list: review subset or full quiz
  const activeQs = $derived(
    reviewKeys
      ? reviewKeys.map((key) => qs.find((q) => q.q === key) ?? { q: key, options: [], answer: -1, explain: "" })
      : qs
  );

  // Questions the user got wrong (or didn't answer), for the review summary.
  const wrongQs = $derived(
    activeQs.filter((q, idx) => answers[idx] === undefined || answers[idx] !== q.answer)
  );

  function choose(idx: number) {
    if (picked !== null) return;
    picked = idx;
    answers[i] = idx;
    const isCorrect = idx === activeQs[i].answer;
    if (isCorrect) score += 1;
    // Record attempt (fire-and-forget; skip if no active subject)
    const sid = app.activeSubjectId;
    if (sid) {
      api.recordAttempt(sid, "quiz", i, activeQs[i].q, isCorrect).catch((e: unknown) => {
        app.pushToast({ kind: "error", title: "Record failed", body: String(e) });
      });
    }
  }

  function next() {
    if (i + 1 >= activeQs.length) { done = true; return; }
    i += 1;
    picked = null;
  }

  function restart() {
    i = 0; picked = null; score = 0; done = false; reviewKeys = null; answers = {};
  }

  async function startReview() {
    const sid = app.activeSubjectId;
    if (!sid) { app.pushToast({ kind: "warning", title: "No subject selected" }); return; }
    try {
      const wrong = await api.reviewSet(sid, "quiz");
      if (wrong.length === 0) {
        app.pushToast({ kind: "success", title: "No wrong answers to review 🎉" });
        return;
      }
      reviewKeys = wrong.map((w) => w.item_key);
      i = 0; picked = null; score = 0; done = false; answers = {};
    } catch (e) {
      app.pushToast({ kind: "error", title: "Review load failed", body: String(e) });
    }
  }
</script>

<div class="fc-wrap">
  {#if done}
    <div class="fc-done">
      <div class="fc-done-glyph">
        <Icon name="check" size={22} color="var(--ok)" />
      </div>
      <h2 class="read">{score} / {activeQs.length} correct</h2>
      <p class="mono muted">{score === activeQs.length ? "Flawless — nice." : "Review the misses, then retry."}</p>
      <div class="row gap-2" style="justify-content: center">
        <button class="btn btn--primary" onclick={restart}>Retry</button>
        <button class="btn" onclick={startReview}>Review wrong answers</button>
        {#if onExit}
          <button class="btn" onclick={onExit}>
            <span style="display:inline-flex;transform:rotate(180deg)"><Icon name="chevron" size={12} /></span> Materials
          </button>
        {/if}
      </div>

      <div class="qz-review">
        <div class="qz-revise">
          <p class="mono qz-revise-head">What to revise</p>
          {#if wrongQs.length === 0}
            <p class="read qz-revise-clear">Nothing to revise — you nailed every question.</p>
          {:else}
            <ul class="qz-revise-list">
              {#each wrongQs as wq (wq.q)}
                <li class="read">{wq.q}</li>
              {/each}
            </ul>
          {/if}
        </div>

        <div class="qz-review-list">
          {#each activeQs as q, idx (idx)}
            {@const userPick = answers[idx]}
            {@const gotIt    = userPick !== undefined && userPick === q.answer}
            <div class="qz-review-card">
              <div class="qz-review-q">
                <span class="quiz-key mono">{idx + 1}</span>
                <p class="read">{q.q}</p>
                <span class="qz-review-mark mono" class:ok={gotIt} class:err={!gotIt}>
                  {gotIt ? "Correct" : "Revise"}
                </span>
              </div>

              {#if q.options.length === 0}
                <p class="mono muted qz-review-empty">
                  (This question is from a previous quiz — retake it to answer again.)
                </p>
              {:else}
                <div class="qz-review-opts">
                  {#each q.options as opt, oi (oi)}
                    {@const isAnswer = oi === q.answer}
                    {@const isPick   = userPick === oi}
                    <div
                      class="qz-review-opt"
                      class:correct={isAnswer}
                      class:wrong={isPick && !isAnswer}
                    >
                      <span class="quiz-key mono">{String.fromCharCode(65 + oi)}</span>
                      <span class="read">{opt}</span>
                      <span class="qz-review-tags">
                        {#if isPick}<span class="badge qz-tag-you">Your answer</span>{/if}
                        {#if isAnswer}<span class="badge qz-tag-correct">Correct</span>{/if}
                      </span>
                    </div>
                  {/each}
                </div>
              {/if}

              {#if q.explain}
                <p class="mono muted qz-review-explain">{q.explain}</p>
              {/if}
            </div>
          {/each}
        </div>
      </div>
    </div>
  {:else}
    {@const q = activeQs[i]}

    <div class="fc-bar-row">
      {#if onExit}
        <button class="btn btn--icon btn--sm btn--ghost" onclick={onExit} title="Back to materials">
          <span style="display:inline-flex;transform:rotate(180deg)"><Icon name="chevron" size={13} /></span>
        </button>
      {/if}
      <div class="fc-progress">
        <div class="fc-bar" style:width="{(i / activeQs.length * 100)}%"></div>
      </div>
      {#if !done && !reviewKeys}
        <button class="btn btn--sm" onclick={startReview} title="Review previously wrong answers">
          Review wrong answers
        </button>
      {/if}
    </div>

    <div class="fc-meta mono">
      <span>{reviewKeys ? "Review" : "Question"} {i + 1} / {activeQs.length}</span>
      <span>Recursion · multiple choice</span>
    </div>

    <div class="quiz-card">
      <p class="quiz-q read">{q.q}</p>

      <div class="quiz-opts">
        {#if q.options.length === 0}
          <p class="mono muted" style="font-size: var(--t-sm); padding: 8px 0;">
            (This question is from a previous quiz — retake that quiz to answer it again.)
          </p>
        {:else}
          {#each q.options as opt, idx (idx)}
            {@const isCorrect = idx === q.answer}
            {@const isPicked  = picked !== null && idx === picked}
            {@const isDim     = picked !== null && !isCorrect && idx !== picked}
            <button
              class="quiz-opt{picked !== null && isCorrect ? ' correct' : ''}{isPicked && !isCorrect ? ' wrong' : ''}{isDim ? ' dim' : ''}"
              onclick={() => choose(idx)}
            >
              <span class="quiz-key mono">{String.fromCharCode(65 + idx)}</span>
              <span class="read">{opt}</span>
              {#if picked !== null && isCorrect}
                <Icon name="check" size={14} color="var(--ok)" />
              {:else if isPicked && !isCorrect}
                <Icon name="x" size={12} color="var(--err)" />
              {/if}
            </button>
          {/each}
        {/if}
      </div>

      {#if picked !== null}
        <div class="quiz-next">
          <span
            class="mono"
            style:color={picked === q.answer ? "var(--ok)" : "var(--err)"}
          >
            {picked === q.answer ? "Correct" : "Not quite"}
          </span>
          <button class="btn btn--sm btn--primary" onclick={next}>
            {i + 1 >= activeQs.length ? "Finish" : "Next"}
            <Icon name="arrowR" size={12} />
          </button>
        </div>

        {#if q.explain}
          <p class="mono muted" style="font-size: var(--t-xs); margin-top: -4px;">{q.explain}</p>
        {/if}
      {/if}
    </div>
  {/if}
</div>

<style>
  /* When the done screen carries a review, let the wrap top-align and the
     review list own the scroll so the page never overflows the viewport. */
  .fc-wrap:has(.qz-review) { justify-content: flex-start; min-height: 0; }
  .fc-wrap:has(.qz-review) .fc-done { width: 100%; min-height: 0; }

  .qz-review {
    width: 100%;
    text-align: left;
    display: flex;
    flex-direction: column;
    gap: var(--sp-4);
    margin-top: var(--sp-4);
    min-height: 0;
  }

  .qz-revise {
    background: var(--surface);
    border: 1px solid var(--border-strong);
    border-radius: var(--rad-4);
    padding: var(--sp-5);
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
  }
  .qz-revise-head {
    font-size: var(--t-xs);
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--fg-faint);
  }
  .qz-revise-clear { font-size: var(--r-md); color: var(--ok); }
  .qz-revise-list { display: flex; flex-direction: column; gap: var(--sp-1); padding-left: 1.1em; }
  .qz-revise-list li { font-size: var(--r-sm); color: var(--fg); list-style: disc; }

  .qz-review-list {
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
    overflow-y: auto;
    min-height: 0;
    max-height: 60vh;
    padding-right: var(--sp-1);
  }

  .qz-review-card {
    background: var(--surface);
    border: 1px solid var(--border-strong);
    border-radius: var(--rad-4);
    padding: var(--sp-5);
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
  }
  .qz-review-q { display: flex; align-items: baseline; gap: var(--sp-3); }
  .qz-review-q .read { flex: 1; font-size: var(--r-md); color: var(--fg-bright); line-height: 1.35; }
  .qz-review-mark {
    flex: none;
    font-size: var(--t-2xs);
    text-transform: uppercase;
    letter-spacing: 0.06em;
  }
  .qz-review-mark.ok  { color: var(--ok); }
  .qz-review-mark.err { color: var(--err); }

  .qz-review-opts { display: flex; flex-direction: column; gap: 7px; }
  .qz-review-opt {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    padding: 10px 12px;
    background: var(--surface-2);
    border: 1px solid var(--border-strong);
    border-radius: var(--rad-3);
    color: var(--fg);
  }
  .qz-review-opt .read { flex: 1; font-size: var(--r-sm); }
  .qz-review-opt.correct {
    border-color: var(--ok);
    background: color-mix(in oklab, var(--ok) 14%, var(--surface));
  }
  .qz-review-opt.correct .quiz-key { border-color: var(--ok); color: var(--ok); }
  .qz-review-opt.wrong {
    border-color: var(--err);
    background: color-mix(in oklab, var(--err) 12%, var(--surface));
  }
  .qz-review-opt.wrong .quiz-key { border-color: var(--err); color: var(--err); }

  .qz-review-tags { flex: none; display: inline-flex; gap: var(--sp-1); }
  .qz-tag-you {
    border-color: color-mix(in oklab, var(--err) 45%, transparent);
    background: color-mix(in oklab, var(--err) 14%, transparent);
    color: var(--err);
  }
  .qz-tag-correct {
    border-color: color-mix(in oklab, var(--ok) 45%, transparent);
    background: color-mix(in oklab, var(--ok) 14%, transparent);
    color: var(--ok);
  }

  .qz-review-empty { font-size: var(--t-sm); }
  .qz-review-explain { font-size: var(--t-xs); line-height: 1.5; }
</style>
