//! Data-access layer over rusqlite. Pure functions taking `&Connection` so they
//! are trivially unit-testable against an in-memory DB.

use crate::db::{new_id, now_ms};
use crate::error::{Error, Result};
use crate::models::*;
use crate::vector::{blob_to_f32s, cosine};
use rusqlite::{params, Connection, OptionalExtension};

// ---- subjects ----------------------------------------------------------

pub fn insert_subject(
    conn: &Connection,
    name: &str,
    code: Option<&str>,
    glyph: Option<&str>,
    color: Option<&str>,
) -> Result<String> {
    let id = new_id();
    let ts = now_ms();
    let pos: i64 = conn
        .query_row("SELECT COALESCE(MAX(position),-1)+1 FROM subjects", [], |r| {
            r.get(0)
        })
        .unwrap_or(0);
    conn.execute(
        "INSERT INTO subjects (id, name, code, glyph, color, status, streak, position, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, 'ready', 0, ?6, ?7, ?7)",
        params![id, name, code, glyph.unwrap_or("◆"), color, pos, ts],
    )?;
    Ok(id)
}

pub fn update_subject(
    conn: &Connection,
    id: &str,
    name: &str,
    code: Option<&str>,
    glyph: Option<&str>,
    color: Option<&str>,
) -> Result<()> {
    let n = conn.execute(
        "UPDATE subjects SET
            name=?2,
            code=?3,
            glyph=COALESCE(?4, glyph),
            color=CASE WHEN ?5 IS NULL THEN color ELSE ?5 END,
            updated_at=?6
         WHERE id=?1",
        params![id, name, code, glyph, color, now_ms()],
    )?;
    if n == 0 {
        return Err(Error::NotFound(format!("subject {id}")));
    }
    Ok(())
}

pub fn delete_subject(conn: &Connection, id: &str) -> Result<()> {
    conn.execute("DELETE FROM subjects WHERE id=?1", params![id])?;
    Ok(())
}

/// Full Subjects → Topics → Sources tree (what the sidebar + dashboard render).
pub fn list_subjects(conn: &Connection) -> Result<Vec<Subject>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, code, glyph, color, status, streak, position, created_at, updated_at
         FROM subjects ORDER BY position, created_at",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok(Subject {
            id: r.get(0)?,
            name: r.get(1)?,
            code: r.get(2)?,
            glyph: r.get(3)?,
            color: r.get(4)?,
            status: r.get(5)?,
            streak: r.get(6)?,
            position: r.get(7)?,
            source_count: 0,
            topics: Vec::new(),
            created_at: r.get(8)?,
            updated_at: r.get(9)?,
        })
    })?;
    let mut subjects: Vec<Subject> = rows.collect::<rusqlite::Result<_>>()?;
    for s in &mut subjects {
        s.topics = list_topics(conn, &s.id)?;
        s.source_count = conn.query_row(
            "SELECT count(*) FROM sources WHERE subject_id=?1",
            params![s.id],
            |r| r.get(0),
        )?;
    }
    Ok(subjects)
}

pub fn get_subject(conn: &Connection, id: &str) -> Result<Subject> {
    list_subjects(conn)?
        .into_iter()
        .find(|s| s.id == id)
        .ok_or_else(|| Error::NotFound(format!("subject {id}")))
}

// ---- topics ------------------------------------------------------------

pub fn insert_topic(
    conn: &Connection,
    subject_id: &str,
    name: &str,
    glyph: Option<&str>,
) -> Result<String> {
    let id = new_id();
    let ts = now_ms();
    let pos: i64 = conn
        .query_row(
            "SELECT COALESCE(MAX(position),-1)+1 FROM topics WHERE subject_id=?1",
            params![subject_id],
            |r| r.get(0),
        )
        .unwrap_or(0);
    conn.execute(
        "INSERT INTO topics (id, subject_id, name, glyph, position, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)",
        params![id, subject_id, name, glyph, pos, ts],
    )?;
    Ok(id)
}

pub fn update_topic(conn: &Connection, id: &str, name: &str, glyph: Option<&str>) -> Result<()> {
    let n = conn.execute(
        "UPDATE topics SET
            name=?2,
            glyph=CASE WHEN ?3 IS NULL THEN glyph ELSE ?3 END,
            updated_at=?4
         WHERE id=?1",
        params![id, name, glyph, now_ms()],
    )?;
    if n == 0 {
        return Err(Error::NotFound(format!("topic {id}")));
    }
    Ok(())
}

pub fn delete_topic(conn: &Connection, id: &str) -> Result<()> {
    conn.execute("DELETE FROM topics WHERE id=?1", params![id])?;
    Ok(())
}

pub fn list_topics(conn: &Connection, subject_id: &str) -> Result<Vec<Topic>> {
    let mut stmt = conn.prepare(
        "SELECT id, subject_id, name, glyph, position FROM topics
         WHERE subject_id=?1 ORDER BY position, created_at",
    )?;
    let rows = stmt.query_map(params![subject_id], |r| {
        Ok(Topic {
            id: r.get(0)?,
            subject_id: r.get(1)?,
            name: r.get(2)?,
            glyph: r.get(3)?,
            position: r.get(4)?,
            sources: Vec::new(),
        })
    })?;
    let mut topics: Vec<Topic> = rows.collect::<rusqlite::Result<_>>()?;
    for t in &mut topics {
        t.sources = list_sources_for_topic(conn, &t.id)?;
    }
    Ok(topics)
}

// ---- sources -----------------------------------------------------------

fn map_source(r: &rusqlite::Row) -> rusqlite::Result<Source> {
    Ok(Source {
        id: r.get(0)?,
        subject_id: r.get(1)?,
        topic_id: r.get(2)?,
        name: r.get(3)?,
        kind: r.get(4)?,
        status: r.get(5)?,
        meta: r.get(6)?,
        origin: r.get(7)?,
        error: r.get(8)?,
        content: r.get(9)?,
        stored_path: r.get(10)?,
        tags: Vec::new(),
        created_at: r.get(11)?,
        updated_at: r.get(12)?,
    })
}

const SOURCE_COLS: &str =
    "id, subject_id, topic_id, name, kind, status, meta, origin, error, content, stored_path, created_at, updated_at";

pub fn insert_source(
    conn: &Connection,
    subject_id: &str,
    topic_id: Option<&str>,
    name: &str,
    kind: &str,
    origin: Option<&str>,
) -> Result<String> {
    let id = new_id();
    let ts = now_ms();
    conn.execute(
        "INSERT INTO sources (id, subject_id, topic_id, name, kind, status, origin, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, 'ingesting', ?6, ?7, ?7)",
        params![id, subject_id, topic_id, name, kind, origin, ts],
    )?;
    Ok(id)
}

pub fn finalize_source(
    conn: &Connection,
    id: &str,
    status: &str,
    meta: Option<&str>,
    content: Option<&str>,
    error: Option<&str>,
) -> Result<()> {
    conn.execute(
        "UPDATE sources SET status=?2, meta=?3, content=?4, error=?5, updated_at=?6 WHERE id=?1",
        params![id, status, meta, content, error, now_ms()],
    )?;
    Ok(())
}

/// Persist the stable on-disk path to a source's original/rendered bytes.
pub fn set_stored_path(conn: &Connection, id: &str, stored_path: &str) -> Result<()> {
    conn.execute(
        "UPDATE sources SET stored_path=?2, updated_at=?3 WHERE id=?1",
        params![id, stored_path, now_ms()],
    )?;
    Ok(())
}

pub fn get_source(conn: &Connection, id: &str) -> Result<Source> {
    let sql = format!("SELECT {SOURCE_COLS} FROM sources WHERE id=?1");
    let mut src = conn
        .query_row(&sql, params![id], map_source)
        .optional()?
        .ok_or_else(|| Error::NotFound(format!("source {id}")))?;
    src.tags = source_tags(conn, id)?;
    Ok(src)
}

pub fn list_sources(conn: &Connection, subject_id: &str) -> Result<Vec<Source>> {
    let sql = format!(
        "SELECT {SOURCE_COLS} FROM sources WHERE subject_id=?1 ORDER BY created_at DESC"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params![subject_id], map_source)?;
    let mut out: Vec<Source> = rows.collect::<rusqlite::Result<_>>()?;
    for s in &mut out {
        s.tags = source_tags(conn, &s.id)?;
    }
    Ok(out)
}

fn list_sources_for_topic(conn: &Connection, topic_id: &str) -> Result<Vec<Source>> {
    let sql = format!(
        "SELECT {SOURCE_COLS} FROM sources WHERE topic_id=?1 ORDER BY created_at"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params![topic_id], map_source)?;
    let mut out: Vec<Source> = rows.collect::<rusqlite::Result<_>>()?;
    for s in &mut out {
        s.tags = source_tags(conn, &s.id)?;
    }
    Ok(out)
}

/// Rename/re-file a source and replace its tag set. Tags live in the
/// `source_tags` join table (same as `attach_tags`), so we clear the existing
/// links and re-attach the provided list.
pub fn update_source(
    conn: &Connection,
    id: &str,
    name: &str,
    topic_id: Option<&str>,
    tags: &[String],
) -> Result<()> {
    let n = conn.execute(
        "UPDATE sources SET name=?2, topic_id=?3, updated_at=?4 WHERE id=?1",
        params![id, name, topic_id, now_ms()],
    )?;
    if n == 0 {
        return Err(Error::NotFound(format!("source {id}")));
    }
    conn.execute("DELETE FROM source_tags WHERE source_id=?1", params![id])?;
    attach_tags(conn, id, tags)?;
    Ok(())
}

pub fn delete_source(conn: &Connection, id: &str) -> Result<()> {
    conn.execute("DELETE FROM sources WHERE id=?1", params![id])?;
    Ok(())
}

// ---- tags --------------------------------------------------------------

pub fn attach_tags(conn: &Connection, source_id: &str, tags: &[String]) -> Result<()> {
    for tag in tags {
        let tag = tag.trim();
        if tag.is_empty() {
            continue;
        }
        let tag_id: String = match conn
            .query_row("SELECT id FROM tags WHERE name=?1", params![tag], |r| r.get(0))
            .optional()?
        {
            Some(id) => id,
            None => {
                let id = new_id();
                conn.execute(
                    "INSERT INTO tags (id, name) VALUES (?1, ?2)",
                    params![id, tag],
                )?;
                id
            }
        };
        conn.execute(
            "INSERT OR IGNORE INTO source_tags (source_id, tag_id) VALUES (?1, ?2)",
            params![source_id, tag_id],
        )?;
    }
    Ok(())
}

pub fn source_tags(conn: &Connection, source_id: &str) -> Result<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT t.name FROM tags t JOIN source_tags st ON st.tag_id=t.id
         WHERE st.source_id=?1 ORDER BY t.name",
    )?;
    let rows = stmt.query_map(params![source_id], |r| r.get::<_, String>(0))?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

// ---- chunks ------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
pub fn insert_chunk(
    conn: &Connection,
    source_id: &str,
    subject_id: &str,
    topic_id: Option<&str>,
    ord: i64,
    text: &str,
    loc: Option<&str>,
    dim: i64,
    embedding: &[u8],
) -> Result<()> {
    conn.execute(
        "INSERT INTO chunks (id, source_id, subject_id, topic_id, ord, text, loc, dim, embedding, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![new_id(), source_id, subject_id, topic_id, ord, text, loc, dim, embedding, now_ms()],
    )?;
    Ok(())
}

/// List a source's stored chunks (text + vector dim) — embedding proof for the UI.
pub fn list_chunks(conn: &Connection, source_id: &str) -> Result<Vec<ChunkInfo>> {
    let mut stmt = conn.prepare(
        "SELECT ord, text, dim, loc FROM chunks WHERE source_id=?1 ORDER BY ord",
    )?;
    let rows = stmt.query_map(params![source_id], |r| {
        Ok(ChunkInfo {
            ord: r.get(0)?,
            text: r.get(1)?,
            dim: r.get(2)?,
            loc: r.get(3)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

pub fn count_chunks(conn: &Connection, source_id: &str) -> Result<i64> {
    Ok(conn.query_row(
        "SELECT count(*) FROM chunks WHERE source_id=?1",
        params![source_id],
        |r| r.get(0),
    )?)
}

/// Cosine top-k over stored chunk vectors, optionally scoped to a subject.
/// (sqlite-vec is the locked upgrade path; this is the foundation scan.)
pub fn search_chunks(
    conn: &Connection,
    subject_id: Option<&str>,
    query_vec: &[f32],
    k: usize,
) -> Result<Vec<ChunkHit>> {
    let sql = "SELECT c.id, c.source_id, s.name, c.text, c.loc, c.embedding
               FROM chunks c JOIN sources s ON s.id=c.source_id
               WHERE (?1 IS NULL OR c.subject_id=?1)";
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map(params![subject_id], |r| {
        let blob: Vec<u8> = r.get(5)?;
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, String>(2)?,
            r.get::<_, String>(3)?,
            r.get::<_, Option<String>>(4)?,
            blob,
        ))
    })?;
    let mut hits: Vec<ChunkHit> = Vec::new();
    for row in rows {
        let (id, source_id, source_name, text, loc, blob) = row?;
        let score = cosine(query_vec, &blob_to_f32s(&blob));
        hits.push(ChunkHit {
            id,
            source_id,
            source_name,
            text,
            loc,
            score,
        });
    }
    hits.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    hits.truncate(k);
    Ok(hits)
}

/// Tokenize a query into significant lowercase terms: split on non-alphanumerics,
/// drop very short tokens and a small stopword list. De-duplicated, capped.
fn query_terms(query: &str) -> Vec<String> {
    const STOPWORDS: &[&str] = &[
        "the", "and", "for", "are", "but", "not", "you", "all", "any", "can", "her", "was",
        "one", "our", "out", "day", "get", "has", "him", "his", "how", "man", "new", "now",
        "old", "see", "two", "way", "who", "boy", "did", "its", "let", "put", "say", "she",
        "too", "use", "what", "when", "with", "this", "that", "from", "have", "your", "about",
        "into", "than", "then", "them", "they", "will", "would", "could", "should", "does",
        "doing", "explain", "describe", "tell", "give", "list", "define",
    ];
    let mut seen = std::collections::HashSet::new();
    let mut terms = Vec::new();
    for raw in query.split(|c: char| !c.is_alphanumeric()) {
        let t = raw.trim().to_lowercase();
        if t.len() < 3 || STOPWORDS.contains(&t.as_str()) {
            continue;
        }
        if seen.insert(t.clone()) {
            terms.push(t);
        }
        if terms.len() >= 12 {
            break;
        }
    }
    terms
}

/// Keyword fallback for retrieval when vector search is unreliable (e.g. the
/// "stub" embedder). Scans chunk text with case-insensitive LIKE for each
/// significant query term, scoped identically to vector search (subject, and
/// optionally a single source), and ranks chunks by the count of distinct
/// query terms they contain. Returns the top-`k`.
pub fn keyword_search_chunks(
    conn: &Connection,
    subject_id: Option<&str>,
    source_id: Option<&str>,
    query: &str,
    k: usize,
) -> Result<Vec<ChunkHit>> {
    let terms = query_terms(query);
    if terms.is_empty() {
        return Ok(Vec::new());
    }
    let sql = "SELECT c.id, c.source_id, s.name, c.text, c.loc
               FROM chunks c JOIN sources s ON s.id=c.source_id
               WHERE (?1 IS NULL OR c.subject_id=?1)
                 AND (?2 IS NULL OR c.source_id=?2)";
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map(params![subject_id, source_id], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, String>(2)?,
            r.get::<_, String>(3)?,
            r.get::<_, Option<String>>(4)?,
        ))
    })?;
    let mut hits: Vec<ChunkHit> = Vec::new();
    for row in rows {
        let (id, source_id, source_name, text, loc) = row?;
        let lower = text.to_lowercase();
        let matched = terms.iter().filter(|t| lower.contains(t.as_str())).count();
        if matched == 0 {
            continue;
        }
        hits.push(ChunkHit {
            id,
            source_id,
            source_name,
            text,
            loc,
            // Score = fraction of distinct query terms present, so it is
            // comparable in spirit to a cosine similarity in [0,1].
            score: matched as f32 / terms.len() as f32,
        });
    }
    hits.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    hits.truncate(k);
    Ok(hits)
}

// ---- context gathering (for synthesis) --------------------------------

/// Concatenate chunk text for a subject (optionally a topic), capped to `max_chars`.
pub fn context_text(
    conn: &Connection,
    subject_id: &str,
    topic_id: Option<&str>,
    max_chars: usize,
) -> Result<(String, i64)> {
    let sql = "SELECT c.text FROM chunks c
               WHERE c.subject_id=?1 AND (?2 IS NULL OR c.topic_id=?2)
               ORDER BY c.source_id, c.ord";
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map(params![subject_id, topic_id], |r| r.get::<_, String>(0))?;
    let mut out = String::new();
    for row in rows {
        let t = row?;
        if out.len() + t.len() > max_chars {
            break;
        }
        out.push_str(&t);
        out.push_str("\n\n");
    }
    let src_count: i64 = conn.query_row(
        "SELECT count(DISTINCT source_id) FROM chunks WHERE subject_id=?1 AND (?2 IS NULL OR topic_id=?2)",
        params![subject_id, topic_id],
        |r| r.get(0),
    )?;
    Ok((out, src_count))
}

// ---- cheatsheet persistence -------------------------------------------

/// Replace the stored cheatsheet for a subject/topic with new sections.
pub fn save_cheatsheet(
    conn: &Connection,
    subject_id: &str,
    topic_id: Option<&str>,
    sections: &[CsSection],
) -> Result<()> {
    conn.execute(
        "DELETE FROM cheatsheets WHERE subject_id=?1 AND IFNULL(topic_id,'')=IFNULL(?2,'')",
        params![subject_id, topic_id],
    )?;
    let cid = new_id();
    let ts = now_ms();
    conn.execute(
        "INSERT INTO cheatsheets (id, subject_id, topic_id, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?4)",
        params![cid, subject_id, topic_id, ts],
    )?;
    for (i, sec) in sections.iter().enumerate() {
        conn.execute(
            "INSERT INTO cheatsheet_sections (id, cheatsheet_id, title, state, ord, body)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                new_id(),
                cid,
                sec.title,
                sec.state,
                i as i64,
                serde_json::to_string(&sec.items)?
            ],
        )?;
    }
    Ok(())
}

/// Read the stored cheatsheet sections for a subject/topic.
pub fn get_cheatsheet_sections(
    conn: &Connection,
    subject_id: &str,
    topic_id: Option<&str>,
) -> Result<Vec<CsSection>> {
    let cid: Option<String> = conn
        .query_row(
            "SELECT id FROM cheatsheets WHERE subject_id=?1 AND IFNULL(topic_id,'')=IFNULL(?2,'') LIMIT 1",
            params![subject_id, topic_id],
            |r| r.get(0),
        )
        .optional()?;
    let Some(cid) = cid else { return Ok(Vec::new()) };
    let mut stmt = conn.prepare(
        "SELECT id, title, state, body FROM cheatsheet_sections WHERE cheatsheet_id=?1 ORDER BY ord",
    )?;
    let rows = stmt.query_map(params![cid], |r| {
        let body: Option<String> = r.get(3)?;
        let items: Vec<CsItem> = body
            .and_then(|b| serde_json::from_str(&b).ok())
            .unwrap_or_default();
        Ok(CsSection {
            id: r.get(0)?,
            title: r.get(1)?,
            state: r.get(2)?,
            items,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

// ---- materials persistence --------------------------------------------

pub fn save_material(
    conn: &Connection,
    subject_id: &str,
    topic_id: Option<&str>,
    kind: &str,
    title: &str,
    meta: &str,
    payload: &serde_json::Value,
) -> Result<String> {
    let id = new_id();
    conn.execute(
        "INSERT INTO materials (id, subject_id, topic_id, kind, title, meta, status, payload, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'ready', ?7, ?8)",
        params![id, subject_id, topic_id, kind, title, meta, payload.to_string(), now_ms()],
    )?;
    Ok(id)
}

pub fn list_materials(conn: &Connection, subject_id: &str) -> Result<Vec<MaterialRec>> {
    let mut stmt = conn.prepare(
        "SELECT m.id, m.kind, m.title, m.meta, m.status, m.payload, t.name
         FROM materials m LEFT JOIN topics t ON t.id=m.topic_id
         WHERE m.subject_id=?1 ORDER BY m.created_at DESC",
    )?;
    let rows = stmt.query_map(params![subject_id], |r| {
        let payload: Option<String> = r.get(5)?;
        Ok(MaterialRec {
            id: r.get(0)?,
            kind: r.get(1)?,
            title: r.get(2)?,
            meta: r.get::<_, Option<String>>(3)?.unwrap_or_default(),
            status: r.get(4)?,
            payload: payload
                .and_then(|p| serde_json::from_str(&p).ok())
                .unwrap_or(serde_json::Value::Null),
            topic: r.get::<_, Option<String>>(6)?.unwrap_or_default(),
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

// ---- chat history (one rolling thread per subject) ---------------------

/// Return the subject's rolling chat thread id, creating it on first use.
pub fn subject_thread(conn: &Connection, subject_id: &str) -> Result<String> {
    if let Some(id) = conn
        .query_row(
            "SELECT id FROM chat_threads WHERE subject_id=?1 ORDER BY created_at LIMIT 1",
            params![subject_id],
            |r| r.get::<_, String>(0),
        )
        .optional()?
    {
        return Ok(id);
    }
    let id = new_id();
    let ts = now_ms();
    conn.execute(
        "INSERT INTO chat_threads (id, subject_id, scope, created_at, updated_at)
         VALUES (?1, ?2, 'subject', ?3, ?3)",
        params![id, subject_id, ts],
    )?;
    Ok(id)
}

/// Append a message to the subject's thread and bump the thread's updated_at.
pub fn add_chat_message(
    conn: &Connection,
    subject_id: &str,
    role: &str,
    text: &str,
) -> Result<()> {
    let tid = subject_thread(conn, subject_id)?;
    let ts = now_ms();
    conn.execute(
        "INSERT INTO chat_messages (id, thread_id, role, text, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![new_id(), tid, role, text, ts],
    )?;
    conn.execute(
        "UPDATE chat_threads SET updated_at=?2 WHERE id=?1",
        params![tid, ts],
    )?;
    Ok(())
}

/// All messages in the subject's thread, oldest first.
pub fn list_chat_messages(conn: &Connection, subject_id: &str) -> Result<Vec<ChatMsg>> {
    let tid = subject_thread(conn, subject_id)?;
    let mut stmt = conn.prepare(
        "SELECT role, text, created_at FROM chat_messages
         WHERE thread_id=?1 ORDER BY created_at ASC",
    )?;
    let rows = stmt.query_map(params![tid], |r| {
        Ok(ChatMsg {
            role: r.get(0)?,
            text: r.get(1)?,
            created_at: r.get(2)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

/// Delete every message in the subject's thread (keeps the thread row).
pub fn clear_chat(conn: &Connection, subject_id: &str) -> Result<()> {
    let tid = subject_thread(conn, subject_id)?;
    conn.execute("DELETE FROM chat_messages WHERE thread_id=?1", params![tid])?;
    Ok(())
}

// ---- settings ----------------------------------------------------------

/// All settings as a JSON object (for the Settings page to hydrate).
pub fn all_settings(conn: &Connection) -> Result<serde_json::Value> {
    let mut stmt = conn.prepare("SELECT key, value FROM settings")?;
    let rows = stmt.query_map([], |r| {
        Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
    })?;
    let mut map = serde_json::Map::new();
    for row in rows {
        let (k, v) = row?;
        map.insert(k, serde_json::Value::String(v));
    }
    Ok(serde_json::Value::Object(map))
}


pub fn get_setting(conn: &Connection, key: &str) -> Result<Option<String>> {
    Ok(conn
        .query_row("SELECT value FROM settings WHERE key=?1", params![key], |r| {
            r.get(0)
        })
        .optional()?)
}

pub fn set_setting(conn: &Connection, key: &str, value: &str) -> Result<()> {
    conn.execute(
        "INSERT INTO settings (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value=excluded.value",
        params![key, value],
    )?;
    Ok(())
}

// ---- long-term memory --------------------------------------------------

pub fn insert_memory(conn: &Connection, content: &str, source: Option<&str>) -> Result<Memory> {
    let id = new_id();
    let ts = now_ms();
    conn.execute(
        "INSERT INTO user_memory (id, content, source, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?4)",
        params![id, content, source, ts],
    )?;
    Ok(Memory {
        id,
        content: content.to_string(),
        source: source.map(|s| s.to_string()),
        created_at: ts,
        updated_at: ts,
    })
}

pub fn list_memory(conn: &Connection) -> Result<Vec<Memory>> {
    let mut stmt = conn.prepare(
        "SELECT id, content, source, created_at, updated_at FROM user_memory ORDER BY created_at DESC",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok(Memory {
            id: r.get(0)?,
            content: r.get(1)?,
            source: r.get(2)?,
            created_at: r.get(3)?,
            updated_at: r.get(4)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

pub fn delete_memory(conn: &Connection, id: &str) -> Result<()> {
    conn.execute("DELETE FROM user_memory WHERE id=?1", params![id])?;
    Ok(())
}

// ---- database stats / maintenance -------------------------------------

/// Count rows in a table, returning 0 if the table does not exist.
fn count_table(conn: &Connection, table: &str) -> i64 {
    conn.query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r.get(0))
        .unwrap_or(0)
}

pub fn content_counts(conn: &Connection) -> (i64, i64, i64) {
    (
        count_table(conn, "subjects"),
        count_table(conn, "sources"),
        count_table(conn, "chunks"),
    )
}

/// Delete all user content while keeping the settings table intact.
pub fn delete_all_content(conn: &Connection) -> Result<()> {
    // Order matters only loosely thanks to ON DELETE CASCADE, but be explicit.
    for table in [
        "chat_messages",
        "chat_threads",
        "cheatsheet_sections",
        "cheatsheets",
        "materials",
        "chunks",
        "source_tags",
        "sources",
        "topics",
        "subjects",
        "tags",
        "user_memory",
    ] {
        // Ignore "no such table" so a partial schema never blocks the wipe.
        let _ = conn.execute(&format!("DELETE FROM {table}"), []);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::AppState;

    #[test]
    fn subject_topic_source_tree_roundtrips() {
        let st = AppState::in_memory().unwrap();
        let c = st.db.lock().unwrap();
        let sid = insert_subject(&c, "Algorithms", Some("CS-3490"), None, None).unwrap();
        let tid = insert_topic(&c, &sid, "Recursion", None).unwrap();
        let srcid = insert_source(&c, &sid, Some(&tid), "lec3.md", "md", None).unwrap();
        finalize_source(&c, &srcid, "ready", Some("3 pages"), Some("body"), None).unwrap();
        attach_tags(&c, &srcid, &["lecture".into(), "exam".into()]).unwrap();

        let subs = list_subjects(&c).unwrap();
        assert_eq!(subs.len(), 1);
        assert_eq!(subs[0].source_count, 1);
        assert_eq!(subs[0].topics.len(), 1);
        assert_eq!(subs[0].topics[0].sources.len(), 1);
        assert_eq!(subs[0].topics[0].sources[0].tags.len(), 2);
    }

    #[test]
    fn settings_upsert() {
        let st = AppState::in_memory().unwrap();
        let c = st.db.lock().unwrap();
        set_setting(&c, "embed_provider", "stub").unwrap();
        set_setting(&c, "embed_provider", "gemini").unwrap();
        assert_eq!(get_setting(&c, "embed_provider").unwrap().unwrap(), "gemini");
    }
}
