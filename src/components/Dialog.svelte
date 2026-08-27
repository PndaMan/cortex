<script lang="ts">
  // Themed confirm/prompt dialog — the Design-System replacement for the native
  // window.confirm / window.prompt. Driven entirely by app.dialog; resolves via
  // app.resolveDialog. Mounted once globally in App.svelte.
  import { app } from "../lib/store.svelte";
  import { translateText } from "../lib/i18n";

  let inputEl = $state<HTMLInputElement | null>(null);
  let draft = $state("");

  // Seed the prompt input whenever a new dialog opens, and focus it.
  $effect(() => {
    const d = app.dialog;
    if (d?.kind === "prompt") {
      draft = d.value ?? "";
      // focus after render
      queueMicrotask(() => {
        inputEl?.focus();
        inputEl?.select();
      });
    }
  });

  function cancel() {
    app.resolveDialog(app.dialog?.kind === "prompt" ? null : false);
  }
  function ok() {
    if (app.dialog?.kind === "prompt") {
      const v = draft.trim();
      app.resolveDialog(v.length ? v : null);
    } else {
      app.resolveDialog(true);
    }
  }
  function onKey(e: KeyboardEvent) {
    if (!app.dialog) return;
    e.stopPropagation();
    if (e.key === "Escape") { e.preventDefault(); cancel(); }
    else if (e.key === "Enter") { e.preventDefault(); ok(); }
  }
</script>

<!-- capture keys so the global keyboard engine doesn't act on them -->
<svelte:window onkeydown={onKey} />

{#if app.dialog}
  <div class="dlg-back" onmousedown={cancel} role="presentation">
    <div class="dlg" role="dialog" aria-modal="true" tabindex="-1" onmousedown={(e) => e.stopPropagation()}>
      <div class="dlg-title">{translateText(app.dialog.title, app.language)}</div>
      {#if app.dialog.body}
        <div class="dlg-body">{translateText(app.dialog.body, app.language)}</div>
      {/if}
      {#if app.dialog.kind === "prompt"}
        {#if app.dialog.label}
          <div class="dlg-label">{translateText(app.dialog.label, app.language)}</div>
        {/if}
        <input
          bind:this={inputEl}
          bind:value={draft}
          class="input dlg-input"
          placeholder={translateText(app.dialog.placeholder ?? "", app.language)}
        />
      {/if}
      <div class="dlg-actions">
        <button class="btn btn--ghost btn--sm" type="button" onclick={cancel}>Cancel</button>
        <button
          class={"btn btn--sm " + (app.dialog.danger ? "btn--danger" : "btn--primary")}
          type="button"
          onclick={ok}
        >
          {translateText(app.dialog.okLabel ?? "OK", app.language)}
        </button>
      </div>
    </div>
  </div>
{/if}

<style>
  .dlg-back {
    position: fixed;
    inset: 0;
    /* Topmost overlay: a confirm must sit above every surface that can trigger
       it — EditModal (200), framework viewer (210), notification detail (240),
       and the fullscreen notes editor (900) — regardless of DOM order. */
    z-index: 1000;
    display: flex;
    align-items: center;
    justify-content: center;
    background: color-mix(in oklab, var(--bg) 62%, transparent);
    backdrop-filter: blur(3px);
    animation: dlg-fade 0.12s ease;
  }
  .dlg {
    width: min(440px, calc(100vw - 48px));
    background: var(--surface);
    border: 1px solid var(--border-strong);
    border-radius: var(--r-lg, 12px);
    box-shadow: 0 18px 50px rgba(0, 0, 0, 0.5);
    padding: 20px;
    animation: dlg-pop 0.13s ease;
  }
  .dlg-title {
    font-family: var(--font-mono);
    font-size: var(--t-md, 14px);
    font-weight: 600;
    color: var(--fg-bright);
  }
  .dlg-body {
    margin-top: 8px;
    font-family: var(--font-mono);
    font-size: var(--t-sm, 12.5px);
    line-height: 1.5;
    color: var(--fg-muted);
  }
  .dlg-label {
    margin-top: 14px;
    margin-bottom: 6px;
    font-size: var(--t-2xs, 10.5px);
    font-weight: 600;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: var(--fg-faint);
  }
  .dlg-input {
    margin-top: 12px;
    width: 100%;
  }
  .dlg-actions {
    margin-top: 18px;
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }
  @keyframes dlg-fade { from { opacity: 0; } to { opacity: 1; } }
  @keyframes dlg-pop { from { opacity: 0; transform: translateY(6px) scale(0.98); } to { opacity: 1; transform: none; } }
</style>
