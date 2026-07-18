// Bulk M4A → MP3 conversion using ffmpeg. Converts every .m4a under
// CRATES/GENRES/ to a 320 kbps MP3 (ID3v2.3 for Serato compatibility),
// archives the original .m4a, and rewrites `pfil` in Serato's database V2.
//
// Serato's cue points / beat grids / waveform overviews are stored in
// MP4-specific `----:com.serato.dj:*` atoms. ffmpeg strips these during
// conversion — the user needs to re-analyze in Serato afterwards.

use crate::{emit_progress, to_serato_db_path, update_serato_db, SeratoDbUpdate};
use serde::Serialize;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::AppHandle;

fn collect_m4a(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(entries) = fs::read_dir(dir) else {
        return out;
    };
    for e in entries.filter_map(|x| x.ok()) {
        let p = e.path();
        if p.is_dir() {
            out.extend(collect_m4a(&p));
        } else if p.is_file()
            && p.extension()
                .and_then(|e| e.to_str())
                .map(|e| e.eq_ignore_ascii_case("m4a"))
                .unwrap_or(false)
        {
            out.push(p);
        }
    }
    out
}

#[derive(Debug, Clone, Serialize)]
pub struct ConvertReport {
    pub total_m4a: usize,
    pub converted: usize,
    pub skipped_existing_mp3: usize,
    pub failed: usize,
    pub serato_db_updated: usize,
    pub archive_root: String,
    pub errors: Vec<String>,
}

#[tauri::command]
pub fn convert_m4a_to_mp3(
    app: AppHandle,
    crates_root: String,
    serato_path: Option<String>,
) -> Result<ConvertReport, String> {
    // Verify ffmpeg is available
    let ffmpeg_check = Command::new("ffmpeg").arg("-version").output();
    if ffmpeg_check.map(|o| !o.status.success()).unwrap_or(true) {
        return Err("ffmpeg not found on PATH — install with `brew install ffmpeg`.".into());
    }

    let root = PathBuf::from(&crates_root);
    let genres = root.join("GENRES");
    if !genres.is_dir() {
        return Err(format!("GENRES folder not found under {}", crates_root));
    }

    let files = collect_m4a(&genres);
    let total = files.len();

    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let archive_root = std::env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("Music")
        .join("_Archive")
        .join(format!("m4a_originals_{}", ts));

    let mut report = ConvertReport {
        total_m4a: total,
        converted: 0,
        skipped_existing_mp3: 0,
        failed: 0,
        serato_db_updated: 0,
        archive_root: archive_root.to_string_lossy().into(),
        errors: Vec::new(),
    };
    let mut db_updates: HashMap<String, SeratoDbUpdate> = HashMap::new();

    emit_progress(Some(&app), "convert_start", 0, total, "Starting M4A → MP3");

    for (idx, m4a) in files.iter().enumerate() {
        let mp3 = m4a.with_extension("mp3");

        if mp3.exists() {
            report.skipped_existing_mp3 += 1;
            // Progress tick even for skips
            if (idx + 1) % 5 == 0 || idx + 1 == total {
                emit_progress(
                    Some(&app),                    "converting",
                    idx + 1,
                    total,
                    &format!(
                        "{} done · {} skipped · {} failed",
                        report.converted, report.skipped_existing_mp3, report.failed
                    ),
                );
            }
            continue;
        }

        let status = Command::new("ffmpeg")
            .arg("-hide_banner")
            .arg("-loglevel")
            .arg("error")
            .arg("-y") // overwrite
            .arg("-i")
            .arg(m4a)
            .arg("-c:a")
            .arg("libmp3lame")
            .arg("-b:a")
            .arg("320k")
            .arg("-map_metadata")
            .arg("0")
            .arg("-id3v2_version")
            .arg("3")
            .arg(&mp3)
            .status();

        let ok = match status {
            Ok(s) if s.success() => mp3.exists() && fs::metadata(&mp3).map(|m| m.len() > 10_000).unwrap_or(false),
            Ok(s) => {
                report
                    .errors
                    .push(format!("{}: ffmpeg exit {}", m4a.display(), s.code().unwrap_or(-1)));
                false
            }
            Err(e) => {
                report.errors.push(format!("{}: spawn {}", m4a.display(), e));
                false
            }
        };

        if !ok {
            report.failed += 1;
            // Clean up a partially-written mp3 to avoid future "skipped_existing" false-positives
            let _ = fs::remove_file(&mp3);
            if (idx + 1) % 5 == 0 || idx + 1 == total {
                emit_progress(
                    Some(&app),                    "converting",
                    idx + 1,
                    total,
                    &format!(
                        "{} done · {} skipped · {} failed",
                        report.converted, report.skipped_existing_mp3, report.failed
                    ),
                );
            }
            continue;
        }

        // Archive the original M4A (preserve subfolder structure under archive_root)
        let rel = m4a.strip_prefix(&genres).unwrap_or(m4a);
        let archive_dest = archive_root.join(rel);
        if let Some(parent) = archive_dest.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if let Err(e) = fs::rename(m4a, &archive_dest).or_else(|_| {
            fs::copy(m4a, &archive_dest).and_then(|_| fs::remove_file(m4a))
        }) {
            report
                .errors
                .push(format!("archive orig {}: {}", m4a.display(), e));
            // Don't count as failure — the MP3 still exists. Just log.
        }

        // Record Serato pfil update: old M4A path → new MP3 path
        db_updates.insert(
            to_serato_db_path(m4a),
            SeratoDbUpdate {
                new_pfil: Some(to_serato_db_path(&mp3)),
                ..Default::default()
            },
        );
        report.converted += 1;

        if (idx + 1) % 5 == 0 || idx + 1 == total {
            emit_progress(
                Some(&app),                "converting",
                idx + 1,
                total,
                &format!(
                    "{} done · {} skipped · {} failed",
                    report.converted, report.skipped_existing_mp3, report.failed
                ),
            );
        }
    }

    if let Some(sp) = serato_path.as_deref() {
        let db_path = PathBuf::from(sp).join("database V2");
        if db_path.exists() && !db_updates.is_empty() {
            emit_progress(Some(&app), "convert_db", 0, 0, "Updating Serato database V2");
            match update_serato_db(&db_path, &db_updates) {
                Ok(n) => report.serato_db_updated = n,
                Err(e) => report.errors.push(format!("serato db: {}", e)),
            }
        }
    }

    emit_progress(Some(&app), "convert_done", total, total, "M4A → MP3 complete");
    Ok(report)
}
