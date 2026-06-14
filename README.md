<div align="center">

<img src="assets/cortex-logo-withbg.png" width="128" alt="Cortex logo" />

# Cortex

**A local-first, open-source NotebookLM alternative — a desktop study OS for serious learners.**

Ingest everything a course throws at you — slides, PDFs, docs, lecture recordings, web pages, YouTube — into a clean **Subjects → Topics → Sources** hierarchy, then turn it into exam-ready study material: cheatsheets, flashcards with spaced repetition, quizzes, two-host audio overviews, infographics, mind maps, and a citation-grounded chat.

Your data never leaves your machine, and you bring your own AI keys.

<p>
  <img alt="License" src="https://img.shields.io/badge/license-Apache--2.0-blue.svg" />
  <img alt="Built with Tauri" src="https://img.shields.io/badge/built%20with-Tauri%202-24C8DB.svg" />
  <img alt="Frontend Svelte 5" src="https://img.shields.io/badge/frontend-Svelte%205-FF3E00.svg" />
  <img alt="Backend Rust" src="https://img.shields.io/badge/backend-Rust-CE422B.svg" />
  <img alt="Platform" src="https://img.shields.io/badge/platform-Linux%20%C2%B7%20macOS%20%C2%B7%20Windows-555.svg" />
  <img alt="Status" src="https://img.shields.io/badge/status-v1.0-2dd5b7.svg" />
</p>

[Why Cortex?](#why-cortex) · [Features](#features) · [Download](#download--install) · [Build from source](#build-from-source) · [Configuration](#configuration) · [How it works](#how-it-works) · [Roadmap](#roadmap) · [Contributing](#contributing)

</div>

---

## Why Cortex?

NotebookLM is great, but it's a web product: your sources live on someone else's servers, you can't pick your model, and it's organized around loose "notebooks." Cortex is the opposite:

| | NotebookLM | **Cortex** |
|---|---|---|
| Data location | Cloud | **100% local SQLite on your machine** |
| AI model | Fixed | **Bring your own** — Gemini, OpenRouter, OpenAI, Claude, or local Ollama |
| Structure | Flat notebooks | **Subjects → Topics → Sources** |
| Study materials | Audio + notes | Cheatsheets, **FSRS flashcards**, quizzes, **timed graded exams**, audio, **infographics, mind maps**, slides |
| Citations & deadlines | — | **Built-in reference manager (APA/MLA) + exam/assignment tracking** |
| Export | — | **Anki `.apkg`**, PDF, portable SQLite, encrypted backups |
| Cost | Subscription | **Free & open source** (you pay only your own API usage) |

## Features

### Ingest anything
- **Sources:** PDF, PPTX, DOCX, plain text/Markdown, web pages, YouTube, audio recordings, and images via vision-model OCR — **including photographed handwritten notes**.
- Office docs are rendered to PDF for faithful slide previews; PDFs use real text extraction.
- Each source is parsed → chunked → embedded → stored with **live progress**, then becomes searchable and citable.

### Generate study material
- **Cheatsheets** — exhaustive, exam-focused, completeness-checked synthesis with callouts, tables, and bar charts. Optional **web-sourced diagrams** per section, with versioned drafts and an approve-diff review flow.
- **Flashcards** with **FSRS spaced repetition** (the modern memory-model scheduler; Again/Hard/Good/Easy, "Study due · N").
- **Exam mode** — timed mock papers (MCQ + written) generated from your sources; MCQs grade locally, written answers are graded by your model with a verify-before-score rubric, per-question feedback, weak-topic callouts, and a one-click **remark** that re-grades with the identical rubric.
- **Quizzes** — multiple-choice with explanations.
- **Audio overviews** — two-host, podcast-style spoken walkthroughs, rendered to real audio via cloud TTS (on-device speech synthesis as fallback).
- **Infographics** — detailed HTML posters with a **timeline of events**, key stats, and a takeaway.
- **Mind maps** — hierarchical concept maps.
- **Slideshows** — presentation outlines.
- Every generator accepts an optional **custom prompt** (NotebookLM-style) to steer focus and tone.

### Chat that cites its sources
- Ask questions scoped to a subject, topic, a tag, or specific sources; answers cite inline as `⟦source · location⟧`.
- **Web mode** 🌐 pulls in live web results and, for visual questions, fetches **images and diagrams** alongside the answer (via your SearXNG).
- Hybrid retrieval: vector search (sqlite-vec) **+** keyword search, merged.

### Stay organized
- **Citation manager** — per-subject bibliography with APA/MLA/Harvard formatting and one-click copy.
- **Deadlines & calendar** — track exams and assignments, with a deadline study checklist and two-way Citations ↔ Calendar sync.
- **Tags** on topics and deadlines for cross-cutting organization.
- **Notes**, a **lecture recorder** with a near-real-time live transcript (~7s adaptive Whisper segments, silence-aware) and an **automatic lecture summary** saved to Notes after every recording.
- **Insights** — a study-analytics dashboard: GitHub-style year heatmap of focus hours (pomodoro + passive in-app time), reviews/accuracy/streaks, a 7-day due forecast, and a "topics needing work" ranking.
- **Global semantic search** (`Ctrl+K`) across notes, sources, transcripts, events, and materials — exact matches first, vector hits as related content.
- A **Pomodoro** focus timer and background **music**: curated ad-free YouTube stations (lofi, synthwave, jazz, classical, rain, forest, 40 Hz) streamed through a headless mpv sidecar, plus your own custom stations.

### Own your data
- **Anki `.apkg` import _and_ export** for flashcard decks, **PDF export**, and a portable **SQLite dump**.
- **Encrypted homelab backups** — snapshot → `age` encrypt → `rclone` upload.
- Everything is local; AI is **bring-your-own-key**; web search is your **self-hosted SearXNG**.

### Built for power users
- **Helix-style modal keyboard engine** with a command palette, leader keys, and fully customizable bindings.
- **10 themes** and live re-skinning (designed to follow the Omarchy palette).
- **Close-to-tray** — closing the window keeps ingest/generation/music running behind a tray icon (Open · Play/Pause · Quit); reminders become OS notifications while hidden.
- A responsive shell that stays intentional when tiled narrow (drawer sidebar below 1080px).

## Download & install

Grab the latest build for your operating system from the
**[Releases page](https://github.com/PndaMan/cortex/releases/latest)** — no
toolchain required.

### macOS
1. Download **`Cortex_x.y.z_universal.dmg`** (works on both Apple Silicon and Intel).
2. Open the `.dmg` and drag **Cortex** into **Applications**.
3. First launch: right-click **Cortex → Open** (the build isn't notarized yet, so
   macOS asks you to confirm an unidentified developer once).

### Windows
1. Download **`Cortex_x.y.z_x64-setup.exe`**.
2. Run it. SmartScreen may warn about an unrecognized app — click
   **More info → Run anyway** (the installer isn't code-signed yet).
3. Launch **Cortex** from the Start menu.

### Linux
| Distro family | File | Install |
|---|---|---|
| Debian / Ubuntu | `Cortex_x.y.z_amd64.deb` | `sudo apt install ./Cortex_*.deb` |
| Fedora / RHEL | `Cortex-x.y.z-1.x86_64.rpm` | `sudo dnf install ./Cortex-*.rpm` |
| Any (portable) | `Cortex_x.y.z_amd64.AppImage` | `chmod +x Cortex_*.AppImage && ./Cortex_*.AppImage` |

> Builds are produced automatically by CI ([`.github/workflows/release.yml`](.github/workflows/release.yml))
> on every `v*` tag. They are **not yet code-signed/notarized**, hence the
> one-time security prompts above.

## Build from source

### Prerequisites
- **Rust** (stable) and the [Tauri 2 system dependencies](https://v2.tauri.app/start/prerequisites/) for your OS (on Linux: WebKitGTK 4.1, GTK 3, libsoup3, etc.).
- **Node 18+** or **[Bun](https://bun.sh)** for the frontend.

```bash
git clone https://github.com/PndaMan/cortex.git
cd cortex
bun install          # or: npm install
bun run tauri dev    # launches the desktop app with hot reload
```

Build a production bundle:

```bash
bun run tauri build
```

> **Most tools ship inside the release builds.** `yt-dlp` (YouTube + music),
> `age`, and `rclone` (encrypted backups) are bundled as sidecars by
> [`scripts/fetch-sidecars.mjs`](scripts/fetch-sidecars.mjs) — no manual install.
> The table below applies mainly to running from source or to the few heavier,
> still-optional tools.

### Optional integrations (enable the features you want)
| Feature | Needs | Notes |
|---|---|---|
| Lecture transcription | local [`openai-whisper`](https://github.com/openai/whisper) / whisper.cpp / auto `faster-whisper`, **or** a homelab Whisper URL | set the Whisper URL in Settings → Integrations to skip local setup |
| Web search / images | a self-hosted [SearXNG](https://docs.searxng.org/) with JSON output enabled | set its URL in Settings |
| Local models | [Ollama](https://ollama.com) (local or homelab) | keyless chat + embeddings |
| Slide previews | LibreOffice (`soffice`) | renders PPTX/DOCX to PDF |
| PDF text | `pdftotext` (poppler) | faster & cleaner than OCR |
| YouTube + music | [`yt-dlp`](https://github.com/yt-dlp/yt-dlp) (**bundled**) + [`mpv`](https://mpv.io/) for playback | yt-dlp ships with the app; mpv still system-installed |
| Encrypted backups | [`age`](https://github.com/FiloSottile/age) + [`rclone`](https://rclone.org/) (**both bundled**) | configure in Settings → Backups |

> **One-command homelab backend.** Don't want to set these up by hand? The
> [`homelab/`](homelab/) folder has a `docker compose` stack that runs SearXNG,
> an OpenAI-compatible Whisper server, and (optionally) Ollama. Bring it up,
> expose it over a reverse proxy or VPN (Tailscale / Netbird), and point Cortex
> at it in **Settings → Integrations**. See [homelab/README.md](homelab/README.md).

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
src/                       Svelte 5 frontend (runes)
  lib/api.ts               typed Tauri command client (mirrors the Rust commands)
  lib/store.svelte.ts      central app state
  lib/keybinds.svelte.ts   Helix-style modal keyboard engine
  components/  views/       UI

src-tauri/src/
  commands.rs              command surface (ingest, generate, chat, search, export…)
  llm.rs                   LLM providers behind one trait (Gemini/OpenAI-compat/Claude/Ollama, BYOK)
  embed.rs                 embedding providers
  vector.rs                f32 BLOB vectors; sqlite-vec ranks, Rust cosine is the fallback
  ingest.rs                parse → chunk → embed → store pipeline
  repo.rs                  all SQLite access
  anki.rs   backup.rs       .apkg export · encrypted backups
  google.rs  notes.rs       Google API helpers · notes
  calendar.rs  review.rs    deadlines/calendar · FSRS scheduling
  exam.rs  analytics.rs   timed exams + grading · study analytics
  mpv.rs                   background music playback
  migrations/              versioned SQLite schema
```

- **Storage:** a single SQLite database in the OS app-data dir, with WAL mode and versioned migrations. Embeddings are stored as little-endian `f32` BLOBs — exactly sqlite-vec's compact format — so retrieval ranks in SQL via `vec_distance_cosine` (with a Rust cosine scan as a transparent fallback).
- **Privacy:** all content, embeddings, and generated material stay in that local DB. The only outbound calls are to the AI provider you configured and (optionally) your own SearXNG.

See [`CORTEX_DESIGN_BRIEF.md`](CORTEX_DESIGN_BRIEF.md) for the full product vision and the locked architecture decisions.

## Tech stack

**Tauri 2** · **Rust** · **SQLite + sqlite-vec** · **Svelte 5** (runes) · **TypeScript** · **Tailwind CSS** · **Vite**

## Roadmap

**Shipped (v1.0):** the Subjects→Topics→Sources core, the ingestion pipeline, all study-material generators (cheatsheets, flashcards, quizzes, audio, infographics, mind maps, slides), citation-grounded chat with web/image mode, FSRS spaced repetition, Anki import/export, timed graded exams with remark, the Insights analytics dashboard (year focus heatmap, weak-topic ranking), global semantic search, auto lecture summaries, close-to-tray, a citation manager + deadlines with calendar sync, tags, sqlite-vec search, encrypted backups, live lecture transcription, drag-and-drop reordering, and a responsive layout that scales toward mobile.

**Next:** accounts & sync (CRDT multi-device), a mobile build, and richer calendar/citation workflows.

## Contributing

Issues and PRs are welcome. The codebase favors **surgical, well-tested changes**:

```bash
bun run check                       # svelte-check (frontend types)
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
