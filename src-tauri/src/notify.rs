//! Background notification check for new Moodle grades / announcements.
//!
//! Mirrors the JS foreground diff (`src/lib/notifications.ts` → `checkNewMoodle`) but runs from the
//! native iOS Background-App-Refresh task (`BGAppRefreshTask`) so new grades/announcements can fire
//! a notification while the app is fully CLOSED. Both paths share the `notif_moodle_seen` setting and
//! the SAME id formats, so they never double-notify. iOS-only in effect (the C entry point is called
//! by the Swift plugin's BGTask handler); harmless elsewhere.

use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;
use tauri::{AppHandle, Manager};
use tauri_plugin_notification::NotificationExt;

use crate::db::AppState;
use crate::repo;

static APP: OnceLock<AppHandle> = OnceLock::new();

/// Stash the app handle so the C background entry point can reach the DB + notifier.
pub fn set_app(app: AppHandle) {
    let _ = APP.set(app);
}

/// Called from the iOS `BGAppRefreshTask` handler (Swift `@_silgen_name` bridge). Syncs Moodle and
/// fires notifications for anything new. Runs synchronously within the task's time budget (~30s).
#[no_mangle]
pub extern "C" fn cortex_ios_background_refresh() {
    let Some(app) = APP.get().cloned() else { return };
    tauri::async_runtime::block_on(async move {
        if let Err(e) = run(&app).await {
            eprintln!("[cortex] background refresh failed: {e}");
        }
    });
}

fn setting_on(conn: &rusqlite::Connection, key: &str) -> bool {
    repo::get_setting(conn, key)
        .ok()
        .flatten()
        .map(|v| v != "false")
        .unwrap_or(true) // default ON
}

async fn run(app: &AppHandle) -> crate::error::Result<()> {
    // 1) Pull fresh Moodle data (best-effort — ignore network/offline errors).
    let _ = crate::moodle::moodle_sync(app.clone()).await;

    // 2) Diff against the already-notified set under the DB lock; collect what to fire.
    let mut to_fire: Vec<(String, String)> = Vec::new();
    {
        let state = app.state::<AppState>();
        let c = state.db.lock().unwrap();

        let mut seen: HashSet<String> = repo::get_setting(&c, "notif_moodle_seen")
            .ok()
            .flatten()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        let first_run = seen.is_empty(); // seed silently on first run (don't spam history)
        let grades_on = setting_on(&c, "notif_grade");
        let ann_on = setting_on(&c, "notif_announcement");

        // course id → display name
        let mut names: HashMap<String, String> = HashMap::new();
        {
            let mut st = c.prepare("SELECT id, fullname, shortname FROM moodle_courses")?;
            let rows = st.query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, Option<String>>(1)?.unwrap_or_default(),
                    r.get::<_, Option<String>>(2)?.unwrap_or_default(),
                ))
            })?;
            for (id, full, short) in rows.flatten() {
                names.insert(id, if full.is_empty() { short } else { full });
            }
        }

        if grades_on {
            let mut st = c.prepare("SELECT course_id, item_name, grade, percentage FROM moodle_grades")?;
            let rows = st.query_map([], |r| {
                Ok((
                    r.get::<_, Option<String>>(0)?.unwrap_or_default(),
                    r.get::<_, Option<String>>(1)?.unwrap_or_default(),
                    r.get::<_, Option<String>>(2)?.unwrap_or_default(),
                    r.get::<_, Option<String>>(3)?.unwrap_or_default(),
                ))
            })?;
            for (cid, item, grade, pct) in rows.flatten() {
                let val = if pct.is_empty() { grade } else { pct };
                let id = format!("grade:{cid}:{item}:{val}");
                if seen.insert(id) && !first_run {
                    let course = names.get(&cid).cloned().unwrap_or_default();
                    to_fire.push((format!("✅ Grade released — {item}"), format!("{course} · {val}")));
                }
            }
        }

        if ann_on {
            let mut st = c.prepare("SELECT id, course_id, subject FROM moodle_announcements")?;
            let rows = st.query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, Option<String>>(1)?.unwrap_or_default(),
                    r.get::<_, Option<String>>(2)?.unwrap_or_default(),
                ))
            })?;
            for (aid, cid, subj) in rows.flatten() {
                let id = format!("ann:{aid}");
                if seen.insert(id) && !first_run {
                    let course = names.get(&cid).cloned().unwrap_or_default();
                    to_fire.push((format!("📣 {subj}"), course));
                }
            }
        }

        // Persist the (bounded) seen set.
        let mut arr: Vec<String> = seen.into_iter().collect();
        if arr.len() > 500 {
            let start = arr.len() - 500;
            arr = arr.split_off(start);
        }
        let _ = repo::set_setting(&c, "notif_moodle_seen", &serde_json::to_string(&arr)?);
    }

    // 3) Fire notifications (outside the DB lock).
    for (title, body) in to_fire {
        let _ = app.notification().builder().title(title).body(body).show();
    }
    Ok(())
}
