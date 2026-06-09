//! Calendar (events + tasks) command surface. Mirrored 1:1 in `src/lib/api.ts`.
//! All commands are synchronous — they lock the DB inline like the sync
//! commands in `commands.rs`. Reminders are polled by the frontend via
//! `check_reminders`, which returns due reminders and marks them notified.

use crate::db::{now_ms, AppState};
use crate::error::Result;
use crate::models::*;
use crate::repo;
use tauri::State;

#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub fn create_event(
    state: State<AppState>,
    subject_id: Option<String>,
    title: String,
    description: Option<String>,
    location: Option<String>,
    color: Option<String>,
    start_ms: i64,
    end_ms: Option<i64>,
    all_day: Option<bool>,
    kind: Option<String>,
    reminder_ms: Option<i64>,
    tags: Option<Vec<String>>,
) -> Result<CalEvent> {
    let c = state.db.lock().unwrap();
    let id = repo::insert_event(
        &c,
        subject_id.as_deref(),
        &title,
        description.as_deref(),
        location.as_deref(),
        color.as_deref(),
        start_ms,
        end_ms,
        all_day.unwrap_or(false),
        kind.as_deref().unwrap_or("event"),
        reminder_ms,
        &tags.unwrap_or_default(),
    )?;
    repo::get_event(&c, &id)
}

#[tauri::command]
pub fn list_events(
    state: State<AppState>,
    subject_id: Option<String>,
    from_ms: Option<i64>,
    to_ms: Option<i64>,
) -> Result<Vec<CalEvent>> {
    let c = state.db.lock().unwrap();
    repo::list_events(&c, subject_id.as_deref(), from_ms, to_ms)
}

#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub fn update_event(
    state: State<AppState>,
    id: String,
    title: String,
    description: Option<String>,
    location: Option<String>,
    color: Option<String>,
    start_ms: i64,
    end_ms: Option<i64>,
    all_day: Option<bool>,
    kind: Option<String>,
    reminder_ms: Option<i64>,
    tags: Option<Vec<String>>,
) -> Result<CalEvent> {
    let c = state.db.lock().unwrap();
    repo::update_event(
        &c,
        &id,
        &title,
        description.as_deref(),
        location.as_deref(),
        color.as_deref(),
        start_ms,
        end_ms,
        all_day.unwrap_or(false),
        kind.as_deref().unwrap_or("event"),
        reminder_ms,
        &tags.unwrap_or_default(),
    )?;
    repo::get_event(&c, &id)
}

/// Set the per-deadline study checklist (ticked topic ids).
#[tauri::command]
pub fn set_event_checklist(
    state: State<AppState>,
    id: String,
    topic_ids: Vec<String>,
) -> Result<CalEvent> {
    let c = state.db.lock().unwrap();
    repo::set_event_checklist(&c, &id, &topic_ids)?;
    repo::get_event(&c, &id)
}

#[tauri::command]
pub fn delete_event(state: State<AppState>, id: String) -> Result<()> {
    let c = state.db.lock().unwrap();
    repo::delete_event(&c, &id)
}

#[tauri::command]
pub fn set_event_done(state: State<AppState>, id: String, done: bool) -> Result<CalEvent> {
    let c = state.db.lock().unwrap();
    repo::set_event_done(&c, &id, done)?;
    repo::get_event(&c, &id)
}

/// Return reminders that are due now and mark them notified so the frontend can
/// raise a notification exactly once. The frontend polls this periodically.
/// With `system_notify` (window hidden in the tray) they are ALSO raised as OS
/// notifications, since in-app toasts would go unseen.
#[tauri::command]
pub fn check_reminders(
    app: tauri::AppHandle,
    state: State<AppState>,
    system_notify: Option<bool>,
) -> Result<Vec<CalEvent>> {
    let due = {
        let c = state.db.lock().unwrap();
        let due = repo::due_reminders(&c, now_ms())?;
        for e in &due {
            repo::mark_notified(&c, &e.id)?;
        }
        due
    };
    if system_notify.unwrap_or(false) {
        use tauri_plugin_notification::NotificationExt;
        for e in &due {
            let body = e
                .location
                .as_deref()
                .map(|l| format!("at {l}"))
                .unwrap_or_else(|| "Reminder".to_string());
            let _ = app
                .notification()
                .builder()
                .title(format!("⏰ {}", e.title))
                .body(body)
                .show();
        }
    }
    Ok(due)
}
