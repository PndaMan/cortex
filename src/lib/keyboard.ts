// Helix-style modal keyboard engine (foundation subset). Mirrors the prototype's
// engine in app/main.jsx: ':' command palette, 'g' prefix, dashboard j/k/h/l nav,
// 'i' insert, Esc → Normal. Mouse paths exist for every action elsewhere.

import { app } from "./store.svelte";

export function installKeyboard(): () => void {
  let gPrefix = false;
  let gTimer: ReturnType<typeof setTimeout> | null = null;

  function onKey(e: KeyboardEvent) {
    const el = document.activeElement as HTMLElement | null;
    const typing =
      !!el &&
      (el.tagName === "INPUT" ||
        el.tagName === "TEXTAREA" ||
        el.isContentEditable);

    if (e.key === "Escape") {
      if (app.cmdkOpen) app.cmdkOpen = false;
      if (typing) el?.blur();
      app.setMode("NOR");
      return;
    }
    if (typing) return;

    // command palette owns its own keys while open
    if (app.cmdkOpen) return;

    if ((e.ctrlKey || e.metaKey) && (e.key === "p" || e.key === "P")) {
      e.preventDefault();
      app.cmdkOpen = true;
      return;
    }
    if (e.metaKey || e.ctrlKey || e.altKey) return;

    if (e.key === ":") {
      e.preventDefault();
      app.cmdkOpen = true;
      return;
    }
    if (e.key === "q" && app.toasts.length) {
      e.preventDefault();
      app.dismissToast(app.toasts[app.toasts.length - 1].id);
      return;
    }

    // g-prefix (gd → dashboard, gg → top)
    if (gPrefix) {
      gPrefix = false;
      if (e.key === "d") {
        app.setView("dashboard");
        return;
      }
      if (e.key === "g") {
        document.querySelector(".workspace-scroll")?.scrollTo({ top: 0 });
        return;
      }
    }
    if (e.key === "g") {
      gPrefix = true;
      if (gTimer) clearTimeout(gTimer);
      gTimer = setTimeout(() => (gPrefix = false), 600);
      return;
    }

    if (e.key === "n") {
      e.preventDefault();
      app.setView("add-source");
      return;
    }
    if (e.key === "t") {
      app.cycleTheme();
      return;
    }
    if (e.key === "i") {
      const ta = document.querySelector<HTMLTextAreaElement>("textarea, input");
      if (ta) {
        e.preventDefault();
        ta.focus();
        app.setMode("INS");
      }
      return;
    }

    // dashboard grid navigation
    if (app.view === "dashboard") {
      const n = app.subjects.length;
      if (e.key === "j" || e.key === "ArrowDown" || e.key === "l" || e.key === "ArrowRight") {
        e.preventDefault();
        app.dashFocus = Math.min(n - 1, app.dashFocus + 1);
      } else if (e.key === "k" || e.key === "ArrowUp" || e.key === "h" || e.key === "ArrowLeft") {
        e.preventDefault();
        app.dashFocus = Math.max(0, app.dashFocus - 1);
      } else if (e.key === "Enter" && app.subjects[app.dashFocus]) {
        e.preventDefault();
        app.openSubject(app.subjects[app.dashFocus].id);
      }
    }
  }

  window.addEventListener("keydown", onKey);
  return () => window.removeEventListener("keydown", onKey);
}
