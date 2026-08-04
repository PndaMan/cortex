//! Moodle integration (experimental).
//!
//! Pulls a student's data from a Moodle uni portal via the Web Services REST
//! API and caches it locally. Auth is a `moodle_mobile_app` token obtained from
//! `/login/token.php` (username+password) or pasted by the user (for SSO sites
//! where password→token is blocked — they extract a token via the browser).
//!
//! We only ever store the TOKEN, never the password. All requests are read-only.
//! Note: exam dates/venues are typically NOT in Moodle — they live in a separate
//! timetabling system — so this covers grades, assignments, calendar deadlines
//! and announcements, not seat/room allocation.

use crate::db::{now_ms, AppState};
use crate::error::{Error, Result};
use crate::repo;
use rusqlite::{params, Connection};
use serde_json::Value;
use tauri::{AppHandle, Manager};

const K_URL: &str = "moodle_url";
const K_TOKEN: &str = "moodle_token";
const K_USERID: &str = "moodle_userid";
const K_LAST_SYNC: &str = "moodle_last_sync";
const SERVICE: &str = "moodle_mobile_app";

fn client() -> reqwest::blocking::Client {
    reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(45))
        // Present as the official Moodle Mobile app: some institutions' WAFs/Moodle
        // configs treat web-service requests differently by User-Agent, and the
        // app demonstrably works against this site.
        .user_agent("MoodleMobile 4.4.0 (44000)")
        .build()
        .unwrap_or_default()
}

/// Normalize a site URL: ensure a scheme, drop any trailing slash.
fn norm_url(raw: &str) -> String {
    let t = raw.trim().trim_end_matches('/');
    if t.starts_with("http://") || t.starts_with("https://") {
        t.to_string()
    } else {
        format!("https://{t}")
    }
}

/// Call a Web Services function. Moodle returns HTTP 200 even for errors, with an
/// `exception`/`message` body — so we inspect the body, not the status.
///
/// Params are sent as a POST form body (this is what the official Moodle mobile
/// app does). A GET query string can be truncated/mangled by an institutional
/// proxy/WAF — which makes Moodle see a malformed `wstoken` and reject it with
/// "Invalid parameter value detected" — so POST is both safer and more standard.
fn ws(url: &str, token: &str, func: &str, params: &[(String, String)]) -> Result<Value> {
    let endpoint = format!("{url}/webservice/rest/server.php");
    let mut q: Vec<(String, String)> = vec![
        ("wstoken".into(), token.to_string()),
        // Moodle's REST endpoint reads the function name from `wsfunction`. Sending
        // it as `moodlewsfunction` left the function name empty, so Moodle threw
        // invalid_parameter_exception ("Missing function name") *after* the token
        // authenticated — the long-standing "Invalid parameter value detected".
        ("wsfunction".into(), func.to_string()),
        ("moodlewsrestformat".into(), "json".into()),
    ];
    q.extend_from_slice(params);
    // Read the raw body first so we can surface it verbatim on error — Moodle hides
    // the offending parameter in `message`/`debuginfo` when server debugging is off,
    // so the raw response is the only remaining signal for diagnosis.
    let body = client().post(&endpoint).form(&q).send()?.text()?;
    let val: Value = serde_json::from_str(&body).map_err(|e| {
        Error::Other(format!(
            "Moodle: response was not JSON ({e}): {}",
            body.chars().take(200).collect::<String>()
        ))
    })?;
    if let Some(obj) = val.as_object() {
        if obj.contains_key("exception") {
            let msg = obj.get("message").and_then(|m| m.as_str()).unwrap_or("request failed");
            // Surface errorcode + debuginfo (when present) + the raw response — they
            // pinpoint *which* parameter/why (e.g. "invalidtoken" vs "invalidparameter").
            let code = obj.get("errorcode").and_then(|m| m.as_str()).unwrap_or("");
            let debug = obj.get("debuginfo").and_then(|m| m.as_str()).unwrap_or("");
            let code_part = if code.is_empty() { String::new() } else { format!(" [{code}]") };
            let debug_part = if debug.is_empty() { String::new() } else { format!(" — {debug}") };
            let raw = body.chars().take(400).collect::<String>();
            return Err(Error::Other(format!("Moodle{code_part}: {msg}{debug_part} :: raw={raw}")));
        }
    }
    Ok(val)
}

/// Exchange username+password for a mobile-service token.
fn fetch_token(url: &str, user: &str, pass: &str) -> Result<String> {
    let endpoint = format!("{url}/login/token.php");
    let resp = client()
        .post(&endpoint)
        .form(&[
            ("username", user),
            ("password", pass),
            ("service", SERVICE),
        ])
        .send()?;
    let val: Value = resp.json()?;
    if let Some(t) = val.get("token").and_then(|t| t.as_str()) {
        return Ok(t.to_string());
    }
    let err = val
        .get("error")
        .and_then(|e| e.as_str())
        .unwrap_or("login failed (web services may be disabled, or this site uses SSO — paste a token instead)");
    Err(Error::Other(format!("Moodle login: {err}")))
}

/// `core_webservice_get_site_info` → (userid, fullname). Doubles as a token check.
fn site_info(url: &str, token: &str) -> Result<(i64, String)> {
    let v = ws(url, token, "core_webservice_get_site_info", &[])?;
    let uid = v
        .get("userid")
        .and_then(|x| x.as_i64())
        .ok_or_else(|| Error::Other("Moodle: token rejected (no userid returned)".into()))?;
    let name = v
        .get("fullname")
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_string();
    Ok((uid, name))
}

fn read_cfg(c: &Connection) -> Option<(String, String)> {
    let url = repo::get_setting(c, K_URL).ok().flatten()?;
    let token = repo::get_setting(c, K_TOKEN).ok().flatten()?;
    if url.trim().is_empty() || token.trim().is_empty() {
        return None;
    }
    Some((url, token))
}

// ---- output shapes ----------------------------------------------------------

#[derive(serde::Serialize)]
pub struct MoodleStatus {
    pub configured: bool,
    pub user_id: i64,
    pub last_sync: i64,
}

#[derive(serde::Serialize)]
pub struct MoodleSummary {
    pub courses: usize,
    pub grades: usize,
    pub deadlines: usize,
    pub announcements: usize,
}

#[derive(serde::Serialize)]
pub struct MoodleCourse {
    pub id: String,
    pub shortname: String,
    pub fullname: String,
}
#[derive(serde::Serialize)]
pub struct MoodleGrade {
    pub course_id: String,
    pub item_name: String,
    pub grade: String,
    pub percentage: String,
    pub feedback: String,
}
#[derive(serde::Serialize)]
pub struct MoodleDeadline {
    pub id: String,
    pub course_id: String,
    pub name: String,
    pub due_at: i64,
    pub kind: String,
    pub status: String,
    pub url: String,
}
#[derive(serde::Serialize)]
pub struct MoodleAnnouncement {
    pub id: String,
    pub course_id: String,
    pub subject: String,
    pub message: String,
    pub posted_at: i64,
    pub url: String,
}
#[derive(serde::Serialize)]
pub struct MoodleData {
    pub courses: Vec<MoodleCourse>,
    pub grades: Vec<MoodleGrade>,
    pub deadlines: Vec<MoodleDeadline>,
    pub announcements: Vec<MoodleAnnouncement>,
}

// ---- commands ---------------------------------------------------------------

/// Connect with username+password (non-SSO sites). Stores token+url+userid.
#[tauri::command]
pub async fn moodle_connect(
    app: AppHandle,
    url: String,
    username: String,
    password: String,
) -> Result<String> {
    tauri::async_runtime::spawn_blocking(move || -> Result<String> {
        let url = norm_url(&url);
        let token = fetch_token(&url, &username, &password)?; // password used here, then dropped
        let (uid, fullname) = site_info(&url, &token)?;
        let state = app.state::<AppState>();
        let c = state.db.lock().unwrap();
        repo::set_setting(&c, K_URL, &url)?;
        repo::set_setting(&c, K_TOKEN, &token)?;
        repo::set_setting(&c, K_USERID, &uid.to_string())?;
        Ok(fullname)
    })
    .await
    .map_err(|e| Error::Other(format!("moodle connect task failed: {e}")))?
}

/// Connect with a pasted token (SSO sites). Verifies it via site_info.
#[tauri::command]
pub async fn moodle_set_token(app: AppHandle, url: String, token: String) -> Result<String> {
    tauri::async_runtime::spawn_blocking(move || -> Result<String> {
        let url = norm_url(&url);
        let token = token.trim().to_string();
        let (uid, fullname) = site_info(&url, &token)?;
        let state = app.state::<AppState>();
        let c = state.db.lock().unwrap();
        repo::set_setting(&c, K_URL, &url)?;
        repo::set_setting(&c, K_TOKEN, &token)?;
        repo::set_setting(&c, K_USERID, &uid.to_string())?;
        Ok(fullname)
    })
    .await
    .map_err(|e| Error::Other(format!("moodle set-token task failed: {e}")))?
}

/// SSO login: open the Moodle mobile launch flow in a window. The user signs in
/// through their institution's SSO (e.g. Microsoft/SAML + MFA) and Moodle
/// redirects to `cortexmoodle://token=<base64>`; we intercept that, decode the
/// token, verify it and store it. Emits `moodle-sso-done` (full name) on success
/// or `moodle-sso-error` (message) on failure.
const SSO_SCHEME: &str = "cortexmoodle";

#[tauri::command]
pub fn moodle_login_sso(app: AppHandle, url: String) -> Result<()> {
    let url = norm_url(&url);
    // Persist the site now so the cortexmoodle:// protocol handler knows which
    // server to verify the token against when the callback fires.
    {
        let state = app.state::<AppState>();
        let c = state.db.lock().unwrap();
        repo::set_setting(&c, K_URL, &url)?;
    }
    let passport = now_ms(); // unique-enough nonce for the launch request
    let launch = format!(
        "{url}/admin/tool/mobile/launch.php?service={SERVICE}&passport={passport}&urlscheme={SSO_SCHEME}"
    );
    let launch_url: tauri::Url = launch
        .parse()
        .map_err(|e| Error::Other(format!("bad launch url: {e}")))?;

    // A previous attempt (especially a failed one) may have left the login window
    // open — re-creating a webview with the same label errors ("already exists"),
    // so close any stale one first.
    if let Some(existing) = app.get_webview_window("moodle-login") {
        let _ = existing.close();
    }

    // The cortexmoodle:// callback is caught by the URI-scheme protocol handler
    // registered in lib.rs (it receives the RAW callback URI, so the base64 token
    // isn't corrupted by URL normalization like on_navigation's parsed Url would be).
    tauri::WebviewWindowBuilder::new(
        &app,
        "moodle-login",
        tauri::WebviewUrl::External(launch_url),
    )
    .title("Sign in to Moodle")
    .inner_size(520.0, 760.0)
    .build()
    .map_err(|e| Error::Other(format!("could not open login window: {e}")))?;
    Ok(())
}

/// Handle the raw `cortexmoodle://token=<base64>` callback URI from the SSO launch
/// flow: extract the token verbatim, verify it, persist it, and notify the UI.
/// Called by the URI-scheme protocol handler (raw string → no token corruption).
pub fn handle_sso_uri(app: &AppHandle, raw_uri: &str) {
    use tauri::Emitter;
    // Everything after "token=", minus any trailing fragment/query/slash. The
    // value may be percent-encoded (base64's + / = encoded as %2B %2F %3D).
    let token_raw = raw_uri
        .split("token=")
        .nth(1)
        .unwrap_or("")
        .split(['#', '?'])
        .next()
        .unwrap_or("")
        .trim_end_matches('/')
        .to_string();
    let url = {
        let state = app.state::<AppState>();
        let c = state.db.lock().unwrap();
        repo::get_setting(&c, K_URL).ok().flatten().unwrap_or_default()
    };
    match handle_sso_token(app, &url, &token_raw) {
        Ok(name) => {
            let _ = app.emit("moodle-sso-done", name);
        }
        Err(e) => {
            // Attach non-secret diagnostics so a remaining failure is debuggable
            // without us being able to reach the institution's Moodle.
            let diag = sso_diag(raw_uri, &token_raw);
            let _ = app.emit("moodle-sso-error", format!("{e} {diag}"));
        }
    }
    if let Some(w) = app.get_webview_window("moodle-login") {
        let _ = w.close();
    }
}

/// Base64-decode a launch token, accepting either STANDARD or URL-SAFE alphabets
/// (some Moodle configs emit URL-safe base64 with `-`/`_`, which STANDARD rejects).
fn b64_decode_flexible(s: &str) -> std::result::Result<Vec<u8>, base64::DecodeError> {
    use base64::Engine;
    let s = s.trim();
    base64::engine::general_purpose::STANDARD
        .decode(s)
        .or_else(|_| base64::engine::general_purpose::URL_SAFE.decode(s))
        .or_else(|_| base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(s))
        .or_else(|_| base64::engine::general_purpose::STANDARD_NO_PAD.decode(s))
}

/// Non-secret diagnostics about the callback: lengths + structure only.
fn sso_diag(raw_uri: &str, token_raw: &str) -> String {
    let pd = percent_encoding::percent_decode_str(token_raw)
        .decode_utf8_lossy()
        .to_string();
    let (dec_len, part_lens) = match b64_decode_flexible(&pd) {
        Ok(b) => {
            let s = String::from_utf8_lossy(&b);
            // Per-segment lengths reveal the layout (e.g. [32,32,64] = md5:::token:::privatetoken),
            // so we can tell which segment is the wstoken without exposing the secret.
            let lens: Vec<usize> = s.split(":::").map(|p| p.trim().len()).collect();
            (b.len(), lens)
        }
        Err(_) => (0usize, Vec::new()),
    };
    format!(
        "[has_token={} raw_len={} b64_len={} decoded_len={} parts={} partlens={:?}]",
        raw_uri.contains("token="),
        raw_uri.len(),
        pd.len(),
        dec_len,
        part_lens.len(),
        part_lens
    )
}

/// Keep only ASCII alphanumerics. Moodle validates `wstoken` as PARAM_ALPHANUM, so
/// any stray byte (a separator fragment, whitespace, a control char from a lossy
/// UTF-8 decode) makes the server reject it with "Invalid parameter value detected"
/// — exactly the failure we saw. Stripping to alphanumerics fixes a contaminated
/// segment without changing a clean one.
fn sanitize_token(s: &str) -> String {
    s.chars().filter(|c| c.is_ascii_alphanumeric()).collect()
}

/// Decode the launch callback token, verify it, and persist it. The raw value may
/// be percent-encoded; after decoding it's base64 of `<md5-sig>:::<token>:::<privatetoken>`
/// (the canonical Moodle-mobile layout, so `parts[1]` is the token) — but sites
/// differ, so we try every segment and use whichever one `get_site_info` actually
/// accepts, rather than assuming the layout.
fn handle_sso_token(app: &AppHandle, url: &str, token_raw: &str) -> Result<String> {
    if token_raw.is_empty() {
        return Err(Error::Other("no token in SSO callback".into()));
    }
    // Percent-decode first (idempotent for un-encoded input), then base64-decode
    // (STANDARD or URL-safe alphabet, depending on the Moodle config).
    let token_b64 = percent_encoding::percent_decode_str(token_raw)
        .decode_utf8_lossy()
        .to_string();
    let decoded = b64_decode_flexible(&token_b64)
        .map_err(|e| Error::Other(format!("token decode failed: {e}")))?;
    let s = String::from_utf8_lossy(&decoded);
    let parts: Vec<&str> = s.split(":::").collect();

    // Candidate tokens, most-likely first: parts[1] (canonical), then the other
    // segments, then the whole decoded blob for single-token sites — each
    // sanitized to alphanumerics and de-duplicated. A wrong candidate just fails
    // get_site_info and we move on.
    let mut candidates: Vec<String> = Vec::new();
    let mut push = |t: String| {
        if t.len() >= 16 && !candidates.contains(&t) {
            candidates.push(t);
        }
    };
    for idx in [1usize, 0, 2] {
        if let Some(p) = parts.get(idx) {
            push(sanitize_token(p));
        }
    }
    if parts.len() < 2 {
        push(sanitize_token(&s));
    }
    if candidates.is_empty() {
        return Err(Error::Other(
            "no usable token in SSO callback (decoded payload had no token-shaped segment)".into(),
        ));
    }

    let mut first_err: Option<Error> = None;
    for token in &candidates {
        match site_info(url, token) {
            Ok((uid, fullname)) => {
                let state = app.state::<AppState>();
                let c = state.db.lock().unwrap();
                repo::set_setting(&c, K_URL, url)?;
                repo::set_setting(&c, K_TOKEN, token)?;
                repo::set_setting(&c, K_USERID, &uid.to_string())?;
                return Ok(fullname);
            }
            Err(e) => {
                if first_err.is_none() {
                    first_err = Some(e);
                }
            }
        }
    }
    Err(first_err.unwrap_or_else(|| Error::Other("SSO token rejected by Moodle".into())))
}

#[tauri::command]
pub fn moodle_status(state: tauri::State<AppState>) -> Result<MoodleStatus> {
    let c = state.db.lock().unwrap();
    let configured = read_cfg(&c).is_some();
    let user_id = repo::get_setting(&c, K_USERID)?
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(0);
    let last_sync = repo::get_setting(&c, K_LAST_SYNC)?
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(0);
    Ok(MoodleStatus {
        configured,
        user_id,
        last_sync,
    })
}

#[tauri::command]
pub fn moodle_disconnect(state: tauri::State<AppState>) -> Result<()> {
    let c = state.db.lock().unwrap();
    for k in [K_TOKEN, K_USERID, K_LAST_SYNC] {
        let _ = repo::set_setting(&c, k, "");
    }
    Ok(())
}

/// Pull courses → grades → assignments/calendar → announcements into the cache.
#[tauri::command]
pub async fn moodle_sync(app: AppHandle) -> Result<MoodleSummary> {
    tauri::async_runtime::spawn_blocking(move || -> Result<MoodleSummary> {
        let state = app.state::<AppState>();
        let (url, token) = {
            let c = state.db.lock().unwrap();
            read_cfg(&c).ok_or_else(|| Error::Other("Moodle is not connected".into()))?
        };
        // Fetch the userid fresh (authoritative) rather than trusting a stored value
        // — a stale/0 userid makes core_enrol_get_users_courses fail with
        // "Invalid parameter value detected". This also re-verifies the token.
        let (userid, _name) = site_info(&url, &token)?;
        {
            let c = state.db.lock().unwrap();
            repo::set_setting(&c, K_USERID, &userid.to_string())?;
        }
        let now = now_ms();
        let mut summary = MoodleSummary {
            courses: 0,
            grades: 0,
            deadlines: 0,
            announcements: 0,
        };

        // --- courses ---
        let courses_v = ws(
            &url,
            &token,
            "core_enrol_get_users_courses",
            &[("userid".into(), userid.to_string())],
        )?;
        let mut course_ids: Vec<i64> = Vec::new();
        if let Some(arr) = courses_v.as_array() {
            let c = state.db.lock().unwrap();
            for course in arr {
                let id = course.get("id").and_then(|x| x.as_i64()).unwrap_or(0);
                if id == 0 {
                    continue;
                }
                course_ids.push(id);
                let short = course.get("shortname").and_then(|x| x.as_str()).unwrap_or("");
                let full = course.get("fullname").and_then(|x| x.as_str()).unwrap_or("");
                c.execute(
                    "INSERT OR REPLACE INTO moodle_courses (id, shortname, fullname, updated_at) \
                     VALUES (?1, ?2, ?3, ?4)",
                    params![id.to_string(), short, full, now],
                )?;
                summary.courses += 1;
            }
        }

        // --- grades (per course) ---
        for cid in &course_ids {
            let gv = ws(
                &url,
                &token,
                "gradereport_user_get_grade_items",
                &[
                    ("courseid".into(), cid.to_string()),
                    ("userid".into(), userid.to_string()),
                ],
            );
            let Ok(gv) = gv else { continue }; // some courses block the grade report
            let c = state.db.lock().unwrap();
            if let Some(users) = gv.get("usergrades").and_then(|x| x.as_array()) {
                for ug in users {
                    if let Some(items) = ug.get("gradeitems").and_then(|x| x.as_array()) {
                        for it in items {
                            let item_id = it.get("id").and_then(|x| x.as_i64()).unwrap_or(0);
                            let name = it.get("itemname").and_then(|x| x.as_str()).unwrap_or("");
                            if name.is_empty() {
                                continue;
                            }
                            let grade = it
                                .get("gradeformatted")
                                .and_then(|x| x.as_str())
                                .unwrap_or("");
                            let pct = it
                                .get("percentageformatted")
                                .and_then(|x| x.as_str())
                                .unwrap_or("");
                            let fb = it.get("feedback").and_then(|x| x.as_str()).unwrap_or("");
                            c.execute(
                                "INSERT OR REPLACE INTO moodle_grades \
                                 (id, course_id, item_name, grade, percentage, feedback, updated_at) \
                                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                                params![
                                    format!("{cid}:{item_id}"),
                                    cid.to_string(),
                                    name,
                                    grade,
                                    pct,
                                    fb,
                                    now
                                ],
                            )?;
                            summary.grades += 1;
                        }
                    }
                }
            }
        }

        // --- assignments → deadlines ---
        if !course_ids.is_empty() {
            let mut ps: Vec<(String, String)> = Vec::new();
            for (i, cid) in course_ids.iter().enumerate() {
                ps.push((format!("courseids[{i}]"), cid.to_string()));
            }
            if let Ok(av) = ws(&url, &token, "mod_assign_get_assignments", &ps) {
                let c = state.db.lock().unwrap();
                if let Some(courses) = av.get("courses").and_then(|x| x.as_array()) {
                    for course in courses {
                        let cid = course.get("id").and_then(|x| x.as_i64()).unwrap_or(0);
                        if let Some(assigns) = course.get("assignments").and_then(|x| x.as_array()) {
                            for a in assigns {
                                let aid = a.get("id").and_then(|x| x.as_i64()).unwrap_or(0);
                                let name = a.get("name").and_then(|x| x.as_str()).unwrap_or("");
                                let due = a.get("duedate").and_then(|x| x.as_i64()).unwrap_or(0);
                                if due == 0 {
                                    continue; // no due date set
                                }
                                c.execute(
                                    "INSERT OR REPLACE INTO moodle_deadlines \
                                     (id, course_id, name, due_at, kind, status, url, updated_at) \
                                     VALUES (?1, ?2, ?3, ?4, 'assignment', '', ?5, ?6)",
                                    params![
                                        format!("assign:{aid}"),
                                        cid.to_string(),
                                        name,
                                        due,
                                        format!("{url}/mod/assign/view.php?id={aid}"),
                                        now
                                    ],
                                )?;
                                summary.deadlines += 1;
                            }
                        }
                    }
                }
            }
        }

        // --- calendar action events → deadlines ---
        let from = (now / 1000) - 30 * 24 * 3600; // last 30 days onward
        if let Ok(cv) = ws(
            &url,
            &token,
            "core_calendar_get_action_events_by_timesort",
            &[("timesortfrom".into(), from.to_string())],
        ) {
            let c = state.db.lock().unwrap();
            if let Some(events) = cv.get("events").and_then(|x| x.as_array()) {
                for e in events {
                    let eid = e.get("id").and_then(|x| x.as_i64()).unwrap_or(0);
                    let name = e.get("name").and_then(|x| x.as_str()).unwrap_or("");
                    let when = e.get("timesort").and_then(|x| x.as_i64()).unwrap_or(0);
                    let cid = e
                        .get("course")
                        .and_then(|cc| cc.get("id"))
                        .and_then(|x| x.as_i64())
                        .unwrap_or(0);
                    let evtype = e.get("eventtype").and_then(|x| x.as_str()).unwrap_or("");
                    let link = e
                        .get("url")
                        .and_then(|x| x.as_str())
                        .unwrap_or("")
                        .to_string();
                    if name.is_empty() || when == 0 {
                        continue;
                    }
                    c.execute(
                        "INSERT OR REPLACE INTO moodle_deadlines \
                         (id, course_id, name, due_at, kind, status, url, updated_at) \
                         VALUES (?1, ?2, ?3, ?4, ?5, '', ?6, ?7)",
                        params![
                            format!("event:{eid}"),
                            cid.to_string(),
                            name,
                            when,
                            if evtype.contains("exam") { "exam" } else { "event" },
                            link,
                            now
                        ],
                    )?;
                    summary.deadlines += 1;
                }
            }
        }

        // --- announcements (news forums) ---
        if !course_ids.is_empty() {
            let mut ps: Vec<(String, String)> = Vec::new();
            for (i, cid) in course_ids.iter().enumerate() {
                ps.push((format!("courseids[{i}]"), cid.to_string()));
            }
            if let Ok(fv) = ws(&url, &token, "mod_forum_get_forums_by_courses", &ps) {
                let news: Vec<(i64, i64)> = fv
                    .as_array()
                    .map(|arr| {
                        arr.iter()
                            .filter(|f| f.get("type").and_then(|t| t.as_str()) == Some("news"))
                            .map(|f| {
                                (
                                    f.get("id").and_then(|x| x.as_i64()).unwrap_or(0),
                                    f.get("course").and_then(|x| x.as_i64()).unwrap_or(0),
                                )
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                for (fid, cid) in news {
                    if fid == 0 {
                        continue;
                    }
                    if let Ok(dv) = ws(
                        &url,
                        &token,
                        "mod_forum_get_forum_discussions",
                        &[("forumid".into(), fid.to_string())],
                    ) {
                        let c = state.db.lock().unwrap();
                        if let Some(discs) = dv.get("discussions").and_then(|x| x.as_array()) {
                            for d in discs {
                                let did = d.get("id").and_then(|x| x.as_i64()).unwrap_or(0);
                                let subj = d.get("subject").and_then(|x| x.as_str()).unwrap_or("");
                                let msg = d.get("message").and_then(|x| x.as_str()).unwrap_or("");
                                let posted =
                                    d.get("timemodified").and_then(|x| x.as_i64()).unwrap_or(0);
                                c.execute(
                                    "INSERT OR REPLACE INTO moodle_announcements \
                                     (id, course_id, subject, message, posted_at, url, updated_at) \
                                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                                    params![
                                        format!("disc:{did}"),
                                        cid.to_string(),
                                        subj,
                                        msg,
                                        posted,
                                        format!("{url}/mod/forum/discuss.php?d={did}"),
                                        now
                                    ],
                                )?;
                                summary.announcements += 1;
                            }
                        }
                    }
                }
            }
        }

        // --- mirror deadlines/exams into the Cortex calendar (events) ---
        // Each synced deadline becomes a calendar event (id "moodle:<deadline-id>"),
        // linked to the Cortex subject whose moodle_course_id matches. Upsert keeps
        // user state (done/reminder) and created_at; assignments → tasks, exams →
        // events. This is what makes Moodle deadlines show up on the calendar.
        {
            let c = state.db.lock().unwrap();
            // course_id (text) → subject_id for linked subjects.
            let mut course_subj: std::collections::HashMap<String, String> = std::collections::HashMap::new();
            {
                let mut st = c.prepare(
                    "SELECT moodle_course_id, id FROM subjects \
                     WHERE moodle_course_id IS NOT NULL AND moodle_course_id <> ''",
                )?;
                let rows = st.query_map([], |r| {
                    Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
                })?;
                for row in rows.flatten() {
                    course_subj.insert(row.0, row.1);
                }
            }
            let deadlines: Vec<(String, String, String, i64, String)> = {
                let mut st = c.prepare(
                    "SELECT id, course_id, name, due_at, kind FROM moodle_deadlines",
                )?;
                let rows = st.query_map([], |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, Option<String>>(1)?.unwrap_or_default(),
                        r.get::<_, Option<String>>(2)?.unwrap_or_default(),
                        r.get::<_, Option<i64>>(3)?.unwrap_or(0),
                        r.get::<_, Option<String>>(4)?.unwrap_or_default(),
                    ))
                })?;
                rows.filter_map(|x| x.ok()).collect()
            };
            for (did, course_id, name, due_at, kind) in deadlines {
                if name.is_empty() || due_at == 0 {
                    continue;
                }
                let ev_id = format!("moodle:{did}");
                let subject_id = course_subj.get(&course_id).cloned();
                let ev_kind = if kind == "exam" { "event" } else { "task" };
                let start_ms = due_at * 1000;
                // New mirrored deadlines default to a day-before reminder so
                // desktop reminder polling (check_reminders) covers Moodle due
                // dates too; the upsert leaves reminder_ms alone afterwards so
                // a user's own choice is never clobbered. A moved due date
                // resets `notified` so the (re-derived) reminder fires again.
                let reminder_ms: Option<i64> =
                    (start_ms > now + 86_400_000).then(|| start_ms - 86_400_000);
                c.execute(
                    "INSERT INTO events (id, subject_id, title, description, start_ms, all_day, kind, done, reminder_ms, notified, created_at, updated_at) \
                     VALUES (?1, ?2, ?3, 'From Moodle', ?4, 0, ?5, 0, ?7, 0, ?6, ?6) \
                     ON CONFLICT(id) DO UPDATE SET \
                       subject_id=excluded.subject_id, title=excluded.title, \
                       start_ms=excluded.start_ms, kind=excluded.kind, updated_at=excluded.updated_at, \
                       notified=CASE WHEN events.start_ms<>excluded.start_ms THEN 0 ELSE events.notified END",
                    params![ev_id, subject_id, name, start_ms, ev_kind, now, reminder_ms],
                )?;
            }
        }

        {
            let c = state.db.lock().unwrap();
            repo::set_setting(&c, K_LAST_SYNC, &now.to_string())?;
        }
        // Mobile: keep the locally-scheduled deadline/exam alerts in step with
        // what we just synced — these fire on time even with the app closed.
        crate::alerts::schedule_deadline_alerts(&app);
        Ok(summary)
    })
    .await
    .map_err(|e| Error::Other(format!("moodle sync task failed: {e}")))?
}

/// Return the cached Moodle data for the UI.
#[tauri::command]
pub fn moodle_data(state: tauri::State<AppState>) -> Result<MoodleData> {
    let c = state.db.lock().unwrap();
    let courses = {
        let mut st = c.prepare("SELECT id, shortname, fullname FROM moodle_courses ORDER BY fullname")?;
        let rows = st.query_map([], |r| {
            Ok(MoodleCourse {
                id: r.get(0)?,
                shortname: r.get::<_, Option<String>>(1)?.unwrap_or_default(),
                fullname: r.get::<_, Option<String>>(2)?.unwrap_or_default(),
            })
        })?;
        rows.filter_map(|x| x.ok()).collect()
    };
    let grades = {
        let mut st = c.prepare(
            "SELECT course_id, item_name, grade, percentage, feedback FROM moodle_grades ORDER BY course_id",
        )?;
        let rows = st.query_map([], |r| {
            Ok(MoodleGrade {
                course_id: r.get(0)?,
                item_name: r.get::<_, Option<String>>(1)?.unwrap_or_default(),
                grade: r.get::<_, Option<String>>(2)?.unwrap_or_default(),
                percentage: r.get::<_, Option<String>>(3)?.unwrap_or_default(),
                feedback: r.get::<_, Option<String>>(4)?.unwrap_or_default(),
            })
        })?;
        rows.filter_map(|x| x.ok()).collect()
    };
    let deadlines = {
        let mut st = c.prepare(
            "SELECT id, course_id, name, due_at, kind, status, url FROM moodle_deadlines ORDER BY due_at",
        )?;
        let rows = st.query_map([], |r| {
            Ok(MoodleDeadline {
                id: r.get(0)?,
                course_id: r.get::<_, Option<String>>(1)?.unwrap_or_default(),
                name: r.get::<_, Option<String>>(2)?.unwrap_or_default(),
                due_at: r.get::<_, Option<i64>>(3)?.unwrap_or(0),
                kind: r.get::<_, Option<String>>(4)?.unwrap_or_default(),
                status: r.get::<_, Option<String>>(5)?.unwrap_or_default(),
                url: r.get::<_, Option<String>>(6)?.unwrap_or_default(),
            })
        })?;
        rows.filter_map(|x| x.ok()).collect()
    };
    let announcements = {
        let mut st = c.prepare(
            "SELECT id, course_id, subject, message, posted_at, url FROM moodle_announcements ORDER BY posted_at DESC",
        )?;
        let rows = st.query_map([], |r| {
            Ok(MoodleAnnouncement {
                id: r.get(0)?,
                course_id: r.get::<_, Option<String>>(1)?.unwrap_or_default(),
                subject: r.get::<_, Option<String>>(2)?.unwrap_or_default(),
                message: r.get::<_, Option<String>>(3)?.unwrap_or_default(),
                posted_at: r.get::<_, Option<i64>>(4)?.unwrap_or(0),
                url: r.get::<_, Option<String>>(5)?.unwrap_or_default(),
            })
        })?;
        rows.filter_map(|x| x.ok()).collect()
    };
    Ok(MoodleData {
        courses,
        grades,
        deadlines,
        announcements,
    })
}

/// Link (or unlink, with course_id=None) a Cortex subject to a Moodle course.
#[tauri::command]
pub fn moodle_link_subject(
    state: tauri::State<AppState>,
    subject_id: String,
    course_id: Option<String>,
) -> Result<()> {
    let c = state.db.lock().unwrap();
    c.execute(
        "UPDATE subjects SET moodle_course_id=?2, updated_at=?3 WHERE id=?1",
        params![subject_id, course_id, now_ms()],
    )?;
    Ok(())
}

/// Normalize for matching: lowercase alphanumerics only.
fn squash(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(|c| c.to_lowercase())
        .collect()
}

/// Auto-link unlinked subjects to Moodle courses by fuzzy code/name match.
/// Returns the number of subjects newly linked.
#[tauri::command]
pub fn moodle_autolink(state: tauri::State<AppState>) -> Result<usize> {
    let c = state.db.lock().unwrap();
    let courses: Vec<(String, String, String)> = {
        let mut st = c.prepare("SELECT id, shortname, fullname FROM moodle_courses")?;
        let rows = st.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, Option<String>>(1)?.unwrap_or_default(),
                r.get::<_, Option<String>>(2)?.unwrap_or_default(),
            ))
        })?;
        rows.filter_map(|x| x.ok()).collect()
    };
    let subjects: Vec<(String, String, String)> = {
        let mut st = c.prepare(
            "SELECT id, name, IFNULL(code,'') FROM subjects WHERE moodle_course_id IS NULL OR moodle_course_id=''",
        )?;
        let rows = st.query_map([], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?))
        })?;
        rows.filter_map(|x| x.ok()).collect()
    };

    let mut linked = 0usize;
    for (sid, name, code) in &subjects {
        let n_name = squash(name);
        let n_code = squash(code);
        let mut best: Option<&str> = None;
        for (cid, short, full) in &courses {
            let n_short = squash(short);
            let n_full = squash(full);
            let code_hit = !n_code.is_empty()
                && (n_short.contains(&n_code) || n_full.contains(&n_code));
            let name_hit = !n_name.is_empty()
                && n_name.len() >= 4
                && (n_full.contains(&n_name)
                    || n_name.contains(&n_full) && !n_full.is_empty()
                    || n_short.contains(&n_name));
            if code_hit || name_hit {
                best = Some(cid);
                if code_hit {
                    break; // code match is high-confidence
                }
            }
        }
        if let Some(cid) = best {
            c.execute(
                "UPDATE subjects SET moodle_course_id=?2, updated_at=?3 WHERE id=?1",
                params![sid, cid, now_ms()],
            )?;
            linked += 1;
        }
    }
    Ok(linked)
}
