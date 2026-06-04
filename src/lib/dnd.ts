// Tiny helper for HTML5 drag-and-drop list reordering: move an item from one
// index to another, returning a new array (originals untouched).
export function moveItem<T>(arr: readonly T[], from: number, to: number): T[] {
  const copy = arr.slice();
  const [it] = copy.splice(from, 1);
  copy.splice(to, 0, it);
  return copy;
}
