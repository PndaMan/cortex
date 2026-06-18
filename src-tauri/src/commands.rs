//! Tauri command surface. Mirrored 1:1 in the frontend `src/lib/api.ts`.
//! Commands are synchronous (rusqlite is sync); Tauri runs them off the UI
//! thread. Network/process work (parse, embed) happens without holding the DB lock.

use crate::db::AppState;
use crate::embed;
use crate::error::{Error, Result};
use crate::ingest;
use crate::llm;
use crate::models::*;
use crate::repo;
use crate::vector::f32s_to_blob;
use rusqlite::Connection;
use std::path::Path;
use tauri::{AppHandle, Emitter, Manager, State};

const NO_MODEL: &str =
    "No model configured — add an API key in Settings → API keys (Gemini or OpenRouter), then pick it under Settings → Models.";

/// Default for everything that reads the `model_chat` setting (chat, plus the
/// auto-rename / transcript helpers). A fast NON-reasoning model: the chat path is
/// blocking (total latency == perceived time-to-first-token) and must reliably emit
/// the inline ⟦source · loc⟧ citation markers the UI renders. DeepSeek V4 Flash is the
/// platform-wide default — very cheap ($0.09/$0.18 per Mtok), 1M context, fast and
/// non-reasoning — falling back to any configured key (see llm::from_spec_or_any).
const DEFAULT_CHAT_MODEL: &str = "openrouter:deepseek/deepseek-v4-flash";

/// Default for the OCR / vision helper (`ocr_via_vision`). MUST be vision-capable —
/// the chat default (DeepSeek V4 Flash) is text-only, so OCR rides its own default so
/// image/scanned-PDF transcription keeps working out of the box. `from_spec_or_any`
/// falls back to whichever keyed provider is available, all of whose fallbacks are
/// multimodal (gpt-4o-mini / gemini-2.5-flash / claude-3.5-sonnet).
const DEFAULT_VISION_MODEL: &str = "openrouter:google/gemini-2.5-flash";

fn truncate(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

/// Build a blocking reqwest client with a request timeout. Shared by the
/// network-touching commands (web_search, ping_url) and ingest's web fetch.
pub fn http_client(timeout_secs: u64) -> reqwest::blocking::Client {
    reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(timeout_secs))
        .build()
        .unwrap_or_default()
}

/// True when the user enabled offline mode (Settings → Data & privacy), which
/// blocks all cloud/network calls — only local Ollama (and the offline stub) run.
fn offline_mode(c: &Connection) -> bool {
    matches!(repo::get_setting(c, "offline_mode"), Ok(Some(v)) if v == "true")
}

const OFFLINE_MSG: &str =
    "Offline mode is on — only local Ollama models can run. Pick an Ollama model in Settings → Models, or turn off offline mode in Settings → Data & privacy.";

/// Reject a cloud LLM call when offline mode is on. `spec` is "provider:model";
/// only `ollama:` (local) is permitted offline.
pub(crate) fn guard_offline_llm(c: &Connection, spec: &str) -> Result<()> {
    if offline_mode(c) && !spec.trim().starts_with("ollama:") {
        return Err(Error::Other(OFFLINE_MSG.into()));
    }
    Ok(())
}

/// The embedding provider to actually use. In offline mode, cloud providers
/// (gemini/openai) are downgraded to the local "stub" embedder so ingestion and
/// retrieval keep working with zero network calls (Ollama embeddings stay local).
fn effective_embed_provider(c: &Connection) -> String {
    let p = repo::get_setting(c, "embed_provider")
        .ok()
        .flatten()
        .unwrap_or_else(|| "stub".into());
    if offline_mode(c) && p != "ollama" && p != "stub" {
        "stub".into()
    } else {
        p
    }
}

/// Apply the user's per-task output-token budget (settings key `budget_<task>`,
/// e.g. budget_cheatsheet) to a freshly built model. This is what makes the
/// Settings → Models token-budget sliders actually do something — without it,
/// OpenRouter sends no max_tokens and defaults to a huge cap, 402-ing when the
/// key's credit limit can't cover it.
pub(crate) fn apply_budget(model: &mut Box<dyn llm::Llm>, c: &Connection, task: &str) {
    if let Ok(Some(b)) = repo::get_setting(c, &format!("budget_{task}")) {
        if let Some(n) = b.trim().parse::<u32>().ok().filter(|n| *n > 0) {
            model.set_max_tokens(n);
        }
    }
}

/// Read all configured provider keys from settings.
pub(crate) fn read_keys(c: &Connection) -> Result<llm::Keys> {
    // Trim keys — a pasted key with a trailing newline/space produces an invalid
    // HTTP header value (reqwest drops it → "Missing Authentication header" 401).
    let key = |k: &str| -> Result<Option<String>> {
        Ok(repo::get_setting(c, k)?
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty()))
    };
    Ok(llm::Keys {
        gemini: key("gemini_api_key")?,
        openrouter: key("openrouter_api_key")?,
        openai: key("openai_api_key")?,
        claude: key("claude_api_key")?,
        custom_endpoint: key("custom_endpoint")?,
        // Resolve through the homelab fallback chain so Ollama chat also works
        // over Tailscale/public, not just on the LAN.
        ollama_url: crate::homelab::resolved_setting(c, "ollama_url"),
    })
}

/// The effective Ollama base URL: the homelab-resolved `ollama_url` (local→Tailscale→
/// public, or unified base + /ollama) if set, else localhost on DESKTOP only. On mobile
/// there is no localhost Ollama, so Ollama is reachable only through the homelab — this
/// returns None when no homelab/ollama url is configured.
fn ollama_base(c: &Connection) -> Option<String> {
    if let Some(u) = crate::homelab::resolved_setting(c, "ollama_url").filter(|s| !s.trim().is_empty()) {
        return Some(u.trim_end_matches('/').to_string());
    }
    if cfg!(mobile) {
        None
    } else {
        Some("http://localhost:11434".to_string())
    }
}

/// List the models actually installed on the configured Ollama server (hits Ollama's
/// native `GET /api/tags`). Returns an EMPTY list (never an error) when Ollama is
/// unreachable or has nothing pulled — the model picker uses "no models" to render an
/// empty Ollama option set rather than offering models that aren't installed.
#[tauri::command]
pub fn ollama_models(state: State<AppState>) -> Vec<String> {
    let base = {
        let c = state.db.lock().unwrap();
        ollama_base(&c)
    };
    let Some(base) = base else { return Vec::new() };
    let url = format!("{base}/api/tags");
    let Ok(resp) = http_client(6).get(&url).send() else { return Vec::new() };
    if !resp.status().is_success() {
        return Vec::new();
    }
    let Ok(json) = resp.json::<serde_json::Value>() else { return Vec::new() };
    json["models"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|m| m["name"].as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

/// Result of a provider connection check (Settings → API keys "verify").
#[derive(serde::Serialize)]
pub struct VerifyResult {
    pub ok: bool,
    pub detail: String,
}

fn verify_outcome(req: reqwest::blocking::RequestBuilder) -> VerifyResult {
    match req.send() {
        Ok(r) if r.status().is_success() => VerifyResult { ok: true, detail: "connected".into() },
        Ok(r) => {
            let code = r.status().as_u16();
            let detail = match code {
                401 | 403 => "invalid key".into(),
                404 => "reached, but endpoint not found".into(),
                _ => format!("HTTP {code}"),
            };
            VerifyResult { ok: false, detail }
        }
        Err(e) => VerifyResult {
            ok: false,
            detail: if e.is_connect() || e.is_timeout() { "unreachable".into() } else { e.to_string() },
        },
    }
}

/// Verify that a provider's stored credential actually works — a lightweight, low-cost
/// AUTHENTICATED probe (a models/key lookup, never a billable generation). Powers the
/// "connected / invalid" badge in Settings → API keys. `provider` is one of
/// gemini | openrouter | openai | claude | custom | ollama.
#[tauri::command]
pub fn verify_provider(state: State<AppState>, provider: String) -> VerifyResult {
    let (keys, ollama) = {
        let c = state.db.lock().unwrap();
        let keys = match read_keys(&c) {
            Ok(k) => k,
            Err(e) => return VerifyResult { ok: false, detail: e.to_string() },
        };
        (keys, ollama_base(&c))
    };
    let nonempty = |o: &Option<String>| o.as_deref().map(str::trim).filter(|s| !s.is_empty()).map(str::to_string);
    let client = http_client(10);
    match provider.as_str() {
        "gemini" => match nonempty(&keys.gemini) {
            Some(k) => verify_outcome(client.get(format!(
                "https://generativelanguage.googleapis.com/v1beta/models?key={k}"
            ))),
            None => VerifyResult { ok: false, detail: "not set".into() },
        },
        "openrouter" => match nonempty(&keys.openrouter) {
            Some(k) => verify_outcome(
                client.get("https://openrouter.ai/api/v1/key").header("Authorization", format!("Bearer {k}")),
            ),
            None => VerifyResult { ok: false, detail: "not set".into() },
        },
        "openai" => match nonempty(&keys.openai) {
            Some(k) => verify_outcome(
                client.get("https://api.openai.com/v1/models").header("Authorization", format!("Bearer {k}")),
            ),
            None => VerifyResult { ok: false, detail: "not set".into() },
        },
        "claude" => match nonempty(&keys.claude) {
            Some(k) => verify_outcome(
                client
                    .get("https://api.anthropic.com/v1/models")
                    .header("x-api-key", k)
                    .header("anthropic-version", "2023-06-01"),
            ),
            None => VerifyResult { ok: false, detail: "not set".into() },
        },
        "custom" => match nonempty(&keys.custom_endpoint) {
            Some(base) => {
                let url = format!("{}/models", base.trim_end_matches('/'));
                let mut rb = client.get(url);
                if let Some(k) = nonempty(&keys.openai) {
                    rb = rb.header("Authorization", format!("Bearer {k}"));
                }
                verify_outcome(rb)
            }
            None => VerifyResult { ok: false, detail: "not set".into() },
        },
        "ollama" => match ollama {
            Some(base) => verify_outcome(client.get(format!("{base}/api/tags"))),
            None => VerifyResult { ok: false, detail: "set a Homelab URL".into() },
        },
        other => VerifyResult { ok: false, detail: format!("unknown provider {other}") },
    }
}

/// Build a concise "About the user" + "Remembered facts" preamble from the
/// profile settings and all stored long-term memories. Empty string when there
/// is nothing personalized to add.
fn profile_preamble(c: &Connection) -> Result<String> {
    let get = |k: &str| repo::get_setting(c, k).ok().flatten().filter(|s| !s.trim().is_empty());
    let name = get("profile_name");
    let level = get("profile_level");
    let field = get("profile_field");
    let about = get("profile_about");

    let mut about_bits: Vec<String> = Vec::new();
    if let Some(n) = &name {
        about_bits.push(format!("Name: {n}"));
    }
    if let Some(l) = &level {
        about_bits.push(format!("Level: {l}"));
    }
    if let Some(f) = &field {
        about_bits.push(format!("Field of study: {f}"));
    }
    if let Some(a) = &about {
        about_bits.push(a.clone());
    }

    let memories = repo::list_memory(c)?;

    let mut out = String::new();
    if !about_bits.is_empty() {
        out.push_str("About the user:\n");
        for b in &about_bits {
            out.push_str("- ");
            out.push_str(b);
            out.push('\n');
        }
    }
    if !memories.is_empty() {
        out.push_str("Remembered facts:\n");
        for m in memories.iter().take(50) {
            out.push_str("- ");
            out.push_str(m.content.trim());
            out.push('\n');
        }
    }
    Ok(out)
}

/// Map `profile_style` to a one-line length/verbosity instruction for synthesis.
fn style_instruction(c: &Connection) -> String {
    let style = repo::get_setting(c, "profile_style")
        .ok()
        .flatten()
        .unwrap_or_default();
    match style.as_str() {
        "concise" => " Keep explanations concise and to the point.".to_string(),
        "detailed" => " Be thorough and detailed in explanations.".to_string(),
        _ => String::new(),
    }
}

/// NotebookLM-style custom steering. The user's free-text instructions are woven
/// into the prompt as a SUBORDINATE "focus" block — they steer emphasis, scope,
/// tone, and what to prioritise, but explicitly defer to the system prompt's
/// output format/JSON contract above them. Empty/whitespace input yields an empty
/// string so prompts stay byte-identical to before when no custom prompt is set.
fn custom_focus(custom: Option<&str>) -> String {
    match custom.map(str::trim).filter(|s| !s.is_empty()) {
        Some(c) => format!(
            "\n\nUSER FOCUS — the student gave these custom instructions for what to \
             emphasise, include, or how to frame this material. Follow them as closely as \
             possible WHILE STILL obeying the exact output format and JSON contract described \
             above (never break the schema, never add prose outside it): {c}"
        ),
        None => String::new(),
    }
}

fn slug(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}

// ---- subjects ----------------------------------------------------------

#[tauri::command]
pub fn list_subjects(state: State<AppState>) -> Result<Vec<Subject>> {
    let c = state.db.lock().unwrap();
    repo::list_subjects(&c)
}

#[tauri::command]
pub fn get_subject(state: State<AppState>, id: String) -> Result<Subject> {
    let c = state.db.lock().unwrap();
    repo::get_subject(&c, &id)
}

#[tauri::command]
pub fn create_subject(
    state: State<AppState>,
    name: String,
    code: Option<String>,
    glyph: Option<String>,
    color: Option<String>,
) -> Result<Subject> {
    let c = state.db.lock().unwrap();
    let id = repo::insert_subject(&c, &name, code.as_deref(), glyph.as_deref(), color.as_deref())?;
    repo::get_subject(&c, &id)
}

#[tauri::command]
pub fn update_subject(
    state: State<AppState>,
    id: String,
    name: String,
    code: Option<String>,
    glyph: Option<String>,
    color: Option<String>,
) -> Result<Subject> {
    let c = state.db.lock().unwrap();
    repo::update_subject(
        &c,
        &id,
        &name,
        code.as_deref(),
        glyph.as_deref(),
        color.as_deref(),
    )?;
    repo::get_subject(&c, &id)
}

#[tauri::command]
pub fn delete_subject(state: State<AppState>, id: String) -> Result<()> {
    let c = state.db.lock().unwrap();
    repo::delete_subject(&c, &id)
}

#[tauri::command]
pub fn archive_subject(state: State<AppState>, id: String, archived: bool) -> Result<()> {
    let c = state.db.lock().unwrap();
    repo::set_subject_archived(&c, &id, archived)
}

#[tauri::command]
pub fn list_archived_subjects(state: State<AppState>) -> Result<Vec<Subject>> {
    let c = state.db.lock().unwrap();
    repo::list_archived_subjects(&c)
}

/// Open a URL in the system's default browser. Used for "Open in Moodle" and
/// other external links — a webview `<a target="_blank">` is a no-op in Tauri,
/// so links must round-trip through the OS opener.
#[tauri::command]
pub fn open_external(url: String) -> Result<()> {
    let url = url.trim();
    // Only allow real web links — never hand arbitrary strings to the shell.
    if !(url.starts_with("http://") || url.starts_with("https://")) {
        return Err(Error::Other("refusing to open a non-http(s) URL".into()));
    }
    use std::process::Command;
    #[cfg(target_os = "linux")]
    let cmds: &[&str] = &["xdg-open"];
    #[cfg(target_os = "macos")]
    let cmds: &[&str] = &["open"];
    #[cfg(target_os = "windows")]
    let cmds: &[&str] = &["explorer"];
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    let cmds: &[&str] = &["xdg-open", "open"];
    for c in cmds {
        if Command::new(c).arg(url).spawn().is_ok() {
            return Ok(());
        }
    }
    Err(Error::Other("couldn't launch a browser".into()))
}

// ---- topics ------------------------------------------------------------

#[tauri::command]
pub fn create_topic(
    state: State<AppState>,
    subject_id: String,
    name: String,
    glyph: Option<String>,
    tags: Option<Vec<String>>,
) -> Result<Subject> {
    let c = state.db.lock().unwrap();
    repo::insert_topic(&c, &subject_id, &name, glyph.as_deref(), &tags.unwrap_or_default())?;
    repo::get_subject(&c, &subject_id)
}

#[tauri::command]
pub fn update_topic(
    state: State<AppState>,
    id: String,
    name: String,
    subject_id: String,
    glyph: Option<String>,
    tags: Option<Vec<String>>,
) -> Result<Subject> {
    let c = state.db.lock().unwrap();
    repo::update_topic(&c, &id, &name, glyph.as_deref(), &tags.unwrap_or_default())?;
    repo::get_subject(&c, &subject_id)
}

#[tauri::command]
pub fn reorder_subjects(state: State<AppState>, ids: Vec<String>) -> Result<Vec<Subject>> {
    let c = state.db.lock().unwrap();
    repo::reorder_subjects(&c, &ids)?;
    repo::list_subjects(&c)
}

#[tauri::command]
pub fn reorder_topics(
    state: State<AppState>,
    subject_id: String,
    ids: Vec<String>,
) -> Result<Subject> {
    let c = state.db.lock().unwrap();
    repo::reorder_topics(&c, &subject_id, &ids)?;
    repo::get_subject(&c, &subject_id)
}

#[tauri::command]
pub fn delete_topic(state: State<AppState>, id: String, subject_id: String) -> Result<Subject> {
    let c = state.db.lock().unwrap();
    repo::delete_topic(&c, &id)?;
    repo::get_subject(&c, &subject_id)
}

// ---- sources -----------------------------------------------------------

#[tauri::command]
pub fn list_sources(state: State<AppState>, subject_id: String) -> Result<Vec<Source>> {
    let c = state.db.lock().unwrap();
    repo::list_sources(&c, &subject_id)
}

/// All sources that failed to ingest (across every subject/topic). The frontend
/// auto-retries these on launch so transient failures heal themselves.
#[tauri::command]
pub fn list_failed_sources(state: State<AppState>) -> Result<Vec<Source>> {
    let c = state.db.lock().unwrap();
    repo::list_failed_sources(&c)
}

#[tauri::command]
pub fn get_source(state: State<AppState>, id: String) -> Result<Source> {
    let c = state.db.lock().unwrap();
    repo::get_source(&c, &id)
}

#[tauri::command]
#[allow(non_snake_case)]
pub fn update_source(
    state: State<AppState>,
    id: String,
    name: String,
    topicId: Option<String>,
    tags: Option<Vec<String>>,
) -> Result<Source> {
    let c = state.db.lock().unwrap();
    repo::update_source(
        &c,
        &id,
        &name,
        topicId.as_deref(),
        &tags.unwrap_or_default(),
    )?;
    repo::get_source(&c, &id)
}

#[tauri::command]
pub fn delete_source(state: State<AppState>, id: String) -> Result<()> {
    let c = state.db.lock().unwrap();
    repo::delete_source(&c, &id)
}

/// Re-file a source to a different subject (and optional topic). Repoints the
/// source's chunks too so retrieval scoping stays correct.
#[tauri::command]
pub fn move_source(
    state: State<AppState>,
    id: String,
    subject_id: String,
    topic_id: Option<String>,
) -> Result<Source> {
    let c = state.db.lock().unwrap();
    repo::move_source(&c, &id, &subject_id, topic_id.as_deref())?;
    repo::get_source(&c, &id)
}

/// Stored chunks for a source — lets the UI confirm parse + embedding happened.
#[tauri::command]
pub fn list_chunks(state: State<AppState>, source_id: String) -> Result<Vec<ChunkInfo>> {
    let c = state.db.lock().unwrap();
    repo::list_chunks(&c, &source_id)
}

// ---- settings ----------------------------------------------------------

#[tauri::command]
pub fn get_setting(state: State<AppState>, key: String) -> Result<Option<String>> {
    let c = state.db.lock().unwrap();
    repo::get_setting(&c, &key)
}

#[tauri::command]
pub fn set_setting(state: State<AppState>, key: String, value: String) -> Result<()> {
    let c = state.db.lock().unwrap();
    repo::set_setting(&c, &key, &value)
}

// ---- ingestion ---------------------------------------------------------

fn emit_progress(app: &AppHandle, source_id: &str, stage: &str, detail: &str, pct: u8) {
    let _ = app.emit(
        "ingest:progress",
        IngestProgress {
            source_id: source_id.to_string(),
            stage: stage.to_string(),
            detail: detail.to_string(),
            pct,
        },
    );
}

/// After ingest, give the source a concise content-based name via the chat
/// model. Best-effort: any failure leaves the original name untouched. Keeps a
/// lecture/week/chapter number from the original filename if present.
fn auto_rename_source(state: &State<AppState>, source_id: &str, original_name: &str, text: &str) {
    if text.trim().chars().count() < 80 {
        return; // too little content to name meaningfully
    }
    let (spec, keys) = {
        let c = state.db.lock().unwrap();
        let spec = match repo::get_setting(&c, "model_chat") {
            Ok(Some(s)) => s,
            _ => DEFAULT_CHAT_MODEL.to_string(),
        };
        match read_keys(&c) {
            Ok(k) => (spec, k),
            Err(_) => return,
        }
    };
    let Some(mut model) = llm::from_spec_or_any(&spec, &keys) else {
        return;
    };
    { let c = state.db.lock().unwrap(); apply_budget(&mut model, &c, "chat"); }
    let excerpt: String = text.chars().take(2500).collect();
    let sys = "You name a study source. Reply with ONLY a concise, specific title (Title Case, \
        max 8 words, no quotes, no file extension, no trailing punctuation). If the original \
        filename contains a lecture/week/chapter/unit/topic number (e.g. \"Lecture 14\", \
        \"Week 3\"), KEEP that number in the title.";
    let user = format!("Original filename: {original_name}\n\nContent excerpt:\n{excerpt}\n\nTitle:");
    let Ok(raw) = model.complete(sys, &user) else {
        return;
    };
    let title = raw
        .lines()
        .next()
        .unwrap_or("")
        .trim()
        .trim_matches('"')
        .trim();
    let title: String = title.chars().take(80).collect();
    if title.chars().count() < 3 {
        return;
    }
    let c = state.db.lock().unwrap();
    let _ = repo::rename_source(&c, source_id, &title);
}

/// OCR an image source or a scanned (text-less) PDF using the configured
/// multimodal model (e.g. an OpenRouter/Gemini vision model). PDFs are rendered
/// to page PNGs via poppler, then sent to the model in small batches. Returns
/// the concatenated transcribed Markdown (empty string ⇒ nothing recognised).
fn ocr_via_vision(state: &State<AppState>, kind: &str, path: Option<&str>) -> Result<String> {
    let path = path.ok_or_else(|| Error::Other("no file to OCR".into()))?;
    let (spec, keys) = {
        let c = state.db.lock().unwrap();
        // Prefer an explicitly-chosen vision model if the user set one; otherwise the
        // vision default (NOT model_chat, which now defaults to a text-only model).
        let spec = repo::get_setting(&c, "model_vision")?
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_VISION_MODEL.into());
        (spec, read_keys(&c)?)
    };
    let model = llm::from_spec_or_any(&spec, &keys).ok_or_else(|| Error::Other(NO_MODEL.into()))?;
    let images: Vec<(String, String)> = if kind == "image" {
        let bytes = std::fs::read(path)?;
        vec![(ingest::image_mime(path).to_string(), llm::b64_encode(&bytes))]
    } else {
        // Cap pages so OCR of a huge scan can't run unbounded (cost + time).
        ingest::pdf_page_images(path, 30)?
            .into_iter()
            .map(|b| ("image/png".to_string(), llm::b64_encode(&b)))
            .collect()
    };
    if images.is_empty() {
        return Ok(String::new());
    }
    // One image per request: batching several large page images made OpenRouter
    // truncate the response ("EOF while parsing"). A failed page is skipped (not
    // fatal) so one bad page can't lose the whole document.
    let mut out = String::new();
    let mut last_err: Option<Error> = None;
    let mut ok_pages = 0;
    for img in &images {
        match model.ocr(std::slice::from_ref(img)) {
            Ok(t) => {
                out.push_str(t.trim());
                out.push_str("\n\n");
                ok_pages += 1;
            }
            Err(e) => last_err = Some(e),
        }
    }
    // Only surface an error if EVERY page failed; otherwise return what we got.
    if ok_pages == 0 {
        if let Some(e) = last_err {
            return Err(e);
        }
    }
    Ok(out)
}

/// Offload document parsing to the homelab ingest/parse service (Apache Tika) when a
/// homelab ingest URL is configured. iOS can't run poppler/libreoffice, so scanned
/// PDFs and legacy `.doc/.ppt` have no on-device path — Tika handles PDF/DOCX/PPTX/
/// legacy and OCRs scanned pages (the `-full` image bundles Tesseract). PUTs the raw
/// file to `{ingest_url}/tika` and returns the extracted plain text.
///
/// `Ok(None)` when no homelab ingest URL is set (desktop, or mobile without a homelab)
/// — the caller then keeps whatever text it already had. `Err` only on a real failure.
fn ingest_remote(state: &State<AppState>, path: &str) -> Result<Option<String>> {
    let base = {
        let c = state.db.lock().unwrap();
        if offline_mode(&c) {
            return Ok(None); // offline mode blocks all network offload
        }
        crate::homelab::resolved_setting(&c, "ingest_url")
    };
    let Some(base) = base else {
        return Ok(None);
    };
    let url = format!("{}/tika", base.trim_end_matches('/'));
    let bytes = std::fs::read(path)?;
    // Parsing + OCR of a big scan is slow — give it room (Tika streams nothing back
    // until done). `Accept: text/plain` makes Tika return the extracted text directly.
    let resp = http_client(180)
        .put(&url)
        .header("Accept", "text/plain; charset=UTF-8")
        .body(bytes)
        .send()
        .map_err(|e| Error::Other(format!("ingest request failed: {e}")))?;
    if !resp.status().is_success() {
        return Err(Error::Other(format!("ingest service HTTP {}", resp.status())));
    }
    Ok(Some(resp.text().map_err(|e| Error::Other(e.to_string()))?))
}

/// Re-run ingestion for an EXISTING source in place: re-parse (re-OCR / re-
/// transcribe), re-chunk, re-embed, replacing its old chunks. Used to retry a
/// failed source or refresh one. Reuses the stored original file (or origin
/// URL) so nothing needs re-uploading.
#[tauri::command]
pub async fn reingest_source(app: AppHandle, id: String) -> Result<IngestResult> {
    tauri::async_runtime::spawn_blocking(move || -> Result<IngestResult> {
        let state = app.state::<AppState>();
        let src = {
            let c = state.db.lock().unwrap();
            repo::get_source(&c, &id)?
        };
        // Reconstruct the ingest input from the stored row.
        let mut input = AddSourceInput {
            subject_id: src.subject_id.clone(),
            topic_id: src.topic_id.clone(),
            name: Some(src.name.clone()),
            kind: Some(src.kind.clone()),
            text: None,
            path: None,
            url: None,
            tags: Vec::new(),
        };
        match src.kind.as_str() {
            "web" | "yt" => input.url = src.origin.clone(),
            "txt" | "md" => {
                input.text = src.content.clone();
                input.path = src.origin.clone();
            }
            _ => input.path = src.stored_path.clone().or_else(|| src.origin.clone()),
        }

        emit_progress(&app, &id, "parsing", "re-reading source", 15);
        let (mut text, mut warning) = ingest::parse(&src.kind, &input)?;

        // Same enrichment as add_source: OCR for images/scanned PDFs, Whisper for audio.
        let needs_ocr = src.kind == "image" || (src.kind == "pdf" && text.trim().is_empty());
        if needs_ocr {
            emit_progress(&app, &id, "parsing", "running OCR (vision model)", 35);
            match ocr_via_vision(&state, &src.kind, input.path.as_deref()) {
                Ok(t) if !t.trim().is_empty() => {
                    text = t;
                    warning = None;
                }
                Ok(_) => {}
                Err(e) => warning = Some(format!("OCR failed: {e}")),
            }
        } else if src.kind == "audio" {
            if let Some(p) = input.path.as_deref() {
                emit_progress(&app, &id, "parsing", "transcribing audio (Whisper)", 35);
                let remote = whisper_remote_url(&state);
                let (t, w) = transcribe(Path::new(p), &app.path().app_data_dir().unwrap_or_else(|_| std::env::temp_dir()), true, remote.as_deref(), &whisper_model(&state));
                if !t.trim().is_empty() {
                    text = t;
                    warning = w;
                } else if w.is_some() {
                    warning = w;
                }
            }
        }

        emit_progress(&app, &id, "chunking", "splitting text", 50);
        let chunks = ingest::chunk_text(&text, 900, 150);
        let (provider, gemini_key, ollama_url) = {
            let c = state.db.lock().unwrap();
            (
                effective_embed_provider(&c),
                repo::get_setting(&c, "gemini_api_key")?,
                crate::homelab::resolved_setting(&c, "ollama_url"),
            )
        };
        let embedder = embed::from_settings(&provider, gemini_key.as_deref(), ollama_url.as_deref());
        emit_progress(&app, &id, "embedding", &format!("{} chunks", chunks.len()), 70);
        let vectors = ingest::embed_chunks(embedder.as_ref(), &chunks)
            .or_else(|_| ingest::embed_chunks(&embed::StubEmbedder, &chunks))?;

        emit_progress(&app, &id, "storing", "writing vectors", 88);
        {
            let c = state.db.lock().unwrap();
            repo::clear_chunks(&c, &id)?;
            for (i, (chunk, vec)) in chunks.iter().zip(vectors.iter()).enumerate() {
                repo::insert_chunk(
                    &c,
                    &id,
                    &src.subject_id,
                    src.topic_id.as_deref(),
                    i as i64,
                    chunk,
                    None,
                    vec.len() as i64,
                    &f32s_to_blob(vec),
                )?;
            }
            let chunk_count = repo::count_chunks(&c, &id)?;
            let status = if chunks.is_empty() { "draft" } else { "ready" };
            let meta = if chunks.is_empty() {
                warning.clone().unwrap_or_else(|| "no extractable text".into())
            } else {
                format!("{chunk_count} chunks · {} chars", text.chars().count())
            };
            repo::finalize_source(&c, &id, status, Some(&meta), Some(&text), warning.as_deref())?;
        }
        auto_rename_source(&state, &id, &src.name, &text);
        emit_progress(&app, &id, "done", "re-ingested", 100);

        let c = state.db.lock().unwrap();
        let source = repo::get_source(&c, &id)?;
        let chunk_count = repo::count_chunks(&c, &id)?;
        Ok(IngestResult {
            source,
            chunk_count,
            chars: text.chars().count() as i64,
            warning,
        })
    })
    .await
    .map_err(|e| Error::Other(format!("re-ingest task failed: {e}")))?
}

/// Copy a just-picked file into app storage and return the stable path.
///
/// On mobile (esp. iOS) the file picker hands back a path in a temporary inbox
/// (`…/tmp/<bundle>-Inbox/…`) that the OS deletes shortly after — so by the time
/// the background ingest job reads it, it's gone ("file not found"). The frontend
/// calls this synchronously at pick time, while the temp file still exists, then
/// ingests from the returned persistent path.
#[tauri::command]
pub async fn stage_upload(app: AppHandle, path: String) -> Result<String> {
    tauri::async_runtime::spawn_blocking(move || -> Result<String> {
        // iOS hands back a percent-encoded file:// URL (e.g.
        // `file:///…/Inbox/NEW%20Student%20Guide.pdf`), not a plain path — taking it
        // literally makes `.exists()` false ("file not found"). Decode it to a real
        // filesystem path (scheme stripped, %20→space) before touching the file.
        let real = if path.starts_with("file://") {
            reqwest::Url::parse(&path)
                .ok()
                .and_then(|u| u.to_file_path().ok())
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_else(|| path.clone())
        } else {
            path.clone()
        };
        let src = std::path::Path::new(&real);
        if !src.exists() {
            return Err(Error::NotFound(format!("file not found: {real}")));
        }
        let dir = app
            .path()
            .app_data_dir()
            .map_err(|e| Error::Other(e.to_string()))?
            .join("staged");
        std::fs::create_dir_all(&dir)?;
        let name = src
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("upload.bin");
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let dest = dir.join(format!("{stamp}-{name}"));
        std::fs::copy(src, &dest).map_err(Error::Io)?;
        dest.to_str()
            .map(|s| s.to_string())
            .ok_or_else(|| Error::Other("staged path is not valid UTF-8".into()))
    })
    .await
    .map_err(|e| Error::Other(format!("stage task failed: {e}")))?
}

/// Full pipeline: detect → parse → chunk → embed → store, emitting progress.
#[tauri::command]
pub async fn add_source(
    app: AppHandle,
    input: AddSourceInput,
) -> Result<IngestResult> {
    tauri::async_runtime::spawn_blocking(move || -> Result<IngestResult> {
    let state = app.state::<AppState>();
    let kind = ingest::detect_kind(&input);
    let display_name = input.name.clone().unwrap_or_else(|| {
        input
            .url
            .clone()
            .or_else(|| input.path.clone())
            .unwrap_or_else(|| format!("untitled.{kind}"))
    });

    // 1. create the row + tags (locked)
    let source_id = {
        let c = state.db.lock().unwrap();
        let origin = input.url.clone().or_else(|| input.path.clone());
        let id = repo::insert_source(
            &c,
            &input.subject_id,
            input.topic_id.as_deref(),
            &display_name,
            &kind,
            origin.as_deref(),
        )?;
        repo::attach_tags(&c, &id, &input.tags)?;
        id
    };

    emit_progress(&app, &source_id, "parsing", &format!("reading {kind}"), 15);

    // 1b. persist the ORIGINAL bytes for file-based kinds so the frontend can
    //     render a real preview (txt/md/url keep stored_path NULL — their text
    //     lives in `content`). pptx/docx are excluded here: they have no inline
    //     original renderer, so only the rendered PDF (step 2b) is persisted.
    let sources_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| Error::Other(e.to_string()))?
        .join("sources");
    let copies_original = matches!(kind.as_str(), "pdf" | "image" | "audio");
    if copies_original {
        if let Some(src_path) = input.path.as_deref() {
            let ext = Path::new(src_path)
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("bin")
                .to_lowercase();
            std::fs::create_dir_all(&sources_dir)?;
            let dest = sources_dir.join(format!("{source_id}.{ext}"));
            if let Err(e) = std::fs::copy(src_path, &dest) {
                let c = state.db.lock().unwrap();
                let msg = format!("failed to store original file: {e}");
                let _ = repo::finalize_source(&c, &source_id, "error", None, None, Some(&msg));
                emit_progress(&app, &source_id, "error", &msg, 100);
                return Err(Error::Io(e));
            }
            if let Some(p) = dest.to_str() {
                let c = state.db.lock().unwrap();
                repo::set_stored_path(&c, &source_id, p)?;
            }
        }
    }

    // 2. parse (no lock — may hit network / libreoffice)
    let parse_res = ingest::parse(&kind, &input);
    let (text, warning) = match parse_res {
        Ok(v) => v,
        Err(e) => {
            let c = state.db.lock().unwrap();
            let _ = repo::finalize_source(&c, &source_id, "error", None, None, Some(&e.to_string()));
            emit_progress(&app, &source_id, "error", &e.to_string(), 100);
            return Err(e);
        }
    };

    // 2a. Documents the on-device parser couldn't read (scanned PDFs, legacy .doc/.ppt,
    // or — on mobile — anything needing poppler/libreoffice) → offload to the homelab
    // ingest/parse service (Apache Tika) when one is configured. No-op (Ok(None)) on
    // desktop / when no homelab ingest URL is set, so existing setups are unaffected.
    let (text, warning) = {
        let is_doc = matches!(kind.as_str(), "pdf" | "docx" | "pptx");
        if is_doc && text.trim().is_empty() {
            if let Some(p) = input.path.as_deref() {
                emit_progress(&app, &source_id, "parsing", "parsing on homelab (ingest service)", 35);
                match ingest_remote(&state, p) {
                    Ok(Some(t)) if !t.trim().is_empty() => (t, None),
                    Ok(_) => (text, warning), // no homelab ingest, or it found nothing
                    Err(e) => (text, Some(format!("homelab ingest failed: {e}"))),
                }
            } else {
                (text, warning)
            }
        } else {
            (text, warning)
        }
    };

    // 2b. Enrich kinds `parse` + homelab still can't read:
    //   • images and scanned (text-less) PDFs → OCR via the configured vision model
    //   • audio files → local Whisper transcription
    let (text, warning) = {
        let needs_ocr = kind == "image" || (kind == "pdf" && text.trim().is_empty());
        if needs_ocr {
            emit_progress(&app, &source_id, "parsing", "reading pages with OCR (vision model)", 35);
            match ocr_via_vision(&state, &kind, input.path.as_deref()) {
                Ok(t) if !t.trim().is_empty() => (t, None),
                Ok(_) => (text, warning),
                Err(e) => (text, Some(format!("OCR failed: {e}"))),
            }
        } else if kind == "audio" {
            if let Some(p) = input.path.as_deref() {
                emit_progress(&app, &source_id, "parsing", "transcribing audio (Whisper)", 35);
                let remote = whisper_remote_url(&state);
                let (t, w) = transcribe(Path::new(p), &app.path().app_data_dir().unwrap_or_else(|_| std::env::temp_dir()), true, remote.as_deref(), &whisper_model(&state));
                if t.trim().is_empty() { (text, w.or(warning)) } else { (t, w) }
            } else {
                (text, warning)
            }
        } else {
            (text, warning)
        }
    };

    // 2b. pptx/docx: best-effort PDF render for an inline slide preview. The text
    //     is already extracted natively (no tools needed), so this is purely
    //     cosmetic — if no office→PDF converter (LibreOffice) is installed, we
    //     skip it and keep the original file as the stored path. This means
    //     Windows/macOS users never have to install LibreOffice just to ingest.
    if matches!(kind.as_str(), "pptx" | "docx") {
        if let Some(src_path) = input.path.as_deref() {
            if ingest::office_converter_available() {
                emit_progress(&app, &source_id, "parsing", "rendering slides to PDF", 25);
                let pdf_dest = sources_dir.join(format!("{source_id}.pdf"));
                match ingest::libreoffice_to_pdf(src_path, &pdf_dest) {
                    Ok(()) => {
                        if let Some(p) = pdf_dest.to_str() {
                            let c = state.db.lock().unwrap();
                            repo::set_stored_path(&c, &source_id, p)?;
                        }
                    }
                    // Converter present but render failed — don't fail ingestion;
                    // the source is still fully usable from its extracted text.
                    Err(e) => {
                        eprintln!("slide preview render failed for {source_id}: {e}");
                    }
                }
            }
        }
    }
    let chars = text.chars().count() as i64;

    emit_progress(&app, &source_id, "chunking", "splitting text", 35);
    let chunks = ingest::chunk_text(&text, 900, 150);

    // 3. embed (no lock). Build embedder from settings.
    emit_progress(
        &app,
        &source_id,
        "embedding",
        &format!("{} chunks", chunks.len()),
        60,
    );
    let (provider, gemini_key, ollama_url) = {
        let c = state.db.lock().unwrap();
        (
            effective_embed_provider(&c),
            repo::get_setting(&c, "gemini_api_key")?,
            crate::homelab::resolved_setting(&c, "ollama_url"),
        )
    };
    let embedder = embed::from_settings(&provider, gemini_key.as_deref(), ollama_url.as_deref());
    emit_progress(
        &app,
        &source_id,
        "embedding",
        &format!("{} chunks · {} embedder", chunks.len(), embedder.name()),
        60,
    );
    let vectors = match ingest::embed_chunks(embedder.as_ref(), &chunks) {
        Ok(v) => v,
        Err(e) => {
            // fall back to the stub so ingestion never hard-fails on a bad key
            let stub = embed::StubEmbedder;
            emit_progress(
                &app,
                &source_id,
                "embedding",
                "provider failed → stub fallback",
                60,
            );
            let _ = e;
            ingest::embed_chunks(&stub, &chunks)?
        }
    };
    let dim = embedder.dim() as i64;

    // 4. store chunks (locked)
    emit_progress(&app, &source_id, "storing", "writing vectors", 85);
    {
        let c = state.db.lock().unwrap();
        for (i, (chunk, vec)) in chunks.iter().zip(vectors.iter()).enumerate() {
            repo::insert_chunk(
                &c,
                &source_id,
                &input.subject_id,
                input.topic_id.as_deref(),
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
            warning.clone().unwrap_or_else(|| "no extractable text".into())
        } else {
            format!("{chunk_count} chunks · {chars} chars")
        };
        repo::finalize_source(
            &c,
            &source_id,
            status,
            Some(&meta),
            Some(&text),
            warning.as_deref(),
        )?;
    }

    // Content-based auto-rename (best-effort, before we return so the refreshed
    // source list shows the new name).
    auto_rename_source(&state, &source_id, &display_name, &text);

    emit_progress(&app, &source_id, "done", "ingested", 100);
    let _ = dim;

    let c = state.db.lock().unwrap();
    let source = repo::get_source(&c, &source_id)?;
    let chunk_count = repo::count_chunks(&c, &source_id)?;
    Ok(IngestResult {
        source,
        chunk_count,
        chars,
        warning,
    })
    })
    .await
    .map_err(|e| Error::Other(format!("background task failed: {e}")))?
}

/// Embed a query and return cosine top-k chunks (foundation for scoped chat).
#[tauri::command]
pub fn search_chunks(
    state: State<AppState>,
    query: String,
    subject_id: Option<String>,
    k: Option<usize>,
) -> Result<Vec<ChunkHit>> {
    let (provider, gemini_key, ollama_url) = {
        let c = state.db.lock().unwrap();
        (
            effective_embed_provider(&c),
            repo::get_setting(&c, "gemini_api_key")?,
            crate::homelab::resolved_setting(&c, "ollama_url"),
        )
    };
    let embedder = embed::from_settings(&provider, gemini_key.as_deref(), ollama_url.as_deref());
    let qvec = embedder
        .embed(&[query])
        .map(|mut v| v.pop().unwrap_or_default())?;
    let c = state.db.lock().unwrap();
    repo::search_chunks(&c, subject_id.as_deref(), &qvec, k.unwrap_or(8))
}

/// Global Ctrl+K search: semantic over every subject's chunks (the existing
/// vector index) + plain-text matches over sources, notes, events and
/// materials. Returns a flat, deduplicated hit list the overlay groups by kind.
#[tauri::command]
pub fn global_search(state: State<AppState>, query: String) -> Result<Vec<SearchHit>> {
    let query = query.trim().to_string();
    if query.is_empty() {
        return Ok(Vec::new());
    }
    let (provider, gemini_key, ollama_url) = {
        let c = state.db.lock().unwrap();
        (
            effective_embed_provider(&c),
            repo::get_setting(&c, "gemini_api_key")?,
            crate::homelab::resolved_setting(&c, "ollama_url"),
        )
    };
    let mut hits: Vec<SearchHit> = Vec::new();

    // Text matches first — instant, no network.
    {
        let c = state.db.lock().unwrap();
        hits.extend(repo::text_search(&c, &query, 5)?);
    }

    // Semantic over the vector index, appended AFTER the exact matches so the
    // default Enter target is always a predictable name hit. Skipped entirely
    // on the stub embedder — its hash vectors rank essentially at random, which
    // made every search "navigate" to whichever subject was ingested first.
    let embedder = embed::from_settings(&provider, gemini_key.as_deref(), ollama_url.as_deref());
    if embedder.name() != "stub" {
        if let Ok(mut v) = embedder.embed(&[query.clone()]) {
            let qvec = v.pop().unwrap_or_default();
            let c = state.db.lock().unwrap();
            if let Ok(mut chunks) = repo::search_chunks(&c, None, &qvec, 8) {
                chunks.sort_by(|a, b| b.score.total_cmp(&a.score));
                for h in chunks {
                    // Junk floor: weakly-related chunks aren't navigation targets.
                    if h.score < 0.3 {
                        continue;
                    }
                    // A name match for the same source may already be present.
                    if hits.iter().any(|x| x.kind == "source" && x.id == h.source_id) {
                        continue;
                    }
                    let subject = repo::get_source(&c, &h.source_id).ok().map(|s| s.subject_id);
                    hits.push(SearchHit {
                        kind: "chunk".into(),
                        id: h.source_id.clone(),
                        subject_id: subject,
                        title: h.source_name,
                        snippet: h.text.chars().take(160).collect(),
                        score: h.score,
                    });
                }
            }
        }
    }
    Ok(hits)
}

/// Seed a few demo subjects/topics/sources so the UI has content on first run.
#[tauri::command]
pub fn seed_demo(state: State<AppState>) -> Result<Vec<Subject>> {
    let c = state.db.lock().unwrap();
    let existing = repo::list_subjects(&c)?;
    if !existing.is_empty() {
        return Ok(existing);
    }
    let demo: &[(&str, &str, &[(&str, &[(&str, &str)])])] = &[
        (
            "Algorithms",
            "CS-3490",
            &[
                ("Recursion", &[("lecture-03-recursion.md", "md"), ("tutorial-notes.md", "md")][..]),
                ("Dynamic programming", &[("lecture-04-dp.md", "md")][..]),
            ][..],
        ),
        (
            "Operating Systems",
            "CS-3500",
            &[("Scheduling", &[("scheduling.md", "md")][..])][..],
        ),
        (
            "Statistical Inference",
            "STAT-3010",
            &[("Maximum likelihood", &[("mle-notes.md", "md")][..])][..],
        ),
    ];
    for (name, code, topics) in demo {
        let sid = repo::insert_subject(&c, name, Some(code), None, None)?;
        for (tname, sources) in *topics {
            let tid = repo::insert_topic(&c, &sid, tname, None, &[])?;
            for (sname, kind) in *sources {
                let srcid = repo::insert_source(&c, &sid, Some(&tid), sname, kind, None)?;
                repo::finalize_source(
                    &c,
                    &srcid,
                    "ready",
                    Some("demo · seeded"),
                    Some("Seeded demo source content."),
                    None,
                )?;
                repo::attach_tags(&c, &srcid, &["lecture".into()])?;
            }
        }
    }
    repo::list_subjects(&c)
}

// ---- AI: chat (RAG) ---------------------------------------------------

/// Heuristic: does this question explicitly ask about marks / assessment weighting?
/// This gates the per-subject module-framework + Moodle-grade injection into chat —
/// the framework is NEVER part of normal retrieval, only pulled in on questions like
/// "what is my A2 weighted?" so everyday chat stays clean and cheap.
fn query_wants_marks(q: &str) -> bool {
    let l = q.to_lowercase();
    const KW: &[&str] = &[
        "weight", "mark", "grade", "%", "percentage", "counts for", "count toward",
        "out of", "pass", "fail", "average", "gpa", "assessment", "predicate",
        "promotion", "subminimum", "final mark", "module mark", "needed to",
    ];
    KW.iter().any(|k| l.contains(k))
}

/// Format the subject's linked-Moodle grade items as "- item: grade (pct)" lines.
/// Empty when the subject isn't linked to a Moodle course or has no synced grades.
fn moodle_grades_for_subject(c: &Connection, subject_id: &str) -> String {
    let course_id: Option<String> = c
        .query_row(
            "SELECT moodle_course_id FROM subjects WHERE id=?1",
            rusqlite::params![subject_id],
            |r| r.get::<_, Option<String>>(0),
        )
        .ok()
        .flatten();
    let Some(course_id) = course_id.filter(|s| !s.is_empty()) else {
        return String::new();
    };
    let Ok(mut stmt) = c.prepare(
        "SELECT item_name, grade, percentage FROM moodle_grades WHERE course_id=?1 ORDER BY id",
    ) else {
        return String::new();
    };
    let rows = stmt.query_map(rusqlite::params![course_id], |r| {
        Ok((
            r.get::<_, Option<String>>(0)?.unwrap_or_default(),
            r.get::<_, Option<String>>(1)?.unwrap_or_default(),
            r.get::<_, Option<String>>(2)?.unwrap_or_default(),
        ))
    });
    let mut out = String::new();
    if let Ok(rows) = rows {
        for (name, grade, pct) in rows.flatten() {
            if name.is_empty() {
                continue;
            }
            let g = if grade.is_empty() { "—".into() } else { grade };
            let p = if pct.is_empty() {
                String::new()
            } else {
                format!(" ({pct})")
            };
            out.push_str(&format!("- {name}: {g}{p}\n"));
        }
    }
    out
}

/// Scoped retrieval-augmented chat. Embeds the query, retrieves top-k chunks
/// (optionally narrowed to a single source), and asks the configured LLM to
/// answer from that context with inline ⟦source · loc⟧ citations.
#[tauri::command]
pub async fn chat_answer(
    app: AppHandle,
    subject_id: String,
    level: String,
    source_id: Option<String>,
    source_ids: Option<Vec<String>>,
    query: String,
    web: Option<bool>,
) -> Result<ChatAnswer> {
    tauri::async_runtime::spawn_blocking(move || -> Result<ChatAnswer> {
    let state = app.state::<AppState>();
    let (embed_provider, ollama_url, chat_spec, keys, preamble, searxng) = {
        let c = state.db.lock().unwrap();
        // DEFAULT_CHAT_MODEL is a fast non-reasoning model (see its doc comment for why
        // chat must not fall back to a reasoning model like Step 3.7).
        let chat_spec =
            repo::get_setting(&c, "model_chat")?.unwrap_or_else(|| DEFAULT_CHAT_MODEL.into());
        guard_offline_llm(&c, &chat_spec)?;
        (
            effective_embed_provider(&c),
            crate::homelab::resolved_setting(&c, "ollama_url"),
            chat_spec,
            read_keys(&c)?,
            profile_preamble(&c)?,
            searxng_base(&c)?,
        )
    };
    // Require a real model before doing any work.
    let mut model = llm::from_spec_or_any(&chat_spec, &keys).ok_or_else(|| Error::Other(NO_MODEL.into()))?;
    { let c = state.db.lock().unwrap(); apply_budget(&mut model, &c, "chat"); }

    // Scope: source-level chats are restricted to a single source's chunks; the
    // keyword path applies this in SQL, the vector path filters its results.
    let scoped_source = if level == "source" { source_id.as_deref() } else { None };

    // Multi-source scope: restrict retrieval to exactly these source ids (the
    // chat scope-switcher's tick-list). Applied to BOTH retrieval paths in Rust.
    let source_set: Option<std::collections::HashSet<String>> = source_ids
        .filter(|v| !v.is_empty())
        .map(|v| v.into_iter().collect());

    // The "stub" embedder (and an empty/unset provider) produces meaningless
    // vectors, so cosine search returns irrelevant chunks. In that case rely on
    // keyword search only. With a real embedder, run BOTH and merge by id so
    // retrieval is robust either way.
    let embeddings_reliable = !embed_provider.is_empty() && embed_provider != "stub";

    let mut hits: Vec<ChunkHit> = if embeddings_reliable {
        let embedder =
            embed::from_settings(&embed_provider, keys.gemini.as_deref(), ollama_url.as_deref());
        let qvec = embedder.embed(&[query.clone()])?.pop().unwrap_or_default();
        let c = state.db.lock().unwrap();
        let mut vec_hits = repo::search_chunks(&c, Some(&subject_id), &qvec, 8)?;
        if let Some(sid) = scoped_source {
            vec_hits.retain(|h| h.source_id == sid);
        }
        let kw_hits =
            repo::keyword_search_chunks(&c, Some(&subject_id), scoped_source, &query, 8)?;
        // Append keyword hits not already present (vector results first).
        for h in kw_hits {
            if !vec_hits.iter().any(|x| x.id == h.id) {
                vec_hits.push(h);
            }
        }
        vec_hits.truncate(8);
        vec_hits
    } else {
        let c = state.db.lock().unwrap();
        repo::keyword_search_chunks(&c, Some(&subject_id), scoped_source, &query, 8)?
    };
    // Restrict to the explicitly-selected sources, if any.
    if let Some(set) = &source_set {
        hits.retain(|h| set.contains(&h.source_id));
    }
    hits.truncate(6);

    let context = hits
        .iter()
        .map(|h| {
            let loc = h.loc.as_deref().map(|l| format!(" · {l}")).unwrap_or_default();
            format!("[{}{}]\n{}", h.source_name, loc, h.text)
        })
        .collect::<Vec<_>>()
        .join("\n\n");

    // Web mode: pull live web snippets (for grounding/examples) and — when the
    // question is visual — image results to show alongside the answer. Requires a
    // configured SearXNG; any failure degrades gracefully to source-only.
    let web_on = web.unwrap_or(false);
    let mut images: Vec<WebImage> = Vec::new();
    let mut web_block = String::new();
    if web_on {
        if let Some(base) = &searxng {
            if let Ok(results) = searxng_raw(base, &query, "general") {
                let snippets: Vec<String> = results
                    .iter()
                    .take(5)
                    .filter_map(|r| {
                        let title = r["title"].as_str().unwrap_or("");
                        let content = r["content"].as_str().unwrap_or("");
                        if title.is_empty() && content.is_empty() {
                            return None;
                        }
                        let host = host_from_url(r["url"].as_str().unwrap_or(""));
                        Some(format!("[web · {host}] {title}\n{content}"))
                    })
                    .collect();
                if !snippets.is_empty() {
                    web_block = format!("\n\nWEB RESULTS:\n{}", snippets.join("\n\n"));
                }
            }
            if wants_images(&query) {
                images = searxng_images(base, &query, 4);
            }
        }
    }

    // Module-framework reference: ONLY when the question is explicitly about marks /
    // weighting. Pull the subject's framework text + their synced Moodle grades so a
    // question like "what is my A2 weighted?" can be answered with real numbers.
    let mut framework_block = String::new();
    if query_wants_marks(&query) {
        let c = state.db.lock().unwrap();
        let fw: Option<String> = c
            .query_row(
                "SELECT text FROM subject_frameworks WHERE subject_id=?1",
                rusqlite::params![subject_id],
                |r| r.get::<_, Option<String>>(0),
            )
            .ok()
            .flatten();
        if let Some(fw) = fw.filter(|t| !t.trim().is_empty()) {
            // Cap to keep tokens bounded — frameworks are short outlines anyway.
            framework_block.push_str(&format!(
                "\n\nMODULE FRAMEWORK (official assessment structure & weighting):\n{}",
                truncate(&fw, 12000)
            ));
        }
        let grades = moodle_grades_for_subject(&c, &subject_id);
        if !grades.is_empty() {
            framework_block
                .push_str(&format!("\n\nYOUR RECORDED MARKS (synced from Moodle):\n{grades}"));
        }
    }

    let base_system = "You are Cortex, a study tutor. Be CONCISE — answer in short, focused chunks \
        (usually 2–5 sentences or a few short bullets), never an essay. Lead with the key idea, then \
        actively promote learning by ending the answer with ONE short guiding question. \
        Ground your answer HEAVILY in the provided source context — it is your primary authority: prefer \
        it over everything, and CITE sources inline as ⟦source-name · location⟧ whenever you use them; \
        never contradict or invent beyond what they support. When the sources genuinely DON'T cover the \
        question, you MAY fall back to your own general knowledge to still be helpful — but keep it brief, \
        FLAG it clearly (e.g. open with \"Not in your sources, but in general …\"), and steer back to the \
        material. Use light Markdown (bold key terms, short bullet lists; a `---` divider only when \
        genuinely needed) and keep it scannable. Put ALL code in fenced triple-backtick blocks with a \
        language tag, and never put math or backticks inside a code block. Write maths as LaTeX so it \
        renders: INLINE maths as \\(…\\) (e.g. \\(x^2\\), \\(\\frac{a}{b}\\)) and DISPLAY equations as \
        $$…$$ on their OWN line. Do not use bare single-dollar $…$ for inline maths. \
        On the FINAL line, write 2–3 SPECIFIC next-step prompts the learner could tap, each a real \
        short phrase about THIS material (never placeholders like 'a' or 'b'), formatted exactly as: \
        `SUGGESTIONS: <first prompt> | <second prompt> | <third prompt>` — e.g. \
        `SUGGESTIONS: Walk me through an example | Why does this hold? | Move on to the next topic`. \
        Do not otherwise mention the suggestions line.";
    let mut system = if preamble.is_empty() {
        base_system.to_string()
    } else {
        format!("{preamble}\nUse the above to personalize tone and examples, but {}", {
            // lowercase the first letter so the sentence reads naturally
            let mut chars = base_system.chars();
            match chars.next() {
                Some(f) => f.to_lowercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
    };
    if web_on {
        // Override the strict sources-only rule above: web mode permits using the
        // WEB RESULTS block and light general knowledge, cited as ⟦web · host⟧.
        system.push_str(
            " WEB MODE IS ON: in addition to the sources, you have a WEB RESULTS block (live web \
             snippets) and MAY use it — plus light general knowledge — to add examples, current \
             facts, or visual explanation. Cite web-derived facts as ⟦web · host⟧. When images or \
             diagrams were fetched they are shown directly beneath your answer, so you can say \
             e.g. \"see the diagram below\" rather than describing pixels.",
        );
    }
    if !framework_block.is_empty() {
        // Override the strict sources-only rule for this assessment question.
        system.push_str(
            " ASSESSMENT QUESTION: the user is asking about marks/weighting. Besides the sources you \
             have a MODULE FRAMEWORK block (official assessment structure & weights) and possibly a \
             YOUR RECORDED MARKS block (their actual Moodle grades). You MAY use these and do the \
             arithmetic to answer directly — show the calculation briefly. If a weight or mark you'd \
             need is missing from those blocks, say exactly what's missing instead of guessing, and \
             do not invent numbers.",
        );
    }
    let system = system.as_str();
    let user = if context.is_empty() {
        format!("(No indexed sources are in scope yet.){framework_block}{web_block}\n\nQUESTION: {query}")
    } else {
        format!("SOURCE CONTEXT:\n{context}{framework_block}{web_block}\n\nQUESTION: {query}")
    };

    let text = model.complete(system, &user)?;

    let citations = hits
        .iter()
        .take(4)
        .map(|h| Citation {
            source_name: h.source_name.clone(),
            loc: h.loc.clone(),
            snippet: truncate(&h.text, 160),
        })
        .collect();

    Ok(ChatAnswer {
        text,
        citations,
        model: model.name(),
        images,
    })
    })
    .await
    .map_err(|e| Error::Other(format!("background task failed: {e}")))?
}

// ---- per-subject module framework -------------------------------------

/// Metadata about a subject's stored module framework. `file_path` is the
/// persisted viewable original (or a rendered PDF); `view_kind` is pdf | image |
/// text. The full extracted text is fetched separately (chat / text fallback).
#[derive(serde::Serialize)]
pub struct FrameworkMeta {
    pub filename: String,
    pub chars: i64,
    pub updated_at: i64,
    pub file_path: Option<String>,
    pub view_kind: String,
}

fn read_framework(c: &Connection, subject_id: &str) -> Option<FrameworkMeta> {
    c.query_row(
        "SELECT filename, LENGTH(text), updated_at, file_path, view_kind \
         FROM subject_frameworks WHERE subject_id=?1",
        rusqlite::params![subject_id],
        |r| {
            Ok(FrameworkMeta {
                filename: r.get::<_, Option<String>>(0)?.unwrap_or_default(),
                chars: r.get::<_, Option<i64>>(1)?.unwrap_or(0),
                updated_at: r.get::<_, Option<i64>>(2)?.unwrap_or(0),
                file_path: r.get::<_, Option<String>>(3)?,
                view_kind: r.get::<_, Option<String>>(4)?.unwrap_or_else(|| "text".into()),
            })
        },
    )
    .ok()
}

/// Store a subject's module framework from a local file. Persists the ORIGINAL so
/// it can be viewed in-app as a PDF (docx/pptx are rendered to PDF, like sources),
/// AND extracts its text for chat. The text is only surfaced to chat when the user
/// explicitly asks about marks/weighting.
#[tauri::command]
pub async fn set_subject_framework(
    app: AppHandle,
    subject_id: String,
    path: String,
) -> Result<FrameworkMeta> {
    tauri::async_runtime::spawn_blocking(move || -> Result<FrameworkMeta> {
        let filename = Path::new(&path)
            .file_name()
            .and_then(|f| f.to_str())
            .unwrap_or("framework")
            .to_string();
        let input = AddSourceInput {
            subject_id: subject_id.clone(),
            topic_id: None,
            name: Some(filename.clone()),
            kind: None,
            text: None,
            path: Some(path.clone()),
            url: None,
            tags: Vec::new(),
        };
        let kind = ingest::detect_kind(&input);
        let (text, _warn) = ingest::parse(&kind, &input)?;
        let text = text.trim().to_string();

        // Persist a viewable original, mirroring the source pipeline: pdf/image
        // are copied verbatim; docx/pptx are rendered to PDF via LibreOffice;
        // txt/md have no document view (fall back to the extracted text).
        let dir = app
            .path()
            .app_data_dir()
            .map_err(|e| Error::Other(e.to_string()))?
            .join("frameworks");
        std::fs::create_dir_all(&dir)?;
        let (file_path, view_kind): (Option<String>, &str) = match kind.as_str() {
            "pdf" | "image" => {
                let ext = Path::new(&path)
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("bin")
                    .to_lowercase();
                let dest = dir.join(format!("{subject_id}.{ext}"));
                std::fs::copy(&path, &dest)?;
                let vk = if kind == "pdf" { "pdf" } else { "image" };
                (dest.to_str().map(|s| s.to_string()), vk)
            }
            "docx" | "pptx" => {
                let dest = dir.join(format!("{subject_id}.pdf"));
                ingest::libreoffice_to_pdf(&path, &dest)?;
                (dest.to_str().map(|s| s.to_string()), "pdf")
            }
            _ => (None, "text"),
        };

        // For text-only kinds the extracted text IS the document, so it must be
        // non-empty; for pdf/image the file is the document (OCR-less text is ok).
        if view_kind == "text" && text.is_empty() {
            return Err(Error::Other(
                "Couldn't read that file — use a PDF, Word, PowerPoint, image or text file.".into(),
            ));
        }
        let now = crate::db::now_ms();
        let chars = text.chars().count() as i64;
        let state = app.state::<AppState>();
        let c = state.db.lock().unwrap();
        c.execute(
            "INSERT OR REPLACE INTO subject_frameworks \
             (subject_id, filename, text, file_path, view_kind, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![subject_id, filename, text, file_path, view_kind, now],
        )?;
        Ok(FrameworkMeta { filename, chars, updated_at: now, file_path, view_kind: view_kind.into() })
    })
    .await
    .map_err(|e| Error::Other(format!("set framework task failed: {e}")))?
}

/// Framework metadata for a subject (None if none uploaded).
#[tauri::command]
pub fn get_subject_framework(
    state: State<AppState>,
    subject_id: String,
) -> Result<Option<FrameworkMeta>> {
    let c = state.db.lock().unwrap();
    Ok(read_framework(&c, &subject_id))
}

/// The full extracted text of a subject's framework, for the in-app reader
/// (None if none uploaded). Separate from `get_subject_framework` so the Overview
/// load stays light — the text is only fetched when the user opens it.
#[tauri::command]
pub fn get_subject_framework_text(
    state: State<AppState>,
    subject_id: String,
) -> Result<Option<String>> {
    let c = state.db.lock().unwrap();
    Ok(c.query_row(
        "SELECT text FROM subject_frameworks WHERE subject_id=?1",
        rusqlite::params![subject_id],
        |r| r.get::<_, Option<String>>(0),
    )
    .ok()
    .flatten())
}

#[tauri::command]
pub fn clear_subject_framework(state: State<AppState>, subject_id: String) -> Result<()> {
    let c = state.db.lock().unwrap();
    // Remove the persisted original file too (best-effort; a leftover is harmless).
    if let Some(meta) = read_framework(&c, &subject_id) {
        if let Some(p) = meta.file_path {
            let _ = std::fs::remove_file(p);
        }
    }
    c.execute(
        "DELETE FROM subject_frameworks WHERE subject_id=?1",
        rusqlite::params![subject_id],
    )?;
    Ok(())
}

/// Set a subject's calendar match keywords (comma-separated) and immediately
/// re-file unassigned calendar events. Returns how many events were newly filed.
#[tauri::command]
pub fn set_subject_aliases(state: State<AppState>, subject_id: String, aliases: String) -> Result<usize> {
    let c = state.db.lock().unwrap();
    c.execute(
        "UPDATE subjects SET calendar_aliases=?2, updated_at=?3 WHERE id=?1",
        rusqlite::params![subject_id, aliases.trim(), crate::db::now_ms()],
    )?;
    repo::retag_calendar_events(&c)
}

/// Re-match all unfiled calendar events to subjects (name/code/alias). Returns
/// the number newly filed.
#[tauri::command]
pub fn retag_calendar_events(state: State<AppState>) -> Result<usize> {
    let c = state.db.lock().unwrap();
    repo::retag_calendar_events(&c)
}

// ---- chat history (one rolling thread per subject) --------------------

#[tauri::command]
pub fn add_chat_message(
    state: State<AppState>,
    subject_id: String,
    role: String,
    text: String,
) -> Result<()> {
    let c = state.db.lock().unwrap();
    repo::add_chat_message(&c, &subject_id, &role, &text)
}

#[tauri::command]
pub fn list_chat_messages(state: State<AppState>, subject_id: String) -> Result<Vec<ChatMsg>> {
    let c = state.db.lock().unwrap();
    repo::list_chat_messages(&c, &subject_id)
}

#[tauri::command]
pub fn clear_chat(state: State<AppState>, subject_id: String) -> Result<()> {
    let c = state.db.lock().unwrap();
    repo::clear_chat(&c, &subject_id)
}

#[tauri::command]
pub fn new_chat(state: State<AppState>, subject_id: String) -> Result<()> {
    let c = state.db.lock().unwrap();
    repo::new_thread(&c, &subject_id)?;
    Ok(())
}

#[tauri::command]
pub fn list_chat_threads(state: State<AppState>, subject_id: String) -> Result<Vec<ThreadInfo>> {
    let c = state.db.lock().unwrap();
    repo::list_threads(&c, &subject_id)
}

#[tauri::command]
pub fn open_chat_thread(
    state: State<AppState>,
    subject_id: String,
    thread_id: String,
) -> Result<()> {
    let c = state.db.lock().unwrap();
    repo::set_active_thread(&c, &subject_id, &thread_id)
}

// ---- AI: cheatsheet synthesis ----------------------------------------

fn parse_cheatsheet(raw: &str) -> Vec<CsSection> {
    match llm::extract_json(raw) {
        Ok(v) => {
            let arr = v.get("sections").and_then(|s| s.as_array()).cloned();
            if let Some(arr) = arr {
                return arr
                    .iter()
                    .filter_map(|s| {
                        let title = s.get("title")?.as_str()?.to_string();
                        let items = s
                            .get("items")
                            .and_then(|i| i.as_array())
                            .map(|items| {
                                items
                                    .iter()
                                    .filter_map(|it| {
                                        Some(CsItem {
                                            t: it.get("t")?.as_str()?.to_string(),
                                            d: it.get("d").and_then(|d| d.as_str()).unwrap_or("").to_string(),
                                        })
                                    })
                                    .collect()
                            })
                            .unwrap_or_default();
                        let image_query = s
                            .get("image_query")
                            .and_then(|q| q.as_str())
                            .map(|q| q.trim().to_string())
                            .filter(|q| !q.is_empty());
                        Some(CsSection {
                            id: slug(&title),
                            title,
                            state: "approved".into(),
                            items,
                            image: None,
                            image_query,
                        })
                    })
                    .collect();
            }
            Vec::new()
        }
        Err(_) => Vec::new(),
    }
}

/// Render a structured infographic spec into a designed poster image via an
/// image model (nano-banana, openrouter:google/gemini-2.5-flash-image). The text
/// model already produced accurate headings/points; the image model only lays
/// them out, so spelling/figures stay correct. Returns a data:image/...;base64 URL.
// Currently unused: infographics render as a crisp HTML poster (InfographicView)
// instead of an image. Retained (not deleted) so the poster-image path can be
// re-enabled later without rebuilding it. Legacy materials that already stored an
// `image` still display via the frontend's `image` branch, which doesn't call this.
#[allow(dead_code)]
fn render_infographic_image(spec: &serde_json::Value, keys: &llm::Keys) -> Option<String> {
    let mut text = String::new();
    if let Some(t) = spec["title"].as_str() {
        text.push_str(&format!("TITLE: {t}\n"));
    }
    if let Some(s) = spec["subtitle"].as_str() {
        text.push_str(&format!("SUBTITLE: {s}\n"));
    }
    for s in spec["sections"].as_array().into_iter().flatten() {
        if let Some(h) = s["heading"].as_str() {
            text.push_str(&format!("\nSECTION — {h}\n"));
        }
        if let Some(v) = s["stat"]["value"].as_str() {
            let lbl = s["stat"]["label"].as_str().unwrap_or("");
            text.push_str(&format!("  HEADLINE FIGURE: {v} {lbl}\n"));
        }
        for p in s["points"].as_array().into_iter().flatten() {
            if let Some(p) = p.as_str() {
                text.push_str(&format!("  • {p}\n"));
            }
        }
    }
    if text.trim().is_empty() {
        return None;
    }
    let prompt = format!(
        "Create a professional, information-dense EDUCATIONAL INFOGRAPHIC POSTER, portrait \
         orientation. Clean editorial Swiss-minimalist style: light off-white (#f5f3ee) \
         background, dark legible sans-serif text, ONE restrained accent colour, strict grid \
         with thin horizontal rules between titled sections. Use simple flat technical \
         line-icons, small clear diagrams (bar charts, demand/supply curves, flow arrows) and a \
         comparison table where it helps. Designed for utility, not fun — NO cartoons, NO \
         clutter, spell every word correctly. Lay out and visualise EXACTLY the following \
         content, preserving all headings, bullet points and numbers verbatim:\n\n{text}"
    );
    // Image generation currently goes through OpenRouter's Gemini image model.
    let model = llm::from_spec_or_any("openrouter:google/gemini-2.5-flash-image", keys)?;
    model.gen_image(&prompt).ok()
}

/// Render a self-contained HTML document (built by the frontend with print
/// styles inlined) to a PDF file at `dest`. Used by "Save as PDF" / "Export all"
/// for cheatsheets and notes — replaces the unreliable `window.print()` path
/// (WebKitGTK frequently no-ops it). Runs off-thread since it shells out.
#[tauri::command]
pub async fn export_pdf(html: String, dest: String) -> Result<()> {
    tauri::async_runtime::spawn_blocking(move || -> Result<()> {
        crate::ingest::html_to_pdf(&html, std::path::Path::new(&dest))
    })
    .await
    .map_err(|e| Error::Other(format!("export task failed: {e}")))?
}

/// Reclaim disk space: checkpoint the WAL and VACUUM the database. Wired to the
/// Settings "Optimize storage" action (was a cosmetic no-op before).
#[tauri::command]
pub fn optimize_db(state: State<AppState>) -> Result<()> {
    let c = state.db.lock().unwrap();
    c.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")?;
    c.execute("VACUUM", [])?;
    Ok(())
}

/// Export the entire database to a single portable SQLite file at `dest` (the
/// locked-in "SQLite dump" export). Checkpoints the WAL first so the copy is
/// complete and self-contained.
#[tauri::command]
pub async fn export_database(app: AppHandle, dest: String) -> Result<()> {
    tauri::async_runtime::spawn_blocking(move || -> Result<()> {
        let state = app.state::<AppState>();
        let db_path = app
            .path()
            .app_data_dir()
            .map_err(|e| Error::Other(e.to_string()))?
            .join("cortex.db");
        {
            let c = state.db.lock().unwrap();
            let _ = c.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);");
        }
        std::fs::copy(&db_path, &dest).map_err(Error::Io)?;
        Ok(())
    })
    .await
    .map_err(|e| Error::Other(format!("export task failed: {e}")))?
}

/// Export a flashcard material to an Anki `.apkg` deck at `dest`. Only flashcard
/// materials are exportable (quiz/audio/etc. have no front/back shape).
#[tauri::command]
pub async fn export_anki(app: AppHandle, material_id: String, dest: String) -> Result<usize> {
    tauri::async_runtime::spawn_blocking(move || -> Result<usize> {
        let state = app.state::<AppState>();
        let mat = {
            let c = state.db.lock().unwrap();
            repo::get_material(&c, &material_id)?
        };
        if mat.kind != "flashcards" {
            return Err(Error::Other(format!(
                "Anki export only supports flashcard decks (this is a {} material).",
                mat.kind
            )));
        }
        // Flashcard payload: a JSON array of {"q":front,"a":back}.
        let cards: Vec<(String, String)> = mat
            .payload
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|c| {
                let q = c["q"].as_str()?.trim();
                let a = c["a"].as_str().unwrap_or("").trim();
                if q.is_empty() {
                    return None;
                }
                Some((q.to_string(), a.to_string()))
            })
            .collect();
        if cards.is_empty() {
            return Err(Error::Other("this deck has no cards to export".into()));
        }
        let deck_name = if mat.title.trim().is_empty() { "Cortex deck" } else { mat.title.trim() };
        crate::anki::export_apkg(std::path::Path::new(&dest), deck_name, &cards)?;
        Ok(cards.len())
    })
    .await
    .map_err(|e| Error::Other(format!("anki export task failed: {e}")))?
}

/// Import an Anki `.apkg` deck file into this subject as flashcard materials —
/// one material per Anki deck. Cards are HTML-stripped to plain text, deduped
/// within the import AND against existing flashcard decks in this subject (by
/// normalized front), then stored in the exact `[{ "q": front, "a": back }]`
/// payload shape the Flashcards view renders. Returns a small summary.
#[tauri::command]
pub async fn import_anki(
    app: AppHandle,
    subject_id: String,
    topic_id: Option<String>,
    path: String,
) -> Result<AnkiImportResult> {
    tauri::async_runtime::spawn_blocking(move || -> Result<AnkiImportResult> {
        let state = app.state::<AppState>();

        // Parse the archive off the DB lock (zip + sqlite read can be slow).
        let decks = crate::anki::import_apkg(std::path::Path::new(&path))?;
        if decks.is_empty() {
            return Err(Error::Other(
                "no flashcards found in this .apkg (no decks with usable cards)".into(),
            ));
        }

        // Gather every existing flashcard front already in this subject so we don't
        // re-import duplicates the user already has. Built once, under one lock.
        let mut existing_fronts: std::collections::HashSet<String> = std::collections::HashSet::new();
        {
            let c = state.db.lock().unwrap();
            for m in repo::list_materials(&c, &subject_id)? {
                if m.kind != "flashcards" {
                    continue;
                }
                for card in m.payload.as_array().into_iter().flatten() {
                    if let Some(q) = card["q"].as_str() {
                        let q = q.trim();
                        if !q.is_empty() {
                            existing_fronts.insert(crate::anki::dedupe_key(q));
                        }
                    }
                }
            }
        }

        // The stem of the imported file — a fallback deck title when Anki's deck
        // name is the generic "Default" or empty.
        let stem = std::path::Path::new(&path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Imported deck")
            .to_string();

        let mut deck_count = 0usize;
        let mut card_count = 0usize;
        let mut skipped = 0usize;

        for deck in decks {
            // Drop cards whose front already exists in this subject (cross-deck
            // dedupe). Within-deck dedupe already happened in import_apkg.
            let mut cards: Vec<serde_json::Value> = Vec::new();
            for card in &deck.cards {
                if !existing_fronts.insert(crate::anki::dedupe_key(&card.front)) {
                    skipped += 1;
                    continue;
                }
                cards.push(serde_json::json!({ "q": card.front, "a": card.back }));
            }
            if cards.is_empty() {
                continue; // every card was a duplicate — no material to create
            }

            // A "Default"/blank Anki deck name carries no meaning; use the filename.
            let title = if deck.name.trim().is_empty() || deck.name.trim() == "Default" {
                stem.clone()
            } else {
                deck.name.trim().to_string()
            };
            let n = cards.len();
            let meta = format!("{n} cards · imported from Anki");
            let payload = serde_json::Value::Array(cards);

            {
                let c = state.db.lock().unwrap();
                repo::save_material(
                    &c,
                    &subject_id,
                    topic_id.as_deref(),
                    "flashcards",
                    &title,
                    &meta,
                    &payload,
                )?;
            }
            deck_count += 1;
            card_count += n;
        }

        if card_count == 0 {
            return Err(Error::Other(
                "every card in this .apkg already exists in this subject".into(),
            ));
        }

        Ok(AnkiImportResult {
            deck_count,
            card_count,
            skipped,
        })
    })
    .await
    .map_err(|e| Error::Other(format!("anki import task failed: {e}")))?
}

/// MAP step: turn ONE source into an exhaustive, compact study digest so a bucket
/// with many sources can be reduced into a single cheatsheet without overflowing
/// the model window — and so every source is guaranteed to be represented.
const CHEATSHEET_MAP_SYSTEM: &str = "You are an exam-focused study-notes extractor. From the SINGLE source \
    provided, extract an EXHAUSTIVE, well-structured Markdown digest capturing EVERY exam-relevant \
    element it contains: every term, definition, concept, theory, model, framework, classification, \
    process, cause/effect, formula, law, rule, named example, case study, date, person, place, and \
    distinction. Do NOT summarise away detail or cherry-pick — when in doubt, INCLUDE it. Preserve \
    the source's own terminology and figures exactly. Output Markdown only (use headings, bullet \
    lists, and tables) with NO preamble, commentary, or code fences.";

/// Reduce one bucket (a topic, or the ungrouped "General" set) into cheatsheet
/// sections. `sources` is each source's (title, full_text), already read from the
/// DB so no lock is held during the (slow) model calls. With one source we
/// synthesize directly from its full text (max fidelity); with several we MAP each
/// to a digest first, then REDUCE the digests — guaranteeing all-source coverage.
/// Returns (sections, sources_used).
fn synthesize_bucket(
    model: &dyn llm::Llm,
    system: &str,
    scope_label: &str,
    sources: &[(String, String)],
) -> Result<(Vec<CsSection>, i64)> {
    let mut used = 0i64;
    let material = if sources.len() <= 1 {
        match sources.first() {
            Some((title, text)) => {
                used = 1;
                format!("SOURCE: {title}\n\n{text}")
            }
            None => String::new(),
        }
    } else {
        let mut digests: Vec<String> = Vec::new();
        for (title, text) in sources {
            let prompt = format!("SOURCE: {title}\n\n{text}\n\nProduce the exhaustive study digest now.");
            match model.complete(CHEATSHEET_MAP_SYSTEM, &prompt) {
                Ok(d) if !d.trim().is_empty() => {
                    digests.push(format!("### SOURCE: {title}\n\n{}", d.trim()));
                    used += 1;
                }
                // A single failed source shouldn't sink the whole sheet; coverage
                // (sources_used) will reflect the gap so the UI can flag it.
                _ => {}
            }
        }
        digests.join("\n\n---\n\n")
    };
    if material.trim().is_empty() {
        return Ok((Vec::new(), 0));
    }
    let user =
        format!("Subject: {scope_label}\n\nSOURCE MATERIAL:\n{material}\n\nProduce the cheatsheet JSON now.");
    let raw = model.complete(system, &user)?;
    let mut sections = parse_cheatsheet(&raw);
    if sections.is_empty() {
        // A real model returned something unparseable — surface it rather than fail silently.
        sections = vec![CsSection {
            id: "notes".into(),
            title: "Notes".into(),
            state: "draft-pending".into(),
            items: vec![CsItem {
                t: "Model returned unstructured output".into(),
                d: truncate(&raw, 600),
            }],
            image: None,
            image_query: None,
        }];
    }
    Ok((sections, used))
}

/// Synthesize a sectioned cheatsheet for ONE bucket: a specific topic
/// (`topic_id = Some`) or the subject's ungrouped "General" sources (`topic_id =
/// None`). The whole-subject sheet is composed from these — see
/// `get_subject_cheatsheet` / `generate_subject_cheatsheet`.
#[tauri::command]
pub async fn generate_cheatsheet(
    app: AppHandle,
    subject_id: String,
    topic_id: Option<String>,
    with_images: Option<bool>,
) -> Result<CheatsheetData> {
    tauri::async_runtime::spawn_blocking(move || -> Result<CheatsheetData> {
    let state = app.state::<AppState>();
    let (bucket, subject_name, topic_name, spec, keys, style, searxng) = {
        let c = state.db.lock().unwrap();
        let subj = repo::get_subject(&c, &subject_id)?;
        let tname = match topic_id.as_deref() {
            Some(tid) => subj
                .topics
                .iter()
                .find(|t| t.id == tid)
                .map(|t| t.name.clone())
                .unwrap_or_else(|| "Topic".into()),
            None => "General".into(),
        };
        // Generation drives off the explicit source list for this ONE bucket (a
        // topic, or the ungrouped "General" set) so a topic's sheet always covers
        // ALL of that topic's sources. The whole-subject sheet is composed from
        // these per-topic sheets elsewhere — never re-synthesized — so the two can
        // never drift.
        let ids = repo::bucket_source_ids(&c, &subject_id, topic_id.as_deref())?;
        let mut bucket: Vec<(String, String)> = Vec::new();
        for sid in &ids {
            let (text, _) =
                repo::context_text(&c, &subject_id, None, Some(std::slice::from_ref(sid)), 200_000)?;
            if text.trim().is_empty() {
                continue;
            }
            let title = repo::get_source(&c, sid)
                .map(|s| s.name)
                .unwrap_or_else(|_| "source".into());
            bucket.push((title, text));
        }
        let spec =
            repo::get_setting(&c, "model_cheatsheet")?.unwrap_or_else(|| "openrouter:deepseek/deepseek-v4-flash".into());
        guard_offline_llm(&c, &spec)?;
        (bucket, subj.name, tname, spec, read_keys(&c)?, style_instruction(&c), searxng_base(&c)?)
    };
    let mut model = llm::from_spec_or_any(&spec, &keys).ok_or_else(|| Error::Other(NO_MODEL.into()))?;
    { let c = state.db.lock().unwrap(); apply_budget(&mut model, &c, "cheatsheet"); }
    if bucket.is_empty() {
        return Err(Error::Other(
            "No source text to synthesize from — add and ingest a source first.".into(),
        ));
    }
    let sources = bucket.len() as i64;

    let system = format!("You are a world-class, exam-focused study-notes synthesizer. Build a \
        COMPLETE, accurate, exam-ready cheatsheet from the source material.\n\
        \n\
        EXHAUSTIVENESS IS THE #1 PRIORITY. Walk through the SOURCE MATERIAL from start to finish \
        and cover EVERY exam-relevant element it contains: every term, definition, concept, theory, \
        model, framework, classification, process, cause/effect, formula, law, rule, named \
        example, case study, date, person, place, and distinction. If a subtopic, heading, or idea \
        appears in the readings, it MUST appear in the cheatsheet — do NOT summarise, sample, or \
        cherry-pick the 'main' ones and drop the rest. When in doubt, INCLUDE it. A student should \
        be able to revise for the exam from this cheatsheet ALONE without reopening the sources. \
        Prefer many thorough items over a few; never collapse several distinct concepts into one \
        item.\n\
        \n\
        DEPTH: flesh out every item with a real, self-contained explanation drawn from the SOURCE \
        MATERIAL — what it is, why it matters, how it relates to neighbouring ideas, and a concrete \
        example or distinguishing detail. Never terse one-liners.\n\
        \n\
        OUTPUT CONTRACT: Respond with ONLY raw JSON — no markdown code fences, no prose before or \
        after. Use this EXACT shape (do not add or rename keys):\n\
        {{\"sections\":[{{\"title\":string,\"image_query\":string|null,\"items\":[{{\"t\":\"term\",\"d\":\"explanation\"}}]}}]}}\n\
        \"image_query\" is OPTIONAL and almost always null. Set it to a 3-6 word web image-search \
        query ONLY when understanding the section genuinely depends on seeing a specific diagram, \
        labelled model, structure, cycle, map, or chart that words alone can't convey (e.g. \
        \"Burgess concentric zone model\", \"animal cell organelles diagram\", \"nitrogen cycle \
        diagram\"). Leave it null for definitions, lists, prose, formulas, mnemonics, and anything \
        a reader doesn't need a picture to grasp — do NOT request decorative images.\n\
        Every item has a short heading \"t\" (the term/concept/rule name) and a RICH MARKDOWN body \
        \"d\". JSON string escaping must stay valid: write newlines inside \"d\" as the two \
        characters backslash-n, escape any double quotes, and never emit a literal control \
        character.\n\
        \n\
        The \"d\" body is RICH MARKDOWN and SHOULD use, wherever it genuinely aids understanding:\n\
        - GitHub-style callouts — a line starting with `> [!NOTE]`, `> [!TIP]`, `> [!WARNING]`, \
        `> [!IMPORTANT]`, or `> [!EXAMPLE]` followed by the callout text on the same or following \
        `> ` lines. Use them for key insights, gotchas, when-to-use guidance, and exam tips.\n\
        - Markdown TABLES (e.g. `| Concept | Use |` then `|---|---|` then data rows) to compare \
        related concepts or list properties side by side.\n\
        - **Bold** key terms, inline `code`, ordered lists for step-by-step worked \
        examples, and short bullet lists.\n\
        - LaTeX MATHS so equations actually render: write INLINE maths as \\(…\\) (e.g. \\(E=mc^2\\), \
        \\(\\frac{{dy}}{{dx}}\\)) and DISPLAY equations as $$…$$ on their OWN line. Use this for EVERY \
        formula/equation instead of plain text; do not use bare single-dollar $…$.\n\
        - A simple BAR CHART for quantitative comparisons: a fenced block opened with \
        three backticks then the word barchart, then one `Label: number` per line, then closing \
        backticks. Use ONLY for real numeric data from the sources (proportions, magnitudes, \
        counts) — never invent numbers.\n\
        \n\
        Produce these sections, in THIS exact order, each fleshed out and comprehensive:\n\
        1. \"Overview\" — a high-level orientation: what this topic is, why it matters, how the \
        pieces fit together, and a quick map of everything the cheatsheet covers below.\n\
        2. \"Key Concepts\" — every main idea, theory, model, process and classification explained \
        in depth; use comparison TABLES where concepts contrast, and a `\x60\x60\x60barchart` block \
        (see below) when comparing quantities.\n\
        3. \"Definitions\" — define EVERY key term the sources introduce, not just a handful. \
        STRUCTURE each \"d\" so it never reads as a wall of text: start with a single concise \
        definition sentence, then a short bulleted list (`- `) of the term's key attributes, \
        examples, or use-cases, and bold (`**term**`) the defined term and any sub-terms. Where \
        two terms are easily confused, contrast them in a small TABLE. Keep prose tight and prefer \
        bullets over long paragraphs. Order terms logically (foundational first), not randomly.\n\
        4. \"Formulas & Rules\" — every formula, law, and rule with inline `code`, plus when-to-use \
        guidance in callouts. (Omit this section ONLY if the sources contain none.)\n\
        5. \"Worked Examples\" — concrete examples and case studies from the source, solved with \
        NUMBERED steps.\n\
        6. \"Common Pitfalls\" — mistakes and misconceptions, each as a `> [!WARNING]` callout.\n\
        7. \"Mnemonics & Quick Recall\" — memory aids and a tight recap for last-minute review.\n\
        \n\
        Use EXACTLY these seven sections and NO others — do NOT invent any extra top-level \
        sections. \"Key Concepts\" is where the BULK of the content lives: when the material has \
        major themes (eras, sub-fields, case domains, e.g. \"Political Geography\", \"The \
        Mechanical Era\"), put EACH as its OWN ITEM under \"Key Concepts\" — the item's \"t\" is the \
        theme name and its \"d\" uses Markdown `## subheadings` (and `###` below) for internal \
        structure — NEVER as a new section. Keeping every topic to the same seven sections is \
        REQUIRED so multiple topics merge cleanly. Omit ONLY \"Formulas & Rules\" or \"Worked \
        Examples\" when the sources genuinely contain none; never drop or rename the others.\n\
        {style}");
    let system = system.as_str();
    let scope = if topic_id.is_some() {
        format!("{subject_name} › {topic_name}")
    } else {
        format!("{subject_name} › General (ungrouped sources)")
    };
    let (mut sections, sources_used) = synthesize_bucket(model.as_ref(), system, &scope, &bucket)?;

    // Illustrate only the sections the synthesis model flagged as genuinely
    // needing a diagram (image_query set) — so we don't burn a web search on
    // every section. Capped so generation stays quick.
    if with_images.unwrap_or(false) {
        if let Some(base) = &searxng {
            let mut used = 0;
            for sec in sections.iter_mut() {
                if used >= 6 {
                    break;
                }
                let Some(q) = sec.image_query.as_deref().filter(|q| !q.is_empty()) else {
                    continue; // model didn't ask for an image here
                };
                if let Some(first) = searxng_images(base, q, 1).into_iter().next() {
                    sec.image = Some(first.img);
                    used += 1;
                }
            }
        }
    }

    {
        let c = state.db.lock().unwrap();
        repo::save_cheatsheet(&c, &subject_id, topic_id.as_deref(), &sections)?;
        repo::snapshot_cheatsheet_version(&c, &subject_id, topic_id.as_deref(), &sections, "generated")?;
    }

    Ok(CheatsheetData {
        subject: subject_name,
        topic: topic_name,
        sources,
        sources_used,
        model: model.name(),
        sections,
    })
    })
    .await
    .map_err(|e| Error::Other(format!("background task failed: {e}")))?
}

/// Read the stored cheatsheet for a subject/topic (None if none generated yet).
#[tauri::command]
pub fn get_cheatsheet(
    state: State<AppState>,
    subject_id: String,
    topic_id: Option<String>,
) -> Result<Option<CheatsheetData>> {
    let c = state.db.lock().unwrap();
    let sections = repo::get_cheatsheet_sections(&c, &subject_id, topic_id.as_deref())?;
    if sections.is_empty() {
        return Ok(None);
    }
    let subj = repo::get_subject(&c, &subject_id)?;
    let tname = topic_id
        .as_ref()
        .and_then(|tid| subj.topics.iter().find(|t| &t.id == tid))
        .map(|t| t.name.clone())
        .unwrap_or_else(|| subj.name.clone());
    let (_, n) = repo::context_text(&c, &subject_id, topic_id.as_deref(), None, 1)?;
    Ok(Some(CheatsheetData {
        subject: subj.name,
        topic: tname,
        sources: n,
        sources_used: n,
        model: "stored".into(),
        sections,
    }))
}

/// Compose the whole-subject cheatsheet from the per-topic (and ungrouped
/// "General") sheets that are ALREADY stored — never re-synthesized — so each
/// topic's block is byte-for-byte that topic's own sheet. Topics with no sources,
/// or no generated sheet yet, contribute nothing. A lightweight divider section
/// (id `__topic__…`, no items) marks the start of each topic. Returns None when no
/// bucket has a sheet yet.
fn compose_subject_cheatsheet(c: &Connection, subject_id: &str) -> Result<Option<CheatsheetData>> {
    let subj = repo::get_subject(c, subject_id)?;
    let mut total: i64 = 0;

    // Buckets in display order: each topic in order, then General (ungrouped) last.
    let mut buckets: Vec<(Option<String>, String)> = subj
        .topics
        .iter()
        .map(|t| (Some(t.id.clone()), t.name.clone()))
        .collect();
    buckets.push((None, "General".into()));

    // Load each bucket's stored sheet (only buckets with sources AND a sheet).
    let mut loaded: Vec<(String, Vec<CsSection>)> = Vec::new(); // (topic name, sections)
    for (tid, name) in &buckets {
        let ids = repo::bucket_source_ids(c, subject_id, tid.as_deref())?;
        if ids.is_empty() {
            continue; // no sources -> no block
        }
        let secs = repo::get_cheatsheet_sections(c, subject_id, tid.as_deref())?;
        if secs.is_empty() {
            continue; // has sources but no sheet generated yet
        }
        total += ids.len() as i64;
        loaded.push((name.clone(), secs));
    }
    if loaded.is_empty() {
        return Ok(None);
    }

    // Merge by SECTION TITLE across topics (first-seen order preserved), so the
    // whole-subject sheet has ONE "Key Concepts", ONE "Definitions", etc. — each
    // holding every topic's items, prefixed by a per-topic divider (a sentinel
    // item whose `t` starts with "__topic__"). Items are copied verbatim, so each
    // topic's content stays byte-for-byte identical to its own sheet.
    let mut order: Vec<String> = Vec::new();
    let mut by_title: std::collections::HashMap<String, Vec<CsItem>> = std::collections::HashMap::new();
    for (topic_name, secs) in &loaded {
        for sec in secs {
            let key = sec.title.trim().to_string();
            if !by_title.contains_key(&key) {
                order.push(key.clone());
            }
            let bucket = by_title.entry(key).or_default();
            bucket.push(CsItem {
                t: format!("__topic__{topic_name}"),
                d: String::new(),
            });
            bucket.extend(sec.items.iter().cloned());
        }
    }

    let sections: Vec<CsSection> = order
        .into_iter()
        .map(|title| {
            let items = by_title.remove(&title).unwrap_or_default();
            CsSection {
                id: slug(&title),
                title,
                state: "approved".into(),
                items,
                image: None,
                image_query: None,
            }
        })
        .collect();

    Ok(Some(CheatsheetData {
        subject: subj.name.clone(),
        topic: subj.name,
        sources: total,
        sources_used: total,
        model: "composed".into(),
        sections,
    }))
}

/// Whole-subject cheatsheet = composition of the stored per-topic sheets.
#[tauri::command]
pub fn get_subject_cheatsheet(
    state: State<AppState>,
    subject_id: String,
) -> Result<Option<CheatsheetData>> {
    let c = state.db.lock().unwrap();
    compose_subject_cheatsheet(&c, &subject_id)
}

/// Regenerate every bucket that has sources (each topic, plus ungrouped "General")
/// then return the freshly-composed whole-subject sheet.
#[tauri::command]
pub async fn generate_subject_cheatsheet(
    app: AppHandle,
    subject_id: String,
    with_images: Option<bool>,
) -> Result<Option<CheatsheetData>> {
    // Determine which buckets have sources (short lock, then release for the calls).
    let buckets: Vec<Option<String>> = {
        let state = app.state::<AppState>();
        let c = state.db.lock().unwrap();
        let subj = repo::get_subject(&c, &subject_id)?;
        let mut b: Vec<Option<String>> = Vec::new();
        for t in &subj.topics {
            if !repo::bucket_source_ids(&c, &subject_id, Some(&t.id))?.is_empty() {
                b.push(Some(t.id.clone()));
            }
        }
        if !repo::bucket_source_ids(&c, &subject_id, None)?.is_empty() {
            b.push(None);
        }
        b
    };
    // Generate buckets CONCURRENTLY — a separate generation per topic, all at
    // once — capped so a big subject doesn't overwhelm the provider's rate
    // limits. Each generate_cheatsheet holds the DB lock only briefly (the slow
    // model calls run lock-free), so concurrency is safe.
    const MAX_CONCURRENT: usize = 5;
    let mut first_err: Option<Error> = None;
    for chunk in buckets.chunks(MAX_CONCURRENT) {
        let mut handles = Vec::new();
        for tid in chunk {
            let app2 = app.clone();
            let sid = subject_id.clone();
            let tid = tid.clone();
            handles.push(tauri::async_runtime::spawn(async move {
                generate_cheatsheet(app2, sid, tid, with_images).await
            }));
        }
        for h in handles {
            match h.await {
                Ok(Ok(_)) => {}
                // One topic failing (e.g. a rate limit) shouldn't sink the rest;
                // remember the first error and keep going — coverage will show gaps.
                Ok(Err(e)) => {
                    first_err.get_or_insert(e);
                }
                Err(e) => {
                    first_err.get_or_insert_with(|| {
                        Error::Other(format!("subject cheatsheet task failed: {e}"))
                    });
                }
            }
        }
    }
    let composed = {
        let state = app.state::<AppState>();
        let c = state.db.lock().unwrap();
        compose_subject_cheatsheet(&c, &subject_id)?
    };
    // Surface a failure only if nothing at all could be produced.
    match (composed, first_err) {
        (Some(data), _) => Ok(Some(data)),
        (None, Some(e)) => Err(e),
        (None, None) => Ok(None),
    }
}

/// Persist a user-edited cheatsheet (from the editor) for a subject/topic and
/// snapshot it as a version so the history/diff records the change.
#[tauri::command]
pub fn update_cheatsheet(
    state: State<AppState>,
    subject_id: String,
    topic_id: Option<String>,
    sections: Vec<CsSection>,
    snapshot: Option<bool>,
) -> Result<()> {
    let c = state.db.lock().unwrap();
    repo::save_cheatsheet(&c, &subject_id, topic_id.as_deref(), &sections)?;
    // Inline autosave passes snapshot=false to avoid spamming the version history on
    // every keystroke; the explicit Save / Esc-exit records one "edited" version.
    if snapshot.unwrap_or(true) {
        repo::snapshot_cheatsheet_version(&c, &subject_id, topic_id.as_deref(), &sections, "edited")?;
    }
    Ok(())
}

/// List stored versions (newest first) for the git-like history panel.
#[tauri::command]
pub fn list_cheatsheet_versions(
    state: State<AppState>,
    subject_id: String,
    topic_id: Option<String>,
) -> Result<Vec<CheatsheetVersionMeta>> {
    let c = state.db.lock().unwrap();
    repo::list_cheatsheet_versions(&c, &subject_id, topic_id.as_deref())
}

/// Read the full section set of one stored version (for diffing).
#[tauri::command]
pub fn get_cheatsheet_version(state: State<AppState>, version_id: String) -> Result<Vec<CsSection>> {
    let c = state.db.lock().unwrap();
    repo::get_cheatsheet_version(&c, &version_id)
}

/// Restore a stored version as the live cheatsheet. First snapshots the CURRENT
/// sheet as a new "before restore" version (so the restore itself is undoable),
/// then overwrites the live sheet with the chosen version's sections — scoped to
/// the version's own subject/topic. Returns the restored cheatsheet.
#[tauri::command]
pub fn restore_cheatsheet_version(
    state: State<AppState>,
    version_id: String,
) -> Result<CheatsheetData> {
    let c = state.db.lock().unwrap();
    // The version row carries the scope it belongs to — restore overwrites THAT sheet.
    let (subject_id, topic_id, sections) = repo::get_cheatsheet_version_full(&c, &version_id)?;
    let topic = topic_id.as_deref();

    // Snapshot the current live sheet first so the restore can itself be undone.
    let current = repo::get_cheatsheet_sections(&c, &subject_id, topic)?;
    if !current.is_empty() {
        repo::snapshot_cheatsheet_version(&c, &subject_id, topic, &current, "before restore")?;
    }

    // Overwrite the live sheet with the chosen version, then record the restore.
    repo::save_cheatsheet(&c, &subject_id, topic, &sections)?;
    repo::snapshot_cheatsheet_version(&c, &subject_id, topic, &sections, "restored")?;

    // Mirror get_cheatsheet's response shape so the view can apply it directly.
    let subj = repo::get_subject(&c, &subject_id)?;
    let tname = topic
        .and_then(|tid| subj.topics.iter().find(|t| t.id == tid))
        .map(|t| t.name.clone())
        .unwrap_or_else(|| subj.name.clone());
    let (_, n) = repo::context_text(&c, &subject_id, topic, None, 1)?;
    Ok(CheatsheetData {
        subject: subj.name,
        topic: tname,
        sources: n,
        sources_used: n,
        model: "stored".into(),
        sections,
    })
}

// ---- AI: material generation -----------------------------------------

/// Generate a study material (flashcards | quiz) from a subject/topic's sources.
#[tauri::command]
pub async fn generate_material(
    app: AppHandle,
    subject_id: String,
    topic_id: Option<String>,
    kind: String,
    title: Option<String>,
    custom_prompt: Option<String>,
    source_ids: Option<Vec<String>>,
    count: Option<u32>,
) -> Result<MaterialRec> {
    tauri::async_runtime::spawn_blocking(move || -> Result<MaterialRec> {
    let state = app.state::<AppState>();
    let setting_key = match kind.as_str() {
        "quiz" => "model_quiz",
        "audio" => "model_audio",
        "flashcards" => "model_flashcard",
        _ => "model_cheatsheet",
    };
    // Title-case a stored voice id ("nova" → "Nova") for use as a host name.
    let cap = |id: String| {
        let mut ch = id.chars();
        match ch.next() {
            Some(f) => f.to_uppercase().chain(ch).collect::<String>(),
            None => "Host".to_string(),
        }
    };
    let (context, subject_name, topic_name, spec, keys, style, host_a, host_b) = {
        let c = state.db.lock().unwrap();
        // The user's explicit source selection is authoritative: scope context to
        // exactly those sources (ignoring topic, since a selection can span topics).
        // Fall back to topic/subject scope only when nothing was selected.
        let has_sel = source_ids.as_ref().map_or(false, |v| !v.is_empty());
        let (ctx, _) = if has_sel {
            repo::context_text(&c, &subject_id, None, source_ids.as_deref(), 18000)?
        } else {
            repo::context_text(&c, &subject_id, topic_id.as_deref(), None, 18000)?
        };
        let subj = repo::get_subject(&c, &subject_id)?;
        let tname = topic_id
            .as_ref()
            .and_then(|tid| subj.topics.iter().find(|t| &t.id == tid))
            .or_else(|| subj.topics.first())
            .map(|t| t.name.clone())
            .unwrap_or_default();
        let spec = repo::get_setting(&c, setting_key)?
            .unwrap_or_else(|| "openrouter:deepseek/deepseek-v4-flash".into());
        guard_offline_llm(&c, &spec)?;
        let host_a = cap(repo::get_setting(&c, "voice_a")?.unwrap_or_else(|| "maya".into()));
        let host_b = cap(repo::get_setting(&c, "voice_b")?.unwrap_or_else(|| "theo".into()));
        (ctx, subj.name, tname, spec, read_keys(&c)?, style_instruction(&c), host_a, host_b)
    };
    let mut model = llm::from_spec_or_any(&spec, &keys).ok_or_else(|| Error::Other(NO_MODEL.into()))?;
    {
        let task = setting_key.strip_prefix("model_").unwrap_or("cheatsheet");
        let c = state.db.lock().unwrap();
        apply_budget(&mut model, &c, task);
    }
    if context.trim().is_empty() {
        return Err(Error::Other(
            "No source text to generate from — add and ingest a source first.".into(),
        ));
    }

    // How many items to generate, when the kind is count-based. The user can set
    // this in GenerateMaterial; clamp to a sane range so a bad value can't ask the
    // model for 0 or 500 cards.
    let quiz_n = count.unwrap_or(9).clamp(3, 30);
    let card_n = count.unwrap_or(14).clamp(4, 40);

    // Per-kind prompt + payload shape.
    let (system, default_title) = match kind.as_str() {
        "quiz" => (
            format!(
                "You generate quiz questions from study material. Output ONLY a JSON array of \
                 EXACTLY {quiz_n} items, each: {{\"q\":\"question\",\"options\":[\"a\",\"b\",\"c\",\"d\"],\
                 \"answer\":<index 0-3>,\"explain\":\"why\"}}. No prose."
            ),
            format!("{topic_name} quiz"),
        ),
        "audio" => (
            format!(
                "You write a two-host podcast-style audio overview script from study material. \
                 The two hosts are named {host_a} and {host_b}. Output ONLY JSON: \
                 {{\"segments\":[{{\"speaker\":\"{host_a}\"|\"{host_b}\",\"text\":\"...\"}}]}}. 12-20 \
                 lively, accurate segments that teach the material conversationally. No prose outside JSON."
            ),
            format!("{topic_name} — audio overview"),
        ),
        "infographic" => (
            "You distill the study material into a DETAILED ONE-POSTER infographic as STRUCTURED \
             JSON (NOT an image, NOT SVG) — it is rendered as a crisp HTML poster, so be \
             information-rich. Output ONLY JSON of this exact shape:\n\
             {\"title\":string,\"subtitle\":string,\
             \"sections\":[{\"emoji\":string,\"heading\":string,\"points\":[string],\
             \"stat\":{\"value\":string,\"label\":string}}],\
             \"timeline\":[{\"date\":string,\"title\":string,\"detail\":string}],\
             \"takeaway\":string}\n\
             Rules:\n\
             - 5-8 \"sections\"; \"heading\" 1-4 words; 3-5 \"points\" per section, each an \
             informative phrase (<= 16 words, plain text, NO markdown) — favour concrete facts, \
             definitions, and cause/effect over vague labels.\n\
             - \"emoji\" is ONE relevant emoji per section.\n\
             - \"stat\" is OPTIONAL per section — include only for a real headline figure from the \
             source (value e.g. \"30%\"/\"1990\", short label).\n\
             - \"timeline\" is a chronological list of 4-8 KEY EVENTS, developments, steps, or \
             milestones the material describes. Each has a \"date\" (a year, range, phase name, or \
             ordinal like \"Step 1\" when no real dates exist), a short \"title\" (<= 8 words), and \
             a \"detail\" (one clear sentence, <= 24 words). Order earliest→latest. Omit the \
             timeline ONLY if the material is genuinely non-sequential and has no events, stages, \
             or process.\n\
             - \"takeaway\" is ONE punchy sentence summarising the single most important idea.\n\
             Cover the most exam-relevant ideas thoroughly. No prose outside the JSON.".to_string(),
            format!("{topic_name} — infographic"),
        ),
        "slideshow" => (
            "You produce a slideshow outline from study material. Output ONLY JSON: \
             {\"slides\":[{\"title\":\"...\",\"bullets\":[\"...\"],\"notes\":\"voiceover\"}]}. 8-12 slides. No prose outside JSON.".to_string(),
            format!("{topic_name} — slideshow"),
        ),
        "mindmap" => (
            "You build a hierarchical MIND MAP / concept map from the study material. Output ONLY \
             JSON of this exact shape:\n\
             {\"central\":string,\"branches\":[{\"label\":string,\"children\":\
             [{\"label\":string,\"children\":[{\"label\":string}]}]}]}\n\
             Rules: \"central\" is the single core topic (1-4 words). 4-7 \"branches\" for the main \
             themes/categories. Each branch has 2-5 \"children\" (key concepts), and a child MAY \
             have its own 2-4 \"children\" for a third level (details/examples) — go three levels \
             deep where the material supports it, otherwise stop at two. Every \"label\" is a tight \
             noun phrase (<= 6 words, plain text, NO markdown, NO trailing punctuation). Capture \
             the material's structure and how ideas relate. No prose outside the JSON.".to_string(),
            format!("{topic_name} — mind map"),
        ),
        _ => (
            format!(
                "You generate study flashcards from material. Output ONLY a JSON array of EXACTLY \
                 {card_n} items, each {{\"q\":\"front\",\"a\":\"back\"}}. Keep the \"a\" SHORT and \
                 punchy — ideally one tight sentence or a few words (8-25 words max); never a \
                 paragraph. **bold** the single key term; use a short `- ` bullet list ONLY when \
                 the answer is genuinely multi-part. No headings, no preamble, no prose outside the JSON."
            ),
            format!("{topic_name} flashcards"),
        ),
    };
    // Uniform guardrail: many models wrap JSON in ```json fences or add prose.
    // extract_json tolerates that, but instructing raw JSON makes it far more
    // reliable end-to-end (and avoids truncation from wasted fence tokens).
    let system = format!(
        "{system}{style} Respond with ONLY raw JSON — no markdown code fences, no prose before or after.{custom}",
        custom = custom_focus(custom_prompt.as_deref())
    );
    let user = format!("Subject: {subject_name} › {topic_name}\n\nSOURCE MATERIAL:\n{context}\n\nGenerate now.");

    let raw = model.complete(&system, &user)?;
    let payload = llm::extract_json(&raw)
        .map_err(|_| Error::Other("model returned unstructured output; try again".into()))?;

    // Infographic now renders as a DETAILED HTML poster (with a timeline) from the
    // structured spec — crisp, correct text instead of an image model's garbled
    // typography. The poster-image path (`render_infographic_image`) is retained but
    // unused; new infographics use the HTML renderer (InfographicView) directly.

    let meta = match kind.as_str() {
        "quiz" => format!("{} questions", payload.as_array().map(|a| a.len()).unwrap_or(0)),
        "flashcards" => format!("{} cards", payload.as_array().map(|a| a.len()).unwrap_or(0)),
        "audio" => format!(
            "{} segments · podcast-style",
            payload["segments"].as_array().map(|a| a.len()).unwrap_or(0)
        ),
        "slideshow" => format!(
            "{} slides",
            payload["slides"].as_array().map(|a| a.len()).unwrap_or(0)
        ),
        "infographic" => format!(
            "{} sections",
            payload["sections"].as_array().map(|a| a.len()).unwrap_or(0)
        ),
        "mindmap" => format!(
            "{} branches",
            payload["branches"].as_array().map(|a| a.len()).unwrap_or(0)
        ),
        _ => "generated".to_string(),
    };
    let title = title.unwrap_or(default_title);

    let id = {
        let c = state.db.lock().unwrap();
        repo::save_material(&c, &subject_id, topic_id.as_deref(), &kind, &title, &meta, &payload)?
    };

    Ok(MaterialRec {
        id,
        kind,
        title,
        topic: topic_name,
        meta,
        status: "ready".into(),
        payload,
    })
    })
    .await
    .map_err(|e| Error::Other(format!("background task failed: {e}")))?
}

#[derive(serde::Deserialize)]
pub struct TtsSegment {
    #[serde(default)]
    pub speaker: String,
    #[serde(default)]
    pub text: String,
}

/// Turn a generated two-host podcast script into a REAL audio file (NotebookLM
/// style) using a cloud TTS — OpenAI's `/audio/speech` with the user's two
/// configured voices. Returns the path to the cached mp3 under `$APPDATA/
/// audio_overviews/` (served to the webview via the asset protocol). The result
/// is cached by material id, so re-calling is instant unless `force` is set. When
/// there's no OpenAI key (offline), this errors and the frontend falls back to
/// on-device speech synthesis.
#[tauri::command]
pub async fn synthesize_overview(
    app: AppHandle,
    material_id: String,
    segments: Vec<TtsSegment>,
    force: Option<bool>,
) -> Result<String> {
    tauri::async_runtime::spawn_blocking(move || -> Result<String> {
        let state = app.state::<AppState>();
        let dir = app
            .path()
            .app_data_dir()
            .unwrap_or_else(|_| std::env::temp_dir())
            .join("audio_overviews");
        std::fs::create_dir_all(&dir).ok();
        let out = dir.join(format!("{material_id}.mp3"));
        if !force.unwrap_or(false) && out.exists() {
            return Ok(out.to_string_lossy().to_string());
        }
        if segments.is_empty() {
            return Err(Error::Other("No script segments to synthesize.".into()));
        }
        // OpenAI key + the two voices (sensible OpenAI-voice defaults).
        let (key, voice_a, voice_b) = {
            let c = state.db.lock().unwrap();
            let key = read_keys(&c)?
                .openai
                .filter(|k| !k.trim().is_empty())
                .ok_or_else(|| {
                    Error::Other(
                        "Add an OpenAI API key in Settings to generate real audio (offline mode uses on-device voices)."
                            .into(),
                    )
                })?;
            let va = repo::get_setting(&c, "tts_voice_a")?.unwrap_or_else(|| "alloy".into());
            let vb = repo::get_setting(&c, "tts_voice_b")?.unwrap_or_else(|| "onyx".into());
            (key, va, vb)
        };
        let client = http_client(180);
        // First distinct speaker → voice_a, the other → voice_b. mp3 frames
        // concatenate cleanly enough for sequential playback in the webview.
        let mut audio: Vec<u8> = Vec::new();
        let mut first: Option<String> = None;
        for seg in &segments {
            let text = seg.text.trim();
            if text.is_empty() {
                continue;
            }
            let spk = seg.speaker.trim().to_lowercase();
            let voice = match &first {
                None => {
                    first = Some(spk.clone());
                    voice_a.clone()
                }
                Some(f) if *f == spk => voice_a.clone(),
                _ => voice_b.clone(),
            };
            let body = serde_json::json!({
                "model": "tts-1",
                "voice": voice,
                "input": text,
                "response_format": "mp3",
            });
            let resp = client
                .post("https://api.openai.com/v1/audio/speech")
                .header("Authorization", format!("Bearer {}", key.trim()))
                .json(&body)
                .send()?;
            if !resp.status().is_success() {
                let code = resp.status();
                let detail = resp.text().unwrap_or_default();
                return Err(Error::Other(format!(
                    "TTS request failed ({code}): {}",
                    truncate(&detail, 200)
                )));
            }
            audio.extend_from_slice(&resp.bytes()?);
        }
        if audio.is_empty() {
            return Err(Error::Other("TTS produced no audio.".into()));
        }
        std::fs::write(&out, &audio)
            .map_err(|e| Error::Other(format!("Could not save audio file: {e}")))?;
        Ok(out.to_string_lossy().to_string())
    })
    .await
    .map_err(|e| Error::Other(format!("background task failed: {e}")))?
}

#[tauri::command]
pub fn list_materials(state: State<AppState>, subject_id: String) -> Result<Vec<MaterialRec>> {
    let c = state.db.lock().unwrap();
    repo::list_materials(&c, &subject_id)
}

#[tauri::command]
pub fn delete_material(state: State<AppState>, id: String) -> Result<()> {
    let c = state.db.lock().unwrap();
    repo::delete_material(&c, &id)
}

#[tauri::command]
pub fn rename_material(state: State<AppState>, id: String, title: String) -> Result<()> {
    let c = state.db.lock().unwrap();
    repo::rename_material(&c, &id, &title)
}

// ---- citations (per-subject bibliography) -----------------------------

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub fn add_citation(
    state: State<AppState>,
    subject_id: String,
    ctype: String,
    title: String,
    authors: Option<String>,
    year: Option<String>,
    container: Option<String>,
    url: Option<String>,
    doi: Option<String>,
    notes: Option<String>,
) -> Result<String> {
    let c = state.db.lock().unwrap();
    repo::insert_citation(
        &c, &subject_id, &ctype, &title,
        authors.as_deref(), year.as_deref(), container.as_deref(),
        url.as_deref(), doi.as_deref(), notes.as_deref(),
    )
}

#[tauri::command]
pub fn list_citations(state: State<AppState>, subject_id: String) -> Result<Vec<Reference>> {
    let c = state.db.lock().unwrap();
    repo::list_citations(&c, &subject_id)
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub fn update_citation(
    state: State<AppState>,
    id: String,
    ctype: String,
    title: String,
    authors: Option<String>,
    year: Option<String>,
    container: Option<String>,
    url: Option<String>,
    doi: Option<String>,
    notes: Option<String>,
) -> Result<()> {
    let c = state.db.lock().unwrap();
    repo::update_citation(
        &c, &id, &ctype, &title,
        authors.as_deref(), year.as_deref(), container.as_deref(),
        url.as_deref(), doi.as_deref(), notes.as_deref(),
    )
}

#[tauri::command]
pub fn delete_citation(state: State<AppState>, id: String) -> Result<()> {
    let c = state.db.lock().unwrap();
    repo::delete_citation(&c, &id)
}

// ---- settings (bulk, for the Settings page) ---------------------------

#[tauri::command]
pub fn get_all_settings(state: State<AppState>) -> Result<serde_json::Value> {
    let c = state.db.lock().unwrap();
    repo::all_settings(&c)
}

#[tauri::command]
pub fn set_settings(state: State<AppState>, values: std::collections::HashMap<String, String>) -> Result<()> {
    let c = state.db.lock().unwrap();
    for (k, v) in values {
        repo::set_setting(&c, &k, &v)?;
    }
    Ok(())
}

// ---- lecture recording + Whisper transcription -----------------------

/// Transcribe an audio file with a local Whisper CLI if one is installed
/// (openai-whisper `whisper`, or whisper.cpp `whisper-cli`/`main`). Returns
/// `(transcript, warning)`. A missing binary is a graceful warning, not an error.
/// The configured homelab Whisper base URL, if any (empty → None).
fn whisper_remote_url(state: &AppState) -> Option<String> {
    let c = state.db.lock().ok()?;
    // Resolve through the homelab fallback chain (local → Tailscale → public).
    crate::homelab::resolved_setting(&c, "whisper_url")
}

/// Model name sent to the remote (OpenAI-compatible) Whisper endpoint. A homelab
/// faster-whisper/speaches server needs a faster-whisper model id it can load, not
/// OpenAI's "whisper-1" — so this is configurable (Settings → Integrations). When the
/// user hasn't picked one we return EMPTY, which makes `transcribe_remote` omit the
/// `model` form field entirely so the homelab Whisper uses its OWN configured default
/// (WHISPER__MODEL). That avoids the "model not installed" error from sending a model
/// id the server doesn't have loaded — the cause of having to install one by hand.
fn whisper_model(state: &AppState) -> String {
    state
        .db
        .lock()
        .ok()
        .and_then(|c| crate::repo::get_setting(&c, "whisper_model").ok().flatten())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_default()
}

// ---- external dependency status -----------------------------------------

#[derive(serde::Serialize)]
pub struct DepStatus {
    pub name: String,
    pub detail: String,
    pub present: bool,
}
#[derive(serde::Serialize)]
pub struct DependencyReport {
    pub manager: String,           // detected package manager (pacman/apt/…)
    pub deps: Vec<DepStatus>,
    pub install_command: String,   // one command to install everything missing
    pub note: String,
}

/// Detect the system package manager so we can suggest the right install command.
fn detect_pkg_manager() -> &'static str {
    if std::env::consts::OS == "macos" {
        return "brew";
    }
    if std::env::consts::OS == "windows" {
        return "winget";
    }
    let id = std::fs::read_to_string("/etc/os-release").unwrap_or_default().to_lowercase();
    if id.contains("arch") || id.contains("manjaro") || id.contains("omarchy") {
        "pacman"
    } else if id.contains("debian") || id.contains("ubuntu") || id.contains("pop") || id.contains("mint") {
        "apt"
    } else if id.contains("fedora") || id.contains("rhel") || id.contains("centos") {
        "dnf"
    } else {
        "pacman" // sensible default for this user's Arch-based setup
    }
}

/// How Cortex was installed, so the UI routes self-updates correctly. Tauri's Linux
/// auto-updater only works for AppImage; deb/rpm/pacman installs must update via the
/// system package manager. AppImage sets the APPIMAGE env var at runtime.
#[tauri::command]
pub fn install_kind() -> String {
    if cfg!(target_os = "linux") {
        if std::env::var_os("APPIMAGE").is_some() {
            "appimage".to_string()
        } else {
            "linux-package".to_string()
        }
    } else if cfg!(target_os = "macos") {
        "macos".to_string()
    } else if cfg!(target_os = "windows") {
        "windows".to_string()
    } else {
        "unknown".to_string()
    }
}

/// Status of every external tool Cortex shells out to + a copy-pasteable command
/// to install whatever's missing. We don't run installers ourselves — system
/// packages need sudo and vary by distro — but this makes setup one paste.
#[tauri::command]
pub fn dependency_status() -> DependencyReport {
    let present = |bins: &[&str]| bins.iter().any(|b| ingest::which(b).is_some());
    // (label, detail, binaries-that-satisfy-it, package per manager, pip?)
    struct Dep { name: &'static str, detail: &'static str, bins: &'static [&'static str], pac: &'static str, apt: &'static str, dnf: &'static str, brew: &'static str, pip: bool }
    let table: &[Dep] = &[
        Dep { name: "PDF text & page images", detail: "poppler", bins: &["pdftotext", "pdftoppm"], pac: "poppler", apt: "poppler-utils", dnf: "poppler-utils", brew: "poppler", pip: false },
        Dep { name: "Office documents (docx/pptx)", detail: "LibreOffice", bins: &["libreoffice", "soffice"], pac: "libreoffice-fresh", apt: "libreoffice", dnf: "libreoffice", brew: "libreoffice", pip: false },
        Dep { name: "Audio conversion", detail: "ffmpeg", bins: &["ffmpeg"], pac: "ffmpeg", apt: "ffmpeg", dnf: "ffmpeg", brew: "ffmpeg", pip: false },
        Dep { name: "OCR (scanned PDFs/images)", detail: "Tesseract", bins: &["tesseract"], pac: "tesseract tesseract-data-eng", apt: "tesseract-ocr", dnf: "tesseract", brew: "tesseract", pip: false },
        Dep { name: "Local transcription", detail: "openai-whisper", bins: &["whisper", "whisper-cli", "main"], pac: "", apt: "", dnf: "", brew: "", pip: true },
        Dep { name: "YouTube ingest", detail: "yt-dlp (auto-downloaded if missing)", bins: &["yt-dlp"], pac: "yt-dlp", apt: "yt-dlp", dnf: "yt-dlp", brew: "yt-dlp", pip: false },
        Dep { name: "Music playback", detail: "mpv", bins: &["mpv"], pac: "mpv", apt: "mpv", dnf: "mpv", brew: "mpv", pip: false },
    ];
    let mgr = detect_pkg_manager();
    let mut deps = Vec::new();
    let mut sys_pkgs: Vec<&str> = Vec::new();
    let mut need_whisper = false;
    for d in table {
        let ok = present(d.bins);
        deps.push(DepStatus { name: d.name.into(), detail: d.detail.into(), present: ok });
        if ok {
            continue;
        }
        if d.pip {
            need_whisper = true;
        } else {
            let pkg = match mgr { "apt" => d.apt, "dnf" => d.dnf, "brew" => d.brew, "winget" => "", _ => d.pac };
            if !pkg.is_empty() {
                sys_pkgs.push(pkg);
            }
        }
    }
    let mut parts: Vec<String> = Vec::new();
    if !sys_pkgs.is_empty() {
        let list = sys_pkgs.join(" ");
        let cmd = match mgr {
            "apt" => format!("sudo apt install -y {list}"),
            "dnf" => format!("sudo dnf install -y {list}"),
            "brew" => format!("brew install {list}"),
            "winget" => String::new(),
            _ => format!("sudo pacman -S --needed {list}"),
        };
        if !cmd.is_empty() {
            parts.push(cmd);
        }
    }
    if need_whisper {
        parts.push("pipx install openai-whisper".to_string());
    }
    DependencyReport {
        manager: mgr.to_string(),
        deps,
        install_command: parts.join(" && "),
        note: if mgr == "winget" {
            "Install these via your package manager of choice.".into()
        } else {
            "Run this in a terminal. yt-dlp is also auto-downloaded by Cortex when needed.".into()
        },
    }
}

/// One-click install of the missing system dependencies. macOS only: Homebrew
/// needs no sudo, so it's safe to run for the user. Linux/Windows managers need a
/// terminal (sudo), so there we return the command for them to paste. Runs only the
/// `brew install …` part (whisper/pipx installs on first transcription instead).
#[tauri::command]
pub async fn install_dependencies() -> Result<String> {
    tauri::async_runtime::spawn_blocking(|| -> Result<String> {
        let report = dependency_status();
        if report.install_command.is_empty() {
            return Ok("All dependencies are already installed.".into());
        }
        if report.manager != "brew" {
            return Err(Error::Other(format!(
                "One-click install is macOS-only (Homebrew needs no sudo). Run this in a terminal:\n{}",
                report.install_command
            )));
        }
        let Some(brew_cmd) = report
            .install_command
            .split(" && ")
            .find(|c| c.starts_with("brew install"))
            .map(str::to_string)
        else {
            return Ok("Nothing to install via Homebrew (whisper installs on first transcription).".into());
        };
        // A GUI app's inherited PATH usually omits Homebrew, so prepend it explicitly.
        let extra = "/opt/homebrew/bin:/opt/homebrew/sbin:/usr/local/bin:/usr/local/sbin";
        let path = std::env::var("PATH")
            .map(|p| format!("{extra}:{p}"))
            .unwrap_or_else(|_| extra.to_string());
        let out = std::process::Command::new("sh")
            .arg("-c")
            .arg(&brew_cmd)
            .env("PATH", path)
            .env("HOMEBREW_NO_AUTO_UPDATE", "1")
            .output()
            .map_err(|e| Error::Other(format!("couldn't start Homebrew: {e}. Is brew installed?")))?;
        if out.status.success() {
            Ok("Dependencies installed — hit Re-check to confirm.".into())
        } else {
            let err = String::from_utf8_lossy(&out.stderr);
            let tail = err.lines().rev().take(8).collect::<Vec<_>>();
            let tail = tail.into_iter().rev().collect::<Vec<_>>().join("\n");
            Err(Error::Other(format!("Homebrew install failed:\n{tail}")))
        }
    })
    .await
    .map_err(|e| Error::Other(e.to_string()))?
}

#[derive(serde::Serialize)]
pub struct FolderFile {
    pub path: String,
    pub name: String,
}

/// Recursively list the ingestable files in a folder (the supported source types),
/// so "Add folder" can queue each one. Bounded depth + capped count so a huge tree
/// can't hang the UI. Skips hidden dirs and anything Cortex can't parse.
#[tauri::command]
pub fn list_folder_sources(dir: String) -> Result<Vec<FolderFile>> {
    const EXTS: &[&str] = &[
        "pdf", "docx", "pptx", "doc", "ppt", "txt", "md", "png", "jpg", "jpeg", "webp",
    ];
    fn walk(dir: &Path, out: &mut Vec<FolderFile>, depth: usize) {
        if depth > 8 || out.len() >= 500 {
            return;
        }
        let Ok(rd) = std::fs::read_dir(dir) else { return };
        for entry in rd.flatten() {
            if out.len() >= 500 {
                return;
            }
            let p = entry.path();
            let hidden = p
                .file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.starts_with('.'))
                .unwrap_or(false);
            if hidden {
                continue;
            }
            if p.is_dir() {
                walk(&p, out, depth + 1);
            } else if let Some(ext) = p.extension().and_then(|e| e.to_str()) {
                if EXTS.contains(&ext.to_lowercase().as_str()) {
                    let name = p
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("file")
                        .to_string();
                    out.push(FolderFile {
                        path: p.to_string_lossy().into_owned(),
                        name,
                    });
                }
            }
        }
    }
    let mut out = Vec::new();
    walk(Path::new(&dir), &mut out, 0);
    Ok(out)
}

/// Restrict a client-supplied audio extension to known containers so it can't
/// smuggle path separators or oddities into the recordings filename.
fn sanitize_ext(ext: Option<&str>) -> &'static str {
    match ext.map(|e| e.trim().to_ascii_lowercase()).as_deref() {
        Some("wav") => "wav",
        Some("ogg") => "ogg",
        Some("mp3") => "mp3",
        Some("m4a") => "m4a",
        Some("mp4") => "mp4",
        Some("flac") => "flac",
        Some("opus") => "opus",
        _ => "webm",
    }
}

fn transcribe(
    file: &Path,
    data_dir: &Path,
    allow_install: bool,
    remote_url: Option<&str>,
    remote_model: &str,
) -> (String, Option<String>) {
    use std::process::Command;
    let outdir = std::env::temp_dir().join(format!("cortex-asr-{}", crate::db::new_id()));
    let _ = std::fs::create_dir_all(&outdir);

    // 0. Remote homelab Whisper (OpenAI-compatible /v1/audio/transcriptions).
    //    Tried first when configured so users never need a local Python toolchain.
    if let Some(base) = remote_url.map(str::trim).filter(|s| !s.is_empty()) {
        match transcribe_remote(base, file, remote_model) {
            Ok(t) => {
                let _ = std::fs::remove_dir_all(&outdir);
                return (t, None); // empty = ran but recognised nothing
            }
            // Remote configured but unreachable/erroring: surface it rather than
            // silently falling back to a (likely absent) local toolchain.
            Err(e) => {
                let _ = std::fs::remove_dir_all(&outdir);
                return (
                    String::new(),
                    Some(format!(
                        "Couldn't reach your homelab Whisper server at {base} — {e}. \
                         Check Settings → Integrations → Remote transcription, or clear it to \
                         transcribe on this machine."
                    )),
                );
            }
        }
    }

    // openai-whisper. Fall back to ~/.local/bin/whisper (pip --user install dir)
    // in case the app was launched without it on PATH (desktop launchers often
    // don't inherit the user's shell PATH).
    let whisper_bin = ingest::which("whisper").or_else(|| {
        // Desktop launchers often don't inherit the user's shell PATH, so also
        // probe the common pip/user/system install locations explicitly.
        let mut cands: Vec<std::path::PathBuf> = Vec::new();
        if let Ok(h) = std::env::var("HOME") {
            cands.push(std::path::Path::new(&h).join(".local/bin/whisper"));
        }
        cands.push("/usr/local/bin/whisper".into());
        cands.push("/usr/bin/whisper".into());
        cands
            .into_iter()
            .find(|p| p.is_file())
            .map(|p| p.to_string_lossy().into_owned())
    });
    if let Some(bin) = whisper_bin {
        let out = Command::new(&bin)
            .arg(file)
            .args(["--model", "base", "--language", "en", "--output_format", "txt", "--output_dir"])
            .arg(&outdir)
            .output();
        if let Ok(o) = out {
            if o.status.success() {
                let stem = file.file_stem().and_then(|s| s.to_str()).unwrap_or("rec");
                if let Ok(t) = std::fs::read_to_string(outdir.join(format!("{stem}.txt"))) {
                    let _ = std::fs::remove_dir_all(&outdir);
                    return (t.trim().to_string(), None);
                }
            }
        }
    }
    // whisper.cpp (expects 16k wav; convert with ffmpeg if available)
    if let Some(bin) = ingest::which("whisper-cli").or_else(|| ingest::which("main")) {
        let wav = outdir.join("rec.wav");
        let ffmpeg = ingest::which("ffmpeg");
        let converted = ffmpeg.is_some()
            && Command::new(ffmpeg.as_deref().unwrap_or("ffmpeg"))
                .args(["-y", "-i"])
                .arg(file)
                .args(["-ar", "16000", "-ac", "1"])
                .arg(&wav)
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false);
        let target = if converted { wav.clone() } else { file.to_path_buf() };
        let out = Command::new(&bin).arg("-f").arg(&target).arg("-otxt").arg("-of").arg(outdir.join("out")).output();
        if let Ok(o) = out {
            if o.status.success() {
                if let Ok(t) = std::fs::read_to_string(outdir.join("out.txt")) {
                    let _ = std::fs::remove_dir_all(&outdir);
                    return (t.trim().to_string(), None);
                }
            }
        }
    }
    // No system Whisper — fall back to a self-managed faster-whisper venv that the
    // app sets up automatically (CTranslate2, no PyTorch). The first call on a
    // fresh machine creates the venv + pip-installs it (slow, one-off); afterwards
    // it's quick. PyAV decodes the audio, so no system ffmpeg is needed.
    // Diagnostic detail collected from the faster-whisper bootstrap so the user
    // gets a real reason instead of a generic "setup failed".
    let setup_detail;
    match faster_whisper_python(data_dir, allow_install) {
        Ok(py) => {
            let models_dir = data_dir.join("whisper-models");
            let _ = std::fs::create_dir_all(&models_dir);
            // faster-whisper decodes via PyAV, whose bundled ffmpeg often can't
            // read the browser's MediaRecorder .webm (Opus) — it throws
            // "Invalid data found when processing input". Pre-convert to 16 kHz
            // mono WAV with system ffmpeg when available; PyAV decodes WAV cleanly.
            // Track WHY conversion didn't happen so a decode failure later can
            // report the real cause instead of always blaming a missing ffmpeg.
            let wav = outdir.join("fw.wav");
            let mut ffmpeg_problem: Option<String> = None;
            let decodable = match ingest::which("ffmpeg") {
                None => {
                    ffmpeg_problem = Some(
                        "ffmpeg isn't installed — install it (e.g. `sudo pacman -S ffmpeg`) \
                         so Cortex can transcode recordings"
                            .into(),
                    );
                    file.to_path_buf()
                }
                Some(ff) => match Command::new(&ff)
                    .args(["-y", "-i"])
                    .arg(file)
                    .args(["-ar", "16000", "-ac", "1"])
                    .arg(&wav)
                    .output()
                {
                    Ok(o) if o.status.success() && wav.is_file() => wav.clone(),
                    Ok(o) => {
                        ffmpeg_problem = Some(format!(
                            "ffmpeg couldn't read the recording ({}) — the file may be \
                             corrupt or empty; try re-recording",
                            last_line(&String::from_utf8_lossy(&o.stderr))
                        ));
                        file.to_path_buf()
                    }
                    Err(e) => {
                        ffmpeg_problem = Some(format!("ffmpeg failed to run: {e}"));
                        file.to_path_buf()
                    }
                },
            };
            // argv[1] = audio file, argv[2] = model cache dir (model auto-downloads here)
            const RUNNER: &str = "import sys\nfrom faster_whisper import WhisperModel\nm=WhisperModel('base.en',device='cpu',compute_type='int8',download_root=sys.argv[2])\nsegs,_=m.transcribe(sys.argv[1],language='en')\nprint(' '.join(s.text.strip() for s in segs))";
            let out = Command::new(&py).arg("-c").arg(RUNNER).arg(&decodable).arg(&models_dir).output();
            match out {
                Ok(o) if o.status.success() => {
                    let t = String::from_utf8_lossy(&o.stdout).trim().to_string();
                    let _ = std::fs::remove_dir_all(&outdir);
                    return (t, None); // empty string = ran but recognised nothing
                }
                Ok(o) => {
                    let err = String::from_utf8_lossy(&o.stderr);
                    eprintln!("[whisper] faster-whisper run failed: {err}");
                    // Distinguish the two common runtime failures: an audio-decode
                    // error (needs system ffmpeg to transcode the recording) vs the
                    // one-off model download.
                    setup_detail = if err.contains("Invalid data") || err.contains("decode_audio") {
                        format!(
                            "couldn't decode the recording — {}",
                            ffmpeg_problem.unwrap_or_else(|| {
                                "the transcoded audio was still unreadable; try re-recording"
                                    .to_string()
                            })
                        )
                    } else {
                        format!(
                            "transcription failed — likely the model download (needs internet on \
                             first use) or low disk. Details: {}",
                            last_line(&err)
                        )
                    };
                }
                Err(e) => setup_detail = format!("couldn't run the Whisper venv: {e}"),
            }
        }
        Err(e) => setup_detail = e,
    }

    let _ = std::fs::remove_dir_all(&outdir);
    (
        String::new(),
        Some(format!(
            "Couldn't transcribe — {setup_detail}. You can instead point Cortex at a homelab \
             Whisper server in Settings → Integrations, or install `openai-whisper` / whisper.cpp \
             manually, then re-record."
        )),
    )
}

/// Last non-empty line of a (possibly multi-line) stderr blob, trimmed to a sane
/// length for a toast/message. Keeps the actionable bit (the real exception).
fn last_line(s: &str) -> String {
    let line = s.lines().rev().map(str::trim).find(|l| !l.is_empty()).unwrap_or("");
    if line.len() > 240 { format!("{}…", &line[..240]) } else { line.to_string() }
}

/// Send an audio file to an OpenAI-compatible transcription endpoint
/// (`{base}/v1/audio/transcriptions`, model `whisper-1`). Returns the text or a
/// short error. This is how the homelab Whisper service is consumed.
fn transcribe_remote(base: &str, file: &Path, model: &str) -> std::result::Result<String, String> {
    let base = base.trim_end_matches('/');
    // Accept either a bare base ("http://host:9009") or a full endpoint.
    let url = if base.ends_with("/audio/transcriptions") {
        base.to_string()
    } else if base.ends_with("/v1") {
        format!("{base}/audio/transcriptions")
    } else {
        format!("{base}/v1/audio/transcriptions")
    };
    let model = model.trim();
    // Build + send a fresh transcription request. The multipart body consumes the file
    // bytes, so we re-read the file each call — which lets us retry after pulling a model.
    let send = || -> std::result::Result<reqwest::blocking::Response, String> {
        let bytes = std::fs::read(file).map_err(|e| format!("read audio: {e}"))?;
        let fname = file
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("audio.webm")
            .to_string();
        let part = reqwest::blocking::multipart::Part::bytes(bytes)
            .file_name(fname)
            .mime_str("application/octet-stream")
            .map_err(|e| e.to_string())?;
        let mut form = reqwest::blocking::multipart::Form::new()
            .text("response_format", "text")
            .part("file", part);
        if !model.is_empty() {
            form = form.text("model", model.to_string());
        }
        // Transcription is slow; allow a generous timeout.
        http_client(600).post(&url).multipart(form).send().map_err(|e| e.to_string())
    };

    let mut resp = send()?;
    // faster-whisper / speaches answers a missing model with 404 "Model '…' is not
    // installed locally. … POST /v1/models …". Pull it on demand once, then retry, so the
    // user doesn't have to download models by hand.
    if resp.status() == reqwest::StatusCode::NOT_FOUND && !model.is_empty() {
        let body = resp.text().unwrap_or_default();
        if body.contains("not installed") || body.contains("/v1/models") {
            let models_url = format!("{}/models", url.trim_end_matches("/audio/transcriptions"));
            let _ = http_client(900)
                .post(&models_url)
                .json(&serde_json::json!({ "model": model }))
                .send(); // best-effort download; the retry surfaces any real failure
            resp = send()?;
        } else {
            return Err(format!("HTTP 404: {}", last_line(&body)));
        }
    }
    if !resp.status().is_success() {
        let code = resp.status();
        let body = resp.text().unwrap_or_default();
        return Err(format!("HTTP {code}: {}", last_line(&body)));
    }
    let body = resp.text().map_err(|e| e.to_string())?;
    // OpenAI returns plain text for response_format=text; some servers wrap JSON.
    let text = if body.trim_start().starts_with('{') {
        serde_json::from_str::<serde_json::Value>(&body)
            .ok()
            .and_then(|v| v["text"].as_str().map(|s| s.to_string()))
            .unwrap_or(body)
    } else {
        body
    };
    Ok(text.trim().to_string())
}

/// A self-managed Python venv under the app data dir with `faster-whisper`
/// installed; returns its interpreter path. When `allow_install` is false it only
/// returns an already-prepared venv (never triggers the slow first-run install) —
/// so the live partial transcriber stays snappy and only the full ingest path
/// bootstraps it.
fn faster_whisper_python(
    data_dir: &Path,
    allow_install: bool,
) -> std::result::Result<std::path::PathBuf, String> {
    use std::process::Command;
    let venv = data_dir.join("whisper-venv");
    let py = venv.join("bin").join("python");
    let ready = venv.join(".ready");
    if py.is_file() && ready.is_file() {
        return Ok(py);
    }
    if !allow_install {
        // The live partial transcriber never bootstraps; just report not-ready.
        return Err("Whisper isn't set up yet".into());
    }
    let py3 = ingest::which("python3")
        .or_else(|| ingest::which("python"))
        .ok_or_else(|| "Python 3 isn't installed or isn't on PATH".to_string())?;

    // A half-built venv from a previous failed attempt would otherwise wedge us;
    // start clean so the bootstrap is deterministic.
    if venv.exists() && !ready.is_file() {
        let _ = std::fs::remove_dir_all(&venv);
    }

    // 1. Create the venv. The classic Debian/Ubuntu failure is a missing
    //    python3-venv package (ensurepip) — detect it from stderr and say so.
    let made = Command::new(&py3).arg("-m").arg("venv").arg(&venv).output();
    match made {
        Ok(o) if o.status.success() && py.is_file() => {}
        Ok(o) => {
            let err = String::from_utf8_lossy(&o.stderr);
            let hint = if err.contains("ensurepip") || err.contains("python3-venv") {
                " — install the venv module (e.g. `sudo apt install python3-venv`)"
            } else {
                ""
            };
            return Err(format!("couldn't create the Python venv{hint}: {}", last_line(&err)));
        }
        Err(e) => return Err(format!("couldn't run python3: {e}")),
    }

    // 2. Install faster-whisper. Capture stderr so network/build failures surface.
    let installed = Command::new(&py)
        .args(["-m", "pip", "install", "-U", "pip", "faster-whisper"])
        .output();
    match installed {
        Ok(o) if o.status.success() => {}
        Ok(o) => {
            return Err(format!(
                "pip couldn't install faster-whisper (needs internet on first use): {}",
                last_line(&String::from_utf8_lossy(&o.stderr))
            ));
        }
        Err(e) => return Err(format!("couldn't run pip: {e}")),
    }

    // 3. Only mark ready once the package actually imports — a half-finished
    //    install must NOT leave a `.ready` marker that wedges every later run.
    let imports = Command::new(&py)
        .args(["-c", "import faster_whisper"])
        .output();
    match imports {
        Ok(o) if o.status.success() => {
            let _ = std::fs::write(&ready, b"1");
            Ok(py)
        }
        Ok(o) => Err(format!(
            "faster-whisper installed but won't import: {}",
            last_line(&String::from_utf8_lossy(&o.stderr))
        )),
        Err(e) => Err(format!("couldn't verify the install: {e}")),
    }
}

/// Transcribe a short rolling audio buffer for the live-recording transcript
/// pane. Writes the bytes to a temp `.webm`, runs the same local Whisper helper
/// `save_recording` uses, deletes the temp file, and returns the text. When no
/// transcriber is installed this returns an empty string (the frontend shows an
/// install note) rather than hard-erroring.
#[tauri::command]
pub async fn transcribe_partial(app: AppHandle, audio: Vec<u8>, ext: Option<String>) -> Result<String> {
    tauri::async_runtime::spawn_blocking(move || -> Result<String> {
    if audio.is_empty() {
        return Ok(String::new()); // nothing captured yet — not an error mid-recording
    }
    // Prefer the app data dir's recordings folder; fall back to the OS temp dir.
    let dir = app
        .path()
        .app_data_dir()
        .map(|d| d.join("recordings"))
        .unwrap_or_else(|_| std::env::temp_dir());
    std::fs::create_dir_all(&dir)?;
    let ext = sanitize_ext(ext.as_deref());
    let file = dir.join(format!("partial-{}.{ext}", crate::db::new_id()));
    std::fs::write(&file, &audio)?;

    let remote = whisper_remote_url(&app.state::<AppState>());
    let (transcript, _warning) = transcribe(&file, &app.path().app_data_dir().unwrap_or_else(|_| std::env::temp_dir()), false, remote.as_deref(), &whisper_model(&app.state::<AppState>()));
    let _ = std::fs::remove_file(&file);
    Ok(transcript)
    })
    .await
    .map_err(|e| Error::Other(format!("background task failed: {e}")))?
}

/// Save a captured lecture recording: write the audio, create an audio source,
/// transcribe it, then chunk + embed the transcript like any other source.
#[tauri::command]
pub async fn save_recording(
    app: AppHandle,
    subject_id: String,
    topic_id: Option<String>,
    name: String,
    audio: Vec<u8>,
    ext: Option<String>,
) -> Result<IngestResult> {
    tauri::async_runtime::spawn_blocking(move || -> Result<IngestResult> {
    if audio.is_empty() {
        return Err(Error::Other(
            "the recording is empty — the microphone produced no audio (check the \
             input device in your system sound settings), so there is nothing to save"
                .into(),
        ));
    }
    let state = app.state::<AppState>();
    // 1. persist the audio file (keep the real container as the extension)
    let dir = app.path().app_data_dir().map_err(|e| Error::Other(e.to_string()))?.join("recordings");
    std::fs::create_dir_all(&dir)?;
    let ext = sanitize_ext(ext.as_deref());
    let file = dir.join(format!("{}.{ext}", crate::db::new_id()));
    std::fs::write(&file, &audio)?;

    // 2. create the source row
    let source_id = {
        let c = state.db.lock().unwrap();
        repo::insert_source(
            &c,
            &subject_id,
            topic_id.as_deref(),
            &name,
            "audio",
            file.to_str(),
        )?
    };
    emit_progress(&app, &source_id, "parsing", "transcribing audio", 25);

    // 3. transcribe (homelab Whisper if configured, else local)
    let remote = whisper_remote_url(&state);
    let (transcript, warning) = transcribe(&file, &app.path().app_data_dir().unwrap_or_else(|_| std::env::temp_dir()), true, remote.as_deref(), &whisper_model(&state));

    if transcript.trim().is_empty() {
        let c = state.db.lock().unwrap();
        repo::finalize_source(&c, &source_id, "draft", warning.as_deref(), None, warning.as_deref())?;
        emit_progress(&app, &source_id, "done", "saved (no transcript)", 100);
        let source = repo::get_source(&c, &source_id)?;
        return Ok(IngestResult { source, chunk_count: 0, chars: 0, warning });
    }

    // 4. chunk + embed the transcript
    emit_progress(&app, &source_id, "chunking", "splitting transcript", 55);
    let chunks = ingest::chunk_text(&transcript, 900, 150);
    let (embed_provider, gemini_key, ollama_url) = {
        let c = state.db.lock().unwrap();
        (
            effective_embed_provider(&c),
            repo::get_setting(&c, "gemini_api_key")?,
            crate::homelab::resolved_setting(&c, "ollama_url"),
        )
    };
    let embedder = embed::from_settings(&embed_provider, gemini_key.as_deref(), ollama_url.as_deref());
    emit_progress(&app, &source_id, "embedding", &format!("{} chunks", chunks.len()), 75);
    let vectors = ingest::embed_chunks(embedder.as_ref(), &chunks)?;

    emit_progress(&app, &source_id, "storing", "writing vectors", 90);
    let chunk_count = {
        let c = state.db.lock().unwrap();
        for (i, (chunk, vec)) in chunks.iter().zip(vectors.iter()).enumerate() {
            repo::insert_chunk(
                &c, &source_id, &subject_id, topic_id.as_deref(), i as i64, chunk, None,
                vec.len() as i64, &f32s_to_blob(vec),
            )?;
        }
        let n = repo::count_chunks(&c, &source_id)?;
        let meta = format!("{n} chunks · transcribed");
        repo::finalize_source(&c, &source_id, "ready", Some(&meta), Some(&transcript), None)?;
        n
    };
    emit_progress(&app, &source_id, "done", "transcribed", 100);

    // 5. auto-summary: distill the lecture into a note in the background so the
    // user comes back to key points + terms without blocking the save.
    spawn_lecture_summary(app.clone(), subject_id.clone(), topic_id.clone(), name.clone(), transcript.clone());

    let c = state.db.lock().unwrap();
    let source = repo::get_source(&c, &source_id)?;
    Ok(IngestResult {
        source,
        chunk_count,
        chars: transcript.chars().count() as i64,
        warning,
    })
    })
    .await
    .map_err(|e| Error::Other(format!("background task failed: {e}")))?
}

/// Background auto-summary of a transcribed lecture → a note ("Summary — <name>")
/// under the same subject/topic. Best-effort: no model configured, offline mode,
/// or an LLM error just logs — the recording itself already saved fine.
fn spawn_lecture_summary(
    app: AppHandle,
    subject_id: String,
    topic_id: Option<String>,
    name: String,
    transcript: String,
) {
    std::thread::spawn(move || {
        let state = app.state::<AppState>();
        let (spec, keys, offline) = {
            let c = state.db.lock().unwrap();
            let spec = match repo::get_setting(&c, "model_chat") {
                Ok(Some(s)) => s,
                _ => DEFAULT_CHAT_MODEL.into(),
            };
            let keys = match read_keys(&c) {
                Ok(k) => k,
                Err(e) => { eprintln!("[summary] keys unavailable: {e}"); return; }
            };
            let offline = matches!(repo::get_setting(&c, "offline_mode"), Ok(Some(v)) if v == "true");
            (spec, keys, offline)
        };
        if offline && !spec.starts_with("ollama") {
            return; // honor offline mode: only a local model may run
        }
        let Some(mut model) = llm::from_spec_or_any(&spec, &keys) else { return };
        {
            let c = state.db.lock().unwrap();
            apply_budget(&mut model, &c, "chat");
        }
        // Keep the prompt inside a sane context window.
        let excerpt: String = transcript.chars().take(24_000).collect();
        let system = "You summarize lecture transcripts for a student's study notes. \
                      Be faithful to the transcript; do not invent content.";
        let user = format!(
            "Summarize this lecture transcript as Markdown with exactly these sections:\n\
             ## Key points — 5-10 tight bullets\n\
             ## Terms — each important term with a one-line definition\n\
             ## Open questions — anything the lecturer left unresolved or flagged as exam-relevant (omit the section if none)\n\n\
             Transcript:\n{excerpt}"
        );
        match model.complete(system, &user) {
            Ok(summary) if !summary.trim().is_empty() => {
                let c = state.db.lock().unwrap();
                if let Err(e) = repo::insert_note(
                    &c,
                    Some(&subject_id),
                    topic_id.as_deref(),
                    &format!("Summary — {name}"),
                    summary.trim(),
                ) {
                    eprintln!("[summary] couldn't save note: {e}");
                } else {
                    use tauri::Emitter;
                    let _ = app.emit("note:created", ());
                }
            }
            Ok(_) => {}
            Err(e) => eprintln!("[summary] generation failed: {e}"),
        }
    });
}

// ---- web search (SearXNG) --------------------------------------------

/// Best-effort host extraction from a URL without pulling in a URL crate.
fn host_from_url(url: &str) -> String {
    let after_scheme = url.split_once("://").map(|(_, r)| r).unwrap_or(url);
    let authority = after_scheme
        .split(['/', '?', '#'])
        .next()
        .unwrap_or(after_scheme);
    // strip userinfo and port
    let authority = authority.rsplit('@').next().unwrap_or(authority);
    authority
        .split(':')
        .next()
        .unwrap_or(authority)
        .trim_start_matches("www.")
        .to_string()
}

/// Query a configured SearXNG instance for web results.
#[tauri::command]
pub async fn web_search(
    app: AppHandle,
    query: String,
    categories: Option<String>,
) -> Result<Vec<WebResult>> {
    tauri::async_runtime::spawn_blocking(move || -> Result<Vec<WebResult>> {
    let state = app.state::<AppState>();
    let base = {
        let c = state.db.lock().unwrap();
        if offline_mode(&c) {
            return Err(Error::Other(
                "Offline mode is on — web search is disabled. Turn it off in Settings → Data & privacy.".into(),
            ));
        }
        searxng_base(&c)?
    }
    .ok_or_else(|| Error::Other("searxng_url not configured".into()))?;
    let cats = categories.filter(|s| !s.is_empty()).unwrap_or_else(|| "general".into());

    let results = searxng_raw(&base, &query, &cats)?;
    let out = results
        .iter()
        .map(|r| {
            let url = r["url"].as_str().unwrap_or("").to_string();
            WebResult {
                title: r["title"].as_str().unwrap_or("").to_string(),
                host: host_from_url(&url),
                url,
                snippet: r["content"].as_str().unwrap_or("").to_string(),
                engine: r["engine"].as_str().unwrap_or("").to_string(),
                img_src: r["img_src"].as_str().map(normalize_img_url),
                thumbnail: r["thumbnail_src"].as_str().map(normalize_img_url),
            }
        })
        .collect();
    Ok(out)
    })
    .await
    .map_err(|e| Error::Other(format!("background task failed: {e}")))?
}

/// Configured SearXNG base URL (resolved through the homelab fallback chain:
/// local → Tailscale → public), or None if unset.
fn searxng_base(c: &Connection) -> Result<Option<String>> {
    Ok(crate::homelab::resolved_setting(c, "searxng_url"))
}

/// Raw SearXNG JSON `results` array for a query + category.
fn searxng_raw(base: &str, query: &str, category: &str) -> Result<Vec<serde_json::Value>> {
    let client = http_client(15);
    let resp = client
        .get(format!("{base}/search"))
        .query(&[
            ("q", query),
            ("format", "json"),
            ("categories", category),
            ("pageno", "1"),
        ])
        .send()?;
    if !resp.status().is_success() {
        return Err(Error::Other(format!("searxng returned {}", resp.status())));
    }
    let json: serde_json::Value = resp.json()?;
    Ok(json["results"].as_array().cloned().unwrap_or_default())
}

/// Top image results for a query (only entries with a usable image URL).
fn searxng_images(base: &str, query: &str, limit: usize) -> Vec<WebImage> {
    searxng_raw(base, query, "images")
        .unwrap_or_default()
        .iter()
        .filter_map(|r| {
            let img = normalize_img_url(r["img_src"].as_str().filter(|s| !s.is_empty())?);
            let thumb = r["thumbnail_src"]
                .as_str()
                .filter(|s| !s.is_empty())
                .map(normalize_img_url)
                .unwrap_or_else(|| img.clone());
            Some(WebImage {
                img,
                thumb,
                title: r["title"].as_str().unwrap_or("").to_string(),
                source: r["url"].as_str().unwrap_or("").to_string(),
            })
        })
        .take(limit)
        .collect()
}

/// Normalize a SearXNG image URL: protocol-relative `//host/…` → `https://…`.
fn normalize_img_url(u: &str) -> String {
    let u = u.trim();
    if let Some(rest) = u.strip_prefix("//") {
        format!("https://{rest}")
    } else {
        u.to_string()
    }
}

/// Heuristic: does this question want a visual (diagram/image/figure)?
fn wants_images(query: &str) -> bool {
    let q = query.to_lowercase();
    const CUES: &[&str] = &[
        "diagram", "image", "picture", "photo", "visual", "illustrat", "figure",
        "graph", "chart", "map", "sketch", "drawing", "anatomy", "structure of",
        "what does", "look like", "show me", "label", "cross-section", "schematic",
    ];
    CUES.iter().any(|c| q.contains(c))
}

// ---- long-term memory -------------------------------------------------

#[tauri::command]
pub fn add_memory(state: State<AppState>, content: String) -> Result<Memory> {
    let c = state.db.lock().unwrap();
    repo::insert_memory(&c, &content, None)
}

#[tauri::command]
pub fn list_memory(state: State<AppState>) -> Result<Vec<Memory>> {
    let c = state.db.lock().unwrap();
    repo::list_memory(&c)
}

#[tauri::command]
pub fn delete_memory(state: State<AppState>, id: String) -> Result<()> {
    let c = state.db.lock().unwrap();
    repo::delete_memory(&c, &id)
}

// ---- custom music stations --------------------------------------------

#[tauri::command]
pub fn list_custom_stations(state: State<AppState>) -> Result<Vec<CustomStation>> {
    let c = state.db.lock().unwrap();
    repo::list_custom_stations(&c)
}

#[tauri::command]
pub fn add_custom_station(
    state: State<AppState>,
    name: String,
    url: String,
    kind: Option<String>,
) -> Result<CustomStation> {
    let c = state.db.lock().unwrap();
    repo::insert_custom_station(&c, &name, &url, kind.as_deref().unwrap_or("youtube"))
}

#[tauri::command]
pub fn delete_custom_station(state: State<AppState>, id: String) -> Result<()> {
    let c = state.db.lock().unwrap();
    repo::delete_custom_station(&c, &id)
}

#[tauri::command]
pub fn reorder_custom_stations(state: State<AppState>, ids: Vec<String>) -> Result<()> {
    let c = state.db.lock().unwrap();
    repo::reorder_custom_stations(&c, &ids)
}

// ---- data maintenance -------------------------------------------------

#[tauri::command]
pub fn db_stats(app: AppHandle, state: State<AppState>) -> Result<DbStats> {
    let (subjects, sources, chunks) = {
        let c = state.db.lock().unwrap();
        repo::content_counts(&c)
    };
    let db_path = app
        .path()
        .app_data_dir()
        .map_err(|e| Error::Other(e.to_string()))?
        .join("cortex.db");
    let db_bytes = std::fs::metadata(&db_path).map(|m| m.len() as i64).unwrap_or(0);
    Ok(DbStats {
        db_bytes,
        subjects,
        sources,
        chunks,
    })
}

#[tauri::command]
pub fn delete_all_data(app: AppHandle, state: State<AppState>) -> Result<()> {
    {
        let c = state.db.lock().unwrap();
        repo::delete_all_content(&c)?;
    }
    // Remove persisted files (originals + recordings), keep the dirs.
    if let Ok(dir) = app.path().app_data_dir() {
        for sub in ["sources", "recordings"] {
            let _ = std::fs::remove_dir_all(dir.join(sub));
        }
    }
    Ok(())
}

/// The user's current Omarchy theme name (e.g. "tokyo-night"), read from
/// `~/.config/omarchy/current/theme.name`. Returns None when Omarchy isn't
/// installed so the "Follow Omarchy theme" toggle can degrade gracefully.
#[tauri::command]
pub fn omarchy_theme() -> Option<String> {
    let home = std::path::PathBuf::from(std::env::var_os("HOME")?);
    let name_file = home.join(".config/omarchy/current/theme.name");
    if let Ok(s) = std::fs::read_to_string(&name_file) {
        let t = s.trim().to_string();
        if !t.is_empty() {
            return Some(t);
        }
    }
    // Fall back to the basename of the `current/theme` symlink target.
    let link = home.join(".config/omarchy/current/theme");
    let target = std::fs::read_link(&link).ok()?;
    target
        .file_name()
        .and_then(|s| s.to_str())
        .map(|s| s.to_string())
}

/// Reachability check for the homelab "Test connection" button.
#[tauri::command]
pub async fn ping_url(url: String) -> Result<bool> {
    tauri::async_runtime::spawn_blocking(move || -> Result<bool> {
        let client = http_client(5);
        match client.get(&url).send() {
            // ANY HTTP response means the server is reachable — a reverse-proxy root
            // commonly answers 404/401/403, which is still "connected". (Matches
            // homelab::reachable(); the old is_success() check reported those as failures.)
            Ok(_) => Ok(true),
            // Only a transport failure (DNS/timeout/TLS/connection refused) is unreachable —
            // surface its message so the "Test connection" button explains WHY it failed.
            Err(e) => Err(Error::Other(format!("{e}"))),
        }
    })
    .await
    .map_err(|e| Error::Other(format!("background task failed: {e}")))?
}

// ---- in-app page fetch (reader-mode browsing inside Web search) --------

/// A fetched web page, reduced to readable content for in-app display. No
/// JavaScript executes — this is a safe reader view, not a live webview.
#[derive(serde::Serialize)]
pub struct FetchedPage {
    pub url: String,
    pub final_url: String,
    pub title: String,
    pub text: String,
    pub links: Vec<PageLink>,
}

#[derive(serde::Serialize)]
pub struct PageLink {
    pub href: String,
    pub text: String,
}

/// Fetch a URL and return its readable text + outbound links so the Web search
/// view can browse pages in-app (no separate window, no SearXNG needed). Runs
/// off-thread via spawn_blocking (reqwest::blocking).
#[tauri::command]
pub async fn fetch_page(url: String) -> Result<FetchedPage> {
    tauri::async_runtime::spawn_blocking(move || -> Result<FetchedPage> {
        let url = url.trim().to_string();
        let client = http_client(20);
        let resp = client
            .get(&url)
            .header(
                "User-Agent",
                "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) \
                 Chrome/124.0 Safari/537.36",
            )
            .header("Accept", "text/html,application/xhtml+xml")
            .send()?;
        if !resp.status().is_success() {
            return Err(Error::Other(format!("fetch failed: HTTP {}", resp.status())));
        }
        let final_url = resp.url().to_string();
        let html = resp.text()?;
        let (title, text, links) = ingest::readable_page(&html, &final_url);
        Ok(FetchedPage {
            url,
            final_url,
            title,
            text,
            links: links
                .into_iter()
                .map(|(href, text)| PageLink { href, text })
                .collect(),
        })
    })
    .await
    .map_err(|e| Error::Other(format!("background task failed: {e}")))?
}

/// Lightweight environment probe for the Settings screen (later slice).
#[tauri::command]
pub fn env_probe() -> Result<serde_json::Value> {
    fn has(cmd: &str) -> bool {
        std::env::var_os("PATH")
            .map(|p| {
                std::env::split_paths(&p).any(|d| d.join(cmd).is_file())
            })
            .unwrap_or(false)
    }
    Ok(serde_json::json!({
        "libreoffice": has("libreoffice") || has("soffice"),
        "ffmpeg": has("ffmpeg"),
        "ollama": has("ollama"),
        "whisper": has("whisper"),
        "yt_dlp": has("yt-dlp"),
    }))
}
