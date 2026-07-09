<script lang="ts">
  // Shared themed reader for a single announcement / deadline / exam. Driven by
  // app.detail and opened from BOTH the notification centre and the subject panel,
  // so the experience is identical everywhere. Sits above every other overlay.
  import { app } from "../lib/store.svelte";
  import * as api from "../lib/api";
  import Icon from "./Icon.svelte";

  const d = $derived(app.detail);
  const subjectName = $derived(
    d?.subjectId ? app.subjects.find((s) => s.id === d.subjectId)?.name ?? null : null
  );

  function fmtFull(ms: number): string {
    if (!ms) return "";
    try {
      return new Date(ms).toLocaleString(undefined, {
        weekday: "short", day: "numeric", month: "short", year: "numeric",
        hour: "2-digit", minute: "2-digit",
      });
    } catch { return ""; }
  }
  function rel(ms: number): string {
    const diff = ms - Date.now();
    const abs = Math.abs(diff), day = 86400000, fut = diff > 0;
    if (abs < 3600000) { const m = Math.max(1, Math.round(abs / 60000)); return fut ? `in ${m}m` : `${m}m ago`; }
    if (abs < day) { const h = Math.round(abs / 3600000); return fut ? `in ${h}h` : `${h}h ago`; }
    const dd = Math.round(abs / day);
    if (fut) return dd === 1 ? "tomorrow" : `in ${dd} days`;
    return dd === 1 ? "yesterday" : `${dd} days ago`;
  }
  // Readable plain text from Moodle's HTML (safer than rendering arbitrary HTML).
  function htmlToText(html: string): string {
    return html
      .replace(/<\s*br\s*\/?>/gi, "\n")
      .replace(/<\/\s*(p|div|li|h[1-6]|tr)\s*>/gi, "\n")
      .replace(/<\s*li[^>]*>/gi, "• ")
      .replace(/<[^>]*>/g, "")
      .replace(/&nbsp;/g, " ").replace(/&amp;/g, "&").replace(/&lt;/g, "<")
      .replace(/&gt;/g, ">").replace(/&#39;/g, "'").replace(/&quot;/g, '"')
      .replace(/[ \t]+/g, " ").replace(/\n{3,}/g, "\n\n").trim();
  }
  const iconFor = (k: string) => (k === "announcement" ? "chat" : k === "exam" ? "star" : "calendar");
  const kindLabel = (k: string) => (k === "announcement" ? "Announcement" : k === "exam" ? "Exam" : "Deadline");

  function openInMoodle() {
    if (!d?.url) return;
    // Surface a failed hand-off to the OS (no browser, blocked scheme, …) as a
    // toast — an unhandled rejection here would just die silently.
    api.openExternal(d.url).catch((e) =>
      app.pushToast({ kind: "error", title: "Couldn't open Moodle", body: String(e) })
    );
  }

  function goToSubject() {
    if (!d?.subjectId) return;
    app.openSubject(d.subjectId);
    app.openSubjectPanel();
    app.closeDetail();
    app.notifOpen = false;
  }

  function onKey(e: KeyboardEvent) {
    if (!app.detail) return;
    e.stopPropagation();
    if (e.key === "Escape") { e.preventDefault(); app.closeDetail(); }
  }
</script>

<svelte:window onkeydown={onKey} />

{#if d}
  <div class="nd-back" role="presentation" onmousedown={(e) => { e.stopPropagation(); app.closeDetail(); }}>
    <div class="nd" role="dialog" aria-modal="true" tabindex="-1" onmousedown={(e) => e.stopPropagation()}>
      <header class="nd-head">
        <span class="nd-ic nd-ic--{d.kind}"><Icon name={iconFor(d.kind)} size={15} /></span>
        <div class="nd-htext">
          <div class="nd-kind mono">{kindLabel(d.kind)}</div>
          <h2 class="nd-title">{d.title}</h2>
        </div>
        <button class="btn btn--icon btn--sm btn--ghost" title="Close" aria-label="Close" onclick={() => app.closeDetail()}>
          <Icon name="x" size={15} />
        </button>
      </header>

      <div class="nd-meta">
        {#if d.course}<span class="nd-chip">{d.course}</span>{/if}
        {#if d.ts}<span class="nd-when mono">{d.kind === "announcement" ? "posted" : "due"} {fmtFull(d.ts)} · {rel(d.ts)}</span>{/if}
      </div>

      <div class="nd-body">
        {#if d.kind === "announcement"}
          {#if d.message.trim()}
            <p class="nd-text read">{htmlToText(d.message)}</p>
          {:else}
            <p class="nd-empty">No content — open it in Moodle for the full post.</p>
          {/if}
        {:else}
          <p class="nd-text read">{kindLabel(d.kind)} {d.ts ? `due ${fmtFull(d.ts)}` : ""}{d.course ? ` for ${d.course}` : ""}.</p>
        {/if}
      </div>

      <footer class="nd-foot">
        {#if subjectName}
          <button class="btn btn--sm btn--ghost" onclick={goToSubject}><Icon name="book" size={12} /> {subjectName}</button>
        {/if}
        <div class="grow"></div>
        {#if d.url}
          <button class="btn btn--sm btn--primary" onclick={openInMoodle}><Icon name="external" size={12} /> Open in Moodle</button>
        {/if}
      </footer>
    </div>
  </div>
{/if}

<style>
  .nd-back {
    position: fixed; inset: 0; z-index: 240;
    display: flex; align-items: center; justify-content: center;
    background: color-mix(in oklab, var(--bg) 66%, transparent);
    backdrop-filter: blur(3px); animation: nd-fade 0.12s ease;
  }
  .nd {
    width: min(620px, calc(100vw - 48px)); max-height: calc(100vh - 80px);
    display: flex; flex-direction: column;
    background: var(--surface); border: 1px solid var(--border-strong);
    border-radius: var(--r-lg, 12px); box-shadow: 0 18px 50px rgba(0, 0, 0, 0.5);
    animation: nd-pop 0.13s ease;
  }
  .nd-head { display: flex; align-items: flex-start; gap: 11px; padding: 16px 18px 12px; }
  .nd-ic {
    flex: none; width: 30px; height: 30px; border-radius: var(--rad-2, 6px);
    display: inline-flex; align-items: center; justify-content: center;
    background: var(--surface-2); color: var(--fg-faint);
  }
  .nd-ic--announcement { color: var(--accent); }
  .nd-ic--exam { color: var(--warn, #e0a458); }
  .nd-htext { flex: 1; min-width: 0; }
  .nd-kind { font-size: var(--t-2xs, 10.5px); text-transform: uppercase; letter-spacing: 0.1em; color: var(--fg-faint); }
  .nd-title { margin: 2px 0 0; font-size: var(--t-lg, 17px); font-weight: 600; color: var(--fg-bright); line-height: 1.25; }
  .nd-meta { display: flex; flex-wrap: wrap; align-items: center; gap: 8px; padding: 0 18px 12px; border-bottom: 1px solid var(--border); }
  .nd-chip { background: var(--surface-2); border: 1px solid var(--border); border-radius: var(--rad-pill, 999px); padding: 2px 10px; font-size: 11px; color: var(--fg); }
  .nd-when { font-size: 11px; color: var(--fg-faint); }
  .nd-body { padding: 14px 18px; overflow-y: auto; }
  .nd-text { margin: 0; white-space: pre-wrap; word-break: break-word; line-height: 1.65; color: var(--fg); font-size: 13.5px; }
  .nd-empty { margin: 0; color: var(--fg-faint); font-style: italic; }
  .nd-foot { display: flex; align-items: center; gap: 8px; padding: 12px 18px; border-top: 1px solid var(--border); }
  .grow { flex: 1; }
  @keyframes nd-fade { from { opacity: 0; } to { opacity: 1; } }
  @keyframes nd-pop { from { opacity: 0; transform: translateY(6px) scale(0.985); } to { opacity: 1; transform: none; } }
</style>
