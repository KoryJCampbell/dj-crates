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

/// Returns a process exit code (0 on success, 1 on failure).
pub fn run_subcommand(handle: AppHandle, subcmd: &str) -> i32 {
    eprintln!("[headless] subcommand: {}", subcmd);
    let cfg = match load_settings(&handle) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("[headless] fatal: {}", e);
            return 1;
        }
    };
    eprintln!("[headless] crates_root = {}", cfg.crates_root);

    let steps: Vec<&str> = match subcmd {
        "pipeline" => vec![
            "sync",
            "sanitize",
            "fix-bpms",
            "label-moods",
            "enrich-popularity",
            "downbeat-cue",
        ],
        other => vec![other],
    };

    let mut failed = 0;
    for step in &steps {
        eprintln!("\n── running: {} ──", step);
        let res = run_step(&handle, step, &cfg);
        match res {
            Ok(msg) => eprintln!("[ok] {}: {}", step, msg),
            Err(e) => {
                eprintln!("[fail] {}: {}", step, e);
                failed += 1;
                // Keep going — a failed sanitize shouldn't stop auto-cue.
            }
        }
    }

    if failed > 0 {
        eprintln!("\n[headless] {} step(s) failed", failed);
        1
    } else {
        eprintln!("\n[headless] all steps ok");
        0
    }
}

fn run_step(handle: &AppHandle, step: &str, cfg: &StoredConfig) -> Result<String, String> {
    match step {
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
