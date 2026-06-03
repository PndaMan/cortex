//! Notes command surface. Mirrored 1:1 in `src/lib/api.ts`. Sync commands lock
//! the DB inline like `commands.rs`; `note_to_source` does heavy (embed) work so
//! it runs in `spawn_blocking` exactly like `add_source` to avoid the
//! reqwest::blocking-in-async runtime panic.

use crate::db::AppState;
use crate::embed;
use crate::error::{Error, Result};
use crate::ingest;
use crate::models::*;
use crate::repo;
use crate::vector::f32s_to_blob;
use tauri::{AppHandle, Manager, State};

#[tauri::command]
pub fn create_note(
    state: State<AppState>,
    subject_id: Option<String>,
    topic_id: Option<String>,
    title: String,
    body: String,
) -> Result<Note> {
    let c = state.db.lock().unwrap();
    let id = repo::insert_note(
        &c,
        subject_id.as_deref(),
        topic_id.as_deref(),
        &title,
        &body,
    )?;
    repo::get_note(&c, &id)
}

#[tauri::command]
pub fn list_notes(state: State<AppState>, subject_id: Option<String>) -> Result<Vec<Note>> {
    let c = state.db.lock().unwrap();
    repo::list_notes(&c, subject_id.as_deref())
}

#[tauri::command]
pub fn get_note(state: State<AppState>, id: String) -> Result<Note> {
    let c = state.db.lock().unwrap();
    repo::get_note(&c, &id)
}

#[tauri::command]
pub fn update_note(
    state: State<AppState>,
    id: String,
    title: String,
    body: String,
) -> Result<Note> {
    let c = state.db.lock().unwrap();
    repo::update_note(&c, &id, &title, &body)?;
    repo::get_note(&c, &id)
}

#[tauri::command]
pub fn delete_note(state: State<AppState>, id: String) -> Result<()> {
    let c = state.db.lock().unwrap();
    repo::delete_note(&c, &id)
}

/// Convert a note into a first-class source: create a `kind="note"` source,
/// link it back to the note, then chunk + embed the note body using the same
/// pipeline `add_source` uses. Heavy work runs in `spawn_blocking`.
#[tauri::command]
pub async fn note_to_source(app: AppHandle, id: String) -> Result<IngestResult> {
    tauri::async_runtime::spawn_blocking(move || -> Result<IngestResult> {
        let state = app.state::<AppState>();

        // 1. read the note + create the source row (locked)
        let (text, source_id, subject_id, topic_id) = {
            let c = state.db.lock().unwrap();
            let note = repo::get_note(&c, &id)?;
            let subject_id = note.subject_id.clone().ok_or_else(|| {
                Error::Other(
                    "note has no subject — assign it to a subject before converting".into(),
                )
            })?;
            let name = if note.title.trim().is_empty() {
                "Untitled note".to_string()
            } else {
                note.title.clone()
            };
            let topic_id = note.topic_id.clone();
            let source_id =
                repo::insert_source(&c, &subject_id, topic_id.as_deref(), &name, "note", None)?;
            repo::set_note_source(&c, &id, &source_id)?;
            (note.body, source_id, subject_id, topic_id)
        };

        let chars = text.chars().count() as i64;
        let chunks = ingest::chunk_text(&text, 900, 150);

        // 2. embed (no lock) — same settings-driven embedder as add_source
        let (provider, gemini_key, ollama_url) = {
            let c = state.db.lock().unwrap();
            (
                repo::get_setting(&c, "embed_provider")?.unwrap_or_else(|| "stub".into()),
                repo::get_setting(&c, "gemini_api_key")?,
                repo::get_setting(&c, "ollama_url")?,
            )
        };
        let embedder =
            embed::from_settings(&provider, gemini_key.as_deref(), ollama_url.as_deref());
        let vectors = match ingest::embed_chunks(embedder.as_ref(), &chunks) {
            Ok(v) => v,
            Err(_) => {
                // fall back to the stub so conversion never hard-fails on a bad key
                let stub = embed::StubEmbedder;
                ingest::embed_chunks(&stub, &chunks)?
            }
        };

        // 3. store chunks + finalize (locked)
        let c = state.db.lock().unwrap();
        for (i, (chunk, vec)) in chunks.iter().zip(vectors.iter()).enumerate() {
            repo::insert_chunk(
                &c,
                &source_id,
                &subject_id,
                topic_id.as_deref(),
                i as i64,
                chunk,
                None,
                vec.len() as i64,
                &f32s_to_blob(vec),
            )?;
        }
        let chunk_count = repo::count_chunks(&c, &source_id)?;
        let status = if chunks.is_empty() { "draft" } else { "ready" };
        let meta = if chunks.is_empty() {
            "empty note".to_string()
        } else {
            format!("{chunk_count} chunks · {chars} chars · from note")
        };
        repo::finalize_source(&c, &source_id, status, Some(&meta), Some(&text), None)?;

        let source = repo::get_source(&c, &source_id)?;
        Ok(IngestResult {
            source,
            chunk_count,
            chars,
            warning: None,
        })
    })
    .await
    .map_err(|e| Error::Other(format!("background task failed: {e}")))?
}
