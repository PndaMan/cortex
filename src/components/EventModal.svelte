<script lang="ts">
  // Themed create/edit modal for calendar events & tasks. Standalone — no Google.
  // Saves through api.*. Built entirely from the design system: .input, .seg/
  // .seg-opt, .st-toggle, .btn, DatePicker, Picker. Progressive disclosure keeps
  // the default modal short: only the essentials show until "Add details".
  import { app, SUBJECT_COLORS } from "../lib/store.svelte";
  import * as api from "../lib/api";
  import type { CalEvent } from "../lib/api";
  import Icon from "./Icon.svelte";
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
  function msToDateInput(ms: number): string {
    const d = new Date(ms);
    return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
  }
  function localInputToMs(v: string): number | null {
    if (!v) return null;
    const m = v.match(/^(\d{4})-(\d{2})-(\d{2})(?:T(\d{2}):(\d{2}))?/);
    if (!m) return null;
    const [, y, mo, da, h, mi] = m;
    return new Date(+y, +mo - 1, +da, +(h ?? 0), +(mi ?? 0), 0, 0).getTime();
  }
  // Just the "YYYY-MM-DD" portion of either input shape.
  function dateOf(v: string): string {
    return v.slice(0, 10);
  }
  // "HH:MM" out of a local-input string, or a fallback.
  function timeOf(v: string, fallback = "09:00"): string {
    const m = /T(\d{2}:\d{2})/.exec(v);
    return m ? m[1] : fallback;
  }

  // ---- reminder offsets ----
  const REMINDER_OPTS = [
    { id: "none", label: "None" },
    { id: "0", label: "At time of event" },
    { id: "600000", label: "10 minutes before" },
    { id: "3600000", label: "1 hour before" },
    { id: "86400000", label: "1 day before" },
  ];

  // ---- kind segmented control (top-level event/task/deadline) ----
  const DEADLINE_KINDS = ["exam", "assignment", "project"] as const;
  type Kind = "event" | "task" | "exam" | "assignment" | "project";
  const isDeadlineKind = (k: string) => (DEADLINE_KINDS as readonly string[]).includes(k);

  // Active accent for the kind control — mirrors CalendarView.kindColor.
  function kindColor(k: Kind): string {
    switch (k) {
      case "task": return "var(--accent)";
      case "exam": return "var(--warn)";
      case "assignment": return "var(--accent)";
      case "project": return "var(--info)";
      default: return "var(--accent)";
    }
  }

  // ---- form state ----
  let title = $state("");
  let description = $state("");
  let location = $state("");
  // Date once + start/end times separately so the "when" block can render a
  // single date picker plus a compact start–end time row.
  let dateVal = $state(""); // "YYYY-MM-DD"
  let startTime = $state("09:00"); // "HH:MM"
  let endTime = $state(""); // "HH:MM" or "" (no end)
  let allDay = $state(false);
  let kind = $state<Kind>("event");
  const isDeadline = $derived(isDeadlineKind(kind));
  function setDeadline() { if (!isDeadline) kind = "exam"; }
  let color = $state<string | null>(null);
  let subjectId = $state<string>("");
  let reminder = $state<string>("none");
  let tagsText = $state("");
  let firstInput = $state<HTMLInputElement | null>(null);

  // Details expander — auto-opens when editing an event that already has
  // optional content, so nothing stays hidden from the user.
  let showDetails = $state(false);

  // Seed the form whenever the target event (or default date) changes.
  $effect(() => {
    const e = event;
    if (e) {
      title = e.title ?? "";
      description = e.description ?? "";
      location = e.location ?? "";
      dateVal = msToDateInput(e.start_ms);
      startTime = timeOf(msToLocalInput(e.start_ms));
      endTime = e.end_ms != null ? timeOf(msToLocalInput(e.end_ms), "") : "";
      allDay = !!e.all_day;
      kind = (["task", "exam", "assignment", "project"] as readonly string[]).includes(e.kind)
        ? (e.kind as Kind)
        : e.kind === "deadline"
          ? "exam"
          : "event";
      color = e.color ?? null;
      subjectId = e.subject_id ?? "";
      reminder = deriveReminder(e.reminder_ms, e.start_ms);
      tagsText = (e.tags ?? []).join(", ");
      showDetails =
        !!e.location || !!e.description || !!e.color ||
        !!e.subject_id || reminder !== "none" || (e.tags ?? []).length > 0;
    } else {
      title = "";
      description = "";
      location = "";
      const base = defaultDateMs != null ? new Date(defaultDateMs) : new Date();
      if (defaultDateMs != null) {
        const atMidnight =
          base.getHours() === 0 && base.getMinutes() === 0 &&
          base.getSeconds() === 0 && base.getMilliseconds() === 0;
        if (atMidnight) base.setHours(9, 0, 0, 0);
      } else {
        base.setMinutes(0, 0, 0);
      }
      dateVal = msToDateInput(base.getTime());
      startTime = timeOf(msToLocalInput(base.getTime()));
      endTime = "";
      allDay = false;
      kind = "event";
      color = null;
      subjectId = "";
      reminder = "none";
      tagsText = "";
      showDetails = false;
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

  // Compose the final start/end epoch ms from date + time fields.
  function startMsOf(): number | null {
    if (!dateVal) return null;
    return localInputToMs(allDay ? dateVal : `${dateVal}T${startTime}`);
  }
  function endMsOf(): number | null {
    if (allDay || !endTime || !dateVal) return null;
    return localInputToMs(`${dateVal}T${endTime}`);
  }

  function computeReminderMs(startMs: number): number | null {
    if (reminder === "none") return null;
    return startMs - +reminder;
  }

  async function save() {
    const t = title.trim();
    const startMs = startMsOf();
    if (!t) {
      app.pushToast({ kind: "warning", title: "Title required" });
      return;
    }
    if (startMs == null) {
      app.pushToast({ kind: "warning", title: "Date required" });
      return;
    }
    const endMs = endMsOf();
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
      tags: tagsText.split(",").map((s) => s.trim()).filter(Boolean),
    };
    try {
      if (event) {
        await api.updateEvent({ id: event.id, ...payload });
      } else {
        await api.createEvent(payload);
      }
      app.notifyEventsChanged();
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
      app.notifyEventsChanged();
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
    } else if (e.key === "Enter" && !e.shiftKey) {
      // Enter saves — but not while composing a multi-line description.
      const tag = (e.target as HTMLElement | null)?.tagName;
      if (tag === "TEXTAREA") return;
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
    <div class="ev-head">
      <div class="ev-title">{event ? "Edit event" : "New event"}</div>
      <button class="btn btn--ghost btn--icon btn--sm" type="button" aria-label="Close" onclick={() => onClose()}>
        <Icon name="x" size={13} />
      </button>
    </div>

    <!-- Title — large, focused on open. -->
    <input
      bind:this={firstInput}
      bind:value={title}
      class="input ev-title-input"
      placeholder="Add a title…"
    />

    <!-- WHEN block: date + all-day toggle, then a start–end time row. -->
    <div class="ev-when">
      <div class="ev-when-row">
        <div class="ev-date">
          <DatePicker value={dateVal} onChange={(v) => (dateVal = dateOf(v))} withTime={false} placeholder="Pick a date" />
        </div>
        <button
          type="button"
          class={"st-toggle" + (allDay ? " on" : "")}
          role="switch"
          aria-checked={allDay}
          aria-label="All day"
          onclick={() => (allDay = !allDay)}
        >
          <span class="st-knob"></span>
        </button>
        <span class="ev-allday-lbl">All day</span>
      </div>

      {#if !allDay}
        <div class="ev-time-row">
          <input class="input ev-time" type="time" bind:value={startTime} aria-label="Start time" />
          <span class="ev-time-dash">→</span>
          <input class="input ev-time" type="time" bind:value={endTime} aria-label="End time" />
          <span class="ev-time-opt">end optional</span>
        </div>
      {/if}
    </div>

    <!-- KIND — segmented control, active accent matches the kind color. -->
    <div class="ev-seg-block" style:--kind-accent={kindColor(kind)}>
      <div class="seg ev-kind-seg" role="group" aria-label="Type">
        <button type="button" class={"seg-opt" + (kind === "event" ? " on" : "")} onclick={() => (kind = "event")}>Event</button>
        <button type="button" class={"seg-opt" + (kind === "task" ? " on" : "")} onclick={() => (kind = "task")}>Task</button>
        <button type="button" class={"seg-opt" + (isDeadline ? " on" : "")} onclick={setDeadline}>Deadline</button>
      </div>
      {#if isDeadline}
        <div class="seg ev-kind-seg ev-kind-sub" role="group" aria-label="Deadline type">
          {#each DEADLINE_KINDS as dk (dk)}
            <button type="button" class={"seg-opt" + (kind === dk ? " on" : "")} onclick={() => (kind = dk)}>
              {dk.charAt(0).toUpperCase() + dk.slice(1)}
            </button>
          {/each}
        </div>
      {/if}
    </div>

    <!-- ADD DETAILS — progressive disclosure for everything optional. -->
    <button
      type="button"
      class="ev-disclose"
      aria-expanded={showDetails}
      onclick={() => (showDetails = !showDetails)}
    >
      <span class={"ev-disclose-caret" + (showDetails ? " open" : "")}>
        <Icon name="chevron" size={11} />
      </span>
      <span>{showDetails ? "Hide details" : "Add details"}</span>
    </button>

    {#if showDetails}
      <div class="ev-details">
        <label class="ev-field">
          <span class="ev-lbl">Location</span>
          <input bind:value={location} class="input" placeholder="Room, link, place…" />
        </label>

        <div class="ev-field">
          <span class="ev-lbl">Subject</span>
          <Picker
            value={subjectId}
            onChange={(id) => (subjectId = id)}
            options={subjectOptions}
            placeholder="— no subject —"
          />
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
          <span class="ev-lbl">Tags <span class="ev-opt">{isDeadline ? "— topics with these tags become this deadline's checklist" : "(optional)"}</span></span>
          <input bind:value={tagsText} class="input" placeholder="e.g. A2, midterm" />
        </label>

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

        <label class="ev-field">
          <span class="ev-lbl">Description</span>
          <textarea
            bind:value={description}
            class="input ev-textarea"
            rows="3"
            placeholder="Notes…"
          ></textarea>
        </label>
      </div>
    {/if}

    <div class="ev-actions">
      {#if event}
        <button class="btn btn--danger btn--sm ev-del" type="button" onclick={del}>Delete</button>
      {/if}
      <button class="btn btn--ghost btn--sm" type="button" onclick={() => onClose()}>Cancel</button>
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
    width: min(480px, calc(100vw - 40px));
    max-height: calc(100vh - 48px);
    overflow-y: auto;
    background: var(--surface);
    border: 1px solid var(--border-strong);
    border-radius: var(--rad-4);
    box-shadow: var(--shadow-pop);
    padding: 18px;
    animation: ev-pop 0.13s ease;
  }
  .ev-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 14px;
  }
  .ev-title {
    font-family: var(--font-mono);
    font-size: var(--t-md);
    font-weight: 600;
    color: var(--fg-bright);
  }

  /* Title input — large, the visual focal point. */
  .ev-title-input {
    width: 100%;
    font-size: var(--t-xl);
    font-weight: 600;
    padding: 10px 12px;
    height: auto;
  }
  .ev-title-input::placeholder { font-weight: 400; }

  /* WHEN block — a soft grouped panel. */
  .ev-when {
    margin-top: 12px;
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 12px;
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: var(--rad-3);
  }
  .ev-when-row {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .ev-date { flex: 1; min-width: 0; }
  .ev-date :global(.dp) { width: 100%; }
  .ev-allday-lbl {
    font-size: var(--t-xs);
    color: var(--fg-muted);
    user-select: none;
    flex: none;
  }
  .ev-time-row {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .ev-time {
    width: 120px;
    flex: none;
    font-family: var(--font-mono);
    font-variant-numeric: tabular-nums;
  }
  .ev-time-dash { color: var(--fg-faint); flex: none; }
  .ev-time-opt {
    font-size: var(--t-2xs);
    color: var(--fg-faint);
    margin-left: auto;
    flex: none;
  }

  /* KIND segmented control — active pill takes the kind's accent. */
  .ev-seg-block {
    margin-top: 12px;
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 8px;
  }
  .ev-kind-seg { flex: none; }
  .ev-kind-seg :global(.seg-opt.on) {
    background: color-mix(in oklab, var(--kind-accent) 24%, var(--surface-3));
    color: var(--fg-bright);
    box-shadow: inset 0 0 0 1px color-mix(in oklab, var(--kind-accent) 55%, transparent);
  }
  .ev-kind-sub { margin-left: 2px; }

  /* Details expander toggle. */
  .ev-disclose {
    margin-top: 14px;
    display: inline-flex;
    align-items: center;
    gap: 7px;
    background: none;
    border: none;
    padding: 4px 0;
    cursor: pointer;
    font-family: var(--font-mono);
    font-size: var(--t-xs);
    color: var(--fg-muted);
  }
  .ev-disclose:hover { color: var(--fg-bright); }
  .ev-disclose-caret {
    display: inline-flex;
    color: var(--fg-faint);
    transition: transform 0.14s ease;
  }
  .ev-disclose-caret.open { transform: rotate(90deg); }

  /* Details block — divider + grouped optional fields. */
  .ev-details {
    margin-top: 8px;
    padding-top: 14px;
    border-top: 1px solid var(--border);
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .ev-field {
    display: block;
  }
  .ev-lbl {
    display: block;
    margin-bottom: 6px;
    font-size: var(--t-2xs);
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
    opacity: 0.8;
  }
  .ev-field .input { width: 100%; }
  .ev-field :global(.picker) { width: 100%; }
  .ev-textarea {
    resize: vertical;
    min-height: 56px;
    font-family: inherit;
    line-height: 1.45;
    padding-top: 7px;
    padding-bottom: 7px;
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
    border-radius: var(--rad-3);
    border: 2px solid transparent;
    cursor: pointer;
    padding: 0;
    transition: transform 0.1s ease, border-color 0.1s ease;
  }
  .swatch:hover { transform: scale(1.12); }
  .swatch.on { border-color: var(--fg-bright); }
  .swatch-clear {
    display: flex;
    align-items: center;
    justify-content: center;
    background: var(--surface-2);
    border: 1px solid var(--border-strong);
  }
  .swatch-x { color: var(--fg-faint); font-size: 14px; line-height: 1; }

  .ev-actions {
    margin-top: 18px;
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }
  .ev-del { margin-right: auto; }

  @keyframes ev-fade {
    from { opacity: 0; }
    to { opacity: 1; }
  }
  @keyframes ev-pop {
    from { opacity: 0; transform: translateY(6px) scale(0.98); }
    to { opacity: 1; transform: none; }
  }
</style>
