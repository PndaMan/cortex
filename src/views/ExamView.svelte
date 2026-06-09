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

<div class="exam-wrap">
  {#if screen === "setup"}
    <div class="exam-setup">
      <header class="exam-head">
        <div>
          <h1 class="read exam-h1">Exam mode</h1>
          <p class="mono faint">Generate a timed exam from {app.activeSubject?.name ?? "your subject"} and grade it instantly.</p>
        </div>
      </header>

      {#if !app.activeSubject}
        <div class="exam-empty mono muted">Open a subject to create an exam.</div>
      {:else}
        <section class="set-group">
          <div class="set-group-h"><h3 class="set-group-t">Topics</h3>
            <p class="set-group-d">Leave all unselected to cover the whole subject.</p></div>
          <div class="set-card">
            <div class="exam-chips">
              {#if topics.length === 0}
                <span class="mono faint">No topics yet — the exam will use all sources.</span>
              {:else}
                {#each topics as t (t.id)}
                  <button class="exam-chip mono{selTopics.includes(t.id) ? ' on' : ''}" onclick={() => toggleTopic(t.id)}>
                    {t.name}
                  </button>
                {/each}
              {/if}
            </div>
          </div>
        </section>

        <section class="set-group">
          <div class="set-group-h"><h3 class="set-group-t">Format</h3></div>
          <div class="set-card exam-format">
            <div class="exam-field">
              <span class="onb-label mono">DURATION</span>
              <div class="exam-durs">
                {#each DURATIONS as d}
                  <button class="exam-dur mono{duration === d ? ' on' : ''}" onclick={() => (duration = d)}>{d}m</button>
                {/each}
              </div>
            </div>

            <div class="exam-field">
              <span class="onb-label mono">MULTIPLE CHOICE</span>
              <div class="exam-count">
                <button class="btn btn--icon btn--sm" onclick={() => clampMcq(mcqCount - 1)} disabled={mcqCount <= 0} title="Fewer">−</button>
                <input class="input exam-count-in mono" type="number" min="0" max="30" value={mcqCount}
                  oninput={(e) => clampMcq(parseInt(e.currentTarget.value, 10))} />
                <button class="btn btn--icon btn--sm" onclick={() => clampMcq(mcqCount + 1)} disabled={mcqCount >= 30} title="More">+</button>
              </div>
            </div>

            <div class="exam-field">
              <span class="onb-label mono">WRITTEN</span>
              <div class="exam-count">
                <button class="btn btn--icon btn--sm" onclick={() => clampWritten(writtenCount - 1)} disabled={writtenCount <= 0} title="Fewer">−</button>
                <input class="input exam-count-in mono" type="number" min="0" max="15" value={writtenCount}
                  oninput={(e) => clampWritten(parseInt(e.currentTarget.value, 10))} />
                <button class="btn btn--icon btn--sm" onclick={() => clampWritten(writtenCount + 1)} disabled={writtenCount >= 15} title="More">+</button>
              </div>
            </div>
          </div>
        </section>

        <div class="exam-actions">
          <button class="btn btn--primary" onclick={startNew} disabled={!canStart}>
            <Icon name="bolt" size={13} /> Generate &amp; start
          </button>
          <span class="mono faint">{mcqCount + writtenCount} questions · {duration} min</span>
        </div>

        {#if pastExams.length > 0}
          <section class="set-group">
            <div class="set-group-h"><h3 class="set-group-t">Past exams</h3></div>
            <div class="exam-past">
              {#each pastExams as e (e.id)}
                <div
                  class="exam-past-row"
                  role="button"
                  tabindex="0"
                  onclick={() => openPast(e)}
                  onkeydown={(ev) => { if (ev.key === "Enter") openPast(e); }}
                >
                  <span class="exam-past-title read">{e.title}</span>
                  <span class="exam-past-meta mono faint">
                    {#if e.status === "graded"}
                      {Math.round(e.score ?? 0)}%
                    {:else}
                      {e.status === "in_progress" ? "In progress" : "Ready"}
                    {/if}
                  </span>
                  <span class="exam-past-status mono" class:ok={e.status === "graded"}>{e.status}</span>
                  <button class="btn btn--icon btn--sm btn--ghost" title="Delete" aria-label="Delete exam" onclick={(ev) => deletePast(e, ev)}>
                    <Icon name="x" size={12} />
                  </button>
                </div>
              {/each}
            </div>
          </section>
        {/if}
      {/if}
    </div>

  {:else if screen === "generating"}
    <div class="exam-gen">
      <div class="exam-gen-card">
        <div class="exam-spinner"></div>
        <h2 class="read">Writing your exam…</h2>
        <p class="mono faint">Generating {mcqCount} multiple-choice and {writtenCount} written questions.</p>
      </div>
    </div>

  {:else if screen === "run"}
    <div class="exam-run">
      <header class="exam-run-head">
        <div class="exam-run-title read">{exam?.title}</div>
        <div class="exam-timer mono" class:low={lowTime}>
          <Icon name="record" size={13} /> {mmss}
        </div>
      </header>

      <div class="exam-questions">
        {#each questions as q, qi (q.id)}
          <div class="exam-q">
            <div class="exam-q-head">
              <span class="exam-q-num mono">{qi + 1}</span>
              <p class="read exam-q-text">{q.q}</p>
              <span class="exam-q-marks mono faint">{q.marks} mark{q.marks === 1 ? "" : "s"}</span>
            </div>
            {#if q.type === "mcq"}
              <div class="exam-opts">
                {#each q.options as opt, oi (oi)}
                  <button class="exam-opt{runAnswers[q.id]?.choice === oi ? ' on' : ''}" onclick={() => pick(q.id, oi)}>
                    <span class="quiz-key mono">{String.fromCharCode(65 + oi)}</span>
                    <span class="read">{opt}</span>
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
          {submitting ? "Grading…" : "Submit exam"}
        </button>
      </div>
    </div>

  {:else if screen === "results"}
    <div class="exam-results">
      <header class="exam-res-head">
        <div class="exam-score">
          <span class="exam-score-pct read">{Math.round(scorePct)}%</span>
          <span class="mono faint">{exam?.title}</span>
        </div>
        <div class="row gap-2">
          <button class="btn btn--primary" onclick={startNew}>Retake</button>
          <button class="btn" onclick={backToSetup}>New exam</button>
        </div>
      </header>

      {#if weakTopics.length > 0}
        <div class="exam-weak">
          <span class="mono exam-weak-h">Topics to revise</span>
          <div class="exam-chips">
            {#each weakTopics as wt}
              <span class="exam-chip mono on">{wt}</span>
            {/each}
          </div>
        </div>
      {/if}

      <div class="exam-review">
        {#each reviewItems as { q, r }, idx (q.id)}
          {@const correct = q.type === "mcq" ? r?.correct : (r?.score ?? 0) >= q.marks}
          {@const partial = q.type === "written" && (r?.score ?? 0) > 0 && (r?.score ?? 0) < q.marks}
          <div class="exam-rev-card">
            <div class="exam-rev-q">
              <span class="exam-q-num mono">{idx + 1}</span>
              <p class="read">{q.q}</p>
              <span class="exam-rev-mark mono" class:ok={correct} class:partial={partial} class:err={!correct && !partial}>
                {(r?.score ?? 0)} / {q.marks}
              </span>
            </div>

            {#if q.type === "mcq"}
              <div class="exam-rev-opts">
                {#each q.options as opt, oi (oi)}
                  {@const isAnswer = oi === q.correct}
                  {@const isPick = r?.your_choice === oi}
                  <div class="exam-rev-opt" class:correct={isAnswer} class:wrong={isPick && !isAnswer}>
                    <span class="quiz-key mono">{String.fromCharCode(65 + oi)}</span>
                    <span class="read">{opt}</span>
                    {#if isPick}<span class="badge">Your answer</span>{/if}
                    {#if isAnswer}<span class="badge">Correct</span>{/if}
                  </div>
                {/each}
              </div>
            {:else}
              <div class="exam-rev-written">
                <p class="mono faint exam-rev-label">Your answer</p>
                <p class="read exam-rev-yours">{r?.your_text || "(blank)"}</p>
              </div>
            {/if}

            {#if r?.feedback}
              <p class="mono muted exam-rev-fb">{r.feedback}</p>
            {/if}
          </div>
        {/each}
      </div>
    </div>
  {/if}
</div>

<style>
  .exam-wrap { height: 100%; overflow-y: auto; padding: var(--sp-6); }

  /* ── setup ── */
  .exam-setup { max-width: 720px; margin: 0 auto; display: flex; flex-direction: column; gap: var(--sp-5); }
  .exam-head { display: flex; justify-content: space-between; align-items: flex-end; }
  .exam-h1 { font-size: var(--r-xl); color: var(--fg-bright); margin-bottom: 2px; }
  .exam-empty { padding: var(--sp-6); text-align: center; }

  .exam-chips { display: flex; flex-wrap: wrap; gap: var(--sp-2); }
  .exam-chip {
    padding: 6px 12px;
    border: 1px solid var(--border-strong);
    border-radius: var(--rad-pill, 999px);
    background: var(--surface-2);
    color: var(--fg);
    font-size: var(--t-sm);
    cursor: pointer;
    transition: var(--t-fast, 120ms);
  }
  .exam-chip.on {
    border-color: var(--accent);
    background: color-mix(in oklab, var(--accent) 16%, var(--surface));
    color: var(--accent);
  }

  .exam-format { display: flex; flex-direction: column; gap: var(--sp-4); }
  .exam-field { display: flex; align-items: center; justify-content: space-between; gap: var(--sp-3); }
  .onb-label { font-size: var(--t-xs); letter-spacing: 0.06em; color: var(--fg-faint); }
  .exam-durs, .exam-count { display: flex; gap: var(--sp-1); align-items: center; }
  .exam-dur {
    padding: 6px 12px; border: 1px solid var(--border-strong); border-radius: var(--rad-3);
    background: var(--surface-2); color: var(--fg); font-size: var(--t-sm); cursor: pointer;
  }
  .exam-dur.on { border-color: var(--accent); background: color-mix(in oklab, var(--accent) 16%, var(--surface)); color: var(--accent); }
  .exam-count-in { width: 56px; text-align: center; }

  .exam-actions { display: flex; align-items: center; gap: var(--sp-3); }

  .exam-past { display: flex; flex-direction: column; gap: var(--sp-1); }
  .exam-past-row {
    display: grid; grid-template-columns: 1fr auto auto auto; align-items: center; gap: var(--sp-3);
    padding: 10px 12px; background: var(--surface); border: 1px solid var(--border-strong);
    border-radius: var(--rad-3); cursor: pointer; text-align: left; width: 100%;
  }
  .exam-past-row:hover { border-color: var(--accent); }
  .exam-past-title { font-size: var(--r-sm); color: var(--fg-bright); }
  .exam-past-meta { font-size: var(--t-sm); }
  .exam-past-status { font-size: var(--t-2xs); text-transform: uppercase; letter-spacing: 0.06em; color: var(--fg-faint); }
  .exam-past-status.ok { color: var(--ok); }

  /* ── generating ── */
  .exam-gen { display: flex; align-items: center; justify-content: center; height: 100%; }
  .exam-gen-card { text-align: center; display: flex; flex-direction: column; align-items: center; gap: var(--sp-3); }
  .exam-spinner {
    width: 36px; height: 36px; border-radius: 50%;
    border: 3px solid var(--border-strong); border-top-color: var(--accent);
    animation: exam-spin 0.8s linear infinite;
  }
  @keyframes exam-spin { to { transform: rotate(360deg); } }

  /* ── run ── */
  .exam-run { max-width: 760px; margin: 0 auto; display: flex; flex-direction: column; gap: var(--sp-4); }
  .exam-run-head {
    position: sticky; top: 0; z-index: 2; display: flex; justify-content: space-between; align-items: center;
    padding: var(--sp-3) 0; background: var(--bg, var(--surface)); backdrop-filter: blur(6px);
  }
  .exam-run-title { font-size: var(--r-md); color: var(--fg-bright); }
  .exam-timer {
    display: inline-flex; align-items: center; gap: var(--sp-1); font-size: var(--r-md);
    padding: 6px 12px; border: 1px solid var(--border-strong); border-radius: var(--rad-3);
    color: var(--fg-bright); background: var(--surface-2);
  }
  .exam-timer.low { color: var(--err); border-color: var(--err); }

  .exam-questions { display: flex; flex-direction: column; gap: var(--sp-4); }
  .exam-q {
    background: var(--surface); border: 1px solid var(--border-strong); border-radius: var(--rad-4); padding: var(--sp-5);
    display: flex; flex-direction: column; gap: var(--sp-3);
  }
  .exam-q-head { display: flex; align-items: baseline; gap: var(--sp-3); }
  .exam-q-num {
    flex: none; width: 22px; height: 22px; display: inline-flex; align-items: center; justify-content: center;
    border: 1px solid var(--border-strong); border-radius: var(--rad-2); font-size: var(--t-xs); color: var(--fg-faint);
  }
  .exam-q-text { flex: 1; font-size: var(--r-md); color: var(--fg-bright); line-height: 1.4; }
  .exam-q-marks { flex: none; font-size: var(--t-xs); }

  .exam-opts { display: flex; flex-direction: column; gap: 7px; }
  .exam-opt {
    display: flex; align-items: center; gap: var(--sp-3); padding: 10px 12px;
    background: var(--surface-2); border: 1px solid var(--border-strong); border-radius: var(--rad-3);
    color: var(--fg); cursor: pointer; text-align: left;
  }
  .exam-opt:hover { border-color: var(--accent); }
  .exam-opt.on { border-color: var(--accent); background: color-mix(in oklab, var(--accent) 14%, var(--surface)); }
  .exam-opt.on .quiz-key { border-color: var(--accent); color: var(--accent); }
  .exam-opt .read { flex: 1; font-size: var(--r-sm); }

  .exam-textarea { width: 100%; resize: vertical; font-family: inherit; }

  .exam-run-foot { display: flex; justify-content: flex-end; padding-bottom: var(--sp-4); }

  /* ── results ── */
  .exam-results { max-width: 760px; margin: 0 auto; display: flex; flex-direction: column; gap: var(--sp-4); }
  .exam-res-head { display: flex; justify-content: space-between; align-items: center; }
  .exam-score { display: flex; flex-direction: column; gap: 2px; }
  .exam-score-pct { font-size: var(--r-2xl, 2rem); color: var(--fg-bright); line-height: 1; }

  .exam-weak {
    background: var(--surface); border: 1px solid var(--border-strong); border-radius: var(--rad-4);
    padding: var(--sp-4); display: flex; flex-direction: column; gap: var(--sp-2);
  }
  .exam-weak-h { font-size: var(--t-xs); text-transform: uppercase; letter-spacing: 0.06em; color: var(--fg-faint); }

  .exam-review { display: flex; flex-direction: column; gap: var(--sp-3); }
  .exam-rev-card {
    background: var(--surface); border: 1px solid var(--border-strong); border-radius: var(--rad-4); padding: var(--sp-5);
    display: flex; flex-direction: column; gap: var(--sp-3);
  }
  .exam-rev-q { display: flex; align-items: baseline; gap: var(--sp-3); }
  .exam-rev-q .read { flex: 1; font-size: var(--r-md); color: var(--fg-bright); line-height: 1.35; }
  .exam-rev-mark { flex: none; font-size: var(--t-2xs); text-transform: uppercase; letter-spacing: 0.06em; }
  .exam-rev-mark.ok { color: var(--ok); }
  .exam-rev-mark.partial { color: var(--warn); }
  .exam-rev-mark.err { color: var(--err); }

  .exam-rev-opts { display: flex; flex-direction: column; gap: 7px; }
  .exam-rev-opt {
    display: flex; align-items: center; gap: var(--sp-3); padding: 9px 12px;
    background: var(--surface-2); border: 1px solid var(--border-strong); border-radius: var(--rad-3); color: var(--fg);
  }
  .exam-rev-opt .read { flex: 1; font-size: var(--r-sm); }
  .exam-rev-opt.correct { border-color: var(--ok); background: color-mix(in oklab, var(--ok) 14%, var(--surface)); }
  .exam-rev-opt.correct .quiz-key { border-color: var(--ok); color: var(--ok); }
  .exam-rev-opt.wrong { border-color: var(--err); background: color-mix(in oklab, var(--err) 12%, var(--surface)); }
  .exam-rev-opt.wrong .quiz-key { border-color: var(--err); color: var(--err); }

  .exam-rev-written { display: flex; flex-direction: column; gap: var(--sp-1); }
  .exam-rev-label { font-size: var(--t-2xs); text-transform: uppercase; letter-spacing: 0.06em; }
  .exam-rev-yours {
    font-size: var(--r-sm); color: var(--fg); background: var(--surface-2);
    border: 1px solid var(--border-strong); border-radius: var(--rad-3); padding: 10px 12px; white-space: pre-wrap;
  }
  .exam-rev-fb { font-size: var(--t-sm); line-height: 1.5; }
</style>
