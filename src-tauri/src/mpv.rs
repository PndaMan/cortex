//! Ad-free YouTube (and arbitrary URL) audio playback via a headless `mpv`
//! sidecar.
//!
//! Why mpv: it plays both normal videos and livestreams, handles the HLS that
//! YouTube livestreams resolve to, and — fed through `yt-dlp` — bypasses the
//! ad-insertion that an embedded YouTube player would show. We run ONE long-
//! lived `mpv --idle` process with no video output and drive it over its JSON
//! IPC socket (load a URL, pause/resume, set volume). The process outlives
//! individual commands so playback is continuous across the app.
//!
//! `yt-dlp` is auto-downloaded (standalone Linux binary) into the app data dir
//! on first use, so the user doesn't have to install it manually. `mpv` and
//! `ffmpeg` are expected on PATH (detected + surfaced in Settings).

use crate::error::{Error, Result};
use crate::models::MediaTools;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;
use tauri::{AppHandle, Manager};

/// The single long-lived mpv child process, if running.
fn mpv_holder() -> &'static Mutex<Option<Child>> {
    static MPV: OnceLock<Mutex<Option<Child>>> = OnceLock::new();
    MPV.get_or_init(|| Mutex::new(None))
}

/// Kill the mpv sidecar — called on app exit so no headless mpv process lingers
/// (on Linux a child isn't auto-reaped when the parent exits).
pub fn shutdown() {
    if let Ok(mut guard) = mpv_holder().lock() {
        if let Some(child) = guard.as_mut() {
            // mpv is spawned as its own process-group leader (see ensure_mpv), so
            // for a YouTube livestream the continuously-running `yt-dlp` feeder
            // shares mpv's group. SIGKILL the whole group so the downloader can't
            // linger (and keep streaming) after the app closes — killing mpv alone
            // would orphan yt-dlp.
            #[cfg(unix)]
            unsafe {
                libc::kill(-(child.id() as i32), libc::SIGKILL);
            }
            let _ = child.kill();
            let _ = child.wait();
        }
        *guard = None;
    }
}

fn data_dir(app: &AppHandle) -> Result<PathBuf> {
    app.path()
        .app_data_dir()
        .map_err(|e| Error::Other(e.to_string()))
}

/// Quit any mpv orphaned by a previous session. If the app crashed, shutdown()
/// never ran — the old mpv (own process group) keeps streaming forever, and a
/// fresh launch used to delete its socket and spawn a SECOND mpv next to it.
/// A live listener on the leftover socket can only be that orphan: tell it to
/// quit before clearing the socket. Called once at app startup.
pub fn cleanup_stale(app: &AppHandle) {
    let Ok(dir) = data_dir(app) else { return };
    let socket = socket_path(&dir);
    if socket.exists() {
        let _ = ipc(&socket, &json!({ "command": ["quit"] }));
        let _ = std::fs::remove_file(&socket);
    }
}

fn socket_path(dir: &Path) -> PathBuf {
    dir.join("mpv.sock")
}

/// Resolve `bin` against PATH, returning its absolute path if found.
fn find_on_path(bin: &str) -> Option<PathBuf> {
    let paths = std::env::var_os("PATH")?;
    std::env::split_paths(&paths)
        .map(|p| p.join(bin))
        .find(|p| p.is_file())
}

/// Ensure a usable `yt-dlp` exists, returning its path. Prefers an existing
/// install (downloaded copy, then PATH); otherwise downloads the standalone
/// Linux binary into `<data>/bin/yt-dlp`. Idempotent.
fn ensure_ytdlp(dir: &Path) -> Result<PathBuf> {
    let downloaded = dir.join("bin").join("yt-dlp");
    if downloaded.is_file() {
        return Ok(downloaded);
    }
    if let Some(p) = find_on_path("yt-dlp") {
        return Ok(p);
    }
    // Download the self-contained build (no system Python needed).
    let url = "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp_linux";
    std::fs::create_dir_all(downloaded.parent().unwrap())?;
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(180))
        .build()?;
    let bytes = client
        .get(url)
        .send()
        .map_err(|e| Error::Other(format!("yt-dlp download failed: {e}")))?
        .error_for_status()
        .map_err(|e| Error::Other(format!("yt-dlp download failed: {e}")))?
        .bytes()?;
    std::fs::write(&downloaded, &bytes)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&downloaded)?.permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&downloaded, perms)?;
    }
    Ok(downloaded)
}

/// Start mpv (idle, no video) if it isn't already running, listening on the IPC
/// socket and configured to find our yt-dlp. Waits briefly for the socket.
fn ensure_mpv(socket: &Path, ytdlp: &Path, volume: u8) -> Result<()> {
    let holder = mpv_holder();
    let mut guard = holder.lock().unwrap();
    if let Some(child) = guard.as_mut() {
        // Still alive? Reuse it.
        if matches!(child.try_wait(), Ok(None)) {
            return Ok(());
        }
        *guard = None;
    }
    // A leftover socket with a live listener is an orphan from a crashed
    // session — quit it so two mpvs never play at once, then clear the socket.
    if socket.exists() {
        let _ = ipc(socket, &json!({ "command": ["quit"] }));
        let _ = std::fs::remove_file(socket);
    }
    let mut cmd = Command::new("mpv");
    cmd.arg("--no-video")
        .arg("--idle=yes")
        .arg("--no-terminal")
        .arg("--really-quiet")
        .arg(format!("--volume={volume}"))
        .arg(format!("--input-ipc-server={}", socket.display()))
        .arg(format!(
            "--script-opts=ytdl_hook-ytdl_path={}",
            ytdlp.display()
        ));
    // Put mpv in its own process group so any `yt-dlp` feeder it spawns shares the
    // group and can be torn down together on shutdown() (see there).
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    let child = cmd.spawn().map_err(|e| {
        Error::Other(format!(
            "couldn't start mpv — install it (e.g. `sudo pacman -S mpv`): {e}"
        ))
    })?;
    *guard = Some(child);
    drop(guard);
    for _ in 0..60 {
        if socket.exists() {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    Ok(())
}

/// Send one JSON command line to mpv's IPC socket.
#[cfg(unix)]
fn ipc(socket: &Path, cmd: &Value) -> Result<()> {
    use std::io::Write;
    use std::os::unix::net::UnixStream;
    let mut stream = UnixStream::connect(socket)
        .map_err(|e| Error::Other(format!("mpv not reachable: {e}")))?;
    let mut line = serde_json::to_vec(cmd)?;
    line.push(b'\n');
    stream.write_all(&line)?;
    Ok(())
}

#[cfg(not(unix))]
fn ipc(_socket: &Path, _cmd: &Value) -> Result<()> {
    Err(Error::Unsupported(
        "URL audio streaming requires a Unix socket (Linux/macOS).".into(),
    ))
}

/// Best-effort IPC (used for pause/stop/volume): if mpv isn't running, there's
/// nothing to control, so a connect failure is silently ignored.
fn ipc_soft(socket: &Path, cmd: &Value) {
    if socket.exists() {
        let _ = ipc(socket, cmd);
    }
}

// ---- commands ----------------------------------------------------------

/// Detect the external tools the YouTube-audio engine relies on.
#[tauri::command]
pub fn media_tools_status(app: AppHandle) -> Result<MediaTools> {
    let dir = data_dir(&app)?;
    let downloaded = dir.join("bin").join("yt-dlp");
    Ok(MediaTools {
        mpv: find_on_path("mpv").is_some(),
        ffmpeg: find_on_path("ffmpeg").is_some(),
        ytdlp: downloaded.is_file() || find_on_path("yt-dlp").is_some(),
        ytdlp_path: downloaded.display().to_string(),
    })
}

/// Stream a URL's audio (YouTube video/livestream or any mpv-playable URL),
/// ad-free, starting playback at `volume` (0–100). Downloads yt-dlp on first use.
#[tauri::command]
pub fn youtube_play(app: AppHandle, url: String, volume: u8) -> Result<()> {
    let dir = data_dir(&app)?;
    let socket = socket_path(&dir);
    let ytdlp = ensure_ytdlp(&dir)?;
    ensure_mpv(&socket, &ytdlp, volume)?;
    // `loadfile … replace` swaps whatever's playing for the new URL.
    ipc(&socket, &json!({ "command": ["loadfile", url, "replace"] }))?;
    ipc(&socket, &json!({ "command": ["set_property", "pause", false] }))?;
    ipc(
        &socket,
        &json!({ "command": ["set_property", "volume", volume as i64] }),
    )?;
    Ok(())
}

#[tauri::command]
pub fn youtube_pause(app: AppHandle) -> Result<()> {
    let dir = data_dir(&app)?;
    ipc_soft(&socket_path(&dir), &json!({ "command": ["set_property", "pause", true] }));
    Ok(())
}

#[tauri::command]
pub fn youtube_resume(app: AppHandle) -> Result<()> {
    let dir = data_dir(&app)?;
    ipc_soft(&socket_path(&dir), &json!({ "command": ["set_property", "pause", false] }));
    Ok(())
}

/// Stop playback but keep mpv idle (ready for the next station).
#[tauri::command]
pub fn youtube_stop(app: AppHandle) -> Result<()> {
    let dir = data_dir(&app)?;
    ipc_soft(&socket_path(&dir), &json!({ "command": ["stop"] }));
    Ok(())
}

#[tauri::command]
pub fn youtube_set_volume(app: AppHandle, volume: u8) -> Result<()> {
    let dir = data_dir(&app)?;
    ipc_soft(
        &socket_path(&dir),
        &json!({ "command": ["set_property", "volume", volume as i64] }),
    );
    Ok(())
}
