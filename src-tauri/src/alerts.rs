//! Smarter notifications: stable ids, tap→route bookkeeping, and (on phones)
//! locally-scheduled deadline alerts.
//!
//! Every OS notification Cortex shows gets a deterministic numeric id plus a
//! stored "route" describing where a tap should land inside the app (a subject,
//! a calendar day). The frontend listens for the notification plugin's
//! `actionPerformed` event, resolves the id via the `notification_route`
//! command, and navigates — deep links that work without APNs or any server.
//!
//! On mobile we additionally pre-schedule local alerts for upcoming Moodle
//! deadlines/exams after every sync (24 h and 1 h before each due date). iOS
//! delivers those on time even when the app is fully closed — scheduled local
//! notifications need no push infrastructure.

use crate::db::AppState;
use crate::error::Result;
use crate::repo;
use serde_json::{json, Value};
use tauri::{AppHandle, Manager};

/// Settings key holding the id→route list. Deliberately NOT in the sync
/// allowlist: notification ids are per-device state.
const K_ROUTES: &str = "notif_routes";
/// Keep the route list bounded; oldest entries fall off first.
const ROUTES_CAP: usize = 300;

/// Deterministic positive i32 for a notification key (FNV-1a folded to 31 bits).
/// Stable ids let a re-sync REPLACE a pending scheduled alert instead of
/// stacking duplicates, and let a tap after an app restart still resolve.
pub fn notif_id(key: &str) -> i32 {
    let mut h: u32 = 0x811c_9dc5;
    for b in key.as_bytes() {
        h ^= u32::from(*b);
        h = h.wrapping_mul(0x0100_0193);
    }
    (h & 0x7fff_ffff) as i32
}

/// Routes are stored as a JSON array (order = insertion age) of objects that
/// carry an `id` field plus the route payload itself.
fn load_routes(c: &rusqlite::Connection) -> Vec<Value> {
    repo::get_setting(c, K_ROUTES)
        .ok()
        .flatten()
        .and_then(|s| serde_json::from_str::<Vec<Value>>(&s).ok())
        .unwrap_or_default()
}

/// Remember where tapping notification `id` should take the user.
pub fn set_route(app: &AppHandle, id: i32, mut route: Value) {
    let state = app.state::<AppState>();
    let Ok(c) = state.db.lock() else { return };
    if let Some(obj) = route.as_object_mut() {
        obj.insert("id".into(), json!(id));
    }
    let mut routes = load_routes(&c);
    routes.retain(|r| r.get("id").and_then(Value::as_i64) != Some(i64::from(id)));
    routes.push(route);
    if routes.len() > ROUTES_CAP {
        let drop = routes.len() - ROUTES_CAP;
        routes.drain(..drop);
    }
    let _ = repo::set_setting(&c, K_ROUTES, &Value::Array(routes).to_string());
}

/// Resolve a tapped notification id back to its route (frontend navigation).
#[tauri::command]
pub fn notification_route(state: tauri::State<AppState>, id: i32) -> Result<Option<Value>> {
    let c = state.db.lock().unwrap();
    Ok(load_routes(&c)
        .into_iter()
        .find(|r| r.get("id").and_then(Value::as_i64) == Some(i64::from(id))))
}

/// Show a notification (best-effort) with a stable id and a tap route.
pub fn notify_routed(app: &AppHandle, key: &str, title: &str, body: &str, route: Value) {
    use tauri_plugin_notification::NotificationExt;
    let id = notif_id(key);
    set_route(app, id, route);
    let _ = app
        .notification()
        .builder()
        .id(id)
        .title(title)
        .body(body)
        .show();
}

#[cfg(test)]
mod tests {
    use super::notif_id;

    #[test]
    fn notif_ids_are_stable_positive_and_distinct() {
        assert_eq!(notif_id("moodle:assign:1:d"), notif_id("moodle:assign:1:d"));
        assert!(notif_id("moodle:assign:1:d") > 0);
        assert!(notif_id("src:abc") > 0);
        assert_ne!(notif_id("moodle:assign:1:d"), notif_id("moodle:assign:1:h"));
        assert_ne!(notif_id("src:abc"), notif_id("ev:abc"));
    }
}

/// (Re)schedule local alerts for upcoming Moodle deadlines/exams: one the day
/// before and one an hour before each due date, capped to the nearest 30
/// deadlines in the next 14 days (iOS allows 64 pending). Called after every
/// Moodle sync so moved/removed deadlines can't fire stale alerts.
///
/// Mobile-only: desktop reminders already flow through `check_reminders` (the
/// mirrored calendar events now carry a default reminder), and the desktop
/// notification backend can't schedule future notifications anyway.
pub fn schedule_deadline_alerts(app: &AppHandle) {
    #[cfg(not(mobile))]
    {
        let _ = app;
    }
    #[cfg(mobile)]
    {
        use tauri_plugin_notification::{NotificationExt, PermissionState, Schedule};
        let n = app.notification();
        // Startup asked once; a background sync must not trigger a permission prompt.
        if !matches!(n.permission_state(), Ok(PermissionState::Granted)) {
            return;
        }
        // Every pending scheduled notification today IS a deadline alert, so a
        // full wipe + reschedule keeps them exactly in step with Moodle.
        let _ = n.cancel_all();

        let now = crate::db::now_ms();
        let horizon = now + 14 * 86_400_000;
        let rows: Vec<(String, String, i64, String, String, Option<String>)> = {
            let state = app.state::<AppState>();
            let Ok(c) = state.db.lock() else { return };
            let Ok(mut st) = c.prepare(
                "SELECT d.id, COALESCE(NULLIF(d.name,''),'Deadline'), d.due_at * 1000, \
                        COALESCE(d.kind,''), \
                        COALESCE(NULLIF(mc.fullname,''), NULLIF(mc.shortname,''), ''), s.id \
                 FROM moodle_deadlines d \
                 LEFT JOIN moodle_courses mc ON mc.id = d.course_id \
                 LEFT JOIN subjects s ON s.moodle_course_id = d.course_id \
                 WHERE d.due_at * 1000 > ?1 AND d.due_at * 1000 <= ?2 \
                 ORDER BY d.due_at LIMIT 30",
            ) else {
                return;
            };
            let Ok(mapped) = st.query_map(rusqlite::params![now, horizon], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, i64>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, String>(4)?,
                    r.get::<_, Option<String>>(5)?,
                ))
            }) else {
                return;
            };
            mapped.filter_map(|x| x.ok()).collect()
        };

        for (did, name, due_ms, kind, course, subject_id) in rows {
            let exam = kind == "exam";
            let course_bit = if course.is_empty() { String::new() } else { format!(" · {course}") };
            for (slot, lead_ms, when) in
                [("d", 86_400_000_i64, "tomorrow"), ("h", 3_600_000_i64, "in 1 hour")]
            {
                let at_ms = due_ms - lead_ms;
                if at_ms <= now {
                    continue;
                }
                let Ok(date) = time::OffsetDateTime::from_unix_timestamp(at_ms / 1000) else {
                    continue;
                };
                let id = notif_id(&format!("moodle:{did}:{slot}"));
                set_route(
                    app,
                    id,
                    json!({
                        "kind": if exam { "exam" } else { "deadline" },
                        "ts": due_ms,
                        "subjectId": subject_id,
                    }),
                );
                let title = if exam {
                    format!("Exam {when} — {name}")
                } else {
                    format!("Due {when} — {name}")
                };
                let _ = n
                    .builder()
                    .id(id)
                    .title(title)
                    .body(format!("{}{course_bit}", if exam { "Exam" } else { "Assignment" }))
                    .schedule(Schedule::At {
                        date,
                        repeating: false,
                        allow_while_idle: true,
                    })
                    .show();
            }
        }
    }
}
