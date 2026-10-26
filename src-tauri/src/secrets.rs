//! API keys, OAuth tokens and the sync password live in the OS credential store —
//! Keychain on macOS/iOS, Credential Manager on Windows, Secret Service (GNOME Keyring,
//! KWallet) on Linux — instead of the SQLite `settings` table.
//!
//! Callers keep using `repo::get_setting` / `set_setting`; those route the keys listed
//! in [`KEYS`] through here. Where no store is usable (Android, or a Linux session with
//! no keyring running) values stay in SQLite exactly as before.
//!
//! The webview never receives a stored secret: `get_setting` / `get_all_settings` hand
//! it [`MASK`] instead, and writing the mask back leaves the stored value untouched.

use rusqlite::{params, Connection, OptionalExtension};
use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

/// Keychain service name (the bundle identifier); the entry name is the settings key.
const SERVICE: &str = "study.cortex.app";

/// Settings keys that hold secrets.
pub const KEYS: &[&str] = &[
    "gemini_api_key",
    "claude_api_key",
    "openai_api_key",
    "openrouter_api_key",
    "custom_api_key",
    "whisper_api_key",
    "google_client_secret",
    "google_access_token",
    "google_refresh_token",
    "moodle_token",
    "homelab_token",
    "sync_pass",
];

/// What the webview sees in place of a stored secret.
pub const MASK: &str = "••••••••";

pub fn is_secret(key: &str) -> bool {
    KEYS.contains(&key)
}

/// A secret typed into Settings. None means "unchanged" (the field still holds the
/// mask). Mask characters left in front of new text are stripped.
pub fn from_input(value: &str) -> Option<String> {
    if value == MASK {
        return None;
    }
    Some(value.replace(MASK, ""))
}

/// The stored value when the webview sends back the mask, otherwise what it sent.
pub fn resolve_input(conn: &Connection, key: &str, value: String) -> String {
    match from_input(&value) {
        Some(v) => v,
        None => crate::repo::get_setting(conn, key).ok().flatten().unwrap_or_default(),
    }
}

/// Pick the platform credential store. Called once at startup; if the store can't be
/// reached, secrets stay in SQLite for this run.
pub fn init() {
    if let Err(e) = platform_store() {
        eprintln!("[secrets] no system keychain, keys stay in the app database: {e}");
        return;
    }
    // Secret Service can fail only on first use (no keyring daemon), so probe now.
    let probe = keyring_core::Entry::new(SERVICE, "probe").and_then(|e| e.get_password());
    if let Err(e) = probe {
        if !matches!(e, keyring_core::Error::NoEntry) {
            eprintln!("[secrets] system keychain unusable, keys stay in the app database: {e}");
            keyring_core::unset_default_store();
        }
    }
}

fn platform_store() -> keyring_core::Result<()> {
    #[cfg(target_os = "macos")]
    keyring_core::set_default_store(apple_native_keyring_store::keychain::Store::new()?);
    #[cfg(target_os = "ios")]
    keyring_core::set_default_store(apple_native_keyring_store::protected::Store::new()?);
    #[cfg(target_os = "windows")]
    keyring_core::set_default_store(windows_native_keyring_store::Store::new()?);
    #[cfg(target_os = "linux")]
    keyring_core::set_default_store(zbus_secret_service_keyring_store::Store::new()?);
    #[cfg(not(any(
        target_os = "macos",
        target_os = "ios",
        target_os = "windows",
        target_os = "linux"
    )))]
    return Err(keyring_core::Error::NotSupportedByStore(std::env::consts::OS.into()));
    #[allow(unreachable_code)]
    Ok(())
}

/// Whether secrets are going to the system keychain (shown in Settings).
pub fn available() -> bool {
    #[cfg(test)]
    return TEST_STORE.with(|s| s.borrow().is_some());
    #[cfg(not(test))]
    keyring_core::get_default_store().is_some()
}

// Only this process writes these entries, so a read-through cache stays correct and
// spares a D-Bus/Keychain round-trip on every LLM call. Off in tests, where each test
// thread has its own store.
#[cfg(not(test))]
static CACHE: LazyLock<Mutex<HashMap<String, Option<String>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

fn cached(key: &str) -> Option<Option<String>> {
    #[cfg(test)]
    return None;
    #[cfg(not(test))]
    CACHE.lock().unwrap().get(key).cloned()
}

fn remember(key: &str, value: Option<String>) {
    #[cfg(not(test))]
    CACHE.lock().unwrap().insert(key.to_string(), value);
    #[cfg(test)]
    let _ = (key, value);
}

/// The keychain value, or None when there is none or no keychain is usable (the
/// caller then falls back to SQLite).
pub fn get(key: &str) -> Option<String> {
    if !available() {
        return None;
    }
    if let Some(v) = cached(key) {
        return v;
    }
    let v = read(key).ok()?;
    remember(key, v.clone());
    v
}

/// Store a value (empty deletes it). False when no keychain is usable or the write
/// failed, in which case the caller keeps the value in SQLite.
pub fn set(key: &str, value: &str) -> bool {
    if !available() {
        return false;
    }
    match write(key, value) {
        Ok(()) => {
            remember(key, (!value.is_empty()).then(|| value.to_string()));
            true
        }
        Err(e) => {
            eprintln!("[secrets] keychain write failed for {key}, keeping it in the app database: {e}");
            remember(key, None);
            false
        }
    }
}

/// Move secrets still sitting in SQLite (from before the keychain, or written while it
/// was unavailable) into the keychain. A row is deleted only after its value reads back.
pub fn migrate(conn: &Connection) {
    if !available() {
        return;
    }
    for &key in KEYS {
        let row: Option<String> = conn
            .query_row("SELECT value FROM settings WHERE key=?1", params![key], |r| r.get(0))
            .optional()
            .ok()
            .flatten();
        let Some(value) = row else { continue };
        if value.is_empty() || (write(key, &value).is_ok() && read(key).ok().flatten().as_deref() == Some(value.as_str())) {
            let _ = conn.execute("DELETE FROM settings WHERE key=?1", params![key]);
            remember(key, (!value.is_empty()).then_some(value));
        }
    }
}

#[cfg(not(test))]
fn read(key: &str) -> keyring_core::Result<Option<String>> {
    match keyring_core::Entry::new(SERVICE, key)?.get_password() {
        Ok(v) => Ok(Some(v)),
        Err(keyring_core::Error::NoEntry) => Ok(None),
        Err(e) => Err(e),
    }
}

#[cfg(not(test))]
fn write(key: &str, value: &str) -> keyring_core::Result<()> {
    let entry = keyring_core::Entry::new(SERVICE, key)?;
    if value.is_empty() {
        return match entry.delete_credential() {
            Ok(()) | Err(keyring_core::Error::NoEntry) => Ok(()),
            Err(e) => Err(e),
        };
    }
    entry.set_password(value)
}

// Tests never touch the real keychain. Each test thread gets its own in-memory store
// (off until `use_test_store`), so the SQLite-only behaviour stays the default.
#[cfg(test)]
thread_local! {
    static TEST_STORE: std::cell::RefCell<Option<HashMap<String, String>>> =
        const { std::cell::RefCell::new(None) };
}

#[cfg(test)]
pub fn use_test_store() {
    TEST_STORE.with(|s| *s.borrow_mut() = Some(HashMap::new()));
}

#[cfg(test)]
fn read(key: &str) -> keyring_core::Result<Option<String>> {
    Ok(TEST_STORE.with(|s| s.borrow().as_ref().and_then(|m| m.get(key).cloned())))
}

#[cfg(test)]
fn write(key: &str, value: &str) -> keyring_core::Result<()> {
    TEST_STORE.with(|s| {
        let mut s = s.borrow_mut();
        let m = s.as_mut().expect("test store");
        if value.is_empty() {
            m.remove(key);
        } else {
            m.insert(key.to_string(), value.to_string());
        }
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::AppState;
    use crate::repo;

    fn sqlite_row(c: &Connection, key: &str) -> Option<String> {
        c.query_row("SELECT value FROM settings WHERE key=?1", params![key], |r| r.get(0))
            .optional()
            .unwrap()
    }

    #[test]
    fn secrets_go_to_the_keychain_and_never_reach_the_webview() {
        use_test_store();
        let st = AppState::in_memory().unwrap();
        let c = st.db.lock().unwrap();
        repo::set_setting(&c, "gemini_api_key", "AIzaREAL").unwrap();
        repo::set_setting(&c, "theme", "osaka-jade").unwrap();

        assert_eq!(sqlite_row(&c, "gemini_api_key"), None, "not stored in SQLite");
        assert_eq!(repo::get_setting(&c, "gemini_api_key").unwrap().as_deref(), Some("AIzaREAL"));
        assert_eq!(sqlite_row(&c, "theme").as_deref(), Some("osaka-jade"));

        let all = repo::all_settings(&c).unwrap();
        assert_eq!(all["gemini_api_key"], MASK);
        assert_eq!(all["theme"], "osaka-jade");
        assert!(all.get("claude_api_key").is_none(), "unset secrets are absent");

        // Clearing removes it everywhere.
        repo::set_setting(&c, "gemini_api_key", "").unwrap();
        assert_eq!(repo::get_setting(&c, "gemini_api_key").unwrap(), None);
    }

    #[test]
    fn without_a_keychain_secrets_stay_in_sqlite_but_stay_masked() {
        let st = AppState::in_memory().unwrap();
        let c = st.db.lock().unwrap();
        repo::set_setting(&c, "sync_pass", "pw").unwrap();
        assert_eq!(sqlite_row(&c, "sync_pass").as_deref(), Some("pw"));
        assert_eq!(repo::get_setting(&c, "sync_pass").unwrap().as_deref(), Some("pw"));
        assert_eq!(repo::all_settings(&c).unwrap()["sync_pass"], MASK);
    }

    #[test]
    fn migrate_moves_existing_rows_into_the_keychain() {
        let st = AppState::in_memory().unwrap();
        let c = st.db.lock().unwrap();
        // Written before the keychain existed.
        repo::set_setting(&c, "moodle_token", "tok123").unwrap();
        repo::set_setting(&c, "openai_api_key", "").unwrap();
        repo::set_setting(&c, "model_chat", "gemini:flash").unwrap();

        use_test_store();
        migrate(&c);
        assert_eq!(sqlite_row(&c, "moodle_token"), None);
        assert_eq!(sqlite_row(&c, "openai_api_key"), None);
        assert_eq!(sqlite_row(&c, "model_chat").as_deref(), Some("gemini:flash"));
        assert_eq!(repo::get_setting(&c, "moodle_token").unwrap().as_deref(), Some("tok123"));
    }

    /// Round-trips a throwaway entry through the real platform store.
    /// Run by hand: `cargo test --lib real_keychain -- --ignored`
    #[test]
    #[ignore]
    fn real_keychain_roundtrip() {
        platform_store().expect("platform store");
        let e = keyring_core::Entry::new("study.cortex.app.selftest", "probe").unwrap();
        e.set_password("roundtrip-ok").unwrap();
        assert_eq!(e.get_password().unwrap(), "roundtrip-ok");
        e.delete_credential().unwrap();
        assert!(matches!(e.get_password(), Err(keyring_core::Error::NoEntry)));
    }

    #[test]
    fn the_mask_sent_back_means_unchanged() {
        assert_eq!(from_input(MASK), None);
        assert_eq!(from_input("sk-new").as_deref(), Some("sk-new"));
        assert_eq!(from_input(&format!("{MASK}sk-new")).as_deref(), Some("sk-new"));
        assert_eq!(from_input("").as_deref(), Some(""));
    }
}
