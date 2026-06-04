use crate::error::Result;
use rusqlite::Connection;
use std::path::PathBuf;
use std::sync::Mutex;

/// Embedded migrations. Index in the array == target `user_version`.
const MIGRATIONS: &[&str] = &[
    include_str!("../migrations/0001_init.sql"),
    include_str!("../migrations/0002_preview_and_memory.sql"),
    include_str!("../migrations/0003_subject_color.sql"),
    include_str!("../migrations/0004_topic_glyph.sql"),
    include_str!("../migrations/0005_notes.sql"),
    include_str!("../migrations/0006_events.sql"),
    include_str!("../migrations/0007_review.sql"),
    include_str!("../migrations/0008_srs.sql"),
    include_str!("../migrations/0009_citations.sql"),
];

/// Shared application state: a single SQLite connection behind a Mutex.
/// rusqlite is synchronous; Tauri runs commands on a worker pool so brief
/// lock contention is acceptable for a single-user desktop app.
pub struct AppState {
    pub db: Mutex<Connection>,
}

/// Register the statically-linked `sqlite-vec` extension as an auto-extension so
/// every connection opened afterward gets the `vec_distance_cosine` SQL function
/// (used by `repo::search_chunks`). No runtime `.so` is loaded — the extension is
/// compiled in and registered via SQLite's auto-extension hook. Idempotent.
fn register_sqlite_vec() {
    use std::sync::Once;
    static VEC_INIT: Once = Once::new();
    VEC_INIT.call_once(|| unsafe {
        rusqlite::ffi::sqlite3_auto_extension(Some(std::mem::transmute(
            sqlite_vec::sqlite3_vec_init as *const (),
        )));
    });
}

impl AppState {
    pub fn new(db_path: &PathBuf) -> Result<Self> {
        register_sqlite_vec();
        let conn = Connection::open(db_path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        run_migrations(&conn)?;
        Ok(Self {
            db: Mutex::new(conn),
        })
    }

    /// In-memory database, for tests.
    #[cfg(test)]
    pub fn in_memory() -> Result<Self> {
        register_sqlite_vec();
        let conn = Connection::open_in_memory()?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        run_migrations(&conn)?;
        Ok(Self {
            db: Mutex::new(conn),
        })
    }
}

/// Apply any migrations whose index is beyond the current `user_version`.
fn run_migrations(conn: &Connection) -> Result<()> {
    let mut version: i64 =
        conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
    while (version as usize) < MIGRATIONS.len() {
        let sql = MIGRATIONS[version as usize];
        conn.execute_batch(sql)?;
        version += 1;
        conn.pragma_update(None, "user_version", version)?;
    }
    Ok(())
}

/// Milliseconds since the Unix epoch.
pub fn now_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

pub fn new_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sqlite_vec_extension_is_registered() {
        // Proves the statically-linked sqlite-vec auto-extension is actually live
        // (so search_chunks uses it, not the Rust fallback). Identical vectors have
        // cosine distance 0; orthogonal vectors distance 1.
        let st = AppState::in_memory().unwrap();
        let conn = st.db.lock().unwrap();
        let a = crate::vector::f32s_to_blob(&[1.0, 0.0, 0.0]);
        let b = crate::vector::f32s_to_blob(&[0.0, 1.0, 0.0]);
        let same: f64 = conn
            .query_row("SELECT vec_distance_cosine(?1, ?1)", [&a], |r| r.get(0))
            .expect("vec_distance_cosine must be registered");
        let orth: f64 = conn
            .query_row("SELECT vec_distance_cosine(?1, ?2)", [&a, &b], |r| r.get(0))
            .unwrap();
        assert!(same.abs() < 1e-5, "identical vectors distance ~0, got {same}");
        assert!((orth - 1.0).abs() < 1e-5, "orthogonal vectors distance ~1, got {orth}");
    }

    #[test]
    fn migrations_apply_and_are_idempotent() {
        let st = AppState::in_memory().unwrap();
        let conn = st.db.lock().unwrap();
        let v: i64 = conn
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .unwrap();
        assert_eq!(v as usize, MIGRATIONS.len());
        // tables exist
        let n: i64 = conn
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='subjects'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, 1);
    }
}
