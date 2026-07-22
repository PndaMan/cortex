//! Background transcription queue.
//!
//! Saving or re-ingesting a lecture used to run the WHOLE transcribe→chunk→embed
//! pipeline inside the invoke — the UI sat on "transcribing"/"generating" for as
//! long as Whisper took (tens of minutes for a long lecture on CPU, or a
//! first-time model pull), and looked hung. Now the command persists the audio,
//! marks the source `ingesting`, enqueues it here and returns immediately; a
//! dedicated worker thread drains the queue one job at a time:
//!
//!   • transcription happens wherever Settings → Transcription points it —
//!     offloaded to the homelab / a cloud API, or on this machine;
//!   • the worker holds a system SLEEP INHIBITOR while jobs run, so a locked or
//!     idle computer keeps transcribing instead of dozing off mid-lecture
//!     (macOS: caffeinate · Linux: systemd-inhibit · Windows:
//!     SetThreadExecutionState);
//!   • progress streams over the existing `ingest:progress` events and a
//!     desktop notification fires when a lecture is ready;
//!   • jobs are DURABLE: sources still `ingesting` at startup are re-enqueued
//!     (resume_pending), so a crash/quit mid-transcription resumes instead of
//!     sticking forever — the old "stuck on generating source" failure mode.

use std::collections::VecDeque;
use std::sync::{Condvar, Mutex};
use tauri::AppHandle;
use tauri::Manager;

static QUEUE: Mutex<VecDeque<String>> = Mutex::new(VecDeque::new());
static WAKE: Condvar = Condvar::new();
static WORKER_STARTED: Mutex<bool> = Mutex::new(false);

/// Queue a source (already persisted, status `ingesting`) for transcription.
pub fn enqueue(app: &AppHandle, source_id: String) {
    {
        let mut q = QUEUE.lock().unwrap();
        if !q.contains(&source_id) {
            q.push_back(source_id);
        }
    }
    ensure_worker(app);
    WAKE.notify_one();
}

/// Re-enqueue every audio source stranded in `ingesting` — crash/quit recovery.
/// Called once at startup (off the critical path).
pub fn resume_pending(app: &AppHandle) {
    let ids: Vec<String> = {
        let Some(state) = app.try_state::<crate::db::AppState>() else { return };
        let Ok(c) = state.db.lock() else { return };
        let Ok(mut stmt) =
            c.prepare("SELECT id FROM sources WHERE status='ingesting' AND kind='audio'")
        else {
            return;
        };
        stmt.query_map([], |r| r.get::<_, String>(0))
            .map(|rows| rows.flatten().collect())
            .unwrap_or_default()
    };
    for id in ids {
        eprintln!("[asr] resuming interrupted transcription for source {id}");
        enqueue(app, id);
    }
}

fn ensure_worker(app: &AppHandle) {
    let mut started = WORKER_STARTED.lock().unwrap();
    if *started {
        return;
    }
    *started = true;
    let app = app.clone();
    let _ = std::thread::Builder::new()
        .name("asr-worker".into())
        .spawn(move || worker(app));
}

fn worker(app: AppHandle) {
    loop {
        let id = {
            let mut q = QUEUE.lock().unwrap();
            loop {
                if let Some(id) = q.pop_front() {
                    break id;
                }
                // Queue drained: let the machine sleep again, then park.
                inhibitor::release();
                q = WAKE.wait(q).unwrap();
            }
        };
        inhibitor::hold();
        // A panicking job (poisoned lock, bad audio) must not kill the worker —
        // the queue outlives any single lecture.
        let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            crate::commands::run_transcription_job(&app, &id);
        }));
        if res.is_err() {
            eprintln!("[asr] transcription job for {id} panicked");
        }
    }
}

/// Keep the machine awake while lectures transcribe. Lock screens never stop a
/// running process — SLEEP does; these are the per-OS "stay awake" affordances.
mod inhibitor {
    use std::sync::Mutex;

    #[allow(dead_code)]
    static CHILD: Mutex<Option<std::process::Child>> = Mutex::new(None);

    pub fn hold() {
        #[cfg(target_os = "macos")]
        {
            let mut c = CHILD.lock().unwrap();
            if c.is_none() {
                // -i: prevent idle system sleep while we run.
                *c = std::process::Command::new("caffeinate").arg("-i").spawn().ok();
            }
        }
        #[cfg(target_os = "linux")]
        {
            let mut c = CHILD.lock().unwrap();
            if c.is_none() {
                // Best-effort: no systemd → no inhibitor, transcription still runs.
                *c = std::process::Command::new("systemd-inhibit")
                    .args([
                        "--what=sleep:idle",
                        "--who=Cortex",
                        "--why=Transcribing lecture audio",
                        "sleep",
                        "infinity",
                    ])
                    .spawn()
                    .ok();
            }
        }
        #[cfg(target_os = "windows")]
        unsafe {
            use windows_sys::Win32::System::Power::{
                SetThreadExecutionState, ES_CONTINUOUS, ES_SYSTEM_REQUIRED,
            };
            // Applies to THIS thread — the worker thread is the one that must
            // keep the system required, and it calls hold() before every job.
            SetThreadExecutionState(ES_CONTINUOUS | ES_SYSTEM_REQUIRED);
        }
    }

    pub fn release() {
        #[cfg(any(target_os = "macos", target_os = "linux"))]
        {
            if let Some(mut child) = CHILD.lock().unwrap().take() {
                let _ = child.kill();
                let _ = child.wait();
            }
        }
        #[cfg(target_os = "windows")]
        unsafe {
            use windows_sys::Win32::System::Power::{SetThreadExecutionState, ES_CONTINUOUS};
            SetThreadExecutionState(ES_CONTINUOUS);
        }
    }
}
