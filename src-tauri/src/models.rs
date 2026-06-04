use serde::{Deserialize, Serialize};

/// Subject — top of the Subjects → Topics → Sources hierarchy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Subject {
    pub id: String,
    pub name: String,
    pub code: Option<String>,
    pub glyph: String,
    pub color: Option<String>,
    pub status: String,
    pub streak: i64,
    pub position: i64,
    #[serde(rename = "sourceCount")]
    pub source_count: i64,
    pub topics: Vec<Topic>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Topic {
    pub id: String,
    pub subject_id: String,
    pub name: String,
    pub glyph: Option<String>,
    pub position: i64,
    pub sources: Vec<Source>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Source {
    pub id: String,
    pub subject_id: String,
    pub topic_id: Option<String>,
    pub name: String,
    pub kind: String,
    pub status: String,
    pub meta: Option<String>,
    pub origin: Option<String>,
    pub error: Option<String>,
    /// Extracted plaintext (for txt/md/url and as fallback display text).
    pub content: Option<String>,
    /// Stable on-disk path to the original (or rendered PDF) for preview.
    pub stored_path: Option<String>,
    pub tags: Vec<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

/// Result of an ingestion run, returned by `add_source`.
#[derive(Debug, Clone, Serialize)]
pub struct IngestResult {
    pub source: Source,
    pub chunk_count: i64,
    pub chars: i64,
    pub warning: Option<String>,
}

/// A stored chunk's info, returned by `list_chunks` — lets the UI prove a
/// source was actually parsed + embedded (text + vector dimension).
#[derive(Debug, Clone, Serialize)]
pub struct ChunkInfo {
    pub ord: i64,
    pub text: String,
    pub dim: i64,
    pub loc: Option<String>,
}

/// A retrieved chunk + its similarity score, returned by `search_chunks`.
#[derive(Debug, Clone, Serialize)]
pub struct ChunkHit {
    pub id: String,
    pub source_id: String,
    pub source_name: String,
    pub text: String,
    pub loc: Option<String>,
    pub score: f32,
}

// ---- chat (RAG) -------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct Citation {
    pub source_name: String,
    pub loc: Option<String>,
    pub snippet: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChatAnswer {
    pub text: String,
    pub citations: Vec<Citation>,
    pub model: String,
}

/// A persisted chat message (belongs to one conversation thread).
#[derive(Debug, Clone, Serialize)]
pub struct ChatMsg {
    pub role: String,
    pub text: String,
    pub created_at: i64,
}

/// Summary of a single conversation thread for the thread list/switcher.
#[derive(Debug, Clone, Serialize)]
pub struct ThreadInfo {
    pub id: String,
    pub title: String,
    pub updated_at: i64,
    pub count: i64,
}

// ---- cheatsheet -------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CsItem {
    pub t: String,
    pub d: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CsSection {
    pub id: String,
    pub title: String,
    #[serde(default = "default_state")]
    pub state: String,
    pub items: Vec<CsItem>,
}

fn default_state() -> String {
    "approved".into()
}

#[derive(Debug, Clone, Serialize)]
pub struct CheatsheetData {
    pub subject: String,
    pub topic: String,
    pub sources: i64,
    pub model: String,
    pub sections: Vec<CsSection>,
}

// ---- generated materials ---------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct MaterialRec {
    pub id: String,
    pub kind: String,
    pub title: String,
    pub topic: String,
    pub meta: String,
    pub status: String,
    pub payload: serde_json::Value,
}

/// Payload the frontend sends to ingest a new source.
#[derive(Debug, Clone, Deserialize)]
pub struct AddSourceInput {
    pub subject_id: String,
    pub topic_id: Option<String>,
    pub name: Option<String>,
    pub kind: Option<String>,
    /// One of: inline `text`, a local `path`, or a `url`.
    pub text: Option<String>,
    pub path: Option<String>,
    pub url: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
}

// ---- web search -------------------------------------------------------

/// A single web search result from a SearXNG instance.
#[derive(Debug, Clone, Serialize)]
pub struct WebResult {
    pub title: String,
    pub url: String,
    pub host: String,
    pub snippet: String,
    pub engine: String,
}

// ---- long-term memory -------------------------------------------------

/// A manually-saved long-term memory fact, injected into chat/synthesis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Memory {
    pub id: String,
    pub content: String,
    pub source: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

// ---- notes ------------------------------------------------------------

/// A free-text note. Can be "converted" into a first-class source (chunked +
/// embedded) later, at which point `source_id` links to the generated source.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Note {
    pub id: String,
    pub subject_id: Option<String>,
    pub topic_id: Option<String>,
    pub title: String,
    pub body: String,
    pub source_id: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

// ---- calendar events / tasks ------------------------------------------

/// A calendar event or task. `kind` is "event" or "task"; reminders are an
/// absolute epoch-ms timestamp the frontend polls (`check_reminders`).
/// `google_id` exists for a later Google Calendar sync slice.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalEvent {
    pub id: String,
    pub subject_id: Option<String>,
    pub title: String,
    pub description: Option<String>,
    pub location: Option<String>,
    pub color: Option<String>,
    pub start_ms: i64,
    pub end_ms: Option<i64>,
    pub all_day: bool,
    pub kind: String, // event | task
    pub done: bool,
    pub reminder_ms: Option<i64>,
    pub notified: bool,
    pub google_id: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

// ---- review (spaced repetition over wrong answers) --------------------

/// One answered quiz/flashcard item. The review set is built from the latest
/// attempt per `item_key`. Mirrored in `api.ts` for the frontend; the Rust side
/// stores attempts via raw params, so the struct itself is not yet constructed.
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attempt {
    pub id: String,
    pub subject_id: String,
    pub material_id: Option<String>,
    pub kind: String, // quiz | flashcard
    pub item_index: i64,
    pub item_key: String,
    pub correct: bool,
    pub created_at: i64,
}

/// A single "re-study this" item returned by `review_set`.
#[derive(Debug, Clone, Serialize)]
pub struct ReviewItem {
    pub item_index: i64,
    pub item_key: String,
}

/// A card due for spaced-repetition review (`srs_due`). `item_key` matches the
/// flashcard front / quiz question so the frontend can locate the live card.
#[derive(Debug, Clone, Serialize)]
pub struct DueCard {
    pub item_index: i64,
    pub item_key: String,
    pub due_at: i64,
    pub reps: i64,
    pub interval_d: i64,
}

/// Result of grading a card with SM-2 — the updated schedule, so the UI can show
/// "next due in N days" feedback.
#[derive(Debug, Clone, Serialize)]
pub struct SrsResult {
    pub due_at: i64,
    pub interval_d: i64,
    pub reps: i64,
    pub ease: f64,
}

/// Per-kind due/total counts for surfacing "N due today".
#[derive(Debug, Clone, Serialize)]
pub struct SrsStats {
    pub due: i64,
    pub total: i64,
}

/// A reference/citation in a subject's bibliography. Formatting (APA/MLA) is done
/// in the frontend from these fields. (Named `Reference` to avoid clashing with
/// the chat-snippet `Citation` type above.)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Reference {
    pub id: String,
    #[serde(rename = "subjectId")]
    pub subject_id: String,
    pub ctype: String,
    pub title: String,
    pub authors: Option<String>,
    pub year: Option<String>,
    pub container: Option<String>,
    pub url: Option<String>,
    pub doi: Option<String>,
    pub notes: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

// ---- database stats ---------------------------------------------------

/// Storage + content counts for the Settings → Data screen.
#[derive(Debug, Clone, Serialize)]
pub struct DbStats {
    pub db_bytes: i64,
    pub subjects: i64,
    pub sources: i64,
    pub chunks: i64,
}

/// Progress event emitted on the `ingest:progress` channel.
#[derive(Debug, Clone, Serialize)]
pub struct IngestProgress {
    pub source_id: String,
    pub stage: String, // parsing | chunking | embedding | storing | done | error
    pub detail: String,
    pub pct: u8,
}
