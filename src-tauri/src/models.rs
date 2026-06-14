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
    /// Moodle course id this subject is linked to (Settings → Experimental), if any.
    #[serde(default)]
    pub moodle_course_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Topic {
    pub id: String,
    pub subject_id: String,
    pub name: String,
    pub glyph: Option<String>,
    pub position: i64,
    #[serde(default)]
    pub tags: Vec<String>,
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

/// Summary returned by `import_anki` — how many decks/cards landed and how many
/// cards were skipped (empty fronts or duplicates of existing/within-import cards).
#[derive(Debug, Clone, Serialize)]
pub struct AnkiImportResult {
    pub deck_count: usize,
    pub card_count: usize,
    pub skipped: usize,
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

/// One result of the global Ctrl+K search, normalized across record types so
/// the overlay can render and navigate uniformly.
#[derive(Debug, Clone, Serialize)]
pub struct SearchHit {
    /// "chunk" | "source" | "note" | "event" | "material"
    pub kind: String,
    pub id: String,
    /// For chunk hits this is the SOURCE id to open; subject_id scopes navigation.
    pub subject_id: Option<String>,
    pub title: String,
    pub snippet: String,
    /// Cosine similarity for semantic hits; 0 for plain text matches.
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
    /// Images fetched from the web when web mode is on (diagrams/examples).
    #[serde(default)]
    pub images: Vec<WebImage>,
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
    /// Optional illustrative image (web-sourced when "include images" is on).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
    /// A short web-image search query the synthesis model supplies ONLY for a
    /// section whose understanding genuinely needs a diagram/figure. Drives
    /// whether we fetch an image at all (saves needless searches). Transient —
    /// never persisted or sent to the frontend.
    #[serde(default, skip_serializing)]
    pub image_query: Option<String>,
}

fn default_state() -> String {
    "approved".into()
}

#[derive(Debug, Clone, Serialize)]
pub struct CheatsheetData {
    pub subject: String,
    pub topic: String,
    /// Total sources in scope.
    pub sources: i64,
    /// Sources actually synthesized into this sheet (== sources unless a per-source
    /// step failed, or for older stored sheets where it equals `sources`).
    #[serde(default)]
    pub sources_used: i64,
    pub model: String,
    pub sections: Vec<CsSection>,
}

/// Lightweight metadata for one stored cheatsheet version (history list); omits
/// the heavy `sections` JSON.
#[derive(Debug, Clone, Serialize)]
pub struct CheatsheetVersionMeta {
    pub id: String,
    pub created_at: i64,
    pub note: String,
    pub section_count: i64,
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

// ---- exams (timed, locally-graded practice exams) --------------------

/// A practice exam row. `questions`/`answers`/`results` are JSON values mirrored
/// 1:1 in the frontend (`src/lib/api.ts` ExamRec). `answers`/`results` are
/// `Null` until the exam is submitted/graded; `started_ms`/`score` until then.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExamRec {
    pub id: String,
    pub subject_id: String,
    /// Topic ids the exam is scoped to (empty = whole subject).
    #[serde(default)]
    pub topic_ids: Vec<String>,
    pub title: String,
    pub duration_min: i64,
    pub questions: serde_json::Value,
    pub answers: serde_json::Value,
    pub results: serde_json::Value,
    pub status: String,
    pub started_ms: Option<i64>,
    pub score: Option<f64>,
    pub created_at: i64,
    pub updated_at: i64,
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
    /// Direct image URL (only for the "images" category).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub img_src: Option<String>,
    /// Smaller preview image URL, when the engine provides one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thumbnail: Option<String>,
}

/// An image result attached to a chat answer or cheatsheet section.
#[derive(Debug, Clone, Serialize)]
pub struct WebImage {
    /// Full-resolution image URL.
    pub img: String,
    /// Thumbnail/preview URL (falls back to `img`).
    pub thumb: String,
    pub title: String,
    /// The page the image was found on.
    pub source: String,
}

// ---- custom music stations --------------------------------------------

/// A user-added station that streams from a pasted URL (YouTube video/live).
/// Only the URL is persisted; audio is streamed on demand via the mpv sidecar.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomStation {
    pub id: String,
    pub name: String,
    pub url: String,
    pub kind: String,
    pub position: i64,
    pub created_at: i64,
}

/// Availability of the external tools the YouTube-audio engine needs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaTools {
    pub mpv: bool,
    pub ffmpeg: bool,
    pub ytdlp: bool,
    /// Where the auto-downloaded yt-dlp binary lives (for display/diagnostics).
    pub ytdlp_path: String,
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
    #[serde(default)]
    pub tags: Vec<String>,
    /// Topic ids ticked off for this deadline's study checklist.
    #[serde(default)]
    pub checklist: Vec<String>,
    /// Assignment priority: "low" | "med" | "high" (None = normal). Previously
    /// encoded as a colour hex in `color` — now a real field.
    pub priority: Option<String>,
    /// Topic ids this assignment covers (previously squatted in `tags`).
    #[serde(default)]
    pub topic_ids: Vec<String>,
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

// ---- study analytics --------------------------------------------------
//
// One `analytics_summary` call returns the whole dashboard payload so the
// frontend renders from a single round-trip / DB lock. Days are local-date
// strings ("YYYY-MM-DD") computed in SQL so per-day buckets line up with the
// user's calendar rather than UTC midnight.

/// Study minutes on one local day (work pomodoro + passive app segments).
#[derive(Debug, Clone, Serialize)]
pub struct DayMinutes {
    pub day: String, // YYYY-MM-DD (local)
    pub minutes: i64,
}

/// A topic flagged as needing more work, ranked by a weakness score blending
/// low review accuracy, high lapse count, and low FSRS stability.
#[derive(Debug, Clone, Serialize)]
pub struct WeakTopic {
    pub subject_id: String,
    pub topic_id: String,
    pub topic_name: String,
    /// Review attempts attributable to this topic in the window.
    pub reviews: i64,
    pub correct: i64,
    /// 0.0-1.0; 0 when no reviews for this topic.
    pub accuracy: f64,
    /// Total lapses across this topic's scheduled cards.
    pub lapses: i64,
    /// Mean FSRS stability (days) over this topic's cards that have one set.
    pub avg_stability: f64,
    /// Short human reason it's flagged (e.g. "Low accuracy · 4 lapses").
    pub reason: String,
}

/// Reviews answered on one local day, with that day's accuracy.
#[derive(Debug, Clone, Serialize)]
pub struct DayReviews {
    pub day: String, // YYYY-MM-DD (local)
    pub reviews: i64,
    pub correct: i64,
    /// 0.0-1.0; 0 when no reviews that day.
    pub accuracy: f64,
}

/// Cards becoming due on one upcoming local day (next-7-day forecast).
#[derive(Debug, Clone, Serialize)]
pub struct DueDay {
    pub day: String, // YYYY-MM-DD (local)
    pub due: i64,
}

/// Per-subject roll-up: study minutes, reviews, and accuracy.
#[derive(Debug, Clone, Serialize)]
pub struct SubjectStat {
    pub subject_id: String,
    pub minutes: i64,
    pub reviews: i64,
    pub correct: i64,
    /// 0.0-1.0; 0 when no reviews for this subject.
    pub accuracy: f64,
}

/// FSRS memory-state totals across all scheduled cards.
#[derive(Debug, Clone, Serialize)]
pub struct FsrsTotals {
    pub cards: i64,
    /// Mean stability in days over cards that have an FSRS stability set.
    pub avg_stability: f64,
    pub lapses: i64,
}

/// The whole Study Analytics dashboard, returned by `analytics_summary`.
#[derive(Debug, Clone, Serialize)]
pub struct AnalyticsSummary {
    /// Per-day study minutes for the window (oldest → newest, gaps filled with 0).
    pub minutes_per_day: Vec<DayMinutes>,
    /// A full rolling year (366 days) of daily study minutes, oldest → newest,
    /// gaps filled with 0 — drives the GitHub-style contributions heatmap.
    /// Always a year regardless of the `days` window param.
    pub year_minutes: Vec<DayMinutes>,
    /// Per-day reviews + accuracy for the window (oldest → newest, gaps filled).
    pub reviews_per_day: Vec<DayReviews>,
    /// Cards due each of the next 7 days (today → today+6, gaps filled with 0).
    pub due_forecast: Vec<DueDay>,
    /// Per-subject totals over the window (only subjects with any activity).
    pub per_subject: Vec<SubjectStat>,
    /// Topics needing the most work (top ~8 across subjects, weakest first).
    pub weak_topics: Vec<WeakTopic>,
    /// FSRS state totals (all scheduled cards, not windowed).
    pub fsrs: FsrsTotals,
    /// Consecutive days ending today with ≥1 review attempt OR work session.
    pub streak: i64,
    /// Study minutes over the last 7 calendar days (rolling).
    pub minutes_week: i64,
    /// Reviews answered over the last 7 calendar days.
    pub reviews_week: i64,
    /// Accuracy over the last 7 calendar days (0.0-1.0).
    pub accuracy_week: f64,
}
