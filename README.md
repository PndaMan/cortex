<div align="center">

# Cortex

**A local-first, open-source NotebookLM alternative — a desktop study OS for serious learners.**

Ingest everything a course throws at you — slides, PDFs, docs, lecture recordings, web pages, YouTube — into a clean **Subjects → Topics → Sources** hierarchy, then turn it into exam-ready study material: cheatsheets, flashcards with spaced repetition, quizzes, two-host audio overviews, infographics, mind maps, and a citation-grounded chat.

Built with **Tauri 2 · Rust · SQLite + sqlite-vec · Svelte 5 · Tailwind**. Your data never leaves your machine, and you bring your own AI keys.

[Features](#features) · [Quick start](#quick-start) · [Configuration](#configuration) · [How it works](#how-it-works) · [Roadmap](#roadmap)

</div>

---

## Why Cortex?

NotebookLM is great, but it's a web product: your sources live on someone else's servers, you can't pick your model, and it's organized around loose "notebooks." Cortex is the opposite:

| | NotebookLM | **Cortex** |
|---|---|---|
| Data location | Cloud | **100% local SQLite on your machine** |
| AI model | Fixed | **Bring your own** — Gemini, OpenRouter, OpenAI, Claude, or local Ollama |
| Structure | Flat notebooks | **Subjects → Topics → Sources** |
| Study materials | Audio + notes | Cheatsheets, **SM-2 flashcards**, quizzes, audio, **infographics, mind maps**, slides |
| Citations & deadlines | — | **Built-in reference manager (APA/MLA) + exam/assignment tracking** |
| Export | — | **Anki `.apkg`**, PDF, portable SQLite, encrypted backups |
| Cost | Subscription | **Free & open source** (you pay only your own API usage) |

## Features

### Ingest anything
- **Sources:** PDF, PPTX, DOCX, plain text/Markdown, web pages, YouTube, audio recordings, and images (via vision-model OCR).
- Office docs are rendered to PDF for faithful slide previews; PDFs use real text extraction.
- Each source is parsed → chunked → embedded → stored with **live progress**, then becomes searchable and citable.

### Generate study material
- **Cheatsheets** — exhaustive, exam-focused, completeness-checked synthesis with callouts, tables, and bar charts. Optional **web-sourced diagrams** per section.
- **Flashcards** with real **SM-2 spaced repetition** ("Study due · N", Again/Hard/Good/Easy scheduling).
- **Quizzes** — multiple-choice with explanations.
- **Audio overviews** — two-host, podcast-style spoken walkthroughs.
- **Infographics** — detailed HTML posters with a **timeline of events**, key stats, and a takeaway.
- **Mind maps** — hierarchical concept maps.
- **Slideshows** — presentation outlines.
- Every generator accepts an optional **custom prompt** (NotebookLM-style) to steer focus and tone.

### Chat that cites its sources
- Ask questions scoped to a subject, topic, or specific sources; answers cite inline as `⟦source · location⟧`.
- **Web mode** 🌐 pulls in live web results and, for visual questions, fetches **images and diagrams** alongside the answer (via your SearXNG).
- Hybrid retrieval: vector search (sqlite-vec) **+** keyword search, merged.

### Stay organized
- **Citation manager** — per-subject bibliography with APA/MLA formatting and one-click copy.
- **Deadlines & calendar** — track exams and assignments with reminders.
- **Notes**, a **lecture recorder** with an incremental live transcript, a **Pomodoro** focus timer, and background **music**.

### Own your data
- **Anki `.apkg` export** for flashcard decks, **PDF export**, and a portable **SQLite dump**.
- **Encrypted homelab backups** — snapshot → `age` encrypt → `rclone` upload.
- Everything is local; AI is **bring-your-own-key**; web search is your **self-hosted SearXNG**.

### Built for power users
- **Helix-style modal keyboard engine** with a command palette, leader keys, and fully customizable bindings.
- **10 themes** and live re-skinning (designed to follow the Omarchy palette).

## Quick start

### Prerequisites
- **Rust** (stable) and the [Tauri 2 system dependencies](https://v2.tauri.app/start/prerequisites/) for your OS (on Linux: WebKitGTK 4.1, GTK 3, libsoup3, etc.).
- **Node 18+** (or **Bun**) for the frontend.

```bash
git clone https://github.com/<your-username>/cortex.git
cd cortex
npm install          # or: bun install
npm run tauri dev    # launches the desktop app with hot reload
```

Build a production bundle:

```bash
npm run tauri build
```

### Optional integrations (enable the features you want)
| Feature | Needs | Notes |
|---|---|---|
| Lecture transcription | [`openai-whisper`](https://github.com/openai/whisper) or whisper.cpp on `PATH` | local, free |
| Web search / images | a self-hosted [SearXNG](https://docs.searxng.org/) with JSON output enabled | set its URL in Settings |
| Slide previews | LibreOffice (`soffice`) | renders PPTX/DOCX to PDF |
| PDF text | `pdftotext` (poppler) | faster & cleaner than OCR |
| Encrypted backups | [`age`](https://github.com/FiloSottile/age) + [`rclone`](https://rclone.org/) | configure in Settings → Backups |

## Configuration

Open **Settings** in the app:

1. **API keys (bring your own).** Paste a key for any of: Google **Gemini**, **OpenRouter**, **OpenAI**, **Anthropic Claude**, or point at a local **Ollama** / custom OpenAI-compatible endpoint. Generation works as soon as *any* key is set.
2. **Per-task models.** Assign a provider/model to each task (chat, cheatsheet, flashcards, quiz, audio, embeddings) — e.g. a cheap flash model for cards, a stronger one for cheatsheets.
3. **Profile.** Your name, level, and field personalize tone and examples.
4. **Web search.** Enter your SearXNG base URL to unlock chat web mode + image fetching.
5. **Backups.** Add an `age` recipient public key and an `rclone` remote to enable encrypted backups.

> No keys? The app still runs end-to-end with a clearly-marked offline stub so you can explore the flow.

## How it works

```
src/                      Svelte 5 frontend (runes)
  lib/api.ts              typed Tauri command client (mirrors the Rust commands)
  lib/store.svelte.ts     central app state
  lib/keybinds.svelte.ts  Helix-style modal keyboard engine
  components/  views/      UI

src-tauri/src/
  commands.rs             command surface (ingest, generate, chat, search, export…)
  llm.rs                  LLM providers behind one trait (Gemini/OpenAI-compat/Claude/Ollama, BYOK)
  embed.rs                embedding providers
  vector.rs               f32 BLOB vectors; sqlite-vec ranks, Rust cosine is the fallback
  repo.rs                 all SQLite access
  ingest.rs               parse → chunk → embed → store pipeline
  anki.rs  backup.rs       .apkg export · encrypted backups
  migrations/             versioned SQLite schema
```

- **Storage:** a single SQLite database in the OS app-data dir, with WAL mode and versioned migrations. Embeddings are stored as little-endian `f32` BLOBs — exactly sqlite-vec's compact format — so retrieval ranks in SQL via `vec_distance_cosine` (with a Rust cosine scan as a transparent fallback).
- **Privacy:** all content, embeddings, and generated material stay in that local DB. The only outbound calls are to the AI provider you configured and (optionally) your own SearXNG.

See [`CORTEX_DESIGN_BRIEF.md`](CORTEX_DESIGN_BRIEF.md) for the full product vision and the locked architecture decisions.

## Roadmap

**Shipped:** the Subjects→Topics→Sources core, the ingestion pipeline, all study-material generators (cheatsheets, flashcards, quizzes, audio, infographics, mind maps, slides), citation-grounded chat with web/image mode, SM-2 spaced repetition, Anki export, a citation manager + deadlines, sqlite-vec search, encrypted backups, live lecture transcription, and a responsive layout that scales toward mobile.

**Next:** accounts & sync (CRDT multi-device), a mobile build, and richer calendar/citation workflows.

## Contributing

Issues and PRs are welcome. The codebase favors **surgical, well-tested changes**:

```bash
npm run check                       # svelte-check (frontend types)
cd src-tauri && cargo test --lib    # Rust unit tests
cargo check                         # fast type/borrow check
```

Please keep changes scoped, match the surrounding style, and add a test when you touch backend logic.

## License

[Apache License 2.0](LICENSE).

---

<div align="center">
<sub>Cortex — study like you own your data, because you do.</sub>
</div>
