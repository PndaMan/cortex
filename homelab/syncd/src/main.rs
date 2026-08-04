//! Cortex live-sync service.
//!
//! A deliberately dumb, ordered, authenticated DELTA LOG with push:
//!   • Clients POST small SQLite "delta" files (rows changed since their last
//!     push); each gets a monotonically increasing sequence number.
//!   • Every connected client holds a WebSocket; the moment a delta lands, its
//!     seq is broadcast — peers fetch and merge it within a second.
//!   • New/behind devices bootstrap from a full snapshot + the deltas after it.
//!   • A periodic client-uploaded snapshot compacts the log (deltas at or below
//!     the snapshot's seq are pruned).
//!
//! The server never interprets the payloads — merging happens client-side with
//! Cortex's existing newest-wins + tombstone logic, and credential fields are
//! encrypted client-side before upload. Auth is HTTP Basic against SYNC_USER /
//! SYNC_PASSWORD (the same credentials as the WebDAV container, so users
//! configure one pair).

use axum::{
    extract::{ws::Message, ws::WebSocket, Path as AxPath, Query, State, WebSocketUpgrade},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post, put},
    Router,
};
use base64::Engine as _;
use std::path::PathBuf;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;
use tokio::sync::broadcast;

struct App {
    data: PathBuf,
    seq: AtomicI64,
    snapshot_seq: AtomicI64,
    tx: broadcast::Sender<i64>,
    user: String,
    pass: String,
}

fn delta_path(data: &std::path::Path, seq: i64) -> PathBuf {
    data.join("deltas").join(format!("{seq:012}.db"))
}

fn list_delta_seqs(data: &std::path::Path) -> Vec<i64> {
    let mut seqs: Vec<i64> = std::fs::read_dir(data.join("deltas"))
        .map(|rd| {
            rd.filter_map(|e| {
                e.ok()?
                    .file_name()
                    .to_str()?
                    .strip_suffix(".db")?
                    .parse::<i64>()
                    .ok()
            })
            .collect()
        })
        .unwrap_or_default();
    seqs.sort_unstable();
    seqs
}

fn check_auth(app: &App, headers: &HeaderMap) -> bool {
    if app.user.is_empty() && app.pass.is_empty() {
        return true; // auth disabled (LAN-only setups)
    }
    let Some(v) = headers.get(header::AUTHORIZATION).and_then(|v| v.to_str().ok()) else {
        return false;
    };
    let Some(b64) = v.strip_prefix("Basic ") else { return false };
    let Ok(raw) = base64::engine::general_purpose::STANDARD.decode(b64) else {
        return false;
    };
    let Ok(s) = String::from_utf8(raw) else { return false };
    let Some((u, p)) = s.split_once(':') else { return false };
    u == app.user && p == app.pass
}

fn unauthorized() -> Response {
    (
        StatusCode::UNAUTHORIZED,
        [(header::WWW_AUTHENTICATE, "Basic realm=\"cortex-syncd\"")],
        "unauthorized",
    )
        .into_response()
}

#[derive(serde::Deserialize)]
struct SinceQ {
    since: Option<i64>,
}

#[derive(serde::Deserialize)]
struct SeqQ {
    seq: Option<i64>,
}

async fn get_seq(State(app): State<Arc<App>>, headers: HeaderMap) -> Response {
    if !check_auth(&app, &headers) {
        return unauthorized();
    }
    axum::Json(serde_json::json!({
        "seq": app.seq.load(Ordering::SeqCst),
        "snapshot_seq": app.snapshot_seq.load(Ordering::SeqCst),
    }))
    .into_response()
}

async fn post_delta(
    State(app): State<Arc<App>>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Response {
    if !check_auth(&app, &headers) {
        return unauthorized();
    }
    if body.is_empty() {
        return (StatusCode::BAD_REQUEST, "empty delta").into_response();
    }
    // Assign the next seq and persist atomically (tmp + rename) so a crashed
    // write can never surface a half-delta to peers.
    let seq = app.seq.fetch_add(1, Ordering::SeqCst) + 1;
    let path = delta_path(&app.data, seq);
    let tmp = path.with_extension("tmp");
    if let Err(e) = tokio::fs::write(&tmp, &body).await {
        return (StatusCode::INTERNAL_SERVER_ERROR, format!("write: {e}")).into_response();
    }
    if let Err(e) = tokio::fs::rename(&tmp, &path).await {
        return (StatusCode::INTERNAL_SERVER_ERROR, format!("rename: {e}")).into_response();
    }
    let _ = app.tx.send(seq); // push to every connected peer
    axum::Json(serde_json::json!({ "seq": seq })).into_response()
}

async fn list_deltas(
    State(app): State<Arc<App>>,
    headers: HeaderMap,
    Query(q): Query<SinceQ>,
) -> Response {
    if !check_auth(&app, &headers) {
        return unauthorized();
    }
    let since = q.since.unwrap_or(0);
    let seqs: Vec<i64> = list_delta_seqs(&app.data).into_iter().filter(|s| *s > since).collect();
    axum::Json(serde_json::json!({ "seqs": seqs })).into_response()
}

async fn get_delta(
    State(app): State<Arc<App>>,
    headers: HeaderMap,
    AxPath(seq): AxPath<i64>,
) -> Response {
    if !check_auth(&app, &headers) {
        return unauthorized();
    }
    match tokio::fs::read(delta_path(&app.data, seq)).await {
        Ok(bytes) => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "application/octet-stream")],
            bytes,
        )
            .into_response(),
        // Pruned by compaction (or never existed): the client falls back to the
        // snapshot bootstrap path.
        Err(_) => (StatusCode::NOT_FOUND, "no such delta (compacted?)").into_response(),
    }
}

async fn put_snapshot(
    State(app): State<Arc<App>>,
    headers: HeaderMap,
    Query(q): Query<SeqQ>,
    body: axum::body::Bytes,
) -> Response {
    if !check_auth(&app, &headers) {
        return unauthorized();
    }
    if body.is_empty() {
        return (StatusCode::BAD_REQUEST, "empty snapshot").into_response();
    }
    let seq = q.seq.unwrap_or_else(|| app.seq.load(Ordering::SeqCst));
    let snap = app.data.join("snapshot.db");
    let tmp = snap.with_extension("tmp");
    if let Err(e) = tokio::fs::write(&tmp, &body).await {
        return (StatusCode::INTERNAL_SERVER_ERROR, format!("write: {e}")).into_response();
    }
    if let Err(e) = tokio::fs::rename(&tmp, &snap).await {
        return (StatusCode::INTERNAL_SERVER_ERROR, format!("rename: {e}")).into_response();
    }
    let _ = tokio::fs::write(app.data.join("snapshot.seq"), seq.to_string()).await;
    app.snapshot_seq.store(seq, Ordering::SeqCst);
    // Compaction: everything the snapshot already contains can go.
    for s in list_delta_seqs(&app.data) {
        if s <= seq {
            let _ = tokio::fs::remove_file(delta_path(&app.data, s)).await;
        }
    }
    axum::Json(serde_json::json!({ "seq": seq })).into_response()
}

async fn get_snapshot(State(app): State<Arc<App>>, headers: HeaderMap) -> Response {
    if !check_auth(&app, &headers) {
        return unauthorized();
    }
    match tokio::fs::read(app.data.join("snapshot.db")).await {
        Ok(bytes) => (
            StatusCode::OK,
            [
                (header::CONTENT_TYPE, "application/octet-stream".to_string()),
                (
                    header::HeaderName::from_static("x-snapshot-seq"),
                    app.snapshot_seq.load(Ordering::SeqCst).to_string(),
                ),
            ],
            bytes,
        )
            .into_response(),
        Err(_) => (StatusCode::NOT_FOUND, "no snapshot yet").into_response(),
    }
}

async fn ws_handler(
    State(app): State<Arc<App>>,
    headers: HeaderMap,
    ws: WebSocketUpgrade,
) -> Response {
    if !check_auth(&app, &headers) {
        return unauthorized();
    }
    ws.on_upgrade(move |sock| ws_loop(app, sock))
}

async fn ws_loop(app: Arc<App>, mut sock: WebSocket) {
    let mut rx = app.tx.subscribe();
    // Greet with the current seq so a client can catch up immediately.
    let hello = serde_json::json!({ "seq": app.seq.load(Ordering::SeqCst) }).to_string();
    if sock.send(Message::Text(hello.into())).await.is_err() {
        return;
    }
    loop {
        tokio::select! {
            seq = rx.recv() => {
                let Ok(seq) = seq else { break };
                let msg = serde_json::json!({ "seq": seq }).to_string();
                if sock.send(Message::Text(msg.into())).await.is_err() {
                    break;
                }
            }
            incoming = sock.recv() => {
                match incoming {
                    // Answer pings implicitly (axum does), ignore client chatter.
                    Some(Ok(_)) => {}
                    _ => break, // closed / errored
                }
            }
        }
    }
}

fn build_app(data: PathBuf, user: String, pass: String) -> Arc<App> {
    std::fs::create_dir_all(data.join("deltas")).expect("create data dir");
    let seq = list_delta_seqs(&data).last().copied().unwrap_or_else(|| {
        // No deltas on disk — resume from the snapshot's seq so numbering never
        // goes backwards after compaction + restart.
        std::fs::read_to_string(data.join("snapshot.seq"))
            .ok()
            .and_then(|s| s.trim().parse().ok())
            .unwrap_or(0)
    });
    let snapshot_seq = std::fs::read_to_string(data.join("snapshot.seq"))
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(0);
    let (tx, _) = broadcast::channel(256);
    Arc::new(App { data, seq: AtomicI64::new(seq), snapshot_seq: AtomicI64::new(snapshot_seq), tx, user, pass })
}

fn router(app: Arc<App>) -> Router {
    Router::new()
        .route("/seq", get(get_seq))
        .route("/deltas", post(post_delta).get(list_deltas))
        .route("/deltas/{seq}", get(get_delta))
        .route("/snapshot", put(put_snapshot).get(get_snapshot))
        .route("/ws", get(ws_handler))
        // Vaults with big libraries still fit comfortably; reject absurdity.
        .layer(tower_http::limit::RequestBodyLimitLayer::new(2 * 1024 * 1024 * 1024))
        .with_state(app)
}

#[tokio::main]
async fn main() {
    let data = PathBuf::from(std::env::var("SYNCD_DATA").unwrap_or_else(|_| "/data".into()));
    let user = std::env::var("SYNC_USER").unwrap_or_default();
    let pass = std::env::var("SYNC_PASSWORD").unwrap_or_default();
    if user.is_empty() && pass.is_empty() {
        eprintln!("syncd: WARNING — SYNC_USER/SYNC_PASSWORD unset, running UNAUTHENTICATED (LAN-only!)");
    }
    let port: u16 = std::env::var("SYNCD_PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(8787);
    let app = build_app(data, user, pass);
    let listener = tokio::net::TcpListener::bind(("0.0.0.0", port)).await.expect("bind");
    eprintln!("cortex-syncd listening on :{port}");
    axum::serve(listener, router(app)).await.expect("serve");
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures_util::{SinkExt, StreamExt};

    async fn spawn_server(auth: bool) -> (String, tempdir::TempDirGuard) {
        let dir = tempdir::guard();
        let (user, pass) = if auth { ("u".into(), "p".into()) } else { (String::new(), String::new()) };
        let app = build_app(dir.path.clone(), user, pass);
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, router(app)).await.unwrap() });
        (format!("http://{addr}"), dir)
    }

    /// Minimal tempdir without a dependency.
    mod tempdir {
        pub struct TempDirGuard {
            pub path: std::path::PathBuf,
        }
        impl Drop for TempDirGuard {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.path);
            }
        }
        pub fn guard() -> TempDirGuard {
            let path = std::env::temp_dir().join(format!(
                "syncd-test-{}-{:x}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            std::fs::create_dir_all(&path).unwrap();
            TempDirGuard { path }
        }
    }

    #[tokio::test]
    async fn delta_roundtrip_and_ws_push() {
        let (base, _dir) = spawn_server(false).await;
        let http = reqwest::Client::new();

        // Open the WS first so we observe the push.
        let ws_url = format!("{}/ws", base.replace("http://", "ws://"));
        let (mut ws, _) = tokio_tungstenite::connect_async(&ws_url).await.unwrap();
        // Greeting carries seq 0.
        let hello = ws.next().await.unwrap().unwrap().into_text().unwrap();
        assert!(hello.contains("\"seq\":0"), "greeting: {hello}");

        // Push a delta.
        let r = http.post(format!("{base}/deltas")).body(b"delta-one".to_vec()).send().await.unwrap();
        assert_eq!(r.status(), 200);
        let seq = r.json::<serde_json::Value>().await.unwrap()["seq"].as_i64().unwrap();
        assert_eq!(seq, 1);

        // WS got the push.
        let pushed = ws.next().await.unwrap().unwrap().into_text().unwrap();
        assert!(pushed.contains("\"seq\":1"), "pushed: {pushed}");
        ws.send(tokio_tungstenite::tungstenite::Message::Close(None)).await.ok();

        // Catch-up listing + fetch.
        let seqs = http
            .get(format!("{base}/deltas?since=0"))
            .send().await.unwrap()
            .json::<serde_json::Value>().await.unwrap();
        assert_eq!(seqs["seqs"], serde_json::json!([1]));
        let body = http.get(format!("{base}/deltas/1")).send().await.unwrap().bytes().await.unwrap();
        assert_eq!(&body[..], b"delta-one");
    }

    #[tokio::test]
    async fn snapshot_compacts_and_seq_survives_restartlike_reload() {
        let (base, dir) = spawn_server(false).await;
        let http = reqwest::Client::new();
        for i in 0..3 {
            http.post(format!("{base}/deltas")).body(format!("d{i}")).send().await.unwrap();
        }
        // Snapshot at seq 2 → deltas 1,2 pruned, 3 kept.
        let r = http.put(format!("{base}/snapshot?seq=2")).body(b"snap".to_vec()).send().await.unwrap();
        assert_eq!(r.status(), 200);
        let seqs = http.get(format!("{base}/deltas?since=0")).send().await.unwrap()
            .json::<serde_json::Value>().await.unwrap();
        assert_eq!(seqs["seqs"], serde_json::json!([3]));
        let snap = http.get(format!("{base}/snapshot")).send().await.unwrap();
        assert_eq!(snap.headers()["x-snapshot-seq"], "2");
        assert_eq!(&snap.bytes().await.unwrap()[..], b"snap");

        // Reload state from disk (as a restart would): seq resumes at 3.
        let app2 = build_app(dir.path.clone(), String::new(), String::new());
        assert_eq!(app2.seq.load(Ordering::SeqCst), 3);
        assert_eq!(app2.snapshot_seq.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn auth_is_enforced_when_configured() {
        let (base, _dir) = spawn_server(true).await;
        let http = reqwest::Client::new();
        let r = http.get(format!("{base}/seq")).send().await.unwrap();
        assert_eq!(r.status(), 401);
        let r = http.get(format!("{base}/seq")).basic_auth("u", Some("p")).send().await.unwrap();
        assert_eq!(r.status(), 200);
        let r = http.get(format!("{base}/seq")).basic_auth("u", Some("wrong")).send().await.unwrap();
        assert_eq!(r.status(), 401);
    }
}
