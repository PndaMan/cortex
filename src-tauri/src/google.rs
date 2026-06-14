//! Google Calendar OAuth (installed-app / loopback flow) + two-way sync.
//!
//! Mirrored 1:1 in `src/lib/api.ts`. The network/loopback work runs off the UI
//! thread via `tauri::async_runtime::spawn_blocking` (same pattern as the heavy
//! commands in `commands.rs`). It uses ONLY the existing deps — the blocking
//! `reqwest` client from `crate::commands::http_client`, `serde_json`, std's
//! `TcpListener`/`Command` — and degrades gracefully when unconfigured.
//!
//! ## OAuth model
//! This is the Google "Desktop app" (installed-app) flow: there is no client
//! secret to protect on a public web server, so the secret is stored locally and
//! the redirect lands on a one-shot `127.0.0.1:<random-port>` loopback listener.
//!
//! Settings keys used:
//! - `google_client_id`, `google_client_secret` — pasted in Settings (required).
//! - `google_access_token`, `google_refresh_token`, `google_token_expiry` (epoch ms).
//! - `google_calendar_id` (default "primary").

use crate::commands::http_client;
use crate::db::AppState;
use crate::error::{Error, Result};
use crate::repo;
use rusqlite::Connection;
use serde::Serialize;
use std::io::{Read, Write};
use std::net::TcpListener;
use tauri::{AppHandle, Manager};

const TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
const AUTH_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const SCOPE: &str = "https://www.googleapis.com/auth/calendar";
/// Refresh slightly before the real expiry so an in-flight request can't race it.
const EXPIRY_SKEW_MS: i64 = 60_000;

#[derive(Debug, Clone, Serialize)]
pub struct GoogleStatus {
    pub connected: bool,
    pub email: Option<String>,
    pub configured: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct SyncResult {
    pub pulled: i64,
    pub pushed: i64,
}

// ---- small helpers -----------------------------------------------------

/// Minimal percent-encoding for query/form values (no extra crate). Encodes
/// everything except the RFC 3986 unreserved set, so it is safe for both URL
/// query parameters and `application/x-www-form-urlencoded` bodies.
fn pct(input: &str) -> String {
    let mut out = String::with_capacity(input.len() * 3);
    for b in input.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// Read a setting, trimming whitespace and treating empty as absent (a pasted
/// credential with a trailing newline would otherwise break header/body values).
fn setting(conn: &Connection, key: &str) -> Result<Option<String>> {
    Ok(repo::get_setting(conn, key)?
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty()))
}

/// `(client_id, client_secret)` if BOTH are configured, else None.
fn client_creds(conn: &Connection) -> Result<Option<(String, String)>> {
    match (
        setting(conn, "google_client_id")?,
        setting(conn, "google_client_secret")?,
    ) {
        (Some(id), Some(secret)) => Ok(Some((id, secret))),
        _ => Ok(None),
    }
}

fn calendar_id(conn: &Connection) -> Result<String> {
    Ok(setting(conn, "google_calendar_id")?.unwrap_or_else(|| "primary".to_string()))
}

fn now_ms() -> i64 {
    crate::db::now_ms()
}

/// Open a URL in the user's default browser. Best-effort across platforms.
fn open_browser(url: &str) {
    use std::process::Command;
    #[cfg(target_os = "linux")]
    let cmds: &[&str] = &["xdg-open"];
    #[cfg(target_os = "macos")]
    let cmds: &[&str] = &["open"];
    #[cfg(target_os = "windows")]
    let cmds: &[&str] = &["explorer"];
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    let cmds: &[&str] = &["xdg-open", "open"];

    for c in cmds {
        if Command::new(c).arg(url).spawn().is_ok() {
            return;
        }
    }
    // Last resort on Windows: `cmd /C start`.
    #[cfg(target_os = "windows")]
    {
        let _ = Command::new("cmd").args(["/C", "start", "", url]).spawn();
    }
}

// ---- date <-> epoch ms (no chrono) -------------------------------------

/// Parse an RFC 3339 / ISO-8601 timestamp (e.g. `2026-06-03T14:30:00-04:00` or
/// `2026-06-03T18:30:00.000Z`) to epoch milliseconds. Handles an explicit
/// `Z` or `±HH:MM` offset and an optional fractional-seconds part. Returns None
/// if the shape is unexpected (caller falls back to skipping the field).
fn parse_rfc3339_ms(s: &str) -> Option<i64> {
    let s = s.trim();
    let (date_part, rest) = s.split_once('T')?;
    let mut d = date_part.split('-');
    let year: i64 = d.next()?.parse().ok()?;
    let month: i64 = d.next()?.parse().ok()?;
    let day: i64 = d.next()?.parse().ok()?;

    // Split off the timezone designator from the time-of-day.
    let (time_part, offset_min) = if let Some(stripped) = rest.strip_suffix('Z') {
        (stripped, 0i64)
    } else if let Some(idx) = rest.rfind(['+', '-']) {
        // The sign must come after the seconds (index > 0) to be an offset, not
        // a malformed value. Times are always HH:MM:SS so idx is well past 0.
        let (t, off) = rest.split_at(idx);
        let sign = if off.starts_with('-') { -1 } else { 1 };
        let off = &off[1..];
        let (oh, om) = off.split_once(':').unwrap_or((off, "0"));
        let oh: i64 = oh.parse().ok()?;
        let om: i64 = om.parse().ok()?;
        (t, sign * (oh * 60 + om))
    } else {
        (rest, 0i64)
    };

    let mut t = time_part.split(':');
    let hour: i64 = t.next()?.parse().ok()?;
    let minute: i64 = t.next()?.parse().ok()?;
    // Seconds may carry a fractional part (".000") — keep only whole seconds.
    let sec_raw = t.next().unwrap_or("0");
    let sec_whole = sec_raw.split('.').next().unwrap_or("0");
    let second: i64 = sec_whole.parse().ok()?;

    let days = days_from_civil(year, month, day);
    let secs = days * 86_400 + hour * 3_600 + minute * 60 + second - offset_min * 60;
    Some(secs * 1_000)
}

/// Parse an all-day `date` value (`YYYY-MM-DD`) to epoch ms at UTC midnight.
fn parse_date_ms(s: &str) -> Option<i64> {
    let mut d = s.trim().split('-');
    let year: i64 = d.next()?.parse().ok()?;
    let month: i64 = d.next()?.parse().ok()?;
    let day: i64 = d.next()?.parse().ok()?;
    Some(days_from_civil(year, month, day) * 86_400 * 1_000)
}

/// Days since the Unix epoch for a civil (proleptic Gregorian) date.
/// From Howard Hinnant's well-known `days_from_civil` algorithm.
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400; // [0, 399]
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1; // [0, 365]
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy; // [0, 146096]
    era * 146_097 + doe - 719_468
}

/// Inverse of `days_from_civil` — civil date (y, m, d) from days since epoch.
fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365; // [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = doy - (153 * mp + 2) / 5 + 1; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 }; // [1, 12]
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// Format epoch ms as an RFC 3339 UTC timestamp (`...Z`) for the Google API.
fn format_rfc3339_utc(ms: i64) -> String {
    let secs = ms.div_euclid(1_000);
    let days = secs.div_euclid(86_400);
    let tod = secs.rem_euclid(86_400);
    let (y, m, d) = civil_from_days(days);
    let (hh, mm, ss) = (tod / 3_600, (tod % 3_600) / 60, tod % 60);
    format!("{y:04}-{m:02}-{d:02}T{hh:02}:{mm:02}:{ss:02}Z")
}

// ---- token handling ----------------------------------------------------

/// Persist an access token + its expiry (and refresh token, when present) from
/// a Google token endpoint response.
fn store_tokens(conn: &Connection, token: &serde_json::Value) -> Result<()> {
    if let Some(access) = token.get("access_token").and_then(|v| v.as_str()) {
        repo::set_setting(conn, "google_access_token", access)?;
    }
    // refresh_token is only returned on the FIRST consent (or with prompt=consent)
    // — never clobber an existing one with an absent value.
    if let Some(refresh) = token.get("refresh_token").and_then(|v| v.as_str()) {
        if !refresh.is_empty() {
            repo::set_setting(conn, "google_refresh_token", refresh)?;
        }
    }
    let expires_in = token.get("expires_in").and_then(|v| v.as_i64()).unwrap_or(3600);
    let expiry = now_ms() + expires_in * 1_000;
    repo::set_setting(conn, "google_token_expiry", &expiry.to_string())?;
    Ok(())
}

/// Return a valid access token, refreshing it via the refresh token when the
/// stored one is missing or (about to be) expired. Errors clearly when there is
/// no refresh token (the user must connect first).
fn valid_access_token(conn: &Connection, client_id: &str, client_secret: &str) -> Result<String> {
    let access = setting(conn, "google_access_token")?;
    let expiry: i64 = setting(conn, "google_token_expiry")?
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);

    if let Some(tok) = &access {
        if now_ms() + EXPIRY_SKEW_MS < expiry {
            return Ok(tok.clone());
        }
    }

    // Need to refresh.
    let refresh = setting(conn, "google_refresh_token")?.ok_or_else(|| {
        Error::Other("Google Calendar is not connected — click \"Connect Google\" first.".into())
    })?;

    let body = format!(
        "client_id={}&client_secret={}&refresh_token={}&grant_type=refresh_token",
        pct(client_id),
        pct(client_secret),
        pct(&refresh),
    );
    let client = http_client(30);
    let resp = client
        .post(TOKEN_URL)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(body)
        .send()?;
    let status = resp.status();
    let json: serde_json::Value = resp.json().unwrap_or(serde_json::Value::Null);
    if !status.is_success() {
        let msg = json
            .get("error_description")
            .or_else(|| json.get("error"))
            .and_then(|v| v.as_str())
            .unwrap_or("token refresh failed");
        return Err(Error::Other(format!("Google token refresh failed: {msg}")));
    }
    store_tokens(conn, &json)?;
    setting(conn, "google_access_token")?
        .ok_or_else(|| Error::Other("Google did not return an access token".into()))
}

/// Fetch the connected account's primary email (best-effort; None on failure).
fn fetch_email(access_token: &str) -> Option<String> {
    let client = http_client(15);
    let resp = client
        .get("https://www.googleapis.com/oauth2/v2/userinfo")
        .bearer_auth(access_token)
        .send()
        .ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let json: serde_json::Value = resp.json().ok()?;
    json.get("email").and_then(|v| v.as_str()).map(|s| s.to_string())
}

// ---- loopback OAuth helper --------------------------------------------

/// Accept exactly one HTTP request on `listener`, extract the `code` query
/// parameter from the request line, reply with a friendly HTML page, and return
/// the code. Times out via the socket read timeout to avoid hanging forever.
fn await_oauth_code(listener: &TcpListener) -> Result<String> {
    let (mut stream, _addr) = listener
        .accept()
        .map_err(|e| Error::Other(format!("OAuth loopback accept failed: {e}")))?;
    let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(120)));

    // The request line + headers are small; a single bounded read is enough to
    // capture "GET /?code=...&scope=... HTTP/1.1".
    let mut buf = [0u8; 8192];
    let n = stream.read(&mut buf).unwrap_or(0);
    let req = String::from_utf8_lossy(&buf[..n]);
    let first_line = req.lines().next().unwrap_or("");

    // first_line looks like: GET /?code=XYZ&scope=... HTTP/1.1
    let path = first_line.split_whitespace().nth(1).unwrap_or("");
    let query = path.split_once('?').map(|(_, q)| q).unwrap_or("");

    let mut code: Option<String> = None;
    let mut err: Option<String> = None;
    for pair in query.split('&') {
        let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
        match k {
            "code" => code = Some(url_decode(v)),
            "error" => err = Some(url_decode(v)),
            _ => {}
        }
    }

    let connected = code.is_some();
    let html = if connected {
        "<html><body style=\"font-family:system-ui;background:#111c18;color:#e6efe9;\
         display:flex;align-items:center;justify-content:center;height:100vh;margin:0\">\
         <div style=\"text-align:center\"><h2 style=\"color:#2dd5b7\">Cortex connected ✓</h2>\
         <p>You can close this tab and return to Cortex.</p></div></body></html>"
    } else {
        "<html><body style=\"font-family:system-ui;background:#111c18;color:#e6efe9;\
         display:flex;align-items:center;justify-content:center;height:100vh;margin:0\">\
         <div style=\"text-align:center\"><h2>Authorization failed</h2>\
         <p>Return to Cortex and try connecting again.</p></div></body></html>"
    };
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        html.len(),
        html
    );
    let _ = stream.write_all(response.as_bytes());
    let _ = stream.flush();

    if let Some(e) = err {
        return Err(Error::Other(format!("Google authorization was denied: {e}")));
    }
    code.ok_or_else(|| Error::Other("no authorization code returned by Google".into()))
}

/// Decode an `application/x-www-form-urlencoded` value (`%XX` + `+`). Used for
/// the redirect query string only.
fn url_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' if i + 2 < bytes.len() => {
                let hi = (bytes[i + 1] as char).to_digit(16);
                let lo = (bytes[i + 2] as char).to_digit(16);
                if let (Some(h), Some(l)) = (hi, lo) {
                    out.push((h * 16 + l) as u8);
                    i += 3;
                    continue;
                }
                out.push(bytes[i]);
                i += 1;
            }
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

// ---- status struct builder --------------------------------------------

fn status_from_settings(conn: &Connection) -> Result<GoogleStatus> {
    let configured = client_creds(conn)?.is_some();
    let connected = setting(conn, "google_refresh_token")?.is_some();
    let email = setting(conn, "google_connected_email")?;
    Ok(GoogleStatus {
        connected,
        email,
        configured,
    })
}

// ---- commands ----------------------------------------------------------

/// Report whether Google credentials are configured and whether we hold a
/// refresh token (connected). Never touches the network.
#[tauri::command]
pub async fn google_status(app: AppHandle) -> Result<GoogleStatus> {
    tauri::async_runtime::spawn_blocking(move || -> Result<GoogleStatus> {
        let state = app.state::<AppState>();
        let c = state.db.lock().unwrap();
        status_from_settings(&c)
    })
    .await
    .map_err(|e| Error::Other(format!("background task failed: {e}")))?
}

/// One calendar in the user's Google calendar list, for the sync-selection UI.
#[derive(serde::Serialize)]
pub struct GoogleCalendar {
    pub id: String,
    pub summary: String,
    pub primary: bool,
    pub selected: bool,
    pub color: String, // backgroundColor hex (for the swatch)
}

/// List the connected account's calendars so the user can choose which ones to
/// pull (e.g. a separate university timetable calendar). `selected` reflects the
/// saved `google_pull_calendars` set (defaulting to the primary/push calendar).
#[tauri::command]
pub async fn google_list_calendars(app: AppHandle) -> Result<Vec<GoogleCalendar>> {
    tauri::async_runtime::spawn_blocking(move || -> Result<Vec<GoogleCalendar>> {
        let state = app.state::<AppState>();
        let (client_id, client_secret, cal_id, selected_csv) = {
            let c = state.db.lock().unwrap();
            let (id, secret) = client_creds(&c)?.ok_or_else(|| {
                Error::Other("Google Calendar is not configured — add credentials in Settings.".into())
            })?;
            (
                id,
                secret,
                calendar_id(&c)?,
                setting(&c, "google_pull_calendars")?.unwrap_or_default(),
            )
        };
        let access = {
            let c = state.db.lock().unwrap();
            valid_access_token(&c, &client_id, &client_secret)?
        };
        let selected: std::collections::HashSet<String> = selected_csv
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        // No explicit selection yet → the push/primary calendar is implicitly on.
        let default_on = selected.is_empty();

        let client = http_client(30);
        let resp = client
            .get("https://www.googleapis.com/calendar/v3/users/me/calendarList?maxResults=250")
            .bearer_auth(&access)
            .send()?;
        let status = resp.status();
        let json: serde_json::Value = resp.json().unwrap_or(serde_json::Value::Null);
        if !status.is_success() {
            let msg = json
                .get("error")
                .and_then(|e| e.get("message"))
                .and_then(|v| v.as_str())
                .unwrap_or("calendar list failed");
            return Err(Error::Other(format!("Could not list Google calendars: {msg}")));
        }
        let mut out = Vec::new();
        if let Some(items) = json.get("items").and_then(|v| v.as_array()) {
            for cal in items {
                let id = cal.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                if id.is_empty() {
                    continue;
                }
                let primary = cal.get("primary").and_then(|v| v.as_bool()).unwrap_or(false);
                let raw = cal
                    .get("summaryOverride")
                    .and_then(|v| v.as_str())
                    .or_else(|| cal.get("summary").and_then(|v| v.as_str()))
                    .unwrap_or("");
                // Strip control/replacement chars (some calendars carry a stray
                // glyph that renders as a box) and fall back to a friendly label.
                let cleaned: String = raw
                    .chars()
                    .filter(|c| !c.is_control() && *c != '\u{fffd}')
                    .collect::<String>()
                    .trim()
                    .to_string();
                let summary = if cleaned.is_empty() || cleaned.chars().all(|c| c.is_ascii_digit()) {
                    "Unnamed calendar".to_string()
                } else {
                    cleaned
                };
                let color = cal
                    .get("backgroundColor")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let sel = selected.contains(&id) || (default_on && (primary || id == cal_id));
                out.push(GoogleCalendar { id, summary, primary, selected: sel, color });
            }
        }
        Ok(out)
    })
    .await
    .map_err(|e| Error::Other(format!("background task failed: {e}")))?
}

/// Run the installed-app OAuth loopback flow: open the consent page in the
/// browser, capture the redirect on a one-shot localhost listener, exchange the
/// code for tokens, and store them. Returns the resulting status.
#[tauri::command]
pub async fn google_connect(app: AppHandle) -> Result<GoogleStatus> {
    tauri::async_runtime::spawn_blocking(move || -> Result<GoogleStatus> {
        let state = app.state::<AppState>();

        // 1. require client credentials.
        let (client_id, client_secret) = {
            let c = state.db.lock().unwrap();
            client_creds(&c)?.ok_or_else(|| {
                Error::Other(
                    "Paste your Google OAuth Client ID and Client secret above (Google Cloud → \
                     APIs & Services → Credentials → OAuth client, type \"Desktop app\"), then \
                     click Connect."
                        .into(),
                )
            })?
        };

        // 2. bind the loopback listener on an ephemeral port.
        let listener = TcpListener::bind("127.0.0.1:0")
            .map_err(|e| Error::Other(format!("could not bind loopback listener: {e}")))?;
        let port = listener
            .local_addr()
            .map_err(|e| Error::Other(e.to_string()))?
            .port();
        let redirect_uri = format!("http://127.0.0.1:{port}");

        // 3. build the consent URL and open it.
        let auth_url = format!(
            "{AUTH_URL}?response_type=code&client_id={}&redirect_uri={}&scope={}&access_type=offline&prompt=consent&include_granted_scopes=true",
            pct(&client_id),
            pct(&redirect_uri),
            pct(SCOPE),
        );
        open_browser(&auth_url);

        // 4. wait for the single redirect carrying the code.
        let code = await_oauth_code(&listener)?;

        // 5. exchange the code for tokens.
        let body = format!(
            "code={}&client_id={}&client_secret={}&redirect_uri={}&grant_type=authorization_code",
            pct(&code),
            pct(&client_id),
            pct(&client_secret),
            pct(&redirect_uri),
        );
        let client = http_client(30);
        let resp = client
            .post(TOKEN_URL)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(body)
            .send()?;
        let ok = resp.status().is_success();
        let json: serde_json::Value = resp.json().unwrap_or(serde_json::Value::Null);
        if !ok {
            let msg = json
                .get("error_description")
                .or_else(|| json.get("error"))
                .and_then(|v| v.as_str())
                .unwrap_or("token exchange failed");
            return Err(Error::Other(format!("Google token exchange failed: {msg}")));
        }

        // 6. persist tokens + the account email, return status.
        let access = json.get("access_token").and_then(|v| v.as_str()).map(|s| s.to_string());
        {
            let c = state.db.lock().unwrap();
            store_tokens(&c, &json)?;
        }
        if let Some(tok) = access {
            if let Some(email) = fetch_email(&tok) {
                let c = state.db.lock().unwrap();
                repo::set_setting(&c, "google_connected_email", &email)?;
            }
        }

        let c = state.db.lock().unwrap();
        status_from_settings(&c)
    })
    .await
    .map_err(|e| Error::Other(format!("background task failed: {e}")))?
}

/// Clear the stored Google tokens (keep the client id/secret so the user can
/// reconnect without re-pasting credentials).
#[tauri::command]
pub async fn google_disconnect(app: AppHandle) -> Result<GoogleStatus> {
    tauri::async_runtime::spawn_blocking(move || -> Result<GoogleStatus> {
        let state = app.state::<AppState>();
        let c = state.db.lock().unwrap();
        for k in [
            "google_access_token",
            "google_refresh_token",
            "google_token_expiry",
            "google_connected_email",
        ] {
            // Clearing to empty string is treated as "absent" by `setting`.
            repo::set_setting(&c, k, "")?;
        }
        status_from_settings(&c)
    })
    .await
    .map_err(|e| Error::Other(format!("background task failed: {e}")))?
}

/// Two-way sync. PULL upcoming events from Google into the local DB (upsert by
/// google id), then PUSH local events that have no google id yet.
#[tauri::command]
pub async fn google_sync(app: AppHandle) -> Result<SyncResult> {
    tauri::async_runtime::spawn_blocking(move || -> Result<SyncResult> {
        let state = app.state::<AppState>();

        // Gather credentials + a valid access token up front.
        let (client_id, client_secret, cal_id) = {
            let c = state.db.lock().unwrap();
            let (id, secret) = client_creds(&c)?.ok_or_else(|| {
                Error::Other("Google Calendar is not configured — add credentials in Settings.".into())
            })?;
            (id, secret, calendar_id(&c)?)
        };
        let access = {
            let c = state.db.lock().unwrap();
            valid_access_token(&c, &client_id, &client_secret)?
        };

        let client = http_client(30);

        // ---- PULL ----------------------------------------------------
        // Pull from every calendar the user selected (so a separate "university"
        // calendar syncs too), defaulting to just the push calendar. Events from
        // 30 days ago onward, expanded to single instances.
        let pull_ids: Vec<String> = {
            let c = state.db.lock().unwrap();
            let csv = setting(&c, "google_pull_calendars")?.unwrap_or_default();
            let ids: Vec<String> = csv
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            if ids.is_empty() { vec![cal_id.clone()] } else { ids }
        };
        // Calendar id → its colour (hex), so pulled events (and the subjects they
        // match) can adopt the same colour you see in Google Calendar.
        let cal_colors: std::collections::HashMap<String, String> = {
            let mut m = std::collections::HashMap::new();
            if let Ok(resp) = client
                .get("https://www.googleapis.com/calendar/v3/users/me/calendarList?maxResults=250")
                .bearer_auth(&access)
                .send()
            {
                if let Ok(json) = resp.json::<serde_json::Value>() {
                    if let Some(items) = json.get("items").and_then(|v| v.as_array()) {
                        for cal in items {
                            if let (Some(id), Some(color)) = (
                                cal.get("id").and_then(|v| v.as_str()),
                                cal.get("backgroundColor").and_then(|v| v.as_str()),
                            ) {
                                m.insert(id.to_string(), color.to_string());
                            }
                        }
                    }
                }
            }
            m
        };
        let time_min = format_rfc3339_utc(now_ms() - 30 * 86_400 * 1_000);
        let mut pulled = 0i64;
        // subject_id → its Google calendar colour. Events themselves stay
        // colourless so they inherit (and follow) their subject's colour; only the
        // subject adopts the Google colour. Per-event colour stays a manual choice.
        let mut subj_colors: std::collections::HashMap<String, String> = std::collections::HashMap::new();
        for pull_id in &pull_ids {
            let cal_color = cal_colors.get(pull_id).cloned();
            let url = format!(
                "https://www.googleapis.com/calendar/v3/calendars/{}/events?singleEvents=true&maxResults=250&orderBy=startTime&timeMin={}",
                pct(pull_id),
                pct(&time_min),
            );
            let resp = client.get(&url).bearer_auth(&access).send()?;
            let status = resp.status();
            let json: serde_json::Value = resp.json().unwrap_or(serde_json::Value::Null);
            if !status.is_success() {
                // One calendar failing (e.g. lost access) shouldn't abort the whole
                // sync — skip it and keep going.
                continue;
            }
            if let Some(items) = json.get("items").and_then(|v| v.as_array()) {
                let c = state.db.lock().unwrap();
                for ev in items {
                    // Skip cancelled events.
                    if ev.get("status").and_then(|v| v.as_str()) == Some("cancelled") {
                        continue;
                    }
                    let Some(gid) = ev.get("id").and_then(|v| v.as_str()) else {
                        continue;
                    };
                    let title = ev
                        .get("summary")
                        .and_then(|v| v.as_str())
                        .unwrap_or("(untitled)")
                        .to_string();
                    let description = ev.get("description").and_then(|v| v.as_str()).map(|s| s.to_string());
                    let location = ev.get("location").and_then(|v| v.as_str()).map(|s| s.to_string());

                    // start/end: either { dateTime } (timed) or { date } (all-day).
                    let (start_ms, all_day) = match parse_endpoint(ev.get("start")) {
                        Some(v) => v,
                        None => continue, // no usable start — skip rather than guess
                    };
                    let end_ms = parse_endpoint(ev.get("end")).map(|(ms, _)| ms);

                    // Record the subject's Google colour (don't colour the event —
                    // it inherits the subject); retag fills subject_id afterwards.
                    if let Some(col) = &cal_color {
                        if let Ok(Some(sid)) = repo::match_event_subject(&c, &title) {
                            subj_colors.entry(sid).or_insert_with(|| col.clone());
                        }
                    }
                    repo::upsert_event_by_google_id(
                        &c,
                        gid,
                        None, // subject_id — filled by retag below (respects manual moves)
                        &title,
                        description.as_deref(),
                        location.as_deref(),
                        None, // no event colour — inherits the subject's colour
                        start_ms,
                        end_ms,
                        all_day,
                        "event",
                        None, // reminder_ms
                    )?;
                    pulled += 1;
                }
            }
        }

        // Auto-file freshly pulled timetable events to their Cortex subject by
        // matching the title (no AI) — name/code/alias.
        {
            let c = state.db.lock().unwrap();
            let _ = repo::retag_calendar_events(&c);
            // Subjects adopt their Google calendar colour; events stay colourless
            // and inherit it. (Per-event colour remains a manual override.)
            for (sid, col) in &subj_colors {
                let _ = c.execute(
                    "UPDATE subjects SET color=?2, updated_at=?3 WHERE id=?1",
                    rusqlite::params![sid, col, now_ms()],
                );
            }
        }

        // ---- PUSH ----------------------------------------------------
        // Local events with no google id yet → create them on Google, then
        // record the returned id so they aren't pushed again.
        let to_push = {
            let c = state.db.lock().unwrap();
            repo::list_events(&c, None, None, None)?
                .into_iter()
                .filter(|e| e.google_id.is_none())
                .collect::<Vec<_>>()
        };

        let mut pushed = 0i64;
        for ev in to_push {
            let mut body = serde_json::Map::new();
            body.insert("summary".into(), serde_json::Value::String(ev.title.clone()));
            if let Some(d) = &ev.description {
                body.insert("description".into(), serde_json::Value::String(d.clone()));
            }
            if let Some(l) = &ev.location {
                body.insert("location".into(), serde_json::Value::String(l.clone()));
            }

            if ev.all_day {
                // All-day events use { date: YYYY-MM-DD }. End date is exclusive
                // in the Google API, so default to the day after start.
                let start_date = format_date(ev.start_ms);
                let end_date = format_date(ev.end_ms.unwrap_or(ev.start_ms + 86_400_000));
                body.insert("start".into(), serde_json::json!({ "date": start_date }));
                body.insert("end".into(), serde_json::json!({ "date": end_date }));
            } else {
                let start = format_rfc3339_utc(ev.start_ms);
                // Google requires an end; default to +1h when the local event has none.
                let end = format_rfc3339_utc(ev.end_ms.unwrap_or(ev.start_ms + 3_600_000));
                body.insert("start".into(), serde_json::json!({ "dateTime": start }));
                body.insert("end".into(), serde_json::json!({ "dateTime": end }));
            }

            let create_url = format!(
                "https://www.googleapis.com/calendar/v3/calendars/{}/events",
                pct(&cal_id)
            );
            let resp = client
                .post(&create_url)
                .bearer_auth(&access)
                .json(&serde_json::Value::Object(body))
                .send();
            // Push is best-effort per-event: a single failure shouldn't abort the
            // whole sync (PULL has already succeeded by this point).
            let Ok(resp) = resp else { continue };
            if !resp.status().is_success() {
                continue;
            }
            let created: serde_json::Value = match resp.json() {
                Ok(v) => v,
                Err(_) => continue,
            };
            if let Some(gid) = created.get("id").and_then(|v| v.as_str()) {
                let c = state.db.lock().unwrap();
                repo::set_event_google_id(&c, &ev.id, gid)?;
                pushed += 1;
            }
        }

        Ok(SyncResult { pulled, pushed })
    })
    .await
    .map_err(|e| Error::Other(format!("background task failed: {e}")))?
}

/// Parse a Google event start/end node into `(epoch_ms, all_day)`.
/// `{ "dateTime": "..." }` → timed; `{ "date": "YYYY-MM-DD" }` → all-day.
fn parse_endpoint(node: Option<&serde_json::Value>) -> Option<(i64, bool)> {
    let node = node?;
    if let Some(dt) = node.get("dateTime").and_then(|v| v.as_str()) {
        return parse_rfc3339_ms(dt).map(|ms| (ms, false));
    }
    if let Some(d) = node.get("date").and_then(|v| v.as_str()) {
        return parse_date_ms(d).map(|ms| (ms, true));
    }
    None
}

/// Format epoch ms as `YYYY-MM-DD` (UTC) for all-day event push.
fn format_date(ms: i64) -> String {
    let days = ms.div_euclid(86_400 * 1_000);
    let (y, m, d) = civil_from_days(days);
    format!("{y:04}-{m:02}-{d:02}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rfc3339_roundtrip_utc() {
        // 2026-06-03T18:30:00Z
        let ms = parse_rfc3339_ms("2026-06-03T18:30:00Z").unwrap();
        assert_eq!(format_rfc3339_utc(ms), "2026-06-03T18:30:00Z");
    }

    #[test]
    fn rfc3339_offset_applied() {
        // -04:00 means 14:30 local == 18:30 UTC.
        let a = parse_rfc3339_ms("2026-06-03T14:30:00-04:00").unwrap();
        let b = parse_rfc3339_ms("2026-06-03T18:30:00Z").unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn rfc3339_fractional_seconds() {
        let a = parse_rfc3339_ms("2026-06-03T18:30:00.000Z").unwrap();
        let b = parse_rfc3339_ms("2026-06-03T18:30:00Z").unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn all_day_date_to_midnight_utc() {
        let ms = parse_date_ms("2026-06-03").unwrap();
        assert_eq!(format_date(ms), "2026-06-03");
        assert_eq!(format_rfc3339_utc(ms), "2026-06-03T00:00:00Z");
    }

    #[test]
    fn pct_encodes_reserved() {
        assert_eq!(pct("a b/c?d=e"), "a%20b%2Fc%3Fd%3De");
        assert_eq!(pct("safe-_.~"), "safe-_.~");
    }

    #[test]
    fn url_decode_basic() {
        assert_eq!(url_decode("a%20b+c"), "a b c");
        assert_eq!(url_decode("4%2F0Ab"), "4/0Ab");
    }
}
