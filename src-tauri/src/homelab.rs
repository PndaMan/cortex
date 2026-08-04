//! Shared homelab endpoint resolution: every self-hosted service (Ollama,
//! Whisper, SearXNG, …) is configured with one "local" URL, and the user can set
//! a global Tailscale base and/or public base. We try local → Tailscale → public
//! and use the first reachable origin, so the same device works on LAN, over
//! Tailscale, or from anywhere — without reconfiguring each service.
//!
//! Resolution is cached briefly (per primary URL) so frequent callers (embeddings,
//! chat, search) don't probe the network on every request. On any failure we fall
//! back to the primary URL, so behaviour is unchanged when no bases are set.

use crate::repo;
use rusqlite::Connection;
use std::collections::{HashMap, HashSet};
use std::sync::Mutex;
use std::time::{Duration, Instant};

const TTL: Duration = Duration::from_secs(60);
static CACHE: Mutex<Option<HashMap<String, (String, Instant)>>> = Mutex::new(None);
// Origins with a background reachability probe in flight, so a burst of resolve()
// calls for the same primary spawns at most one network probe.
static PROBING: Mutex<Option<HashSet<String>>> = Mutex::new(None);

/// Replace `primary`'s origin (scheme://host:port) with `base`'s, keeping the
/// path/query. e.g. swap_origin("http://192.168.1.5:9009/v1", "http://lab.ts.net")
/// → "http://lab.ts.net:9009/v1" (port kept from primary unless base sets one).
fn swap_origin(primary: &str, base: &str) -> Option<String> {
    let p = reqwest::Url::parse(primary).ok()?;
    let b = reqwest::Url::parse(base.trim().trim_end_matches('/')).ok()?;
    let mut out = p.clone();
    out.set_scheme(b.scheme()).ok()?;
    out.set_host(b.host_str()).ok()?;
    // If the base names an explicit port use it; otherwise keep the service's port.
    if b.port().is_some() {
        out.set_port(b.port()).ok()?;
    }
    Some(out.as_str().trim_end_matches('/').to_string())
}

/// Quick reachability probe: any HTTP response (even 404/401) means the host is up.
fn reachable(origin: &str) -> bool {
    let client = reqwest::blocking::Client::builder()
        .connect_timeout(Duration::from_secs(2)) // fail fast on an unreachable LAN URL (phones)
        .timeout(Duration::from_secs(3))
        .build()
        .unwrap_or_default();
    client.get(origin).send().is_ok()
}

/// Resolve a homelab service's effective base URL (local → Tailscale → public),
/// preferring the first REACHABLE origin.
///
/// NON-BLOCKING by contract. Callers run on hot paths — including *synchronous* Tauri
/// commands, which execute on the GTK/event-loop thread (e.g. `verify_provider` →
/// `read_keys`). Probing the network inline there froze the whole UI ("Application Not
/// Responding") whenever the cache was cold and a base was unreachable (~60s timeout).
/// So this NEVER does network I/O on the caller's thread: it returns the cached origin
/// if known (even slightly stale) or the primary otherwise, and refreshes the cache from
/// a BACKGROUND probe. The sync loop's `warm` pre-populates the cache, so steady-state
/// callers get the reachable origin directly; the worst case is one request against the
/// primary before the background probe lands.
pub fn resolve(primary: &str, ts: Option<&str>, pubb: Option<&str>) -> String {
    let primary = primary.trim().trim_end_matches('/').to_string();
    if primary.is_empty() {
        return primary;
    }
    let ts = ts.map(str::trim).filter(|s| !s.is_empty()).map(str::to_string);
    let pubb = pubb.map(str::trim).filter(|s| !s.is_empty()).map(str::to_string);
    if ts.is_none() && pubb.is_none() {
        return primary; // no fallbacks configured — nothing to probe
    }

    let cached = {
        let mut guard = CACHE.lock().unwrap();
        guard
            .get_or_insert_with(HashMap::new)
            .get(&primary)
            .map(|(url, at)| (url.clone(), at.elapsed() < TTL))
    };
    match cached {
        Some((url, true)) => url, // fresh — trust it
        Some((url, false)) => {
            spawn_probe(primary, ts, pubb); // stale — refresh in the background...
            url // ...but hand back the last-good origin NOW, never block
        }
        None => {
            spawn_probe(primary.clone(), ts, pubb);
            primary // unknown — best guess is the primary; probe in the background
        }
    }
}

/// Kick off a single background reachability probe for `primary` (deduped: at most one
/// in flight per origin), updating the cache when it lands. Blocking network lives ONLY
/// here, on a throwaway thread — never on a caller's thread.
fn spawn_probe(primary: String, ts: Option<String>, pubb: Option<String>) {
    {
        let mut guard = PROBING.lock().unwrap();
        if !guard.get_or_insert_with(HashSet::new).insert(primary.clone()) {
            return; // a probe for this origin is already running
        }
    }
    std::thread::spawn(move || {
        let chosen = probe(&primary, ts.as_deref(), pubb.as_deref());
        CACHE
            .lock()
            .unwrap()
            .get_or_insert_with(HashMap::new)
            .insert(primary.clone(), (chosen, Instant::now()));
        if let Some(set) = PROBING.lock().unwrap().as_mut() {
            set.remove(&primary);
        }
    });
}

/// Pick the first reachable origin among local/Tailscale/public. BLOCKING (network) —
/// only ever called off the hot path: the background probe and `warm`, never inline.
fn probe(primary: &str, ts: Option<&str>, pubb: Option<&str>) -> String {
    let mut candidates = vec![primary.to_string()];
    if let Some(t) = ts {
        if let Some(u) = swap_origin(primary, t) {
            candidates.push(u);
        }
    }
    if let Some(p) = pubb {
        if let Some(u) = swap_origin(primary, p) {
            candidates.push(u);
        }
    }
    candidates
        .iter()
        .find(|c| reachable(c))
        .cloned()
        .unwrap_or_else(|| primary.to_string())
}

/// Proactively resolve every homelab service so the reachable origin is cached
/// (TTL) before the user needs it — called from the background sync loop so first
/// foreground use (chat, transcription, search, sync) doesn't pay a cold network
/// probe. Best-effort: a service that isn't configured or isn't reachable just falls
/// back to its primary, exactly as on-demand resolution would. Under the unified
/// `homelab_base` all services share one origin, so this is a single probe round
/// (the first resolve caches the base; the rest hit the cache).
/// CRITICAL: this must NOT hold the DB lock while probing the network. `reachable()`
/// is pure network (it never touches the DB), but the SQLite connection lives behind a
/// single `Mutex`; holding that mutex across a multi-second reachability probe blocks
/// every other DB access — including the *synchronous* `get_all_settings` command, which
/// runs on the GTK/event-loop thread. That blocked the whole UI ("Application Not
/// Responding") until the probe timed out (~60s on an unreachable Tailscale/public base).
/// So: snapshot the config under a SHORT lock, release it, THEN probe lock-free.
pub fn warm(state: &crate::db::AppState) {
    let (ts, pubb, primaries) = {
        let Ok(c) = state.db.lock() else {
            return;
        };
        let ts = repo::get_setting(&c, "homelab_tailscale_base").ok().flatten();
        let pubb = repo::get_setting(&c, "homelab_public_base").ok().flatten();
        let mut primaries: Vec<String> = Vec::new();
        for key in ["sync_url", "ollama_url", "whisper_url", "searxng_url", "ingest_url"] {
            if let Some(p) = warm_primary(&c, key) {
                if !primaries.contains(&p) {
                    primaries.push(p);
                }
            }
        }
        (ts, pubb, primaries)
    }; // <-- DB lock released here, before any network probe
    // We're on the background sync thread, so a blocking probe is fine here — it
    // populates the cache directly, so hot-path resolve() returns the reachable origin
    // without ever probing inline.
    for primary in primaries {
        let chosen = probe(&primary, ts.as_deref(), pubb.as_deref());
        CACHE
            .lock()
            .unwrap()
            .get_or_insert_with(HashMap::new)
            .insert(primary, (chosen, Instant::now()));
    }
}

/// The origin `resolve()` should probe for a service, computed WITHOUT any network: the
/// explicit per-service URL if set, else the first configured homelab base. Mirrors the
/// resolve targets in [`resolved_setting`] so `warm` pre-populates the same cache keys.
fn warm_primary(conn: &Connection, key: &str) -> Option<String> {
    if let Some(raw) = repo::get_setting(conn, key)
        .ok()
        .flatten()
        .filter(|s| !s.trim().is_empty())
    {
        return Some(raw);
    }
    if service_path(key).is_some() {
        return ["homelab_base", "homelab_tailscale_base", "homelab_public_base"]
            .iter()
            .find_map(|k| {
                repo::get_setting(conn, k)
                    .ok()
                    .flatten()
                    .filter(|s| !s.trim().is_empty())
            })
            .map(|b| b.trim().trim_end_matches('/').to_string());
    }
    None
}

/// Path prefix each service lives under when the unified single-URL homelab is
/// used — everything is reached through one base URL + a reverse proxy that
/// strips these prefixes (see homelab/Caddyfile).
fn service_path(key: &str) -> Option<&'static str> {
    match key {
        "searxng_url" => Some("/searxng"),
        "whisper_url" => Some("/whisper"),
        "ollama_url" => Some("/ollama"),
        "ingest_url" => Some("/ingest"),
        "sync_url" => Some("/sync"),
        "syncd_url" => Some("/syncd"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use crate::db::AppState;
    use crate::repo;

    #[test]
    fn unified_base_derives_service_paths_and_overrides_win() {
        let st = AppState::in_memory().unwrap();
        let c = st.db.lock().unwrap();

        // No config → nothing resolves.
        assert_eq!(super::resolved_setting(&c, "searxng_url"), None);

        // One homelab base → every service derived from it (no Tailscale/public set,
        // so resolve() returns the base unchanged + the path).
        repo::set_setting(&c, "homelab_base", "http://10.0.0.5:8080").unwrap();
        assert_eq!(
            super::resolved_setting(&c, "searxng_url").as_deref(),
            Some("http://10.0.0.5:8080/searxng")
        );
        assert_eq!(
            super::resolved_setting(&c, "whisper_url").as_deref(),
            Some("http://10.0.0.5:8080/whisper")
        );
        assert_eq!(
            super::resolved_setting(&c, "ollama_url").as_deref(),
            Some("http://10.0.0.5:8080/ollama")
        );

        // An explicit per-service override beats the unified base.
        repo::set_setting(&c, "ollama_url", "http://192.168.1.9:11434").unwrap();
        assert_eq!(
            super::resolved_setting(&c, "ollama_url").as_deref(),
            Some("http://192.168.1.9:11434")
        );

        // A non-service key with no value still resolves to None.
        assert_eq!(super::resolved_setting(&c, "moodle_url"), None);
    }

    #[test]
    fn homelab_token_rides_derived_urls_but_never_sync_or_overrides() {
        let st = AppState::in_memory().unwrap();
        let c = st.db.lock().unwrap();
        repo::set_setting(&c, "homelab_base", "http://10.0.0.5:8080").unwrap();
        repo::set_setting(&c, "homelab_token", "sekrit").unwrap();

        // Base-derived service URLs carry the token as Basic credentials.
        assert_eq!(
            super::resolved_setting(&c, "whisper_url").as_deref(),
            Some("http://cortex:sekrit@10.0.0.5:8080/whisper")
        );
        // /sync has its own WebDAV credentials — never the homelab token.
        assert_eq!(
            super::resolved_setting(&c, "sync_url").as_deref(),
            Some("http://10.0.0.5:8080/sync")
        );
        // Explicit per-service overrides may point off-homelab — no token.
        repo::set_setting(&c, "ollama_url", "http://192.168.1.9:11434").unwrap();
        assert_eq!(
            super::resolved_setting(&c, "ollama_url").as_deref(),
            Some("http://192.168.1.9:11434")
        );
    }
}

/// Read a settings URL key and resolve it through the homelab fallback chain.
///
/// Two configurations are supported:
///  • **Unified** (preferred): one `homelab_base` URL with every service behind a
///    path prefix (`/searxng`, `/whisper`, `/ollama`). Set the base once and all
///    services follow, with the same local→Tailscale→public auto-resolution.
///  • **Legacy**: an explicit per-service URL (`searxng_url`, `whisper_url`,
///    `ollama_url`). Used as a fallback when `homelab_base` is unset, so existing
///    setups keep working unchanged.
pub fn resolved_setting(conn: &Connection, key: &str) -> Option<String> {
    // Read the fallback bases once, up front, so the (network) resolve below is
    // connection-free — see resolve()'s contract about never probing under the DB lock.
    let ts = repo::get_setting(conn, "homelab_tailscale_base").ok().flatten();
    let pubb = repo::get_setting(conn, "homelab_public_base").ok().flatten();
    // An explicit per-service URL wins (override) — keeps existing setups working
    // and lets a service live somewhere other than the unified homelab.
    if let Some(raw) = repo::get_setting(conn, key)
        .ok()
        .flatten()
        .filter(|s| !s.trim().is_empty())
    {
        return Some(resolve(&raw, ts.as_deref(), pubb.as_deref()));
    }
    // Otherwise derive it from a homelab base URL + the service's path. Prefer the LAN
    // base, but fall back to the Tailscale/public base when that's all the user set —
    // e.g. a phone that only ever reaches the homelab over Tailscale (no LAN URL).
    if let Some(path) = service_path(key) {
        let base = ["homelab_base", "homelab_tailscale_base", "homelab_public_base"]
            .iter()
            .find_map(|k| {
                repo::get_setting(conn, k)
                    .ok()
                    .flatten()
                    .filter(|s| !s.trim().is_empty())
            });
        if let Some(base) = base {
            let resolved = resolve(base.trim().trim_end_matches('/'), ts.as_deref(), pubb.as_deref());
            return Some(inject_token(conn, key, format!("{resolved}{path}")));
        }
    }
    None
}

/// Attach the homelab access token to a homelab-DERIVED service URL as URL
/// credentials (`cortex:<token>@host`) — reqwest turns userinfo into an
/// `Authorization: Basic` header on every request, so one injection point here
/// authenticates whisper/searxng/ollama/ingest calls with no per-call changes.
/// The Caddy proxy enforces the matching `basic_auth` when CORTEX_TOKEN is set
/// (see homelab/Caddyfile), which is what makes a PUBLIC homelab URL safe.
///
/// Deliberately NOT applied to:
///  • `sync_url` — the WebDAV target has its own username/password (two Basic
///    credentials can't coexist), so the proxy exempts /sync from token auth;
///  • explicit per-service override URLs — those may point at non-homelab hosts
///    (e.g. a cloud endpoint) that must never see the homelab token.
fn inject_token(conn: &Connection, key: &str, url: String) -> String {
    // Both sync services authenticate with their OWN Basic credentials
    // (sync_user/sync_pass) — two Basic headers can't coexist, so the homelab
    // token never rides these URLs; the proxy exempts them from token auth.
    if key == "sync_url" || key == "syncd_url" {
        return url;
    }
    let Some(token) = repo::get_setting(conn, "homelab_token")
        .ok()
        .flatten()
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
    else {
        return url;
    };
    let Ok(mut u) = reqwest::Url::parse(&url) else {
        return url;
    };
    if !u.username().is_empty() || u.password().is_some() {
        return url; // the user already put credentials in the URL — respect them
    }
    if u.set_username("cortex").is_err() || u.set_password(Some(&token)).is_err() {
        return url;
    }
    u.to_string()
}
