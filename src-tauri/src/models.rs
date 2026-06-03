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

/// A persisted chat message (one rolling thread per subject).
#[derive(Debug, Clone, Serialize)]
pub struct ChatMsg {
    pub role: String,
    pub text: String,
    pub created_at: i64,
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
