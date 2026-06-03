// Mock content for views whose backend lands in a later slice (cheatsheet
// synthesis, chat, materials, SearXNG, music). Ported from the Claude Design
// prototype's app/data.js so the UI is faithful and fully clickable. Real data
// (subjects/topics/sources/chunks) comes from the Rust backend via api.ts.

export interface CsItem { t: string; d: string; flag?: "changed" }
export interface CsSection {
  id: string;
  title: string;
  state: "approved" | "draft-pending" | "idle";
  pending?: number;
  items: CsItem[];
}
export interface DiffChange { type: "ctx" | "add" | "del"; text: string }
export interface DiffSection { id: string; title: string; changes: DiffChange[] }
export interface ChatMessage { role: "system" | "user" | "assistant"; text: string }
export interface Station { id: string; name: string; kind: string; cat: string; ico: string }
export interface Material {
  id: string; type: "flashcards" | "quiz" | "audio" | "slideshow" | "infographic";
  title: string; topic: string; meta: string; status: string; launch?: string;
}
export interface SearchResult {
  id: string; title: string; url: string; host: string; fav: string; favBg: string;
  cat: string; engines: string[]; snippet: string; reader: string[];
}

export const cheatsheet = {
  subject: "Algorithms",
  topic: "Recursion",
  sources: 2,
  sections: [
    {
      id: "def", title: "Definitions", state: "approved",
      items: [
        { t: "Recursion", d: "A function defined in terms of itself, reducing a problem to smaller instances until a base case halts the descent." },
        { t: "Base case", d: "The smallest input solved directly, without further recursion. Every path must reach one or the recursion never terminates." },
        { t: "Recursive case", d: "The step that calls the function on a strictly smaller sub-input, moving toward the base case." },
      ],
    },
    {
      id: "key", title: "Key Concepts", state: "draft-pending", pending: 3,
      items: [
        { t: "Call stack", d: "Each recursive call pushes a frame; depth equals the longest chain of unresolved calls. Deep recursion risks stack overflow." },
        { t: "Overlapping subproblems", d: "The same sub-inputs recur across the call tree — the precondition memoization exploits.", flag: "changed" },
        { t: "Optimal substructure", d: "An optimal solution is composed of optimal solutions to its subproblems." },
      ],
    },
    {
      id: "form", title: "Formulas", state: "idle",
      items: [
        { t: "Master theorem", d: "T(n) = aT(n/b) + f(n) — compare f(n) with n^(log_b a) to read off the asymptotic class." },
        { t: "Fibonacci recurrence", d: "F(n) = F(n−1) + F(n−2); naive recursion is O(φⁿ), memoized is O(n)." },
      ],
    },
    {
      id: "ex", title: "Worked Examples", state: "idle",
      items: [
        { t: "Factorial", d: "fact(n) = n·fact(n−1), fact(0)=1. Linear depth, no branching." },
        { t: "Merge sort", d: "Split, recurse on halves, merge. T(n)=2T(n/2)+O(n) ⇒ O(n log n)." },
      ],
    },
    {
      id: "pit", title: "Common Pitfalls", state: "idle",
      items: [
        { t: "Missing base case", d: "Infinite descent → stack overflow. Always handle the smallest input first." },
        { t: "Recomputation", d: "Naive recursion re-solves subproblems exponentially. Memoize or tabulate." },
      ],
    },
    {
      id: "recall", title: "Quick Recall", state: "idle",
      items: [
        { t: "Two signatures of DP", d: "Overlapping subproblems + optimal substructure." },
        { t: "Top-down vs bottom-up", d: "Memoized recursion vs iterative tabulation — equivalent; choose by stack-depth risk." },
      ],
    },
  ] as CsSection[],
};

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

export const chatThread = {
  scope: { subject: "Algorithms", topic: "Recursion", source: "lecture-3.pdf" },
  messages: [
    { role: "system", text: "scope set to Source: lecture-03-recursion.pdf" },
    { role: "user", text: "Why does memoization help here?" },
    { role: "assistant", text: "Because the naive recursion recomputes the same subproblems exponentially often. Caching each result collapses the call tree to O(n) distinct states ⟦lecture-3 · p.14⟧. Without it you re-solve fib(k) for every path that reaches it ⟦tutorial-rec · 12:30⟧." },
    { role: "user", text: "So when is top-down worse than bottom-up?" },
    { role: "assistant", text: "Top-down keeps the recursion, so very deep state spaces risk a stack overflow ⟦lecture-3 · p.16⟧. Bottom-up is iterative — no stack depth — but computes every state whether you need it or not. Choose by whether your reachable state set is sparse." },
  ] as ChatMessage[],
  replies: {
    source: "Within lecture-3 alone, the key move is the recurrence on slide 14 ⟦lecture-3 · p.14⟧ — everything else follows from unrolling it.",
    topic: "Across the whole Recursion topic, three sources agree the base case is the most common bug ⟦tutorial-rec · 04:10⟧, and the master theorem ⟦lecture-3 · p.9⟧ is your fastest complexity read.",
    subject: "Zooming out to all of Algorithms: recursion underpins divide-and-conquer, DP, and graph traversal. The shared idea is reducing to optimal subproblems ⟦lecture-4 · p.3⟧.",
  },
};

export const stations: Station[] = [
  { id: "lofi", name: "lofi — rainy night", kind: "lo-fi beats", cat: "Beats", ico: "music" },
  { id: "jazz", name: "Late-night jazz café", kind: "lo-fi beats", cat: "Beats", ico: "music" },
  { id: "classical", name: "Bach · Goldberg Variations", kind: "classical", cat: "Instrumental", ico: "music" },
  { id: "piano", name: "Reflective piano", kind: "classical", cat: "Instrumental", ico: "music" },
  { id: "brown", name: "Brown noise · deep", kind: "noise", cat: "Noise", ico: "waveform" },
  { id: "rain", name: "Rain on a tent", kind: "nature", cat: "Noise", ico: "waveform" },
  { id: "cafe", name: "Coffee shop ambience", kind: "nature", cat: "Noise", ico: "waveform" },
  { id: "focus", name: "Hyperfocus drones", kind: "ambient", cat: "Focus", ico: "bolt" },
  { id: "binaural", name: "Binaural · 40Hz gamma", kind: "ambient", cat: "Focus", ico: "bolt" },
];

export const materials: Material[] = [
  { id: "fc-rec", type: "flashcards", title: "Recursion essentials", topic: "Recursion", meta: "24 cards · 3 due today", status: "ready", launch: "flashcards" },
  { id: "fc-dp", type: "flashcards", title: "DP patterns", topic: "Dynamic programming", meta: "18 cards · 0 due", status: "ready", launch: "flashcards" },
  { id: "qz-rec", type: "quiz", title: "Recursion check", topic: "Recursion", meta: "10 questions · last 84%", status: "ready", launch: "quiz" },
  { id: "qz-graph", type: "quiz", title: "Graph traversal", topic: "Graphs", meta: "8 questions · not attempted", status: "ready", launch: "quiz" },
  { id: "au-rec", type: "audio", title: "Recursion — deep dive", topic: "Recursion", meta: "12:30 · podcast-style", status: "ready", launch: "audio" },
  { id: "au-dp", type: "audio", title: "DP in ten minutes", topic: "Dynamic programming", meta: "9:48 · podcast-style", status: "ready", launch: "audio" },
  { id: "sl-rec", type: "slideshow", title: "Recursion visualized", topic: "Recursion", meta: "14 slides · 3:20 video", status: "ready" },
  { id: "ig-dp", type: "infographic", title: "The DP decision tree", topic: "Dynamic programming", meta: "1 poster · A3", status: "ready" },
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

export const searchResults: SearchResult[] = [
  {
    id: "wiki-dp", title: "Dynamic programming — Wikipedia", url: "https://en.wikipedia.org/wiki/Dynamic_programming",
    host: "en.wikipedia.org", fav: "W", favBg: "#3b6ea5", cat: "General", engines: ["wikipedia", "google", "brave"],
    snippet: "Dynamic programming is both a mathematical optimization method and an algorithmic paradigm. The method was developed by Richard Bellman in the 1950s and breaks a problem into overlapping subproblems…",
    reader: [
      "Dynamic programming (DP) is both a mathematical optimization method and a computer-programming method. In both contexts it refers to simplifying a complicated problem by breaking it down into simpler sub-problems in a recursive manner.",
      "If sub-problems can be nested recursively inside larger problems, so that dynamic-programming methods are applicable, then there is a relation between the value of the larger problem and the values of the sub-problems. In the optimization literature this relationship is called the Bellman equation.",
      "The technique of storing solutions to subproblems instead of recomputing them is called memoization. Memoization is a top-down approach; tabulation is the equivalent bottom-up approach that fills a table iteratively.",
      "Two key attributes that a problem must have for dynamic programming to apply are optimal substructure and overlapping sub-problems.",
    ],
  },
  {
    id: "cpa-dp", title: "Introduction to Dynamic Programming - cp-algorithms", url: "https://cp-algorithms.com/dynamic_programming/intro-to-dp.html",
    host: "cp-algorithms.com", fav: "C", favBg: "#2dd5b7", cat: "General", engines: ["google", "duckduckgo"],
    snippet: "A practical, competitive-programming oriented introduction to DP: states, transitions, and the order of evaluation. Worked examples for coin change, longest increasing subsequence and knapsack…",
    reader: [
      "Dynamic programming is a method for solving problems by breaking them into subproblems and reusing the answers. The art of DP is identifying the state — the minimal description of a subproblem — and the transition between states.",
      "Consider the classic coin-change problem: given coins and a target amount, find the fewest coins. The state dp[a] is the minimum number of coins to make amount a. The transition is dp[a] = 1 + min over coins c of dp[a − c].",
      "The order of evaluation matters: you must compute dp[a − c] before dp[a]. For one-dimensional amount DP, iterating amounts from low to high guarantees this.",
      "Once the recurrence is correct, complexity is simply the number of states times the work per transition. Here that is O(amount × coins).",
    ],
  },
  {
    id: "mit-ocw", title: "6.006 Introduction to Algorithms — Dynamic Programming | MIT OCW", url: "https://ocw.mit.edu/courses/6-006-introduction-to-algorithms-spring-2020/",
    host: "ocw.mit.edu", fav: "M", favBg: "#a31f34", cat: "Science", engines: ["google", "semantic scholar"],
    snippet: "Lecture videos and notes covering memoization, bottom-up DP, and the five-step DP process: subproblems, guess, recurrence, order, and solve the original problem…",
    reader: [
      "MIT 6.006 frames dynamic programming as a five-step process. First, define the subproblems. Second, guess part of the solution. Third, relate subproblem solutions with a recurrence. Fourth, recurse and memoize (or build a table bottom-up). Fifth, solve the original problem.",
      "A useful mantra from the course: 'DP ≈ careful brute force.' You are exploring every possibility, but reusing overlapping work so the total cost stays polynomial.",
      "The running time of a DP is the number of distinct subproblems multiplied by the time per subproblem, provided each subproblem is computed once and cached.",
    ],
  },
  {
    id: "so-memo", title: "Difference between memoization and tabulation? - Stack Overflow", url: "https://stackoverflow.com/questions/6184869/",
    host: "stackoverflow.com", fav: "S", favBg: "#e07a26", cat: "General", engines: ["duckduckgo", "brave"],
    snippet: "Top answer: memoization is top-down and lazy — it only solves subproblems that are actually reached. Tabulation is bottom-up and eager — it fills every cell whether or not you need it…",
    reader: [
      "Memoization is top-down: you write the natural recursion and cache results as you go. It only ever computes the subproblems your inputs actually reach, which can be a big win when the reachable state space is sparse.",
      "Tabulation is bottom-up: you iterate over states in dependency order and fill a table. There is no recursion, so no stack-depth risk, but you compute every state whether or not it is needed.",
      "Rule of thumb: prefer memoization when the state space is large but sparsely reached; prefer tabulation when you'll need most states anyway.",
    ],
  },
  {
    id: "yt-dp", title: "5 Simple Steps for Solving Dynamic Programming Problems", url: "https://www.youtube.com/watch?v=aPQY__2H3tE",
    host: "youtube.com", fav: "▶", favBg: "#ff0000", cat: "Videos", engines: ["youtube", "google"],
    snippet: "A 20-minute walkthrough of a repeatable framework for DP interview problems, with the unbounded knapsack as the running example…",
    reader: [
      "This talk presents a repeatable framework: (1) verify the DP property, (2) express the recurrence, (3) decide memoize vs tabulate, (4) define the base cases, (5) read off the complexity.",
      "The unbounded knapsack is used as the running example, showing how a small change to the transition (allowing reuse of items) changes the loop order but not the overall approach.",
    ],
  },
];

export const search = { suggested: "dynamic programming memoization", results: searchResults };

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
