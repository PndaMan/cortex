<script lang="ts">
  // Per-subject Kanban board. Cards are the subject's assignments / deadlines /
  // exams / tasks (CalEvents), grouped into To-do / Doing / Done by their
  // `status`. Drag a card between columns (or use the ◂ ▸ buttons) to move it;
  // status is kept in sync with the calendar's done flag. New manual cards are
  // created as `task` events and can be tagged by topic.
  import { app } from "../lib/store.svelte";
  import * as api from "../lib/api";
  import Icon from "../components/Icon.svelte";

  type Status = "todo" | "doing" | "done";
  const COLUMNS: { id: Status; label: string }[] = [
    { id: "todo", label: "To-do" },
    { id: "doing", label: "Doing" },
    { id: "done", label: "Done" },
  ];
  // Kinds that belong on the board (plain calendar "event"s are excluded).
  const BOARD_KINDS = new Set(["assignment", "project", "deadline", "exam", "task"]);

  const subj = $derived(app.activeSubject);
  let events = $state<api.CalEvent[]>([]);
  let loading = $state(true);
  let adding = $state<Status | null>(null);
  let newTitle = $state("");

  // Reload whenever the subject changes OR any event changes elsewhere (the
  // Assignments form, the calendar) so the board and list stay in lock-step.
  $effect(() => {
    const sid = subj?.id;
    void app.eventsChangedNonce;
    if (!sid) { events = []; loading = false; return; }
    loading = true;
    let cancelled = false;
    api.listEvents(sid).then((e) => {
      if (!cancelled) { events = e.filter((ev) => BOARD_KINDS.has(ev.kind)); loading = false; }
    }).catch(() => { if (!cancelled) { events = []; loading = false; } });
    return () => { cancelled = true; };
  });

  // ── pointer-based drag (native HTML5 DnD is unreliable in WebKitGTK) ──
  let dragId = $state<string | null>(null);   // card currently being dragged
  let overCol = $state<Status | null>(null);  // column under the pointer
  let ghost = $state<{ x: number; y: number; title: string } | null>(null);
  // Press bookkeeping until we cross the movement threshold (so taps/buttons
  // still register as clicks rather than drags).
  let press: { id: string; title: string; x: number; y: number } | null = null;

  function colAt(x: number, y: number): Status | null {
    const el = document.elementFromPoint(x, y)?.closest("[data-col]") as HTMLElement | null;
    const c = el?.dataset.col;
    return c === "todo" || c === "doing" || c === "done" ? c : null;
  }

  function onCardPointerDown(e: PointerEvent, ev: api.CalEvent) {
    if (e.button !== 0) return;
    press = { id: ev.id, title: ev.title, x: e.clientX, y: e.clientY };
    window.addEventListener("pointermove", onPointerMove);
    window.addEventListener("pointerup", onPointerUp);
  }
  function onPointerMove(e: PointerEvent) {
    if (press && !dragId) {
      if (Math.hypot(e.clientX - press.x, e.clientY - press.y) < 5) return; // not yet a drag
      dragId = press.id;
    }
    if (!dragId) return;
    e.preventDefault();
    ghost = { x: e.clientX, y: e.clientY, title: press?.title ?? "" };
    overCol = colAt(e.clientX, e.clientY);
  }
  function onPointerUp() {
    window.removeEventListener("pointermove", onPointerMove);
    window.removeEventListener("pointerup", onPointerUp);
    if (dragId && overCol) move(dragId, overCol);
    dragId = null; overCol = null; ghost = null; press = null;
  }

  const byStatus = (s: Status) => events.filter((e) => (e.status || "todo") === s);

  function topicName(id: string): string {
    return subj?.topics.find((t) => t.id === id)?.name ?? "";
  }

  async function move(id: string, status: Status) {
    const ev = events.find((e) => e.id === id);
    if (!ev || ev.status === status) return;
    // Optimistic; reconcile on failure.
    events = events.map((e) => (e.id === id ? { ...e, status } : e));
    try {
      await api.setEventStatus(id, status);
      app.notifyEventsChanged(); // keep the Assignments list + calendar in sync
    } catch (err) {
      app.pushToast({ kind: "error", title: "Couldn't move card", body: String(err) });
      events = events.map((e) => (e.id === id ? { ...e, status: ev.status } : e));
    }
  }

  function nextStatus(s: Status, dir: 1 | -1): Status {
    const order: Status[] = ["todo", "doing", "done"];
    const i = Math.min(2, Math.max(0, order.indexOf(s) + dir));
    return order[i];
  }

  async function addCard(status: Status) {
    const title = newTitle.trim();
    if (!title || !subj) { adding = null; newTitle = ""; return; }
    try {
      const ev = await api.createEvent({
        title,
        subjectId: subj.id,
        startMs: Date.now(),
        allDay: true,
        kind: "task",
      });
      // New cards arrive as "todo"; move if added to another column.
      if (status !== "todo") {
        await api.setEventStatus(ev.id, status);
        ev.status = status;
      }
      events = [...events, ev];
      app.notifyEventsChanged();
    } catch (err) {
      app.pushToast({ kind: "error", title: "Couldn't add card", body: String(err) });
    }
    newTitle = "";
    adding = null;
  }

  const prioClass = (p: string | null) =>
    p === "high" ? "p-high" : p === "med" ? "p-med" : p === "low" ? "p-low" : "";
  const kindLabel = (k: string) =>
    ({ assignment: "Assignment", project: "Project", deadline: "Deadline", exam: "Exam", task: "Task" })[k] ?? k;
  function dueLabel(e: api.CalEvent): string {
    if (e.kind === "task") return "";
    const d = new Date(e.start_ms);
    return d.toLocaleDateString(undefined, { month: "short", day: "numeric" });
  }
</script>

<div class="board">
  {#if loading}
    <div class="board-empty">Loading board…</div>
  {:else}
    {#each COLUMNS as col (col.id)}
      <section
        class="bcol{overCol === col.id && dragId ? ' over' : ''}"
        role="list"
        data-col={col.id}
      >
        <header class="bcol-h">
          <span class="bcol-t">{col.label}</span>
          <span class="bcol-n">{byStatus(col.id).length}</span>
          <button class="bcol-add" title="Add card" onclick={() => { adding = col.id; newTitle = ""; }}>
            <Icon name="plus" size={13} />
          </button>
        </header>

        {#if adding === col.id}
          <div class="bcard bcard-new">
            <!-- svelte-ignore a11y_autofocus -->
            <input
              class="bcard-input"
              placeholder="Card title…"
              bind:value={newTitle}
              autofocus
              onkeydown={(e) => { if (e.key === "Enter") addCard(col.id); else if (e.key === "Escape") { adding = null; newTitle = ""; } }}
              onblur={() => addCard(col.id)}
            />
          </div>
        {/if}

        <div class="bcol-cards">
          {#each byStatus(col.id) as e (e.id)}
            <article
              class="bcard{dragId === e.id ? ' dragging' : ''}"
              role="listitem"
              onpointerdown={(ev) => onCardPointerDown(ev, e)}
            >
              <div class="bcard-top">
                <span class="bcard-kind">{kindLabel(e.kind)}</span>
                {#if e.priority}<span class="bcard-prio {prioClass(e.priority)}">{e.priority}</span>{/if}
                {#if dueLabel(e)}<span class="bcard-due">{dueLabel(e)}</span>{/if}
              </div>
              <div class="bcard-title">{e.title}</div>
              {#if e.topic_ids?.length}
                <div class="bcard-tags">
                  {#each e.topic_ids as tid (tid)}
                    {#if topicName(tid)}<span class="bcard-tag">{topicName(tid)}</span>{/if}
                  {/each}
                </div>
              {/if}
              <div class="bcard-move">
                <button title="Move left" disabled={col.id === "todo"} onpointerdown={(ev) => ev.stopPropagation()} onclick={() => move(e.id, nextStatus(col.id, -1))}>◂</button>
                <button title="Move right" disabled={col.id === "done"} onpointerdown={(ev) => ev.stopPropagation()} onclick={() => move(e.id, nextStatus(col.id, 1))}>▸</button>
              </div>
            </article>
          {/each}
          {#if byStatus(col.id).length === 0 && adding !== col.id}
            <div class="bcol-empty">Drop cards here</div>
          {/if}
        </div>
      </section>
    {/each}
  {/if}
</div>

{#if ghost}
  <div class="bdrag-ghost" style:left="{ghost.x}px" style:top="{ghost.y}px">{ghost.title}</div>
{/if}

<style>
  .board {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 14px;
    padding: 18px 22px;
    height: 100%;
    overflow: hidden;
    align-items: start;
  }
  .board-empty { color: var(--fg-faint); padding: 40px; grid-column: 1 / -1; }
  .bcol {
    display: flex; flex-direction: column; min-height: 0; max-height: 100%;
    background: var(--surface-2); border: 1px solid var(--border);
    border-radius: var(--rad-3, 12px); padding: 10px;
    transition: border-color var(--dur-fast) var(--ease), background var(--dur-fast) var(--ease);
  }
  .bcol.over { border-color: var(--accent); background: color-mix(in oklab, var(--accent) 8%, var(--surface-2)); }
  .bcol-h { display: flex; align-items: center; gap: 8px; padding: 2px 4px 10px; }
  .bcol-t { font-family: var(--font-mono); font-size: var(--t-2xs, 10.5px); letter-spacing: 0.12em; text-transform: uppercase; color: var(--fg-muted); }
  .bcol-n { font-size: var(--t-2xs); color: var(--fg-faint); background: var(--surface-3); border-radius: 999px; padding: 1px 7px; }
  .bcol-add { margin-left: auto; background: none; border: none; color: var(--fg-faint); cursor: pointer; display: inline-flex; padding: 2px; border-radius: 6px; }
  .bcol-add:hover { color: var(--fg-bright); background: var(--surface-3); }
  .bcol-cards { display: flex; flex-direction: column; gap: 8px; overflow-y: auto; min-height: 40px; padding: 2px; }
  .bcol-empty { color: var(--fg-faint); font-size: var(--t-2xs); text-align: center; padding: 18px 8px; border: 1px dashed var(--border); border-radius: 8px; }
  .bcard {
    background: var(--surface); border: 1px solid var(--border); border-radius: 9px;
    padding: 9px 10px; cursor: grab; transition: border-color var(--dur-fast) var(--ease), transform var(--dur-fast) var(--ease);
    touch-action: none; user-select: none; -webkit-user-select: none;
  }
  .bcard:hover { border-color: var(--border-strong); }
  .bcard.dragging { opacity: 0.4; cursor: grabbing; }
  /* The card that follows the pointer while dragging. */
  .bdrag-ghost {
    position: fixed; z-index: 1000; transform: translate(10px, 8px);
    max-width: 240px; padding: 8px 11px; pointer-events: none;
    background: var(--surface); border: 1px solid var(--accent); border-radius: 9px;
    color: var(--fg-bright); font-size: var(--t-sm, 12.5px);
    box-shadow: 0 10px 30px rgba(0,0,0,0.4); white-space: nowrap; overflow: hidden; text-overflow: ellipsis;
  }
  .bcard-new { cursor: default; }
  .bcard-input { width: 100%; background: transparent; border: none; color: var(--fg-bright); font-size: var(--t-sm, 12.5px); outline: none; }
  .bcard-top { display: flex; align-items: center; gap: 6px; margin-bottom: 5px; }
  .bcard-kind { font-size: var(--t-2xs, 10px); letter-spacing: 0.06em; text-transform: uppercase; color: var(--fg-faint); }
  .bcard-prio { font-size: var(--t-2xs); padding: 0 6px; border-radius: 999px; text-transform: uppercase; }
  .p-high { background: color-mix(in oklab, var(--err) 22%, transparent); color: var(--err); }
  .p-med { background: color-mix(in oklab, var(--warn) 22%, transparent); color: var(--warn); }
  .p-low { background: var(--surface-3); color: var(--fg-faint); }
  .bcard-due { margin-left: auto; font-size: var(--t-2xs); color: var(--fg-faint); }
  .bcard-title { font-size: var(--t-sm, 12.5px); color: var(--fg-bright); line-height: 1.35; }
  .bcard-tags { display: flex; flex-wrap: wrap; gap: 4px; margin-top: 7px; }
  .bcard-tag { font-size: var(--t-2xs, 10px); padding: 1px 7px; border-radius: 999px; background: var(--surface-3); color: var(--fg-muted); }
  .bcard-move { display: flex; justify-content: flex-end; gap: 4px; margin-top: 7px; opacity: 0; transition: opacity var(--dur-fast) var(--ease); }
  .bcard:hover .bcard-move { opacity: 1; }
  .bcard-move button { background: var(--surface-3); border: none; color: var(--fg-muted); border-radius: 6px; width: 22px; height: 18px; cursor: pointer; font-size: 11px; line-height: 1; }
  .bcard-move button:hover:not(:disabled) { color: var(--fg-bright); }
  .bcard-move button:disabled { opacity: 0.3; cursor: default; }
</style>
