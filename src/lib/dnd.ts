// List reordering for Cortex.
//
// We deliberately do NOT use the HTML5 drag-and-drop API. On WebKitGTK two
// things go wrong with it: (1) the native drag ghost is a device-pixel-scaled
// snapshot that looks absurdly zoomed, and (2) calling `dataTransfer.setDragImage`
// to suppress that ghost crashes the WebKit web process (SIGSEGV → the whole
// app dies). So reordering is implemented with plain pointer events plus our own
// floating preview element, which we fully control.

// Move an item from one index to another, returning a new array (originals
// untouched).
export function moveItem<T>(arr: readonly T[], from: number, to: number): T[] {
  const copy = arr.slice();
  const [it] = copy.splice(from, 1);
  copy.splice(to, 0, it);
  return copy;
}

export interface ReorderOpts {
  /** This item's position in its list. */
  index: number;
  /** Shared id for every item in the same list (e.g. "subjects", "topics:<id>"). */
  group: string;
  /** Called on drop with the original and target indices (from !== to). */
  onReorder: (from: number, to: number) => void;
  /** Pixels of movement before a press becomes a drag (lets clicks through). */
  threshold?: number;
  /** Touch only: hold this long (ms) before a drag arms, so scrolling a list
   *  never accidentally reorders. Mouse drags arm immediately (threshold). */
  holdMs?: number;
}

const ATTR_GROUP = "data-reorder-group";
const ATTR_INDEX = "data-reorder-index";

/**
 * Svelte action: `use:reorderable={{ index, group, onReorder }}`.
 *
 * Mouse: press-and-drag past a small threshold to reorder (clicks still pass
 * through). Touch: native scrolling works normally, and a drag only arms after a
 * long-press (hold) — if the finger moves first it's treated as a scroll, so you
 * can't accidentally reorder while scrolling. A real drag shows a dimmed source,
 * a drop indicator on the hovered sibling, and a floating preview.
 */
export function reorderable(node: HTMLElement, opts: ReorderOpts) {
  let { index, group, onReorder, threshold = 6, holdMs = 380 } = opts;
  node.setAttribute(ATTR_GROUP, group);
  node.setAttribute(ATTR_INDEX, String(index));
  // Native scrolling stays enabled (no touch-action:none). On touch we block the
  // scroll ourselves — via the non-passive touchmove below — only once a drag is
  // actually armed by a long-press, so normal list scrolling is never hijacked.

  let startX = 0;
  let startY = 0;
  let lastTestX = -999;
  let lastTestY = -999;
  let pressing = false;
  let dragging = false;
  let armed = false; // true once a drag is allowed (immediately for mouse, after hold for touch)
  let isTouch = false;
  let holdTimer: ReturnType<typeof setTimeout> | null = null;
  let preview: HTMLElement | null = null;
  let overEl: HTMLElement | null = null;
  let overIndex = -1;

  function cancelHold() {
    if (holdTimer) { clearTimeout(holdTimer); holdTimer = null; }
  }

  function clearOver() {
    overEl?.classList.remove("reorder-over");
    overEl = null;
    overIndex = -1;
  }

  function teardown() {
    cancelHold();
    if (preview) { preview.remove(); preview = null; }
    node.classList.remove("reorder-dragging");
    clearOver();
    document.body.classList.remove("reorder-active");
    pressing = false;
    dragging = false;
    armed = false;
  }

  function startDrag() {
    dragging = true;
    node.classList.add("reorder-dragging");
    document.body.classList.add("reorder-active");
    const r = node.getBoundingClientRect();
    preview = node.cloneNode(true) as HTMLElement;
    preview.removeAttribute(ATTR_GROUP);
    preview.classList.add("reorder-preview");
    preview.style.position = "fixed";
    preview.style.left = `${r.left}px`;
    preview.style.top = `${r.top}px`;
    preview.style.width = `${r.width}px`;
    preview.style.height = `${r.height}px`;
    preview.style.margin = "0";
    preview.style.pointerEvents = "none";
    preview.style.zIndex = "9999";
    document.body.appendChild(preview);
  }

  function onPointerDown(e: PointerEvent) {
    if (!e.isPrimary) return;
    if (e.pointerType !== "touch" && e.button !== 0) return;
    // Don't hijack presses that land on an interactive control INSIDE the row
    // (edit/add/delete buttons, the expand twisty). Capturing the pointer there
    // makes WebKitGTK retarget the synthesized click to the capturing row, so
    // the control's own handler never runs and the icons appear dead. The row
    // itself carries role="button", so only bail for a nested interactive el.
    const el = e.target as HTMLElement | null;
    if (el && el !== node) {
      const interactive = el.closest("button, [role='button'], a, input, select, textarea");
      if (interactive && interactive !== node && node.contains(interactive)) return;
    }
    isTouch = e.pointerType === "touch";
    pressing = true;
    armed = !isTouch; // mouse arms immediately; touch arms only after the hold below
    startX = e.clientX;
    startY = e.clientY;
    lastTestX = -999;
    lastTestY = -999;
    if (isTouch) {
      cancelHold();
      const pid = e.pointerId;
      holdTimer = setTimeout(() => {
        holdTimer = null;
        if (!pressing) return;
        armed = true;
        try { node.setPointerCapture(pid); } catch { /* non-fatal */ }
        startDrag(); // lift the card so the long-press is felt
      }, holdMs);
    } else {
      try { node.setPointerCapture(e.pointerId); } catch { /* non-fatal */ }
    }
  }

  function onPointerMove(e: PointerEvent) {
    if (!pressing) return;
    const dx = e.clientX - startX;
    const dy = e.clientY - startY;
    if (!armed) {
      // Touch, before the long-press completes: real movement = the user is
      // scrolling, not reordering — abort the press and let the page scroll.
      if (Math.abs(dx) > 10 || Math.abs(dy) > 10) {
        cancelHold();
        pressing = false;
      }
      return;
    }
    if (!dragging) {
      if (Math.abs(dx) < threshold && Math.abs(dy) < threshold) return;
      startDrag();
    }
    if (preview) {
      preview.style.transform = `translate(${dx}px, ${dy}px)`;
    }
    // Hit-testing (elementFromPoint + closest) forces layout, so skip it until
    // the pointer has actually moved a few px since the last test.
    if (Math.abs(e.clientX - lastTestX) < 4 && Math.abs(e.clientY - lastTestY) < 4) return;
    lastTestX = e.clientX;
    lastTestY = e.clientY;
    // Find the sibling under the pointer (the preview ignores hit-testing).
    const under = document.elementFromPoint(e.clientX, e.clientY) as HTMLElement | null;
    const target = under?.closest<HTMLElement>(`[${ATTR_GROUP}="${cssEscape(group)}"]`) ?? null;
    if (target !== overEl) {
      clearOver();
      if (target && target !== node) {
        overEl = target;
        overIndex = Number(target.getAttribute(ATTR_INDEX));
        overEl.classList.add("reorder-over");
      }
    }
  }

  // Stop the page from scrolling ONLY while an armed touch-drag is in progress.
  // Non-passive so preventDefault actually takes effect; a no-op otherwise, so
  // normal scrolling over the card is untouched.
  function onTouchMove(e: TouchEvent) {
    if (dragging && isTouch && e.cancelable) e.preventDefault();
  }

  function onPointerUp() {
    if (dragging && overIndex >= 0 && overIndex !== index) {
      const from = index;
      const to = overIndex;
      // Suppress the click that a pointerup would otherwise synthesize so a
      // drag doesn't also "open" the item.
      suppressNextClick();
      onReorder(from, to);
    }
    teardown();
  }

  function onPointerCancel() { teardown(); }

  node.addEventListener("pointerdown", onPointerDown);
  node.addEventListener("pointermove", onPointerMove);
  node.addEventListener("pointerup", onPointerUp);
  node.addEventListener("pointercancel", onPointerCancel);
  node.addEventListener("touchmove", onTouchMove, { passive: false });

  return {
    update(next: ReorderOpts) {
      index = next.index;
      group = next.group;
      onReorder = next.onReorder;
      threshold = next.threshold ?? 6;
      holdMs = next.holdMs ?? 380;
      node.setAttribute(ATTR_GROUP, group);
      node.setAttribute(ATTR_INDEX, String(index));
    },
    destroy() {
      node.removeEventListener("pointerdown", onPointerDown);
      node.removeEventListener("pointermove", onPointerMove);
      node.removeEventListener("pointerup", onPointerUp);
      node.removeEventListener("pointercancel", onPointerCancel);
      node.removeEventListener("touchmove", onTouchMove);
      teardown();
    },
  };
}

// One-shot capturing click swallow, so the click synthesized after a drag's
// pointerup doesn't trigger the element's onclick (e.g. opening a subject).
function suppressNextClick() {
  const swallow = (e: MouseEvent) => {
    e.preventDefault();
    e.stopPropagation();
    window.removeEventListener("click", swallow, true);
  };
  window.addEventListener("click", swallow, true);
  // Safety: if no click arrives, drop the listener shortly after.
  setTimeout(() => window.removeEventListener("click", swallow, true), 350);
}

// Minimal CSS.escape fallback for attribute selectors (group ids can contain ':').
function cssEscape(s: string): string {
  // @ts-ignore - CSS.escape exists in WebKit
  if (typeof CSS !== "undefined" && CSS.escape) return CSS.escape(s);
  return s.replace(/["\\]/g, "\\$&");
}
