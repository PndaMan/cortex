//! LIVE sync client — the instant, delta-based path (homelab `syncd` service).
//!
//! The WebDAV snapshot sync (sync.rs) ships the whole database on a timer; this
//! module makes edits show up across devices in about a second instead:
//!
//!   • every local DB write (captured by a SQLite update hook) marks the engine
//!     dirty; after a short debounce a DELTA — a tiny SQLite file holding only
//!     the rows changed since the last acknowledged push — goes to syncd, which
//!     assigns it a sequence number and pushes the seq to every connected peer
//!     over WebSocket;
//!   • peers fetch the delta and merge it with the SAME newest-wins + tombstone
//!     logic as snapshot sync (`sync::merge_attached`) — one merge engine, two
//!     transports;
//!   • the push watermark only advances on server ack, so an offline device
//!     simply accumulates and replays when it reconnects: the only failure mode
//!     is "unreachable", and it self-heals;
//!   • credential values are sealed client-side before upload (same
//!     `seal_snapshot_credentials`), and auth is the sync user/password;
//!   • periodically a full sealed snapshot compacts the server's log and serves
//!     as the bootstrap for brand-new devices.
//!
//! WebDAV remains the transport for the binary vault (source files/recordings)
//! and the snapshot fallback when syncd isn't deployed — background_tick keeps
//! doing both; this engine simply front-runs it for DB rows.

use crate::db::AppState;
use crate::error::{Error, Result};
use crate::repo;
use rusqlite::Connection;
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::{Condvar, Mutex};
use tauri::{AppHandle, Emitter, Manager};

const K_APPLIED: &str = "livesync_applied_seq";
const K_PUSHED_AT: &str = "livesync_pushed_at";
/// Compact (upload a full snapshot) when the server log grows past this many
/// deltas beyond its snapshot.
const COMPACT_EVERY: i64 = 200;

/// True while the WebSocket to syncd is up — surfaced as "Live" in Settings.
pub static CONNECTED: AtomicBool = AtomicBool::new(false);
/// Suppresses the update hook while WE write (merging remote deltas, advancing
/// watermarks) so applying a peer's change never triggers an echo push.
static SELF_WRITE: AtomicBool = AtomicBool::new(false);
/// Wakes the push worker; the i64 is a change generation for debouncing.
static DIRTY: Mutex<i64> = Mutex::new(0);
static WAKE: Condvar = Condvar::new();
static GEN: AtomicI64 = AtomicI64::new(0);

/// Called from the SQLite update hook on every row change.
pub fn mark_dirty(table: &str) {
    // Settings churn constantly (watermarks, UI state) and sync separately;
    // chunks always ride their source's update. Skip both to cut hook noise.
    if table == "settings" || SELF_WRITE.load(Ordering::Relaxed) {
        return;
    }
    let g = GEN.fetch_add(1, Ordering::SeqCst) + 1;
    *DIRTY.lock().unwrap() = g;
    WAKE.notify_all();
}

struct LiveCfg {
    base: String, // e.g. http://host:8080/syncd (resolved local→ts→public)
    user: String,
    pass: String,
}

fn read_live_cfg(c: &Connection) -> Option<LiveCfg> {
    let enabled = repo::get_setting(c, "sync_enabled").ok().flatten().as_deref() == Some("true");
    if !enabled {
        return None;
    }
    let base = crate::homelab::resolved_setting(c, "syncd_url")?;
    Some(LiveCfg {
        base: base.trim_end_matches('/').to_string(),
        user: repo::get_setting(c, "sync_user").ok().flatten().unwrap_or_default(),
        pass: repo::get_setting(c, "sync_pass").ok().flatten().unwrap_or_default(),
    })
}

fn http() -> reqwest::blocking::Client {
    reqwest::blocking::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(5))
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .unwrap_or_default()
}

fn get_json(cfg: &LiveCfg, path: &str) -> Result<serde_json::Value> {
    let r = http()
        .get(format!("{}{path}", cfg.base))
        .basic_auth(&cfg.user, Some(&cfg.pass))
        .send()
        .map_err(|e| Error::Other(format!("syncd: {e}")))?;
    if !r.status().is_success() {
        return Err(Error::Other(format!("syncd HTTP {}", r.status())));
    }
    r.json().map_err(|e| Error::Other(format!("syncd json: {e}")))
}

fn setting_i64(c: &Connection, key: &str) -> i64 {
    repo::get_setting(c, key).ok().flatten().and_then(|s| s.parse().ok()).unwrap_or(0)
}

fn set_setting_i64(c: &Connection, key: &str, v: i64) {
    SELF_WRITE.store(true, Ordering::SeqCst);
    let _ = repo::set_setting(c, key, &v.to_string());
    SELF_WRITE.store(false, Ordering::SeqCst);
}

// ─────────────────────────── delta building ───────────────────────────

/// Export every row changed since `since` into a fresh delta DB at `out`.
/// Returns the number of exported rows (0 ⇒ nothing to push).
fn build_delta(c: &Connection, out: &std::path::Path, since: i64) -> Result<i64> {
    let _ = std::fs::remove_file(out);
    c.execute("ATTACH DATABASE ?1 AS delta", [out.to_string_lossy().as_ref()])?;
    let mut exported: i64 = 0;
    let res = (|| -> Result<i64> {
        // Table inventory mirrors the merge: everything except internals.
        let tables: Vec<String> = {
            let mut st = c.prepare(
                "SELECT name FROM main.sqlite_master WHERE type='table' \
                 AND name NOT LIKE 'sqlite_%' AND name NOT IN ('settings','tombstones')",
            )?;
            let rows = st.query_map([], |r| r.get::<_, String>(0))?;
            rows.filter_map(|r| r.ok()).collect()
        };
        for t in &tables {
            let cols: Vec<String> = {
                let mut st = c.prepare(&format!("PRAGMA main.table_info(\"{t}\")"))?;
                let rows = st.query_map([], |r| r.get::<_, String>(1))?;
                rows.filter_map(|r| r.ok()).collect()
            };
            let has = |name: &str| cols.iter().any(|c| c == name);
            let filter = if t == "chunks" {
                // Chunks change together with their parent source (reingest
                // replaces them under the same source id) but carry only
                // created_at themselves.
                format!(
                    "created_at > {since} OR source_id IN \
                     (SELECT id FROM main.sources WHERE updated_at > {since})"
                )
            } else if has("updated_at") {
                format!("updated_at > {since}")
            } else if has("created_at") {
                // Append-only tables (chat history, …) merge additively.
                format!("created_at > {since}")
            } else {
                continue; // no timestamps — snapshot/compaction covers it
            };
            c.execute(
                &format!("CREATE TABLE delta.\"{t}\" AS SELECT * FROM main.\"{t}\" WHERE {filter}"),
                [],
            )?;
            exported += c.query_row(
                &format!("SELECT COUNT(*) FROM delta.\"{t}\""),
                [],
                |r| r.get::<_, i64>(0),
            )?;
        }
        c.execute(
            &format!(
                "CREATE TABLE delta.tombstones AS \
                 SELECT * FROM main.tombstones WHERE deleted_at > {since}"
            ),
            [],
        )?;
        exported += c.query_row("SELECT COUNT(*) FROM delta.tombstones", [], |r| r.get::<_, i64>(0))?;
        // Preference settings ride every delta (small; merge side allowlists +
        // unseals). Full copy — sealing happens on the file after DETACH.
        c.execute("CREATE TABLE delta.settings AS SELECT key, value FROM main.settings", [])?;
        Ok(exported)
    })();
    let _ = c.execute("DETACH DATABASE delta", []);
    res
}

/// Build + push one delta. Returns Ok(true) if something was pushed.
fn push_once(app: &AppHandle) -> Result<bool> {
    let state = app.state::<AppState>();
    let (cfg, since, pass) = {
        let c = state.db.lock().unwrap();
        let Some(cfg) = read_live_cfg(&c) else { return Ok(false) };
        let pass = repo::get_setting(&c, "sync_pass").ok().flatten().unwrap_or_default();
        (cfg, setting_i64(&c, K_PUSHED_AT), pass)
    };
    let build_start = crate::sync::now_ms();
    let tmp = std::env::temp_dir().join(format!("cortex-delta-{build_start}.db"));
    let exported = {
        let c = state.db.lock().unwrap();
        build_delta(&c, &tmp, since)?
    };
    if exported == 0 {
        let _ = std::fs::remove_file(&tmp);
        // Still advance the watermark: nothing changed in (since, build_start].
        let c = state.db.lock().unwrap();
        set_setting_i64(&c, K_PUSHED_AT, build_start);
        return Ok(false);
    }
    crate::sync::seal_snapshot_credentials(&tmp, &pass)?;
    let bytes = std::fs::read(&tmp).map_err(Error::Io)?;
    let _ = std::fs::remove_file(&tmp);
    let r = http()
        .post(format!("{}/deltas", cfg.base))
        .basic_auth(&cfg.user, Some(&cfg.pass))
        .body(bytes)
        .send()
        .map_err(|e| Error::Other(format!("syncd push: {e}")))?;
    if !r.status().is_success() {
        return Err(Error::Other(format!("syncd push HTTP {}", r.status())));
    }
    let seq = r
        .json::<serde_json::Value>()
        .ok()
        .and_then(|v| v["seq"].as_i64())
        .unwrap_or(0);
    {
        let c = state.db.lock().unwrap();
        set_setting_i64(&c, K_PUSHED_AT, build_start);
        // Our own delta needs no re-apply; only fast-forward when nothing else
        // landed in between (otherwise catch-up fetches the gap AND ours —
        // re-merging our own rows is a no-op).
        if seq == setting_i64(&c, K_APPLIED) + 1 {
            set_setting_i64(&c, K_APPLIED, seq);
        }
    }
    // Compaction: keep the server log short and give new devices a bootstrap.
    maybe_compact(app, &cfg, seq, &pass);
    Ok(true)
}

fn maybe_compact(app: &AppHandle, cfg: &LiveCfg, seq: i64, pass: &str) {
    let Ok(v) = get_json(cfg, "/seq") else { return };
    let snapshot_seq = v["snapshot_seq"].as_i64().unwrap_or(0);
    if seq - snapshot_seq < COMPACT_EVERY && snapshot_seq != 0 {
        return;
    }
    let state = app.state::<AppState>();
    let Ok(db_path) = app.path().app_data_dir().map(|d| d.join("cortex.db")) else { return };
    let tmp = std::env::temp_dir().join(format!("cortex-compact-{seq}.db"));
    {
        let c = state.db.lock().unwrap();
        let _: std::result::Result<String, _> =
            c.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |r| r.get(0));
        if std::fs::copy(&db_path, &tmp).is_err() {
            return;
        }
    }
    if crate::sync::seal_snapshot_credentials(&tmp, pass).is_err() {
        let _ = std::fs::remove_file(&tmp);
        return;
    }
    if let Ok(bytes) = std::fs::read(&tmp) {
        let _ = http()
            .put(format!("{}/snapshot?seq={seq}", cfg.base))
            .basic_auth(&cfg.user, Some(&cfg.pass))
            .body(bytes)
            .send();
    }
    let _ = std::fs::remove_file(&tmp);
}

// ─────────────────────────── applying deltas ───────────────────────────

/// Fetch and merge everything newer than our applied watermark. Emits
/// `sync:applied` (frontend refreshes) when anything landed.
fn catch_up(app: &AppHandle) -> Result<()> {
    let state = app.state::<AppState>();
    let Some(cfg) = ({ let c = state.db.lock().unwrap(); read_live_cfg(&c) }) else {
        return Ok(());
    };
    let v = get_json(&cfg, "/seq")?;
    let server_seq = v["seq"].as_i64().unwrap_or(0);
    let snapshot_seq = v["snapshot_seq"].as_i64().unwrap_or(0);
    let mut applied = { let c = state.db.lock().unwrap(); setting_i64(&c, K_APPLIED) };
    if applied > server_seq {
        applied = 0; // server log was reset — re-bootstrap (merging is idempotent)
    }
    if applied >= server_seq {
        return Ok(());
    }
    let mut merged_any = false;
    // Behind the snapshot (or brand new): bootstrap from it first.
    if applied < snapshot_seq {
        let r = http()
            .get(format!("{}/snapshot", cfg.base))
            .basic_auth(&cfg.user, Some(&cfg.pass))
            .send()
            .map_err(|e| Error::Other(format!("syncd snapshot: {e}")))?;
        if r.status().is_success() {
            let seq = r
                .headers()
                .get("x-snapshot-seq")
                .and_then(|h| h.to_str().ok())
                .and_then(|s| s.parse().ok())
                .unwrap_or(snapshot_seq);
            let bytes = r.bytes().map_err(|e| Error::Other(e.to_string()))?;
            apply_file(app, &bytes)?;
            let c = state.db.lock().unwrap();
            set_setting_i64(&c, K_APPLIED, seq);
            applied = seq;
            merged_any = true;
        }
    }
    let seqs = get_json(&cfg, &format!("/deltas?since={applied}"))?;
    for seq in seqs["seqs"].as_array().into_iter().flatten().filter_map(|s| s.as_i64()) {
        let r = http()
            .get(format!("{}/deltas/{seq}", cfg.base))
            .basic_auth(&cfg.user, Some(&cfg.pass))
            .send()
            .map_err(|e| Error::Other(format!("syncd delta {seq}: {e}")))?;
        if !r.status().is_success() {
            continue; // compacted away — the next snapshot bootstrap covers it
        }
        let bytes = r.bytes().map_err(|e| Error::Other(e.to_string()))?;
        apply_file(app, &bytes)?;
        let c = state.db.lock().unwrap();
        set_setting_i64(&c, K_APPLIED, seq);
        merged_any = true;
    }
    if merged_any {
        let _ = app.emit("sync:applied", serde_json::json!({ "seq": server_seq }));
    }
    Ok(())
}

/// Merge one downloaded delta/snapshot through the shared merge engine.
fn apply_file(app: &AppHandle, bytes: &[u8]) -> Result<()> {
    let state = app.state::<AppState>();
    let tmp = std::env::temp_dir().join(format!("cortex-apply-{}.db", crate::db::new_id()));
    std::fs::write(&tmp, bytes).map_err(Error::Io)?;
    let res = {
        let c = state.db.lock().unwrap();
        SELF_WRITE.store(true, Ordering::SeqCst);
        let r = crate::sync::merge_attached(&c, &tmp);
        SELF_WRITE.store(false, Ordering::SeqCst);
        r
    };
    let _ = std::fs::remove_file(&tmp);
    res
}

// ─────────────────────────── worker threads ───────────────────────────

/// Spawn the live-sync engine: a push worker (debounced deltas) and a WebSocket
/// listener (instant pull). Both are resilient loops that idle harmlessly when
/// live sync is unconfigured.
pub fn start(app: &AppHandle) {
    // Push worker.
    {
        let app = app.clone();
        std::thread::Builder::new()
            .name("livesync-push".into())
            .spawn(move || push_worker(app))
            .ok();
    }
    // WebSocket listener.
    {
        let app = app.clone();
        std::thread::Builder::new()
            .name("livesync-ws".into())
            .spawn(move || ws_worker(app))
            .ok();
    }
}

fn push_worker(app: AppHandle) {
    let mut backoff = 5u64;
    loop {
        // Wait for a change…
        {
            let mut g = DIRTY.lock().unwrap();
            while *g == 0 {
                g = WAKE.wait(g).unwrap();
            }
            *g = 0;
        }
        // …then debounce: keep waiting while more changes stream in (an
        // embedding run inserts hundreds of rows — ship one delta, not 300).
        loop {
            let before = GEN.load(Ordering::SeqCst);
            std::thread::sleep(std::time::Duration::from_millis(1200));
            if GEN.load(Ordering::SeqCst) == before {
                break;
            }
        }
        match push_once(&app) {
            Ok(_) => backoff = 5,
            Err(e) => {
                eprintln!("[livesync] push failed: {e} — retrying in {backoff}s");
                let app2 = app.clone();
                let delay = backoff;
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_secs(delay));
                    let _ = app2; // re-mark dirty so the worker retries
                    mark_dirty("retry");
                });
                backoff = (backoff * 3).min(120);
            }
        }
    }
}

fn ws_worker(app: AppHandle) {
    use tungstenite::client::IntoClientRequest;
    use tungstenite::Message;
    let mut backoff = 3u64;
    loop {
        let cfg = {
            let state = app.state::<AppState>();
            let c = state.db.lock().unwrap();
            read_live_cfg(&c)
        };
        let Some(cfg) = cfg else {
            std::thread::sleep(std::time::Duration::from_secs(30));
            continue;
        };
        let ws_url = format!(
            "{}/ws",
            cfg.base
                .replacen("https://", "wss://", 1)
                .replacen("http://", "ws://", 1)
        );
        let connected = (|| -> std::result::Result<(), String> {
            let mut req = ws_url.clone().into_client_request().map_err(|e| e.to_string())?;
            if !cfg.user.is_empty() || !cfg.pass.is_empty() {
                use base64::Engine as _;
                let cred = base64::engine::general_purpose::STANDARD
                    .encode(format!("{}:{}", cfg.user, cfg.pass));
                req.headers_mut().insert(
                    "Authorization",
                    format!("Basic {cred}").parse().map_err(|_| "bad auth header")?,
                );
            }
            let (mut ws, _resp) = tungstenite::connect(req).map_err(|e| e.to_string())?;
            // A read timeout turns silence into a liveness ping instead of a
            // dead socket nobody notices (NATs love silently dropping these).
            if let tungstenite::stream::MaybeTlsStream::Plain(t) = ws.get_ref() {
                let _ = t.set_read_timeout(Some(std::time::Duration::from_secs(60)));
            }
            CONNECTED.store(true, Ordering::SeqCst);
            let _ = catch_up(&app); // whatever landed while we were away
            loop {
                match ws.read() {
                    Ok(Message::Text(txt)) => {
                        let seq = serde_json::from_str::<serde_json::Value>(&txt)
                            .ok()
                            .and_then(|v| v["seq"].as_i64())
                            .unwrap_or(0);
                        let state = app.state::<AppState>();
                        let applied = {
                            let c = state.db.lock().unwrap();
                            setting_i64(&c, K_APPLIED)
                        };
                        if seq > applied {
                            let _ = catch_up(&app);
                        }
                    }
                    Ok(_) => {}
                    Err(tungstenite::Error::Io(e))
                        if e.kind() == std::io::ErrorKind::WouldBlock
                            || e.kind() == std::io::ErrorKind::TimedOut =>
                    {
                        // Quiet minute — ping to prove the pipe is alive.
                        if ws.send(Message::Ping(vec![].into())).is_err() {
                            return Err("ping failed".into());
                        }
                    }
                    Err(e) => return Err(e.to_string()),
                }
            }
        })();
        CONNECTED.store(false, Ordering::SeqCst);
        if let Err(e) = connected {
            eprintln!("[livesync] ws: {e} — reconnecting in {backoff}s");
        }
        std::thread::sleep(std::time::Duration::from_secs(backoff));
        backoff = (backoff * 2).min(60);
        // A successful long-lived session resets the backoff quickly.
        if CONNECTED.load(Ordering::SeqCst) {
            backoff = 3;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The heart of live sync: a delta built on device A merges into device B
    /// through the shared merge engine — rows arrive, edits win by recency,
    /// tombstoned deletes apply, and unchanged-since-watermark rows stay home.
    #[test]
    fn delta_roundtrip_between_two_devices() {
        let a = AppState::in_memory().unwrap();
        let b = AppState::in_memory().unwrap();

        // Device A creates a subject; device B knows nothing.
        let subj_id = {
            let ca = a.db.lock().unwrap();
            repo::insert_subject(&ca, "Quantum Physics", None, None, None).unwrap()
        };

        // Build A's delta since 0 (everything).
        let delta = std::env::temp_dir().join(format!("livesync-test-{}.db", crate::db::new_id()));
        let n = {
            let ca = a.db.lock().unwrap();
            build_delta(&ca, &delta, 0).unwrap()
        };
        assert!(n >= 1, "delta should carry the new subject, got {n} rows");

        // Apply on B via the shared merge engine.
        {
            let cb = b.db.lock().unwrap();
            crate::sync::merge_attached(&cb, &delta).unwrap();
            let name: String = cb
                .query_row("SELECT name FROM subjects WHERE id=?1", [&subj_id], |r| r.get(0))
                .unwrap();
            assert_eq!(name, "Quantum Physics");
        }

        // A second delta since "now" is empty — the watermark works.
        let now = crate::sync::now_ms() + 10;
        let delta2 = std::env::temp_dir().join(format!("livesync-test2-{}.db", crate::db::new_id()));
        let n2 = {
            let ca = a.db.lock().unwrap();
            build_delta(&ca, &delta2, now).unwrap()
        };
        assert_eq!(n2, 0, "nothing changed after the watermark");

        // Delete on A → tombstone rides the next delta → B's row dies too.
        {
            let ca = a.db.lock().unwrap();
            ca.execute("DELETE FROM subjects WHERE id=?1", [&subj_id]).unwrap();
        }
        let delta3 = std::env::temp_dir().join(format!("livesync-test3-{}.db", crate::db::new_id()));
        let n3 = {
            let ca = a.db.lock().unwrap();
            build_delta(&ca, &delta3, now).unwrap()
        };
        assert!(n3 >= 1, "tombstone should ride the delta");
        {
            let cb = b.db.lock().unwrap();
            crate::sync::merge_attached(&cb, &delta3).unwrap();
            let gone: i64 = cb
                .query_row("SELECT COUNT(*) FROM subjects WHERE id=?1", [&subj_id], |r| r.get(0))
                .unwrap();
            assert_eq!(gone, 0, "tombstoned delete must apply on the peer");
        }
        for p in [&delta, &delta2, &delta3] {
            let _ = std::fs::remove_file(p);
        }
    }
}
