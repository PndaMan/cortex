<script lang="ts">
  // OS-adaptive custom title bar. The window is frameless (decorations:false),
  // so this thin strip provides the drag region + window controls, laid out to
  // match each platform's convention:
  //   • macOS   → traffic-light style controls on the LEFT
  //   • Windows → square min / maximize / close controls on the RIGHT
  //   • Linux   → same right-side controls (GNOME/KDE default to the right)
  import { getCurrentWindow } from "@tauri-apps/api/window";

  // Cheap, dependency-free platform detection from the webview UA. Good enough
  // to pick a control layout (WKWebView / WebView2 / WebKitGTK all report it).
  const ua = navigator.userAgent;
  const isMac = /Mac/i.test(ua);
  const isWin = /Win/i.test(ua);
  const platform: "mac" | "win" | "linux" = isMac ? "mac" : isWin ? "win" : "linux";

  const win = getCurrentWindow();
  let maximized = $state(false);

  // Keep the maximize/restore glyph in sync with the actual window state.
  $effect(() => {
    let un: (() => void) | undefined;
    win.isMaximized().then((m) => (maximized = m)).catch(() => {});
    win
      .onResized(() => {
        win.isMaximized().then((m) => (maximized = m)).catch(() => {});
      })
      .then((u) => (un = u))
      .catch(() => {});
    return () => un?.();
  });

  const minimize = () => win.minimize().catch(() => {});
  const toggleMax = () => win.toggleMaximize().catch(() => {});
  const close = () => win.close().catch(() => {});
</script>

<div class="tb" class:mac={platform === "mac"} data-tauri-drag-region>
  {#if platform === "mac"}
    <!-- macOS: traffic-light controls, left-aligned. -->
    <div class="tb-mac-ctl">
      <button class="tl tl-close" onclick={close} aria-label="Close" title="Close">
        <svg viewBox="0 0 8 8"><path d="M2 2 L6 6 M6 2 L2 6" /></svg>
      </button>
      <button class="tl tl-min" onclick={minimize} aria-label="Minimize" title="Minimize">
        <svg viewBox="0 0 8 8"><path d="M2 4 H6" /></svg>
      </button>
      <button class="tl tl-max" onclick={toggleMax} aria-label="Zoom" title="Zoom">
        <svg viewBox="0 0 8 8"><path d="M2.5 2.5 H5.5 V5.5 Z M5.5 5.5 H2.5 V2.5 Z" /></svg>
      </button>
    </div>
    <div class="tb-title" data-tauri-drag-region>Cortex</div>
  {:else}
    <!-- Windows / Linux: title left, square controls right. -->
    <div class="tb-title win" data-tauri-drag-region>Cortex</div>
    <div class="tb-win-ctl">
      <button class="wc" onclick={minimize} aria-label="Minimize" title="Minimize">
        <svg viewBox="0 0 10 10"><path d="M1 5 H9" /></svg>
      </button>
      <button class="wc" onclick={toggleMax} aria-label={maximized ? "Restore" : "Maximize"} title={maximized ? "Restore" : "Maximize"}>
        {#if maximized}
          <svg viewBox="0 0 10 10"><path d="M2.5 2.5 H7.5 V7.5 H2.5 Z M2.5 4 H1 V9 H6 V7.5" /></svg>
        {:else}
          <svg viewBox="0 0 10 10"><rect x="1.5" y="1.5" width="7" height="7" /></svg>
        {/if}
      </button>
      <button class="wc wc-close" onclick={close} aria-label="Close" title="Close">
        <svg viewBox="0 0 10 10"><path d="M1.5 1.5 L8.5 8.5 M8.5 1.5 L1.5 8.5" /></svg>
      </button>
    </div>
  {/if}
</div>

<style>
  .tb {
    display: flex;
    align-items: center;
    height: 100%;
    width: 100%;
    background: var(--bg-sunken);
    border-bottom: 1px solid var(--border);
    user-select: none;
    -webkit-user-select: none;
    overflow: hidden;
  }
  .tb-title {
    flex: 1;
    font-family: var(--font-mono);
    font-size: var(--t-2xs, 10.5px);
    letter-spacing: 0.14em;
    text-transform: uppercase;
    color: var(--fg-faint);
    padding: 0 12px;
    line-height: 1;
  }
  /* macOS: title centred; controls take the left. */
  .tb.mac .tb-title { text-align: center; padding-left: 0; }

  /* ---- macOS traffic lights ---- */
  .tb-mac-ctl { display: flex; align-items: center; gap: 8px; padding: 0 12px; }
  .tl {
    width: 12px; height: 12px; border-radius: 50%; border: none; padding: 0;
    display: inline-flex; align-items: center; justify-content: center;
    cursor: default;
  }
  .tl svg { width: 8px; height: 8px; opacity: 0; transition: opacity 0.1s ease; }
  .tl svg path { stroke: rgba(0, 0, 0, 0.55); stroke-width: 1.2; fill: none; stroke-linecap: round; }
  .tb-mac-ctl:hover .tl svg { opacity: 1; }
  .tl-close { background: #ff5f57; }
  .tl-min { background: #febc2e; }
  .tl-max { background: #28c840; }

  /* ---- Windows / Linux controls ---- */
  .tb-win-ctl { display: flex; align-items: stretch; height: 100%; margin-left: auto; }
  .wc {
    width: 44px; height: 100%; border: none; background: transparent; padding: 0;
    display: inline-flex; align-items: center; justify-content: center;
    color: var(--fg-muted); cursor: default;
    transition: background 0.1s ease, color 0.1s ease;
  }
  .wc svg { width: 10px; height: 10px; }
  .wc svg path, .wc svg rect { stroke: currentColor; stroke-width: 1; fill: none; }
  .wc:hover { background: color-mix(in oklab, var(--fg) 12%, transparent); color: var(--fg-bright); }
  .wc-close:hover { background: #e81123; color: #fff; }
</style>
