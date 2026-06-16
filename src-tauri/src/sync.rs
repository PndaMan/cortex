//! Live homelab sync (smart row-level merge, whole vault).
//!
//! The vault is the local SQLite DB **plus** the on-disk binary originals
//! (`sources/`, `recordings/`). Both are synced to a WebDAV endpoint.
//!
//! Conflict handling is per-record, not per-database:
//!   • On launch we pull the remote DB and MERGE it into the local one — union
//!     by primary key, newest `updated_at` wins. A row present on only one side
//!     is never dropped, so nothing is deleted "needlessly".
//!   • Genuine deletes propagate via the `tombstones` table (written by DB
//!     triggers): a delete wins over a row unless the row was edited *after* the
//!     delete. See migration 0019.
//!   • Binary originals sync both directions and are never deleted. Filenames
//!     are `{source_id}.{ext}`, so after syncing we re-point each source's
//!     (absolute, machine-specific) `stored_path` to the local copy — without
//!     bumping `updated_at`, so the path difference can't cause sync churn.
//!
//! A logical stamp file (`cortex.stamp`, epoch-ms of the last push) decides
//! when the remote has advanced; it's immune to clock/header quirks.

use crate::db::AppState;
use crate::error::{Error, Result};
use crate::homelab;
use crate::repo;
use rusqlite::Connection;
use std::collections::HashSet;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Manager};

const K_ENABLED: &str = "sync_enabled";
const K_URL: &str = "sync_url"; // local / LAN endpoint
const K_URL_TS: &str = "sync_url_tailscale";
const K_URL_PUB: &str = "sync_url_public";
const K_MODE: &str = "sync_mode"; // auto | local | tailscale | public
const K_USER: &str = "sync_user";
const K_PASS: &str = "sync_pass";
const K_LAST_AT: &str = "sync_last_at"; // epoch-ms of the version we currently hold

const REMOTE_DB: &str = "cortex.db";
const REMOTE_STAMP: &str = "cortex.stamp";

pub struct SyncCfg {
    pub url: String,
    pub user: String,
    pub pass: String,
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// A configured endpoint URL (trimmed), or None if unset/empty.
fn endpoint(c: &Connection, key: &str) -> Option<String> {
    let u = repo::get_setting(c, key).ok().flatten()?;
    let u = u.trim().trim_end_matches('/').to_string();
    if u.is_empty() { None } else { Some(u) }
}

/// Quick reachability probe for an endpoint (short timeout). Reachable when the
/// stamp file returns any HTTP response that isn't a transport error — including
/// 404 (no stamp yet) and 401/403 (auth, but the host is up).
fn reachable(cfg: &SyncCfg) -> bool {
    let quick = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(4))
        .build()
        .unwrap_or_default();
    auth(quick.get(file_url(cfg, REMOTE_STAMP)), cfg).send().is_ok()
}

/// Read sync config: None when disabled, no URL set, or (in auto mode) no endpoint
/// is currently reachable. The endpoints are tried in order — local → Tailscale →
/// public — and the first reachable one is used, so the same device works on LAN,
/// over Tailscale, or from anywhere without reconfiguring.
pub fn read_cfg(c: &Connection) -> Option<SyncCfg> {
    read_cfg_inner(c, true)
}

/// Manual "Sync now" config: same as `read_cfg` but skips the enable gate — an explicit
/// sync should run whenever a target is configured, even if background auto-sync is off.
pub fn read_cfg_manual(c: &Connection) -> Option<SyncCfg> {
    read_cfg_inner(c, false)
}

fn read_cfg_inner(c: &Connection, require_enabled: bool) -> Option<SyncCfg> {
    if require_enabled
        && repo::get_setting(c, K_ENABLED).ok().flatten().as_deref() != Some("true")
    {
        return None;
    }
    let user = repo::get_setting(c, K_USER).ok().flatten().unwrap_or_default();
    let pass = repo::get_setting(c, K_PASS).ok().flatten().unwrap_or_default();
    let mode = repo::get_setting(c, K_MODE).ok().flatten().unwrap_or_else(|| "auto".into());

    let local = endpoint(c, K_URL);
    let ts = endpoint(c, K_URL_TS);
    let public = endpoint(c, K_URL_PUB);
    let mut candidates: Vec<String> = match mode.as_str() {
        "local" => vec![local],
        "tailscale" => vec![ts],
        "public" => vec![public],
        _ => vec![local, ts, public], // auto: ordered fallback
    }
    .into_iter()
    .flatten()
    .collect();
    // On mobile the per-tier sync URL fields are hidden, so sync ALWAYS follows the
    // single Homelab URL (→ its /sync WebDAV, with the homelab's own local→Tailscale→
    // public reachability pick). This avoids a stale explicit sync_url pointing at the
    // proxy root without /sync, or at a Tailscale host the phone can't reach.
    #[cfg(mobile)]
    {
        candidates = homelab::resolved_setting(c, "sync_url")
            .map(|u| vec![u.trim_end_matches('/').to_string()])
            .unwrap_or_default();
    }
    // No explicit sync URL set? Derive it from the unified homelab base (base + /sync),
    // so configuring the single Homelab URL is all that's needed — resolved_setting
    // already does the local→Tailscale→public reachability pick.
    if candidates.is_empty() {
        if let Some(u) = homelab::resolved_setting(c, "sync_url") {
            candidates.push(u.trim_end_matches('/').to_string());
        }
    }
    if candidates.is_empty() {
        return None;
    }
    // A single configured endpoint: use it directly (no probe — let the real
    // request surface any error). Multiple: probe and pick the first reachable.
    if candidates.len() == 1 {
        return Some(SyncCfg { url: candidates.into_iter().next().unwrap(), user, pass });
    }
    for url in &candidates {
        let cfg = SyncCfg { url: url.clone(), user: user.clone(), pass: pass.clone() };
        if reachable(&cfg) {
            return Some(cfg);
        }
    }
    None // configured but nothing reachable right now — skip silently (background)
}

fn client() -> reqwest::blocking::Client {
    reqwest::blocking::Client::builder()
        // Bounded so a non-responsive or non-WebDAV target fails with a clear error
        // instead of leaving the UI on "Syncing…" for minutes. LAN/Tailscale targets
        // answer in well under this, and a multi-MB DB/file push still fits comfortably.
        .connect_timeout(std::time::Duration::from_secs(8))
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .unwrap_or_default()
}

fn auth(
    rb: reqwest::blocking::RequestBuilder,
    cfg: &SyncCfg,
) -> reqwest::blocking::RequestBuilder {
    if cfg.user.is_empty() {
        rb
    } else {
        rb.basic_auth(&cfg.user, Some(&cfg.pass))
    }
}

fn file_url(cfg: &SyncCfg, name: &str) -> String {
    format!("{}/{}", cfg.url, name)
}

/// Remote logical stamp (epoch-ms of the last push), or None if nothing's there.
fn remote_stamp(cfg: &SyncCfg) -> Result<Option<i64>> {
    let resp = auth(client().get(file_url(cfg, REMOTE_STAMP)), cfg)
        .send()
        .map_err(|e| Error::Other(format!("sync: {e}")))?;
    if resp.status().as_u16() == 404 {
        return Ok(None);
    }
    if !resp.status().is_success() {
        return Err(Error::Other(format!("sync: stamp HTTP {}", resp.status())));
    }
    let body = resp.text().unwrap_or_default();
    Ok(body.trim().parse::<i64>().ok())
}

/// PUT bytes to a remote file (WebDAV/HTTP). Treats any 2xx as success.
fn put(cfg: &SyncCfg, name: &str, body: Vec<u8>) -> Result<()> {
    let resp = auth(client().put(file_url(cfg, name)).body(body), cfg)
        .send()
        .map_err(|e| Error::Other(format!("sync: upload {name}: {e}")))?;
    if !resp.status().is_success() {
        return Err(Error::Other(format!(
            "sync: upload {name} HTTP {}",
            resp.status()
        )));
    }
    Ok(())
}

fn get_bytes(cfg: &SyncCfg, name: &str) -> Result<Vec<u8>> {
    let resp = auth(client().get(file_url(cfg, name)), cfg)
        .send()
        .map_err(|e| Error::Other(format!("sync: download {name}: {e}")))?;
    if !resp.status().is_success() {
        return Err(Error::Other(format!(
            "sync: download {name} HTTP {}",
            resp.status()
        )));
    }
    resp.bytes()
        .map(|b| b.to_vec())
        .map_err(|e| Error::Other(format!("sync: read {name}: {e}")))
}

// ---- pull + merge (background, never blocks startup) ------------------------

/// Pull the remote snapshot and MERGE it row-by-row into the LIVE local DB
/// (union by id, newest `updated_at` wins, tombstones applied). Local-only rows
/// survive — nothing is dropped. Runs async off the UI thread; the frontend
/// calls it on launch *after* the window is shown, so startup never waits on
/// homelab network I/O. Returns true if a newer remote was merged in.
#[tauri::command]
pub async fn sync_pull(app: AppHandle) -> Result<bool> {
    tauri::async_runtime::spawn_blocking(move || -> Result<bool> {
        let state = app.state::<AppState>();
        let (cfg, local_at) = {
            let c = state.db.lock().unwrap();
            let Some(cfg) = read_cfg(&c) else {
                return Ok(false); // sync disabled / unconfigured
            };
            let local_at = repo::get_setting(&c, K_LAST_AT)
                .ok()
                .flatten()
                .and_then(|s| s.parse::<i64>().ok())
                .unwrap_or(0);
            (cfg, local_at)
        };
        let remote = match remote_stamp(&cfg)? {
            Some(s) => s,
            None => return Ok(false), // nothing on the remote yet
        };
        if remote <= local_at {
            return Ok(false); // remote hasn't advanced since our last merge
        }
        let bytes = get_bytes(&cfg, REMOTE_DB)?;
        let tmp = std::env::temp_dir().join(format!("cortex-pull-{remote}.db"));
        std::fs::write(&tmp, &bytes).map_err(Error::Io)?;
        {
            let c = state.db.lock().unwrap();
            merge_attached(&c, &tmp)?;
            // We now hold the remote version; the launch push uploads the union
            // back with a fresh, higher stamp.
            repo::set_setting(&c, K_LAST_AT, &remote.to_string())?;
        }
        let _ = std::fs::remove_file(&tmp);
        Ok(true)
    })
    .await
    .map_err(|e| Error::Other(format!("sync pull task failed: {e}")))?
}

/// Whether a `settings` key is a device-independent PREFERENCE that's safe to
/// sync across devices. The `settings` table is otherwise excluded from sync
/// because it also holds credentials + device-specific endpoints/state. This is
/// an allowlist with a hard credential guard — API keys, tokens and URLs never
/// sync, even if a future key sneaks into an allowlisted group.
fn is_syncable_setting(key: &str) -> bool {
    // Opt-in exception (user-requested): the Moodle connection IS synced so a linked
    // phone is authed and shares course matching without re-logging in. Subject↔course
    // links + the courses themselves already sync (they're DB rows); this adds the
    // token/site so the phone can fetch fresh data. It rides the user's own homelab
    // WebDAV, so the token never leaves their control.
    //
    // Provider API keys are deliberately NOT synced (security): a billable OpenRouter/
    // Claude/OpenAI/Gemini key — and the custom endpoint — is a real secret, and syncing
    // it would copy it to every linked device + the WebDAV store. Each device holds its
    // own; the keys tab promises "never synced". The `_key` substring guard below also
    // backstops this if a future key sneaks into a group.
    const SYNCED_CREDS: &[&str] = &["moodle_url", "moodle_token", "moodle_userid"];
    if SYNCED_CREDS.contains(&key) {
        return true;
    }
    // Hard exclusions first (credentials, device endpoints, device-local state).
    const BLOCK_SUBSTR: &[&str] = &["_key", "token", "secret", "password", "_url"];
    if BLOCK_SUBSTR.iter().any(|b| key.contains(b)) {
        return false;
    }
    const BLOCK_PREFIX: &[&str] = &[
        "sync", "moodle", "google", "last_", "homelab", "tailscale", "whisper",
        "searxng", "ollama", "offline",
    ];
    if BLOCK_PREFIX.iter().any(|p| key.starts_with(p)) {
        return false;
    }
    // Allowlisted preference groups: keybinds, model/budget choices, pomodoro,
    // profile fields.
    const ALLOW_PREFIX: &[&str] = &["keybind_", "model_", "budget_", "pomo_", "profile_"];
    if ALLOW_PREFIX.iter().any(|p| key.starts_with(p)) {
        return true;
    }
    // Allowlisted standalone preferences.
    const ALLOW_EXACT: &[&str] = &[
        "theme", "follow_omarchy", "reading_font", "density", "default_station",
        "autoplay", "web_images_enabled", "exp_moodle", "cs_memory", "station_favs",
        "host_voices",
    ];
    ALLOW_EXACT.contains(&key)
}

/// Names of user tables to merge (everything except internal/never-merged ones).
fn user_tables(conn: &Connection, schema: &str) -> Result<Vec<String>> {
    let mut st = conn.prepare(&format!(
        "SELECT name FROM {schema}.sqlite_master WHERE type='table' \
         AND name NOT LIKE 'sqlite_%' AND name NOT IN ('settings','tombstones')"
    ))?;
    let rows = st.query_map([], |r| r.get::<_, String>(0))?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

/// Column names of a table in the given schema (`main` / `rmt`), declared order.
fn columns_of(conn: &Connection, schema: &str, table: &str) -> Result<Vec<String>> {
    let mut st = conn.prepare(&format!("PRAGMA {schema}.table_info(\"{table}\")"))?;
    let rows = st.query_map([], |r| r.get::<_, String>(1))?; // col 1 = name
    Ok(rows.filter_map(|r| r.ok()).collect())
}

/// Local columns of a table.
fn columns(conn: &Connection, table: &str) -> Result<Vec<String>> {
    columns_of(conn, "main", table)
}

/// Does `schema` contain a table named `name`?
fn has_table(conn: &Connection, schema: &str, name: &str) -> bool {
    conn.query_row(
        &format!("SELECT 1 FROM {schema}.sqlite_master WHERE type='table' AND name=?1"),
        [name],
        |_| Ok(()),
    )
    .is_ok()
}

/// Smart merge of `remote` into `local` (both standalone SQLite files):
///   • tables with `id` + `updated_at`  → upsert; remote overwrites local only
///     when strictly newer (in-place UPDATE, never a delete-then-insert that
///     could cascade away children).
///   • other tables                     → additive INSERT OR IGNORE (remote-only
///     rows added, existing rows untouched).
///   • tombstones unioned, then applied (with FK ON so deletes cascade) — a
///     delete removes a row only when the row wasn't edited after the delete.
/// `settings` is never merged (each device keeps its own creds/sync state).
#[cfg(test)]
fn merge_db(local: &Path, remote: &Path) -> Result<()> {
    let conn = Connection::open(local)?;
    merge_attached(&conn, remote)
}

/// The merge itself, run against an already-open connection (the launch path
/// uses the *live* DB connection so the merge happens in the background after
/// the window is shown — never blocking startup on network I/O).
fn merge_attached(conn: &Connection, remote: &Path) -> Result<()> {
    // Bulk inserts may transiently violate FKs (a child arriving before its
    // parent); we re-enable FK only for the tombstone-apply pass.
    conn.pragma_update(None, "foreign_keys", "OFF")?;
    conn.execute(
        "ATTACH DATABASE ?1 AS rmt",
        [remote.to_string_lossy().as_ref()],
    )?;

    let local_tables = user_tables(conn, "main")?;
    let remote_tables: HashSet<String> =
        user_tables(conn,"rmt")?.into_iter().collect();

    for t in &local_tables {
        if !remote_tables.contains(t) {
            continue; // table missing on remote (older app version) — skip
        }
        // Use only columns present in BOTH schemas, so an older remote (missing
        // a column a later migration added) doesn't blow up the SELECT.
        let remote_cols: HashSet<String> =
            columns_of(conn, "rmt", t)?.into_iter().collect();
        let cols: Vec<String> = columns(conn, t)?
            .into_iter()
            .filter(|c| remote_cols.contains(c))
            .collect();
        if cols.is_empty() {
            continue;
        }
        let has_id = cols.iter().any(|c| c == "id");
        let has_upd = cols.iter().any(|c| c == "updated_at");
        let collist = cols
            .iter()
            .map(|c| format!("\"{c}\""))
            .collect::<Vec<_>>()
            .join(",");
        let sql = if has_id && has_upd {
            let set = cols
                .iter()
                .filter(|c| *c != "id")
                .map(|c| format!("\"{c}\"=excluded.\"{c}\""))
                .collect::<Vec<_>>()
                .join(",");
            format!(
                "INSERT INTO main.\"{t}\" ({collist}) SELECT {collist} FROM rmt.\"{t}\" WHERE true \
                 ON CONFLICT(id) DO UPDATE SET {set} WHERE excluded.updated_at > \"{t}\".updated_at"
            )
        } else {
            format!(
                "INSERT OR IGNORE INTO main.\"{t}\" ({collist}) SELECT {collist} FROM rmt.\"{t}\""
            )
        };
        conn.execute(&sql, [])?;
    }

    // Union tombstones from both sides — only if the remote has the table. A DB
    // written by the OLD sync (schema < 0019) has no `tombstones`; selecting
    // from it would error. Local tombstones still apply below regardless.
    if has_table(conn, "rmt", "tombstones") {
        conn.execute(
            "INSERT OR REPLACE INTO main.tombstones \
             SELECT entity_table, entity_id, MAX(deleted_at) FROM ( \
               SELECT * FROM main.tombstones UNION ALL SELECT * FROM rmt.tombstones \
             ) GROUP BY entity_table, entity_id",
            [],
        )?;
    }

    // Apply tombstones with FK ON so a parent delete cascades to its children.
    conn.pragma_update(None, "foreign_keys", "ON")?;
    for t in &local_tables {
        let cols = columns(conn,t)?;
        if cols.iter().any(|c| c == "id") && cols.iter().any(|c| c == "updated_at") {
            let sql = format!(
                "DELETE FROM main.\"{t}\" WHERE id IN \
                   (SELECT entity_id FROM main.tombstones WHERE entity_table='{t}') \
                 AND updated_at <= (SELECT deleted_at FROM main.tombstones \
                   WHERE entity_table='{t}' AND entity_id=main.\"{t}\".id)"
            );
            conn.execute(&sql, [])?;
        }
    }

    // Selectively merge PREFERENCE settings (theme, keybinds, model choices,
    // profile, pomodoro, …). The `settings` table is excluded from the table loop
    // above because it also holds creds/endpoints; here we copy only allowlisted,
    // device-independent keys. No per-key timestamp exists, so this is
    // last-push-wins — fine for preferences.
    if has_table(conn, "rmt", "settings") {
        let pairs: Vec<(String, String)> = {
            let mut st = conn.prepare("SELECT key, value FROM rmt.settings")?;
            let rows = st.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
            rows.filter_map(|x| x.ok()).collect()
        };
        for (k, v) in pairs {
            if is_syncable_setting(&k) {
                conn.execute(
                    "INSERT INTO main.settings (key, value) VALUES (?1, ?2) \
                     ON CONFLICT(key) DO UPDATE SET value=excluded.value",
                    rusqlite::params![k, v],
                )?;
            }
        }
    }

    conn.execute("DETACH DATABASE rmt", [])?;
    // Fold the WAL back into the main file so the merged result is self-contained.
    let _: std::result::Result<String, _> =
        conn.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |r| r.get(0));
    Ok(())
}

// ---- commands ---------------------------------------------------------------

#[derive(serde::Serialize)]
pub struct SyncStatus {
    pub enabled: bool,
    pub configured: bool,
    pub last_at: i64, // 0 = never
}

#[tauri::command]
pub fn sync_status(state: tauri::State<AppState>) -> Result<SyncStatus> {
    let c = state.db.lock().unwrap();
    let enabled = repo::get_setting(&c, K_ENABLED)?.as_deref() == Some("true");
    // "Configured" means a sync target is SET — an explicit sync_url OR the unified
    // homelab_base. This is a pure settings read: a status check must NOT probe the
    // network (resolved_setting → resolve() does blocking reachability probes, which on
    // a phone stall on the unreachable LAN URL and wrongly flip the pill to "off").
    let configured = repo::get_setting(&c, K_URL)?
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false)
        || repo::get_setting(&c, "homelab_base")?
            .map(|s| !s.trim().is_empty())
            .unwrap_or(false);
    let last_at = repo::get_setting(&c, K_LAST_AT)?
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(0);
    Ok(SyncStatus {
        enabled,
        configured,
        last_at,
    })
}

/// Reachability + auth check for the Settings "Test" button.
#[tauri::command]
pub async fn sync_test(url: String, user: String, pass: String) -> Result<bool> {
    tauri::async_runtime::spawn_blocking(move || -> Result<bool> {
        let cfg = SyncCfg {
            url: url.trim().trim_end_matches('/').to_string(),
            user,
            pass,
        };
        // A reachable, auth-OK endpoint returns 2xx or 404 for the stamp file;
        // 401/403 means auth failed, anything else means it's not reachable.
        match auth(client().get(file_url(&cfg, REMOTE_STAMP)), &cfg).send() {
            Ok(r) => Ok(r.status().is_success() || r.status().as_u16() == 404),
            Err(_) => Ok(false),
        }
    })
    .await
    .map_err(|e| Error::Other(format!("sync test task failed: {e}")))?
}

/// Push a consistent snapshot of the local DB to the homelab (the "auto store"
/// half of live sync — called debounced from the frontend after changes).
#[tauri::command]
pub async fn sync_push(app: AppHandle) -> Result<i64> {
    tauri::async_runtime::spawn_blocking(move || -> Result<i64> {
        let state = app.state::<AppState>();
        let db_path = app
            .path()
            .app_data_dir()
            .map_err(|e| Error::Other(e.to_string()))?
            .join("cortex.db");
        let cfg = {
            let c = state.db.lock().unwrap();
            read_cfg_manual(&c).ok_or_else(|| {
                Error::Other(
                    "Sync target not set — add a Homelab URL (or sync URL) in Settings → Integrations.".into(),
                )
            })?
        };
        // Checkpoint the WAL into the main file, then copy a clean snapshot.
        let ts = now_ms();
        let tmp = std::env::temp_dir().join(format!("cortex-sync-{ts}.db"));
        {
            let c = state.db.lock().unwrap();
            let _: std::result::Result<String, _> =
                c.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |r| r.get(0));
            std::fs::copy(&db_path, &tmp).map_err(Error::Io)?;
        }
        let bytes = std::fs::read(&tmp).map_err(Error::Io)?;
        let _ = std::fs::remove_file(&tmp);
        put(&cfg, REMOTE_DB, bytes)?;
        put(&cfg, REMOTE_STAMP, ts.to_string().into_bytes())?;
        {
            let c = state.db.lock().unwrap();
            repo::set_setting(&c, K_LAST_AT, &ts.to_string())?;
        }
        // Sync the binary vault (originals + recordings), both directions, no
        // deletes — then re-point source paths to the local copies. Best-effort:
        // a file/WebDAV hiccup must not fail the DB push that already succeeded.
        if let Some(data_dir) = db_path.parent() {
            sync_files(&cfg, data_dir, "sources");
            sync_files(&cfg, data_dir, "recordings");
            let c = state.db.lock().unwrap();
            let _ = repoint_source_files(&c, &data_dir.join("sources"));
        }
        Ok(ts)
    })
    .await
    .map_err(|e| Error::Other(format!("sync push task failed: {e}")))?
}

// ---- binary file sync (WebDAV) ---------------------------------------------

fn dav_request(
    cfg: &SyncCfg,
    method: &[u8],
    path: &str,
) -> reqwest::blocking::RequestBuilder {
    let m = reqwest::Method::from_bytes(method).unwrap_or(reqwest::Method::GET);
    auth(client().request(m, file_url(cfg, path)), cfg)
}

/// Create a remote collection (directory). Ignores already-exists / 405.
fn mkcol(cfg: &SyncCfg, path: &str) {
    let _ = dav_request(cfg, b"MKCOL", path).send();
}

/// File names directly under a remote collection, via PROPFIND Depth:1.
fn propfind_names(cfg: &SyncCfg, dir: &str) -> HashSet<String> {
    let mut out = HashSet::new();
    let body = "<?xml version=\"1.0\"?><d:propfind xmlns:d=\"DAV:\">\
                <d:prop><d:resourcetype/></d:prop></d:propfind>";
    let resp = dav_request(cfg, b"PROPFIND", dir)
        .header("Depth", "1")
        .header("Content-Type", "application/xml")
        .body(body)
        .send();
    let Ok(resp) = resp else {
        return out;
    };
    if !resp.status().is_success() && resp.status().as_u16() != 207 {
        return out; // dir missing or PROPFIND unsupported — treat as empty
    }
    let text = resp.text().unwrap_or_default();
    for href in extract_hrefs(&text) {
        let trimmed = href.trim_end_matches('/');
        if let Some(name) = trimmed.rsplit('/').next() {
            // Skip the collection's own entry; real files always carry an ext.
            if !name.is_empty() && name.contains('.') {
                out.insert(name.to_string());
            }
        }
    }
    out
}

/// Pull the text inside every `<…href>…</…href>` (namespace-prefix agnostic).
fn extract_hrefs(xml: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = xml;
    while let Some(i) = rest.find("href") {
        rest = &rest[i + 4..];
        let Some(gt) = rest.find('>') else { break };
        let after = &rest[gt + 1..];
        let Some(lt) = after.find('<') else { break };
        let val = after[..lt].trim();
        if !val.is_empty() {
            out.push(val.to_string());
        }
        rest = &after[lt..];
    }
    out
}

/// Sync one local subdir of binary originals with `files/{sub}` on the remote,
/// both directions, never deleting. Filenames are content-addressed
/// (`{id}.{ext}`), so presence is identity — no byte comparison needed.
fn sync_files(cfg: &SyncCfg, data_dir: &Path, sub: &str) {
    let local_dir = data_dir.join(sub);
    mkcol(cfg, "files");
    mkcol(cfg, &format!("files/{sub}"));
    let remote = propfind_names(cfg, &format!("files/{sub}"));

    // Upload local files the remote lacks.
    let mut local: HashSet<String> = HashSet::new();
    if let Ok(rd) = std::fs::read_dir(&local_dir) {
        for entry in rd.flatten() {
            if !entry.path().is_file() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            local.insert(name.clone());
            if !remote.contains(&name) {
                if let Ok(bytes) = std::fs::read(entry.path()) {
                    let _ = put(cfg, &format!("files/{sub}/{name}"), bytes);
                }
            }
        }
    }

    // Download remote files we don't have locally.
    let _ = std::fs::create_dir_all(&local_dir);
    for name in &remote {
        if local.contains(name) {
            continue;
        }
        if let Ok(bytes) = get_bytes(cfg, &format!("files/{sub}/{name}")) {
            let _ = std::fs::write(local_dir.join(name), bytes);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{now_ms, AppState};
    use std::fs;

    #[test]
    fn syncable_settings_never_include_credentials() {
        // Provider API keys & device endpoints must NEVER sync across devices.
        for k in [
            "gemini_api_key", "openrouter_api_key", "openai_api_key", "claude_api_key",
            "custom_endpoint", "ollama_url", "searxng_url",
            "whisper_url", "sync_url", "sync_enabled", "google_calendar_token",
            "last_subject_id", "offline_mode",
        ] {
            assert!(!is_syncable_setting(k), "{k} must not sync");
        }
        // The Moodle connection is the one deliberate credential exception (opt-in, rides
        // the user's own homelab WebDAV) so a linked phone shares course matching.
        for k in ["moodle_url", "moodle_token", "moodle_userid"] {
            assert!(is_syncable_setting(k), "{k} should sync (Moodle opt-in)");
        }
        // Preferences SHOULD sync.
        for k in [
            "theme", "density", "reading_font", "keybind_cmdk", "keybind_preset",
            "model_chat", "budget_cheatsheet", "pomo_workMin", "profile_name",
            "default_station", "web_images_enabled", "cs_memory",
        ] {
            assert!(is_syncable_setting(k), "{k} should sync");
        }
    }

    fn ins(c: &Connection, id: &str, name: &str, upd: i64) {
        c.execute(
            "INSERT INTO subjects (id,name,created_at,updated_at) VALUES (?1,?2,?3,?4)",
            rusqlite::params![id, name, upd, upd],
        )
        .unwrap();
    }

    #[test]
    fn merge_unions_keeps_local_and_applies_tombstones() {
        let dir = std::env::temp_dir().join(format!("cortex-merge-{}", now_ms()));
        fs::create_dir_all(&dir).unwrap();
        let lp = dir.join("local.db");
        let rp = dir.join("remote.db");

        {
            let local = AppState::new(&lp).unwrap();
            let remote = AppState::new(&rp).unwrap();
            let lc = local.db.lock().unwrap();
            let rc = remote.db.lock().unwrap();

            ins(&lc, "shared", "old-name", 100); // remote newer → should win
            ins(&rc, "shared", "new-name", 200);
            ins(&lc, "local-only", "keep-me", 100); // must survive the merge
            ins(&rc, "remote-only", "bring-me", 100); // must arrive
            ins(&lc, "doomed", "x", 100); // deleted on remote → tombstone wins
            ins(&rc, "doomed", "x", 100);
            rc.execute("DELETE FROM subjects WHERE id='doomed'", []).unwrap();

            for c in [&lc, &rc] {
                let _: std::result::Result<String, _> =
                    c.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |r| r.get(0));
            }
        } // connections close here

        merge_db(&lp, &rp).unwrap();

        let c = Connection::open(&lp).unwrap();
        let name: String = c
            .query_row("SELECT name FROM subjects WHERE id='shared'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(name, "new-name", "newer updated_at must win (ISC-11)");
        let count = |id: &str| -> i64 {
            c.query_row(
                "SELECT count(*) FROM subjects WHERE id=?1",
                [id],
                |r| r.get(0),
            )
            .unwrap()
        };
        assert_eq!(count("local-only"), 1, "local-only row must survive (ISC-12)");
        assert_eq!(count("remote-only"), 1, "remote-only row must arrive (ISC-10)");
        assert_eq!(count("doomed"), 0, "tombstoned row must be deleted");

        drop(c);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn merge_tolerates_old_remote_without_tombstones() {
        // Mimics the homelab DB pushed by the PREVIOUS sync system: schema < 0019,
        // so it has no `tombstones` table. The merge must not error on it.
        let dir = std::env::temp_dir().join(format!("cortex-oldremote-{}", now_ms()));
        fs::create_dir_all(&dir).unwrap();
        let lp = dir.join("local.db");
        let rp = dir.join("remote.db");

        {
            let local = AppState::new(&lp).unwrap();
            let remote = AppState::new(&rp).unwrap();
            let lc = local.db.lock().unwrap();
            let rc = remote.db.lock().unwrap();
            ins(&rc, "from-laptop", "all-my-data", 100); // remote-only row to pull in
            // Strip the new bits so `remote` looks like an old-schema DB.
            rc.execute("DROP TRIGGER IF EXISTS tomb_subjects", []).unwrap();
            rc.execute("DROP TABLE IF EXISTS tombstones", []).unwrap();
            for c in [&lc, &rc] {
                let _: std::result::Result<String, _> =
                    c.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |r| r.get(0));
            }
        }

        merge_db(&lp, &rp).expect("merging an old-schema remote must not error");

        let c = Connection::open(&lp).unwrap();
        let n: i64 = c
            .query_row(
                "SELECT count(*) FROM subjects WHERE id='from-laptop'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, 1, "remote-only row must arrive even from an old-schema DB");
        drop(c);
        let _ = fs::remove_dir_all(&dir);
    }
}

/// Re-point each source's absolute, machine-specific `stored_path` to the local
/// copy of its file (named `{id}.{ext}`). Deliberately does NOT touch
/// `updated_at` — a path is local state, not a user edit; bumping it would make
/// the row look "newer" and ping-pong the path between devices forever.
fn repoint_source_files(conn: &Connection, sources_dir: &Path) -> Result<()> {
    let Ok(rd) = std::fs::read_dir(sources_dir) else {
        return Ok(());
    };
    for entry in rd.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let fname = entry.file_name().to_string_lossy().to_string();
        let Some((id, _ext)) = fname.rsplit_once('.') else {
            continue;
        };
        if let Some(p) = path.to_str() {
            let _ = conn.execute(
                "UPDATE sources SET stored_path=?2 WHERE id=?1 AND stored_path IS NOT ?2",
                rusqlite::params![id, p],
            );
        }
    }
    Ok(())
}
