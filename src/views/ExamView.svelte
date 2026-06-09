<script lang="ts">
  // Exam Mode: generate a timed MCQ+written exam scoped to a subject's topics,
  // take it under a countdown, then review the locally-graded results. Mirrors the
  // generation flow of GenerateMaterial and the run/review shape of Quiz, using
  // only existing design-system tokens/classes.
  import { app } from "../lib/store.svelte";
  import * as api from "../lib/api";
  import Icon from "../components/Icon.svelte";

  type Screen = "setup" | "generating" | "run" | "results";

  // ── setup state ──────────────────────────────────────────────
  const topics = $derived(app.activeSubject?.topics ?? []);
  let selTopics = $state<string[]>([]);
  const DURATIONS = [15, 30, 60, 90] as const;
  let duration = $state<number>(30);
  let mcqCount = $state(8);
  let writtenCount = $state(2);

  let screen = $state<Screen>("setup");
  let genError = $state<string | null>(null);
  let exam = $state<api.ExamRec | null>(null);

  // ── past exams ───────────────────────────────────────────────
  let pastExams = $state<api.ExamRec[]>([]);
  async function loadPast() {
    const sid = app.activeSubjectId;
    if (!sid) { pastExams = []; return; }
    try { pastExams = await api.listExams(sid); } catch { pastExams = []; }
  }
  $effect(() => { void app.activeSubjectId; loadPast(); });

  function toggleTopic(id: string) {
    selTopics = selTopics.includes(id) ? selTopics.filter((t) => t !== id) : [...selTopics, id];
  }
  function clampMcq(n: number) { mcqCount = Math.max(0, Math.min(30, Math.round(n) || 0)); }
  function clampWritten(n: number) { writtenCount = Math.max(0, Math.min(15, Math.round(n) || 0)); }

  const canStart = $derived(!!app.activeSubject && mcqCount + writtenCount > 0);

  // ── generate + start ─────────────────────────────────────────
  async function startNew() {
    const sub = app.activeSubject;
    if (!sub) { app.pushToast({ kind: "error", title: "No active subject", body: "Open a subject first." }); return; }
    if (mcqCount + writtenCount === 0) return;
    screen = "generating";
    genError = null;
    try {
      const e = await api.generateExam(
        sub.id,
        selTopics.length ? selTopics : undefined,
        duration,
        mcqCount,
        writtenCount,
      );
      const started = await api.startExam(e.id);
      exam = started;
      beginRun(started);
      void loadPast();
    } catch (err) {
      genError = err instanceof Error ? err.message : String(err);
      screen = "setup";
      app.pushToast({ kind: "error", title: "Couldn't generate exam", body: genError });
    }
  }

  // ── run state ────────────────────────────────────────────────
  let questions = $state<any[]>([]);
  // answers keyed by question id: { choice?: number; text?: string }
  let runAnswers = $state<Record<string, { choice?: number; text?: string }>>({});
  let remainingMs = $state(0);
  let submitting = $state(false);
  let results = $state<any | null>(null);
  let timer: ReturnType<typeof setInterval> | null = null;
  let submitted = false; // guards against double auto/manual submit

  function beginRun(e: api.ExamRec) {
    questions = Array.isArray(e.questions) ? e.questions : [];
    runAnswers = {};
    results = null;
    submitted = false;
    const startMs = e.started_ms ?? Date.now();
    const endMs = startMs + e.duration_min * 60_000;
    remainingMs = Math.max(0, endMs - Date.now());
    screen = "run";
    startTimer(endMs);
  }

  function startTimer(endMs: number) {
    stopTimer();
    timer = setInterval(() => {
      remainingMs = Math.max(0, endMs - Date.now());
      if (remainingMs <= 0) {
        stopTimer();
        void doSubmit(true);
      }
    }, 250);
  }
  function stopTimer() {
    if (timer) { clearInterval(timer); timer = null; }
  }

  const mmss = $derived.by(() => {
    const s = Math.max(0, Math.ceil(remainingMs / 1000));
    const m = Math.floor(s / 60);
    return `${String(m).padStart(2, "0")}:${String(s % 60).padStart(2, "0")}`;
  });
  // Warn under 2 minutes, escalate to error under 1 minute.
  const warnTime = $derived(remainingMs > 0 && remainingMs < 120_000);
  const lowTime = $derived(remainingMs > 0 && remainingMs < 60_000);

  function pick(qid: string, idx: number) {
    runAnswers = { ...runAnswers, [qid]: { ...runAnswers[qid], choice: idx } };
  }
  function write(qid: string, text: string) {
    runAnswers = { ...runAnswers, [qid]: { ...runAnswers[qid], text } };
  }

  async function confirmSubmit() {
    const ok = await app.confirm({
      title: "Submit exam?",
      body: "Your answers will be graded and the exam locked.",
      okLabel: "Submit",
    });
    if (ok) void doSubmit(false);
  }

  async function doSubmit(auto: boolean) {
    if (submitted || !exam) return;
    submitted = true;
    stopTimer();
    submitting = true;
    if (auto) app.pushToast({ kind: "info", title: "Time's up", body: "Submitting your exam…" });
    const payload: api.ExamAnswerInput[] = questions.map((q) => ({
      id: q.id,
      choice: runAnswers[q.id]?.choice ?? null,
      text: runAnswers[q.id]?.text ?? null,
    }));
    try {
      results = await api.submitExam(exam.id, payload);
      screen = "results";
      void loadPast();
    } catch (err) {
      submitted = false; // allow a retry on failure
      app.pushToast({ kind: "error", title: "Submit failed", body: err instanceof Error ? err.message : String(err) });
    } finally {
      submitting = false;
    }
  }

  // ── open a past exam ─────────────────────────────────────────
  async function openPast(e: api.ExamRec) {
    if (e.status === "graded") {
      try {
        const full = await api.getExam(e.id);
        exam = full;
        questions = Array.isArray(full.questions) ? full.questions : [];
        results = full.results;
        screen = "results";
      } catch (err) {
        app.pushToast({ kind: "error", title: "Couldn't open exam", body: String(err) });
      }
    } else {
      // ready / in_progress → (re)start it
      try {
        const started = await api.startExam(e.id);
        exam = started;
        beginRun(started);
      } catch (err) {
        app.pushToast({ kind: "error", title: "Couldn't open exam", body: String(err) });
      }
    }
  }

  async function deletePast(e: api.ExamRec, ev: MouseEvent) {
    ev.stopPropagation();
    const ok = await app.confirm({ title: "Delete exam?", body: e.title, danger: true, okLabel: "Delete" });
    if (!ok) return;
    try { await api.deleteExam(e.id); await loadPast(); } catch (err) {
      app.pushToast({ kind: "error", title: "Delete failed", body: String(err) });
    }
  }

  function backToSetup() {
    stopTimer();
    exam = null;
    results = null;
    screen = "setup";
    void loadPast();
  }

  // ── results helpers ──────────────────────────────────────────
  const reviewItems = $derived.by(() => {
    const rq: any[] = results?.questions ?? [];
    return questions.map((q) => ({ q, r: rq.find((x) => x.id === q.id) }));
  });
  // Weakest topics: those the exam covered, surfaced as a callout. The backend
  // returns the scoped topic names; with per-question topic data absent we list
  // them so the user knows what to revise after a low score.
  const weakTopics = $derived.by(() => {
    const ts: any[] = results?.topics ?? [];
    return ts.map((t) => t.topic).filter(Boolean);
  });
  const scorePct = $derived(results?.score ?? exam?.score ?? 0);

  // ── navigation block while running (mirror Flashcards/Quiz) ──
  $effect(() => {
    const running = screen === "run";
    window.__cortexModalOpen = running;
    return () => { window.__cortexModalOpen = false; };
  });
  // Stop the timer if the view unmounts mid-exam.
  $effect(() => () => stopTimer());
</script>

<div class="workspace-scroll">
  {#if screen === "setup"}
    <!-- Setup mirrors a Settings tab: set-pane shell, set-head, set-groups + set-cards. -->
    <div class="set-pane">
      <header class="set-head">
        <div class="eyebrow">Exam mode</div>
        <h1 class="read set-title">Sit a timed exam</h1>
        <p class="set-sub">Generate a timed MCQ + written exam from {app.activeSubject?.name ?? "your subject"} and have it graded instantly.</p>
      </header>

      {#if !app.activeSubject}
        <div class="set-card">
          <div class="set-row"><div class="set-row-l"><div class="set-row-d">Open a subject to create an exam.</div></div></div>
        </div>
      {:else}
        <!-- Topics: small dashed tag-chip-add chips with a check when selected, like
             Settings → Profile → "Explain with". -->
        <section class="set-group">
          <div class="set-group-h">
            <h3 class="set-group-t">Topics</h3>
            <p class="set-group-d">Leave all unselected to cover the whole subject.</p>
          </div>
          <div class="set-card">
            <div class="set-row stacked">
              <div class="set-row-r">
                {#if topics.length === 0}
                  <div class="set-row-d">No topics yet — the exam will use all of this subject's sources.</div>
                {:else}
                  <div class="tag-suggest" style="margin-top:0">
                    {#each topics as t (t.id)}
                      <button
                        type="button"
                        class={"tag-chip-add" + (selTopics.includes(t.id) ? " on" : "")}
                        onclick={() => toggleTopic(t.id)}
                      >
                        {#if selTopics.includes(t.id)}<Icon name="check" size={9} />{/if}
                        {t.name}
                      </button>
                    {/each}
                  </div>
                {/if}
              </div>
            </div>
          </div>
        </section>

        <!-- Format: seg duration + two stepper rows, exactly the Settings pomodoro pattern. -->
        <section class="set-group">
          <div class="set-group-h"><h3 class="set-group-t">Format</h3></div>
          <div class="set-card">
            <div class="set-row">
              <div class="set-row-l">
                <div class="set-row-t">Duration</div>
                <div class="set-row-d">How long the countdown runs.</div>
              </div>
              <div class="set-row-r">
                <div class="seg">
                  {#each DURATIONS as d}
                    <button type="button" class={"seg-opt" + (duration === d ? " on" : "")} onclick={() => (duration = d)}>{d} min</button>
                  {/each}
                </div>
              </div>
            </div>

            <div class="set-row">
              <div class="set-row-l">
                <div class="set-row-t">Multiple choice</div>
                <div class="set-row-d">One mark each.</div>
              </div>
              <div class="set-row-r">
                <div class="exam-step">
                  <button class="btn btn--icon btn--sm" onclick={() => clampMcq(mcqCount - 1)} disabled={mcqCount <= 0} aria-label="fewer multiple choice questions">−</button>
                  <span class="mono exam-step-v">{mcqCount}</span>
                  <button class="btn btn--icon btn--sm" onclick={() => clampMcq(mcqCount + 1)} disabled={mcqCount >= 30} aria-label="more multiple choice questions">+</button>
                </div>
              </div>
            </div>

            <div class="set-row">
              <div class="set-row-l">
                <div class="set-row-t">Written</div>
                <div class="set-row-d">Two to five marks each, graded by the model.</div>
              </div>
              <div class="set-row-r">
                <div class="exam-step">
                  <button class="btn btn--icon btn--sm" onclick={() => clampWritten(writtenCount - 1)} disabled={writtenCount <= 0} aria-label="fewer written questions">−</button>
                  <span class="mono exam-step-v">{writtenCount}</span>
                  <button class="btn btn--icon btn--sm" onclick={() => clampWritten(writtenCount + 1)} disabled={writtenCount >= 15} aria-label="more written questions">+</button>
                </div>
              </div>
            </div>
          </div>
        </section>

        <!-- Primary action + summary, mirroring GenerateMaterial's footer. -->
        <div class="exam-cta">
          <button class="btn btn--primary" onclick={startNew} disabled={!canStart}>
            <Icon name="bolt" size={13} /> Generate &amp; start
          </button>
          <span class="mono faint">{mcqCount + writtenCount} questions · {duration} min</span>
        </div>

        {#if pastExams.length > 0}
          <section class="set-group">
            <div class="set-group-h"><h3 class="set-group-t">Past exams</h3></div>
            <div class="set-card">
              {#each pastExams as e (e.id)}
                <div
                  class="set-row exam-past"
                  role="button"
                  tabindex="0"
                  onclick={() => openPast(e)}
                  onkeydown={(ev) => { if (ev.key === "Enter") openPast(e); }}
                >
                  <div class="set-row-l">
                    <div class="set-row-t">{e.title}</div>
                    <div class="set-row-d">
                      {e.status === "graded" ? "Graded" : e.status === "in_progress" ? "In progress" : "Ready to start"}
                    </div>
                  </div>
                  <div class="set-row-r exam-past-r">
                    {#if e.status === "graded"}
                      <span class="mono exam-past-score">{Math.round(e.score ?? 0)}%</span>
                    {/if}
                    <span class="badge">{e.status === "graded" ? "graded" : e.status === "in_progress" ? "resume" : "start"}</span>
                    <button class="btn btn--icon btn--sm btn--ghost" title="Delete exam" aria-label="Delete exam" onclick={(ev) => deletePast(e, ev)}>
                      <Icon name="x" size={12} />
                    </button>
                  </div>
                </div>
              {/each}
            </div>
          </section>
        {/if}
      {/if}
    </div>

  {:else if screen === "generating"}
    <!-- Same visual language as GeneratingCard (gen-card + is-spin). -->
    <div class="set-pane exam-center">
      <div class="exam-gen-card">
        <span class="is-spin"></span>
        <div class="exam-gen-text">
          <span class="mono exam-gen-label">Writing your exam…</span>
          <span class="mono faint exam-gen-sub">{mcqCount} multiple-choice · {writtenCount} written</span>
        </div>
      </div>
    </div>

  {:else if screen === "run"}
    <div class="exam-run">
      <header class="exam-run-head">
        <div class="exam-run-title read">{exam?.title}</div>
        <div class="exam-timer mono" class:warn={warnTime && !lowTime} class:low={lowTime}>
          <Icon name="record" size={13} /> {mmss}
        </div>
      </header>

      <div class="exam-qs">
        {#each questions as q, qi (q.id)}
          <!-- Question card mirrors Quiz.svelte's quiz-card / quiz-opt markup. -->
          <div class="quiz-card">
            <div class="exam-q-head">
              <span class="quiz-key mono">{qi + 1}</span>
              <p class="quiz-q read">{q.q}</p>
              <span class="mono faint exam-q-marks">{q.marks} mark{q.marks === 1 ? "" : "s"}</span>
            </div>
            {#if q.type === "mcq"}
              <div class="quiz-opts">
                {#each q.options as opt, oi (oi)}
                  <button class="quiz-opt{runAnswers[q.id]?.choice === oi ? ' correct' : ''}" onclick={() => pick(q.id, oi)}>
                    <span class="quiz-key mono">{String.fromCharCode(65 + oi)}</span>
                    <span class="read">{opt}</span>
                    {#if runAnswers[q.id]?.choice === oi}
                      <Icon name="check" size={14} color="var(--accent)" />
                    {/if}
                  </button>
                {/each}
              </div>
            {:else}
              <textarea
                class="input exam-textarea"
                rows="4"
                placeholder="Write your answer…"
                value={runAnswers[q.id]?.text ?? ""}
                oninput={(e) => write(q.id, e.currentTarget.value)}
              ></textarea>
            {/if}
          </div>
        {/each}
      </div>

      <div class="exam-run-foot">
        <button class="btn btn--primary" onclick={confirmSubmit} disabled={submitting}>
          {#if submitting}<span class="is-spin"></span> Grading…{:else}<Icon name="check" size={13} /> Submit exam{/if}
        </button>
      </div>
    </div>

  {:else if screen === "results"}
    <div class="set-pane">
      <header class="set-head">
        <div class="eyebrow">Results</div>
        <h1 class="read set-title">{exam?.title}</h1>
        {#if results?.graded_by}
          <p class="set-sub mono">Written answers graded by {results.graded_by} — double-check anything that looks off.</p>
        {/if}
      </header>

      <!-- Score as an AnalyticsView-style stat card row. -->
      <div class="exam-stats">
        <div class="exam-stat">
          <div class="exam-stat-k mono">Score</div>
          <div class="exam-stat-v">{Math.round(scorePct)}<span class="exam-stat-u">%</span></div>
        </div>
        {#if results}
          <div class="exam-stat">
            <div class="exam-stat-k mono">Marks earned</div>
            <div class="exam-stat-v">{results.earned ?? 0}<span class="exam-stat-u"> / {results.total ?? 0}</span></div>
          </div>
          <div class="exam-stat">
            <div class="exam-stat-k mono">Questions</div>
            <div class="exam-stat-v">{questions.length}</div>
          </div>
        {/if}
      </div>

      <div class="exam-cta">
        <button class="btn btn--primary" onclick={startNew}><Icon name="bolt" size={13} /> Retake</button>
        <button class="btn" onclick={backToSetup}><Icon name="book" size={13} /> New exam</button>
      </div>

      {#if weakTopics.length > 0}
        <!-- Weak-topic callout: a set-card with a warn accent. -->
        <section class="set-group">
          <div class="set-group-h"><h3 class="set-group-t">Topics to revise</h3></div>
          <div class="set-card exam-weak">
            <div class="set-row stacked">
              <div class="set-row-r">
                <div class="tag-suggest" style="margin-top:0">
                  {#each weakTopics as wt}
                    <span class="tag-chip-add on">{wt}</span>
                  {/each}
                </div>
              </div>
            </div>
          </div>
        </section>
      {/if}

      <!-- Per-question review reuses Quiz.svelte's answered-state classes. -->
      <section class="set-group">
        <div class="set-group-h"><h3 class="set-group-t">Review</h3></div>
        <div class="exam-review">
          {#each reviewItems as { q, r }, idx (q.id)}
            {@const correct = q.type === "mcq" ? r?.correct : (r?.score ?? 0) >= q.marks}
            {@const partial = q.type === "written" && (r?.score ?? 0) > 0 && (r?.score ?? 0) < q.marks}
            <div class="quiz-card">
              <div class="exam-q-head">
                <span class="quiz-key mono">{idx + 1}</span>
                <p class="quiz-q read">{q.q}</p>
                <span class="mono exam-rev-mark" class:ok={correct} class:warn={partial} class:err={!correct && !partial}>
                  {(r?.score ?? 0)} / {q.marks}
                </span>
              </div>

              {#if q.type === "mcq"}
                <div class="quiz-opts">
                  {#each q.options as opt, oi (oi)}
                    {@const isAnswer = oi === q.correct}
                    {@const isPick = r?.your_choice === oi}
                    <div class="quiz-opt{isAnswer ? ' correct' : ''}{isPick && !isAnswer ? ' wrong' : ''}">
                      <span class="quiz-key mono">{String.fromCharCode(65 + oi)}</span>
                      <span class="read">{opt}</span>
                      {#if isPick}<span class="badge">Your answer</span>{/if}
                      {#if isAnswer}<Icon name="check" size={13} color="var(--ok)" />{/if}
                    </div>
                  {/each}
                </div>
              {:else}
                <div class="exam-rev-written">
                  <span class="mono faint exam-rev-label">Your answer</span>
                  <p class="read exam-rev-yours">{r?.your_text || "(blank)"}</p>
                </div>
              {/if}

              {#if r?.feedback}
                <p class="mono muted exam-rev-fb">{r.feedback}</p>
              {/if}
            </div>
          {/each}
        </div>
      </section>
    </div>
  {/if}
</div>

<style>
  /* Stepper value (matches the Settings pomodoro stepper inline style). */
  .exam-step { display: flex; align-items: center; gap: 8px; }
  .exam-step-v { min-width: 36px; text-align: center; color: var(--fg-bright); }

  /* Primary action + summary row, like GenerateMaterial's footer. */
  .exam-cta { display: flex; align-items: center; gap: var(--sp-3); margin-bottom: var(--sp-7); }

  /* Past-exam rows: clickable Settings rows. */
  .exam-past { cursor: pointer; }
  .exam-past:hover { background: var(--surface-2); }
  .exam-past-r { display: flex; align-items: center; gap: var(--sp-3); min-width: 0; }
  .exam-past-score { color: var(--fg-bright); font-variant-numeric: tabular-nums; }

  /* Generating: GeneratingCard look, centered. */
  .exam-center { display: flex; align-items: center; justify-content: center; min-height: 50vh; }
  .exam-gen-card {
    display: flex; align-items: center; gap: 12px;
    padding: 14px 18px; border: 1px solid var(--border-strong);
    border-radius: var(--rad-4); background: var(--surface-2);
  }
  .exam-gen-text { display: flex; flex-direction: column; gap: 2px; }
  .exam-gen-label { color: var(--fg-bright); font-size: var(--t-sm); }
  .exam-gen-sub { font-size: var(--t-xs); }

  /* Run screen. */
  .exam-run { max-width: 720px; margin: 0 auto; padding: var(--sp-6) var(--sp-7); display: flex; flex-direction: column; gap: var(--sp-4); }
  .exam-run-head {
    position: sticky; top: 0; z-index: 2; display: flex; align-items: center; justify-content: space-between;
    gap: var(--sp-4); padding: var(--sp-3) 0; background: var(--bg); border-bottom: 1px solid var(--border);
  }
  .exam-run-title { font-size: var(--r-md); color: var(--fg-bright); min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .exam-timer {
    flex: none; display: inline-flex; align-items: center; gap: 6px; font-size: var(--t-lg);
    padding: 6px 12px; border: 1px solid var(--border-strong); border-radius: var(--rad-2);
    color: var(--fg-bright); background: var(--surface-2); font-variant-numeric: tabular-nums;
  }
  .exam-timer.warn { color: var(--warn); border-color: color-mix(in oklab, var(--warn) 50%, var(--border-strong)); }
  .exam-timer.low { color: var(--err); border-color: var(--err); }

  .exam-qs { display: flex; flex-direction: column; gap: var(--sp-4); }
  .exam-q-head { display: flex; align-items: baseline; gap: var(--sp-3); margin-bottom: var(--sp-2); }
  .exam-q-head .quiz-q { flex: 1; margin: 0; }
  .exam-q-marks { flex: none; font-size: var(--t-xs); }
  .exam-textarea { width: 100%; resize: vertical; font-family: inherit; }
  .exam-run-foot { display: flex; justify-content: flex-end; padding-bottom: var(--sp-6); }

  /* Results stat cards (replicates AnalyticsView's an-stat, which is scoped there). */
  .exam-stats { display: grid; grid-template-columns: repeat(3, 1fr); gap: var(--sp-4); margin-bottom: var(--sp-5); }
  @media (max-width: 760px) { .exam-stats { grid-template-columns: repeat(2, 1fr); } }
  .exam-stat { background: var(--surface); border: 1px solid var(--border); border-radius: var(--rad-3); padding: 14px 16px; }
  .exam-stat-k { font-size: var(--t-2xs); letter-spacing: 0.08em; text-transform: uppercase; color: var(--fg-faint); }
  .exam-stat-v { margin-top: 8px; font-size: var(--r-xl); font-weight: 600; color: var(--fg-bright); font-variant-numeric: tabular-nums; }
  .exam-stat-u { font-size: var(--t-sm); font-weight: 400; color: var(--fg-faint); }

  /* Weak-topic callout: warn-accented set-card. */
  .exam-weak { border-color: color-mix(in oklab, var(--warn) 40%, var(--border)); background: color-mix(in oklab, var(--warn) 6%, var(--surface)); }

  /* Review (reuses quiz-card / quiz-opt; only the mark badge + written block are glue). */
  .exam-review { display: flex; flex-direction: column; gap: var(--sp-3); }
  .exam-rev-mark { flex: none; font-size: var(--t-2xs); text-transform: uppercase; letter-spacing: 0.06em; }
  .exam-rev-mark.ok { color: var(--ok); }
  .exam-rev-mark.warn { color: var(--warn); }
  .exam-rev-mark.err { color: var(--err); }
  .exam-rev-written { display: flex; flex-direction: column; gap: var(--sp-1); margin-top: var(--sp-2); }
  .exam-rev-label { font-size: var(--t-2xs); text-transform: uppercase; letter-spacing: 0.06em; }
  .exam-rev-yours {
    font-size: var(--r-sm); color: var(--fg); background: var(--surface-2);
    border: 1px solid var(--border-strong); border-radius: var(--rad-3); padding: 10px 12px; white-space: pre-wrap;
  }
  .exam-rev-fb { font-size: var(--t-sm); line-height: 1.5; margin-top: var(--sp-2); }
</style>
