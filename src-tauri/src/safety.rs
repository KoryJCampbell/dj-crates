// Safety rails for the one-button sync pipeline.
//
// - Serato-running guard: refuse to rewrite crates/db while Serato DJ Pro is
//   open (Serato rewrites its db on quit and would clobber ours — or worse).
// - Pipeline lock: one pipeline at a time across UI button, dj-daily and
//   dj-weekly. Stale locks (dead pid) are broken automatically.
// - Pre-sync snapshots: zip Subcrates + database V2 before any destructive
//   write, rotating the newest SNAPSHOT_KEEP.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const SNAPSHOT_KEEP: usize = 14;

fn state_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".dj-crates-tools")
}

/// True if Serato DJ Pro (or Lite) is currently running.
pub fn serato_running() -> bool {
    for name in ["Serato DJ Pro", "Serato DJ Lite"] {
        if let Ok(out) = Command::new("/usr/bin/pgrep").arg("-x").arg(name).output() {
            if out.status.success() {
                return true;
            }
        }
    }
    false
}

#[derive(serde::Serialize, serde::Deserialize)]
struct LockInfo {
    pid: u32,
    stage: String,
    started_at: String,
}

/// Held for the duration of a pipeline run; the lock file is removed on drop.
pub struct PipelineLock {
    path: PathBuf,
}

impl PipelineLock {
    pub fn acquire(stage: &str) -> Result<PipelineLock, String> {
        let dir = state_dir();
        let _ = fs::create_dir_all(&dir);
        let path = dir.join("pipeline.lock");

        if let Ok(bytes) = fs::read(&path) {
            if let Ok(info) = serde_json::from_slice::<LockInfo>(&bytes) {
                if pid_alive(info.pid) {
                    return Err(format!(
                        "another pipeline is running (pid {} · stage {} · since {})",
                        info.pid, info.stage, info.started_at
                    ));
                }
                // Stale lock from a dead process — break it.
                eprintln!(
                    "[safety] breaking stale pipeline.lock (dead pid {})",
                    info.pid
                );
            }
        }

        let info = LockInfo {
            pid: std::process::id(),
            stage: stage.to_string(),
            started_at: now_stamp(),
        };
        fs::write(&path, serde_json::to_vec(&info).unwrap_or_default())
            .map_err(|e| format!("lock write: {}", e))?;
        Ok(PipelineLock { path })
    }

    /// Update the stage recorded in the lock file (best-effort, for visibility).
    pub fn set_stage(&self, stage: &str) {
        let info = LockInfo {
            pid: std::process::id(),
            stage: stage.to_string(),
            started_at: now_stamp(),
        };
        let _ = fs::write(&self.path, serde_json::to_vec(&info).unwrap_or_default());
    }
}

impl Drop for PipelineLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

fn pid_alive(pid: u32) -> bool {
    // kill -0 semantics without a signal crate.
    Command::new("/bin/kill")
        .arg("-0")
        .arg(pid.to_string())
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn now_stamp() -> String {
    // yyyymmdd-HHMMSS from `date` — avoids pulling in chrono.
    Command::new("/bin/date")
        .arg("+%Y%m%d-%H%M%S")
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|| "unknown-time".into())
}

/// Zip Subcrates/ + `database V2` into ~/.dj-crates-tools/snapshots/ and
/// prune old snapshots. Returns the snapshot path.
pub fn snapshot_serato(serato_dir: &Path) -> Result<PathBuf, String> {
    let snap_dir = state_dir().join("snapshots");
    fs::create_dir_all(&snap_dir).map_err(|e| format!("snapshot dir: {}", e))?;

    // Stage the two items into a temp folder so one `ditto` call zips both.
    let stamp = now_stamp();
    let stage = snap_dir.join(format!(".staging-{}", stamp));
    let _ = fs::remove_dir_all(&stage);
    fs::create_dir_all(&stage).map_err(|e| format!("staging: {}", e))?;

    let subcrates = serato_dir.join("Subcrates");
    if subcrates.exists() {
        run_ok(
            Command::new("/usr/bin/ditto")
                .arg(&subcrates)
                .arg(stage.join("Subcrates")),
            "ditto Subcrates",
        )?;
    }
    let db = serato_dir.join("database V2");
    if db.exists() {
        fs::copy(&db, stage.join("database V2")).map_err(|e| format!("copy db: {}", e))?;
    }
    let neworder = serato_dir.join("neworder.pref");
    if neworder.exists() {
        let _ = fs::copy(&neworder, stage.join("neworder.pref"));
    }

    let zip_path = snap_dir.join(format!("serato-{}.zip", stamp));
    run_ok(
        Command::new("/usr/bin/ditto")
            .arg("-c")
            .arg("-k")
            .arg("--sequesterRsrc")
            .arg(&stage)
            .arg(&zip_path),
        "ditto zip",
    )?;
    let _ = fs::remove_dir_all(&stage);

    // Rotate: keep the newest SNAPSHOT_KEEP snapshots.
    let mut snaps: Vec<PathBuf> = fs::read_dir(&snap_dir)
        .map_err(|e| format!("snapshot list: {}", e))?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .map(|n| {
                    let n = n.to_string_lossy();
                    n.starts_with("serato-") && n.ends_with(".zip")
                })
                .unwrap_or(false)
        })
        .collect();
    snaps.sort();
    while snaps.len() > SNAPSHOT_KEEP {
        let old = snaps.remove(0);
        let _ = fs::remove_file(old);
    }

    Ok(zip_path)
}

fn run_ok(cmd: &mut Command, what: &str) -> Result<(), String> {
    let out = cmd.output().map_err(|e| format!("{}: {}", what, e))?;
    if !out.status.success() {
        return Err(format!(
            "{}: {}",
            what,
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(())
}
