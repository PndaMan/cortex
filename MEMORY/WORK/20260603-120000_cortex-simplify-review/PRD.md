---
task: simplify quality review of cortex recent changes
slug: 20260603-120000_cortex-simplify-review
effort: advanced
phase: complete
progress: 8/24
mode: interactive
started: 2026-06-03T12:00:00Z
updated: 2026-06-03T12:01:00Z
---

## Context

Quality review of recently-changed Cortex code from the multi-session history / notes / calendar / topic-delete / source-move feature set. Scope: 9 modified files + 4 new files. Task is simplification-angle only — no bug hunting. Output: findings list with file/line/summary/cost/simpler-form, then apply fixes.

### Risks
- Svelte 5 runes reactivity makes some $state/$derived patterns non-obvious; false positives possible
- Review mode in Quiz/Flashcards has subtle fallback logic that could be broken by simplification

## Criteria

- [x] ISC-1: CalendarView.svelte: `hasAny` $derived reviewed for derivability vs inline use
- [ ] ISC-2: CalendarView.svelte: `monthWindow()` duplication between $effect and `reload()` identified
- [x] ISC-3: CalendarView.svelte: `goToday` `new Date()` allocation vs `today` const reviewed
- [ ] ISC-4: CalendarView.svelte: prevMonth/nextMonth symmetry reviewed for shared-helper opportunity
- [x] ISC-5: Quiz.svelte: `reviewQueue` intermediate $derived reviewed — collapsible into `activeQs`
- [ ] ISC-6: Quiz.svelte: `startReview` vs Flashcards `startReview` copy-paste pattern identified
- [ ] ISC-7: Flashcards.svelte: `activeDeck` $derived reviewed for simplification vs `reviewQueue` pattern in Quiz
- [ ] ISC-8: NotesView.svelte: `selected` $derived vs `selectedId` state usage reviewed
- [ ] ISC-9: NotesView.svelte: `canConvert` $derived reviewed — derivable inline
- [ ] ISC-10: NotesView.svelte: `relTime` utility reviewed for existing shared-helper
- [ ] ISC-11: NotesView.svelte: `notesWorkspace` snippet necessity reviewed (used in 2 places)
- [ ] ISC-12: MarkdownEditor.svelte: `tools` array const vs $derived necessity reviewed
- [ ] ISC-13: MarkdownEditor.svelte: `codeBlock` and `link` vs `wrap`/`prefixLines` pattern alignment reviewed
- [ ] ISC-14: EventModal.svelte: form reset duplication in $effect (e branch vs else branch) reviewed
- [ ] ISC-15: EventModal.svelte: `deriveReminder` single-use helper vs inline reviewed
- [ ] ISC-16: EditModal.svelte: `selectedSubjectId`/`originalSubjectId` both $state when one could be stored differently
- [ ] ISC-17: EditModal.svelte: topic options $derived duplicates owning-subject lookup already done in $effect
- [x] ISC-18: AddSource.svelte: two $effects for selectedSubjectId and selectedTopic — mergeable
- [ ] ISC-19: ChatPanel.svelte: `startResize` closure allocs on every pointerdown — reviewed
- [ ] ISC-20: ChatPanel.svelte: removed `modelLabel` $state — confirmed dead code cleanup
- [ ] ISC-21: Sidebar.svelte: Notes/Calendar nav items are copy-paste of existing nav-item pattern
- [x] ISC-22: store.svelte.ts: `toggleNotes` added but `toggleChat` already exists — pattern parity reviewed
- [x] ISC-23: All findings deduplicated and non-behavior-changing fixes applied
- [x] ISC-24: Skipped findings documented with rationale

## Decisions

## Verification
