// Live OpenRouter model catalog. The /models endpoint is public and CORS-open
// (access-control-allow-origin: *), so we fetch it straight from the webview and
// memoize the promise — one network call per session.

export type OrModel = {
  id: string; // e.g. "anthropic/claude-sonnet-4.5"
  label: string; // human name, e.g. "Anthropic: Claude Sonnet 4.5"
  sub: string; // "$3/M in · $15/M out · 200K ctx"
  recommended: boolean;
};

// Curated cost/quality picks, pinned to the top in this order (a sensible default
// shortlist; the full 300+ catalog is one keystroke away via search).
const RECOMMENDED: string[] = [
  "deepseek/deepseek-v4-flash",
  "google/gemini-2.5-flash",
  "google/gemini-2.5-flash-lite",
  "openai/gpt-5-mini",
  "openai/gpt-4o-mini",
  "anthropic/claude-3.5-haiku",
  "deepseek/deepseek-chat",
  "meta-llama/llama-3.3-70b-instruct",
  "openai/gpt-4o",
  "anthropic/claude-3.7-sonnet",
  "google/gemini-2.5-pro",
  "anthropic/claude-sonnet-4.5",
  "openai/gpt-5",
  "deepseek/deepseek-r1",
];

type RawModel = {
  id: string;
  name?: string;
  context_length?: number | null;
  pricing?: { prompt?: string; completion?: string };
};

let cache: Promise<OrModel[]> | null = null;

/** Fetch (once, memoized) the full OpenRouter catalog, sorted recommended-first then cheapest-input-first. */
export function loadOpenRouterModels(): Promise<OrModel[]> {
  if (!cache) cache = fetchModels().catch((e) => { cache = null; throw e; });
  return cache;
}

async function fetchModels(): Promise<OrModel[]> {
  const res = await fetch("https://openrouter.ai/api/v1/models");
  if (!res.ok) throw new Error(`OpenRouter models ${res.status}`);
  const json = (await res.json()) as { data?: RawModel[] };
  const rank = new Map(RECOMMENDED.map((id, i) => [id, i] as const));

  const models = (json.data ?? []).map((m) => {
    const pin = parseFloat(m.pricing?.prompt ?? "0") || 0;
    const pout = parseFloat(m.pricing?.completion ?? "0") || 0;
    return {
      id: m.id,
      label: m.name || m.id,
      sub: subLine(pin, pout, m.context_length ?? null),
      recommended: rank.has(m.id),
      priceIn: pin,
    };
  });

  models.sort((a, b) => {
    const ra = rank.get(a.id) ?? Infinity;
    const rb = rank.get(b.id) ?? Infinity;
    if (ra !== rb) return ra - rb; // recommended first, in curated order
    const pa = a.priceIn < 0 ? Infinity : a.priceIn; // variable (-1) sinks to the bottom
    const pb = b.priceIn < 0 ? Infinity : b.priceIn;
    if (pa !== pb) return pa - pb; // then cheapest input first
    return a.label.localeCompare(b.label);
  });

  return models.map(({ priceIn: _priceIn, ...m }) => m);
}

// pricing.prompt/completion are USD *per token*; ×1e6 → per-1M-tokens.
function dollarsPerM(perToken: number): string {
  const v = perToken * 1e6;
  if (v === 0) return "$0";
  if (v < 1) return `$${v.toFixed(2)}`;
  if (v < 10) return v % 1 === 0 ? `$${v.toFixed(0)}` : `$${v.toFixed(2)}`;
  return `$${Math.round(v)}`;
}
function ctxLabel(c: number | null): string | null {
  if (!c) return null;
  return c >= 1000 ? `${Math.round(c / 1000)}K ctx` : `${c} ctx`;
}
function subLine(pin: number, pout: number, ctx: number | null): string {
  const c = ctxLabel(ctx);
  let price: string;
  if (pin < 0 || pout < 0) price = "variable price";
  else if (pin === 0 && pout === 0) price = "free";
  else price = `${dollarsPerM(pin)}/M in · ${dollarsPerM(pout)}/M out`;
  return c ? `${price} · ${c}` : price;
}
