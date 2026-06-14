//! Data-access layer over rusqlite. Pure functions taking `&Connection` so they
//! are trivially unit-testable against an in-memory DB.

use crate::db::{new_id, now_ms};
use crate::error::{Error, Result};
use crate::models::*;
use crate::vector::{blob_to_f32s, cosine};
use rusqlite::{params, Connection, OptionalExtension};
use std::collections::HashMap;

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

fn map_subject(r: &rusqlite::Row) -> rusqlite::Result<Subject> {
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
        moodle_course_id: r.get(10)?,
    })
}

const SUBJECT_COLS: &str =
    "id, name, code, glyph, color, status, streak, position, created_at, updated_at, moodle_course_id";

/// Full Subjects → Topics → Sources tree (what the sidebar + dashboard render).
/// Batched: a fixed 5 queries regardless of subject/topic/source counts
/// (previously 1 + per-subject topics + per-topic sources + per-source tags).
pub fn list_subjects(conn: &Connection) -> Result<Vec<Subject>> {
    let sql = format!("SELECT {SUBJECT_COLS} FROM subjects ORDER BY position, created_at");
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([], map_subject)?;
    let mut subjects: Vec<Subject> = rows.collect::<rusqlite::Result<_>>()?;

    let mut counts: HashMap<String, i64> = HashMap::new();
    let mut stmt =
        conn.prepare("SELECT subject_id, count(*) FROM sources GROUP BY subject_id")?;
    let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))?;
    for row in rows {
        let (sid, n) = row?;
        counts.insert(sid, n);
    }

    let mut topics_by_subject = topics_grouped(conn)?;
    for s in &mut subjects {
        s.topics = topics_by_subject.remove(&s.id).unwrap_or_default();
        s.source_count = counts.get(&s.id).copied().unwrap_or(0);
    }
    Ok(subjects)
}

pub fn get_subject(conn: &Connection, id: &str) -> Result<Subject> {
    let sql = format!("SELECT {SUBJECT_COLS} FROM subjects WHERE id=?1");
    let mut s = conn
        .query_row(&sql, params![id], map_subject)
        .optional()?
        .ok_or_else(|| Error::NotFound(format!("subject {id}")))?;
    s.topics = list_topics(conn, id)?;
    s.source_count = conn.query_row(
        "SELECT count(*) FROM sources WHERE subject_id=?1",
        params![id],
        |r| r.get(0),
    )?;
    Ok(s)
}

// ---- topics ------------------------------------------------------------

pub fn insert_topic(
    conn: &Connection,
    subject_id: &str,
    name: &str,
    glyph: Option<&str>,
    tags: &[String],
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
        "INSERT INTO topics (id, subject_id, name, glyph, position, tags, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)",
        params![id, subject_id, name, glyph, pos, tags_to_text(tags), ts],
    )?;
    Ok(id)
}

pub fn update_topic(
    conn: &Connection,
    id: &str,
    name: &str,
    glyph: Option<&str>,
    tags: &[String],
) -> Result<()> {
    let n = conn.execute(
        "UPDATE topics SET
            name=?2,
            glyph=CASE WHEN ?3 IS NULL THEN glyph ELSE ?3 END,
            tags=?4,
            updated_at=?5
         WHERE id=?1",
        params![id, name, glyph, tags_to_text(tags), now_ms()],
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

/// Persist a new ordering: each id's `position` becomes its index in `ids`.
pub fn reorder_subjects(conn: &Connection, ids: &[String]) -> Result<()> {
    let tx = conn.unchecked_transaction()?;
    let ts = now_ms();
    for (i, id) in ids.iter().enumerate() {
        tx.execute(
            "UPDATE subjects SET position=?2, updated_at=?3 WHERE id=?1",
            params![id, i as i64, ts],
        )?;
    }
    tx.commit()?;
    Ok(())
}

pub fn reorder_topics(conn: &Connection, subject_id: &str, ids: &[String]) -> Result<()> {
    let tx = conn.unchecked_transaction()?;
    let ts = now_ms();
    for (i, id) in ids.iter().enumerate() {
        tx.execute(
            "UPDATE topics SET position=?2, updated_at=?3 WHERE id=?1 AND subject_id=?4",
            params![id, i as i64, ts, subject_id],
        )?;
    }
    tx.commit()?;
    Ok(())
}

fn map_topic(r: &rusqlite::Row) -> rusqlite::Result<Topic> {
    Ok(Topic {
        id: r.get(0)?,
        subject_id: r.get(1)?,
        name: r.get(2)?,
        glyph: r.get(3)?,
        position: r.get(4)?,
        tags: text_to_tags(r.get(5)?),
        sources: Vec::new(),
    })
}

pub fn list_topics(conn: &Connection, subject_id: &str) -> Result<Vec<Topic>> {
    let mut stmt = conn.prepare(
        "SELECT id, subject_id, name, glyph, position, tags FROM topics
         WHERE subject_id=?1 ORDER BY position, created_at",
    )?;
    let rows = stmt.query_map(params![subject_id], map_topic)?;
    let mut topics: Vec<Topic> = rows.collect::<rusqlite::Result<_>>()?;

    // All topic-filed sources of this subject in one query instead of one per topic.
    let sql = format!(
        "SELECT {SOURCE_COLS} FROM sources \
         WHERE subject_id=?1 AND topic_id IS NOT NULL ORDER BY created_at"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params![subject_id], map_source)?;
    let mut by_topic = group_sources_by_topic(rows, tags_by_source(conn, Some(subject_id))?)?;
    for t in &mut topics {
        t.sources = by_topic.remove(&t.id).unwrap_or_default();
    }
    Ok(topics)
}

/// Every subject's topics (with their sources + tags) in 3 fixed queries,
/// grouped by subject id. Backs the sidebar tree via `list_subjects`.
fn topics_grouped(conn: &Connection) -> Result<HashMap<String, Vec<Topic>>> {
    let mut stmt = conn.prepare(
        "SELECT id, subject_id, name, glyph, position, tags FROM topics
         ORDER BY position, created_at",
    )?;
    let rows = stmt.query_map([], map_topic)?;
    let topics: Vec<Topic> = rows.collect::<rusqlite::Result<_>>()?;

    let sql = format!(
        "SELECT {SOURCE_COLS} FROM sources WHERE topic_id IS NOT NULL ORDER BY created_at"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([], map_source)?;
    let mut by_topic = group_sources_by_topic(rows, tags_by_source(conn, None)?)?;

    let mut out: HashMap<String, Vec<Topic>> = HashMap::new();
    for mut t in topics {
        t.sources = by_topic.remove(&t.id).unwrap_or_default();
        out.entry(t.subject_id.clone()).or_default().push(t);
    }
    Ok(out)
}

/// Collect mapped source rows into topic_id → sources, attaching tags from a
/// prefetched map. Row order (created_at) is preserved within each topic.
fn group_sources_by_topic(
    rows: impl Iterator<Item = rusqlite::Result<Source>>,
    mut tags: HashMap<String, Vec<String>>,
) -> Result<HashMap<String, Vec<Source>>> {
    let mut by_topic: HashMap<String, Vec<Source>> = HashMap::new();
    for row in rows {
        let mut s = row?;
        s.tags = tags.remove(&s.id).unwrap_or_default();
        if let Some(tid) = s.topic_id.clone() {
            by_topic.entry(tid).or_default().push(s);
        }
    }
    Ok(by_topic)
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
    let mut tags = tags_by_source(conn, Some(subject_id))?;
    for s in &mut out {
        s.tags = tags.remove(&s.id).unwrap_or_default();
    }
    Ok(out)
}

/// Sources that didn't ingest cleanly: hard failures (`error`) and audio that
/// produced no transcript (`draft` with an error recorded). Used to auto-retry
/// ingestion on app launch so transient failures (offline, model not yet set up)
/// resolve themselves once conditions are right.
pub fn list_failed_sources(conn: &Connection) -> Result<Vec<Source>> {
    let sql = format!(
        "SELECT {SOURCE_COLS} FROM sources \
         WHERE status='error' OR (status='draft' AND error IS NOT NULL) \
         ORDER BY updated_at"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([], map_source)?;
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

/// Tag names for many sources at once (source_id → sorted names), optionally
/// restricted to one subject. One query instead of one per source.
fn tags_by_source(
    conn: &Connection,
    subject_id: Option<&str>,
) -> Result<HashMap<String, Vec<String>>> {
    let mut out: HashMap<String, Vec<String>> = HashMap::new();
    let base = "SELECT st.source_id, t.name FROM source_tags st \
                JOIN tags t ON t.id=st.tag_id";
    let mut collect = |rows: &mut dyn Iterator<Item = rusqlite::Result<(String, String)>>| {
        for row in rows {
            let (sid, name) = row?;
            out.entry(sid).or_default().push(name);
        }
        Ok::<_, Error>(())
    };
    match subject_id {
        Some(sub) => {
            let sql = format!(
                "{base} JOIN sources s ON s.id=st.source_id \
                 WHERE s.subject_id=?1 ORDER BY t.name"
            );
            let mut stmt = conn.prepare(&sql)?;
            let mut rows =
                stmt.query_map(params![sub], |r| Ok((r.get::<_, String>(0)?, r.get(1)?)))?;
            collect(&mut rows)?;
        }
        None => {
            let sql = format!("{base} ORDER BY t.name");
            let mut stmt = conn.prepare(&sql)?;
            let mut rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get(1)?)))?;
            collect(&mut rows)?;
        }
    }
    Ok(out)
}

// ---- global text search --------------------------------------------------

/// Case-insensitive substring matches across sources, notes, events and
/// materials (up to `per_kind` each), normalized into SearchHits for the
/// global Ctrl+K overlay. Semantic chunk search complements this in
/// commands::global_search.
pub fn text_search(conn: &Connection, query: &str, per_kind: usize) -> Result<Vec<SearchHit>> {
    let pat = format!("%{}%", query.replace('%', "\\%").replace('_', "\\_"));
    let mut out: Vec<SearchHit> = Vec::new();

    let mut stmt = conn.prepare(
        "SELECT id, subject_id, name, COALESCE(meta,'') FROM sources \
         WHERE name LIKE ?1 ESCAPE '\\' ORDER BY updated_at DESC LIMIT ?2",
    )?;
    let rows = stmt.query_map(params![pat, per_kind as i64], |r| {
        Ok(SearchHit {
            kind: "source".into(),
            id: r.get(0)?,
            subject_id: Some(r.get(1)?),
            title: r.get(2)?,
            snippet: r.get(3)?,
            score: 0.0,
        })
    })?;
    out.extend(rows.collect::<rusqlite::Result<Vec<_>>>()?);

    let mut stmt = conn.prepare(
        "SELECT id, subject_id, title, substr(body, 1, 160) FROM notes \
         WHERE title LIKE ?1 ESCAPE '\\' OR body LIKE ?1 ESCAPE '\\' \
         ORDER BY updated_at DESC LIMIT ?2",
    )?;
    let rows = stmt.query_map(params![pat, per_kind as i64], |r| {
        Ok(SearchHit {
            kind: "note".into(),
            id: r.get(0)?,
            subject_id: r.get(1)?,
            title: r.get(2)?,
            snippet: r.get(3)?,
            score: 0.0,
        })
    })?;
    out.extend(rows.collect::<rusqlite::Result<Vec<_>>>()?);

    let mut stmt = conn.prepare(
        "SELECT id, subject_id, title, COALESCE(location, '') FROM events \
         WHERE title LIKE ?1 ESCAPE '\\' ORDER BY start_ms DESC LIMIT ?2",
    )?;
    let rows = stmt.query_map(params![pat, per_kind as i64], |r| {
        Ok(SearchHit {
            kind: "event".into(),
            id: r.get(0)?,
            subject_id: r.get(1)?,
            title: r.get(2)?,
            snippet: r.get(3)?,
            score: 0.0,
        })
    })?;
    out.extend(rows.collect::<rusqlite::Result<Vec<_>>>()?);

    let mut stmt = conn.prepare(
        "SELECT id, subject_id, title, kind FROM materials \
         WHERE title LIKE ?1 ESCAPE '\\' ORDER BY created_at DESC LIMIT ?2",
    )?;
    let rows = stmt.query_map(params![pat, per_kind as i64], |r| {
        Ok(SearchHit {
            kind: "material".into(),
            id: r.get(0)?,
            subject_id: Some(r.get(1)?),
            title: r.get(2)?,
            snippet: r.get::<_, String>(3)?,
            score: 0.0,
        })
    })?;
    out.extend(rows.collect::<rusqlite::Result<Vec<_>>>()?);

    Ok(out)
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
    source_ids: Option<&[String]>,
    max_chars: usize,
) -> Result<(String, i64)> {
    // Build the WHERE clause (+ positional binds) once so the text query and the
    // source-count query stay in lock-step. `source_ids`, when present, is the
    // user's explicit selection and takes precedence — it scopes to exactly those
    // sources (across whatever topics), so a generation reflects what was picked.
    let mut where_sql = String::from(" WHERE c.subject_id = ?");
    let mut binds: Vec<String> = vec![subject_id.to_string()];
    if let Some(tid) = topic_id {
        where_sql.push_str(" AND c.topic_id = ?");
        binds.push(tid.to_string());
    }
    if let Some(ids) = source_ids.filter(|s| !s.is_empty()) {
        where_sql.push_str(" AND c.source_id IN (");
        for (i, id) in ids.iter().enumerate() {
            if i > 0 {
                where_sql.push(',');
            }
            where_sql.push('?');
            binds.push(id.clone());
        }
        where_sql.push(')');
    }

    let text_sql = format!(
        "SELECT c.source_id, c.text FROM chunks c{where_sql} ORDER BY c.source_id, c.ord"
    );
    let mut stmt = conn.prepare(&text_sql)?;
    let rows = stmt.query_map(rusqlite::params_from_iter(binds.iter()), |r| {
        Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
    })?;
    // Group chunks per source (query is ordered by source_id, ord), preserving order.
    let mut per_source: Vec<Vec<String>> = Vec::new();
    let mut cur_id: Option<String> = None;
    for row in rows {
        let (sid, t) = row?;
        if cur_id.as_deref() != Some(sid.as_str()) {
            per_source.push(Vec::new());
            cur_id = Some(sid);
        }
        per_source.last_mut().unwrap().push(t);
    }

    // Fair allocation: give every source at least its even share of the budget so
    // a few long sources can't crowd the rest out entirely (the old code `break`ed
    // at the first overflow and silently dropped every later source). A second
    // round-robin pass spends any leftover budget on sources that still have more.
    let n_sources = per_source.len().max(1);
    let per_source_budget = (max_chars / n_sources).max(1);
    let mut out = String::new();
    let mut idx: Vec<usize> = vec![0; per_source.len()]; // next-chunk cursor per source
    for (si, chunks) in per_source.iter().enumerate() {
        let mut used = 0usize;
        while idx[si] < chunks.len() {
            let t = &chunks[idx[si]];
            if used + t.len() > per_source_budget && used > 0 {
                break;
            }
            if out.len() + t.len() + 2 > max_chars {
                break;
            }
            out.push_str(t);
            out.push_str("\n\n");
            used += t.len() + 2;
            idx[si] += 1;
        }
    }
    // Round-robin the remainder so leftover budget is used without re-starving anyone.
    let mut progress = true;
    while progress && out.len() < max_chars {
        progress = false;
        for (si, chunks) in per_source.iter().enumerate() {
            if idx[si] >= chunks.len() {
                continue;
            }
            let t = &chunks[idx[si]];
            if out.len() + t.len() + 2 > max_chars {
                continue;
            }
            out.push_str(t);
            out.push_str("\n\n");
            idx[si] += 1;
            progress = true;
        }
    }

    let count_sql = format!("SELECT count(DISTINCT c.source_id) FROM chunks c{where_sql}");
    let src_count: i64 =
        conn.query_row(&count_sql, rusqlite::params_from_iter(binds.iter()), |r| r.get(0))?;
    Ok((out, src_count))
}

/// Source ids in a "bucket": a specific topic (`Some(topic_id)`) or the subject's
/// ungrouped sources (`None` → `topic_id IS NULL`, the "General" bucket). Ordered
/// by creation so generation is deterministic. Only sources that produced chunks
/// (i.e. have ingestable text) are returned — empty sources add nothing to cover.
pub fn bucket_source_ids(
    conn: &Connection,
    subject_id: &str,
    topic_id: Option<&str>,
) -> Result<Vec<String>> {
    let (sql, ids): (String, Vec<String>) = match topic_id {
        Some(tid) => (
            "SELECT s.id FROM sources s WHERE s.subject_id=?1 AND s.topic_id=?2 \
             AND EXISTS (SELECT 1 FROM chunks c WHERE c.source_id=s.id) ORDER BY s.created_at"
                .into(),
            vec![subject_id.to_string(), tid.to_string()],
        ),
        None => (
            "SELECT s.id FROM sources s WHERE s.subject_id=?1 AND s.topic_id IS NULL \
             AND EXISTS (SELECT 1 FROM chunks c WHERE c.source_id=s.id) ORDER BY s.created_at"
                .into(),
            vec![subject_id.to_string()],
        ),
    };
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(rusqlite::params_from_iter(ids.iter()), |r| r.get::<_, String>(0))?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r?);
    }
    Ok(out)
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
            "INSERT INTO cheatsheet_sections (id, cheatsheet_id, title, state, ord, body, image)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                new_id(),
                cid,
                sec.title,
                sec.state,
                i as i64,
                serde_json::to_string(&sec.items)?,
                sec.image,
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
        "SELECT id, title, state, body, image FROM cheatsheet_sections WHERE cheatsheet_id=?1 ORDER BY ord",
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
            image: r.get(4)?,
            image_query: None, // transient; never stored
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

/// Snapshot the full section set as a version for the git-like history/diff, then
/// prune to the most recent `KEEP` versions for that scope. Called on every save
/// (generation and manual edit) with a short `note` ("generated" / "edited").
pub fn snapshot_cheatsheet_version(
    conn: &Connection,
    subject_id: &str,
    topic_id: Option<&str>,
    sections: &[CsSection],
    note: &str,
) -> Result<()> {
    const KEEP: i64 = 20;
    conn.execute(
        "INSERT INTO cheatsheet_versions (id, subject_id, topic_id, created_at, note, sections)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            new_id(),
            subject_id,
            topic_id,
            now_ms(),
            note,
            serde_json::to_string(sections)?,
        ],
    )?;
    // Keep only the newest KEEP rows for this exact scope.
    conn.execute(
        "DELETE FROM cheatsheet_versions
         WHERE subject_id=?1 AND IFNULL(topic_id,'')=IFNULL(?2,'')
           AND id NOT IN (
             SELECT id FROM cheatsheet_versions
             WHERE subject_id=?1 AND IFNULL(topic_id,'')=IFNULL(?2,'')
             ORDER BY created_at DESC, rowid DESC LIMIT ?3
           )",
        params![subject_id, topic_id, KEEP],
    )?;
    Ok(())
}

/// List the stored versions for a scope, newest first (id, created_at ms, note,
/// section count) — without the heavy `sections` JSON.
pub fn list_cheatsheet_versions(
    conn: &Connection,
    subject_id: &str,
    topic_id: Option<&str>,
) -> Result<Vec<CheatsheetVersionMeta>> {
    let mut stmt = conn.prepare(
        "SELECT id, created_at, note, sections FROM cheatsheet_versions
         WHERE subject_id=?1 AND IFNULL(topic_id,'')=IFNULL(?2,'')
         ORDER BY created_at DESC, rowid DESC",
    )?;
    let rows = stmt.query_map(params![subject_id, topic_id], |r| {
        let sections: String = r.get(3)?;
        let count = serde_json::from_str::<Vec<CsSection>>(&sections)
            .map(|s| s.len())
            .unwrap_or(0);
        Ok(CheatsheetVersionMeta {
            id: r.get(0)?,
            created_at: r.get(1)?,
            note: r.get(2)?,
            section_count: count as i64,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

/// Read the full section set for a single stored version.
pub fn get_cheatsheet_version(conn: &Connection, version_id: &str) -> Result<Vec<CsSection>> {
    let sections: Option<String> = conn
        .query_row(
            "SELECT sections FROM cheatsheet_versions WHERE id=?1",
            params![version_id],
            |r| r.get(0),
        )
        .optional()?;
    Ok(sections
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default())
}

/// Read a version's owning scope (subject_id, topic_id) plus its sections — used
/// by restore, which needs to know which sheet to overwrite. Errors if the
/// version id doesn't exist.
pub fn get_cheatsheet_version_full(
    conn: &Connection,
    version_id: &str,
) -> Result<(String, Option<String>, Vec<CsSection>)> {
    let row: Option<(String, Option<String>, String)> = conn
        .query_row(
            "SELECT subject_id, topic_id, sections FROM cheatsheet_versions WHERE id=?1",
            params![version_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()?;
    let (subject_id, topic_id, sections_json) =
        row.ok_or_else(|| Error::Other(format!("version not found: {version_id}")))?;
    let sections: Vec<CsSection> = serde_json::from_str(&sections_json).unwrap_or_default();
    Ok((subject_id, topic_id, sections))
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

// ---- exams (timed, locally-graded practice exams) ---------------------

/// Map one `exams` row (the canonical 13-column SELECT order) to an `ExamRec`.
/// JSON columns are parsed leniently — a corrupt/empty value yields `Null` (or an
/// empty topic list) rather than failing the whole query.
fn row_to_exam(r: &rusqlite::Row) -> rusqlite::Result<ExamRec> {
    let topic_ids: Option<String> = r.get(2)?;
    let answers: Option<String> = r.get(6)?;
    let results: Option<String> = r.get(7)?;
    Ok(ExamRec {
        id: r.get(0)?,
        subject_id: r.get(1)?,
        topic_ids: topic_ids
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default(),
        title: r.get(3)?,
        duration_min: r.get(4)?,
        questions: serde_json::from_str(&r.get::<_, String>(5)?)
            .unwrap_or(serde_json::Value::Null),
        answers: answers
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or(serde_json::Value::Null),
        results: results
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or(serde_json::Value::Null),
        status: r.get(8)?,
        started_ms: r.get(9)?,
        score: r.get(10)?,
        created_at: r.get(11)?,
        updated_at: r.get(12)?,
    })
}

const EXAM_COLS: &str = "id, subject_id, topic_ids, title, duration_min, questions, \
    answers, results, status, started_ms, score, created_at, updated_at";

/// Persist a freshly-generated exam (status 'ready') and return its id.
pub fn insert_exam(
    conn: &Connection,
    subject_id: &str,
    topic_ids: &[String],
    title: &str,
    duration_min: i64,
    questions: &serde_json::Value,
) -> Result<String> {
    let id = new_id();
    let ts = now_ms();
    let topics_json = serde_json::to_string(topic_ids).unwrap_or_else(|_| "[]".into());
    conn.execute(
        "INSERT INTO exams (id, subject_id, topic_ids, title, duration_min, questions, \
         status, created_at, updated_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'ready', ?7, ?7)",
        params![id, subject_id, topics_json, title, duration_min, questions.to_string(), ts],
    )?;
    Ok(id)
}

/// Fetch a single exam by id. Errors if not found.
pub fn get_exam(conn: &Connection, id: &str) -> Result<ExamRec> {
    conn.query_row(
        &format!("SELECT {EXAM_COLS} FROM exams WHERE id=?1"),
        params![id],
        row_to_exam,
    )
    .optional()?
    .ok_or_else(|| Error::NotFound(format!("exam {id}")))
}

/// A subject's exams, newest first (drives the setup screen's past-exam list).
pub fn list_exams(conn: &Connection, subject_id: &str) -> Result<Vec<ExamRec>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {EXAM_COLS} FROM exams WHERE subject_id=?1 ORDER BY created_at DESC"
    ))?;
    let rows = stmt.query_map(params![subject_id], row_to_exam)?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

/// Mark an exam started: status 'in_progress' + the start timestamp (idempotent —
/// re-starting keeps the original start time so the countdown stays honest).
pub fn start_exam(conn: &Connection, id: &str) -> Result<()> {
    let ts = now_ms();
    let n = conn.execute(
        "UPDATE exams SET status='in_progress', \
         started_ms=COALESCE(started_ms, ?2), updated_at=?2 \
         WHERE id=?1 AND status<>'graded'",
        params![id, ts],
    )?;
    if n == 0 {
        // Either no such exam, or it's already graded — surface a clear error only
        // when the row is genuinely missing.
        let exists: bool = conn
            .query_row("SELECT 1 FROM exams WHERE id=?1", params![id], |_| Ok(true))
            .optional()?
            .unwrap_or(false);
        if !exists {
            return Err(Error::NotFound(format!("exam {id}")));
        }
    }
    Ok(())
}

/// Persist a graded submission: the student's answers, the grading results, the
/// final score %, and status 'graded'.
pub fn finalize_exam(
    conn: &Connection,
    id: &str,
    answers: &serde_json::Value,
    results: &serde_json::Value,
    score: f64,
) -> Result<()> {
    let n = conn.execute(
        "UPDATE exams SET answers=?2, results=?3, score=?4, status='graded', updated_at=?5 \
         WHERE id=?1",
        params![id, answers.to_string(), results.to_string(), score, now_ms()],
    )?;
    if n == 0 {
        return Err(Error::NotFound(format!("exam {id}")));
    }
    Ok(())
}

/// Delete an exam.
pub fn delete_exam(conn: &Connection, id: &str) -> Result<()> {
    conn.execute("DELETE FROM exams WHERE id=?1", params![id])?;
    Ok(())
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

// ---- custom music stations ---------------------------------------------

pub fn list_custom_stations(conn: &Connection) -> Result<Vec<CustomStation>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, url, kind, position, created_at
           FROM custom_stations ORDER BY position, created_at",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok(CustomStation {
            id: r.get(0)?,
            name: r.get(1)?,
            url: r.get(2)?,
            kind: r.get(3)?,
            position: r.get(4)?,
            created_at: r.get(5)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

pub fn insert_custom_station(
    conn: &Connection,
    name: &str,
    url: &str,
    kind: &str,
) -> Result<CustomStation> {
    let id = new_id();
    let ts = now_ms();
    // Append to the end of the list.
    let pos: i64 = conn
        .query_row("SELECT COALESCE(MAX(position) + 1, 0) FROM custom_stations", [], |r| r.get(0))
        .unwrap_or(0);
    conn.execute(
        "INSERT INTO custom_stations (id, name, url, kind, position, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![id, name, url, kind, pos, ts],
    )?;
    Ok(CustomStation {
        id,
        name: name.to_string(),
        url: url.to_string(),
        kind: kind.to_string(),
        position: pos,
        created_at: ts,
    })
}

pub fn delete_custom_station(conn: &Connection, id: &str) -> Result<()> {
    conn.execute("DELETE FROM custom_stations WHERE id=?1", params![id])?;
    Ok(())
}

/// Persist a new ordering for custom stations: each id's `position` becomes its
/// index in `ids` (so list_custom_stations returns them in this order).
pub fn reorder_custom_stations(conn: &Connection, ids: &[String]) -> Result<()> {
    let tx = conn.unchecked_transaction()?;
    for (i, id) in ids.iter().enumerate() {
        tx.execute(
            "UPDATE custom_stations SET position=?2 WHERE id=?1",
            params![id, i as i64],
        )?;
    }
    tx.commit()?;
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

/// Tags are stored as a ';'-separated text list (tags shouldn't contain ';').
pub fn tags_to_text(tags: &[String]) -> Option<String> {
    let t: Vec<&str> = tags.iter().map(|s| s.trim()).filter(|s| !s.is_empty()).collect();
    if t.is_empty() { None } else { Some(t.join(";")) }
}
fn text_to_tags(s: Option<String>) -> Vec<String> {
    s.map(|s| {
        s.split(';')
            .map(|x| x.trim().to_string())
            .filter(|x| !x.is_empty())
            .collect()
    })
    .unwrap_or_default()
}
/// The deadline checklist (done topic ids) is stored as a JSON array.
fn text_to_ids(s: Option<String>) -> Vec<String> {
    s.and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}

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
        tags: text_to_tags(r.get(16)?),
        checklist: text_to_ids(r.get(17)?),
        priority: r.get(18)?,
        topic_ids: text_to_tags(r.get(19)?),
        created_at: r.get(14)?,
        updated_at: r.get(15)?,
    })
}

const EVENT_COLS: &str = "id, subject_id, title, description, location, color, start_ms, end_ms, \
    all_day, kind, done, reminder_ms, notified, google_id, created_at, updated_at, tags, checklist, \
    priority, topic_ids";

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
    tags: &[String],
    priority: Option<&str>,
    topic_ids: &[String],
) -> Result<String> {
    let id = new_id();
    let ts = now_ms();
    conn.execute(
        "INSERT INTO events
            (id, subject_id, title, description, location, color, start_ms, end_ms,
             all_day, kind, done, reminder_ms, notified, created_at, updated_at, tags,
             priority, topic_ids)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 0, ?11, 0, ?12, ?12, ?13, ?14, ?15)",
        params![
            id, subject_id, title, description, location, color, start_ms, end_ms,
            all_day as i64, kind, reminder_ms, ts, tags_to_text(tags),
            priority, tags_to_text(topic_ids)
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
    tags: &[String],
    priority: Option<&str>,
    topic_ids: &[String],
) -> Result<()> {
    // Editing an event resets its notified flag so a moved reminder fires again.
    let n = conn.execute(
        "UPDATE events SET
            title=?2, description=?3, location=?4, color=?5, start_ms=?6, end_ms=?7,
            all_day=?8, kind=?9, reminder_ms=?10, notified=0, updated_at=?11, tags=?12,
            priority=?13, topic_ids=?14
         WHERE id=?1",
        params![
            id, title, description, location, color, start_ms, end_ms,
            all_day as i64, kind, reminder_ms, now_ms(), tags_to_text(tags),
            priority, tags_to_text(topic_ids)
        ],
    )?;
    if n == 0 {
        return Err(Error::NotFound(format!("event {id}")));
    }
    Ok(())
}

/// Set the deadline study checklist (the topic ids ticked off for this event).
pub fn set_event_checklist(conn: &Connection, id: &str, topic_ids: &[String]) -> Result<()> {
    let json = serde_json::to_string(topic_ids).unwrap_or_else(|_| "[]".into());
    let n = conn.execute(
        "UPDATE events SET checklist=?2, updated_at=?3 WHERE id=?1",
        params![id, json, now_ms()],
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

// ---- FSRS spaced repetition -------------------------------------------
//
// FSRS-4.5 (open spaced-repetition scheduler): each card carries a memory
// `stability` (days until recall probability drops to 90%) and `difficulty`
// (1-10). Replaces SM-2, whose fixed 1→6→×ease ladder badly over/under-spaces
// real retention. Default published parameters; same grading API as before.

const FSRS_W: [f64; 17] = [
    0.4872, 1.4003, 3.7145, 13.8206, 5.1618, 1.2298, 0.8975, 0.0310, 1.6474,
    0.1367, 1.0461, 2.1072, 0.0793, 0.3246, 1.5870, 0.2272, 2.8755,
];
const FSRS_DECAY: f64 = -0.5;
const FSRS_FACTOR: f64 = 19.0 / 81.0;
const FSRS_RETENTION: f64 = 0.9; // schedule reviews at the 90% recall point

/// First-review stability for grade `g` (1=Again … 4=Easy).
fn fsrs_init_stability(g: usize) -> f64 {
    FSRS_W[g - 1].max(0.1)
}
fn fsrs_init_difficulty(g: usize) -> f64 {
    (FSRS_W[4] - (g as f64 - 3.0) * FSRS_W[5]).clamp(1.0, 10.0)
}
fn fsrs_next_difficulty(d: f64, g: usize) -> f64 {
    let next = d - FSRS_W[6] * (g as f64 - 3.0);
    // mean-reverts toward the initial "Easy" difficulty so D can't run away
    (FSRS_W[7] * fsrs_init_difficulty(4) + (1.0 - FSRS_W[7]) * next).clamp(1.0, 10.0)
}
/// Probability of recall after `elapsed_d` days at stability `s`.
fn fsrs_retrievability(elapsed_d: f64, s: f64) -> f64 {
    (1.0 + FSRS_FACTOR * elapsed_d / s.max(0.1)).powf(FSRS_DECAY)
}
fn fsrs_next_stability(d: f64, s: f64, r: f64, g: usize) -> f64 {
    if g == 1 {
        // post-lapse stability: shrinks, never exceeds what it was
        let sf = FSRS_W[11]
            * d.powf(-FSRS_W[12])
            * ((s + 1.0).powf(FSRS_W[13]) - 1.0)
            * (FSRS_W[14] * (1.0 - r)).exp();
        sf.min(s).max(0.1)
    } else {
        let hard = if g == 2 { FSRS_W[15] } else { 1.0 };
        let easy = if g == 4 { FSRS_W[16] } else { 1.0 };
        let grow = FSRS_W[8].exp()
            * (11.0 - d)
            * s.powf(-FSRS_W[9])
            * ((FSRS_W[10] * (1.0 - r)).exp() - 1.0);
        (s * (1.0 + grow * hard * easy)).max(0.1)
    }
}
/// Days until recall probability decays to FSRS_RETENTION.
fn fsrs_interval(s: f64) -> i64 {
    let days = s / FSRS_FACTOR * (FSRS_RETENTION.powf(1.0 / FSRS_DECAY) - 1.0);
    (days.round() as i64).clamp(1, 36_500)
}

/// Grade a card with FSRS and upsert its schedule. `quality` is 0-5 from the
/// existing UI (Again≈1, Hard≈3, Good≈4, Easy≈5), mapped to FSRS's four grades.
/// Also logs an `attempts` row (correct = quality >= 3) so the legacy "review
/// missed" set keeps working. Returns the new schedule.
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

    // Current schedule, or defaults for a brand-new card. stability/difficulty
    // are NULL on rows last scheduled under SM-2 — seeded below.
    #[allow(clippy::type_complexity)]
    let existing: Option<(f64, i64, i64, i64, Option<f64>, Option<f64>, i64)> = conn
        .query_row(
            "SELECT ease, interval_d, reps, lapses, stability, difficulty, updated_at
             FROM srs_cards WHERE subject_id=?1 AND kind=?2 AND item_key=?3",
            params![subject_id, kind, item_key],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?)),
        )
        .optional()?;
    let is_new = existing.is_none();
    let (mut ease, mut interval_d, mut reps, mut lapses, stability, difficulty, last_seen) =
        existing.unwrap_or((2.5, 0, 0, 0, None, None, now));

    let g: usize = match q {
        0..=2 => 1, // Again
        3 => 2,     // Hard
        4 => 3,     // Good
        _ => 4,     // Easy
    };
    let (s, d) = if is_new {
        (fsrs_init_stability(g), fsrs_init_difficulty(g))
    } else {
        // SM-2 rows seed FSRS from their interval (≈ stability at 90% retention)
        // and a rough ease→difficulty mapping; FSRS self-corrects from there.
        let s0 = stability.unwrap_or((interval_d as f64).max(0.5));
        let d0 = difficulty.unwrap_or((11.0 - 3.0 * ease).clamp(1.0, 10.0));
        let elapsed_d = ((now - last_seen) as f64 / 86_400_000.0).max(0.0);
        let r = fsrs_retrievability(elapsed_d, s0);
        (fsrs_next_stability(d0, s0, r, g), fsrs_next_difficulty(d0, g))
    };

    if g == 1 {
        // Lapse: relearn tomorrow (stability already shrunk by the forget branch).
        reps = 0;
        interval_d = 1;
        lapses += 1;
    } else {
        reps += 1;
        interval_d = fsrs_interval(s);
    }
    // Keep the SM-2 ease as a legacy display value so existing UI stays stable.
    let qf = q as f64;
    ease = (ease + (0.1 - (5.0 - qf) * (0.08 + (5.0 - qf) * 0.02))).max(1.3);
    let due_at = now + interval_d * 86_400_000;

    conn.execute(
        "INSERT INTO srs_cards
            (id, subject_id, material_id, kind, item_index, item_key,
             ease, interval_d, reps, lapses, last_grade, due_at, created_at, updated_at,
             stability, difficulty)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?13,?14,?15)
         ON CONFLICT(subject_id, kind, item_key) DO UPDATE SET
            material_id=excluded.material_id, item_index=excluded.item_index,
            ease=excluded.ease, interval_d=excluded.interval_d, reps=excluded.reps,
            lapses=excluded.lapses, last_grade=excluded.last_grade,
            due_at=excluded.due_at, updated_at=excluded.updated_at,
            stability=excluded.stability, difficulty=excluded.difficulty",
        params![
            new_id(), subject_id, material_id, kind, item_index, item_key,
            ease, interval_d, reps, lapses, q, due_at, now, s, d
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

// ---- pomodoro sessions + study analytics ------------------------------

/// Record one study segment. `kind` is "work" (a finished pomodoro focus
/// phase), "break" (logged for completeness), or "app" (passive focused
/// in-app time, accumulated while the window is visible+focused). Both "work"
/// and "app" rows count toward study minutes in the analytics dashboard.
pub fn insert_pomodoro_session(
    conn: &Connection,
    subject_id: Option<&str>,
    kind: &str,
    started_ms: i64,
    ended_ms: i64,
) -> Result<String> {
    let id = new_id();
    conn.execute(
        "INSERT INTO pomodoro_sessions
            (id, subject_id, kind, started_ms, ended_ms, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![id, subject_id, kind, started_ms, ended_ms, now_ms()],
    )?;
    Ok(id)
}

/// Build the whole Study Analytics dashboard in one pass.
///
/// `days` bounds the per-day charts (study minutes, reviews/accuracy) and the
/// per-subject roll-up. Per-day buckets use the LOCAL calendar date so they
/// match what the user sees, not UTC midnight. SQL returns only days/subjects
/// with activity; we fill the gaps (and the 7-day due forecast) in Rust so the
/// charts always span a contiguous range. Everything runs on the single
/// connection the caller already holds — one lock for the whole dashboard.
pub fn analytics_summary(conn: &Connection, days: i64) -> Result<AnalyticsSummary> {
    let days = days.clamp(1, 365);
    let today_floor = day_floor_ms(now_ms());
    // Inclusive window start at local midnight, `days` days ago (so a 30-day
    // window covers today plus the previous 29 days).
    let since_ms = today_floor - (days - 1) * DAY_MS;
    // The heatmap always spans a full rolling year (366 days, leap-safe).
    let year_since_ms = today_floor - (YEAR_DAYS - 1) * DAY_MS;

    // ── per-day study minutes (work + passive app segments) ──
    // Minutes are summed from each segment's own duration so a partially-skipped
    // session still contributes its real elapsed time. "app" rows are passive
    // focused in-app time (e.g. studying the cheatsheet) so study time isn't 0
    // for users who never run a pomodoro. We query the FULL YEAR once and derive
    // both the windowed `minutes_per_day` and the year-long heatmap from it.
    let mut stmt = conn.prepare(
        "SELECT date(started_ms/1000, 'unixepoch', 'localtime') AS d,
                SUM(ended_ms - started_ms) AS ms
         FROM pomodoro_sessions
         WHERE kind IN ('work','app') AND started_ms >= ?1
         GROUP BY d",
    )?;
    let mut minutes_by_day: std::collections::HashMap<String, i64> = std::collections::HashMap::new();
    for row in stmt.query_map(params![year_since_ms], |r| {
        Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))
    })? {
        let (d, ms) = row?;
        minutes_by_day.insert(d, ms / 60_000);
    }

    // Full-year daily series for the contributions heatmap (oldest → newest).
    let mut year_minutes = Vec::with_capacity(YEAR_DAYS as usize);
    for i in 0..YEAR_DAYS {
        let day = local_day_str(year_since_ms + i * DAY_MS);
        let minutes = *minutes_by_day.get(&day).unwrap_or(&0);
        year_minutes.push(DayMinutes { day, minutes });
    }

    // ── per-day reviews + accuracy (attempts) ──
    let mut stmt = conn.prepare(
        "SELECT date(created_at/1000, 'unixepoch', 'localtime') AS d,
                COUNT(*) AS n,
                SUM(correct) AS ok
         FROM attempts
         WHERE created_at >= ?1
         GROUP BY d",
    )?;
    let mut reviews_by_day: std::collections::HashMap<String, (i64, i64)> =
        std::collections::HashMap::new();
    for row in stmt.query_map(params![since_ms], |r| {
        Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?, r.get::<_, i64>(2)?))
    })? {
        let (d, n, ok) = row?;
        reviews_by_day.insert(d, (n, ok));
    }

    // Walk the contiguous day range oldest → newest, filling zeros for gaps.
    let mut minutes_per_day = Vec::with_capacity(days as usize);
    let mut reviews_per_day = Vec::with_capacity(days as usize);
    for i in 0..days {
        let day = local_day_str(since_ms + i * DAY_MS);
        let minutes = *minutes_by_day.get(&day).unwrap_or(&0);
        minutes_per_day.push(DayMinutes { day: day.clone(), minutes });
        let (reviews, correct) = *reviews_by_day.get(&day).unwrap_or(&(0, 0));
        let accuracy = if reviews > 0 { correct as f64 / reviews as f64 } else { 0.0 };
        reviews_per_day.push(DayReviews { day, reviews, correct, accuracy });
    }

    // ── current streak: consecutive days ending today with ANY activity ──
    let today = local_day_str(now_ms());
    let mut streak = 0i64;
    let mut cursor = day_floor_ms(now_ms());
    loop {
        let day = local_day_str(cursor);
        let had_work = *minutes_by_day.get(&day).unwrap_or(&0) > 0;
        let had_review = reviews_by_day.get(&day).map(|(n, _)| *n > 0).unwrap_or(false);
        if had_work || had_review {
            streak += 1;
            cursor -= DAY_MS;
        } else {
            // Today with no activity yet doesn't break a streak earned yesterday:
            // skip today once, then require unbroken activity backward.
            if day == today {
                cursor -= DAY_MS;
                continue;
            }
            break;
        }
        // Stop once we walk past the full-year window (no data beyond it). The
        // year of study-minute data is loaded above, so a long streak counts.
        if cursor < year_since_ms {
            break;
        }
    }

    // ── per-subject roll-up (minutes + reviews + accuracy) over the window ──
    // Two grouped queries merged by subject id; avoids a cross-join double count.
    let mut subj: std::collections::HashMap<String, SubjectStat> = std::collections::HashMap::new();
    let mut stmt = conn.prepare(
        "SELECT subject_id, SUM(ended_ms - started_ms) AS ms
         FROM pomodoro_sessions
         WHERE kind IN ('work','app') AND started_ms >= ?1 AND subject_id IS NOT NULL
         GROUP BY subject_id",
    )?;
    for row in stmt.query_map(params![since_ms], |r| {
        Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))
    })? {
        let (sid, ms) = row?;
        subj.entry(sid.clone())
            .or_insert_with(|| SubjectStat {
                subject_id: sid,
                minutes: 0,
                reviews: 0,
                correct: 0,
                accuracy: 0.0,
            })
            .minutes = ms / 60_000;
    }
    let mut stmt = conn.prepare(
        "SELECT subject_id, COUNT(*) AS n, SUM(correct) AS ok
         FROM attempts
         WHERE created_at >= ?1
         GROUP BY subject_id",
    )?;
    for row in stmt.query_map(params![since_ms], |r| {
        Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?, r.get::<_, i64>(2)?))
    })? {
        let (sid, n, ok) = row?;
        let e = subj.entry(sid.clone()).or_insert_with(|| SubjectStat {
            subject_id: sid,
            minutes: 0,
            reviews: 0,
            correct: 0,
            accuracy: 0.0,
        });
        e.reviews = n;
        e.correct = ok;
    }
    let mut per_subject: Vec<SubjectStat> = subj.into_values().collect();
    for s in per_subject.iter_mut() {
        s.accuracy = if s.reviews > 0 { s.correct as f64 / s.reviews as f64 } else { 0.0 };
    }
    // Most-studied first, then most-reviewed — stable, useful ordering for a table.
    per_subject.sort_by(|a, b| {
        b.minutes
            .cmp(&a.minutes)
            .then(b.reviews.cmp(&a.reviews))
            .then(a.subject_id.cmp(&b.subject_id))
    });

    // ── due forecast: cards becoming due each of the next 7 days ──
    let mut stmt = conn.prepare(
        "SELECT date(due_at/1000, 'unixepoch', 'localtime') AS d, COUNT(*) AS n
         FROM srs_cards
         WHERE due_at >= ?1 AND due_at < ?2
         GROUP BY d",
    )?;
    let forecast_start = day_floor_ms(now_ms());
    let forecast_end = forecast_start + 7 * DAY_MS;
    let mut due_by_day: std::collections::HashMap<String, i64> = std::collections::HashMap::new();
    for row in stmt.query_map(params![forecast_start, forecast_end], |r| {
        Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))
    })? {
        let (d, n) = row?;
        due_by_day.insert(d, n);
    }
    let mut due_forecast = Vec::with_capacity(7);
    for i in 0..7 {
        let day = local_day_str(forecast_start + i * DAY_MS);
        let due = *due_by_day.get(&day).unwrap_or(&0);
        due_forecast.push(DueDay { day, due });
    }

    // ── FSRS totals (all scheduled cards, not windowed) ──
    let cards: i64 = conn.query_row("SELECT COUNT(*) FROM srs_cards", [], |r| r.get(0))?;
    let lapses: i64 = conn
        .query_row("SELECT COALESCE(SUM(lapses), 0) FROM srs_cards", [], |r| r.get(0))?;
    // Only average over cards that actually carry an FSRS stability (legacy SM-2
    // rows have NULL until first re-graded under FSRS).
    let avg_stability: f64 = conn
        .query_row(
            "SELECT COALESCE(AVG(stability), 0.0) FROM srs_cards WHERE stability IS NOT NULL",
            [],
            |r| r.get(0),
        )?;
    let fsrs = FsrsTotals { cards, avg_stability, lapses };

    // ── rolling 7-day headline figures ──
    let week_start = day_floor_ms(now_ms()) - 6 * DAY_MS;
    let minutes_week: i64 = conn.query_row(
        "SELECT COALESCE(SUM(ended_ms - started_ms), 0) / 60000
         FROM pomodoro_sessions WHERE kind IN ('work','app') AND started_ms >= ?1",
        params![week_start],
        |r| r.get(0),
    )?;
    let (reviews_week, correct_week): (i64, i64) = conn.query_row(
        "SELECT COUNT(*), COALESCE(SUM(correct), 0) FROM attempts WHERE created_at >= ?1",
        params![week_start],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    let accuracy_week = if reviews_week > 0 {
        correct_week as f64 / reviews_week as f64
    } else {
        0.0
    };

    let weak_topics = weak_topics(conn, since_ms)?;

    Ok(AnalyticsSummary {
        minutes_per_day,
        year_minutes,
        reviews_per_day,
        due_forecast,
        per_subject,
        weak_topics,
        fsrs,
        streak,
        minutes_week,
        reviews_week,
        accuracy_week,
    })
}

/// Rank topics that need the most work, blending low review accuracy, high
/// lapses, and low FSRS stability. Attribution flows attempts/cards → materials
/// → topics: rows whose material has no `topic_id` (or no material) are simply
/// skipped — we never guess. Returns the weakest ~8 across all subjects.
fn weak_topics(conn: &Connection, since_ms: i64) -> Result<Vec<WeakTopic>> {
    // Accumulator keyed by topic id, carrying its subject + name and the merged
    // attempt/card signals.
    struct Acc {
        subject_id: String,
        topic_name: String,
        reviews: i64,
        correct: i64,
        lapses: i64,
        stab_sum: f64,
        stab_n: i64,
    }
    let mut by_topic: std::collections::HashMap<String, Acc> = std::collections::HashMap::new();

    // ── review attempts attributed via the answered material's topic ──
    let mut stmt = conn.prepare(
        "SELECT t.id, t.subject_id, t.name, COUNT(*) AS n, SUM(a.correct) AS ok
         FROM attempts a
         JOIN materials m ON m.id = a.material_id
         JOIN topics t ON t.id = m.topic_id
         WHERE a.created_at >= ?1 AND m.topic_id IS NOT NULL
         GROUP BY t.id",
    )?;
    for row in stmt.query_map(params![since_ms], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, String>(2)?,
            r.get::<_, i64>(3)?,
            r.get::<_, i64>(4)?,
        ))
    })? {
        let (tid, sid, name, n, ok) = row?;
        let e = by_topic.entry(tid).or_insert_with(|| Acc {
            subject_id: sid,
            topic_name: name,
            reviews: 0,
            correct: 0,
            lapses: 0,
            stab_sum: 0.0,
            stab_n: 0,
        });
        e.reviews += n;
        e.correct += ok;
    }

    // ── FSRS lapses + stability attributed via each card's material topic ──
    let mut stmt = conn.prepare(
        "SELECT t.id, t.subject_id, t.name,
                COALESCE(SUM(c.lapses), 0) AS lapses,
                COALESCE(SUM(c.stability), 0.0) AS stab_sum,
                COUNT(c.stability) AS stab_n
         FROM srs_cards c
         JOIN materials m ON m.id = c.material_id
         JOIN topics t ON t.id = m.topic_id
         WHERE m.topic_id IS NOT NULL
         GROUP BY t.id",
    )?;
    for row in stmt.query_map([], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, String>(2)?,
            r.get::<_, i64>(3)?,
            r.get::<_, f64>(4)?,
            r.get::<_, i64>(5)?,
        ))
    })? {
        let (tid, sid, name, lapses, stab_sum, stab_n) = row?;
        let e = by_topic.entry(tid).or_insert_with(|| Acc {
            subject_id: sid,
            topic_name: name,
            reviews: 0,
            correct: 0,
            lapses: 0,
            stab_sum: 0.0,
            stab_n: 0,
        });
        e.lapses += lapses;
        e.stab_sum += stab_sum;
        e.stab_n += stab_n;
    }

    // Score each topic: higher == weaker. Three independent signals, each scaled
    // to ~0..1 so none dominates, then summed:
    //   • inaccuracy  = 1 - accuracy            (only meaningful with reviews)
    //   • lapse load  = lapses / (lapses + 3)   (saturating, so 1 lapse ≠ 10)
    //   • fragility   = 1 - stability/(stability+14)  (low stability → high)
    let mut scored: Vec<(f64, WeakTopic)> = by_topic
        .into_iter()
        .map(|(topic_id, a)| {
            let accuracy = if a.reviews > 0 { a.correct as f64 / a.reviews as f64 } else { 0.0 };
            let avg_stability = if a.stab_n > 0 { a.stab_sum / a.stab_n as f64 } else { 0.0 };

            let inaccuracy = if a.reviews > 0 { 1.0 - accuracy } else { 0.0 };
            let lapse_load = a.lapses as f64 / (a.lapses as f64 + 3.0);
            // Only penalize fragility when we actually have a stability reading.
            let fragility = if a.stab_n > 0 {
                1.0 - avg_stability / (avg_stability + 14.0)
            } else {
                0.0
            };
            let score = inaccuracy + lapse_load + fragility;

            // A short, human reason naming the dominant weakness(es).
            let mut bits: Vec<String> = Vec::new();
            if a.reviews > 0 && accuracy < 0.7 {
                bits.push(format!("{}% accuracy", (accuracy * 100.0).round() as i64));
            }
            if a.lapses > 0 {
                bits.push(format!("{} lapse{}", a.lapses, if a.lapses == 1 { "" } else { "s" }));
            }
            if a.stab_n > 0 && avg_stability < 7.0 {
                bits.push("low retention".into());
            }
            let reason = if bits.is_empty() { "Needs review".into() } else { bits.join(" · ") };

            (
                score,
                WeakTopic {
                    subject_id: a.subject_id,
                    topic_id,
                    topic_name: a.topic_name,
                    reviews: a.reviews,
                    correct: a.correct,
                    accuracy,
                    lapses: a.lapses,
                    avg_stability,
                    reason,
                },
            )
        })
        // Drop topics with no weakness signal at all (perfect & stable → not "weak").
        .filter(|(score, _)| *score > 0.0)
        .collect();

    // Weakest first; ties broken by topic name so ordering is stable run-to-run.
    scored.sort_by(|a, b| {
        b.0.partial_cmp(&a.0)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.1.topic_name.cmp(&b.1.topic_name))
    });

    Ok(scored.into_iter().take(8).map(|(_, wt)| wt).collect())
}

/// Milliseconds in a day — analytics steps the day cursor by this.
const DAY_MS: i64 = 86_400_000;

/// Days in the contributions heatmap window (a full rolling year, leap-safe).
const YEAR_DAYS: i64 = 366;

/// Local-midnight (ms epoch) of the day containing `ms`. Uses SQLite's own
/// 'localtime' conversion via a tiny helper query so the day boundaries match
/// the GROUP BY date() buckets exactly (same TZ rules), avoiding off-by-one
/// drift between Rust and SQLite timezone handling.
fn day_floor_ms(ms: i64) -> i64 {
    // Fallback to a crude UTC floor only if the (always-available) datetime
    // functions somehow fail; correctness here just affects bucket alignment.
    LOCAL_MIDNIGHT
        .with(|c| {
            let conn = c.borrow();
            conn.query_row(
                "SELECT CAST(strftime('%s', date(?1/1000, 'unixepoch', 'localtime')) AS INTEGER) * 1000",
                params![ms],
                |r| r.get::<_, i64>(0),
            )
            .ok()
        })
        .unwrap_or_else(|| ms - ms.rem_euclid(DAY_MS))
}

/// Local-date string ("YYYY-MM-DD") for `ms` — matches the SQL date() buckets.
fn local_day_str(ms: i64) -> String {
    LOCAL_MIDNIGHT
        .with(|c| {
            let conn = c.borrow();
            conn.query_row(
                "SELECT date(?1/1000, 'unixepoch', 'localtime')",
                params![ms],
                |r| r.get::<_, String>(0),
            )
            .ok()
        })
        .unwrap_or_default()
}

thread_local! {
    // A scratch in-memory connection used purely for SQLite's date/time
    // functions (timezone-correct day math). Cheap, thread-local, no schema —
    // keeps the day-bucket helpers in lockstep with the GROUP BY queries above
    // without borrowing the app connection (which the caller already holds).
    static LOCAL_MIDNIGHT: std::cell::RefCell<Connection> =
        std::cell::RefCell::new(Connection::open_in_memory().expect("scratch conn"));
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
        let tid = insert_topic(&c, &sid, "Recursion", None, &[]).unwrap();
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
    fn srs_fsrs_schedule_progresses_and_lapses() {
        let st = AppState::in_memory().unwrap();
        let c = st.db.lock().unwrap();
        let sid = insert_subject(&c, "Bio", None, None, None).unwrap();
        let key = "What is ATP?";

        // First "Good" grade: new card → interval ≈ initial Good stability (~4d).
        let r1 = srs_grade(&c, &sid, None, "flashcard", 0, key, 4).unwrap();
        assert_eq!(r1.reps, 1);
        assert!(r1.interval_d >= 1, "got {}", r1.interval_d);

        // Repeated grades at the SAME instant: retrievability is still 1.0, so
        // FSRS (correctly) grants no stability gain — interval must not shrink.
        // Growth-with-elapsed-time is asserted in fsrs_math_is_sane.
        let r2 = srs_grade(&c, &sid, None, "flashcard", 0, key, 4).unwrap();
        assert_eq!(r2.reps, 2);
        assert!(r2.interval_d >= r1.interval_d, "{} < {}", r2.interval_d, r1.interval_d);
        let r3 = srs_grade(&c, &sid, None, "flashcard", 0, key, 5).unwrap();
        assert_eq!(r3.reps, 3);
        assert!(r3.interval_d >= r2.interval_d, "{} < {}", r3.interval_d, r2.interval_d);

        // Exactly one schedule row (upsert, not insert-per-grade); 1 total card.
        assert_eq!(srs_stats(&c, &sid, "flashcard").unwrap().total, 1);

        // "Again" lapse resets reps to 0 and relearns tomorrow.
        let r4 = srs_grade(&c, &sid, None, "flashcard", 0, key, 1).unwrap();
        assert_eq!(r4.reps, 0);
        assert_eq!(r4.interval_d, 1);

        // The legacy "wrong items" set picks up the lapse (latest attempt wrong).
        assert_eq!(wrong_items(&c, &sid, "flashcard").unwrap().len(), 1);
    }

    #[test]
    fn fsrs_math_is_sane() {
        // Initial stabilities are ordered Again < Hard < Good < Easy.
        assert!(fsrs_init_stability(1) < fsrs_init_stability(2));
        assert!(fsrs_init_stability(2) < fsrs_init_stability(3));
        assert!(fsrs_init_stability(3) < fsrs_init_stability(4));
        // Difficulty stays in [1,10] and Easy reduces it.
        let d = fsrs_init_difficulty(3);
        assert!((1.0..=10.0).contains(&d));
        assert!(fsrs_next_difficulty(d, 4) < d);
        assert!(fsrs_next_difficulty(d, 1) > d);
        // Retrievability decays with elapsed time.
        assert!(fsrs_retrievability(0.0, 5.0) > fsrs_retrievability(10.0, 5.0));
        // Successful review grows stability; a lapse shrinks it.
        let s = 10.0;
        let r = fsrs_retrievability(10.0, s);
        assert!(fsrs_next_stability(d, s, r, 3) > s);
        assert!(fsrs_next_stability(d, s, r, 1) < s);
        // At 90% retention the interval is ≈ the stability (FSRS design point).
        assert_eq!(fsrs_interval(10.0), 10);
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
            &c, None, "Exam", None, None, None, 1_000, Some(2_000), false, "event", Some(500), &[], None, &[],
        )
        .unwrap();
        let _future = insert_event(
            &c, None, "Later", None, None, None, 9_000, None, false, "task", Some(8_000), &[], None, &[],
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
        let bt = insert_topic(&c, &b, "T", None, &[]).unwrap();
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

    #[test]
    fn analytics_summary_rolls_up_minutes_reviews_and_streak() {
        let st = AppState::in_memory().unwrap();
        let c = st.db.lock().unwrap();
        let sid = insert_subject(&c, "Math", None, None, None).unwrap();

        // A 25-minute work session + 10 minutes of passive app time, plus two
        // answers. Anchor the segments to local MIDDAY today so they never cross
        // a day boundary (a session that did would correctly bucket to its start
        // day) — keeps the per-day assertions deterministic at any run time.
        let midday = day_floor_ms(now_ms()) + 12 * 60 * 60_000;
        insert_pomodoro_session(&c, Some(&sid), "work", midday, midday + 25 * 60_000).unwrap();
        insert_pomodoro_session(&c, Some(&sid), "app", midday, midday + 10 * 60_000).unwrap();
        // A break segment must NOT count toward study minutes.
        insert_pomodoro_session(&c, Some(&sid), "break", midday, midday + 5 * 60_000).unwrap();
        record_attempt(&c, &sid, None, "quiz", 0, "Q1", true).unwrap();
        record_attempt(&c, &sid, None, "quiz", 1, "Q2", false).unwrap();

        let s = analytics_summary(&c, 30).unwrap();
        // 30-day window of contiguous days, today is the last bucket.
        assert_eq!(s.minutes_per_day.len(), 30);
        assert_eq!(s.minutes_per_day.last().unwrap().minutes, 35, "work + app, no break");
        // The heatmap series always spans a full year, today last, same minutes.
        assert_eq!(s.year_minutes.len(), 366);
        assert_eq!(s.year_minutes.last().unwrap().minutes, 35, "today's bucket");
        assert_eq!(
            s.year_minutes.last().unwrap().day,
            s.minutes_per_day.last().unwrap().day,
            "both series end on today"
        );
        assert_eq!(s.minutes_week, 35);
        assert_eq!(s.reviews_week, 2);
        assert!((s.accuracy_week - 0.5).abs() < 1e-9, "1 of 2 correct");
        assert_eq!(s.streak, 1, "today has activity");
        assert_eq!(s.due_forecast.len(), 7);

        // Per-subject roll-up carries both minutes and reviews for the subject.
        assert_eq!(s.per_subject.len(), 1);
        let ps = &s.per_subject[0];
        assert_eq!(ps.subject_id, sid);
        assert_eq!(ps.minutes, 35);
        assert_eq!(ps.reviews, 2);
        assert_eq!(ps.correct, 1);
    }

    #[test]
    fn weak_topics_attributes_via_material_and_skips_unattributable() {
        let st = AppState::in_memory().unwrap();
        let c = st.db.lock().unwrap();
        let sid = insert_subject(&c, "Bio", None, None, None).unwrap();
        let weak = insert_topic(&c, &sid, "Krebs cycle", None, &[]).unwrap();
        let payload = serde_json::json!({});
        let mat = save_material(&c, &sid, Some(&weak), "quiz", "Quiz", "", &payload).unwrap();

        // Topic "Krebs cycle": mostly wrong + a lapse → should surface as weak.
        // NB: srs_grade also records an attempt (q>=3 == correct), so the two
        // grades below add two more attributable attempts on top of these three.
        record_attempt(&c, &sid, Some(&mat), "quiz", 0, "Q1", false).unwrap();
        record_attempt(&c, &sid, Some(&mat), "quiz", 1, "Q2", false).unwrap();
        record_attempt(&c, &sid, Some(&mat), "quiz", 2, "Q3", true).unwrap();
        srs_grade(&c, &sid, Some(&mat), "quiz", 0, "Q1", 4).unwrap(); // schedule (+1 correct attempt)
        srs_grade(&c, &sid, Some(&mat), "quiz", 0, "Q1", 1).unwrap(); // lapse   (+1 wrong attempt)

        // An attempt with NO material (unattributable) must be ignored, not crash.
        record_attempt(&c, &sid, None, "quiz", 9, "Loose", false).unwrap();

        let s = analytics_summary(&c, 30).unwrap();
        assert_eq!(s.weak_topics.len(), 1, "only the attributable topic ranks");
        let w = &s.weak_topics[0];
        assert_eq!(w.topic_id, weak);
        assert_eq!(w.topic_name, "Krebs cycle");
        // 3 explicit + 2 from grading; 2 correct (Q3 + the grade-4); the loose
        // material-less attempt is excluded.
        assert_eq!(w.reviews, 5);
        assert_eq!(w.correct, 2);
        assert!(w.lapses >= 1, "the lapse is attributed to the topic");
        assert!(!w.reason.is_empty());
    }

    #[test]
    fn cheatsheet_version_restore_roundtrips() {
        let st = AppState::in_memory().unwrap();
        let c = st.db.lock().unwrap();
        let sid = insert_subject(&c, "Algorithms", None, None, None).unwrap();

        let sec = |title: &str, term: &str, def: &str| CsSection {
            id: title.to_lowercase().replace(' ', "-"),
            title: title.into(),
            state: "approved".into(),
            items: vec![CsItem { t: term.into(), d: def.into() }],
            image: None,
            image_query: None,
        };

        // v1: save + snapshot the original sheet.
        let v1 = vec![sec("Key Concepts", "Big-O", "growth rate")];
        save_cheatsheet(&c, &sid, None, &v1).unwrap();
        snapshot_cheatsheet_version(&c, &sid, None, &v1, "generated").unwrap();

        // v2: edit to a different sheet (now the live one).
        let v2 = vec![sec("Key Concepts", "Big-O", "EDITED growth rate")];
        save_cheatsheet(&c, &sid, None, &v2).unwrap();
        snapshot_cheatsheet_version(&c, &sid, None, &v2, "edited").unwrap();

        // The oldest stored version is the original (newest-first ordering).
        let versions = list_cheatsheet_versions(&c, &sid, None).unwrap();
        assert_eq!(versions.len(), 2);
        let original_id = versions.last().unwrap().id.clone();

        // get_cheatsheet_version_full returns the right scope + payload.
        let (got_sub, got_topic, got_secs) =
            get_cheatsheet_version_full(&c, &original_id).unwrap();
        assert_eq!(got_sub, sid);
        assert_eq!(got_topic, None);
        assert_eq!(got_secs[0].items[0].d, "growth rate");

        // Restore the original: snapshot current, overwrite live sheet, re-snapshot.
        let current = get_cheatsheet_sections(&c, &sid, None).unwrap();
        snapshot_cheatsheet_version(&c, &sid, None, &current, "before restore").unwrap();
        save_cheatsheet(&c, &sid, None, &got_secs).unwrap();
        snapshot_cheatsheet_version(&c, &sid, None, &got_secs, "restored").unwrap();

        // Live sheet now matches the original; the edit is preserved in history.
        let live = get_cheatsheet_sections(&c, &sid, None).unwrap();
        assert_eq!(live[0].items[0].d, "growth rate");
        let after = list_cheatsheet_versions(&c, &sid, None).unwrap();
        assert_eq!(after.len(), 4, "before-restore + restored snapshots added");
        assert_eq!(after[0].note, "restored");
        assert_eq!(after[1].note, "before restore");
    }
}
