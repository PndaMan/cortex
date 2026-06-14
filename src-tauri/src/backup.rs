//! Encrypted homelab backups: snapshot the SQLite DB, encrypt it with `age`
//! (recipient public key from settings), then upload via `rclone` to a configured
//! remote. Both are external CLIs the user installs; we detect them and surface
//! clear, actionable errors when they're missing or unconfigured. Nothing is ever
//! uploaded unencrypted.

use crate::db::{now_ms, AppState};
use crate::error::{Error, Result};
use crate::repo;
use serde::Serialize;
use std::process::Command;
use tauri::{AppHandle, Manager};

const KEY_RECIPIENT: &str = "backup_age_recipient";
const KEY_REMOTE: &str = "backup_rclone_remote";
const KEY_LAST_AT: &str = "backup_last_at";
const KEY_LAST_DEST: &str = "backup_last_dest";

#[derive(Serialize)]
pub struct BackupStatus {
    /// `age` binary is on PATH.
    pub age_found: bool,
    /// `rclone` binary is on PATH.
    pub rclone_found: bool,
    /// An age recipient public key is configured.
    pub recipient_set: bool,
    /// An rclone remote path is configured.
    pub remote_set: bool,
    /// Epoch-ms of the last successful backup, if any.
    pub last_at: Option<i64>,
    /// Remote destination of the last successful backup, if any.
    pub last_dest: Option<String>,
}

/// True if `bin --version` (or `--help` for rclone) spawns successfully.
fn tool_exists(bin: &str) -> bool {
    crate::ingest::tool(bin).is_some()
}

/// Resolve a tool's invocation path: bundled sidecar, then PATH, else the bare
/// name (so the error message still mentions the tool).
fn bin(name: &str) -> String {
    crate::ingest::tool(name).unwrap_or_else(|| name.to_string())
}

#[tauri::command]
pub fn backup_status(state: tauri::State<AppState>) -> Result<BackupStatus> {
    let c = state.db.lock().unwrap();
    let recipient = repo::get_setting(&c, KEY_RECIPIENT)?.unwrap_or_default();
    let remote = repo::get_setting(&c, KEY_REMOTE)?.unwrap_or_default();
    let last_at = repo::get_setting(&c, KEY_LAST_AT)?.and_then(|s| s.parse::<i64>().ok());
    let last_dest = repo::get_setting(&c, KEY_LAST_DEST)?;
    Ok(BackupStatus {
        age_found: tool_exists("age"),
        rclone_found: tool_exists("rclone"),
        recipient_set: !recipient.trim().is_empty(),
        remote_set: !remote.trim().is_empty(),
        last_at,
        last_dest,
    })
}

/// Snapshot → encrypt with age → upload with rclone. Returns the remote path.
#[tauri::command]
pub async fn backup_now(app: AppHandle) -> Result<String> {
    tauri::async_runtime::spawn_blocking(move || -> Result<String> {
        let state = app.state::<AppState>();
        let (recipient, remote, db_path) = {
            let c = state.db.lock().unwrap();
            let recipient = repo::get_setting(&c, KEY_RECIPIENT)?.unwrap_or_default();
            let remote = repo::get_setting(&c, KEY_REMOTE)?.unwrap_or_default();
            // Checkpoint the WAL so the file copy is complete & self-contained.
            let _ = c.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);");
            (recipient, remote, db_file_path(&app)?)
        };

        let recipient = recipient.trim();
        let remote = remote.trim().trim_end_matches('/');
        if recipient.is_empty() {
            return Err(Error::Other(
                "No age recipient set. Paste your age public key (age1…) in Settings → Backups.".into(),
            ));
        }
        if remote.is_empty() {
            return Err(Error::Other(
                "No rclone remote set. Enter a remote like \"homelab:cortex\" in Settings → Backups.".into(),
            ));
        }
        if !tool_exists("age") {
            return Err(Error::Other(
                "`age` is not installed — it encrypts the backup. Install it (https://github.com/FiloSottile/age) and retry.".into(),
            ));
        }
        if !tool_exists("rclone") {
            return Err(Error::Other(
                "`rclone` is not installed — it uploads the backup. Install it (https://rclone.org) and retry.".into(),
            ));
        }

        let ts = now_ms();
        let stamp = stamp(ts);
        let tmp_db = std::env::temp_dir().join(format!("cortex-backup-{ts}.db"));
        let tmp_enc = std::env::temp_dir().join(format!("cortex-backup-{ts}.db.age"));
        // Always clean up temp files, even on early return.
        let _guard = TempGuard(vec![tmp_db.clone(), tmp_enc.clone()]);

        std::fs::copy(&db_path, &tmp_db).map_err(Error::Io)?;

        // Encrypt: age -r <recipient> -o <enc> <db>
        run(
            Command::new(bin("age"))
                .arg("-r")
                .arg(recipient)
                .arg("-o")
                .arg(&tmp_enc)
                .arg(&tmp_db),
            "age",
        )?;

        // Upload: rclone copyto <enc> <remote>/cortex-<stamp>.db.age
        let dest = format!("{remote}/cortex-{stamp}.db.age");
        run(
            Command::new(bin("rclone")).arg("copyto").arg(&tmp_enc).arg(&dest),
            "rclone",
        )?;

        {
            let c = state.db.lock().unwrap();
            repo::set_setting(&c, KEY_LAST_AT, &ts.to_string())?;
            repo::set_setting(&c, KEY_LAST_DEST, &dest)?;
        }
        Ok(dest)
    })
    .await
    .map_err(|e| Error::Other(format!("backup task failed: {e}")))?
}

/// Run a command, turning a non-zero exit into a readable error with stderr.
fn run(cmd: &mut Command, label: &str) -> Result<()> {
    let out = cmd
        .output()
        .map_err(|e| Error::Other(format!("failed to run {label}: {e}")))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        let err = err.trim();
        return Err(Error::Other(format!(
            "{label} failed: {}",
            if err.is_empty() { "unknown error" } else { err }
        )));
    }
    Ok(())
}

fn db_file_path(app: &AppHandle) -> Result<std::path::PathBuf> {
    Ok(app
        .path()
        .app_data_dir()
        .map_err(|e| Error::Other(e.to_string()))?
        .join("cortex.db"))
}

/// UTC `YYYYMMDD-HHMMSS` stamp from epoch ms (no chrono dependency).
fn stamp(ms: i64) -> String {
    let secs = ms / 1000;
    let days = secs / 86_400;
    let tod = secs % 86_400;
    let (h, mi, s) = (tod / 3600, (tod % 3600) / 60, tod % 60);
    // Civil-from-days (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}{m:02}{d:02}-{h:02}{mi:02}{s:02}")
}

/// Deletes the listed temp files when dropped.
struct TempGuard(Vec<std::path::PathBuf>);
impl Drop for TempGuard {
    fn drop(&mut self) {
        for p in &self.0 {
            let _ = std::fs::remove_file(p);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stamp_formats_known_epoch() {
        // 2026-06-04T14:37:00Z = 1780583820 s.
        assert_eq!(stamp(1_780_583_820_000), "20260604-143700");
        // Unix epoch.
        assert_eq!(stamp(0), "19700101-000000");
    }
}
