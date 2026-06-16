# Cortex — pre-launch scrub checklist

From the pre-launch audit. (Commit-history AI-authorship rewrite intentionally skipped.)

## 🔴 Must-fix before posting
- [x] **Privacy overclaim** — README.md "data never leaves your machine" → honest local-first + "what you send to the AI provider leaves" wording.
- [x] **CSP disabled** — tauri.conf.json `csp: null` → real policy (`script-src 'self'`, etc.). ⚠️ needs a runtime smoke-test (CSP errors only show at runtime).
- [x] **Raw LLM-SVG render** — InfographicView.svelte: `sanitizeSvg()` strips scripts/foreignObject/event-handlers/js-URLs before `{@html}`.
- [x] **Font licenses** — `src/assets/fonts/OFL.txt` (canonical SIL OFL 1.1 + the 3 copyrights).
- [x] **License fields** — `license` added to package.json + Cargo.toml; `NOTICE` added.
- [x] **"Ad-free YouTube"** — README.md + music.ts reworded to neutral "study-music / played by mpv".
- [x] **School default** — Settings.svelte mdUrl default + placeholder no longer leak the university.

## 🟡 Polish
- [x] **URL scheme allow-listing** — `src/lib/url.ts` (`safeUrl`/`safeImgSrc`) applied to RichText, ChatPanel, Cheatsheet links/images.
- [x] **PDF.js hardening** — SourceViewer.svelte getDocument: `isEvalSupported:false, enableScripting:false`.
- [x] **Version drift** — package.json + Cargo.toml → 1.0.21 (match tauri.conf.json).
- [x] **Stray files** — deleted Cortex-handoff.zip + "cortex-logo-transparent (1).png".
- [ ] **AUR/Homebrew sha256** — committed templates carry placeholders; the publish-packages CI rewrites them with real checksums on each tagged release (so live installs are fine). No action needed unless you ship from the template by hand.

## Deliberately NOT doing
- AI-authorship commit-trailer rewrite (288 commits) — your call, kept as-is.
- Maintainer name/email in CI/PKGBUILD — intentional public maintainer identity.

## Before you actually post
- [ ] Build + run once and click through with the new CSP (recording, web images, PDF view, infographic, music) — confirm nothing is blocked.
