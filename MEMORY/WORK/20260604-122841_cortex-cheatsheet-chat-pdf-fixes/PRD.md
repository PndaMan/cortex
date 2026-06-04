---
task: Fix cortex cheatsheet parse, chat toggle, PDF export
slug: 20260604-122841_cortex-cheatsheet-chat-pdf-fixes
effort: deep
phase: execute
progress: 25/29
mode: interactive
started: 2026-06-04T12:28:41+02:00
updated: 2026-06-04T12:28:41+02:00
---

## Context

Three user-reported regressions in Cortex (Tauri + Svelte 5 + Rust), plus the
remaining backlog recorded in memory. Priority order: (1) cheatsheets, (2) chat
window toggle, (3) PDF export — then memory backlog.

**Bug 1 — Cheatsheets show "Model returned unstructured output".**
`generate_cheatsheet` (commands.rs:782) calls `parse_cheatsheet` →
`llm::extract_json` (llm.rs:324). When parsing yields zero sections it falls back
to a single "Model returned unstructured output" section (commands.rs:851-862).
The screenshot shows the model DID return valid-looking JSON
(`{"sections":[{"title":"Overview","items":[{"t":"...","d":"...**integrative..."`)
yet the fallback fired. Existing repair (`escape_unescaped_controls`, llm.rs:379)
only fixes literal control chars; it does NOT fix unescaped interior double
quotes, which LLMs routinely emit inside rich-markdown "d" bodies and which break
both the brace-matching slice extractor AND `serde_json`.

**Bug 2 — Chat panel won't hide via `c` in source/cheatsheet/materials; works in notes.**
The screenshot shows the chat dock AND the "Ask c" FAB visible at once.
`SourceViewer.svelte:268` renders `<ChatPanel/>` UNGATED by `app.chatOpen` — so
in the source view the panel can never be hidden. The subject dock + FAB in
App.svelte:40-44/187-200 use complementary `chatOpen` conditions; the
simultaneous FAB+dock state must be reproduced/diagnosed by running the app.

**Bug 3 — PDF export "completely broken".**
Both cheatsheet and notes export call `window.print()` (Cheatsheet.svelte:156,191;
NotesView.svelte:128) relying on `@media print` CSS. On Linux WebKitGTK
`window.print()` is commonly a no-op. No backend PDF command exists. Needs a
reliable path (verify whether print works at runtime first).

**Memory backlog (secondary):** runtime-pending GUI items (mic grant,
preview render, music, live SearXNG, keybind/theme apply, empty state); known
gap: sources with no topic don't appear in the sidebar tree; descoped: image
OCR, auto memory-capture; homelab SearXNG JSON requirement.

### Risks
- Chat subject-dock bug may not be reproducible from static analysis — must run app.
- Unescaped-quote JSON repair is heuristic; could mis-handle legitimate escaped quotes.
- PDF backend generation may pull heavy deps or external binaries — needs a decision.
- Several ISC require live GUI observation, not just compile/test.

## Criteria

### Cheatsheet JSON parsing (Rust)
- [x] ISC-1: extract_json parses object with literal newlines in string values
- [x] ISC-2: extract_json parses object with literal tabs in string values
- [x] ISC-3: extract_json parses object with unescaped interior double-quotes in "d"
- [x] ISC-4: extract_json still parses ```json-fenced output with trailing prose
- [x] ISC-5: extract_json still parses a bare JSON array
- [x] ISC-6: parse_cheatsheet returns non-empty sections for realistic rich-markdown reply
- [x] ISC-7: "unstructured output" fallback fires only on genuinely unparseable input
- [x] ISC-8: all llm.rs JSON unit tests pass under `cargo test` (6/6)
- [x] ISC-B1: OpenRouter gzip "error decoding response body" fixed (Accept-Encoding: identity)
- [x] ISC-B2: cheatsheet prompt reworked for exhaustive exam coverage + richer Definitions
- [x] ISC-B3: cheatsheet source-context budget raised 24k→120k chars

### Chat toggle (Svelte)
- [x] ISC-9: SourceViewer chat pane hidden when app.chatOpen is false
- [x] ISC-10: SourceViewer chat pane visible when app.chatOpen is true
- [x] ISC-11: pressing `c` in source view hides the source chat pane
- [x] ISC-12: cheatsheet tab: pressing `c` hides the chat dock — USER-CONFIRMED live
- [x] ISC-13: materials tab: pressing `c` hides the chat dock (same fix)
- [x] ISC-14: notes view chat toggle still works (regression)
- [x] ISC-A1: chat FAB and chat dock never visible simultaneously (root cause fixed)

### PDF export
- [x] ISC-15: "Save as PDF" produces a real, openable PDF file (chromium render verified)
- [x] ISC-16: "Export all" produces a PDF containing every topic sheet
- [x] ISC-17: exported PDF excludes sidebar, chat, statusbar chrome (standalone HTML)
- [x] ISC-18: Notes "Save as PDF" produces a real PDF file
- [x] ISC-19: export surfaces an error toast on failure (no silent fail)

### Build / runtime verification
- [x] ISC-20: `cargo check` compiles cleanly
- [x] ISC-21: `svelte-check` reports 0 errors
- [x] ISC-22: `vite build` succeeds
- [x] ISC-23: app launches via `tauri dev` without panic (live)

### Memory backlog
- [ ] ISC-24: sources with no topic appear in the sidebar tree
- [ ] ISC-25: runtime-pending memory items triaged (fixed or explicitly re-deferred)
- [ ] ISC-26: memory updated to reflect what landed this session

## Decisions

## Verification
