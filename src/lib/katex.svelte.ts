// Shared lazy KaTeX loader. KaTeX (~270KB JS + its stylesheet) is heavy and most
// documents contain no math, so it stays out of app startup and idle memory and
// loads exactly once — the first time ANY RichText renders math. The loader lives
// here (module-level, shared) rather than inside each RichText: a math-heavy
// cheatsheet mounts dozens of RichText components, and they should share one
// import and one reactive "is it ready" signal instead of each firing its own.
// Reading `katex.ensure()` in a component's render path re-typesets that subtree
// from escaped source to real math the moment the library lands.

type KatexModule = typeof import("katex").default;

class KatexLoader {
  mod = $state<KatexModule | null>(null);
  private loading = false;

  // Returns the module if ready; otherwise kicks off a one-time load and returns
  // null. Safe to call on every math token — after the first call it's a flag
  // check. The `mod` read makes callers reactive, so they re-render on load.
  ensure(): KatexModule | null {
    if (this.mod || this.loading) return this.mod;
    this.loading = true;
    Promise.all([import("katex"), import("katex/dist/katex.min.css")])
      .then(([m]) => { this.mod = m.default; })
      .catch(() => { this.loading = false; });
    return null;
  }
}

export const katex = new KatexLoader();
