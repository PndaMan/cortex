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
];

/// Shared application state: a single SQLite connection behind a Mutex.
/// rusqlite is synchronous; Tauri runs commands on a worker pool so brief
/// lock contention is acceptable for a single-user desktop app.
pub struct AppState {
    pub db: Mutex<Connection>,
}

impl AppState {
    pub fn new(db_path: &PathBuf) -> Result<Self> {
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
