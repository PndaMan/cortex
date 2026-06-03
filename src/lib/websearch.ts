// Module-level cache so the in-app web browser (WebSearch.svelte) keeps its
// tabs, history, and results when you navigate away and come back — it behaves
// like a real browser rather than resetting every time the view unmounts.
// Plain (non-reactive) storage; WebSearch seeds its $state from this on mount
// and writes back via an $effect.
export const wsCache: {
  tabs: unknown[] | null;
  activeId: string | null;
  seq: number;
} = {
  tabs: null,
  activeId: null,
  seq: 1,
};
