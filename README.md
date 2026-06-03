# Cortex

A desktop **study OS** for university students — "NotebookLM, but structured and
serious." Ingest anything a lecture throws at you (slides, PDFs, recordings, web,
YouTube) into **Subjects → Topics → Sources**, and generate completeness-focused
cheatsheets, flashcards, quizzes, audio overviews, and scoped chat.

Built with **Tauri 2 · Rust · SQLite (+ vector search) · Svelte 5 · Tailwind**.
Desktop-first on Omarchy Linux. See `CORTEX_DESIGN_BRIEF.md` for the full product
vision and the locked design decisions.

> **Status: v0.1 foundation slice.** This milestone delivers the app shell,
> the Subjects→Topics→Sources data core, and a working **source-ingestion
> pipeline** (parse → chunk → embed → store) with live progress. Chat, the
> cheatsheet diff/approve flow, flashcards/quizzes, materials, the lecture
> recorder, music, and web search are scaffolded for later slices.

## Architecture

```
src/                  Svelte 5 frontend (runes)
  lib/api.ts          typed Tauri command client (mirrors commands.rs)
  lib/store.svelte.ts central app state (runes class)
  lib/keyboard.ts     Helix-style modal keyboard engine
  components/         Sidebar, StatusBar, CommandPalette, ToastStack, Icon
  views/              Dashboard, SubjectView, AddSource
  styles/             design-system CSS (verbatim from the Claude Design handoff)
src-tauri/            Rust backend
  migrations/         SQLite schema (user_version migrations)
  src/db.rs           connection + migration runner + AppState
  src/repo.rs         CRUD + tree assembly + cosine search
  src/ingest.rs       detect → parse (txt/md/html + libreoffice for pdf/docx/pptx) → chunk
  src/embed.rs        Embedder trait: stub (default) · Gemini · Ollama
  src/vector.rs       f32 BLOB (de)serialize + cosine  (sqlite-vec is the drop-in upgrade)
  src/commands.rs     the Tauri command surface
```

**Embeddings & cost:** embeddings are stored as `f32` BLOBs and searched with a
Rust-side cosine scan behind a small seam, so dropping in `sqlite-vec` later is a
localized change. The default embedder is a dependency-free deterministic **stub**
(works fully offline, zero config). Set `embed_provider` to `gemini`
(text-embedding-004, needs `gemini_api_key`) or `ollama` (`nomic-embed-text`) in the
`settings` table to use a real provider.

## Prerequisites

- Rust + Cargo, [Bun](https://bun.sh), and the Tauri 2 Linux deps
  (webkit2gtk-4.1, gtk3, libsoup3 — already present on this machine).
- **LibreOffice** (used headless to extract text from PDF/DOCX/PPTX).
- Optional, for later slices: `ollama`, `whisper`, `yt-dlp`, `ffmpeg`.

## Run

```bash
bun install                 # frontend deps + Tauri CLI
bun run tauri dev           # launches the desktop app (Vite + Rust)
```

First launch seeds a few demo subjects so the UI has content. Add a source from
**Add source** → *Paste URL* / *Paste text* / *Upload file*; watch the ingest
progress bar, then find it under the subject's **Sources** tab.

## Develop / test

```bash
# frontend
bunx vite build
bunx svelte-check --tsconfig ./tsconfig.json

# backend
cd src-tauri && cargo test       # 11 unit tests (db, repo, ingest, embed, vector)
cargo build                      # full app binary
```

## Keyboard (Helix-style, foundation subset)

`:` command palette · `n` add source · `t` cycle Omarchy theme · `g d` dashboard ·
`j/k/h/l` navigate dashboard · `Enter` open · `q` dismiss toast · `Esc` normal mode.
Every keyboard action also has a mouse path.
