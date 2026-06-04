// Tiny helper for HTML5 drag-and-drop list reordering: move an item from one
// index to another, returning a new array (originals untouched).
export function moveItem<T>(arr: readonly T[], from: number, to: number): T[] {
  const copy = arr.slice();
  const [it] = copy.splice(from, 1);
  copy.splice(to, 0, it);
  return copy;
}

// A 1×1 transparent drag image. WebKitGTK's default drag ghost is a large,
// device-pixel-scaled snapshot of the element (looks like it "zooms way too
// much"); setting this as the drag image suppresses it so we can show our own
// in-place feedback (dimmed source + accent drop line) instead.
let _emptyImg: HTMLImageElement | null = null;
function emptyImg(): HTMLImageElement {
  if (!_emptyImg) {
    _emptyImg = new Image();
    _emptyImg.src =
      "data:image/gif;base64,R0lGODlhAQABAIAAAAAAAP/// yH5BAEAAAAALAAAAAABAAEAAAIBRAA7".replace(/\s/g, "");
  }
  return _emptyImg;
}

/** Call in a `dragstart` handler to hide the native (zoomed) drag ghost. */
export function hideDragImage(e: DragEvent) {
  try {
    e.dataTransfer?.setDragImage(emptyImg(), 0, 0);
  } catch {
    /* setDragImage unsupported — harmless */
  }
}
