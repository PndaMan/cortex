//! Native iOS bridge for Cortex.
//!
//! Exposes the background lecture recorder (AVAudioSession + Live Activity, implemented in the
//! `CortexShared` Swift package) and widget-snapshot writing to the JS frontend. Everything real
//! lives in Swift; this crate is the thin Tauri plugin that routes `invoke('plugin:cortex-ios|…')`
//! calls to the registered iOS plugin. On non-iOS targets the commands compile but return an
//! "iOS only" error (the frontend only calls them on iOS), so the desktop build is unaffected.

use serde::{de::DeserializeOwned, Serialize};
use tauri::{
    plugin::{Builder, TauriPlugin},
    AppHandle, Runtime,
};
#[cfg(target_os = "ios")]
use tauri::Manager;

mod models;
pub use models::*;

#[cfg(target_os = "ios")]
tauri::ios_plugin_binding!(init_plugin_cortex_ios);

// ── error ───────────────────────────────────────────────────────────────────

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Tauri(#[from] tauri::Error),
    #[error("{0}")]
    Msg(String),
}

impl Serialize for Error {
    fn serialize<S: serde::Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

pub type Result<T> = std::result::Result<T, Error>;

// ── iOS plugin handle (managed state) ────────────────────────────────────────

#[cfg(target_os = "ios")]
struct CortexIos<R: Runtime>(tauri::plugin::PluginHandle<R>);

#[cfg(target_os = "ios")]
fn forward<R: Runtime, T: Serialize, U: DeserializeOwned>(
    app: &AppHandle<R>,
    cmd: &str,
    payload: T,
) -> Result<U> {
    let state = app.state::<CortexIos<R>>();
    // run_mobile_plugin returns PluginInvokeError (no From impl on our Error) — stringify it.
    state
        .0
        .run_mobile_plugin(cmd, payload)
        .map_err(|e| Error::Msg(e.to_string()))
}

#[cfg(not(target_os = "ios"))]
fn forward<R: Runtime, T: Serialize, U: DeserializeOwned>(
    _app: &AppHandle<R>,
    _cmd: &str,
    _payload: T,
) -> Result<U> {
    Err(Error::Msg("cortex-ios is only available on iOS".into()))
}

// ── commands (forwarded to Swift) ────────────────────────────────────────────

#[tauri::command]
async fn start_recording<R: Runtime>(app: AppHandle<R>, args: StartArgs) -> Result<RecordingPath> {
    forward(&app, "startRecording", args)
}

#[tauri::command]
async fn stop_recording<R: Runtime>(app: AppHandle<R>) -> Result<RecordingPath> {
    forward(&app, "stopRecording", ())
}

#[tauri::command]
async fn pause_recording<R: Runtime>(app: AppHandle<R>) -> Result<Ok> {
    forward(&app, "pauseRecording", ())
}

#[tauri::command]
async fn resume_recording<R: Runtime>(app: AppHandle<R>) -> Result<Ok> {
    forward(&app, "resumeRecording", ())
}

#[tauri::command]
async fn recording_state<R: Runtime>(app: AppHandle<R>) -> Result<RecordingStatus> {
    forward(&app, "recordingState", ())
}

#[tauri::command]
async fn read_recording_bytes<R: Runtime>(app: AppHandle<R>, args: PathArgs) -> Result<Bytes> {
    forward(&app, "readRecordingBytes", args)
}

#[tauri::command]
async fn list_inbox<R: Runtime>(app: AppHandle<R>) -> Result<InboxList> {
    forward(&app, "listInbox", ())
}

#[tauri::command]
async fn delete_recording<R: Runtime>(app: AppHandle<R>, args: PathArgs) -> Result<Ok> {
    forward(&app, "deleteRecording", args)
}

#[tauri::command]
async fn set_widget_snapshot<R: Runtime>(app: AppHandle<R>, args: SnapshotArgs) -> Result<Ok> {
    forward(&app, "setWidgetSnapshot", args)
}

#[tauri::command]
async fn mic_permission_status<R: Runtime>(app: AppHandle<R>) -> Result<MicStatus> {
    forward(&app, "micPermissionStatus", ())
}

#[tauri::command]
async fn request_mic_permission<R: Runtime>(app: AppHandle<R>) -> Result<MicPermission> {
    forward(&app, "requestMicPermission", ())
}

#[tauri::command]
async fn open_app_settings<R: Runtime>(app: AppHandle<R>) -> Result<Ok> {
    forward(&app, "openAppSettings", ())
}

// ── plugin init ──────────────────────────────────────────────────────────────

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("cortex-ios")
        .invoke_handler(tauri::generate_handler![
            start_recording,
            stop_recording,
            pause_recording,
            resume_recording,
            recording_state,
            read_recording_bytes,
            list_inbox,
            delete_recording,
            set_widget_snapshot,
            mic_permission_status,
            request_mic_permission,
            open_app_settings
        ])
        .setup(|_app, _api| {
            #[cfg(target_os = "ios")]
            {
                let handle = _api.register_ios_plugin(init_plugin_cortex_ios)?;
                _app.manage(CortexIos(handle));
            }
            Ok(())
        })
        .build()
}
