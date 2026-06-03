<script lang="ts">
  import * as mock from "../lib/mock";
  import Icon from "../components/Icon.svelte";

  let { onExit, questions: questionsProp }: { onExit?: () => void; questions?: { q: string; options: string[]; answer: number; explain: string }[] } = $props();

  const qs = $derived(questionsProp && questionsProp.length > 0 ? questionsProp : mock.quiz);

  let i      = $state(0);
  let picked = $state<number | null>(null);
  let score  = $state(0);
  let done   = $state(false);

  function choose(idx: number) {
    if (picked !== null) return;
    picked = idx;
    if (idx === qs[i].answer) score += 1;
  }

  function next() {
    if (i + 1 >= qs.length) { done = true; return; }
    i += 1;
    picked = null;
  }

  function restart() {
    i = 0; picked = null; score = 0; done = false;
  }
</script>

<div class="fc-wrap">
  {#if done}
    <div class="fc-done">
      <div class="fc-done-glyph">
        <Icon name="check" size={22} color="var(--ok)" />
      </div>
      <h2 class="read">{score} / {qs.length} correct</h2>
      <p class="mono muted">{score === qs.length ? "Flawless — nice." : "Review the misses, then retry."}</p>
      <div class="row gap-2" style="justify-content: center">
        <button class="btn btn--primary" onclick={restart}>Retry</button>
        {#if onExit}
          <button class="btn" onclick={onExit}>
            <span style="display:inline-flex;transform:rotate(180deg)"><Icon name="chevron" size={12} /></span> Materials
          </button>
        {/if}
      </div>
    </div>
  {:else}
    {@const q = qs[i]}

    <div class="fc-bar-row">
      {#if onExit}
        <button class="btn btn--icon btn--sm btn--ghost" onclick={onExit} title="Back to materials">
          <span style="display:inline-flex;transform:rotate(180deg)"><Icon name="chevron" size={13} /></span>
        </button>
      {/if}
      <div class="fc-progress">
        <div class="fc-bar" style:width="{(i / qs.length * 100)}%"></div>
      </div>
    </div>

    <div class="fc-meta mono">
      <span>Question {i + 1} / {qs.length}</span>
      <span>Recursion · multiple choice</span>
    </div>

    <div class="quiz-card">
      <p class="quiz-q read">{q.q}</p>

      <div class="quiz-opts">
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
            {i + 1 >= qs.length ? "Finish" : "Next"}
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
