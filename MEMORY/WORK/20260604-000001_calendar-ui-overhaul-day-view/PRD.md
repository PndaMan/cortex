---
task: Calendar UI overhaul with day view and am/pm
slug: 20260604-000001_calendar-ui-overhaul-day-view
effort: advanced
phase: execute
progress: 32/32
mode: interactive
started: 2026-06-04T00:00:01Z
updated: 2026-06-04T00:01:00Z
---

## Context

CalendarView.svelte is the standalone month-grid calendar in Cortex (Tauri v2 + Svelte 5 runes). EventModal.svelte handles create/edit. User feedback identified 5 areas for improvement: (1) am/pm label format, (2) more info in month-grid pills, (3) location under title everywhere, (4) full day-view mode when clicking a day cell (not immediate create), (5) cleaner subject filter picker styling.

Work is surgical to CalendarView.svelte only (EventModal already correct). No store, route, or API changes. All CSS via existing custom props.

### Risks
- Day view mode switching must not break month grid state (year/month)
- `pill` flex layout change for second line must not break overflow/truncation
- `selectedDay` state must be reset or remain stable when switching between month/day
- Picker wrapper class must not conflict with Picker.svelte's internal `.picker` class
- Two-line pills in tiny cells may be too tall — need to constrain height carefully

## Criteria

- [x] ISC-1: `timeLabel` returns "9am" not "9a" for zero-minute AM times
- [x] ISC-2: `timeLabel` returns "2pm" not "2p" for zero-minute PM times
- [x] ISC-3: `timeLabel` returns "9:30am" for half-hour AM times
- [x] ISC-4: `timeLabel` returns "1:15pm" for quarter-hour PM times
- [x] ISC-5: `timeLabel` returns "" for all-day events (unchanged)
- [x] ISC-6: Month-grid pills show time label before title (already existed — preserved)
- [x] ISC-7: Month-grid pills show location on second line when location is non-null
- [x] ISC-8: Month-grid pill location line is faint color (--fg-faint)
- [x] ISC-9: Month-grid pill location line is smaller font (--t-2xs or smaller)
- [x] ISC-10: Month-grid pill location line is truncated (text-overflow: ellipsis)
- [x] ISC-11: Day view: internal `mode` $state of type "month" | "day" exists in CalendarView
- [x] ISC-12: Day view: internal `selectedDay` $state of type Date | null exists
- [x] ISC-13: Clicking a day cell sets `selectedDay` and switches `mode` to "day"
- [x] ISC-14: Day view renders instead of month grid when `mode === "day"`
- [x] ISC-15: Day view header shows ‹ prev-day button
- [x] ISC-16: Day view header shows › next-day button
- [x] ISC-17: Day view header shows "Month" button that sets `mode` back to "month"
- [x] ISC-18: Day view header shows the long date (e.g. "Wednesday, June 4 2026")
- [x] ISC-19: Day view shows a "+" / "Add event" button for creating on that day
- [x] ISC-20: Day view event list is sorted by start_ms ascending
- [x] ISC-21: Day view event rows show time range (start – end or just start if no end)
- [x] ISC-22: Day view event rows show title
- [x] ISC-23: Day view event rows show location when present (faint, truncated)
- [x] ISC-24: Day view event rows show description when present (truncated, 2-line clamp)
- [x] ISC-25: Day view event rows have a left color accent using pillColor
- [x] ISC-26: Clicking a day view event row opens EventModal for editing
- [x] ISC-27: Day view empty state shows when no events and has a "+ Add event" button
- [x] ISC-28: Day view "add event" opens EventModal with defaultDateMs set to selectedDay's date
- [x] ISC-29: Subject filter Picker is wrapped in a bordered container styled with --border-strong + --surface-2
- [x] ISC-30: Subject filter container has --r-lg border-radius
- [x] ISC-31: Header row layout is tidy (consistent alignment of nav, filter, and New button)
- [x] ISC-32: No TypeScript errors introduced (typed $state, typed function signatures)

## Decisions

- Day nav prev/next: update `year` and `month` when crossing month boundaries so "Month" button shows correct month
- Pills: change pill to `flex-direction: column` with `align-items: flex-start`; keep existing header row (dot/check + time + title) in a single inner div; location in second `.pill-loc` span
- Picker wrapper: a `.picker-wrap` div with `border: 1px solid var(--border-strong); border-radius: var(--r-lg); background: var(--surface-2); overflow: hidden` — wrapping the `<Picker>` component so Picker's own `.picker-btn` styles inherit cleanly inside it

## Verification
