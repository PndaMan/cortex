// Demo fixtures for the few screens whose backend isn't wired yet (the cheatsheet
// diff preview, the study-session flashcards/quiz, and the music station list).
// Real data (subjects/topics/sources/chunks/search/chat) comes from the Rust
// backend via api.ts. The old seeded cheatsheet/chat/materials/search fixtures
// were removed once those views moved to real data.

export interface DiffChange { type: "ctx" | "add" | "del"; text: string }
export interface DiffSection { id: string; title: string; changes: DiffChange[] }
export interface Station { id: string; name: string; kind: string; cat: string; ico: string }

export const diff = {
  source: "cp-algorithms.com/dp",
  sections: [
    {
      id: "key", title: "Key Concepts",
      changes: [
        { type: "ctx", text: "Call stack: each recursive call pushes a frame; depth equals the longest chain of unresolved calls." },
        { type: "del", text: "Overlapping subproblems occur in recursion." },
        { type: "add", text: "Overlapping subproblems: the same sub-inputs recur across the call tree — the precondition memoization exploits." },
        { type: "add", text: "Top-down (memoized) vs bottom-up (tabulated) are equivalent; choose by stack-depth risk." },
        { type: "ctx", text: "Optimal substructure: an optimal solution is composed of optimal solutions to its subproblems." },
      ],
    },
    {
      id: "form", title: "Formulas",
      changes: [
        { type: "ctx", text: "Master theorem: T(n) = aT(n/b) + f(n)." },
        { type: "add", text: "State-transition form: dp[i] = min over choices of (cost + dp[prev]). The recurrence IS the algorithm." },
      ],
    },
    {
      id: "ex", title: "Worked Examples",
      changes: [
        { type: "ctx", text: "Factorial: fact(n) = n·fact(n−1), fact(0)=1." },
        { type: "del", text: "Merge sort splits the array." },
        { type: "add", text: "Merge sort: split, recurse on halves, merge. T(n)=2T(n/2)+O(n) ⇒ O(n log n)." },
        { type: "add", text: "Coin change: dp[a] = 1 + min over coins c of dp[a−c]; classic bottom-up DP over amounts." },
      ],
    },
    {
      id: "pit", title: "Common Pitfalls",
      changes: [
        { type: "ctx", text: "Missing base case → infinite descent → stack overflow." },
        { type: "add", text: "Forgetting to memoize: silently turns an O(n) solution back into exponential time." },
        { type: "add", text: "Mutating shared state across recursive calls — leads to subtle, order-dependent bugs." },
      ],
    },
  ] as DiffSection[],
};

// Built-in stations, all streamed ad-free through the mpv sidecar (see
// src/lib/music.ts for the matching ids → YouTube URLs). Songs first, then
// noises — the music panel renders them in this order under one "Stations"
// header. Ids must match music.ts so playback resolves.
export const stations: Station[] = [
  // ── songs ──
  { id: "synthwave", name: "Neon Drive", kind: "synthwave", cat: "Stations", ico: "music" },
  { id: "lofi", name: "Lofi Girl", kind: "lofi hip-hop", cat: "Stations", ico: "music" },
  { id: "jazz", name: "Late Night Keys", kind: "jazz", cat: "Stations", ico: "music" },
  { id: "classical", name: "Grand Hall", kind: "classical", cat: "Stations", ico: "music" },
  // ── noises ──
  { id: "rain", name: "Rainfall", kind: "rain noise", cat: "Stations", ico: "waveform" },
  { id: "forest", name: "Forest Floor", kind: "forest noise", cat: "Stations", ico: "waveform" },
  { id: "binaural", name: "Deep Focus 40 Hz", kind: "40 Hz binaural", cat: "Stations", ico: "bolt" },
];

// Flashcards for the study session (Anki-style)
export const flashcards = [
  { q: "What two properties must a problem have for dynamic programming to apply?", a: "Overlapping subproblems and optimal substructure." },
  { q: "What is a base case?", a: "The smallest input solved directly without further recursion; every path must reach one." },
  { q: "Time complexity of naive vs memoized Fibonacci?", a: "Naive is O(φⁿ) (exponential); memoized is O(n)." },
  { q: "Master theorem form?", a: "T(n) = aT(n/b) + f(n) — compare f(n) with n^(log_b a)." },
  { q: "Top-down vs bottom-up DP?", a: "Memoized recursion vs iterative tabulation — equivalent; choose by stack-depth risk." },
];

// Quiz questions (multiple choice)
export const quiz = [
  {
    q: "Which pair of properties signals a dynamic-programming problem?",
    options: ["Overlapping subproblems + optimal substructure", "Greedy choice + matroid", "Divide and conquer + sorting", "Memoization + recursion only"],
    answer: 0,
    explain: "DP applies when subproblems overlap (so caching helps) and the optimum is built from sub-optima.",
  },
  {
    q: "Naive recursive Fibonacci runs in…",
    options: ["O(n)", "O(n log n)", "O(φⁿ) exponential", "O(n²)"],
    answer: 2,
    explain: "Each call branches into two, re-solving overlapping subproblems exponentially often.",
  },
  {
    q: "Bottom-up tabulation avoids which risk that top-down memoization has?",
    options: ["Cache misses", "Stack overflow from deep recursion", "Incorrect base cases", "Higher asymptotic complexity"],
    answer: 1,
    explain: "Tabulation is iterative, so there's no recursion depth to overflow the call stack.",
  },
];

// Renders ⟦source · loc⟧ citation markers in assistant text as clickable chips.
export function parseCitations(text: string): Array<{ t: "text" | "cite"; v: string }> {
  const out: Array<{ t: "text" | "cite"; v: string }> = [];
  const re = /⟦([^⟧]+)⟧/g;
  let last = 0;
  let m: RegExpExecArray | null;
  while ((m = re.exec(text)) !== null) {
    if (m.index > last) out.push({ t: "text", v: text.slice(last, m.index) });
    out.push({ t: "cite", v: m[1].trim() });
    last = m.index + m[0].length;
  }
  if (last < text.length) out.push({ t: "text", v: text.slice(last) });
  return out;
}
