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
