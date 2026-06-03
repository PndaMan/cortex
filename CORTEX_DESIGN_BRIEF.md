# Cortex — Design Brief for Claude Design

A hand-off document describing what to design before any code is written. Paste relevant sections directly into Claude Design (claude.ai/design) to commission mocks.

---

## 1. What Cortex is

Cortex is a desktop study OS for serious university students. It ingests anything a lecture throws at you — slides, PDFs, recorded audio, web pages, YouTube — and produces structured study materials per subject: cheatsheets, flashcards, audio overviews, infographics, slideshow videos, and a chat you can scope to a single source, a topic, or an entire subject.

The differentiator vs NotebookLM: **structured hierarchy** (Subjects → Topics → Sources), **completeness-focused cheatsheets** with enforced sections and approve-to-merge diffs (NotebookLM silently drops key points), **lecture recording as a first-class native feature**, **persistent ad-free study music**, **in-app web search via self-hosted SearXNG**, and an offline-first stance with an optional homelab accelerator for heavy jobs.

Built in Rust + Tauri + Svelte. Desktop first (Omarchy Linux), web and mobile later.

Target user persona: a final-year undergrad or postgrad who already lives in the terminal, uses a tiling window manager, owns flashcards as a study technique, and would happily learn vim bindings if it makes them faster.

---

## 2. Visual direction

**Aesthetic**: scholarly, premium, information-dense but breathable. Reads as a serious tool a grad student would brag about — not a consumer EdTech app, not a Material-Design SaaS.

**Theme system**:
- Default: dark, scholarly, low-glare
- Reads Omarchy's current theme palette dynamically from the system. Omarchy ships 14+ palettes (Tokyo Night, Catppuccin, Nord, Everforest, Gruvbox, Kanagawa, etc.). Cortex should re-skin live when the user switches Omarchy theme.
- Provide a manual override for non-Omarchy users (future web/mobile) with a hand-picked set of the same palettes.

**Typography target** (Claude Design to finalize):
- Monospace for system chrome, code, IDs, keybinding hints (suggestions: JetBrains Mono, Berkeley Mono, Geist Mono)
- Serif or premium sans for cheatsheet content + reading view (suggestions: Charter, Newsreader, Inter, Geist Sans)
- Strong typographic hierarchy — student is reading dense material; size/weight matters more than color for structure.

**Motion**: minimal, purposeful only. No bouncy easing, no decorative animation. Acceptable: subtle fade on panel mount, smooth scroll, modal slide-in. Unacceptable: confetti, page-flip, parallax.

**Density**: high but breathable. Think Linear, not Notion. Think Bloomberg Terminal-inspired (information first, chrome second) — not a copy of it.

---

## 3. Interaction model — Helix-style modal

Cortex is keyboard-first. The user has accepted vim-style modal navigation.

- **Normal mode**: navigation, manipulation. `j/k/h/l`, `gg/G`, `w/b`, `/` for in-page search, `?` for backward search, `:` for command palette.
- **Insert mode**: typing into chat, notes, fields. Esc returns to Normal.
- **Select mode**: Helix-style multi-cursor / range selection.
- **Space leader**: `space` opens a hint pane showing context-aware actions (Helix WhichKey style).
- **Command palette** (`:` or `Ctrl+P` for mouse users): fuzzy search every action, file, subject, topic, source, recent chat.

Always-visible chrome:
- **Status bar** (bottom): current mode, current scope (Subject/Topic/Source), recording indicator if live, mini music player (track + play/pause), key-hint hints if relevant.
- **Mode indicator** (left of status bar): colored block like Helix.
- **Notification toasts**: corner, dismissible with `q`.

Mouse must work too — every keyboard action has a discoverable mouse path. Cortex is keyboard-first, not keyboard-only.

---

## 4. Priority screens (v0.1)

Design these in this order. Stars mark differentiator screens — most design attention here.

1. **Subject dashboard** (entry point). Card grid of subjects. Each card shows: title, source count, "cheatsheet ready / needs review" status, recent activity, study-streak indicator. Empty state: "Add your first subject."
2. **Subject view** ⭐. Split: left nav (Topics tree), middle (cheatsheet preview + recent chats), right (sources list with type badges: PDF, PPTX, audio, web, etc.). Tabs at top: Cheatsheet | Sources | Chats | Flashcards | Quizzes | Materials.
3. **Source viewer + side chat** ⭐. Split pane: left renders the source (PDF.js for PDFs, audio waveform + transcript for recordings, web view for URLs); right is a scoped chat with citation chips that highlight passages on click. Resizable.
4. **Cheatsheet view + approve-diff modal** ⭐⭐. The killer screen. Sectioned cheatsheet (Definitions, Key Concepts, Formulas, Worked Examples, Common Pitfalls, Quick Recall — sections are per-subject editable). When a new source is added, an "auto-draft updates pending" banner appears. Clicking it opens a diff view: red/green side-by-side per section, accept-all / accept-section / reject. **This is the answer to NotebookLM's silent-deletion problem.**
5. **Hierarchical chat panel + scope switcher** ⭐. Sidebar chat. A scope chip at the top (`Subject: Algorithms` / `Topic: Recursion` / `Source: lecture-3.pdf`) — click to widen/narrow scope. Chat history grouped by scope. Streaming responses. Citation chips inline.
6. **Flashcard study session**. Anki-style card flip, SRS rating (Again / Hard / Good / Easy), session progress bar, "deck complete" celebration that is *not* obnoxious.
7. **Quiz mode**. Generated multiple-choice / short-answer / cloze deletion. One question per screen, immediate feedback after answer.
8. **Pomodoro / focus HUD**. Minimal — sits in a corner. Shows time remaining, current subject, music control. "Distraction blocker" optional toggle (hides everything except focus HUD).
9. **Lecture recorder** ⭐. Big record button, waveform visualizer, live transcription streaming below, "tag this moment" hotkey to bookmark, "stop & save" creates source.
10. **Audio overview player**. Generated podcast-style audio. Standard player chrome (play/pause/scrub) + a synced transcript view that highlights the current sentence.
11. **Sources tray**. Modal/drawer for adding a source. Four big buttons: Upload File | Paste URL | Record Lecture | Snap Photo (OCR). After source added, shows ingest progress (parsing → chunking → embedding → done).
12. **Settings — Models tab**. Per-task model assignment table: Chat / Cheatsheet synthesis / Audio overview script / Quiz generation / Flashcard generation / Embedding. Each row: provider (Gemini / Claude / OpenAI / Ollama / Custom) + model name + token budget.
13. **Settings — Homelab tab**. Toggle "Use homelab for heavy jobs", endpoint URL field, "Test connection" button, per-job toggles (Whisper, LLM, Backups).
14. **In-app SearXNG search panel**. Split pane: left is results list, right is reader view of selected result with "Add as source" button. Searches feed into chat context.
15. **Onboarding** ⭐. First-run wizard, 4-5 steps. Welcome → BYOK paste (Gemini key) → Omarchy theme detection → optional homelab setup → "create your first subject." Skippable, but should feel like a calm walkthrough not a sales pitch.
16. **Music player widget**. Persistent mini player in the status bar; expandable to a full panel with curated study stations (Lofi Girl, ChilledCow, classical, brown noise) and a YouTube URL input.

(v0.2 screens to keep in mind but not design yet: mind map / concept graph view, infographic editor, slideshow video composer, citation manager, calendar.)

---

## 5. Component inventory (recurring)

- **Status bar** (mode block, scope chip, music mini, recording indicator)
- **Command palette** (fuzzy, sectioned results)
- **Scope switcher chip** (used in chat header, also in some search contexts)
- **Cheatsheet section card** (idle, draft-pending, approved-recently states)
- **Source card** (badge for type: PDF / PPTX / DOCX / web / YT / audio / image)
- **Chat bubble** (user, assistant, system, with citation chip variant)
- **Citation chip** (clickable, hovering shows source + page/timestamp)
- **Tree view** (Subjects → Topics → Sources, collapsible)
- **Card grid** (subjects on dashboard)
- **Diff view** (cheatsheet section old vs new, line-level)
- **Notification toast** (info, warning, error, success)
- **Empty states** (one per major surface, friendly + actionable)
- **Loading states** (skeletons preferred, no spinning gifs)
- **Keyboard hint chip** (small `kbd`-style block, often inline near actions)

---

## 6. Constraints

- Renders in a Tauri webview (Chromium engine) — modern CSS, container queries, `:has()`, view transitions all fair game.
- Looks correct at 1080p, 1440p, and 4K. No fixed-px-only layouts.
- WCAG AA contrast minimum, across every Omarchy palette.
- Keyboard-first means visible key hints near non-obvious actions, but they should not be visually overwhelming.
- Local SQLite means most operations are instant — do not perform "loading shimmer" theatre. Show skeletons only when actually waiting on AI/network.
- The user's machine is fast and lightweight (Omarchy + Hyprland). Cortex should *feel* native, not Electron-bloated, even though Tauri ships a webview.

---

## 7. What I need back from Claude Design

**Must-have for v0.1 build to start**:
- High-fidelity mocks of screens 2, 3, 4, 5, 9, 15 (the starred ones + onboarding)
- A design system spec: typography stack, spacing scale, color token names (mapped to Omarchy variables), motion principles, elevation rules
- Component library: every item in §5 above, in default + hover + active + disabled states
- The cheatsheet **approve-diff micro-interaction** as a video or click-through prototype — this is the differentiator screen and motion matters
- The **hierarchical chat scope switcher** pattern, including transitions when widening/narrowing scope mid-conversation

**Nice-to-have**:
- Mocks of screens 1, 6, 8, 10, 11, 12, 16
- Dark + at least 2 alternative Omarchy palettes (Tokyo Night + Catppuccin Mocha would be good demos)
- A landing page mock for the eventual web release

**Format**:
- Figma file or Claude Design canvas link
- Exported design tokens as JSON (Style Dictionary or DTCG format)
- SVG / PNG exports of icons and illustrations

---

## 8. Tone & references

**Pull from**:
- **Linear** — density, polish, motion economy, command palette UX
- **Helix editor** — modal interaction, status line, keybinding visibility
- **Reflect.app** — calm scholarly knowledge work
- **Tana** — information density without claustrophobia
- **Obsidian** — pane management, plugin-system visual style (for our v0.x plugin future)
- **Arc browser** — sidebar-first organization
- **Anki** — study session pacing (but more polished)
- **Bloomberg Terminal** — info density philosophy (not visual copy)

**Avoid**:
- NotebookLM's whitespace-heavy generic-Material look
- Generic SaaS dashboard look (rounded cards on grey)
- Consumer EdTech look (Duolingo, Khan Academy — too gamified, too playful)
- Glassmorphism (too 2021)
- Skeuomorphism

---

## 9. Open questions for Claude Design to answer

1. Serif or sans for cheatsheet content — defend the pick with a sample
2. Should the scope switcher be a dropdown, a segmented control, or a breadcrumb-style path?
3. Cheatsheet diff view: side-by-side, inline (Git-style), or hybrid?
4. Lecture recorder waveform: real-time amplitude bars, or a scrubbable timeline that builds left-to-right?
5. How does the Pomodoro HUD relate to the status bar — replace it, sit above it, or float?

---

*End of brief.*
