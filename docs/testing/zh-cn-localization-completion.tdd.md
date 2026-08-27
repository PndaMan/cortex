# Simplified Chinese localization completion — TDD evidence

## Source and user journey

This work was derived directly from the request to eliminate the 418 remaining
English UI audit findings. No external plan file was used.

As a user who selects Simplified Chinese, I want every reviewed static Svelte UI
string to have a Chinese rendering, while technical identifiers and user-authored
content remain unchanged.

## RED → GREEN evidence

| Guarantee | Test or command | Type | Result | Evidence |
|---|---|---|---|---|
| Every Svelte UI file is included in the localization audit | `bun test src/lib/i18n.test.ts` | Integration | PASS | The suite checks all 58 `.svelte` files. |
| The previous untranslated inventory is rejected | `bun test src/lib/i18n.test.ts` before the translation update | Regression | RED | The new assertion failed with 418 findings. |
| No reviewed static English copy remains without a translation or explicit technical classification | `bun test src/lib/i18n.test.ts` after the translation update | Regression | PASS | 13 tests passed; the global finding list is empty. |
| The standalone audit agrees with the regression test | `bun run i18n:audit` | Static audit | PASS | `0 untranslated static strings across 58 Svelte files`. |
| The localized application still type-checks and bundles | `bun run check` and `bun run build` | Build | PASS | Svelte reported 0 errors and 0 warnings; Vite completed the production build. |

## Coverage and known gaps

`bun test --coverage src/lib/i18n.test.ts` reports 86.32% line coverage for the
static audit module. Aggregate coverage is 43.36% because the same module imports
the browser-only DOM observer from `i18n.ts`; the Bun test environment has no DOM
implementation and therefore cannot execute that path. Translation behavior,
dynamic patterns, user-content boundaries, and the full 58-file audit are covered.
The DOM observer itself remains a browser/E2E coverage gap.

The audit intentionally preserves four technical examples: a DOI placeholder, an
API-key placeholder, an author-name format, and the `Osaka Jade` theme name.
