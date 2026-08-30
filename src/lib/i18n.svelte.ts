/**
 * Minimal i18n layer for Cortex (gettext-style).
 *
 * The English UI string is the key: `t("Settings")` returns the ru translation
 * when the active locale is `ru`, and falls back to the key itself (English)
 * otherwise. Two consequences:
 *   • en users pay zero dictionary lookups (fast path returns the key);
 *   • a missing translation degrades to English, never to a broken label.
 *
 * Reactivity: `i18n` is a module-level $state rune (this is a .svelte.ts
 * module), so a locale switch re-renders every component whose output reads
 * `t()`. Translations live in `./i18n-ru` — a pure data module, kept separate
 * so the dictionary can be regenerated without touching this logic.
 *
 * Persistence: the choice is stored in the local settings table as `ui_lang`.
 * Before it round-trips (or when unset), the locale is detected from the OS /
 * browser language preference. Language is deliberately per-device — it is NOT
 * in the sync allowlist.
 *
 * Usage:
 *   import { t } from "$lib/i18n.svelte";
 *   <h1>{t("Dashboard")}</h1>
 *   t("Studied {n} min", { n: 12 })
 */

import { getAllSettings, setSetting } from "./api";
import { ru } from "./i18n-ru";

export type Locale = "en" | "ru";

/** Picker metadata (label shown in its own language). */
export const LOCALES: { id: Locale; label: string }[] = [
  { id: "en", label: "English" },
  { id: "ru", label: "Русский" },
];

export const SETTINGS_KEY = "ui_lang";

function detect(): Locale {
  try {
    const langs = navigator.languages ?? [navigator.language];
    for (const l of langs) if (l?.toLowerCase().startsWith("ru")) return "ru";
  } catch { /* no navigator (SSR-ish) — stay English */ }
  return "en";
}

/** Reactive locale container — read by `t()`, written by `setLocale()`. */
export const i18n = $state({ locale: detect() as Locale });

/** Translate `key` (the English string), interpolating `{name}` vars. */
export function t(key: string, vars?: Record<string, string | number>): string {
  if (i18n.locale === "en") return key;
  let out: string = ru[key] ?? key;
  if (vars) {
    for (const [k, v] of Object.entries(vars)) out = out.replaceAll(`{${k}}`, String(v));
  }
  return out;
}

/** Load the persisted choice once at startup (fire-and-forget from App.svelte). */
export async function initI18n(): Promise<void> {
  try {
    const saved = (await getAllSettings())[SETTINGS_KEY];
    if (saved === "ru" || saved === "en") i18n.locale = saved;
  } catch { /* browser dev mode / no backend — detection stands */ }
}

/** Switch locale at runtime and persist (best-effort). */
export async function setLocale(l: Locale): Promise<void> {
  if (i18n.locale === l) return;
  i18n.locale = l;
  try {
    await setSetting(SETTINGS_KEY, l);
  } catch { /* browser dev mode */ }
}
