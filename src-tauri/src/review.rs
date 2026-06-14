//! Review (spaced repetition over wrong answers) command surface. Mirrored
//! 1:1 in `src/lib/api.ts`. Synchronous, inline DB lock like `commands.rs`.

use crate::db::AppState;
use crate::error::Result;
use crate::models::*;
use crate::repo;
use tauri::State;

#[tauri::command]
pub fn record_attempt(
    state: State<AppState>,
    subject_id: String,
    material_id: Option<String>,
    kind: String,
    item_index: i64,
    item_key: String,
    correct: bool,
) -> Result<()> {
    let c = state.db.lock().unwrap();
    repo::record_attempt(
        &c,
        &subject_id,
        material_id.as_deref(),
        &kind,
        item_index,
        &item_key,
        correct,
    )?;
    Ok(())
}

/// The items to re-study: those whose most recent attempt was incorrect.
#[tauri::command]
pub fn review_set(
    state: State<AppState>,
    subject_id: String,
    kind: String,
) -> Result<Vec<ReviewItem>> {
    let c = state.db.lock().unwrap();
    repo::wrong_items(&c, &subject_id, &kind)
}

/// Grade a card with SM-2 and persist its next-due schedule. `quality` 0-5
/// (Again≈1, Hard≈3, Good≈4, Easy≈5). Returns the updated schedule.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub fn srs_grade(
    state: State<AppState>,
    subject_id: String,
    material_id: Option<String>,
    kind: String,
    item_index: i64,
    item_key: String,
    quality: i64,
) -> Result<SrsResult> {
    let c = state.db.lock().unwrap();
    repo::srs_grade(
        &c,
        &subject_id,
        material_id.as_deref(),
        &kind,
        item_index,
        &item_key,
        quality,
    )
}

/// Cards due for review now (due_at <= now), oldest first.
#[tauri::command]
pub fn srs_due(state: State<AppState>, subject_id: String, kind: String) -> Result<Vec<DueCard>> {
    let c = state.db.lock().unwrap();
    repo::srs_due(&c, &subject_id, &kind)
}

/// Preview next interval (days) per grade [again, hard, good, easy] for a card,
/// so the study UI can show what each button will schedule.
#[tauri::command]
pub fn srs_preview(
    state: State<AppState>,
    subject_id: String,
    kind: String,
    item_key: String,
) -> Result<Vec<i64>> {
    let c = state.db.lock().unwrap();
    Ok(repo::srs_preview(&c, &subject_id, &kind, &item_key)?.to_vec())
}

/// Due-now and total scheduled-card counts for a subject+kind.
#[tauri::command]
pub fn srs_stats(state: State<AppState>, subject_id: String, kind: String) -> Result<SrsStats> {
    let c = state.db.lock().unwrap();
    repo::srs_stats(&c, &subject_id, &kind)
}
