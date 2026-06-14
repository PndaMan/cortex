<script lang="ts">
  // Notification centre — a themed right-side drawer aggregating Moodle
  // announcements + upcoming deadlines across all linked courses. Read-state is
  // tracked per-device (store → localStorage); opening an item marks it read.
  import { app } from "../lib/store.svelte";
  import type { NotifItem } from "../lib/store.svelte";
  import Icon from "./Icon.svelte";

  // Two groups: things coming up (deadlines/exams, soonest first) and recent
  // announcements (newest first).
  const upcoming = $derived(
    app.notifications.filter((n) => n.kind !== "announcement").sort((a, b) => a.ts - b.ts)
  );
  const announcements = $derived(
    app.notifications.filter((n) => n.kind === "announcement").sort((a, b) => b.ts - a.ts)
  );
  const isRead = (n: NotifItem) => !!app.notifReadIds[n.id];

  function when(ts: number): string {
    const diff = ts - Date.now();
    const abs = Math.abs(diff);
    const day = 86400000;
    const fut = diff > 0;
    if (abs < 3600000) { const m = Math.max(1, Math.round(abs / 60000)); return fut ? `in ${m}m` : `${m}m ago`; }
    if (abs < day) { const h = Math.round(abs / 3600000); return fut ? `in ${h}h` : `${h}h ago`; }
    const d = Math.round(abs / day);
    if (fut) return d === 1 ? "tomorrow" : `in ${d}d`;
    return d === 1 ? "yesterday" : `${d}d ago`;
  }
  const iconFor = (k: NotifItem["kind"]) => (k === "announcement" ? "chat" : k === "exam" ? "star" : "calendar");

  function activate(n: NotifItem) {
    app.markNotificationRead(n.id);
    app.openDetail(n); // themed in-app reader (with an "Open in Moodle" link)
  }

  function onKey(e: KeyboardEvent) {
    if (!app.notifOpen) return;
    if (e.key === "Escape") { e.preventDefault(); e.stopPropagation(); app.notifOpen = false; }
  }
</script>

<svelte:window onkeydown={onKey} />

{#if app.notifOpen}
  <div class="nc-back" role="presentation" onmousedown={() => (app.notifOpen = false)}>
    <div class="nc" role="dialog" aria-modal="true" aria-label="Notifications" tabindex="-1" onmousedown={(e) => e.stopPropagation()}>
      <header class="nc-head">
        <Icon name="bell" size={15} />
        <span class="nc-title">Notifications</span>
        {#if app.unreadCount > 0}<span class="nc-count">{app.unreadCount}</span>{/if}
        <div class="grow"></div>
        <button class="btn btn--icon btn--sm btn--ghost" title="Sync now" aria-label="Sync now" onclick={() => app.syncMoodle()} disabled={app.moodleSyncing}>
          <Icon name="refresh" size={13} />
        </button>
        <button class="btn btn--icon btn--sm btn--ghost" title="Close" aria-label="Close" onclick={() => (app.notifOpen = false)}>
          <Icon name="x" size={14} />
        </button>
      </header>

      {#if app.unreadCount > 0}
        <div class="nc-actions">
          <button class="nc-markall" onclick={() => app.markAllNotificationsRead()}>
            <Icon name="check" size={11} /> Mark all as read
          </button>
        </div>
      {/if}

      <div class="nc-body">
        {#if app.notifications.length === 0}
          <div class="nc-empty">
            <Icon name="bell" size={26} color="var(--fg-faint)" />
            <p>You're all caught up.</p>
            <span class="nc-empty-sub mono">{app.moodleData.courses.length ? "No announcements or deadlines." : "Connect your portal in Settings → Experimental."}</span>
          </div>
        {:else}
          {#if upcoming.length}
            <div class="nc-group mono">Due soon</div>
            {#each upcoming as n (n.id)}
              {@render item(n)}
            {/each}
          {/if}
          {#if announcements.length}
            <div class="nc-group mono">Announcements</div>
            {#each announcements as n (n.id)}
              {@render item(n)}
            {/each}
          {/if}
        {/if}
      </div>
    </div>
  </div>
{/if}

{#snippet item(n: NotifItem)}
  <button class="nc-item" class:unread={!isRead(n)} onclick={() => activate(n)}>
    <span class="nc-dot" class:on={!isRead(n)} aria-hidden="true"></span>
    <span class="nc-ic"><Icon name={iconFor(n.kind)} size={13} /></span>
    <span class="nc-main">
      <span class="nc-item-title">{n.title}</span>
      <span class="nc-item-sub">{n.course}{n.course && n.kind !== "announcement" ? " · " : ""}{n.kind === "exam" ? "exam" : ""}</span>
    </span>
    <span class="nc-when mono" class:soon={n.kind !== "announcement" && n.ts - Date.now() < 3 * 86400000 && n.ts > Date.now()}>{when(n.ts)}</span>
  </button>
{/snippet}

<style>
  .nc-back {
    position: fixed; inset: 0; z-index: 180;
    background: color-mix(in oklab, var(--bg) 45%, transparent);
    animation: nc-fade 0.12s ease;
  }
  .nc {
    position: absolute; top: 0; right: 0; height: 100%;
    width: min(400px, 92vw);
    display: flex; flex-direction: column;
    background: var(--surface); border-left: 1px solid var(--border-strong);
    box-shadow: -16px 0 48px rgba(0, 0, 0, 0.4);
    animation: nc-slide 0.16s cubic-bezier(0.22, 0.61, 0.36, 1);
  }
  .nc-head { display: flex; align-items: center; gap: 9px; padding: 15px 16px; border-bottom: 1px solid var(--border); color: var(--fg-bright); }
  .nc-title { font-weight: 600; font-size: var(--t-md, 14px); }
  .nc-count {
    background: var(--accent); color: var(--accent-fg); font-size: 11px; font-weight: 700;
    min-width: 18px; height: 18px; padding: 0 5px; border-radius: 999px;
    display: inline-flex; align-items: center; justify-content: center; line-height: 1;
  }
  .grow { flex: 1; }

  .nc-actions { padding: 8px 16px; border-bottom: 1px solid var(--border); }
  .nc-markall {
    display: inline-flex; align-items: center; gap: 6px;
    background: none; border: none; color: var(--accent); cursor: pointer;
    font-size: 12px; font-family: var(--font-mono); padding: 2px 0;
  }
  .nc-markall:hover { text-decoration: underline; }

  .nc-body { flex: 1; overflow-y: auto; padding: 6px 0 16px; }

  .nc-group {
    font-size: var(--t-2xs, 10.5px); text-transform: uppercase; letter-spacing: 0.1em;
    color: var(--fg-faint); padding: 14px 16px 6px;
  }
  .nc-item {
    display: flex; align-items: flex-start; gap: 9px; width: 100%;
    padding: 9px 16px; text-decoration: none; color: inherit; text-align: left;
    background: none; border: none; border-left: 2px solid transparent; cursor: pointer;
    font: inherit; transition: background 0.1s;
  }
  .nc-item:hover { background: var(--surface-2); }
  .nc-item.unread { border-left-color: var(--accent); background: color-mix(in oklab, var(--accent) 6%, transparent); }
  .nc-item.unread:hover { background: color-mix(in oklab, var(--accent) 11%, transparent); }

  .nc-dot { flex: none; width: 7px; height: 7px; border-radius: 50%; margin-top: 6px; background: transparent; }
  .nc-dot.on { background: var(--accent); }
  .nc-ic { flex: none; margin-top: 1px; color: var(--fg-faint); }
  .nc-main { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 2px; }
  .nc-item-title {
    font-size: 13px; color: var(--fg); line-height: 1.35;
    display: -webkit-box; -webkit-line-clamp: 2; line-clamp: 2; -webkit-box-orient: vertical; overflow: hidden;
  }
  .nc-item.unread .nc-item-title { color: var(--fg-bright); font-weight: 500; }
  .nc-item-sub { font-size: 11px; color: var(--fg-faint); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .nc-when { flex: none; font-size: 11px; color: var(--fg-faint); white-space: nowrap; margin-top: 1px; }
  .nc-when.soon { color: var(--warn, #e0a458); }

  .nc-empty { display: flex; flex-direction: column; align-items: center; gap: 10px; text-align: center; padding: 64px 28px; color: var(--fg-faint); }
  .nc-empty p { margin: 0; color: var(--fg); font-size: 14px; }
  .nc-empty-sub { font-size: 11px; }

  @keyframes nc-fade { from { opacity: 0; } to { opacity: 1; } }
  @keyframes nc-slide { from { transform: translateX(100%); } to { transform: none; } }
</style>
