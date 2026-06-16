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
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

const TTL: Duration = Duration::from_secs(60);
static CACHE: Mutex<Option<HashMap<String, (String, Instant)>>> = Mutex::new(None);

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

/// Resolve a homelab service's effective base URL given the user's Tailscale/public
/// bases. Returns `primary` unchanged when no bases are set or nothing else is
/// reachable. Result is cached for `TTL`.
pub fn resolve(conn: &Connection, primary: &str) -> String {
    let primary = primary.trim().trim_end_matches('/').to_string();
    if primary.is_empty() {
        return primary;
    }
    let ts = repo::get_setting(conn, "homelab_tailscale_base")
        .ok()
        .flatten()
        .filter(|s| !s.trim().is_empty());
    let pubb = repo::get_setting(conn, "homelab_public_base")
        .ok()
        .flatten()
        .filter(|s| !s.trim().is_empty());
    if ts.is_none() && pubb.is_none() {
        return primary; // no fallbacks configured — nothing to do
    }

    {
        let mut guard = CACHE.lock().unwrap();
        let map = guard.get_or_insert_with(HashMap::new);
        if let Some((url, at)) = map.get(&primary) {
            if at.elapsed() < TTL {
                return url.clone();
            }
        }
    }

    let mut candidates = vec![primary.clone()];
    if let Some(t) = &ts {
        if let Some(u) = swap_origin(&primary, t) {
            candidates.push(u);
        }
    }
    if let Some(p) = &pubb {
        if let Some(u) = swap_origin(&primary, p) {
            candidates.push(u);
        }
    }
    let chosen = candidates
        .iter()
        .find(|c| reachable(c))
        .cloned()
        .unwrap_or_else(|| primary.clone());

    let mut guard = CACHE.lock().unwrap();
    guard
        .get_or_insert_with(HashMap::new)
        .insert(primary, (chosen.clone(), Instant::now()));
    chosen
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
    // An explicit per-service URL wins (override) — keeps existing setups working
    // and lets a service live somewhere other than the unified homelab.
    if let Some(raw) = repo::get_setting(conn, key)
        .ok()
        .flatten()
        .filter(|s| !s.trim().is_empty())
    {
        return Some(resolve(conn, &raw));
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
            let resolved = resolve(conn, base.trim().trim_end_matches('/'));
            return Some(format!("{resolved}{path}"));
        }
    }
    None
}
