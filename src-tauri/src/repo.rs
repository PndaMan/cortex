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
/// Rename a source (used by content-based auto-naming after ingest).
pub fn rename_source(conn: &Connection, id: &str, name: &str) -> Result<()> {
    conn.execute(
        "UPDATE sources SET name=?2, updated_at=?3 WHERE id=?1",
        params![id, name, now_ms()],
    )?;
    Ok(())
}

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

/// Drop all stored chunks for a source (used before a re-ingest).
pub fn clear_chunks(conn: &Connection, source_id: &str) -> Result<()> {
    conn.execute("DELETE FROM chunks WHERE source_id=?1", params![source_id])?;
    Ok(())
}

/// Cosine top-k over stored chunk vectors, optionally scoped to a subject.
/// Uses the statically-linked `sqlite-vec` extension's `vec_distance_cosine` to
/// rank in SQL (embeddings are stored as little-endian f32 BLOBs, which is exactly
/// sqlite-vec's compact float32 format). Falls back to the Rust cosine scan if the
/// SQL path fails for any reason (e.g. an unexpected dimension/format edge case).
pub fn search_chunks(
    conn: &Connection,
    subject_id: Option<&str>,
    query_vec: &[f32],
    k: usize,
) -> Result<Vec<ChunkHit>> {
    let qblob = crate::vector::f32s_to_blob(query_vec);
    match search_chunks_vec(conn, subject_id, &qblob, k) {
        Ok(hits) => Ok(hits),
        // sqlite-vec errors (e.g. dimension mismatch across mixed embed models)
        // shouldn't take down retrieval — fall back to the tolerant Rust scan.
        Err(_) => search_chunks_scan(conn, subject_id, query_vec, k),
    }
}

/// sqlite-vec path: rank entirely in SQL via `vec_distance_cosine`. The
/// `length` guard skips rows whose vector dimension differs from the query so a
/// single mismatched blob can't error the whole query.
fn search_chunks_vec(
    conn: &Connection,
    subject_id: Option<&str>,
    query_blob: &[u8],
    k: usize,
) -> Result<Vec<ChunkHit>> {
    let sql = "SELECT c.id, c.source_id, s.name, c.text, c.loc,
                      vec_distance_cosine(c.embedding, ?2) AS dist
               FROM chunks c JOIN sources s ON s.id=c.source_id
               WHERE (?1 IS NULL OR c.subject_id=?1)
                 AND length(c.embedding) = length(?2)
               ORDER BY dist ASC
               LIMIT ?3";
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map(params![subject_id, query_blob, k as i64], |r| {
        let dist: f64 = r.get(5)?;
        Ok(ChunkHit {
            id: r.get(0)?,
            source_id: r.get(1)?,
            source_name: r.get(2)?,
            text: r.get(3)?,
            loc: r.get(4)?,
            // cosine distance → similarity, matching the prior cosine() semantics.
            score: (1.0 - dist) as f32,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

/// Fallback: Rust-side cosine scan over all in-scope chunk vectors.
fn search_chunks_scan(
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

pub fn delete_material(conn: &Connection, id: &str) -> Result<()> {
    conn.execute("DELETE FROM materials WHERE id=?1", params![id])?;
    Ok(())
}

pub fn rename_material(conn: &Connection, id: &str, title: &str) -> Result<()> {
    let n = conn.execute(
        "UPDATE materials SET title=?2 WHERE id=?1",
        params![id, title],
    )?;
    if n == 0 {
        return Err(Error::NotFound(format!("material {id}")));
    }
    Ok(())
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

/// Fetch a single material by id (for export). Errors if not found.
pub fn get_material(conn: &Connection, id: &str) -> Result<MaterialRec> {
    conn.query_row(
        "SELECT m.id, m.kind, m.title, m.meta, m.status, m.payload, t.name
         FROM materials m LEFT JOIN topics t ON t.id=m.topic_id
         WHERE m.id=?1",
        params![id],
        |r| {
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
        },
    )
    .optional()?
    .ok_or_else(|| Error::NotFound(format!("material {id}")))
}

// ---- chat history (multiple conversation threads per subject) ----------

/// Settings key holding the active thread id for a subject.
fn active_thread_key(subject_id: &str) -> String {
    format!("active_thread_{subject_id}")
}

/// Insert a brand-new (empty) conversation thread for the subject and return its id.
fn insert_thread(conn: &Connection, subject_id: &str) -> Result<String> {
    let id = new_id();
    let ts = now_ms();
    conn.execute(
        "INSERT INTO chat_threads (id, subject_id, scope, created_at, updated_at)
         VALUES (?1, ?2, 'subject', ?3, ?3)",
        params![id, subject_id, ts],
    )?;
    Ok(id)
}

/// Resolve the subject's currently active thread, creating one if needed.
///
/// Order of preference: the thread recorded in the `active_thread_<subject>`
/// setting (if it still exists) → the most recently updated thread → a fresh
/// thread. The resolved id is always persisted back to the setting.
pub fn current_thread(conn: &Connection, subject_id: &str) -> Result<String> {
    // 1. Honour the recorded active thread if its row still exists.
    if let Some(saved) = get_setting(conn, &active_thread_key(subject_id))? {
        let exists: bool = conn
            .query_row(
                "SELECT 1 FROM chat_threads WHERE id=?1 AND subject_id=?2",
                params![saved, subject_id],
                |_| Ok(true),
            )
            .optional()?
            .unwrap_or(false);
        if exists {
            return Ok(saved);
        }
    }
    // 2. Fall back to the most recent thread for the subject.
    let recent: Option<String> = conn
        .query_row(
            "SELECT id FROM chat_threads WHERE subject_id=?1 ORDER BY updated_at DESC LIMIT 1",
            params![subject_id],
            |r| r.get::<_, String>(0),
        )
        .optional()?;
    // 3. Otherwise create a fresh thread.
    let id = match recent {
        Some(id) => id,
        None => insert_thread(conn, subject_id)?,
    };
    set_setting(conn, &active_thread_key(subject_id), &id)?;
    Ok(id)
}

/// Start a fresh conversation thread for the subject and make it active.
pub fn new_thread(conn: &Connection, subject_id: &str) -> Result<String> {
    let id = insert_thread(conn, subject_id)?;
    set_setting(conn, &active_thread_key(subject_id), &id)?;
    Ok(id)
}

/// Mark an existing thread as the subject's active conversation.
pub fn set_active_thread(conn: &Connection, subject_id: &str, thread_id: &str) -> Result<()> {
    set_setting(conn, &active_thread_key(subject_id), thread_id)
}

/// Append a message to the subject's active thread and bump its updated_at.
pub fn add_chat_message(
    conn: &Connection,
    subject_id: &str,
    role: &str,
    text: &str,
) -> Result<()> {
    let tid = current_thread(conn, subject_id)?;
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

/// All messages in the subject's active thread, oldest first.
pub fn list_chat_messages(conn: &Connection, subject_id: &str) -> Result<Vec<ChatMsg>> {
    let tid = current_thread(conn, subject_id)?;
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

/// Delete every message in the subject's active thread (keeps the thread row).
pub fn clear_chat(conn: &Connection, subject_id: &str) -> Result<()> {
    let tid = current_thread(conn, subject_id)?;
    conn.execute("DELETE FROM chat_messages WHERE thread_id=?1", params![tid])?;
    Ok(())
}

/// Summaries of every conversation thread for a subject, newest first.
pub fn list_threads(conn: &Connection, subject_id: &str) -> Result<Vec<ThreadInfo>> {
    let mut stmt = conn.prepare(
        "SELECT id, updated_at FROM chat_threads WHERE subject_id=?1 ORDER BY updated_at DESC",
    )?;
    let rows = stmt.query_map(params![subject_id], |r| {
        Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))
    })?;
    let threads: Vec<(String, i64)> = rows.collect::<rusqlite::Result<_>>()?;
    let mut out = Vec::with_capacity(threads.len());
    for (id, updated_at) in threads {
        let count: i64 = conn.query_row(
            "SELECT count(*) FROM chat_messages WHERE thread_id=?1",
            params![id],
            |r| r.get(0),
        )?;
        // Title = first user message, truncated; else a placeholder.
        let first: Option<String> = conn
            .query_row(
                "SELECT text FROM chat_messages WHERE thread_id=?1 AND role='user'
                 ORDER BY created_at ASC LIMIT 1",
                params![id],
                |r| r.get(0),
            )
            .optional()?;
        let title = match first {
            Some(t) => {
                let t = t.trim();
                if t.chars().count() > 48 {
                    let truncated: String = t.chars().take(48).collect();
                    format!("{truncated}…")
                } else {
                    t.to_string()
                }
            }
            None => "New conversation".to_string(),
        };
        out.push(ThreadInfo {
            id,
            title,
            updated_at,
            count,
        });
    }
    Ok(out)
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

// ---- notes -------------------------------------------------------------

fn map_note(r: &rusqlite::Row) -> rusqlite::Result<Note> {
    Ok(Note {
        id: r.get(0)?,
        subject_id: r.get(1)?,
        topic_id: r.get(2)?,
        title: r.get(3)?,
        body: r.get(4)?,
        source_id: r.get(5)?,
        created_at: r.get(6)?,
        updated_at: r.get(7)?,
    })
}

const NOTE_COLS: &str =
    "id, subject_id, topic_id, title, body, source_id, created_at, updated_at";

pub fn insert_note(
    conn: &Connection,
    subject_id: Option<&str>,
    topic_id: Option<&str>,
    title: &str,
    body: &str,
) -> Result<String> {
    let id = new_id();
    let ts = now_ms();
    conn.execute(
        "INSERT INTO notes (id, subject_id, topic_id, title, body, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)",
        params![id, subject_id, topic_id, title, body, ts],
    )?;
    Ok(id)
}

pub fn list_notes(conn: &Connection, subject_id: Option<&str>) -> Result<Vec<Note>> {
    let sql = format!(
        "SELECT {NOTE_COLS} FROM notes
         WHERE (?1 IS NULL OR subject_id=?1) ORDER BY updated_at DESC"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params![subject_id], map_note)?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

pub fn get_note(conn: &Connection, id: &str) -> Result<Note> {
    let sql = format!("SELECT {NOTE_COLS} FROM notes WHERE id=?1");
    conn.query_row(&sql, params![id], map_note)
        .optional()?
        .ok_or_else(|| Error::NotFound(format!("note {id}")))
}

pub fn update_note(conn: &Connection, id: &str, title: &str, body: &str) -> Result<()> {
    let n = conn.execute(
        "UPDATE notes SET title=?2, body=?3, updated_at=?4 WHERE id=?1",
        params![id, title, body, now_ms()],
    )?;
    if n == 0 {
        return Err(Error::NotFound(format!("note {id}")));
    }
    Ok(())
}

/// Link a note to the source generated when it was converted.
pub fn set_note_source(conn: &Connection, id: &str, source_id: &str) -> Result<()> {
    conn.execute(
        "UPDATE notes SET source_id=?2, updated_at=?3 WHERE id=?1",
        params![id, source_id, now_ms()],
    )?;
    Ok(())
}

pub fn delete_note(conn: &Connection, id: &str) -> Result<()> {
    conn.execute("DELETE FROM notes WHERE id=?1", params![id])?;
    Ok(())
}

// ---- calendar events / tasks ------------------------------------------

fn map_event(r: &rusqlite::Row) -> rusqlite::Result<CalEvent> {
    Ok(CalEvent {
        id: r.get(0)?,
        subject_id: r.get(1)?,
        title: r.get(2)?,
        description: r.get(3)?,
        location: r.get(4)?,
        color: r.get(5)?,
        start_ms: r.get(6)?,
        end_ms: r.get(7)?,
        all_day: r.get::<_, i64>(8)? != 0,
        kind: r.get(9)?,
        done: r.get::<_, i64>(10)? != 0,
        reminder_ms: r.get(11)?,
        notified: r.get::<_, i64>(12)? != 0,
        google_id: r.get(13)?,
        created_at: r.get(14)?,
        updated_at: r.get(15)?,
    })
}

const EVENT_COLS: &str = "id, subject_id, title, description, location, color, start_ms, end_ms, \
    all_day, kind, done, reminder_ms, notified, google_id, created_at, updated_at";

#[allow(clippy::too_many_arguments)]
pub fn insert_event(
    conn: &Connection,
    subject_id: Option<&str>,
    title: &str,
    description: Option<&str>,
    location: Option<&str>,
    color: Option<&str>,
    start_ms: i64,
    end_ms: Option<i64>,
    all_day: bool,
    kind: &str,
    reminder_ms: Option<i64>,
) -> Result<String> {
    let id = new_id();
    let ts = now_ms();
    conn.execute(
        "INSERT INTO events
            (id, subject_id, title, description, location, color, start_ms, end_ms,
             all_day, kind, done, reminder_ms, notified, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 0, ?11, 0, ?12, ?12)",
        params![
            id, subject_id, title, description, location, color, start_ms, end_ms,
            all_day as i64, kind, reminder_ms, ts
        ],
    )?;
    Ok(id)
}

pub fn list_events(
    conn: &Connection,
    subject_id: Option<&str>,
    from_ms: Option<i64>,
    to_ms: Option<i64>,
) -> Result<Vec<CalEvent>> {
    let sql = format!(
        "SELECT {EVENT_COLS} FROM events
         WHERE (?1 IS NULL OR subject_id=?1)
           AND (?2 IS NULL OR start_ms >= ?2)
           AND (?3 IS NULL OR start_ms <= ?3)
         ORDER BY start_ms ASC"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params![subject_id, from_ms, to_ms], map_event)?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

pub fn get_event(conn: &Connection, id: &str) -> Result<CalEvent> {
    let sql = format!("SELECT {EVENT_COLS} FROM events WHERE id=?1");
    conn.query_row(&sql, params![id], map_event)
        .optional()?
        .ok_or_else(|| Error::NotFound(format!("event {id}")))
}

#[allow(clippy::too_many_arguments)]
pub fn update_event(
    conn: &Connection,
    id: &str,
    title: &str,
    description: Option<&str>,
    location: Option<&str>,
    color: Option<&str>,
    start_ms: i64,
    end_ms: Option<i64>,
    all_day: bool,
    kind: &str,
    reminder_ms: Option<i64>,
) -> Result<()> {
    // Editing an event resets its notified flag so a moved reminder fires again.
    let n = conn.execute(
        "UPDATE events SET
            title=?2, description=?3, location=?4, color=?5, start_ms=?6, end_ms=?7,
            all_day=?8, kind=?9, reminder_ms=?10, notified=0, updated_at=?11
         WHERE id=?1",
        params![
            id, title, description, location, color, start_ms, end_ms,
            all_day as i64, kind, reminder_ms, now_ms()
        ],
    )?;
    if n == 0 {
        return Err(Error::NotFound(format!("event {id}")));
    }
    Ok(())
}

pub fn set_event_done(conn: &Connection, id: &str, done: bool) -> Result<()> {
    let n = conn.execute(
        "UPDATE events SET done=?2, updated_at=?3 WHERE id=?1",
        params![id, done as i64, now_ms()],
    )?;
    if n == 0 {
        return Err(Error::NotFound(format!("event {id}")));
    }
    Ok(())
}

pub fn delete_event(conn: &Connection, id: &str) -> Result<()> {
    conn.execute("DELETE FROM events WHERE id=?1", params![id])?;
    Ok(())
}

/// Events whose reminder is due and not yet notified.
pub fn due_reminders(conn: &Connection, now: i64) -> Result<Vec<CalEvent>> {
    let sql = format!(
        "SELECT {EVENT_COLS} FROM events
         WHERE reminder_ms IS NOT NULL AND reminder_ms <= ?1 AND notified = 0
         ORDER BY reminder_ms ASC"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params![now], map_event)?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

pub fn mark_notified(conn: &Connection, id: &str) -> Result<()> {
    conn.execute(
        "UPDATE events SET notified=1, updated_at=?2 WHERE id=?1",
        params![id, now_ms()],
    )?;
    Ok(())
}

/// Insert or update an event keyed by its Google Calendar id (sync path).
/// Provided ahead of the Google Calendar sync slice (no caller yet).
#[allow(dead_code)]
#[allow(clippy::too_many_arguments)]
pub fn upsert_event_by_google_id(
    conn: &Connection,
    google_id: &str,
    subject_id: Option<&str>,
    title: &str,
    description: Option<&str>,
    location: Option<&str>,
    color: Option<&str>,
    start_ms: i64,
    end_ms: Option<i64>,
    all_day: bool,
    kind: &str,
    reminder_ms: Option<i64>,
) -> Result<String> {
    let ts = now_ms();
    let existing: Option<String> = conn
        .query_row(
            "SELECT id FROM events WHERE google_id=?1",
            params![google_id],
            |r| r.get(0),
        )
        .optional()?;
    match existing {
        Some(id) => {
            conn.execute(
                "UPDATE events SET
                    subject_id=?2, title=?3, description=?4, location=?5, color=?6,
                    start_ms=?7, end_ms=?8, all_day=?9, kind=?10, reminder_ms=?11, updated_at=?12
                 WHERE id=?1",
                params![
                    id, subject_id, title, description, location, color, start_ms, end_ms,
                    all_day as i64, kind, reminder_ms, ts
                ],
            )?;
            Ok(id)
        }
        None => {
            let id = new_id();
            conn.execute(
                "INSERT INTO events
                    (id, subject_id, title, description, location, color, start_ms, end_ms,
                     all_day, kind, done, reminder_ms, notified, google_id, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 0, ?11, 0, ?12, ?13, ?13)",
                params![
                    id, subject_id, title, description, location, color, start_ms, end_ms,
                    all_day as i64, kind, reminder_ms, google_id, ts
                ],
            )?;
            Ok(id)
        }
    }
}

/// Attach a Google Calendar event id to a local event after pushing it to
/// Google (the sync path uses this to avoid re-creating already-synced events).
pub fn set_event_google_id(conn: &Connection, id: &str, google_id: &str) -> Result<()> {
    conn.execute(
        "UPDATE events SET google_id=?2, updated_at=?3 WHERE id=?1",
        params![id, google_id, now_ms()],
    )?;
    Ok(())
}

// ---- review (spaced repetition) ---------------------------------------

#[allow(clippy::too_many_arguments)]
pub fn record_attempt(
    conn: &Connection,
    subject_id: &str,
    material_id: Option<&str>,
    kind: &str,
    item_index: i64,
    item_key: &str,
    correct: bool,
) -> Result<String> {
    let id = new_id();
    conn.execute(
        "INSERT INTO attempts
            (id, subject_id, material_id, kind, item_index, item_key, correct, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![id, subject_id, material_id, kind, item_index, item_key, correct as i64, now_ms()],
    )?;
    Ok(id)
}

/// Distinct items (by item_key) whose MOST RECENT attempt was incorrect — the
/// set to re-study. One row per item_key, carrying its item_index.
pub fn wrong_items(conn: &Connection, subject_id: &str, kind: &str) -> Result<Vec<ReviewItem>> {
    // Use rowid (insertion order) as the "latest" key, not created_at — two
    // attempts can share a millisecond, and a created_at tie would match BOTH
    // rows and wrongly keep an already-corrected item in the review set.
    let mut stmt = conn.prepare(
        "SELECT a.item_index, a.item_key
         FROM attempts a
         JOIN (
             SELECT item_key, MAX(rowid) AS latest_row
             FROM attempts
             WHERE subject_id=?1 AND kind=?2
             GROUP BY item_key
         ) m ON m.item_key = a.item_key AND m.latest_row = a.rowid
         WHERE a.correct = 0
         ORDER BY a.item_index",
    )?;
    let rows = stmt.query_map(params![subject_id, kind], |r| {
        Ok(ReviewItem {
            item_index: r.get(0)?,
            item_key: r.get(1)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

// ---- SM-2 spaced repetition -------------------------------------------

/// Grade a card with the SM-2 algorithm and upsert its schedule. `quality` is
/// 0-5 (Again≈1, Hard≈3, Good≈4, Easy≈5). Also logs an `attempts` row (correct =
/// quality >= 3) so the legacy "review missed" set keeps working. Returns the new
/// schedule. SM-2 reference: ease' = ease + (0.1 - (5-q)(0.08 + (5-q)0.02)), min 1.3.
pub fn srs_grade(
    conn: &Connection,
    subject_id: &str,
    material_id: Option<&str>,
    kind: &str,
    item_index: i64,
    item_key: &str,
    quality: i64,
) -> Result<SrsResult> {
    let q = quality.clamp(0, 5);
    let now = now_ms();

    // Current schedule for this item, or SM-2 defaults for a brand-new card.
    let existing: Option<(f64, i64, i64, i64)> = conn
        .query_row(
            "SELECT ease, interval_d, reps, lapses FROM srs_cards
             WHERE subject_id=?1 AND kind=?2 AND item_key=?3",
            params![subject_id, kind, item_key],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .optional()?;
    let (mut ease, mut interval_d, mut reps, mut lapses) = existing.unwrap_or((2.5, 0, 0, 0));

    if q < 3 {
        // Lapse: reset reps, relearn tomorrow.
        reps = 0;
        interval_d = 1;
        lapses += 1;
    } else {
        reps += 1;
        interval_d = match reps {
            1 => 1,
            2 => 6,
            _ => ((interval_d as f64) * ease).round() as i64,
        }
        .max(1);
    }
    let qf = q as f64;
    ease = (ease + (0.1 - (5.0 - qf) * (0.08 + (5.0 - qf) * 0.02))).max(1.3);
    let due_at = now + interval_d * 86_400_000;

    conn.execute(
        "INSERT INTO srs_cards
            (id, subject_id, material_id, kind, item_index, item_key,
             ease, interval_d, reps, lapses, last_grade, due_at, created_at, updated_at)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?13)
         ON CONFLICT(subject_id, kind, item_key) DO UPDATE SET
            material_id=excluded.material_id, item_index=excluded.item_index,
            ease=excluded.ease, interval_d=excluded.interval_d, reps=excluded.reps,
            lapses=excluded.lapses, last_grade=excluded.last_grade,
            due_at=excluded.due_at, updated_at=excluded.updated_at",
        params![
            new_id(), subject_id, material_id, kind, item_index, item_key,
            ease, interval_d, reps, lapses, q, due_at, now
        ],
    )?;

    // Keep the legacy attempt log in sync (drives `wrong_items`).
    record_attempt(conn, subject_id, material_id, kind, item_index, item_key, q >= 3)?;

    Ok(SrsResult { due_at, interval_d, reps, ease })
}

/// Cards whose `due_at` has arrived (<= now), oldest-due first — the study queue.
pub fn srs_due(conn: &Connection, subject_id: &str, kind: &str) -> Result<Vec<DueCard>> {
    let mut stmt = conn.prepare(
        "SELECT item_index, item_key, due_at, reps, interval_d FROM srs_cards
         WHERE subject_id=?1 AND kind=?2 AND due_at<=?3 ORDER BY due_at ASC",
    )?;
    let rows = stmt.query_map(params![subject_id, kind, now_ms()], |r| {
        Ok(DueCard {
            item_index: r.get(0)?,
            item_key: r.get(1)?,
            due_at: r.get(2)?,
            reps: r.get(3)?,
            interval_d: r.get(4)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

/// Due-now and total scheduled-card counts for a subject+kind (e.g. "5 due").
pub fn srs_stats(conn: &Connection, subject_id: &str, kind: &str) -> Result<SrsStats> {
    let due: i64 = conn.query_row(
        "SELECT count(*) FROM srs_cards WHERE subject_id=?1 AND kind=?2 AND due_at<=?3",
        params![subject_id, kind, now_ms()],
        |r| r.get(0),
    )?;
    let total: i64 = conn.query_row(
        "SELECT count(*) FROM srs_cards WHERE subject_id=?1 AND kind=?2",
        params![subject_id, kind],
        |r| r.get(0),
    )?;
    Ok(SrsStats { due, total })
}

// ---- citations (per-subject bibliography) -----------------------------

#[allow(clippy::too_many_arguments)]
pub fn insert_citation(
    conn: &Connection,
    subject_id: &str,
    ctype: &str,
    title: &str,
    authors: Option<&str>,
    year: Option<&str>,
    container: Option<&str>,
    url: Option<&str>,
    doi: Option<&str>,
    notes: Option<&str>,
) -> Result<String> {
    let id = new_id();
    let ts = now_ms();
    conn.execute(
        "INSERT INTO citations
            (id, subject_id, ctype, title, authors, year, container, url, doi, notes, created_at, updated_at)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?11)",
        params![id, subject_id, ctype, title, authors, year, container, url, doi, notes, ts],
    )?;
    Ok(id)
}

fn row_to_citation(r: &rusqlite::Row) -> rusqlite::Result<Reference> {
    Ok(Reference {
        id: r.get(0)?,
        subject_id: r.get(1)?,
        ctype: r.get(2)?,
        title: r.get(3)?,
        authors: r.get(4)?,
        year: r.get(5)?,
        container: r.get(6)?,
        url: r.get(7)?,
        doi: r.get(8)?,
        notes: r.get(9)?,
        created_at: r.get(10)?,
        updated_at: r.get(11)?,
    })
}

pub fn list_citations(conn: &Connection, subject_id: &str) -> Result<Vec<Reference>> {
    let mut stmt = conn.prepare(
        "SELECT id, subject_id, ctype, title, authors, year, container, url, doi, notes,
                created_at, updated_at
         FROM citations WHERE subject_id=?1 ORDER BY created_at DESC",
    )?;
    let rows = stmt.query_map(params![subject_id], row_to_citation)?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

#[allow(clippy::too_many_arguments)]
pub fn update_citation(
    conn: &Connection,
    id: &str,
    ctype: &str,
    title: &str,
    authors: Option<&str>,
    year: Option<&str>,
    container: Option<&str>,
    url: Option<&str>,
    doi: Option<&str>,
    notes: Option<&str>,
) -> Result<()> {
    let n = conn.execute(
        "UPDATE citations SET ctype=?2, title=?3, authors=?4, year=?5, container=?6,
            url=?7, doi=?8, notes=?9, updated_at=?10 WHERE id=?1",
        params![id, ctype, title, authors, year, container, url, doi, notes, now_ms()],
    )?;
    if n == 0 {
        return Err(Error::NotFound(format!("citation {id}")));
    }
    Ok(())
}

pub fn delete_citation(conn: &Connection, id: &str) -> Result<()> {
    conn.execute("DELETE FROM citations WHERE id=?1", params![id])?;
    Ok(())
}

// ---- source move (re-file across subject/topic) -----------------------

/// Re-file a source to a new subject (and optional topic), keeping retrieval
/// scoping correct by updating the denormalized subject_id/topic_id on the
/// source's chunks too.
pub fn move_source(
    conn: &Connection,
    source_id: &str,
    subject_id: &str,
    topic_id: Option<&str>,
) -> Result<()> {
    let n = conn.execute(
        "UPDATE sources SET subject_id=?2, topic_id=?3, updated_at=?4 WHERE id=?1",
        params![source_id, subject_id, topic_id, now_ms()],
    )?;
    if n == 0 {
        return Err(Error::NotFound(format!("source {source_id}")));
    }
    conn.execute(
        "UPDATE chunks SET subject_id=?2, topic_id=?3 WHERE source_id=?1",
        params![source_id, subject_id, topic_id],
    )?;
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
    fn srs_sm2_schedule_progresses_and_lapses() {
        let st = AppState::in_memory().unwrap();
        let c = st.db.lock().unwrap();
        let sid = insert_subject(&c, "Bio", None, None, None).unwrap();
        let key = "What is ATP?";

        // First "Good" grade: a new card → interval 1 day, reps 1.
        let r1 = srs_grade(&c, &sid, None, "flashcard", 0, key, 4).unwrap();
        assert_eq!(r1.reps, 1);
        assert_eq!(r1.interval_d, 1);

        // Second "Good": SM-2 second step → interval 6 days, reps 2.
        let r2 = srs_grade(&c, &sid, None, "flashcard", 0, key, 4).unwrap();
        assert_eq!(r2.reps, 2);
        assert_eq!(r2.interval_d, 6);

        // Third "Good": interval = round(6 * ease) with ease > 1.3.
        let r3 = srs_grade(&c, &sid, None, "flashcard", 0, key, 4).unwrap();
        assert_eq!(r3.reps, 3);
        assert!(r3.interval_d > 6, "interval should grow: {}", r3.interval_d);

        // Exactly one schedule row (upsert, not insert-per-grade); 1 total card.
        assert_eq!(srs_stats(&c, &sid, "flashcard").unwrap().total, 1);

        // "Again" lapse resets reps to 0 and interval back to 1 day.
        let r4 = srs_grade(&c, &sid, None, "flashcard", 0, key, 1).unwrap();
        assert_eq!(r4.reps, 0);
        assert_eq!(r4.interval_d, 1);

        // The legacy "wrong items" set picks up the lapse (latest attempt wrong).
        assert_eq!(wrong_items(&c, &sid, "flashcard").unwrap().len(), 1);
    }

    #[test]
    fn citations_crud_roundtrips() {
        let st = AppState::in_memory().unwrap();
        let c = st.db.lock().unwrap();
        let sid = insert_subject(&c, "History", None, None, None).unwrap();
        let id = insert_citation(
            &c, &sid, "book", "The Guns of August", Some("Tuchman, B."),
            Some("1962"), Some("Macmillan"), None, None, Some("ch. 1"),
        )
        .unwrap();
        let list = list_citations(&c, &sid).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].title, "The Guns of August");
        assert_eq!(list[0].authors.as_deref(), Some("Tuchman, B."));

        update_citation(
            &c, &id, "book", "The Guns of August (rev.)", Some("Tuchman, B."),
            Some("1962"), Some("Macmillan"), Some("https://example.com"), None, None,
        )
        .unwrap();
        let list = list_citations(&c, &sid).unwrap();
        assert_eq!(list[0].title, "The Guns of August (rev.)");
        assert_eq!(list[0].url.as_deref(), Some("https://example.com"));

        delete_citation(&c, &id).unwrap();
        assert!(list_citations(&c, &sid).unwrap().is_empty());
    }

    #[test]
    fn settings_upsert() {
        let st = AppState::in_memory().unwrap();
        let c = st.db.lock().unwrap();
        set_setting(&c, "embed_provider", "stub").unwrap();
        set_setting(&c, "embed_provider", "gemini").unwrap();
        assert_eq!(get_setting(&c, "embed_provider").unwrap().unwrap(), "gemini");
    }

    #[test]
    fn notes_crud_roundtrips() {
        let st = AppState::in_memory().unwrap();
        let c = st.db.lock().unwrap();
        let sid = insert_subject(&c, "Bio", None, None, None).unwrap();
        let id = insert_note(&c, Some(&sid), None, "Cells", "mitochondria").unwrap();
        assert_eq!(list_notes(&c, Some(&sid)).unwrap().len(), 1);
        update_note(&c, &id, "Cells v2", "powerhouse").unwrap();
        assert_eq!(get_note(&c, &id).unwrap().title, "Cells v2");
        let srcid = insert_source(&c, &sid, None, "Cells", "note", None).unwrap();
        set_note_source(&c, &id, &srcid).unwrap();
        assert_eq!(get_note(&c, &id).unwrap().source_id.as_deref(), Some(srcid.as_str()));
        delete_note(&c, &id).unwrap();
        assert!(list_notes(&c, None).unwrap().is_empty());
    }

    #[test]
    fn events_reminders_and_done() {
        let st = AppState::in_memory().unwrap();
        let c = st.db.lock().unwrap();
        let due = insert_event(
            &c, None, "Exam", None, None, None, 1_000, Some(2_000), false, "event", Some(500),
        )
        .unwrap();
        let _future = insert_event(
            &c, None, "Later", None, None, None, 9_000, None, false, "task", Some(8_000),
        )
        .unwrap();
        // only the past-due, un-notified reminder comes back at now=1000
        let reminders = due_reminders(&c, 1_000).unwrap();
        assert_eq!(reminders.len(), 1);
        assert_eq!(reminders[0].id, due);
        mark_notified(&c, &due).unwrap();
        assert!(due_reminders(&c, 1_000).unwrap().is_empty());
        set_event_done(&c, &due, true).unwrap();
        assert!(get_event(&c, &due).unwrap().done);
        assert_eq!(list_events(&c, None, Some(0), Some(5_000)).unwrap().len(), 1);
    }

    #[test]
    fn review_set_uses_latest_attempt() {
        let st = AppState::in_memory().unwrap();
        let c = st.db.lock().unwrap();
        let sid = insert_subject(&c, "Chem", None, None, None).unwrap();
        record_attempt(&c, &sid, None, "quiz", 0, "Q1", false).unwrap();
        // a later correct attempt on Q1 removes it from the review set
        record_attempt(&c, &sid, None, "quiz", 0, "Q1", true).unwrap();
        record_attempt(&c, &sid, None, "quiz", 1, "Q2", false).unwrap();
        let wrong = wrong_items(&c, &sid, "quiz").unwrap();
        assert_eq!(wrong.len(), 1);
        assert_eq!(wrong[0].item_key, "Q2");
    }

    #[test]
    fn move_source_repoints_chunks() {
        let st = AppState::in_memory().unwrap();
        let c = st.db.lock().unwrap();
        let a = insert_subject(&c, "A", None, None, None).unwrap();
        let b = insert_subject(&c, "B", None, None, None).unwrap();
        let bt = insert_topic(&c, &b, "T", None).unwrap();
        let src = insert_source(&c, &a, None, "s.md", "md", None).unwrap();
        insert_chunk(&c, &src, &a, None, 0, "text", None, 1, &[0u8, 0, 0, 0]).unwrap();
        move_source(&c, &src, &b, Some(&bt)).unwrap();
        assert_eq!(get_source(&c, &src).unwrap().subject_id, b);
        let scoped: i64 = c
            .query_row(
                "SELECT count(*) FROM chunks WHERE subject_id=?1 AND topic_id=?2",
                params![b, bt],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(scoped, 1);
    }
}
