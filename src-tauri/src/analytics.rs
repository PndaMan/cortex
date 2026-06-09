//! Study Analytics command surface — pomodoro session logging + the one-shot
//! dashboard summary. Mirrored 1:1 in `src/lib/api.ts`. Synchronous, inline DB
//! lock like `review.rs` / `commands.rs`.

use crate::db::AppState;
use crate::error::Result;
use crate::models::AnalyticsSummary;
use crate::repo;
use tauri::State;

/// Persist one finished pomodoro segment (called when a work/break phase
/// completes). `subject_id` is whichever subject was active at that moment.
#[tauri::command]
pub fn log_pomodoro_session(
    state: State<AppState>,
    subject_id: Option<String>,
    kind: String,
    started_ms: i64,
    ended_ms: i64,
) -> Result<()> {
    let c = state.db.lock().unwrap();
    repo::insert_pomodoro_session(&c, subject_id.as_deref(), &kind, started_ms, ended_ms)?;
    Ok(())
}

/// The whole Study Analytics dashboard in one struct, computed under a single
/// lock. `days` (default 30) bounds the per-day charts and per-subject roll-up.
#[tauri::command]
pub fn analytics_summary(state: State<AppState>, days: Option<i64>) -> Result<AnalyticsSummary> {
    let c = state.db.lock().unwrap();
    repo::analytics_summary(&c, days.unwrap_or(30))
}
