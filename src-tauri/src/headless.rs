// Headless / CLI entry point. Invoked when the app is launched with
// `--headless <subcommand>` (typically by cron via dj-daily).
//
// Loads the same settings the UI uses, then calls the Tauri command
// functions directly (they each take an AppHandle; we pass the one from
// the app's setup() callback). No window is shown to the user.

use std::path::PathBuf;
use tauri::{AppHandle, Manager};

use crate::tagging;

#[derive(serde::Deserialize, Debug)]
struct SettingsFile {
    config: StoredConfig,
}

#[derive(serde::Deserialize, Debug)]
struct StoredConfig {
    #[serde(default, rename = "cratesRoot")]
    crates_root: String,
    #[serde(default, rename = "seratoPath")]
    serato_path: String,
    #[serde(default, rename = "pythonVenv")]
    python_venv: String,
    #[serde(default, rename = "scriptsDir")]
    scripts_dir: String,
    #[serde(default, rename = "spotifyClientId")]
    spotify_client_id: String,
    #[serde(default, rename = "spotifyClientSecret")]
    spotify_client_secret: String,
    #[serde(default, rename = "unsortedPath")]
    unsorted_path: String,
}

fn settings_path(handle: &AppHandle) -> PathBuf {
    // Use Tauri's resolver so this works regardless of bundle identifier changes.
    handle
        .path()
        .app_config_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("settings.json")
}

fn load_settings(handle: &AppHandle) -> Result<StoredConfig, String> {
    let p = settings_path(handle);
    let bytes = std::fs::read(&p)
        .map_err(|e| format!("settings read ({}): {}", p.display(), e))?;
    let parsed: SettingsFile = serde_json::from_slice(&bytes)
        .map_err(|e| format!("settings parse: {}", e))?;
    if parsed.config.crates_root.is_empty() {
        return Err("cratesRoot not configured — open the app UI once first".into());
    }
    Ok(parsed.config)
}

fn none_if_empty(s: &str) -> Option<String> {
    if s.trim().is_empty() {
        None
    } else {
        Some(s.to_string())
    }
}

/// Steps that rewrite Serato state (crates, database V2) — these require the
/// Serato-closed guard and a pre-run snapshot.
fn writes_serato(step: &str) -> bool {
    matches!(
        step,
        "sync" | "sanitize" | "fix-bpms" | "label-moods" | "find-dupes"
    )
}

/// The one-button pipeline: file new arrivals, clean tags, cue, then mirror
/// to Serato and Apple Music. label-moods stays out until its UTF-8 panic is
/// fixed; enrich-popularity stays out until the Spotify search actually
/// returns data (see AUDIT-2026-07-17.md #5).
pub const PIPELINE_STEPS: &[&str] = &[
    "intake",
    "sanitize",
    "fix-bpms",
    "downbeat-cue",
    "sync",
    "apple-music",
];

/// Run a sequence of steps with the full safety rails (Serato guard, pipeline
/// lock, pre-run snapshot). Shared by the headless CLI and the UI's one-button
/// command. Returns (per-step summary lines, failed count); Err only for
/// fatal pre-checks that stop the run before any step executes.
pub fn run_steps_core(
    handle: &AppHandle,
    steps: &[&str],
) -> Result<(Vec<String>, usize), String> {
    let cfg = load_settings(handle)?;
    eprintln!("[headless] crates_root = {}", cfg.crates_root);

    // Guard: never rewrite crates/db while Serato is open.
    if steps.iter().any(|s| writes_serato(s)) && crate::safety::serato_running() {
        return Err("Serato DJ is running — quit it and retry".into());
    }

    // One pipeline at a time (UI button, dj-daily, dj-weekly share this lock).
    let lock = crate::safety::PipelineLock::acquire(steps.first().unwrap_or(&"run"))?;

    // Snapshot Serato state before the first destructive step.
    if steps.iter().any(|s| writes_serato(s)) && !cfg.serato_path.is_empty() {
        let p = crate::safety::snapshot_serato(std::path::Path::new(&cfg.serato_path))
            .map_err(|e| format!("snapshot failed, refusing to continue: {}", e))?;
        eprintln!("[safety] snapshot: {}", p.display());
    }

    let mut failed = 0;
    let mut summary: Vec<String> = Vec::new();
    for step in steps {
        eprintln!("\n── running: {} ──", step);
        lock.set_stage(step);
        match run_step(handle, step, &cfg) {
            Ok(msg) => {
                eprintln!("[ok] {}: {}", step, msg);
                summary.push(format!("{}: {}", step, msg));
            }
            Err(e) => {
                eprintln!("[fail] {}: {}", step, e);
                summary.push(format!("{}: FAILED ({})", step, e));
                failed += 1;
                // Keep going — a failed sanitize shouldn't stop auto-cue.
            }
        }
    }

    // One digestible line for logs/notifications.
    eprintln!("\n[summary] {}", summary.join(" · "));
    Ok((summary, failed))
}

/// Returns a process exit code (0 on success, 1 on failure).
pub fn run_subcommand(handle: AppHandle, subcmd: &str) -> i32 {
    eprintln!("[headless] subcommand: {}", subcmd);
    let steps: Vec<&str> = match subcmd {
        "pipeline" => PIPELINE_STEPS.to_vec(),
        other => vec![other],
    };

    match run_steps_core(&handle, &steps) {
        Err(e) => {
            eprintln!("[headless] fatal: {}", e);
            1
        }
        Ok((_, failed)) if failed > 0 => {
            eprintln!("[headless] {} step(s) failed", failed);
            1
        }
        Ok(_) => {
            eprintln!("[headless] all steps ok");
            0
        }
    }
}

/// Resolve the python interpreter: configured venv first, then system python3.
fn python_bin(cfg: &StoredConfig) -> String {
    let venv = cfg.python_venv.trim();
    if !venv.is_empty() && std::path::Path::new(venv).exists() {
        venv.to_string()
    } else {
        "python3".to_string()
    }
}

/// Run a helper script from scripts_dir, returning the RESULT: line.
fn run_python_helper(
    cfg: &StoredConfig,
    script: &str,
    args: &[&str],
) -> Result<String, String> {
    let scripts = cfg.scripts_dir.trim();
    if scripts.is_empty() {
        return Err("scriptsDir not configured".into());
    }
    let script_path = std::path::Path::new(scripts).join(script);
    if !script_path.exists() {
        return Err(format!("script not found: {}", script_path.display()));
    }
    let out = std::process::Command::new(python_bin(cfg))
        .arg(&script_path)
        .args(args)
        .output()
        .map_err(|e| format!("spawn {}: {}", script, e))?;
    let stdout = String::from_utf8_lossy(&out.stdout);
    // Surface the script's own progress lines in our log.
    for line in stdout.lines() {
        eprintln!("[{}] {}", script, line);
    }
    if !out.status.success() {
        return Err(format!(
            "{} exited {}: {}",
            script,
            out.status,
            String::from_utf8_lossy(&out.stderr)
                .lines()
                .last()
                .unwrap_or("")
        ));
    }
    stdout
        .lines()
        .rev()
        .find(|l| l.starts_with("RESULT:"))
        .map(|l| l[7..].to_string())
        .ok_or_else(|| format!("{} produced no RESULT line", script))
}

fn run_step(handle: &AppHandle, step: &str, cfg: &StoredConfig) -> Result<String, String> {
    match step {
        "intake" => {
            // File new arrivals out of UNASSIGNED (and Unsorted, if configured)
            // using the conservative artist-index + genre-tag cascade.
            run_python_helper(cfg, "classify_unassigned.py", &["--crates-root", &cfg.crates_root, "--execute"])
        }
        "apple-music" => {
            // Mirror GENRES/PLAYLISTS folder trees into Music.app playlists.
            // Incremental: only leaves whose track counts drifted are rebuilt.
            run_python_helper(cfg, "apple_music_sync.py", &["--crates-root", &cfg.crates_root])
        }
        "sync" => {
            let r = crate::sync_to_serato(
                handle.clone(),
                cfg.crates_root.clone(),
                cfg.serato_path.clone(),
                none_if_empty(&cfg.python_venv),
                none_if_empty(&cfg.scripts_dir),
                none_if_empty(&cfg.unsorted_path),
                true,  // clean
                false, // preview
            )?;
            Ok(format!(
                "{} crates · {} tracks · {} bpm fixed",
                r.crates_written, r.total_track_entries, r.bpm_adjusted
            ))
        }
        "sanitize" => {
            let r = tagging::sanitize_library(
                handle.clone(),
                cfg.crates_root.clone(),
                none_if_empty(&cfg.serato_path),
                none_if_empty(&cfg.spotify_client_id),
                none_if_empty(&cfg.spotify_client_secret),
            )?;
            Ok(format!(
                "{}/{} updated · {} comments",
                r.updated, r.scanned, r.comments_written
            ))
        }
        "fix-bpms" => {
            let r = tagging::normalize_bpms(
                handle.clone(),
                cfg.crates_root.clone(),
                none_if_empty(&cfg.serato_path),
            )?;
            Ok(format!(
                "{} scanned · {} halved · {} doubled · {} quartered",
                r.scanned, r.halved, r.doubled, r.quartered
            ))
        }
        "label-moods" => {
            let r = tagging::label_moods(
                handle.clone(),
                cfg.crates_root.clone(),
                none_if_empty(&cfg.serato_path),
            )?;
            Ok(format!("{}/{} labeled", r.labeled, r.scanned))
        }
        "enrich-popularity" => {
            if cfg.spotify_client_id.is_empty() || cfg.spotify_client_secret.is_empty() {
                return Err("spotifyClientId / Secret not configured — skipping".into());
            }
            let r = crate::enrich_spotify_popularity(
                handle.clone(),
                cfg.spotify_client_id.clone(),
                cfg.spotify_client_secret.clone(),
                false, // force
            )?;
            Ok(format!(
                "{} total · {} fetched · {} cached · {} not found",
                r.total, r.enriched, r.cached, r.not_found
            ))
        }
        "find-dupes" => {
            let r = crate::dupes::find_duplicates(
                handle.clone(),
                cfg.crates_root.clone(),
                none_if_empty(&cfg.serato_path),
                Some(true), // write the SMART%%Duplicates Review crate
            )?;
            Ok(format!(
                "{} scanned · {} groups · {} extra copies · {:.2} GB reclaimable · report: {}",
                r.scanned,
                r.groups,
                r.extra_copies,
                r.wasted_bytes as f64 / 1e9,
                r.report_path
            ))
        }
        "downbeat-cue" => {
            let r = tagging::auto_cue_library(
                handle.clone(),
                cfg.crates_root.clone(),
                none_if_empty(&cfg.python_venv),
                none_if_empty(&cfg.scripts_dir),
                Some(16), // bars_per_cue
                Some(1),  // num_cues
                Some(7),  // cue_index (slot 8)
                Some(false), // force — incremental, skip already-cued
            )?;
            Ok(format!(
                "{}/{} cued · {} skipped",
                r.written, r.scanned, r.skipped
            ))
        }
        other => Err(format!("unknown subcommand: {}", other)),
    }
}
