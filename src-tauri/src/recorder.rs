//! Native lecture-recording backend.
//!
//! On iOS the webview CANNOT capture the microphone: Tauri serves the app from a
//! custom URL scheme, which WKWebView does not treat as a secure context, so
//! `navigator.mediaDevices` never exists — no amount of Info.plist permissions
//! fixes that. Capture therefore runs natively through AVAudioRecorder behind
//! these commands (AAC/.m4a straight to a temp file, level metering for the UI
//! waveform). Combined with `UIBackgroundModes: audio` (stamped into the iOS
//! Info.plist by CI) and the PlayAndRecord audio session, recording keeps
//! running while the app is backgrounded or the phone is locked.
//!
//! On desktop/Android these commands answer with a clear "not supported here" —
//! the web engine (getUserMedia/MediaRecorder) owns those platforms.

use crate::error::{Error, Result};
use tauri::AppHandle;

/// Result of a stopped native recording: where the audio landed + its length.
#[derive(serde::Serialize)]
pub struct NativeRecording {
    pub path: String,
    pub secs: f64,
}

/// One metering sample while a native recording runs.
#[derive(serde::Serialize)]
pub struct NativeMeter {
    /// Input level 0..1 for the waveform.
    pub level: f32,
    /// Elapsed recording time in seconds (authoritative — survives lock).
    pub secs: f64,
}

/// Directory native recordings are written to (inside the app sandbox's tmp).
fn rec_dir() -> std::path::PathBuf {
    std::env::temp_dir().join("cortex-native-rec")
}

/// Guard: only paths produced by the native recorder may be consumed/deleted by
/// the path-based commands, so the webview can't point them at arbitrary files.
fn assert_native_rec_path(path: &str) -> Result<std::path::PathBuf> {
    let p = std::path::Path::new(path);
    let dir = rec_dir().canonicalize().map_err(|e| Error::Other(format!("recorder dir missing: {e}")))?;
    let canon = p
        .canonicalize()
        .map_err(|e| Error::Other(format!("recording not found at {path}: {e}")))?;
    if !canon.starts_with(&dir) {
        return Err(Error::Other("not a native recording file".into()));
    }
    Ok(canon)
}

/// Save a lecture recording that already lives in a backend file (the native
/// iOS capture path — audio never crosses the JS bridge). Reads the file, runs
/// the exact same persist→transcribe→chunk→embed pipeline as `save_recording`,
/// then removes the temp file.
#[tauri::command]
pub async fn save_recording_path(
    app: AppHandle,
    subject_id: String,
    topic_id: Option<String>,
    name: String,
    path: String,
) -> Result<crate::models::IngestResult> {
    let canon = assert_native_rec_path(&path)?;
    let audio = std::fs::read(&canon)?;
    let ext = canon
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_string());
    let res = crate::commands::save_recording(app, subject_id, topic_id, name, audio, ext).await;
    if res.is_ok() {
        let _ = std::fs::remove_file(&canon);
    }
    res
}

/// Delete a stopped-but-unsaved native recording (the user discarded the take).
#[tauri::command]
pub fn native_rec_discard(path: String) -> Result<()> {
    let canon = assert_native_rec_path(&path)?;
    std::fs::remove_file(&canon)?;
    Ok(())
}

// ─────────────────────────── iOS implementation ───────────────────────────
#[cfg(target_os = "ios")]
mod ios {
    use super::*;
    use block2::RcBlock;
    use objc2::rc::Retained;
    use objc2::runtime::AnyObject;
    use objc2::runtime::Bool;
    use objc2::AnyThread;
    use objc2_avf_audio::{
        AVAudioRecorder, AVAudioSession, AVAudioSessionCategoryOptions,
        AVAudioSessionCategoryPlayAndRecord, AVEncoderBitRateKey, AVFormatIDKey,
        AVNumberOfChannelsKey, AVSampleRateKey,
    };
    use objc2_foundation::{NSDictionary, NSNumber, NSString, NSURL};
    use std::sync::mpsc;
    use std::sync::Mutex;

    /// kAudioFormatMPEG4AAC — fourcc 'aac ' (CoreAudioTypes).
    const K_AUDIO_FORMAT_MPEG4_AAC: u32 = 0x6161_6320;

    /// The one active recorder. AVAudioRecorder is safe to drive from any thread
    /// (its API is thread-agnostic); the Retained pointer just isn't marked Send,
    /// hence the wrapper. All access goes through the mutex.
    struct Handle {
        rec: Retained<AVAudioRecorder>,
        path: std::path::PathBuf,
    }
    unsafe impl Send for Handle {}
    static ACTIVE: Mutex<Option<Handle>> = Mutex::new(None);

    /// Ask for (or confirm) mic permission. Blocks the command thread until the
    /// user answers the system prompt — first call shows the iOS mic dialog.
    fn ensure_permission(session: &AVAudioSession) -> Result<()> {
        let (tx, rx) = mpsc::channel::<bool>();
        let block = RcBlock::new(move |granted: Bool| {
            let _ = tx.send(granted.as_bool());
        });
        unsafe { session.requestRecordPermission(&block) };
        match rx.recv_timeout(std::time::Duration::from_secs(120)) {
            Ok(true) => Ok(()),
            Ok(false) => Err(Error::Other(
                "Microphone access is denied. Enable it in Settings → Cortex → Microphone, then try again.".into(),
            )),
            Err(_) => Err(Error::Other("Timed out waiting for microphone permission.".into())),
        }
    }

    pub fn start() -> Result<()> {
        // Cheap early check WITHOUT holding the lock across the permission wait —
        // ensure_permission can block for minutes on the system dialog, and any
        // concurrent native_rec_* call must not hang on the mutex meanwhile.
        if ACTIVE.lock().unwrap().is_some() {
            return Err(Error::Other("A recording is already running.".into()));
        }
        unsafe {
            let session = AVAudioSession::sharedInstance();
            ensure_permission(&session)?;
            // PlayAndRecord (+ MixWithOthers) so recording coexists with any app
            // audio and — with UIBackgroundModes:audio — survives lock/background.
            session
                .setCategory_withOptions_error(
                    AVAudioSessionCategoryPlayAndRecord,
                    AVAudioSessionCategoryOptions::MixWithOthers
                        | AVAudioSessionCategoryOptions::AllowBluetoothHFP,
                )
                .map_err(|e| Error::Other(format!("audio session category: {e}")))?;
            session
                .setActive_error(true)
                .map_err(|e| Error::Other(format!("audio session activate: {e}")))?;

            let dir = rec_dir();
            std::fs::create_dir_all(&dir)?;
            let path = dir.join(format!("rec-{}.m4a", crate::db::new_id()));
            let url = NSURL::fileURLWithPath(&NSString::from_str(&path.to_string_lossy()));

            // AAC mono @44.1kHz / 64kbps — ~12 MB per 25-min lecture, decodes
            // everywhere (ffmpeg, PyAV, speaches).
            let keys: [&NSString; 4] = [
                AVFormatIDKey,
                AVSampleRateKey,
                AVNumberOfChannelsKey,
                AVEncoderBitRateKey,
            ];
            let format = NSNumber::new_u32(K_AUDIO_FORMAT_MPEG4_AAC);
            let rate = NSNumber::new_f64(44_100.0);
            let channels = NSNumber::new_u32(1);
            let bitrate = NSNumber::new_u32(64_000);
            let values: [&AnyObject; 4] = [
                format.as_ref(),
                rate.as_ref(),
                channels.as_ref(),
                bitrate.as_ref(),
            ];
            let settings: Retained<NSDictionary<NSString, AnyObject>> =
                NSDictionary::from_slices(&keys, &values);

            let rec = AVAudioRecorder::initWithURL_settings_error(
                AVAudioRecorder::alloc(),
                &url,
                &settings,
            )
            .map_err(|e| Error::Other(format!("couldn't create the recorder: {e}")))?;
            rec.setMeteringEnabled(true);
            if !rec.record() {
                return Err(Error::Other(
                    "The recorder failed to start — is another app holding the microphone?".into(),
                ));
            }
            // Re-take the lock only for the insert; re-check in case a racing
            // start() won while we waited on the permission dialog.
            let mut active = ACTIVE.lock().unwrap();
            if active.is_some() {
                rec.stop();
                let _ = rec.deleteRecording();
                return Err(Error::Other("A recording is already running.".into()));
            }
            *active = Some(Handle { rec, path });
        }
        Ok(())
    }

    pub fn pause() -> Result<()> {
        let active = ACTIVE.lock().unwrap();
        let h = active.as_ref().ok_or_else(|| Error::Other("no active recording".into()))?;
        unsafe { h.rec.pause() };
        Ok(())
    }

    pub fn resume() -> Result<()> {
        let active = ACTIVE.lock().unwrap();
        let h = active.as_ref().ok_or_else(|| Error::Other("no active recording".into()))?;
        unsafe {
            if !h.rec.record() {
                return Err(Error::Other("couldn't resume the recording".into()));
            }
        }
        Ok(())
    }

    pub fn stop() -> Result<NativeRecording> {
        let mut active = ACTIVE.lock().unwrap();
        let h = active.take().ok_or_else(|| Error::Other("no active recording".into()))?;
        let secs = unsafe {
            let secs = h.rec.currentTime(); // must be read BEFORE stop() finalizes
            h.rec.stop();
            let _ = AVAudioSession::sharedInstance().setActive_error(false);
            secs
        };
        if !h.path.is_file() {
            return Err(Error::Other("the recording file was not written".into()));
        }
        Ok(NativeRecording { path: h.path.to_string_lossy().into_owned(), secs })
    }

    pub fn cancel() -> Result<()> {
        let mut active = ACTIVE.lock().unwrap();
        if let Some(h) = active.take() {
            unsafe {
                h.rec.stop();
                let _ = h.rec.deleteRecording();
                let _ = AVAudioSession::sharedInstance().setActive_error(false);
            }
            let _ = std::fs::remove_file(&h.path);
        }
        Ok(())
    }

    pub fn meter() -> Result<super::NativeMeter> {
        let active = ACTIVE.lock().unwrap();
        let h = active.as_ref().ok_or_else(|| Error::Other("no active recording".into()))?;
        let (db, secs) = unsafe {
            h.rec.updateMeters();
            (h.rec.averagePowerForChannel(0), h.rec.currentTime())
        };
        Ok(super::NativeMeter {
            // dBFS (-160..0) → linear 0..1.
            level: 10f32.powf(db / 20.0).clamp(0.0, 1.0),
            // Authoritative elapsed time: webview JS timers freeze while the phone
            // is locked, so the UI clock resyncs from here.
            secs,
        })
    }
}

// ───────────────────────────── commands ─────────────────────────────
// Thin cross-platform wrappers: real work on iOS, honest errors elsewhere.

#[cfg(not(target_os = "ios"))]
fn unsupported<T>() -> Result<T> {
    Err(Error::Unsupported(
        "native recording only exists on iOS — this platform records in the webview".into(),
    ))
}

// async: start blocks waiting for the user to answer the mic-permission dialog
// (up to two minutes) — a sync command would freeze the webview for the wait,
// and could deadlock if the permission callback needs the main run loop.
#[tauri::command]
pub async fn native_rec_start() -> Result<()> {
    #[cfg(target_os = "ios")]
    return tauri::async_runtime::spawn_blocking(ios::start)
        .await
        .map_err(|e| Error::Other(format!("recorder task failed: {e}")))?;
    #[cfg(not(target_os = "ios"))]
    unsupported()
}

#[tauri::command]
pub fn native_rec_pause() -> Result<()> {
    #[cfg(target_os = "ios")]
    return ios::pause();
    #[cfg(not(target_os = "ios"))]
    unsupported()
}

#[tauri::command]
pub fn native_rec_resume() -> Result<()> {
    #[cfg(target_os = "ios")]
    return ios::resume();
    #[cfg(not(target_os = "ios"))]
    unsupported()
}

#[tauri::command]
pub fn native_rec_stop() -> Result<NativeRecording> {
    #[cfg(target_os = "ios")]
    return ios::stop();
    #[cfg(not(target_os = "ios"))]
    unsupported()
}

#[tauri::command]
pub fn native_rec_cancel() -> Result<()> {
    #[cfg(target_os = "ios")]
    return ios::cancel();
    #[cfg(not(target_os = "ios"))]
    unsupported()
}

#[tauri::command]
pub fn native_rec_level() -> Result<NativeMeter> {
    #[cfg(target_os = "ios")]
    return ios::meter();
    #[cfg(not(target_os = "ios"))]
    unsupported()
}
