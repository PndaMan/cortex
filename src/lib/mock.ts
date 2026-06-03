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

export const stations: Station[] = [
  { id: "lofi", name: "Groove Salad · chill", kind: "ambient downtempo", cat: "Beats", ico: "music" },
  { id: "jazz", name: "Sonic Universe · jazz", kind: "jazz fusion", cat: "Beats", ico: "music" },
  { id: "classical", name: "Drone Zone · ambient", kind: "atmospheric ambient", cat: "Instrumental", ico: "music" },
  { id: "piano", name: "Deep Space One · space", kind: "deep space ambient", cat: "Instrumental", ico: "music" },
  { id: "brown", name: "Brown noise · deep", kind: "generated noise", cat: "Noise", ico: "waveform" },
  { id: "rain", name: "Rain · white-noise", kind: "generated noise", cat: "Noise", ico: "waveform" },
  { id: "cafe", name: "Café ambience", kind: "generated noise", cat: "Noise", ico: "waveform" },
  { id: "focus", name: "Space Station · focus", kind: "space ambient", cat: "Focus", ico: "bolt" },
  { id: "binaural", name: "Binaural · 40Hz gamma", kind: "generated tones", cat: "Focus", ico: "bolt" },
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
