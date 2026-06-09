//! Live homelab sync (last-write-wins, whole-database).
//!
//! The local SQLite DB is the unit of sync. We PUT a consistent snapshot to a
//! WebDAV endpoint after changes (debounced by the frontend) and, on launch,
//! pull a newer remote copy before opening the DB. "Newer" is decided by a
//! logical stamp file (`cortex.stamp`, epoch-ms of the last push) rather than
//! HTTP dates, so it's immune to clock/header quirks. This is single-user
//! multi-device: concurrent edits resolve last-write-wins, not per-record.

use crate::db::AppState;
use crate::error::{Error, Result};
use crate::repo;
use rusqlite::Connection;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Manager};

const K_ENABLED: &str = "sync_enabled";
const K_URL: &str = "sync_url";
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

/// Read sync config from settings; None when sync is disabled or no URL is set.
pub fn read_cfg(c: &Connection) -> Option<SyncCfg> {
    if repo::get_setting(c, K_ENABLED).ok().flatten().as_deref() != Some("true") {
        return None;
    }
    let url = repo::get_setting(c, K_URL)
        .ok()
        .flatten()?
        .trim()
        .trim_end_matches('/')
        .to_string();
    if url.is_empty() {
        return None;
    }
    Some(SyncCfg {
        url,
        user: repo::get_setting(c, K_USER).ok().flatten().unwrap_or_default(),
        pass: repo::get_setting(c, K_PASS).ok().flatten().unwrap_or_default(),
    })
}

fn client() -> reqwest::blocking::Client {
    reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
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

// ---- launch pull ------------------------------------------------------------

/// Before the app opens its DB connection: if the remote holds a newer snapshot
/// than the local copy, replace the local DB with it. Best-effort and silent —
/// any failure just leaves the local DB untouched so the app still starts.
pub fn pull_on_launch(db_path: &Path) {
    if !db_path.exists() {
        return; // fresh install — nothing to pull into yet
    }
    let (cfg, local_at) = {
        let Ok(conn) = Connection::open(db_path) else {
            return;
        };
        let Some(cfg) = read_cfg(&conn) else {
            return;
        };
        let local_at = repo::get_setting(&conn, K_LAST_AT)
            .ok()
            .flatten()
            .and_then(|s| s.parse::<i64>().ok())
            .unwrap_or(0);
        (cfg, local_at)
        // conn drops here, closing the handle before we replace the file
    };
    let remote = match remote_stamp(&cfg) {
        Ok(Some(s)) => s,
        _ => return,
    };
    if remote <= local_at {
        return; // local is current or newer — push will handle the reverse
    }
    let Ok(bytes) = get_bytes(&cfg, REMOTE_DB) else {
        return;
    };
    let tmp = db_path.with_extension("db.synctmp");
    if std::fs::write(&tmp, &bytes).is_err() {
        return;
    }
    if std::fs::rename(&tmp, db_path).is_err() {
        let _ = std::fs::remove_file(&tmp);
        return;
    }
    // The fresh snapshot is self-contained; stale WAL/SHM would corrupt it.
    let _ = std::fs::remove_file(db_path.with_extension("db-wal"));
    let _ = std::fs::remove_file(db_path.with_extension("db-shm"));
    // Record that we now hold the remote version.
    if let Ok(c2) = Connection::open(db_path) {
        let _ = repo::set_setting(&c2, K_LAST_AT, &remote.to_string());
    }
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
    let configured = repo::get_setting(&c, K_URL)?
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
            read_cfg(&c).ok_or_else(|| Error::Other("sync is not configured".into()))?
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
        Ok(ts)
    })
    .await
    .map_err(|e| Error::Other(format!("sync push task failed: {e}")))?
}
