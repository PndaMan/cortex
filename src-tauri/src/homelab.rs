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

/// Read a settings URL key and resolve it through the homelab fallback chain.
pub fn resolved_setting(conn: &Connection, key: &str) -> Option<String> {
    let raw = repo::get_setting(conn, key).ok().flatten()?;
    if raw.trim().is_empty() {
        return None;
    }
    Some(resolve(conn, &raw))
}
