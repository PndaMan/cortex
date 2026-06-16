// Anchor a picker menu to its trigger as a VIEWPORT-FIXED box, so no overflow:hidden /
// scroll ancestor (e.g. the settings table) can clip it. Flips up when there's more room
// above, and shifts left to stay fully on screen.

export type MenuPos = {
  left: number;
  width: number;
  up: boolean;
  top: number | null;
  bottom: number | null;
};

export function menuPosition(rect: DOMRect, minWidth = 0, gap = 5, margin = 8): MenuPos {
  const vw = window.innerWidth;
  const vh = window.innerHeight;
  const width = Math.min(Math.max(rect.width, minWidth), vw - margin * 2);
  let left = rect.left;
  if (left + width > vw - margin) left = vw - margin - width; // shift on-screen (right edge)
  if (left < margin) left = margin; // …and left edge
  const below = vh - rect.bottom;
  const above = rect.top;
  const up = below < 240 && above > below;
  return {
    left,
    width,
    up,
    top: up ? null : rect.bottom + gap,
    bottom: up ? vh - rect.top + gap : null,
  };
}

/** Inline style for a fixed menu positioned by menuPosition(). */
export function menuStyle(p: MenuPos): string {
  return `left:${p.left}px; width:${p.width}px; ${p.up ? `bottom:${p.bottom}px` : `top:${p.top}px`}`;
}
