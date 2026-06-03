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

  // Review mode: null = normal quiz, string[] = only these question texts
  let reviewKeys  = $state<string[] | null>(null);
  // Active question list: review subset or full quiz
  const activeQs = $derived(
    reviewKeys
      ? reviewKeys.map((key) => qs.find((q) => q.q === key) ?? { q: key, options: [], answer: -1, explain: "" })
      : qs
  );

  function choose(idx: number) {
    if (picked !== null) return;
    picked = idx;
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
    i = 0; picked = null; score = 0; done = false; reviewKeys = null;
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
      i = 0; picked = null; score = 0; done = false;
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
