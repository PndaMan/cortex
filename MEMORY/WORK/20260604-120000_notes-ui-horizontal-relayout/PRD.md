---
task: notes UI horizontal master-detail spacious relayout
slug: 20260604-120000_notes-ui-horizontal-relayout
effort: advanced
phase: complete
progress: 28/28
mode: interactive
started: 2026-06-04T12:00:00Z
updated: 2026-06-04T12:01:00Z
---

## Context

Cortex notes page (NotesView.svelte + MarkdownEditor.svelte) is cramped and stacks vertically. The grid columns are technically set but the workspace doesn't fill height, so the editor stays short and narrow. The fix is: make the full-page view use full viewport height (flex column from workspace-scroll down), make the notes grid stretch-align (not start), make the detail pane flex-grow, and make MarkdownEditor flex-grow to fill all remaining vertical space. Embedded mode stays single-column/narrower. No other files touched.

### Risks
- SVG/flex height propagation: without explicit heights at every ancestor, flex:1 won't work. Must ensure workspace-scroll, notes-page, notes all have proper height contexts.
- The `.notes-page` max-width wrapper could prevent horizontal expansion — need to let it use full available width while keeping the page-head constrained.
- MarkdownEditor is used standalone as a component — changes to its sizing must not break other uses (it should fill its container, which is safe if container controls sizing).

## Criteria

- [x] ISC-1: `.notes-page` container fills full workspace height vertically
- [x] ISC-2: `.notes` grid uses `align-items: stretch` so both columns are equal height
- [x] ISC-3: `.notes-detail` pane uses `flex: 1 1 0` to fill remaining grid height
- [x] ISC-4: MarkdownEditor `.md` container uses `flex: 1 1 0` and `min-height: 0`
- [x] ISC-5: MarkdownEditor textarea fills full width of its container
- [x] ISC-6: MarkdownEditor textarea grows vertically via `flex: 1 1 0` inside flex column
- [x] ISC-7: MarkdownEditor preview fills full width of its container
- [x] ISC-8: MarkdownEditor preview grows vertically via `flex: 1 1 0`
- [x] ISC-9: Preview text has a readable max-width cap (e.g. 72ch) centered within the pane
- [x] ISC-10: Textarea has NO max-width cap (uses full editor width)
- [x] ISC-11: Note list column is fixed-width ~260px in full view
- [x] ISC-12: Note list column uses `align-self: stretch` / fills full height
- [x] ISC-13: Note list items area scrolls independently within its column
- [x] ISC-14: Left/right columns are separated by a visible border
- [x] ISC-15: Full-page view has comfortable padding (>=16px) around content
- [x] ISC-16: Title input row has adequate vertical padding (>=12px top/bottom)
- [x] ISC-17: Toolbar stays a single row at normal widths, wraps gracefully when narrow
- [x] ISC-18: Embedded mode collapses list to ~180px column (existing behavior preserved)
- [x] ISC-19: Embedded mode detail pane still fills remaining width
- [x] ISC-20: New note button in list header remains visible and functional
- [x] ISC-21: "Save" button remains in actions row, disabled when saved
- [x] ISC-22: "Delete" button remains in actions row with margin-right:auto
- [x] ISC-23: "Convert to source" button remains, disabled without active subject
- [x] ISC-24: Empty state (no notes) renders correctly inside list column
- [x] ISC-25: Detail empty state renders when no note selected
- [x] ISC-26: No hardcoded color values — only CSS custom properties used
- [x] ISC-27: No TypeScript errors introduced — svelte-check: 0 errors, 0 new warnings
- [x] ISC-28: No a11y regressions (role/aria attributes preserved)

## Decisions

## Verification
