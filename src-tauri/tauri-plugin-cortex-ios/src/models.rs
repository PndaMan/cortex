use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartArgs {
    /// Subject the lecture belongs to (shown on the Live Activity).
    #[serde(default)]
    pub subject: Option<String>,
    /// Accent colour hex (so the Live Activity matches the in-app theme).
    #[serde(default)]
    pub accent: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PathArgs {
    pub path: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotArgs {
    /// The full widget snapshot, already serialized to JSON on the JS side.
    pub json: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordingPath {
    /// Filesystem path of the recording (empty if none).
    #[serde(default)]
    pub path: String,
    /// Captured duration in seconds.
    #[serde(default)]
    pub duration_sec: f64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordingStatus {
    pub is_recording: bool,
    pub is_paused: bool,
    #[serde(default)]
    pub elapsed: f64,
    #[serde(default)]
    pub file_name: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Bytes {
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InboxFile {
    pub path: String,
    pub name: String,
    #[serde(default)]
    pub subject: Option<String>,
    #[serde(default)]
    pub size: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InboxList {
    pub files: Vec<InboxFile>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Ok {
    pub ok: bool,
}
