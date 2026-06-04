---
task: HTML timeline infographic + custom prompt all generators
slug: 20260604-163700_infographic-timeline-custom-prompts
effort: advanced
phase: complete
progress: 22/22
mode: interactive
started: 2026-06-04T14:37:00Z
updated: 2026-06-04T14:55:00Z
---

## Context

Two user-requested features, then a follow-up to prioritise the unbuilt-feature list:

1. **Detailed HTML infographic with a timeline of events.** The current infographic
   path renders the structured spec to a poster *image* via nano-banana (Gemini image),
   which garbles text. The user explicitly chose the HTML render path ("do the html
   infographic but make it detailed") and wants a timeline of events as a core element.
2. **NotebookLM-style custom prompt on EVERY generator.** Users can type their own
   instructions when generating any material; this is woven into the prompt alongside —
   never replacing — the system prompt that anchors the output format/role.

Generation lives in `src-tauri/src/commands.rs`:
- `generate_cheatsheet` (system+user → `model.complete`)
- `generate_material` (per-kind system for quiz/audio/flashcards/infographic/slideshow)
Both already append a global `style_instruction`. Frontend: `src/lib/api.ts`,
`src/views/GenerateMaterial.svelte`, `src/views/Cheatsheet.svelte`,
`src/components/InfographicView.svelte`.

### Plan
- Backend: add `custom_prompt: Option<String>` to both commands; a `custom_focus()`
  helper builds a fenced "USER FOCUS" block appended AFTER style, instructing the model
  to honour custom instructions WHILE still obeying the format/JSON contract. Empty/
  whitespace custom prompt → empty string (zero regression).
- Infographic: extend the JSON schema with a `timeline` array and richer points; stop
  injecting the nano-banana `image` for new infographics so the HTML poster renders.
  Keep `render_infographic_image` + the InfographicView image branch for legacy
  materials that already carry an `image`.
- InfographicView: render a vertical timeline (date marker + title + optional detail,
  connected line) plus the existing section grid.
- Frontend: forward `customPrompt` through api.ts; add an optional custom-instructions
  textarea in GenerateMaterial Details and in the Cheatsheet empty-state.

### Risks
- Removing the image injection could regress old infographics → mitigated by keeping the
  image branch and only skipping NEW image generation.
- Custom prompt could let users break the JSON contract → mitigated by appending it as a
  subordinate "focus" block that explicitly defers to the format rules above it.
- Svelte 5 runes: new state must use `$state`; new textarea must not break existing
  reactive title suggestion.

## Criteria

> SCOPE CHANGE (user interrupt): custom prompt is for the 5 MATERIAL generators
> ONLY, not the cheatsheet. Cheatsheet criteria were reverted (struck below).

Backend — custom prompt:
- [x] ISC-1: ~~generate_cheatsheet custom_prompt~~ REVERTED — cheatsheet excluded
- [x] ISC-2: generate_material accepts custom_prompt Option<String> param
- [x] ISC-3: custom_focus helper trims and ignores empty/whitespace input
- [x] ISC-4: ~~cheatsheet system custom focus~~ REVERTED — cheatsheet excluded
- [x] ISC-5: material system appends custom focus after style
- [x] ISC-6: custom focus text tells model to keep output contract
- [x] ISC-7: empty custom_prompt leaves material prompt byte-identical

Backend — infographic:
- [x] ISC-8: infographic JSON schema includes timeline array field
- [x] ISC-9: infographic prompt requests chronological dated events
- [x] ISC-10: infographic prompt requests richer detailed section points
- [x] ISC-11: generate_material skips nano-banana image for new infographic
- [x] ISC-12: render_infographic_image function retained for legacy materials

Frontend — API:
- [x] ISC-13: ~~api.generateCheatsheet customPrompt~~ REVERTED
- [x] ISC-14: api.generateMaterial forwards customPrompt arg

Frontend — GenerateMaterial:
- [x] ISC-15: GenerateMaterial has custom-instructions textarea in Details
- [x] ISC-16: GenerateMaterial customPrompt uses $state rune
- [x] ISC-17: generate() passes customPrompt into api closure

Frontend — InfographicView timeline:
- [x] ISC-20: InfographicView types include timeline array
- [x] ISC-21: timeline renders as vertical node list
- [x] ISC-22: each node shows date marker and event title
- [x] ISC-23: each node shows optional detail text
- [x] ISC-24: connecting line styled between timeline nodes
- [x] ISC-25: section grid still renders alongside timeline
- [x] ISC-26: legacy image branch in InfographicView unchanged

Build:
- [x] ISC-27: cargo check compiles with no errors
- [x] ISC-28: frontend svelte-check/build passes

Anti-criteria:
- [x] ISC-A1: style_instruction behaviour unchanged for all generators
- [x] ISC-A2: quiz/flashcards/slideshow output contracts unchanged

## Decisions

- Custom prompt is appended to the SHARED material system string (after style +
  raw-JSON guardrail) so it applies to all 5 kinds uniformly with no per-kind
  branching. It is framed as a SUBORDINATE "USER FOCUS" block that defers to the
  output/JSON contract above it, so users can steer content without breaking the
  schema.
- Cheatsheet intentionally excluded (user interrupt). The exclusion is structural —
  `generate_cheatsheet` is a separate command that never takes the param.
- Infographic image generation removed from the live path (user chose HTML render
  for crisp text). `render_infographic_image` kept behind `#[allow(dead_code)]`
  rather than deleted, so the poster-image mode can be re-enabled later (steering
  rule: don't remove deliberately-built components without asking).
- `.set-textarea` global utility reused for the custom-prompt textarea (matches
  Settings) instead of inline styles (per /simplify reuse finding).

## Verification

- `cargo check`: clean — 0 warnings, 0 errors (the now-unmutated `payload` warning
  was fixed; dead-fn allowed explicitly).
- `npm run check` (svelte-check): 132 files, 0 errors, 0 warnings.
- `/simplify`: 4 parallel agents (reuse/simplify/efficiency/altitude). Acted on 2
  findings — reused `.set-textarea`, softened the dead-fn comment. Efficiency agent
  noted the change REMOVES a per-infographic image-model call (net win).
- PENDING (needs the running app + an API key): visual confirmation of a freshly
  generated infographic timeline, and a custom-prompt-steered material. Code path
  typechecks end-to-end; Tauri camelCase→snake_case bridge confirmed consistent
  with existing subjectId/topicId params.
</content>
