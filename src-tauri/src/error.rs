use serde::Serialize;

/// Unified error type for Cortex backend. Serializes to a plain string so the
/// frontend receives a readable message from a failed `invoke`.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("database error: {0}")]
    Db(#[from] rusqlite::Error),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("network error: {0}")]
    Http(reqwest::Error),

    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("not found: {0}")]
    NotFound(String),

    #[error("unsupported: {0}")]
    Unsupported(String),

    #[error("{0}")]
    Other(String),
}

/// reqwest errors print the request URL. Strip its query and `user:pass@` first so a
/// key or token carried there never lands in a message that is shown, logged or stored.
impl From<reqwest::Error> for Error {
    fn from(e: reqwest::Error) -> Self {
        Error::Http(redact(e))
    }
}

/// Use before formatting a reqwest error anywhere it doesn't go through `Error`.
pub fn redact(mut e: reqwest::Error) -> reqwest::Error {
    if let Some(url) = e.url_mut() {
        url.set_query(None);
        let _ = url.set_username("");
        let _ = url.set_password(None);
    }
    e
}

impl Serialize for Error {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    #[test]
    fn http_errors_never_print_query_or_credentials() {
        // Port 1 on loopback refuses at once, so this fails without touching the network.
        let e = reqwest::blocking::get("http://cortex:hunter2@127.0.0.1:1/asr?key=AIzaSECRET")
            .unwrap_err();
        assert!(e.to_string().contains("AIzaSECRET"), "premise: reqwest prints the URL");
        let msg = super::Error::from(e).to_string();
        assert!(msg.contains("127.0.0.1"), "keeps the host for diagnosis: {msg}");
        assert!(!msg.contains("AIzaSECRET") && !msg.contains("hunter2"), "leaked: {msg}");
    }
}
