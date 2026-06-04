<script lang="ts">
  // Themed create/edit modal for calendar events & tasks. Mirrors EditModal's
  // backdrop/dialog pattern. Standalone — no Google. Saves through api.*.
  import { app, SUBJECT_COLORS } from "../lib/store.svelte";
  import * as api from "../lib/api";
  import type { CalEvent } from "../lib/api";
  import Picker from "./Picker.svelte";
  import DatePicker from "./DatePicker.svelte";

  let {
    event,
    defaultDateMs,
    onClose,
    onSaved,
  }: {
    event: CalEvent | null;
    defaultDateMs?: number;
    onClose: () => void;
    onSaved: () => void;
  } = $props();

  // ---- datetime-local <-> epoch ms (local time) helpers ----
  // <input type="datetime-local"> works in the browser's local time and has no
  // timezone; we pad each field and (de)serialize via the local Date ctor.
  function pad(n: number): string {
    return String(n).padStart(2, "0");
  }
  function msToLocalInput(ms: number): string {
    const d = new Date(ms);
    return (
      `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}` +
      `T${pad(d.getHours())}:${pad(d.getMinutes())}`
    );
  }
  function localInputToMs(v: string): number | null {
    if (!v) return null;
    // Accepts "YYYY-MM-DDTHH:MM" or date-only "YYYY-MM-DD" (all-day).
    const m = v.match(/^(\d{4})-(\d{2})-(\d{2})(?:T(\d{2}):(\d{2}))?/);
    if (!m) return null;
    const [, y, mo, da, h, mi] = m;
    return new Date(+y, +mo - 1, +da, +(h ?? 0), +(mi ?? 0), 0, 0).getTime();
  }

  // ---- reminder offsets ----
  const REMINDER_OPTS = [
    { id: "none", label: "None" },
    { id: "0", label: "At time of event" },
    { id: "300000", label: "5 minutes before" },
    { id: "3600000", label: "1 hour before" },
    { id: "86400000", label: "1 day before" },
  ];

  // ---- form state ----
  let title = $state("");
  let description = $state("");
  let location = $state("");
  let startVal = $state("");
  let endVal = $state("");
  let allDay = $state(false);
  // Deadlines are typed: exam | assignment | project. The segmented control shows
  // Event / Task / Deadline, and a sub-row picks the deadline type when relevant.
  const DEADLINE_KINDS = ["exam", "assignment", "project"] as const;
  type Kind = "event" | "task" | "exam" | "assignment" | "project";
  let kind = $state<Kind>("event");
  const isDeadline = $derived((DEADLINE_KINDS as readonly string[]).includes(kind));
  function setDeadline() { if (!isDeadline) kind = "exam"; }
  let color = $state<string | null>(null);
  let subjectId = $state<string>("");
  let reminder = $state<string>("none");
  let firstInput = $state<HTMLInputElement | null>(null);

  // Seed the form whenever the target event (or default date) changes.
  $effect(() => {
    const e = event;
    if (e) {
      title = e.title ?? "";
      description = e.description ?? "";
      location = e.location ?? "";
      startVal = msToLocalInput(e.start_ms);
      endVal = e.end_ms != null ? msToLocalInput(e.end_ms) : "";
      allDay = !!e.all_day;
      kind = (["task", "exam", "assignment", "project"] as readonly string[]).includes(e.kind)
        ? (e.kind as Kind)
        : e.kind === "deadline" // legacy generic deadline → default to exam
          ? "exam"
          : "event";
      color = e.color ?? null;
      subjectId = e.subject_id ?? "";
      reminder = deriveReminder(e.reminder_ms, e.start_ms);
    } else {
      title = "";
      description = "";
      location = "";
      // Default to the clicked time, or 9:00 AM if only a day (midnight) was
      // given, or now when nothing was provided.
      const base = defaultDateMs != null ? new Date(defaultDateMs) : new Date();
      if (defaultDateMs != null) {
        const atMidnight =
          base.getHours() === 0 &&
          base.getMinutes() === 0 &&
          base.getSeconds() === 0 &&
          base.getMilliseconds() === 0;
        if (atMidnight) base.setHours(9, 0, 0, 0);
      }
      startVal = msToLocalInput(base.getTime());
      endVal = "";
      allDay = false;
      kind = "event";
      color = null;
      subjectId = "";
      reminder = "none";
    }
    queueMicrotask(() => {
      firstInput?.focus();
      firstInput?.select();
    });
  });

  function deriveReminder(reminderMs: number | null, startMs: number): string {
    if (reminderMs == null) return "none";
    const offset = startMs - reminderMs;
    const match = REMINDER_OPTS.find((o) => o.id !== "none" && +o.id === offset);
    return match ? match.id : "none";
  }

  const subjectOptions = $derived([
    { id: "", label: "— no subject —" },
    ...app.subjects.map((s) => ({ id: s.id, label: s.name })),
  ]);

  function computeReminderMs(startMs: number): number | null {
    if (reminder === "none") return null;
    return startMs - +reminder;
  }

  async function save() {
    const t = title.trim();
    const startMs = localInputToMs(startVal);
    if (!t) {
      app.pushToast({ kind: "warning", title: "Title required" });
      return;
    }
    if (startMs == null) {
      app.pushToast({ kind: "warning", title: "Start time required" });
      return;
    }
    const endMs = endVal ? localInputToMs(endVal) : null;
    const reminderMs = computeReminderMs(startMs);
    const payload = {
      title: t,
      startMs,
      subjectId: subjectId || null,
      description: description.trim() || null,
      location: location.trim() || null,
      color: color,
      endMs,
      allDay,
      kind,
      reminderMs,
    };
    try {
      if (event) {
        await api.updateEvent({ id: event.id, ...payload });
      } else {
        await api.createEvent(payload);
      }
      onSaved();
      onClose();
    } catch (e) {
      app.pushToast({ kind: "error", title: "Save failed", body: String(e) });
    }
  }

  async function del() {
    if (!event) return;
    const ok = await app.confirm({
      title: "Delete event?",
      danger: true,
      okLabel: "Delete",
    });
    if (!ok) return;
    try {
      await api.deleteEvent(event.id);
      onSaved();
      onClose();
    } catch (e) {
      app.pushToast({ kind: "error", title: "Delete failed", body: String(e) });
    }
  }

  function onKey(e: KeyboardEvent) {
    e.stopPropagation();
    if (e.key === "Escape") {
      e.preventDefault();
      onClose();
    } else if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) {
      e.preventDefault();
      save();
    }
  }
</script>

<svelte:window onkeydown={onKey} />

<div class="ev-back" role="presentation" onmousedown={() => onClose()}>
  <div
    class="ev"
    role="dialog"
    aria-modal="true"
    tabindex="-1"
    onmousedown={(e) => e.stopPropagation()}
  >
    <div class="ev-title">{event ? "Edit event" : "New event"}</div>

    <label class="ev-field">
      <span class="ev-lbl">Title</span>
      <input
        bind:this={firstInput}
        bind:value={title}
        class="input"
        placeholder="What's happening?"
      />
    </label>

    <div class="ev-field">
      <span class="ev-lbl">Type</span>
      <div class="ev-seg" role="group" aria-label="Type">
        <button
          type="button"
          class={"ev-seg-btn" + (kind === "event" ? " on" : "")}
          onclick={() => (kind = "event")}>Event</button
        >
        <button
          type="button"
          class={"ev-seg-btn" + (kind === "task" ? " on" : "")}
          onclick={() => (kind = "task")}>Task</button
        >
        <button
          type="button"
          class={"ev-seg-btn" + (isDeadline ? " on" : "")}
          onclick={setDeadline}>Deadline</button
        >
      </div>
    </div>

    {#if isDeadline}
      <div class="ev-field">
        <span class="ev-lbl">Deadline type</span>
        <div class="ev-seg" role="group" aria-label="Deadline type">
          {#each DEADLINE_KINDS as dk (dk)}
            <button
              type="button"
              class={"ev-seg-btn" + (kind === dk ? " on" : "")}
              onclick={() => (kind = dk)}
            >{dk.charAt(0).toUpperCase() + dk.slice(1)}</button>
          {/each}
        </div>
      </div>
    {/if}

    <div class="ev-row">
      <div class="ev-field ev-grow">
        <span class="ev-lbl">Start</span>
        <DatePicker value={startVal} onChange={(v) => (startVal = v)} withTime={!allDay} placeholder="Pick a date" />
      </div>
      <div class="ev-field ev-grow">
        <span class="ev-lbl">End <span class="ev-opt">(optional)</span></span>
        <DatePicker value={endVal} onChange={(v) => (endVal = v)} withTime={!allDay} placeholder="—" />
      </div>
    </div>

    <label class="ev-toggle">
      <input type="checkbox" bind:checked={allDay} />
      <span>All day</span>
    </label>

    <div class="ev-field">
      <span class="ev-lbl">Subject <span class="ev-opt">(optional)</span></span>
      <Picker
        value={subjectId}
        onChange={(id) => (subjectId = id)}
        options={subjectOptions}
        placeholder="— no subject —"
      />
    </div>

    <div class="ev-field">
      <span class="ev-lbl">Color</span>
      <div class="ev-colors">
        {#each SUBJECT_COLORS as c}
          <button
            type="button"
            class={"swatch" + (color === c ? " on" : "")}
            style:background={c}
            aria-label={c}
            onclick={() => (color = c)}
          ></button>
        {/each}
        <button
          type="button"
          class={"swatch swatch-clear" + (color === null ? " on" : "")}
          aria-label="No color"
          title="No color (use subject / accent)"
          onclick={() => (color = null)}
        >
          <span class="swatch-x">×</span>
        </button>
      </div>
    </div>

    <div class="ev-field">
      <span class="ev-lbl">Reminder</span>
      <Picker
        value={reminder}
        onChange={(id) => (reminder = id)}
        options={REMINDER_OPTS}
        placeholder="None"
      />
    </div>

    <label class="ev-field">
      <span class="ev-lbl">Location <span class="ev-opt">(optional)</span></span>
      <input bind:value={location} class="input" placeholder="Room, link, place…" />
    </label>

    <label class="ev-field">
      <span class="ev-lbl">Description <span class="ev-opt">(optional)</span></span>
      <textarea
        bind:value={description}
        class="input ev-textarea"
        rows="3"
        placeholder="Notes…"
      ></textarea>
    </label>

    <div class="ev-actions">
      {#if event}
        <button
          class="btn btn--danger btn--sm"
          type="button"
          style="margin-right:auto"
          onclick={del}>Delete</button
        >
      {/if}
      <button class="btn btn--ghost btn--sm" type="button" onclick={() => onClose()}
        >Cancel</button
      >
      <button class="btn btn--primary btn--sm" type="button" onclick={save}>Save</button>
    </div>
  </div>
</div>

<style>
  .ev-back {
    position: fixed;
    inset: 0;
    z-index: 200;
    display: flex;
    align-items: center;
    justify-content: center;
    background: color-mix(in oklab, var(--bg) 62%, transparent);
    backdrop-filter: blur(3px);
    animation: ev-fade 0.12s ease;
  }
  .ev {
    width: min(520px, calc(100vw - 48px));
    max-height: calc(100vh - 48px);
    overflow-y: auto;
    background: var(--surface);
    border: 1px solid var(--border-strong);
    border-radius: var(--r-lg, 12px);
    box-shadow: 0 18px 50px rgba(0, 0, 0, 0.5);
    padding: 20px;
    animation: ev-pop 0.13s ease;
  }
  .ev-title {
    font-family: var(--font-mono);
    font-size: var(--t-md, 14px);
    font-weight: 600;
    color: var(--fg-bright);
    margin-bottom: 14px;
  }
  .ev-field {
    display: block;
    margin-top: 12px;
  }
  .ev-row {
    display: flex;
    gap: 12px;
    align-items: flex-start;
  }
  .ev-grow {
    flex: 1;
    min-width: 0;
  }
  .ev-lbl {
    display: block;
    margin-bottom: 6px;
    font-size: var(--t-2xs, 10.5px);
    font-weight: 600;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: var(--fg-faint);
  }
  .ev-opt {
    letter-spacing: 0;
    text-transform: none;
    font-weight: 400;
    color: var(--fg-faint);
    opacity: 0.7;
  }
  .ev-field .input {
    width: 100%;
  }
  .ev-field :global(.dp) {
    width: 100%;
  }
  .ev-textarea {
    resize: vertical;
    min-height: 56px;
    font-family: inherit;
    line-height: 1.45;
    padding-top: 7px;
    padding-bottom: 7px;
  }
  /* segmented control */
  .ev-seg {
    display: inline-flex;
    border: 1px solid var(--border-strong);
    border-radius: 8px;
    overflow: hidden;
    background: var(--surface-2);
  }
  .ev-seg-btn {
    appearance: none;
    border: none;
    background: transparent;
    color: var(--fg-muted);
    font-family: var(--font-mono);
    font-size: var(--t-xs, 11.5px);
    padding: 6px 16px;
    cursor: pointer;
    transition: background 0.1s ease, color 0.1s ease;
  }
  .ev-seg-btn:hover {
    color: var(--fg-bright);
    background: var(--surface-3);
  }
  .ev-seg-btn.on {
    background: var(--accent);
    color: var(--accent-fg);
  }
  /* all-day toggle */
  .ev-toggle {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-top: 12px;
    font-size: var(--t-xs, 11.5px);
    color: var(--fg-muted);
    cursor: pointer;
    user-select: none;
  }
  .ev-toggle input {
    width: 15px;
    height: 15px;
    accent-color: var(--accent);
    cursor: pointer;
  }
  /* color swatches */
  .ev-colors {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
  }
  .swatch {
    width: 22px;
    height: 22px;
    border-radius: 6px;
    border: 2px solid transparent;
    cursor: pointer;
    padding: 0;
    transition: transform 0.1s ease, border-color 0.1s ease;
  }
  .swatch:hover {
    transform: scale(1.12);
  }
  .swatch.on {
    border-color: var(--fg-bright);
  }
  .swatch-clear {
    display: flex;
    align-items: center;
    justify-content: center;
    background: var(--surface-2);
    border: 1px solid var(--border-strong);
  }
  .swatch-x {
    color: var(--fg-faint);
    font-size: 14px;
    line-height: 1;
  }
  .ev-actions {
    margin-top: 20px;
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }
  @keyframes ev-fade {
    from {
      opacity: 0;
    }
    to {
      opacity: 1;
    }
  }
  @keyframes ev-pop {
    from {
      opacity: 0;
      transform: translateY(6px) scale(0.98);
    }
    to {
      opacity: 1;
      transform: none;
    }
  }
</style>
