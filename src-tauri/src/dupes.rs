// Duplicate finder — groups tracks that share the same normalized
// artist|title and near-identical duration, ranks each group by audio
// quality, and surfaces the redundant copies for review.
//
// Read-only by design: writes a JSON report to ~/.dj-crates-tools/ and
// (optionally) a "SMART%%Duplicates Review" crate so groups can be
// auditioned side-by-side in Serato. Never deletes or moves audio files.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;
use std::time::Duration;
use tauri::AppHandle;

use crate::{
    build_crate_bytes, emit_progress, library_cache_path, read_neworder_entries,
    scan_library_read_only, spotify_key, write_neworder_entries, TrackMeta,
};

/// Two files count as "the same recording" only if their durations are within
/// this many seconds — keeps radio edits / extended mixes out of the report.
const DURATION_TOLERANCE_SECS: f64 = 3.0;

/// How stale the sync-maintained library cache can be before we rescan.
const CACHE_MAX_AGE_SECS: u64 = 48 * 3600;

pub const REVIEW_CRATE: &str = "SMART%%Duplicates Review";

#[derive(Debug, Clone)]
pub(crate) struct Candidate {
    pub serato_path: String,
    pub abs_path: String,
    pub artist: String,
    pub title: String,
    pub duration_secs: f64,
    pub bitrate: u32,
    pub sample_rate: u32,
    pub file_size: u64,
    pub file_type: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DupeTrackReport {
    pub path: String,
    pub keep: bool,
    pub bitrate: u32,
    pub sample_rate: u32,
    pub file_size: u64,
    pub duration_secs: f64,
    pub file_type: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DupeGroupReport {
    pub artist: String,
    pub title: String,
    pub tracks: Vec<DupeTrackReport>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DupesResult {
    pub scanned: usize,
    pub groups: usize,
    pub extra_copies: usize,
    pub wasted_bytes: u64,
    pub report_path: String,
    pub crate_name: Option<String>,
    pub sample: Vec<String>,
    pub used_cache: bool,
}

#[derive(Serialize)]
struct DupesReportFile<'a> {
    scanned: usize,
    total_groups: usize,
    extra_copies: usize,
    wasted_bytes: u64,
    groups: &'a [DupeGroupReport],
}

fn quality_rank(c: &Candidate) -> (u32, u32, u64) {
    // Bitrate first (lossless reads as ~900+ kbps in lofty, so FLAC naturally
    // outranks 320 mp3), then sample rate, then raw size.
    (c.bitrate, c.sample_rate, c.file_size)
}

/// Group candidates into duplicate sets. Returns index vectors into `cands`,
/// each sorted best-quality-first, in a deterministic order.
pub(crate) fn group_duplicates(cands: &[Candidate]) -> Vec<Vec<usize>> {
    let mut buckets: HashMap<String, Vec<usize>> = HashMap::new();
    for (i, c) in cands.iter().enumerate() {
        if c.artist.trim().is_empty() || c.title.trim().is_empty() {
            continue;
        }
        buckets
            .entry(spotify_key(&c.artist, &c.title))
            .or_default()
            .push(i);
    }

    let mut groups: Vec<Vec<usize>> = Vec::new();
    for (_, mut idxs) in buckets {
        if idxs.len() < 2 {
            continue;
        }
        // Cluster by duration so different versions of the same song
        // (radio edit vs extended) don't get lumped together.
        idxs.sort_by(|a, b| {
            cands[*a]
                .duration_secs
                .partial_cmp(&cands[*b].duration_secs)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let mut cluster: Vec<usize> = Vec::new();
        let mut cluster_start = 0.0f64;
        for &i in &idxs {
            let d = cands[i].duration_secs;
            if cluster.is_empty() {
                cluster_start = d;
                cluster.push(i);
            } else if d - cluster_start <= DURATION_TOLERANCE_SECS {
                cluster.push(i);
            } else {
                if cluster.len() >= 2 {
                    groups.push(std::mem::take(&mut cluster));
                } else {
                    cluster.clear();
                }
                cluster_start = d;
                cluster.push(i);
            }
        }
        if cluster.len() >= 2 {
            groups.push(cluster);
        }
    }

    // Best copy first within each group.
    for g in groups.iter_mut() {
        g.sort_by(|a, b| {
            quality_rank(&cands[*b])
                .cmp(&quality_rank(&cands[*a]))
                .then_with(|| cands[*a].serato_path.cmp(&cands[*b].serato_path))
        });
    }
    // Deterministic report order.
    groups.sort_by(|a, b| cands[a[0]].serato_path.cmp(&cands[b[0]].serato_path));
    groups
}

#[tauri::command]
pub(crate) fn find_duplicates(
    app: AppHandle,
    crates_root: String,
    serato_path: Option<String>,
    write_crate: Option<bool>,
) -> Result<DupesResult, String> {
    let root = PathBuf::from(&crates_root);
    if !root.exists() {
        return Err(format!("Crates directory not found: {}", crates_root));
    }

    // Reuse the sync-maintained library cache when fresh; otherwise rescan
    // (and refresh the cache for Smart Crates while we're at it).
    let cache_path = library_cache_path();
    let used_cache = fs::metadata(&cache_path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.elapsed().ok())
        .map(|age| age < Duration::from_secs(CACHE_MAX_AGE_SECS))
        .unwrap_or(false);

    emit_progress(
        Some(&app),
        "dupes_loading",
        0,
        0,
        if used_cache {
            "Loading library cache"
        } else {
            "Scanning library (cache missing or stale)"
        },
    );

    let library: Vec<TrackMeta> = if used_cache {
        let data = fs::read(&cache_path).map_err(|e| format!("Cache read: {}", e))?;
        serde_json::from_slice(&data).map_err(|e| format!("Cache parse: {}", e))?
    } else {
        let lib = scan_library_read_only(&root, &app);
        if let Ok(json) = serde_json::to_vec(&lib) {
            let _ = fs::write(&cache_path, json);
        }
        lib
    };

    // GENRES only — PLAYLISTS entries are references (often symlinks) into the
    // genre tree. Resolve symlinks so two paths to one file never count as dupes.
    let mut seen: HashSet<String> = HashSet::new();
    let mut cands: Vec<Candidate> = Vec::new();
    for t in &library {
        if t.hierarchy.first().map(|h| h == "GENRES") != Some(true) {
            continue;
        }
        let identity = fs::canonicalize(&t.abs_path)
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| t.serato_path.clone());
        if !seen.insert(identity) {
            continue;
        }
        cands.push(Candidate {
            serato_path: t.serato_path.clone(),
            abs_path: t.abs_path.to_string_lossy().to_string(),
            artist: t.artist.clone(),
            title: t.title.clone(),
            duration_secs: t.duration_secs,
            bitrate: t.bitrate,
            sample_rate: t.sample_rate,
            file_size: t.file_size,
            file_type: t.file_type.clone(),
        });
    }

    emit_progress(
        Some(&app),
        "dupes_grouping",
        0,
        cands.len(),
        "Grouping by artist · title · duration",
    );
    let groups_idx = group_duplicates(&cands);

    let mut report_groups: Vec<DupeGroupReport> = Vec::new();
    let mut extra_copies = 0usize;
    let mut wasted_bytes = 0u64;
    let mut crate_paths: Vec<String> = Vec::new();
    for g in &groups_idx {
        let best = &cands[g[0]];
        let mut tracks = Vec::new();
        for (rank, &i) in g.iter().enumerate() {
            let c = &cands[i];
            let keep = rank == 0;
            if !keep {
                extra_copies += 1;
                wasted_bytes += c.file_size;
            }
            crate_paths.push(c.serato_path.clone());
            tracks.push(DupeTrackReport {
                path: c.abs_path.clone(),
                keep,
                bitrate: c.bitrate,
                sample_rate: c.sample_rate,
                file_size: c.file_size,
                duration_secs: c.duration_secs,
                file_type: c.file_type.clone(),
            });
        }
        report_groups.push(DupeGroupReport {
            artist: best.artist.clone(),
            title: best.title.clone(),
            tracks,
        });
    }

    // JSON report next to the other app caches.
    let report_path = library_cache_path().with_file_name("duplicates_report.json");
    let report_file = DupesReportFile {
        scanned: cands.len(),
        total_groups: report_groups.len(),
        extra_copies,
        wasted_bytes,
        groups: &report_groups,
    };
    if let Ok(json) = serde_json::to_vec_pretty(&report_file) {
        let _ = fs::write(&report_path, json);
    }

    // Review crate: every group's tracks together, so they can be compared
    // and auditioned side-by-side in Serato before any manual cleanup.
    let mut crate_name: Option<String> = None;
    if write_crate.unwrap_or(true) {
        if let Some(sp) = serato_path.as_ref().filter(|s| !s.trim().is_empty()) {
            let serato_dir = PathBuf::from(sp);
            let subcrates_dir = serato_dir.join("Subcrates");
            if subcrates_dir.exists() {
                let crate_file = subcrates_dir.join(format!("{}.crate", REVIEW_CRATE));
                if report_groups.is_empty() {
                    // No dupes — drop a stale review crate from a previous run.
                    let _ = fs::remove_file(&crate_file);
                } else {
                    fs::write(&crate_file, build_crate_bytes(&crate_paths))
                        .map_err(|e| format!("Review crate write: {}", e))?;

                    // Ensure the SMART group exists + is registered (same as smart crates).
                    let top_level = subcrates_dir.join("SMART.crate");
                    if !top_level.exists() {
                        let _ = fs::write(&top_level, build_crate_bytes(&[]));
                    }
                    let neworder_path = serato_dir.join("neworder.pref");
                    let mut entries = read_neworder_entries(&neworder_path);
                    if !entries.iter().any(|e| e == "SMART") {
                        let insert_at = entries
                            .iter()
                            .position(|e| e == "GENRES" || e == "PLAYLISTS")
                            .unwrap_or(entries.len());
                        entries.insert(insert_at, "SMART".to_string());
                        let _ = write_neworder_entries(&neworder_path, &entries);
                    }
                    crate_name = Some(REVIEW_CRATE.to_string());
                }
            }
        }
    }

    let sample: Vec<String> = report_groups
        .iter()
        .take(8)
        .map(|g| format!("{} — {} ×{}", g.artist, g.title, g.tracks.len()))
        .collect();

    emit_progress(
        Some(&app),
        "done",
        cands.len(),
        cands.len(),
        "Duplicate scan complete",
    );

    Ok(DupesResult {
        scanned: cands.len(),
        groups: report_groups.len(),
        extra_copies,
        wasted_bytes,
        report_path: report_path.to_string_lossy().to_string(),
        crate_name,
        sample,
        used_cache,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cand(
        path: &str,
        artist: &str,
        title: &str,
        duration: f64,
        bitrate: u32,
        size: u64,
    ) -> Candidate {
        Candidate {
            serato_path: path.to_string(),
            abs_path: format!("/{}", path),
            artist: artist.to_string(),
            title: title.to_string(),
            duration_secs: duration,
            bitrate,
            sample_rate: 44100,
            file_size: size,
            file_type: "mp3".to_string(),
        }
    }

    #[test]
    fn same_song_close_duration_groups_best_first() {
        let cands = vec![
            cand("a/low.mp3", "SZA", "Snooze", 201.0, 128, 3_000_000),
            cand("a/high.mp3", "SZA", "Snooze", 202.5, 320, 8_000_000),
        ];
        let groups = group_duplicates(&cands);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0], vec![1, 0], "320kbps copy ranked first");
    }

    #[test]
    fn different_durations_are_different_versions() {
        let cands = vec![
            cand("a/radio.mp3", "Kaytranada", "Witchy", 195.0, 320, 8_000_000),
            cand("a/extended.mp3", "Kaytranada", "Witchy", 312.0, 320, 12_000_000),
        ];
        assert!(group_duplicates(&cands).is_empty());
    }

    #[test]
    fn normalization_matches_case_and_punctuation() {
        let cands = vec![
            cand("a/one.mp3", "D'Angelo", "Untitled (How Does It Feel)", 431.0, 256, 7_000_000),
            cand("a/two.mp3", "dangelo", "untitled how does it feel", 430.0, 320, 9_000_000),
        ];
        let groups = group_duplicates(&cands);
        assert_eq!(groups.len(), 1);
        assert_eq!(cands[groups[0][0]].bitrate, 320);
    }

    #[test]
    fn singletons_and_untagged_are_skipped() {
        let cands = vec![
            cand("a/solo.mp3", "Erykah Badu", "On & On", 227.0, 320, 8_000_000),
            cand("a/blank1.mp3", "", "", 227.0, 320, 8_000_000),
            cand("a/blank2.mp3", "", "", 227.0, 320, 8_000_000),
        ];
        assert!(group_duplicates(&cands).is_empty());
    }

    #[test]
    fn bitrate_tie_breaks_on_file_size() {
        let cands = vec![
            cand("a/small.mp3", "Daniel Caesar", "Best Part", 209.0, 320, 7_900_000),
            cand("a/big.mp3", "Daniel Caesar", "Best Part", 209.4, 320, 8_400_000),
        ];
        let groups = group_duplicates(&cands);
        assert_eq!(groups.len(), 1);
        assert_eq!(cands[groups[0][0]].serato_path, "a/big.mp3");
    }

    #[test]
    fn three_copies_two_flagged() {
        let cands = vec![
            cand("a/1.mp3", "Tems", "Free Mind", 235.0, 128, 3_500_000),
            cand("a/2.mp3", "Tems", "Free Mind", 235.5, 320, 9_000_000),
            cand("a/3.mp3", "Tems", "Free Mind", 236.0, 192, 5_500_000),
        ];
        let groups = group_duplicates(&cands);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].len(), 3);
        assert_eq!(cands[groups[0][0]].bitrate, 320, "keeper is highest bitrate");
    }
}
