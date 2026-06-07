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
}

const ATTR_GROUP = "data-reorder-group";
const ATTR_INDEX = "data-reorder-index";

/**
 * Svelte action: `use:reorderable={{ index, group, onReorder }}`.
 *
 * Press-and-drag to reorder. A press that never moves past the threshold is
 * left alone so the element's own click handler still fires (cards open on
 * click). A real drag shows a dimmed source, a drop indicator on the hovered
 * sibling, and a small floating preview that tracks the pointer.
 */
export function reorderable(node: HTMLElement, opts: ReorderOpts) {
  let { index, group, onReorder, threshold = 6 } = opts;
  node.setAttribute(ATTR_GROUP, group);
  node.setAttribute(ATTR_INDEX, String(index));
  node.style.touchAction = node.style.touchAction || "none";

  let startX = 0;
  let startY = 0;
  let lastTestX = -999;
  let lastTestY = -999;
  let pressing = false;
  let dragging = false;
  let preview: HTMLElement | null = null;
  let overEl: HTMLElement | null = null;
  let overIndex = -1;

  function clearOver() {
    overEl?.classList.remove("reorder-over");
    overEl = null;
    overIndex = -1;
  }

  function teardown() {
    if (preview) { preview.remove(); preview = null; }
    node.classList.remove("reorder-dragging");
    clearOver();
    document.body.classList.remove("reorder-active");
    pressing = false;
    dragging = false;
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
    if (e.button !== 0 || !e.isPrimary) return;
    pressing = true;
    startX = e.clientX;
    startY = e.clientY;
    try { node.setPointerCapture(e.pointerId); } catch { /* non-fatal */ }
  }

  function onPointerMove(e: PointerEvent) {
    if (!pressing) return;
    if (!dragging) {
      if (Math.abs(e.clientX - startX) < threshold && Math.abs(e.clientY - startY) < threshold) return;
      startDrag();
    }
    if (preview) {
      preview.style.transform = `translate(${e.clientX - startX}px, ${e.clientY - startY}px)`;
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

  return {
    update(next: ReorderOpts) {
      index = next.index;
      group = next.group;
      onReorder = next.onReorder;
      threshold = next.threshold ?? 6;
      node.setAttribute(ATTR_GROUP, group);
      node.setAttribute(ATTR_INDEX, String(index));
    },
    destroy() {
      node.removeEventListener("pointerdown", onPointerDown);
      node.removeEventListener("pointermove", onPointerMove);
      node.removeEventListener("pointerup", onPointerUp);
      node.removeEventListener("pointercancel", onPointerCancel);
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
