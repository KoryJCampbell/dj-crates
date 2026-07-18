// Post-sync tagging passes — BPM normalization and mood labeling.
// These run against files already in CRATES/GENRES/<genre>/... and write
// back to ID3 tags. Separate from the main sync flow so they can be run
// independently via their own buttons.

use crate::spotify::SpotifyClient;
use crate::{
    emit_progress, load_spotify_cache, spotify_key, to_serato_db_path, update_serato_db,
    SeratoDbUpdate,
};
use lofty::config::WriteOptions;
use lofty::prelude::*;
use lofty::probe::Probe;
use lofty::tag::{ItemKey, Tag};
use serde::Serialize;
use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use tauri::AppHandle;
use walkdir::WalkDir;

const AUDIO_EXTENSIONS: &[&str] = &[
    "mp3", "flac", "wav", "ogg", "aif", "aiff", "aac", "m4a", "alac",
];

// Per-genre BPM ranges. Trap is folded into Hip-Hop's halftime 68-105 range
// — a 140 detected BPM trap track will halve to 70 to blend with R&B.
// Anything not in this list falls back to DEFAULT_BPM.
const GENRE_BPM: &[(&str, f32, f32)] = &[
    ("R&B", 68.0, 110.0),
    ("Neo-Soul", 68.0, 110.0),
    ("Soul", 68.0, 115.0),
    ("Gospel", 68.0, 120.0),
    ("Jazz", 68.0, 120.0),
    ("Hip-Hop", 68.0, 105.0),
    ("Hip-Hop:Rap", 68.0, 105.0),
    ("Afrobeats", 95.0, 125.0),
    ("Amapiano", 105.0, 125.0),
    ("House", 115.0, 132.0),
    ("Afro House", 115.0, 132.0),
    ("Dancehall", 68.0, 110.0),
    ("Reggae", 68.0, 95.0),
    ("Funk", 95.0, 125.0),
    ("Disco", 100.0, 130.0),
    ("Jersey Club", 128.0, 135.0),
    ("Baltimore Club", 128.0, 135.0),
    ("Baile Funk", 125.0, 135.0),
    ("Moombahton", 105.0, 115.0),
    ("Gqom", 115.0, 130.0),
    ("Kwaito", 100.0, 115.0),
    ("UK Funky", 120.0, 135.0),
    ("UK Garage", 128.0, 135.0),
    ("Techno", 120.0, 135.0),
    ("Electronic", 115.0, 135.0),
    ("Lo-Fi", 68.0, 100.0),
    ("Lounge", 68.0, 110.0),
    ("Motown", 95.0, 130.0),
    ("Latin", 85.0, 130.0),
    ("Reggaeton", 88.0, 100.0),
    ("Soca", 125.0, 135.0),
];

const DEFAULT_BPM: (f32, f32) = (68.0, 135.0);

// Genres considered "vocal" — Sing Along mood only fires on these.
const VOCAL_GENRES: &[&str] = &[
    "R&B", "Soul", "Neo-Soul", "Gospel", "Hip-Hop", "Hip-Hop:Rap",
    "Motown", "Pop",
];

// Club genres that route to Hype regardless of BPM.
const CLUB_HYPE_GENRES: &[&str] = &[
    "Jersey Club", "Baltimore Club", "Baile Funk", "Gqom", "UK Funky",
];

fn is_audio(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| AUDIO_EXTENSIONS.iter().any(|x| x.eq_ignore_ascii_case(e)))
        .unwrap_or(false)
}

fn genre_bpm_range(genre: &str) -> (f32, f32) {
    GENRE_BPM
        .iter()
        .find(|(g, _, _)| g.eq_ignore_ascii_case(genre))
        .map(|(_, mn, mx)| (*mn, *mx))
        .unwrap_or(DEFAULT_BPM)
}

/// Pull the genre folder name from a path under CRATES/GENRES/<Genre>/...
fn infer_genre(path: &Path, crates_root: &Path) -> Option<String> {
    let rel = path.strip_prefix(crates_root).ok()?;
    let mut it = rel.components();
    let first = it.next()?;
    if first.as_os_str().to_str()? != "GENRES" {
        return None;
    }
    it.next()
        .and_then(|c| c.as_os_str().to_str().map(String::from))
}

#[derive(Debug, PartialEq)]
enum BpmAdjust {
    None,
    Halved(f32),
    Doubled(f32),
    Quartered(f32),
}

/// Half-BPM tolerance on the range boundary so a clean halving/doubling that
/// lands right on the edge (e.g. 135/2 = 67.5 vs a 68 min) still counts.
const BPM_TOLERANCE: f32 = 0.6;

fn in_range(v: f32, min: f32, max: f32) -> bool {
    v >= min - BPM_TOLERANCE && v <= max + BPM_TOLERANCE
}

/// If BPM falls outside the genre's expected range, try halving/doubling
/// until it does. Returns how we adjusted (if at all) for reporting.
fn normalize_for_genre(bpm: f32, genre: &str) -> BpmAdjust {
    let (min, max) = genre_bpm_range(genre);
    if bpm <= 0.0 || in_range(bpm, min, max) {
        return BpmAdjust::None;
    }
    let halved = bpm / 2.0;
    if in_range(halved, min, max) {
        return BpmAdjust::Halved(halved);
    }
    let doubled = bpm * 2.0;
    if in_range(doubled, min, max) {
        return BpmAdjust::Doubled(doubled);
    }
    let quartered = bpm / 4.0;
    if in_range(quartered, min, max) {
        return BpmAdjust::Quartered(quartered);
    }
    BpmAdjust::None
}

// ── Mood ──────────────────────────────────────────────────────────────────

fn is_vocal_genre(g: &str) -> bool {
    VOCAL_GENRES.iter().any(|v| v.eq_ignore_ascii_case(g))
}

fn is_club_hype_genre(g: &str) -> bool {
    CLUB_HYPE_GENRES.iter().any(|v| v.eq_ignore_ascii_case(g))
}

/// Decision tree for the Label = mood field. Returns None if nothing fits
/// (no guess-labels — better to leave blank than lie).
fn assign_mood(bpm: f32, genre: &str, popularity: Option<i32>) -> Option<&'static str> {
    // 1. Club hype genres always route to Hype
    if is_club_hype_genre(genre) {
        return Some("Hype");
    }

    // 2. Hip-Hop at trap-level BPM (post-normalization this is rare since
    //    trap gets halved; catches cases where BPM fix hasn't run yet).
    let hh = genre.eq_ignore_ascii_case("Hip-Hop") || genre.eq_ignore_ascii_case("Hip-Hop:Rap");
    if hh && bpm >= 125.0 {
        return Some("Hype");
    }

    // 3. Sing Along — popular + vocal-heavy genre.
    if let Some(pop) = popularity {
        if pop >= 60 && is_vocal_genre(genre) {
            return Some("Sing Along");
        }
    }

    // 4. BPM-based phase moods.
    if bpm <= 0.0 {
        return None;
    }
    let peak_genres = [
        "House", "Afro House", "Amapiano", "Afrobeats", "Dancehall",
        "Techno", "Electronic", "Moombahton", "Soca",
    ];
    let is_peak_genre = peak_genres.iter().any(|g| g.eq_ignore_ascii_case(genre));

    if bpm >= 118.0 && bpm <= 135.0 && is_peak_genre {
        return Some("Peak Hour");
    }
    if bpm >= 92.0 && bpm <= 117.0 {
        return Some("Warmup");
    }
    if bpm >= 80.0 && bpm < 92.0 {
        return Some("Slowdown");
    }
    if bpm >= 68.0 && bpm < 80.0 {
        return Some("Ending");
    }
    None
}

// ── File I/O ──────────────────────────────────────────────────────────────

fn read_bpm_artist_title(path: &Path) -> (f32, String, String) {
    // Lofty has known panic cases on malformed files. Wrap the whole read
    // in catch_unwind so one bad file can't bring down the whole pass.
    let path = path.to_path_buf();
    std::panic::catch_unwind(move || {
        match Probe::open(&path).and_then(|p| p.read()) {
            Ok(tagged) => {
                let tag = tagged.primary_tag().or(tagged.first_tag());
                let bpm = tag
                    .and_then(|t| {
                        t.get_string(&ItemKey::IntegerBpm)
                            .or_else(|| t.get_string(&ItemKey::Bpm))
                    })
                    .and_then(|s| s.parse::<f32>().ok())
                    .unwrap_or(0.0);
                let artist = tag
                    .and_then(|t| t.artist())
                    .map(|s| s.to_string())
                    .unwrap_or_default();
                let title = tag
                    .and_then(|t| t.title())
                    .map(|s| s.to_string())
                    .unwrap_or_default();
                (bpm, artist, title)
            }
            Err(_) => (0.0, String::new(), String::new()),
        }
    })
    .unwrap_or_else(|_| (0.0, String::new(), String::new()))
}

fn write_single_tag(path: &Path, key: ItemKey, value: String) -> Result<(), String> {
    // catch_unwind guards against lofty panics on certain malformed files
    // (we'd rather report an error per file than crash the whole pass).
    let path_buf = path.to_path_buf();
    let value_clone = value.clone();
    let result = std::panic::catch_unwind(move || -> Result<(), String> {
        let mut tagged = Probe::open(&path_buf)
            .map_err(|e| format!("probe: {}", e))?
            .read()
            .map_err(|e| format!("read: {}", e))?;
        if tagged.primary_tag().is_none() {
            let tag_type = tagged.primary_tag_type();
            tagged.insert_tag(Tag::new(tag_type));
        }
        let tag = tagged
            .primary_tag_mut()
            .ok_or_else(|| "no primary tag".to_string())?;
        tag.insert_text(key, value_clone);
        tagged
            .save_to_path(&path_buf, WriteOptions::default())
            .map_err(|e| format!("save: {}", e))?;
        Ok(())
    });
    match result {
        Ok(r) => r,
        Err(_) => Err(format!("panic writing tag ({})", value)),
    }
}

// ── Reports ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct BpmFixReport {
    pub scanned: usize,
    pub halved: usize,
    pub doubled: usize,
    pub quartered: usize,
    pub unchanged: usize,
    pub errors: Vec<String>,
    pub serato_db_updated: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct BpmDetectReport {
    pub scanned: usize,
    pub written: usize,
    pub skipped: usize,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AutoCueReport {
    pub scanned: usize,
    pub written: usize,
    pub skipped: usize,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ClearTagsReport {
    pub scanned: usize,
    pub cleared: usize,
    pub serato_db_updated: usize,
    pub errors: Vec<String>,
}

/// Wipe the Comment, Grouping, and Label ID3/MP4 fields on a single file.
/// Safe against lofty panics via catch_unwind.
pub(crate) fn clear_meta_tags_on_file(path: &Path) -> Result<(), String> {
    let p = path.to_path_buf();
    std::panic::catch_unwind(move || -> Result<(), String> {
        let mut tagged = Probe::open(&p)
            .map_err(|e| format!("probe: {}", e))?
            .read()
            .map_err(|e| format!("read: {}", e))?;
        let mut changed = false;
        if let Some(tag) = tagged.primary_tag_mut() {
            if tag.get_string(&ItemKey::Comment).is_some() {
                tag.remove_key(&ItemKey::Comment);
                changed = true;
            }
            if tag.get_string(&ItemKey::ContentGroup).is_some() {
                tag.remove_key(&ItemKey::ContentGroup);
                changed = true;
            }
            if tag.get_string(&ItemKey::Publisher).is_some() {
                tag.remove_key(&ItemKey::Publisher);
                changed = true;
            }
        }
        if changed {
            tagged
                .save_to_path(&p, WriteOptions::default())
                .map_err(|e| format!("save: {}", e))?;
        }
        Ok(())
    })
    .unwrap_or_else(|_| Err("panic during clear".into()))
}

#[derive(Debug, Clone, Serialize)]
pub struct MoodReport {
    pub scanned: usize,
    pub labeled: usize,
    pub blank: usize,
    pub errors: Vec<String>,
    pub by_mood: Vec<(String, usize)>,
    pub serato_db_updated: usize,
}

// ── Commands ──────────────────────────────────────────────────────────────

#[tauri::command]
pub fn normalize_bpms(
    app: AppHandle,
    crates_root: String,
    serato_path: Option<String>,
) -> Result<BpmFixReport, String> {
    let root = PathBuf::from(&crates_root);
    let genres_dir = root.join("GENRES");
    if !genres_dir.exists() {
        return Err(format!("GENRES folder not found under {}", crates_root));
    }

    let files: Vec<PathBuf> = WalkDir::new(&genres_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file() && is_audio(e.path()))
        .map(|e| e.path().to_path_buf())
        .collect();
    let total = files.len();
    emit_progress(Some(&app), "bpm_start", 0, total, "Scanning for BPM fixes");

    let mut report = BpmFixReport {
        scanned: 0,
        halved: 0,
        doubled: 0,
        quartered: 0,
        unchanged: 0,
        errors: Vec::new(),
        serato_db_updated: 0,
    };

    // Collect per-file BPM updates for Serato's db V2 so Serato sees the
    // new values on its next launch without the user having to "Read Tags".
    let mut db_updates: HashMap<String, SeratoDbUpdate> = HashMap::new();

    for (idx, path) in files.iter().enumerate() {
        report.scanned += 1;
        let genre = infer_genre(path, &root).unwrap_or_default();
        let (bpm, _, _) = read_bpm_artist_title(path);
        let adjust = normalize_for_genre(bpm, &genre);
        let new_bpm = match adjust {
            BpmAdjust::None => {
                report.unchanged += 1;
                None
            }
            BpmAdjust::Halved(v) => {
                report.halved += 1;
                Some(v)
            }
            BpmAdjust::Doubled(v) => {
                report.doubled += 1;
                Some(v)
            }
            BpmAdjust::Quartered(v) => {
                report.quartered += 1;
                Some(v)
            }
        };
        if let Some(v) = new_bpm {
            let formatted = format!("{:.2}", v);
            if let Err(e) = write_single_tag(path, ItemKey::IntegerBpm, formatted.clone()) {
                report.errors.push(format!("{}: {}", path.display(), e));
            } else {
                db_updates.insert(
                    to_serato_db_path(path),
                    SeratoDbUpdate {
                        tbpm: Some(formatted),
                        ..Default::default()
                    },
                );
            }
        }
        if (idx + 1) % 200 == 0 || idx + 1 == total {
            emit_progress(
                Some(&app),                "bpm_fixing",
                idx + 1,
                total,
                &format!(
                    "halved {} · doubled {} · unchanged {}",
                    report.halved, report.doubled, report.unchanged
                ),
            );
        }
    }

    if let Some(sp) = serato_path.as_deref() {
        let db_path = PathBuf::from(sp).join("database V2");
        if db_path.exists() && !db_updates.is_empty() {
            emit_progress(Some(&app), "bpm_db_update", 0, 0, "Updating Serato database V2");
            match update_serato_db(&db_path, &db_updates) {
                Ok(n) => report.serato_db_updated = n,
                Err(e) => report.errors.push(format!("serato db update: {}", e)),
            }
        }
    }

    emit_progress(Some(&app), "bpm_done", total, total, "BPM pass complete");
    Ok(report)
}

#[tauri::command]
pub fn label_moods(
    app: AppHandle,
    crates_root: String,
    serato_path: Option<String>,
) -> Result<MoodReport, String> {
    let root = PathBuf::from(&crates_root);
    let genres_dir = root.join("GENRES");
    if !genres_dir.exists() {
        return Err(format!("GENRES folder not found under {}", crates_root));
    }

    let files: Vec<PathBuf> = WalkDir::new(&genres_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file() && is_audio(e.path()))
        .map(|e| e.path().to_path_buf())
        .collect();
    let total = files.len();

    emit_progress(Some(&app), "mood_start", 0, total, "Loading Spotify popularity cache");
    let popularity = load_spotify_cache();
    emit_progress(
        Some(&app),        "mood_start",
        0,
        total,
        &format!("Cache: {} tracks · Labeling {} files", popularity.len(), total),
    );

    let mut report = MoodReport {
        scanned: 0,
        labeled: 0,
        blank: 0,
        errors: Vec::new(),
        by_mood: Vec::new(),
        serato_db_updated: 0,
    };
    let mut counts: HashMap<String, usize> = Default::default();
    let mut db_updates: HashMap<String, SeratoDbUpdate> = HashMap::new();

    for (idx, path) in files.iter().enumerate() {
        report.scanned += 1;
        let genre = infer_genre(path, &root).unwrap_or_default();
        let (bpm, artist, title) = read_bpm_artist_title(path);
        let pop = if !artist.is_empty() && !title.is_empty() {
            popularity.get(&spotify_key(&artist, &title)).copied()
        } else {
            None
        };
        match assign_mood(bpm, &genre, pop) {
            Some(mood) => {
                report.labeled += 1;
                *counts.entry(mood.to_string()).or_insert(0) += 1;
                if let Err(e) = write_single_tag(path, ItemKey::Publisher, mood.to_string()) {
                    report.errors.push(format!("{}: {}", path.display(), e));
                } else {
                    db_updates.insert(
                        to_serato_db_path(path),
                        SeratoDbUpdate {
                            tlbl: Some(mood.to_string()),
                            ..Default::default()
                        },
                    );
                }
            }
            None => {
                report.blank += 1;
            }
        }
        if (idx + 1) % 200 == 0 || idx + 1 == total {
            emit_progress(
                Some(&app),                "mood_labeling",
                idx + 1,
                total,
                &format!("{} labeled · {} blank", report.labeled, report.blank),
            );
        }
    }

    if let Some(sp) = serato_path.as_deref() {
        let db_path = PathBuf::from(sp).join("database V2");
        if db_path.exists() && !db_updates.is_empty() {
            emit_progress(Some(&app), "mood_db_update", 0, 0, "Updating Serato database V2");
            match update_serato_db(&db_path, &db_updates) {
                Ok(n) => report.serato_db_updated = n,
                Err(e) => report.errors.push(format!("serato db update: {}", e)),
            }
        }
    }

    report.by_mood = counts.into_iter().collect();
    report.by_mood.sort_by(|a, b| b.1.cmp(&a.1));
    emit_progress(Some(&app), "mood_done", total, total, "Mood pass complete");
    Ok(report)
}

// ── Clear Tags + Sanitize commands ────────────────────────────────────────

#[tauri::command]
pub fn clear_tags_library(
    app: AppHandle,
    crates_root: String,
    serato_path: Option<String>,
) -> Result<ClearTagsReport, String> {
    let root = PathBuf::from(&crates_root);
    let genres = root.join("GENRES");
    if !genres.is_dir() {
        return Err(format!("GENRES folder not found under {}", crates_root));
    }
    let files: Vec<PathBuf> = WalkDir::new(&genres)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file() && is_audio(e.path()))
        .map(|e| e.path().to_path_buf())
        .collect();
    let total = files.len();
    emit_progress(Some(&app), "clear_start", 0, total, "Clearing Comment/Grouping/Label");

    let mut report = ClearTagsReport {
        scanned: 0,
        cleared: 0,
        serato_db_updated: 0,
        errors: Vec::new(),
    };
    let mut db_updates: HashMap<String, SeratoDbUpdate> = HashMap::new();

    for (idx, p) in files.iter().enumerate() {
        report.scanned += 1;
        match clear_meta_tags_on_file(p) {
            Ok(_) => {
                report.cleared += 1;
                // Write empty tlbl / tgrp / tcmt into Serato's db so the
                // UI doesn't keep showing cached values after we wipe the
                // file-level ID3 tags.
                db_updates.insert(
                    to_serato_db_path(p),
                    SeratoDbUpdate {
                        tlbl: Some(String::new()),
                        tgrp: Some(String::new()),
                        tcmt: Some(String::new()),
                        ..Default::default()
                    },
                );
            }
            Err(e) => report.errors.push(format!("{}: {}", p.display(), e)),
        }
        if (idx + 1) % 100 == 0 || idx + 1 == total {
            emit_progress(
                Some(&app),                "clearing",
                idx + 1,
                total,
                &format!("{} cleared", report.cleared),
            );
        }
    }

    if let Some(sp) = serato_path.as_deref() {
        let db_path = PathBuf::from(sp).join("database V2");
        if db_path.exists() && !db_updates.is_empty() {
            emit_progress(Some(&app), "clear_db", 0, 0, "Clearing fields in Serato database V2");
            match update_serato_db(&db_path, &db_updates) {
                Ok(n) => report.serato_db_updated = n,
                Err(e) => report.errors.push(format!("serato db: {}", e)),
            }
        }
    }

    emit_progress(Some(&app), "clear_done", total, total, "Tag clear complete");
    Ok(report)
}

#[derive(Debug, Clone, Serialize)]
pub struct SanitizeReport {
    pub scanned: usize,
    pub updated: usize,
    pub comments_written: usize,
    pub serato_db_updated: usize,
    pub errors: Vec<String>,
}

fn artist_genres_cache_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_default();
    PathBuf::from(home).join(".dj-crates-tools").join("spotify_artist_genres.json")
}

fn load_artist_genres_cache() -> std::collections::HashMap<String, Vec<String>> {
    let path = artist_genres_cache_path();
    if !path.exists() {
        return Default::default();
    }
    std::fs::read(&path)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

fn save_artist_genres_cache(cache: &std::collections::HashMap<String, Vec<String>>) {
    let path = artist_genres_cache_path();
    if let Some(p) = path.parent() {
        let _ = std::fs::create_dir_all(p);
    }
    if let Ok(json) = serde_json::to_vec_pretty(cache) {
        let _ = std::fs::write(&path, json);
    }
}

fn norm_artist_key(s: &str) -> String {
    s.to_lowercase()
        .replace(['-', '_'], " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Result of running write_sanitized_tags on one file.
struct SanitizeWriteResult {
    changed: bool,
    normalized_year: Option<String>,
    artist: String,
    wrote_comment: bool,
}

/// Write genre + grouping + (normalized) year + optionally Comment=subgenre-
/// array to the file. If `comment_override` is Some, the Comment field is
/// replaced with that string; if None, Comment is left untouched.
fn write_sanitized_tags(
    path: &Path,
    genre: &str,
    grouping: &str,
    comment_override: Option<&str>,
) -> Result<SanitizeWriteResult, String> {
    let p = path.to_path_buf();
    let genre_s = genre.to_string();
    let grouping_s = grouping.to_string();
    let comment_s = comment_override.map(|s| s.to_string());
    std::panic::catch_unwind(move || -> Result<SanitizeWriteResult, String> {
        let mut tagged = Probe::open(&p)
            .map_err(|e| format!("probe: {}", e))?
            .read()
            .map_err(|e| format!("read: {}", e))?;
        if tagged.primary_tag().is_none() {
            let tag_type = tagged.primary_tag_type();
            tagged.insert_tag(Tag::new(tag_type));
        }
        let tag = tagged
            .primary_tag_mut()
            .ok_or_else(|| "no primary tag".to_string())?;
        let mut changed = false;
        let mut normalized_year: Option<String> = None;
        let mut wrote_comment = false;

        let artist_name = tag.artist().map(|s| s.to_string()).unwrap_or_default();

        // Genre
        let current_genre = tag.genre().map(|s| s.to_string()).unwrap_or_default();
        if current_genre != genre_s {
            tag.set_genre(genre_s);
            changed = true;
        }

        // Grouping
        let current_grp = tag
            .get_string(&ItemKey::ContentGroup)
            .map(|s| s.to_string())
            .unwrap_or_default();
        if current_grp != grouping_s {
            if grouping_s.is_empty() {
                tag.remove_key(&ItemKey::ContentGroup);
            } else {
                tag.insert_text(ItemKey::ContentGroup, grouping_s);
            }
            changed = true;
        }

        // Year — Deezer writes things like "2000-01-01", "2000-01".
        let raw_year = tag
            .get_string(&ItemKey::Year)
            .or_else(|| tag.get_string(&ItemKey::RecordingDate))
            .map(|s| s.to_string())
            .unwrap_or_default();
        if !raw_year.is_empty() {
            let yr: String = raw_year
                .chars()
                .take_while(|c| c.is_ascii_digit())
                .take(4)
                .collect();
            if yr.len() == 4 && yr != raw_year {
                tag.insert_text(ItemKey::Year, yr.clone());
                tag.remove_key(&ItemKey::RecordingDate);
                normalized_year = Some(yr);
                changed = true;
            }
        }

        // Comment = Spotify subgenre array (optional override).
        if let Some(new_comment) = comment_s {
            let current = tag
                .get_string(&ItemKey::Comment)
                .map(|s| s.to_string())
                .unwrap_or_default();
            if current != new_comment {
                if new_comment.is_empty() {
                    tag.remove_key(&ItemKey::Comment);
                } else {
                    tag.insert_text(ItemKey::Comment, new_comment);
                }
                changed = true;
                wrote_comment = true;
            }
        }

        if changed {
            tagged
                .save_to_path(&p, WriteOptions::default())
                .map_err(|e| format!("save: {}", e))?;
        }
        Ok(SanitizeWriteResult {
            changed,
            normalized_year,
            artist: artist_name,
            wrote_comment,
        })
    })
    .unwrap_or_else(|_| {
        Err("panic during sanitize".into())
    })
}

#[tauri::command]
pub fn sanitize_library(
    app: AppHandle,
    crates_root: String,
    serato_path: Option<String>,
    spotify_client_id: Option<String>,
    spotify_client_secret: Option<String>,
) -> Result<SanitizeReport, String> {
    let root = PathBuf::from(&crates_root);
    let genres = root.join("GENRES");
    if !genres.is_dir() {
        return Err(format!("GENRES folder not found under {}", crates_root));
    }
    let files: Vec<PathBuf> = WalkDir::new(&genres)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file() && is_audio(e.path()))
        .map(|e| e.path().to_path_buf())
        .collect();
    let total = files.len();
    emit_progress(Some(&app), "sanitize_start", 0, total, "Sanitizing metadata");

    // Optional Spotify lookups for the Comment = subgenre array feature.
    let mut spotify = match (spotify_client_id, spotify_client_secret) {
        (Some(id), Some(secret)) => SpotifyClient::new(id, secret),
        _ => None,
    };
    // Persistent cache of artist → [genres] so we only hit Spotify once per
    // unique artist across all runs.
    let mut artist_cache = load_artist_genres_cache();
    let cache_size_before = artist_cache.len();

    let mut db_updates: HashMap<String, SeratoDbUpdate> = HashMap::new();
    let mut report = SanitizeReport {
        scanned: 0,
        updated: 0,
        comments_written: 0,
        serato_db_updated: 0,
        errors: Vec::new(),
    };

    for (idx, p) in files.iter().enumerate() {
        report.scanned += 1;
        let rel = match p.strip_prefix(&genres) {
            Ok(r) => r,
            Err(_) => continue,
        };
        let parts: Vec<String> = rel
            .iter()
            .filter_map(|s| s.to_str().map(|x| x.to_string()))
            .collect();
        if parts.len() < 2 {
            continue; // loose file at GENRES/ root — no genre
        }
        let genre_tag = parts[0].replace(':', "/");
        let grouping_tag = if parts.len() >= 3 {
            parts[parts.len() - 2].clone()
        } else {
            String::new()
        };

        // First pass: read artist from the file so we can look up its Spotify
        // genres. (write_sanitized_tags returns the artist, but we want it
        // beforehand to decide the comment override.)
        let artist = match Probe::open(p).and_then(|pb| pb.read()) {
            Ok(tf) => tf
                .primary_tag()
                .or(tf.first_tag())
                .and_then(|t| t.artist())
                .map(|s| s.to_string())
                .unwrap_or_default(),
            Err(_) => String::new(),
        };

        // Resolve this artist's genre array (via cache → Spotify API)
        let mut comment_override: Option<String> = None;
        if !artist.is_empty() {
            let key = norm_artist_key(&artist);
            if !artist_cache.contains_key(&key) {
                if let Some(sp) = spotify.as_mut() {
                    match sp.search_artist(&artist) {
                        Ok(Some(a)) => {
                            artist_cache.insert(key.clone(), a.genres);
                        }
                        _ => {
                            // Cache the miss so we don't retry every run
                            artist_cache.insert(key.clone(), Vec::new());
                        }
                    }
                }
            }
            if let Some(genres) = artist_cache.get(&key) {
                if !genres.is_empty() {
                    comment_override = Some(genres.join(" | "));
                }
            }
        }

        match write_sanitized_tags(p, &genre_tag, &grouping_tag, comment_override.as_deref()) {
            Ok(res) => {
                if res.changed {
                    report.updated += 1;
                    db_updates.insert(
                        to_serato_db_path(p),
                        SeratoDbUpdate {
                            tgen: Some(genre_tag.clone()),
                            tgrp: Some(grouping_tag.clone()),
                            ttyr: res.normalized_year,
                            // Push the subgenre array into Serato's DB
                            // comment column too, not just the file's ID3
                            // tag — Serato reads the DB for its track list.
                            tcmt: if res.wrote_comment {
                                comment_override.clone()
                            } else {
                                None
                            },
                            ..Default::default()
                        },
                    );
                }
                if res.wrote_comment {
                    report.comments_written += 1;
                }
            }
            Err(e) => report.errors.push(format!("{}: {}", p.display(), e)),
        }

        if (idx + 1) % 100 == 0 || idx + 1 == total {
            emit_progress(
                Some(&app),                "sanitizing",
                idx + 1,
                total,
                &format!(
                    "{} updated · {} comments · cache {}",
                    report.updated,
                    report.comments_written,
                    artist_cache.len()
                ),
            );
            // Periodic cache flush so long runs don't lose state on ctrl-C
            if artist_cache.len() > cache_size_before
                && ((idx + 1) % 500 == 0 || idx + 1 == total)
            {
                save_artist_genres_cache(&artist_cache);
            }
        }
    }

    save_artist_genres_cache(&artist_cache);

    if let Some(sp) = serato_path.as_deref() {
        let db_path = PathBuf::from(sp).join("database V2");
        if db_path.exists() && !db_updates.is_empty() {
            emit_progress(Some(&app), "sanitize_db", 0, 0, "Updating Serato database V2");
            match update_serato_db(&db_path, &db_updates) {
                Ok(n) => report.serato_db_updated = n,
                Err(e) => report.errors.push(format!("serato db: {}", e)),
            }
        }
    }

    emit_progress(Some(&app), "sanitize_done", total, total, "Sanitize complete");
    Ok(report)
}

// ── Audio-based BPM detection (shells out to librosa) ────────────────────

/// Kick off `scripts/detect_bpms.py` in the project venv and stream its
/// line-oriented progress back to the frontend as `sync:progress` events.
/// The Python script does HPSS + tempo detection with a genre-aware prior
/// so hi-hats don't fool the detector into doubletime.
#[tauri::command]
pub fn detect_bpms(
    app: AppHandle,
    crates_root: String,
    python_venv: Option<String>,
    scripts_dir: Option<String>,
    limit: Option<u32>,
    force: Option<bool>,
) -> Result<BpmDetectReport, String> {
    // Default to the project venv + scripts dir; Settings can override.
    let default_project = "/Users/koryjcampbell/Projects/dj-crates-tools";
    let python = python_venv
        .as_deref()
        .filter(|s| !s.is_empty())
        .map(|v| format!("{}/bin/python3", v.trim_end_matches('/')))
        .unwrap_or_else(|| format!("{}/.venv/bin/python3", default_project));
    let scripts = scripts_dir
        .as_deref()
        .filter(|s| !s.is_empty())
        .unwrap_or(default_project)
        .to_string();
    let script_path = format!("{}/scripts/detect_bpms.py", scripts.trim_end_matches('/'));

    if !Path::new(&python).exists() {
        return Err(format!("Python not found at {}. Set Python Venv in Settings.", python));
    }
    if !Path::new(&script_path).exists() {
        return Err(format!(
            "detect_bpms.py not found at {}. Set Scripts Dir in Settings.",
            script_path
        ));
    }

    let mut cmd = Command::new(&python);
    cmd.arg(&script_path)
        .arg("--crates-root")
        .arg(&crates_root);
    if let Some(n) = limit {
        cmd.arg("--limit").arg(n.to_string());
    }
    if force.unwrap_or(false) {
        cmd.arg("--force");
    }
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

    let mut child = cmd.spawn().map_err(|e| format!("spawn python: {}", e))?;
    let stdout = child.stdout.take().ok_or_else(|| "no stdout pipe".to_string())?;
    let reader = BufReader::new(stdout);

    let mut report = BpmDetectReport {
        scanned: 0,
        written: 0,
        skipped: 0,
        errors: Vec::new(),
    };

    emit_progress(Some(&app), "bpm_detect_start", 0, 0, "Starting librosa analysis");

    for line in reader.lines().map_while(Result::ok) {
        let parts: Vec<&str> = line.split('\t').collect();
        match parts.first().copied() {
            Some("PROGRESS") => {
                let cur = parts.get(1).and_then(|s| s.parse::<usize>().ok()).unwrap_or(0);
                let tot = parts.get(2).and_then(|s| s.parse::<usize>().ok()).unwrap_or(0);
                let msg = parts.get(3).unwrap_or(&"");
                emit_progress(Some(&app), "bpm_detecting", cur, tot, msg);
            }
            Some("OK") => {
                // Final DONE line carries authoritative counts; OK just flows live.
            }
            Some("SKIP") => {}
            Some("ERR") => {
                if let Some(p) = parts.get(1) {
                    let reason = parts.get(2).unwrap_or(&"");
                    report.errors.push(format!("{}: {}", p, reason));
                }
            }
            Some("DONE") => {
                report.scanned = parts.get(1).and_then(|s| s.parse::<usize>().ok()).unwrap_or(0);
                report.written = parts.get(2).and_then(|s| s.parse::<usize>().ok()).unwrap_or(0);
                report.skipped = parts.get(3).and_then(|s| s.parse::<usize>().ok()).unwrap_or(0);
            }
            _ => {}
        }
    }

    let status = child.wait().map_err(|e| format!("wait: {}", e))?;
    if !status.success() {
        let code = status.code().map(|c| c.to_string()).unwrap_or_else(|| "?".to_string());
        return Err(format!("python exited with code {}", code));
    }

    emit_progress(
        Some(&app),        "bpm_detect_done",
        report.scanned,
        report.scanned,
        "Audio BPM detection complete",
    );
    Ok(report)
}

// ── Auto Cue (shells out to librosa + Serato Markers2 writer) ────────────

#[tauri::command]
pub fn auto_cue_library(
    app: AppHandle,
    crates_root: String,
    python_venv: Option<String>,
    scripts_dir: Option<String>,
    bars_per_cue: Option<u32>,
    num_cues: Option<u32>,
    cue_index: Option<u32>,
    force: Option<bool>,
) -> Result<AutoCueReport, String> {
    let default_project = "/Users/koryjcampbell/Projects/dj-crates-tools";
    let python = python_venv
        .as_deref()
        .filter(|s| !s.is_empty())
        .map(|v| format!("{}/bin/python3", v.trim_end_matches('/')))
        .unwrap_or_else(|| format!("{}/.venv/bin/python3", default_project));
    let scripts = scripts_dir
        .as_deref()
        .filter(|s| !s.is_empty())
        .unwrap_or(default_project)
        .to_string();
    let script_path = format!("{}/scripts/autocue.py", scripts.trim_end_matches('/'));

    if !Path::new(&python).exists() {
        return Err(format!("Python not found at {}.", python));
    }
    if !Path::new(&script_path).exists() {
        return Err(format!("autocue.py not found at {}", script_path));
    }

    let bars = bars_per_cue.unwrap_or(16).to_string();
    let cues = num_cues.unwrap_or(1).to_string();
    let idx = cue_index.unwrap_or(7).to_string();

    let mut cmd = Command::new(&python);
    cmd.arg(&script_path)
        .arg("--crates-root")
        .arg(&crates_root)
        .arg("--bars")
        .arg(&bars)
        .arg("--cues")
        .arg(&cues)
        .arg("--cue-index")
        .arg(&idx);
    if force.unwrap_or(false) {
        cmd.arg("--force");
    }
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

    let mut child = cmd.spawn().map_err(|e| format!("spawn python: {}", e))?;
    let stdout = child.stdout.take().ok_or_else(|| "no stdout".to_string())?;
    let stderr = child.stderr.take();
    let reader = BufReader::new(stdout);

    // Drain stderr in a background thread so it's there if we need it on failure
    let stderr_handle = stderr.map(|s| {
        std::thread::spawn(move || {
            use std::io::Read;
            let mut buf = String::new();
            let _ = BufReader::new(s).read_to_string(&mut buf);
            buf
        })
    });

    let mut report = AutoCueReport {
        scanned: 0,
        written: 0,
        skipped: 0,
        errors: Vec::new(),
    };

    emit_progress(Some(&app), "autocue_start", 0, 0, "Starting auto-cue analysis");

    for line in reader.lines().map_while(Result::ok) {
        let parts: Vec<&str> = line.split('\t').collect();
        match parts.first().copied() {
            Some("PROGRESS") => {
                let cur = parts.get(1).and_then(|s| s.parse::<usize>().ok()).unwrap_or(0);
                let tot = parts.get(2).and_then(|s| s.parse::<usize>().ok()).unwrap_or(0);
                let msg = parts.get(3).unwrap_or(&"");
                emit_progress(Some(&app), "autocue_working", cur, tot, msg);
            }
            Some("OK") => {}
            Some("SKIP") => {}
            Some("ERR") => {
                if let Some(p) = parts.get(1) {
                    let r = parts.get(2).unwrap_or(&"");
                    report.errors.push(format!("{}: {}", p, r));
                }
            }
            Some("DONE") => {
                report.scanned =
                    parts.get(1).and_then(|s| s.parse::<usize>().ok()).unwrap_or(0);
                report.written =
                    parts.get(2).and_then(|s| s.parse::<usize>().ok()).unwrap_or(0);
                report.skipped =
                    parts.get(3).and_then(|s| s.parse::<usize>().ok()).unwrap_or(0);
            }
            _ => {}
        }
    }

    let status = child.wait().map_err(|e| format!("wait: {}", e))?;
    let stderr_out = stderr_handle
        .and_then(|h| h.join().ok())
        .unwrap_or_default();
    if !status.success() {
        let code = status.code().map(|c| c.to_string()).unwrap_or_else(|| "?".into());
        // Surface the last few lines of Python stderr so the summary is debuggable
        let tail: String = stderr_out
            .lines()
            .filter(|l| !l.trim().is_empty())
            .rev()
            .take(3)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<Vec<_>>()
            .join(" | ");
        return Err(format!("python exited with code {} — {}", code, tail));
    }

    emit_progress(Some(&app), "autocue_done", report.scanned, report.scanned, "Auto-cue complete");
    Ok(report)
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rnb_at_135_halves_to_67_5() {
        let r = normalize_for_genre(135.0, "R&B");
        assert_eq!(r, BpmAdjust::Halved(67.5));
    }

    #[test]
    fn house_in_range_untouched() {
        let r = normalize_for_genre(124.0, "House");
        assert_eq!(r, BpmAdjust::None);
    }

    #[test]
    fn hiphop_trap_at_140_halves_to_70() {
        let r = normalize_for_genre(140.0, "Hip-Hop");
        assert_eq!(r, BpmAdjust::Halved(70.0));
    }

    #[test]
    fn house_at_60_doubles_to_120() {
        let r = normalize_for_genre(60.0, "House");
        assert_eq!(r, BpmAdjust::Doubled(120.0));
    }

    #[test]
    fn zero_bpm_is_no_op() {
        assert_eq!(normalize_for_genre(0.0, "R&B"), BpmAdjust::None);
    }

    #[test]
    fn extreme_bpm_quarters() {
        // 280 BPM R&B track → 70.0 via quartering
        let r = normalize_for_genre(280.0, "R&B");
        assert_eq!(r, BpmAdjust::Quartered(70.0));
    }

    #[test]
    fn jersey_club_is_hype_regardless_of_bpm() {
        assert_eq!(assign_mood(100.0, "Jersey Club", None), Some("Hype"));
        assert_eq!(assign_mood(132.0, "Baltimore Club", None), Some("Hype"));
    }

    #[test]
    fn popular_rnb_is_sing_along() {
        assert_eq!(assign_mood(95.0, "R&B", Some(75)), Some("Sing Along"));
    }

    #[test]
    fn unpopular_rnb_falls_to_bpm_bucket() {
        assert_eq!(assign_mood(70.0, "R&B", Some(10)), Some("Ending"));
        assert_eq!(assign_mood(85.0, "R&B", Some(10)), Some("Slowdown"));
        assert_eq!(assign_mood(100.0, "R&B", Some(10)), Some("Warmup"));
    }

    #[test]
    fn house_at_peak_bpm_is_peak_hour() {
        assert_eq!(assign_mood(124.0, "House", None), Some("Peak Hour"));
        assert_eq!(assign_mood(120.0, "Amapiano", None), Some("Peak Hour"));
    }

    #[test]
    fn ending_is_slowest_bucket() {
        assert_eq!(assign_mood(72.0, "Soul", None), Some("Ending"));
        assert_eq!(assign_mood(68.0, "Neo-Soul", None), Some("Ending"));
    }

    #[test]
    fn popularity_doesnt_apply_to_club_genres() {
        // Even a 'popular' jersey club track stays Hype, not Sing Along —
        // club hype genres are checked first.
        assert_eq!(assign_mood(130.0, "Jersey Club", Some(80)), Some("Hype"));
    }

    #[test]
    fn blank_when_nothing_fits() {
        // BPM 0 + unknown genre + no popularity → nothing to label
        assert_eq!(assign_mood(0.0, "", None), None);
    }

    #[test]
    fn infer_genre_reads_folder_level() {
        let root = PathBuf::from("/music/CRATES");
        let p = PathBuf::from("/music/CRATES/GENRES/R&B/EDITS/foo.mp3");
        assert_eq!(infer_genre(&p, &root).as_deref(), Some("R&B"));
    }

    #[test]
    fn infer_genre_returns_none_for_non_genres_paths() {
        let root = PathBuf::from("/music/CRATES");
        let p = PathBuf::from("/music/CRATES/PLAYLISTS/foo.mp3");
        assert_eq!(infer_genre(&p, &root), None);
    }
}
