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

/// Read all configured provider keys from settings.
fn read_keys(c: &Connection) -> Result<llm::Keys> {
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
    })
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

// ---- topics ------------------------------------------------------------

#[tauri::command]
pub fn create_topic(
    state: State<AppState>,
    subject_id: String,
    name: String,
    glyph: Option<String>,
) -> Result<Subject> {
    let c = state.db.lock().unwrap();
    repo::insert_topic(&c, &subject_id, &name, glyph.as_deref())?;
    repo::get_subject(&c, &subject_id)
}

#[tauri::command]
pub fn update_topic(
    state: State<AppState>,
    id: String,
    name: String,
    subject_id: String,
    glyph: Option<String>,
) -> Result<Subject> {
    let c = state.db.lock().unwrap();
    repo::update_topic(&c, &id, &name, glyph.as_deref())?;
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

    // 2b. pptx/docx: render a PDF for inline slide preview, overriding the
    //     stored original-bytes path so the frontend shows PDF pages. A failed
    //     render is a hard error (don't silently ship a half-ingested source).
    if matches!(kind.as_str(), "pptx" | "docx") {
        if let Some(src_path) = input.path.as_deref() {
            emit_progress(&app, &source_id, "parsing", "rendering slides to PDF", 25);
            let pdf_dest = sources_dir.join(format!("{source_id}.pdf"));
            if let Err(e) = ingest::libreoffice_to_pdf(src_path, &pdf_dest) {
                let c = state.db.lock().unwrap();
                let _ = repo::finalize_source(&c, &source_id, "error", None, None, Some(&e.to_string()));
                emit_progress(&app, &source_id, "error", &e.to_string(), 100);
                return Err(e);
            }
            if let Some(p) = pdf_dest.to_str() {
                let c = state.db.lock().unwrap();
                repo::set_stored_path(&c, &source_id, p)?;
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
            repo::get_setting(&c, "embed_provider")?.unwrap_or_else(|| "stub".into()),
            repo::get_setting(&c, "gemini_api_key")?,
            repo::get_setting(&c, "ollama_url")?,
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
            repo::get_setting(&c, "embed_provider")?.unwrap_or_else(|| "stub".into()),
            repo::get_setting(&c, "gemini_api_key")?,
            repo::get_setting(&c, "ollama_url")?,
        )
    };
    let embedder = embed::from_settings(&provider, gemini_key.as_deref(), ollama_url.as_deref());
    let qvec = embedder
        .embed(&[query])
        .map(|mut v| v.pop().unwrap_or_default())?;
    let c = state.db.lock().unwrap();
    repo::search_chunks(&c, subject_id.as_deref(), &qvec, k.unwrap_or(8))
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
            let tid = repo::insert_topic(&c, &sid, tname, None)?;
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

/// Scoped retrieval-augmented chat. Embeds the query, retrieves top-k chunks
/// (optionally narrowed to a single source), and asks the configured LLM to
/// answer from that context with inline ⟦source · loc⟧ citations.
#[tauri::command]
pub async fn chat_answer(
    app: AppHandle,
    subject_id: String,
    level: String,
    source_id: Option<String>,
    query: String,
) -> Result<ChatAnswer> {
    tauri::async_runtime::spawn_blocking(move || -> Result<ChatAnswer> {
    let state = app.state::<AppState>();
    let (embed_provider, ollama_url, chat_spec, keys, preamble) = {
        let c = state.db.lock().unwrap();
        (
            repo::get_setting(&c, "embed_provider")?.unwrap_or_else(|| "stub".into()),
            repo::get_setting(&c, "ollama_url")?,
            repo::get_setting(&c, "model_chat")?.unwrap_or_else(|| "gemini:gemini-2.5-flash".into()),
            read_keys(&c)?,
            profile_preamble(&c)?,
        )
    };
    // Require a real model before doing any work.
    let model = llm::from_spec_or_any(&chat_spec, &keys).ok_or_else(|| Error::Other(NO_MODEL.into()))?;

    // Scope: source-level chats are restricted to a single source's chunks; the
    // keyword path applies this in SQL, the vector path filters its results.
    let scoped_source = if level == "source" { source_id.as_deref() } else { None };

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
    hits.truncate(6);

    let context = hits
        .iter()
        .map(|h| {
            let loc = h.loc.as_deref().map(|l| format!(" · {l}")).unwrap_or_default();
            format!("[{}{}]\n{}", h.source_name, loc, h.text)
        })
        .collect::<Vec<_>>()
        .join("\n\n");

    let base_system = "You are Cortex, a study tutor. Be CONCISE — answer in short, focused chunks \
        (usually 2–5 sentences or a few short bullets), never an essay. Lead with the key idea, then \
        actively promote learning by ending the answer with ONE short guiding question. \
        Ground answers in the provided source context and CITE sources inline as ⟦source-name · location⟧ \
        whenever you use them. If the sources don't cover it, say so in one line, then explain briefly from \
        general knowledge. Use light Markdown (bold key terms, short bullet lists; a `---` divider only when \
        genuinely needed) and keep it scannable. \
        On the FINAL line output exactly: `SUGGESTIONS: a | b | c` — 2–3 very short next-step prompts the \
        learner could tap next (e.g. a deeper follow-up, or moving on to the next logical topic). Do not \
        otherwise mention the suggestions line.";
    let system = if preamble.is_empty() {
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
    let system = system.as_str();
    let user = if context.is_empty() {
        format!("(No indexed sources are in scope yet.)\n\nQUESTION: {query}")
    } else {
        format!("SOURCE CONTEXT:\n{context}\n\nQUESTION: {query}")
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
    })
    })
    .await
    .map_err(|e| Error::Other(format!("background task failed: {e}")))?
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
                        Some(CsSection {
                            id: slug(&title),
                            title,
                            state: "approved".into(),
                            items,
                        })
                    })
                    .collect();
            }
            Vec::new()
        }
        Err(_) => Vec::new(),
    }
}

/// Synthesize a sectioned cheatsheet from a subject/topic's indexed sources.
#[tauri::command]
pub async fn generate_cheatsheet(
    app: AppHandle,
    subject_id: String,
    topic_id: Option<String>,
) -> Result<CheatsheetData> {
    tauri::async_runtime::spawn_blocking(move || -> Result<CheatsheetData> {
    let state = app.state::<AppState>();
    let (context, sources, subject_name, topic_name, spec, keys, style) = {
        let c = state.db.lock().unwrap();
        let (ctx, n) = repo::context_text(&c, &subject_id, topic_id.as_deref(), 24000)?;
        let subj = repo::get_subject(&c, &subject_id)?;
        let tname = topic_id
            .as_ref()
            .and_then(|tid| subj.topics.iter().find(|t| &t.id == tid))
            .or_else(|| subj.topics.first())
            .map(|t| t.name.clone())
            .unwrap_or_default();
        let spec =
            repo::get_setting(&c, "model_cheatsheet")?.unwrap_or_else(|| "gemini:gemini-2.5-pro".into());
        (ctx, n, subj.name, tname, spec, read_keys(&c)?, style_instruction(&c))
    };
    let model = llm::from_spec_or_any(&spec, &keys).ok_or_else(|| Error::Other(NO_MODEL.into()))?;
    if context.trim().is_empty() {
        return Err(Error::Other(
            "No source text to synthesize from — add and ingest a source first.".into(),
        ));
    }

    let system = format!("You are an expert study-notes synthesizer for a university student. Produce a \
        COMPLETE, accurate cheatsheet from the source material — completeness matters more than \
        brevity; do not drop key points. Output ONLY valid JSON, no prose, in this exact shape: \
        {{\"sections\":[{{\"title\":string,\"items\":[{{\"t\":\"term\",\"d\":\"explanation\"}}]}}]}}. \
        Use exactly these sections in this order: Definitions, Key Concepts, Formulas, Worked \
        Examples, Common Pitfalls, Quick Recall.{style}");
    let system = system.as_str();
    let user = format!("Subject: {subject_name} › {topic_name}\n\nSOURCE MATERIAL:\n{context}\n\nProduce the cheatsheet JSON now.");

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
        }];
    }

    {
        let c = state.db.lock().unwrap();
        repo::save_cheatsheet(&c, &subject_id, topic_id.as_deref(), &sections)?;
    }

    Ok(CheatsheetData {
        subject: subject_name,
        topic: topic_name,
        sources,
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
        .or_else(|| subj.topics.first())
        .map(|t| t.name.clone())
        .unwrap_or_default();
    let (_, n) = repo::context_text(&c, &subject_id, topic_id.as_deref(), 1)?;
    Ok(Some(CheatsheetData {
        subject: subj.name,
        topic: tname,
        sources: n,
        model: "stored".into(),
        sections,
    }))
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
) -> Result<MaterialRec> {
    tauri::async_runtime::spawn_blocking(move || -> Result<MaterialRec> {
    let state = app.state::<AppState>();
    let setting_key = match kind.as_str() {
        "quiz" => "model_quiz",
        "audio" => "model_audio",
        "flashcards" => "model_flashcard",
        _ => "model_cheatsheet",
    };
    let (context, subject_name, topic_name, spec, keys, style) = {
        let c = state.db.lock().unwrap();
        let (ctx, _) = repo::context_text(&c, &subject_id, topic_id.as_deref(), 18000)?;
        let subj = repo::get_subject(&c, &subject_id)?;
        let tname = topic_id
            .as_ref()
            .and_then(|tid| subj.topics.iter().find(|t| &t.id == tid))
            .or_else(|| subj.topics.first())
            .map(|t| t.name.clone())
            .unwrap_or_default();
        let spec = repo::get_setting(&c, setting_key)?
            .unwrap_or_else(|| "gemini:gemini-2.5-flash".into());
        (ctx, subj.name, tname, spec, read_keys(&c)?, style_instruction(&c))
    };
    let model = llm::from_spec_or_any(&spec, &keys).ok_or_else(|| Error::Other(NO_MODEL.into()))?;
    if context.trim().is_empty() {
        return Err(Error::Other(
            "No source text to generate from — add and ingest a source first.".into(),
        ));
    }

    // Per-kind prompt + payload shape.
    let (system, default_title) = match kind.as_str() {
        "quiz" => (
            "You generate quiz questions from study material. Output ONLY a JSON array of 8-10 \
             items, each: {\"q\":\"question\",\"options\":[\"a\",\"b\",\"c\",\"d\"],\"answer\":<index 0-3>,\"explain\":\"why\"}. No prose.".to_string(),
            format!("{topic_name} quiz"),
        ),
        "audio" => (
            "You write a two-host podcast-style audio overview script from study material. Output \
             ONLY JSON: {\"segments\":[{\"speaker\":\"Maya\"|\"Theo\",\"text\":\"...\"}]}. 12-20 lively, \
             accurate segments that teach the material conversationally. No prose outside JSON.".to_string(),
            format!("{topic_name} — audio overview"),
        ),
        "infographic" => (
            "You produce a clean, self-contained SVG infographic (max 900x1200, dark background \
             #111c18, accent #2dd5b7, legible text) summarizing the study material as a poster. \
             Output ONLY JSON: {\"svg\":\"<svg ...>...</svg>\"}. No prose outside JSON.".to_string(),
            format!("{topic_name} — infographic"),
        ),
        "slideshow" => (
            "You produce a slideshow outline from study material. Output ONLY JSON: \
             {\"slides\":[{\"title\":\"...\",\"bullets\":[\"...\"],\"notes\":\"voiceover\"}]}. 8-12 slides. No prose outside JSON.".to_string(),
            format!("{topic_name} — slideshow"),
        ),
        _ => (
            "You generate study flashcards from material. Output ONLY a JSON array of 12-18 items, \
             each: {\"q\":\"front/question\",\"a\":\"back/answer\"}. No prose.".to_string(),
            format!("{topic_name} flashcards"),
        ),
    };
    let system = format!("{system}{style}");
    let user = format!("Subject: {subject_name} › {topic_name}\n\nSOURCE MATERIAL:\n{context}\n\nGenerate now.");

    let raw = model.complete(&system, &user)?;
    let payload = llm::extract_json(&raw)
        .map_err(|_| Error::Other("model returned unstructured output; try again".into()))?;
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
        "infographic" => "SVG poster".to_string(),
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

#[tauri::command]
pub fn list_materials(state: State<AppState>, subject_id: String) -> Result<Vec<MaterialRec>> {
    let c = state.db.lock().unwrap();
    repo::list_materials(&c, &subject_id)
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
fn transcribe(file: &Path) -> (String, Option<String>) {
    use std::process::Command;
    let outdir = std::env::temp_dir().join(format!("cortex-asr-{}", crate::db::new_id()));
    let _ = std::fs::create_dir_all(&outdir);

    // openai-whisper. Fall back to ~/.local/bin/whisper (pip --user install dir)
    // in case the app was launched without it on PATH (desktop launchers often
    // don't inherit the user's shell PATH).
    let whisper_bin = ingest::which("whisper").or_else(|| {
        std::env::var("HOME").ok().and_then(|h| {
            let p = std::path::Path::new(&h).join(".local/bin/whisper");
            if p.exists() { Some(p.to_string_lossy().into_owned()) } else { None }
        })
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
        let converted = ingest::which("ffmpeg").is_some()
            && Command::new("ffmpeg")
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
    let _ = std::fs::remove_dir_all(&outdir);
    (
        String::new(),
        Some(
            "Recording saved, but no Whisper transcriber was found. Install `openai-whisper` \
             (pip install openai-whisper) or whisper.cpp, then re-record to get a transcript."
                .into(),
        ),
    )
}

/// Transcribe a short rolling audio buffer for the live-recording transcript
/// pane. Writes the bytes to a temp `.webm`, runs the same local Whisper helper
/// `save_recording` uses, deletes the temp file, and returns the text. When no
/// transcriber is installed this returns an empty string (the frontend shows an
/// install note) rather than hard-erroring.
#[tauri::command]
pub async fn transcribe_partial(app: AppHandle, audio: Vec<u8>) -> Result<String> {
    tauri::async_runtime::spawn_blocking(move || -> Result<String> {
    // Prefer the app data dir's recordings folder; fall back to the OS temp dir.
    let dir = app
        .path()
        .app_data_dir()
        .map(|d| d.join("recordings"))
        .unwrap_or_else(|_| std::env::temp_dir());
    std::fs::create_dir_all(&dir)?;
    let file = dir.join(format!("partial-{}.webm", crate::db::new_id()));
    std::fs::write(&file, &audio)?;

    let (transcript, _warning) = transcribe(&file);
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
) -> Result<IngestResult> {
    tauri::async_runtime::spawn_blocking(move || -> Result<IngestResult> {
    let state = app.state::<AppState>();
    // 1. persist the audio file
    let dir = app.path().app_data_dir().map_err(|e| Error::Other(e.to_string()))?.join("recordings");
    std::fs::create_dir_all(&dir)?;
    let file = dir.join(format!("{}.webm", crate::db::new_id()));
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

    // 3. transcribe (local Whisper if available)
    let (transcript, warning) = transcribe(&file);

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
            repo::get_setting(&c, "embed_provider")?.unwrap_or_else(|| "stub".into()),
            repo::get_setting(&c, "gemini_api_key")?,
            repo::get_setting(&c, "ollama_url")?,
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
        repo::get_setting(&c, "searxng_url")?
    };
    let base = base
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| Error::Other("searxng_url not configured".into()))?;
    let base = base.trim_end_matches('/');
    let cats = categories.filter(|s| !s.is_empty()).unwrap_or_else(|| "general".into());

    let client = http_client(15);
    let resp = client
        .get(format!("{base}/search"))
        .query(&[
            ("q", query.as_str()),
            ("format", "json"),
            ("categories", cats.as_str()),
            ("pageno", "1"),
        ])
        .send()?;
    if !resp.status().is_success() {
        return Err(Error::Other(format!("searxng returned {}", resp.status())));
    }
    let json: serde_json::Value = resp.json()?;
    let results = json["results"].as_array().cloned().unwrap_or_default();
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
            }
        })
        .collect();
    Ok(out)
    })
    .await
    .map_err(|e| Error::Other(format!("background task failed: {e}")))?
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

/// Reachability check for the homelab "Test connection" button.
#[tauri::command]
pub async fn ping_url(url: String) -> Result<bool> {
    tauri::async_runtime::spawn_blocking(move || -> Result<bool> {
        let client = http_client(5);
        match client.get(&url).send() {
            Ok(resp) => Ok(resp.status().is_success()),
            Err(_) => Ok(false),
        }
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
