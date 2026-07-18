use base64::{engine::general_purpose, Engine as _};
use lofty::prelude::*;
use lofty::probe::Probe;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter};
use walkdir::WalkDir;

mod cleanup;
mod convert;
mod dupes;
mod headless;
mod normalize;
mod prune;
mod safety;
mod spotify;
mod tagging;
mod unsorted;
use unsorted::UnsortedTrack;

const AUDIO_EXTENSIONS: &[&str] = &[
    "mp3", "flac", "wav", "ogg", "aif", "aiff", "aac", "m4a", "alac",
];

const BPM_MIN: f64 = 65.0;
const BPM_MAX: f64 = 135.0;

// Auto-generated "fresh arrivals" crates, rebuilt on every sync. They live
// under the PLAYLISTS%% prefix so the clean pass removes stale ones for free.
const FRESH_WEEK_CRATE: &str = "PLAYLISTS%%New This Week";
const FRESH_MONTH_CRATE: &str = "PLAYLISTS%%New This Month";
const FRESH_WEEK_DAYS: u64 = 7;
const FRESH_MONTH_DAYS: u64 = 30;

// Subgenres where BPM should be capped at 90
const LOW_BPM_SUBGENRES: &[&str] = &[
    "Trap",
    "General Trap",
    "Trap Soul",
    "Southern",
    "Crunk",
    "Cloud Rap",
    "Chopped & Screwed",
];

// ── Types ──────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ScanResult {
    pub total_files: usize,
    pub genres: Vec<GenreInfo>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GenreInfo {
    pub name: String,
    pub track_count: usize,
    pub subgenres: Vec<SubgenreInfo>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SubgenreInfo {
    pub name: String,
    pub track_count: usize,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TopLevelCrate {
    pub name: String,
    pub track_count: usize,
    pub subcrate_count: usize,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PythonStepResult {
    pub success: bool,
    pub output: String,
    pub error: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TrackMeta {
    serato_path: String,
    abs_path: PathBuf,
    title: String,
    artist: String,
    album: String,
    genre: String,
    subgenre: String,
    year: String,
    bpm: f64,
    key: String,
    duration_secs: f64,
    file_size: u64,
    bitrate: u32,
    sample_rate: u32,
    track_number: u32,
    file_type: String,
    /// Hierarchy path parts (e.g. ["GENRES", "Hip-Hop:Rap", "Boom Bap", "1990s"])
    #[serde(default)]
    hierarchy: Vec<String>,
    /// Spotify popularity 0-100. Populated by enrich_spotify_popularity.
    #[serde(default)]
    popularity: Option<i32>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SyncResult {
    pub preview: bool,
    pub crates_to_create: Vec<String>,
    pub crates_to_delete: Vec<String>,
    pub crates_written: usize,
    pub total_track_entries: usize,
    pub bpm_adjusted: usize,
    pub database_tracks_new: usize,
    pub energy_analyzed: usize,
    pub top_level: Vec<TopLevelCrate>,
    pub unsorted: Vec<UnsortedTrack>,
    pub new_this_week: usize,
    pub new_this_month: usize,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ProgressPayload {
    pub stage: String,
    pub current: usize,
    pub total: usize,
    pub message: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct SmartCrateRules {
    pub year_min: Option<i32>,
    pub year_max: Option<i32>,
    pub bpm_min: Option<f64>,
    pub bpm_max: Option<f64>,
    /// OR match: any of these genres (case-insensitive substring)
    #[serde(default)]
    pub genres: Vec<String>,
    /// OR match: any of these subgenres (case-insensitive substring)
    #[serde(default)]
    pub subgenres: Vec<String>,
    /// OR match: artist contains any
    #[serde(default)]
    pub artists: Vec<String>,
    /// Title contains
    pub title_contains: Option<String>,
    /// Spotify popularity minimum (0-100)
    pub popularity_min: Option<i32>,
    /// Spotify popularity maximum (0-100)
    pub popularity_max: Option<i32>,
    /// Cap the number of tracks included
    pub limit: Option<usize>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SmartCrateResult {
    pub preview: bool,
    pub crate_name: String,
    pub matched_tracks: usize,
    pub sample_tracks: Vec<String>,
}

// ── Helpers ────────────────────────────────────────────────────────────────

fn is_audio_file(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| AUDIO_EXTENSIONS.contains(&e.to_lowercase().as_str()))
        .unwrap_or(false)
}

fn to_serato_path(filepath: &Path) -> String {
    let s = filepath.to_string_lossy();
    s.trim_start_matches('/').to_string()
}

/// Was `t` within the last `days` days of `now`? Timestamps in the future
/// (clock skew, restored backups) count as fresh rather than being dropped.
fn is_within_days(t: SystemTime, now: SystemTime, days: u64) -> bool {
    now.duration_since(t)
        .map(|age| age.as_secs() <= days * 86_400)
        .unwrap_or(true)
}

/// When was this file added to the library? Prefers creation time (APFS
/// birthtime — survives tag rewrites, which bump mtime) and falls back to
/// mtime on filesystems without it.
fn file_added_time(meta: &fs::Metadata) -> Option<SystemTime> {
    meta.created().or_else(|_| meta.modified()).ok()
}

/// Normalize BPM to the 68-135 range.
/// For low-BPM subgenres (Trap, etc.), cap at 90.
fn normalize_bpm(bpm: f64, subgenre: &str) -> f64 {
    if bpm <= 0.0 {
        return 0.0;
    }

    let mut normalized = bpm;

    // Half if over max, double if under min
    while normalized > BPM_MAX {
        normalized /= 2.0;
    }
    while normalized < BPM_MIN {
        normalized *= 2.0;
    }

    // For trap/southern hip-hop subgenres, BPM should be in the lower range
    let is_low_bpm_genre = LOW_BPM_SUBGENRES
        .iter()
        .any(|s| subgenre.eq_ignore_ascii_case(s));

    if is_low_bpm_genre && normalized > 90.0 {
        // These genres are typically felt at half time
        let halved = normalized / 2.0;
        if halved >= BPM_MIN {
            normalized = halved;
        }
    }

    // Round to 2 decimal places
    (normalized * 100.0).round() / 100.0
}

/// Read ID3/metadata from an audio file using lofty
fn read_track_meta(path: &Path, genre_from_path: &str, subgenre_from_path: &str) -> TrackMeta {
    let serato_path = to_serato_path(path);
    let file_size = fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    let mut meta = TrackMeta {
        serato_path,
        abs_path: path.to_path_buf(),
        title: String::new(),
        artist: String::new(),
        album: String::new(),
        genre: genre_from_path.to_string(),
        subgenre: subgenre_from_path.to_string(),
        year: String::new(),
        bpm: 0.0,
        key: String::new(),
        duration_secs: 0.0,
        hierarchy: Vec::new(),
        popularity: None,
        file_size,
        bitrate: 0,
        sample_rate: 0,
        track_number: 0,
        file_type: ext.clone(),
    };

    let tagged = match Probe::open(path).and_then(|p| p.read()) {
        Ok(t) => t,
        Err(_) => return meta,
    };

    // Get properties
    let props = tagged.properties();
    meta.duration_secs = props.duration().as_secs_f64();
    meta.bitrate = props.audio_bitrate().unwrap_or(0);
    meta.sample_rate = props.sample_rate().unwrap_or(0);

    // Get tags (try primary tag first)
    if let Some(tag) = tagged.primary_tag().or_else(|| tagged.first_tag()) {
        meta.title = tag.title().unwrap_or_default().to_string();
        meta.artist = tag.artist().unwrap_or_default().to_string();
        meta.album = tag.album().unwrap_or_default().to_string();
        meta.year = tag.year().map(|y| y.to_string()).unwrap_or_default();
        meta.track_number = tag.track().unwrap_or(0);

        // Try to get genre from tag
        if let Some(g) = tag.genre() {
            if meta.genre.is_empty() {
                meta.genre = g.to_string();
            }
        }

        // BPM — lofty maps TBPM/tmpo to IntegerBpm, not Bpm. Try both.
        let bpm_str = tag
            .get_string(&lofty::tag::ItemKey::IntegerBpm)
            .or_else(|| tag.get_string(&lofty::tag::ItemKey::Bpm));
        if let Some(bpm_item) = bpm_str {
            if let Ok(bpm) = bpm_item.parse::<f64>() {
                meta.bpm = bpm;
            }
        }

        // Key
        if let Some(key) = tag.get_string(&lofty::tag::ItemKey::InitialKey) {
            meta.key = key.to_string();
        }
    }

    // Normalize BPM
    if meta.bpm > 0.0 {
        meta.bpm = normalize_bpm(meta.bpm, &meta.subgenre);
    }

    meta
}

// ── Serato Binary Format ───────────────────────────────────────────────────

pub(crate) fn encode_utf16be(text: &str) -> Vec<u8> {
    let mut bytes = Vec::new();
    for c in text.encode_utf16() {
        bytes.push((c >> 8) as u8);
        bytes.push(c as u8);
    }
    bytes
}

pub(crate) fn make_tlv(tag: &str, data: &[u8]) -> Vec<u8> {
    let mut result = Vec::new();
    result.extend_from_slice(tag.as_bytes());
    result.extend_from_slice(&(data.len() as u32).to_be_bytes());
    result.extend_from_slice(data);
    result
}

fn build_crate_bytes(track_paths: &[String]) -> Vec<u8> {
    let mut parts = Vec::new();

    let version = encode_utf16be("1.0/Serato ScratchLive Crate");
    parts.extend(make_tlv("vrsn", &version));

    let columns = [
        ("song", "250"),
        ("artist", "250"),
        ("bpm", "30"),
        ("key", "30"),
        ("album", "250"),
        ("length", "250"),
        ("comment", "250"),
    ];

    for (name, width) in &columns {
        let mut col_data = Vec::new();
        col_data.extend(make_tlv("tvcn", &encode_utf16be(name)));
        col_data.extend(make_tlv("tvcw", &encode_utf16be(width)));
        parts.extend(make_tlv("ovct", &col_data));
    }

    let mut sorted_paths = track_paths.to_vec();
    sorted_paths.sort();
    for path in &sorted_paths {
        let ptrk_data = make_tlv("ptrk", &encode_utf16be(path));
        parts.extend(make_tlv("otrk", &ptrk_data));
    }

    parts
}

/// Build a single otrk entry for the database V2 file
fn build_db_track_entry(track: &TrackMeta) -> Vec<u8> {
    let mut inner = Vec::new();
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as u32;

    // Text fields
    inner.extend(make_tlv("ttyp", &encode_utf16be(&track.file_type)));
    inner.extend(make_tlv("pfil", &encode_utf16be(&track.serato_path)));
    inner.extend(make_tlv("tsng", &encode_utf16be(&track.title)));
    inner.extend(make_tlv("tart", &encode_utf16be(&track.artist)));
    inner.extend(make_tlv("talb", &encode_utf16be(&track.album)));
    inner.extend(make_tlv("tgen", &encode_utf16be(&track.genre)));

    // Duration formatted as MM:SS.ms
    let mins = (track.duration_secs / 60.0) as u32;
    let secs = track.duration_secs % 60.0;
    let duration_str = format!("{:02}:{:05.2}", mins, secs);
    inner.extend(make_tlv("tlen", &encode_utf16be(&duration_str)));

    // File size
    let size_str = if track.file_size > 1_000_000 {
        format!("{:.1}MB", track.file_size as f64 / 1_000_000.0)
    } else {
        format!("{:.0}KB", track.file_size as f64 / 1_000.0)
    };
    inner.extend(make_tlv("tsiz", &encode_utf16be(&size_str)));

    // Bitrate
    if track.bitrate > 0 {
        let bitrate_str = format!("{}.0kbps", track.bitrate);
        inner.extend(make_tlv("tbit", &encode_utf16be(&bitrate_str)));
    }

    // Sample rate
    if track.sample_rate > 0 {
        let sr_str = if track.sample_rate >= 1000 {
            format!("{:.1}k", track.sample_rate as f64 / 1000.0)
        } else {
            format!("{}", track.sample_rate)
        };
        inner.extend(make_tlv("tsmp", &encode_utf16be(&sr_str)));
    }

    // BPM
    if track.bpm > 0.0 {
        let bpm_str = format!("{:.2}", track.bpm);
        inner.extend(make_tlv("tbpm", &encode_utf16be(&bpm_str)));
    }

    // Year
    if !track.year.is_empty() {
        inner.extend(make_tlv("ttyr", &encode_utf16be(&track.year)));
    }

    // Timestamps
    let now_str = now.to_string();
    inner.extend(make_tlv("tadd", &encode_utf16be(&now_str)));

    // Key
    if !track.key.is_empty() {
        inner.extend(make_tlv("tkey", &encode_utf16be(&track.key)));
    }

    // Unsigned int fields
    inner.extend(make_tlv("uadd", &now.to_be_bytes()));
    inner.extend(make_tlv("utkn", &track.track_number.to_be_bytes()));
    inner.extend(make_tlv("utme", &now.to_be_bytes()));
    inner.extend(make_tlv("ufsb", &(track.file_size as u32).to_be_bytes()));
    inner.extend(make_tlv("udsc", &1u32.to_be_bytes()));
    inner.extend(make_tlv("utpc", &0u32.to_be_bytes()));

    // Boolean fields
    let zero: [u8; 1] = [0];
    let one: [u8; 1] = [1];
    inner.extend(make_tlv("bhrt", &zero));
    inner.extend(make_tlv("bmis", &zero));
    inner.extend(make_tlv("bply", &zero));
    inner.extend(make_tlv("blop", &zero));
    inner.extend(make_tlv("bitu", &zero));
    inner.extend(make_tlv("bovc", &one));
    inner.extend(make_tlv("bcrt", &zero));
    inner.extend(make_tlv("biro", &zero));
    inner.extend(make_tlv("bwlb", &zero));
    inner.extend(make_tlv("bwll", &zero));
    inner.extend(make_tlv("buns", &zero));
    inner.extend(make_tlv("bbgl", &zero));
    inner.extend(make_tlv("bkrk", &zero));

    make_tlv("otrk", &inner)
}

/// Updates to apply to one track's entry in Serato's database V2.
/// Any `Some(...)` field replaces (or inserts) that TLV; `None` leaves the
/// existing value alone. All other TLVs in the `otrk` are preserved
/// byte-for-byte — cues, beat grids, Serato's analysis data, everything.
#[derive(Debug, Clone, Default)]
pub(crate) struct SeratoDbUpdate {
    pub tlbl: Option<String>, // Label column (our mood)
    pub tgrp: Option<String>, // Grouping column (subgenre / edit marker)
    pub tbpm: Option<String>, // BPM as text "NN.NN"
    pub tgen: Option<String>, // Genre column
    pub ttyr: Option<String>, // Year ("YYYY" — normalized from full dates)
    pub tcmt: Option<String>, // Comment column (Spotify subgenre array)
    /// New Serato path (no leading slash). Set when a file has been moved
    /// on disk and Serato's `pfil` needs to be rewritten.
    pub new_pfil: Option<String>,
    /// If true, the `otrk` entry is dropped from the output entirely.
    pub delete: bool,
}

fn extract_pfil_from_otrk(inner: &[u8]) -> Option<String> {
    let mut pos = 0;
    while pos + 8 <= inner.len() {
        let tag = &inner[pos..pos + 4];
        let length = u32::from_be_bytes([
            inner[pos + 4],
            inner[pos + 5],
            inner[pos + 6],
            inner[pos + 7],
        ]) as usize;
        pos += 8;
        if pos + length > inner.len() {
            break;
        }
        if tag == b"pfil" {
            let slice = &inner[pos..pos + length];
            let mut s = String::with_capacity(slice.len() / 2);
            for i in (0..slice.len()).step_by(2) {
                if i + 1 < slice.len() {
                    let ch = u16::from_be_bytes([slice[i], slice[i + 1]]);
                    if let Some(c) = char::from_u32(ch as u32) {
                        s.push(c);
                    }
                }
            }
            return Some(s);
        }
        pos += length;
    }
    None
}

fn rebuild_otrk_inner_with_updates(inner: &[u8], update: &SeratoDbUpdate) -> Vec<u8> {
    let mut out = Vec::with_capacity(inner.len() + 64);
    let mut seen_tlbl = false;
    let mut seen_tgrp = false;
    let mut seen_tbpm = false;
    let mut seen_tgen = false;
    let mut seen_ttyr = false;
    let mut seen_tcmt = false;
    let mut pos = 0;

    while pos + 8 <= inner.len() {
        let tag = &inner[pos..pos + 4];
        let length = u32::from_be_bytes([
            inner[pos + 4],
            inner[pos + 5],
            inner[pos + 6],
            inner[pos + 7],
        ]) as usize;
        let full_end = pos + 8 + length;
        if full_end > inner.len() {
            break;
        }

        let replaced = match tag {
            b"pfil" if update.new_pfil.is_some() => {
                out.extend(make_tlv(
                    "pfil",
                    &encode_utf16be(update.new_pfil.as_ref().unwrap()),
                ));
                true
            }
            b"tlbl" if update.tlbl.is_some() => {
                seen_tlbl = true;
                out.extend(make_tlv("tlbl", &encode_utf16be(update.tlbl.as_ref().unwrap())));
                true
            }
            b"tgrp" if update.tgrp.is_some() => {
                seen_tgrp = true;
                out.extend(make_tlv("tgrp", &encode_utf16be(update.tgrp.as_ref().unwrap())));
                true
            }
            b"tbpm" if update.tbpm.is_some() => {
                seen_tbpm = true;
                out.extend(make_tlv("tbpm", &encode_utf16be(update.tbpm.as_ref().unwrap())));
                true
            }
            b"tgen" if update.tgen.is_some() => {
                seen_tgen = true;
                out.extend(make_tlv("tgen", &encode_utf16be(update.tgen.as_ref().unwrap())));
                true
            }
            b"ttyr" if update.ttyr.is_some() => {
                seen_ttyr = true;
                out.extend(make_tlv("ttyr", &encode_utf16be(update.ttyr.as_ref().unwrap())));
                true
            }
            b"tcmt" if update.tcmt.is_some() => {
                seen_tcmt = true;
                out.extend(make_tlv("tcmt", &encode_utf16be(update.tcmt.as_ref().unwrap())));
                true
            }
            _ => false,
        };

        if !replaced {
            out.extend_from_slice(&inner[pos..full_end]);
        }
        pos = full_end;
    }

    // Append any fields we wanted to set but didn't find existing TLVs for.
    if !seen_tlbl {
        if let Some(v) = &update.tlbl {
            out.extend(make_tlv("tlbl", &encode_utf16be(v)));
        }
    }
    if !seen_tgrp {
        if let Some(v) = &update.tgrp {
            out.extend(make_tlv("tgrp", &encode_utf16be(v)));
        }
    }
    if !seen_tbpm {
        if let Some(v) = &update.tbpm {
            out.extend(make_tlv("tbpm", &encode_utf16be(v)));
        }
    }
    if !seen_tgen {
        if let Some(v) = &update.tgen {
            out.extend(make_tlv("tgen", &encode_utf16be(v)));
        }
    }
    if !seen_ttyr {
        if let Some(v) = &update.ttyr {
            out.extend(make_tlv("ttyr", &encode_utf16be(v)));
        }
    }
    if !seen_tcmt {
        if let Some(v) = &update.tcmt {
            out.extend(make_tlv("tcmt", &encode_utf16be(v)));
        }
    }

    out
}

/// Rewrite Serato's `database V2` in place, applying the given per-path
/// updates to matching `otrk` entries. Preserves all other bytes byte-for-byte
/// so cue points / beat grids / analysis data are untouched.
///
/// Backs up the existing db to `database V2.bak` before writing.
/// Returns the number of `otrk` entries that were updated.
///
/// NOTE: Serato reads its db into memory at startup. Users need to quit and
/// relaunch Serato for these changes to appear in the UI.
pub(crate) fn update_serato_db(
    db_path: &Path,
    updates: &HashMap<String, SeratoDbUpdate>,
) -> Result<usize, String> {
    if updates.is_empty() {
        return Ok(0);
    }
    let data = fs::read(db_path).map_err(|e| format!("db read: {}", e))?;

    let mut out = Vec::with_capacity(data.len() + 64 * updates.len());
    let mut count = 0usize;
    let mut pos = 0;

    while pos + 8 <= data.len() {
        let tag = &data[pos..pos + 4];
        let length = u32::from_be_bytes([
            data[pos + 4],
            data[pos + 5],
            data[pos + 6],
            data[pos + 7],
        ]) as usize;
        let full_end = pos + 8 + length;
        if full_end > data.len() {
            // Malformed tail — append rest as-is and stop
            out.extend_from_slice(&data[pos..]);
            pos = data.len();
            break;
        }

        if tag == b"otrk" {
            let inner = &data[pos + 8..full_end];
            if let Some(path_str) = extract_pfil_from_otrk(inner) {
                if let Some(update) = updates.get(&path_str) {
                    if update.delete {
                        // Drop the entire otrk — track no longer in library.
                        count += 1;
                        pos = full_end;
                        continue;
                    }
                    let new_inner = rebuild_otrk_inner_with_updates(inner, update);
                    let new_length = new_inner.len() as u32;
                    out.extend_from_slice(b"otrk");
                    out.extend_from_slice(&new_length.to_be_bytes());
                    out.extend(new_inner);
                    count += 1;
                    pos = full_end;
                    continue;
                }
            }
        }
        out.extend_from_slice(&data[pos..full_end]);
        pos = full_end;
    }

    // Rotating backup (the old only-once .bak froze at its first write and was
    // useless months later). Keep the newest 10 dated backups.
    let stamp = std::process::Command::new("/bin/date")
        .arg("+%Y%m%d-%H%M%S")
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|| "unknown".into());
    let db_name = db_path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    if let Some(dir) = db_path.parent() {
        let bak_prefix = format!("{}.bak.", db_name);
        let backup = dir.join(format!("{}{}", bak_prefix, stamp));
        let _ = fs::copy(db_path, &backup);
        let mut baks: Vec<PathBuf> = fs::read_dir(dir)
            .map(|rd| {
                rd.filter_map(|e| e.ok())
                    .map(|e| e.path())
                    .filter(|p| {
                        p.file_name()
                            .map(|n| n.to_string_lossy().starts_with(&bak_prefix))
                            .unwrap_or(false)
                    })
                    .collect()
            })
            .unwrap_or_default();
        baks.sort();
        while baks.len() > 10 {
            let _ = fs::remove_file(baks.remove(0));
        }
    }
    // Atomic write: temp + rename, so a crash can't truncate the db.
    // Temp name derives from the target so concurrent writers (and parallel
    // tests sharing a temp dir) never collide.
    let tmp = db_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(format!(
            "{}.tmp",
            db_path.file_name().unwrap_or_default().to_string_lossy()
        ));
    fs::write(&tmp, &out)
        .and_then(|_| fs::rename(&tmp, db_path))
        .map_err(|e| {
            let _ = fs::remove_file(&tmp);
            format!("db write: {}", e)
        })?;
    Ok(count)
}

/// Turn an absolute filesystem path into the form Serato stores in `pfil`
/// (leading slash stripped).
pub(crate) fn to_serato_db_path(p: &Path) -> String {
    p.to_string_lossy().trim_start_matches('/').to_string()
}

/// Parse existing database V2 to get set of known track paths
fn parse_existing_db_paths(db_path: &Path) -> HashMap<String, bool> {
    let mut known = HashMap::new();

    let data = match fs::read(db_path) {
        Ok(d) => d,
        Err(_) => return known,
    };

    let mut pos = 0;
    while pos + 8 <= data.len() {
        let tag = &data[pos..pos + 4];
        let length = u32::from_be_bytes([
            data[pos + 4],
            data[pos + 5],
            data[pos + 6],
            data[pos + 7],
        ]) as usize;
        pos += 8;

        if tag == b"otrk" && pos + length <= data.len() {
            // Look for pfil inside this otrk
            let inner_end = pos + length;
            let mut ipos = pos;
            while ipos + 8 <= inner_end {
                let itag = &data[ipos..ipos + 4];
                let ilen = u32::from_be_bytes([
                    data[ipos + 4],
                    data[ipos + 5],
                    data[ipos + 6],
                    data[ipos + 7],
                ]) as usize;
                ipos += 8;

                if itag == b"pfil" && ipos + ilen <= inner_end {
                    // Decode UTF-16 BE path
                    let mut path_str = String::new();
                    let slice = &data[ipos..ipos + ilen];
                    for i in (0..slice.len()).step_by(2) {
                        if i + 1 < slice.len() {
                            let ch = u16::from_be_bytes([slice[i], slice[i + 1]]);
                            if let Some(c) = char::from_u32(ch as u32) {
                                path_str.push(c);
                            }
                        }
                    }
                    known.insert(path_str, true);
                    break;
                }
                ipos += ilen;
            }
        }
        pos += length;
    }

    known
}

// ── Commands ───────────────────────────────────────────────────────────────

#[tauri::command]
fn scan_library(library_path: String) -> Result<ScanResult, String> {
    let root = PathBuf::from(&library_path);
    if !root.exists() {
        return Err(format!("Path does not exist: {}", library_path));
    }

    let mut genre_map: HashMap<String, HashMap<String, usize>> = HashMap::new();
    let mut total_files = 0;

    for entry in WalkDir::new(&root)
        .into_iter()
        .filter_entry(|e| !e.file_name().to_string_lossy().starts_with('.'))
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if !path.is_file() || !is_audio_file(path) {
            continue;
        }
        total_files += 1;

        if let Ok(rel) = path.strip_prefix(&root) {
            let parts: Vec<&str> = rel
                .components()
                .filter_map(|c| c.as_os_str().to_str())
                .collect();
            if parts.len() >= 2 {
                let genre = parts[0].to_string();
                let subgenre = parts[1].to_string();
                *genre_map
                    .entry(genre)
                    .or_default()
                    .entry(subgenre)
                    .or_insert(0) += 1;
            }
        }
    }

    let mut genres: Vec<GenreInfo> = genre_map
        .into_iter()
        .map(|(name, subs)| {
            let subgenres: Vec<SubgenreInfo> = subs
                .into_iter()
                .map(|(sub_name, count)| SubgenreInfo {
                    name: sub_name,
                    track_count: count,
                })
                .collect();
            let track_count = subgenres.iter().map(|s| s.track_count).sum();
            GenreInfo {
                name,
                track_count,
                subgenres,
            }
        })
        .collect();
    genres.sort_by(|a, b| b.track_count.cmp(&a.track_count));

    Ok(ScanResult {
        total_files,
        genres,
    })
}

#[tauri::command]
fn run_python_script(script_path: String, args: Vec<String>) -> Result<PythonStepResult, String> {
    let mut cmd = Command::new("python3");
    cmd.arg(&script_path);
    for arg in &args {
        cmd.arg(arg);
    }

    match cmd.output() {
        Ok(output) => Ok(PythonStepResult {
            success: output.status.success(),
            output: String::from_utf8_lossy(&output.stdout).to_string(),
            error: String::from_utf8_lossy(&output.stderr).to_string(),
        }),
        Err(e) => Err(format!("Failed to run script: {}", e)),
    }
}

pub(crate) fn emit_progress(app: Option<&AppHandle>, stage: &str, current: usize, total: usize, msg: &str) {
    if let Some(h) = app {
        let _ = h.emit(
            "sync:progress",
            ProgressPayload {
                stage: stage.to_string(),
                current,
                total,
                message: msg.to_string(),
            },
        );
    } else {
        // Headless/CLI mode — print to stderr so cron logs capture progress
        if total > 0 {
            eprintln!("[{}] {}/{} {}", stage, current, total, msg);
        } else {
            eprintln!("[{}] {}", stage, msg);
        }
    }
}

fn read_neworder_entries(path: &Path) -> Vec<String> {
    let mut entries: Vec<String> = Vec::new();
    let Ok(data) = fs::read(path) else { return entries };

    let text: String = data
        .chunks(2)
        .filter_map(|pair| {
            if pair.len() == 2 {
                let ch = u16::from_be_bytes([pair[0], pair[1]]);
                char::from_u32(ch as u32)
            } else {
                None
            }
        })
        .collect();

    for line in text.split('\n') {
        let line = line.trim();
        if let Some(name) = line.strip_prefix("[crate]") {
            if !name.is_empty() && !entries.iter().any(|e| e == name) {
                entries.push(name.to_string());
            }
        }
    }
    entries
}

fn write_neworder_entries(path: &Path, entries: &[String]) -> std::io::Result<()> {
    let mut content = Vec::new();

    for ch in "[begin record]".encode_utf16() {
        content.push((ch >> 8) as u8);
        content.push(ch as u8);
    }
    content.extend_from_slice(&[0x00, 0x0A]);

    for name in entries {
        let line = format!("[crate]{}", name);
        for ch in line.encode_utf16() {
            content.push((ch >> 8) as u8);
            content.push(ch as u8);
        }
        content.extend_from_slice(&[0x00, 0x0A]);
    }

    for ch in "[end record]".encode_utf16() {
        content.push((ch >> 8) as u8);
        content.push(ch as u8);
    }
    content.extend_from_slice(&[0x00, 0x0A]);

    fs::write(path, content)
}

fn scan_library_read_only(crates_root: &Path, app: &AppHandle) -> Vec<TrackMeta> {
    let genres_dir = crates_root.join("GENRES");
    let playlists_dir = crates_root.join("PLAYLISTS");
    let dirs_to_scan: Vec<PathBuf> = [genres_dir, playlists_dir]
        .into_iter()
        .filter(|d| d.exists())
        .collect();

    let total: usize = dirs_to_scan
        .iter()
        .flat_map(|d| WalkDir::new(d).into_iter().filter_map(|e| e.ok()))
        .filter(|e| e.path().is_file() && is_audio_file(e.path()))
        .count();

    emit_progress(Some(app), "scanning", 0, total, "Scanning library");

    let mut full_library: Vec<TrackMeta> = Vec::new();
    let mut scanned = 0usize;

    for scan_dir in &dirs_to_scan {
        for entry in WalkDir::new(scan_dir)
            .into_iter()
            .filter_entry(|e| !e.file_name().to_string_lossy().starts_with('.'))
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            if !path.is_file() || !is_audio_file(path) {
                continue;
            }

            if let Ok(rel) = path.strip_prefix(crates_root) {
                let parts: Vec<String> = rel
                    .parent()
                    .map(|p| {
                        p.components()
                            .filter_map(|c| c.as_os_str().to_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default();

                let (genre_from_path, subgenre_from_path) = if parts.len() >= 3 && parts[0] == "GENRES" {
                    (parts[1].clone(), parts[2].clone())
                } else {
                    (String::new(), String::new())
                };

                let mut track = read_track_meta(path, &genre_from_path, &subgenre_from_path);
                track.hierarchy = parts;
                full_library.push(track);

                scanned += 1;
                if scanned % 100 == 0 || scanned == total {
                    emit_progress(Some(app), "scanning", scanned, total, "Scanning library");
                }
            }
        }
    }

    full_library
}

fn library_cache_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let dir = PathBuf::from(home).join(".dj-crates-tools");
    let _ = fs::create_dir_all(&dir);
    dir.join("library.json")
}

pub(crate) fn spotify_cache_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let dir = PathBuf::from(home).join(".dj-crates-tools");
    let _ = fs::create_dir_all(&dir);
    dir.join("spotify_cache.json")
}

pub(crate) fn spotify_key(artist: &str, title: &str) -> String {
    let clean = |s: &str| {
        s.trim()
            .to_lowercase()
            .chars()
            .filter(|c| c.is_alphanumeric() || c.is_whitespace())
            .collect::<String>()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    };
    format!("{}|{}", clean(artist), clean(title))
}

pub(crate) fn load_spotify_cache() -> HashMap<String, i32> {
    let path = spotify_cache_path();
    if !path.exists() {
        return HashMap::new();
    }
    fs::read(&path)
        .ok()
        .and_then(|d| serde_json::from_slice(&d).ok())
        .unwrap_or_default()
}

fn save_spotify_cache(cache: &HashMap<String, i32>) {
    if let Ok(json) = serde_json::to_vec_pretty(cache) {
        let _ = fs::write(spotify_cache_path(), json);
    }
}

fn get_spotify_token(client_id: &str, client_secret: &str) -> Result<String, String> {
    let creds = format!("{}:{}", client_id, client_secret);
    let encoded = general_purpose::STANDARD.encode(creds);
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| format!("HTTP client build failed: {}", e))?;
    let resp = client
        .post("https://accounts.spotify.com/api/token")
        .header("Authorization", format!("Basic {}", encoded))
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body("grant_type=client_credentials")
        .send()
        .map_err(|e| format!("Token request failed: {}", e))?;

    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().unwrap_or_default();
        return Err(format!(
            "Spotify auth failed ({}). Check your Client ID/Secret. {}",
            status, body
        ));
    }

    let json: serde_json::Value = resp
        .json()
        .map_err(|e| format!("Token parse error: {}", e))?;
    json.get("access_token")
        .and_then(|v| v.as_str())
        .map(String::from)
        .ok_or_else(|| "No access_token in Spotify response".into())
}

fn strip_featuring(title: &str) -> String {
    // Remove things like "(feat. X)", "[ft. Y]", "(with Z)" that throw off Spotify search
    let mut out = title.to_string();
    for open in ['(', '['] {
        while let Some(start) = out.find(open) {
            let close = if open == '(' { ')' } else { ']' };
            if let Some(end) = out[start..].find(close) {
                let segment = &out[start + 1..start + end];
                let lower = segment.to_lowercase();
                if lower.starts_with("feat")
                    || lower.starts_with("ft.")
                    || lower.starts_with("ft ")
                    || lower.starts_with("with ")
                    || lower.starts_with("prod")
                {
                    out.replace_range(start..=start + end, "");
                    continue;
                }
            }
            break;
        }
    }
    out.trim().to_string()
}

fn search_spotify_popularity(
    client: &reqwest::blocking::Client,
    token: &str,
    artist: &str,
    title: &str,
) -> Result<Option<i32>, String> {
    // Artist can contain "Artist1 & Artist2" or "Artist feat. Artist2"
    let primary_artist = artist
        .split(&[',', '&'][..])
        .next()
        .unwrap_or(artist)
        .split(" feat")
        .next()
        .unwrap_or(artist)
        .trim();

    let clean_title = strip_featuring(title);

    let q = format!("track:\"{}\" artist:\"{}\"", clean_title, primary_artist);
    let url = format!(
        "https://api.spotify.com/v1/search?q={}&type=track&limit=1",
        urlencoding::encode(&q)
    );

    let resp = client
        .get(&url)
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .map_err(|e| format!("Search failed: {}", e))?;

    let status = resp.status();
    if status.as_u16() == 429 {
        std::thread::sleep(std::time::Duration::from_secs(5));
        return Err("Rate limited (429)".into());
    }
    if !status.is_success() {
        return Err(format!("Spotify API error: {}", status));
    }

    let json: serde_json::Value = resp.json().map_err(|e| format!("Parse error: {}", e))?;
    let pop = json
        .get("tracks")
        .and_then(|t| t.get("items"))
        .and_then(|i| i.as_array())
        .and_then(|arr| arr.first())
        .and_then(|item| item.get("popularity"))
        .and_then(|p| p.as_i64())
        .map(|p| p as i32);

    Ok(pop)
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EnrichResult {
    pub total: usize,
    pub enriched: usize,
    pub cached: usize,
    pub not_found: usize,
    pub errors: usize,
}

/// Result of the one-button full pipeline: per-step summary lines plus how
/// many steps failed (the run keeps going past individual failures).
#[derive(serde::Serialize)]
pub struct PipelineResult {
    pub summary: Vec<String>,
    pub failed: usize,
}

/// The one-button sync: intake → sanitize → fix-bpms → downbeat-cue →
/// sync → apple-music, wrapped in the safety rails (Serato-closed guard,
/// pipeline lock, pre-run snapshot). `async` so the multi-minute run does
/// not block Tauri's main thread / freeze the UI.
#[tauri::command(async)]
fn run_full_pipeline(app: AppHandle) -> Result<PipelineResult, String> {
    let (summary, failed) = headless::run_steps_core(&app, headless::PIPELINE_STEPS)?;
    Ok(PipelineResult { summary, failed })
}

#[tauri::command(async)]
fn sync_to_serato(
    app: AppHandle,
    crates_root: String,
    serato_path: String,
    python_venv: Option<String>,
    scripts_dir: Option<String>,
    unsorted_path: Option<String>,
    clean: bool,
    preview: bool,
) -> Result<SyncResult, String> {
    let crates_dir = PathBuf::from(&crates_root);
    let serato_dir = PathBuf::from(&serato_path);
    let subcrates_dir = serato_dir.join("Subcrates");
    let db_path = serato_dir.join("database V2");

    if !crates_dir.exists() {
        return Err(format!("Crates directory not found: {}", crates_root));
    }
    if !subcrates_dir.exists() {
        return Err(format!(
            "Serato Subcrates directory not found: {}",
            subcrates_dir.display()
        ));
    }

    emit_progress(Some(&app), "starting", 0, 0, if preview { "Preview mode" } else { "Starting sync" });

    // Scan the Unsorted folder (optional) and return routing suggestions on preview.
    // On apply (preview=false) the frontend has already invoked apply_unsorted_moves
    // for the approved set, so newly-placed files land in the subsequent scan below.
    let mut unsorted_tracks: Vec<UnsortedTrack> = Vec::new();
    if preview {
        if let Some(up) = unsorted_path.as_ref() {
            if !up.is_empty() {
                emit_progress(Some(&app), "scanning_unsorted", 0, 0, "Scanning Unsorted folder");
                match unsorted::scan_unsorted(
                    up.clone(),
                    crates_root.clone(),
                    None,
                    None,
                ) {
                    Ok(list) => unsorted_tracks = list,
                    Err(e) => {
                        emit_progress(Some(&app), "unsorted_error", 0, 0, &format!("Unsorted scan: {}", e));
                    }
                }
            }
        }
    }

    // Collect the list of synced crates that would be deleted on clean.
    // NOTE: nothing is deleted here — the actual delete happens in the commit
    // phase at the end, after every replacement crate has been built, so an
    // interrupted run can no longer gut Subcrates (the 2026-04-28 incident).
    let mut crates_to_delete: Vec<String> = Vec::new();
    if clean {
        if let Ok(entries) = fs::read_dir(&subcrates_dir) {
            for entry in entries.filter_map(|e| e.ok()) {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.ends_with(".crate")
                    && (name.starts_with("GENRES%%")
                        || name.starts_with("PLAYLISTS%%")
                        || name == "GENRES.crate"
                        || name == "PLAYLISTS.crate")
                {
                    crates_to_delete.push(name.trim_end_matches(".crate").to_string());
                }
            }
        }
    }

    // Run energy analysis on new/untagged tracks via Python (skipped in preview)
    let mut energy_analyzed = 0usize;
    if !preview {
        if let (Some(venv), Some(scripts)) = (python_venv.as_ref(), scripts_dir.as_ref()) {
            let venv_python = PathBuf::from(venv);
            let script = PathBuf::from(scripts).join("analyze_new_tracks.py");
            if venv_python.exists() && script.exists() {
                emit_progress(Some(&app), "analyzing_energy", 0, 0, "Analyzing new tracks with Python");
                if let Ok(output) = Command::new(&venv_python).arg(&script).output() {
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    if let Some(line) = stdout.lines().find(|l| l.starts_with("RESULT:")) {
                        for part in line[7..].split(',') {
                            if let Some(val) = part.strip_prefix("analyzed=") {
                                energy_analyzed = val.parse().unwrap_or(0);
                            }
                        }
                    }
                }
            }
        }
    }

    // Parse existing database to find which tracks are already known
    let existing_tracks = parse_existing_db_paths(&db_path);

    // First pass: count audio files for progress totals
    emit_progress(Some(&app), "counting", 0, 0, "Counting audio files");
    let genres_dir = crates_dir.join("GENRES");
    let playlists_dir = crates_dir.join("PLAYLISTS");
    let dirs_to_scan: Vec<&Path> = [&genres_dir, &playlists_dir]
        .iter()
        .filter(|d| d.exists())
        .map(|d| d.as_path())
        .collect();
    let total_files: usize = dirs_to_scan
        .iter()
        .flat_map(|d| WalkDir::new(d).into_iter().filter_map(|e| e.ok()))
        .filter(|e| e.path().is_file() && is_audio_file(e.path()))
        .count();

    emit_progress(Some(&app), "scanning", 0, total_files, "Scanning library");

    // Scan all tracks, read metadata, build crate hierarchy
    let mut crates: HashMap<String, Vec<String>> = HashMap::new();
    let mut all_tracks: Vec<TrackMeta> = Vec::new();
    let mut full_library: Vec<TrackMeta> = Vec::new();
    let mut bpm_adjusted = 0usize;
    let mut scanned = 0usize;

    // Fresh-arrival tracking for the New This Week / New This Month crates.
    let now = SystemTime::now();
    let mut new_week: Vec<String> = Vec::new();
    let mut new_month: Vec<String> = Vec::new();

    // One library entry per REAL file: the folder tree uses symlinks to put a
    // track in multiple crates (e.g. Kory Likes + its genre folder), so track
    // identity must resolve through the link. Crate PLACEMENT still comes
    // from where the link sits.
    let mut db_new_seen: HashSet<String> = HashSet::new();
    let mut lib_seen: HashSet<String> = HashSet::new();

    for scan_dir in &dirs_to_scan {
        for entry in WalkDir::new(scan_dir)
            .into_iter()
            .filter_entry(|e| !e.file_name().to_string_lossy().starts_with('.'))
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            // NOTE: is_file() follows symlinks, so broken links are skipped here.
            if !path.is_file() || !is_audio_file(path) {
                continue;
            }
            // Canonical target of the (possibly linked) file — this is what
            // goes into crates and database V2 so Serato sees ONE track with
            // one analysis / cue set, in as many crates as it appears.
            let canonical_buf = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
            let canonical = canonical_buf.as_path();

            if let Ok(rel) = path.strip_prefix(&crates_dir) {
                let parts: Vec<String> = rel
                    .parent()
                    .map(|p| {
                        p.components()
                            .filter_map(|c| c.as_os_str().to_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default();

                // Extract genre and subgenre from path for BPM rules
                // Path: GENRES/Genre/Subgenre/Decade/Year/file
                let (genre_from_path, subgenre_from_path) = if parts.len() >= 3
                    && parts[0] == "GENRES"
                {
                    (parts[1].clone(), parts[2].clone())
                } else {
                    (String::new(), String::new())
                };

                // Read track metadata from the canonical file
                let mut track = read_track_meta(canonical, &genre_from_path, &subgenre_from_path);
                track.hierarchy = parts.clone();

                // Check if BPM was adjusted (write back only in non-preview mode)
                if track.bpm > 0.0 {
                    if let Ok(mut tagged) = Probe::open(canonical).and_then(|p| p.read()) {
                        let raw_bpm = tagged
                            .primary_tag()
                            .or_else(|| tagged.first_tag())
                            .and_then(|t| {
                                t.get_string(&lofty::tag::ItemKey::IntegerBpm)
                                    .or_else(|| t.get_string(&lofty::tag::ItemKey::Bpm))
                            })
                            .and_then(|s| s.parse::<f64>().ok())
                            .unwrap_or(0.0);

                        if raw_bpm > 0.0 && (raw_bpm - track.bpm).abs() > 0.01 {
                            bpm_adjusted += 1;
                            if !preview {
                                if let Some(tag) = tagged.primary_tag_mut() {
                                    tag.insert_text(lofty::tag::ItemKey::IntegerBpm, format!("{:.2}", track.bpm));
                                    let _ = tagged.save_to_path(canonical, lofty::config::WriteOptions::default());
                                }
                            }
                        }
                    }
                }

                let serato_p = track.serato_path.clone();

                // Fresh arrivals — GENRES files only (PLAYLISTS entries are
                // symlinks whose timestamps reflect playlist assembly, not
                // when the track actually arrived).
                if parts.first().map(|p| p == "GENRES").unwrap_or(false) {
                    if let Some(added) = entry.metadata().ok().as_ref().and_then(file_added_time) {
                        if is_within_days(added, now, FRESH_MONTH_DAYS) {
                            new_month.push(serato_p.clone());
                            if is_within_days(added, now, FRESH_WEEK_DAYS) {
                                new_week.push(serato_p.clone());
                            }
                        }
                    }
                }

                // Only add to database if not already known — and only once
                // per canonical file, however many links point at it.
                if !existing_tracks.contains_key(&track.serato_path)
                    && db_new_seen.insert(track.serato_path.clone())
                {
                    all_tracks.push(track.clone());
                }
                // Library cache keeps one entry per real file. GENRES is
                // scanned before PLAYLISTS, so the genre-tree hierarchy wins.
                if lib_seen.insert(track.serato_path.clone()) {
                    full_library.push(track.clone());
                }

                // Add to crate hierarchy
                for i in 1..=parts.len() {
                    let crate_name = parts[..i].join("%%");
                    crates
                        .entry(crate_name)
                        .or_default()
                        .push(serato_p.clone());
                }

                scanned += 1;
                if scanned % 100 == 0 || scanned == total_files {
                    emit_progress(Some(&app), "scanning", scanned, total_files, "Scanning library");
                }
            }
        }
    }

    // Inject the auto-generated fresh-arrival crates. They sit under the
    // PLAYLISTS%% prefix, so the clean pass above already removed last
    // sync's versions and they rebuild from scratch every run.
    let new_this_week = new_week.len();
    let new_this_month = new_month.len();
    if !new_week.is_empty() {
        crates.insert(FRESH_WEEK_CRATE.to_string(), new_week);
    }
    if !new_month.is_empty() {
        crates.insert(FRESH_MONTH_CRATE.to_string(), new_month);
    }

    // Build list of crate names that would be / were written
    let mut sorted_names: Vec<String> = crates.keys().cloned().collect();
    sorted_names.sort_by_key(|k| k.matches("%%").count());
    let mut crates_to_create: Vec<String> = sorted_names
        .iter()
        .filter(|n| crates.get(*n).map(|t| !t.is_empty()).unwrap_or(false))
        .cloned()
        .collect();
    crates_to_create.sort();

    let mut crates_written = 0;
    let mut total_track_entries = 0;

    emit_progress(Some(&app), "writing_crates", 0, sorted_names.len(), if preview { "Preview: planning crates" } else { "Writing crate files" });

    // Stage all new crate files in a temp dir next to Subcrates, then commit
    // with a fast delete+rename pass at the end. The exposure window drops
    // from the whole scan/build (minutes) to the rename loop (milliseconds).
    let staging_dir = subcrates_dir.join(format!(".sync-staging-{}", std::process::id()));
    if !preview {
        let _ = fs::remove_dir_all(&staging_dir);
        fs::create_dir_all(&staging_dir)
            .map_err(|e| format!("staging dir: {}", e))?;
    }

    for (idx, crate_name) in sorted_names.iter().enumerate() {
        if let Some(tracks) = crates.get(crate_name) {
            if tracks.is_empty() {
                continue;
            }
            // Same canonical file reachable twice within one folder (e.g. a
            // link sitting next to its target) must not duplicate the row.
            let mut row_seen: HashSet<&String> = HashSet::new();
            let tracks: Vec<String> = tracks
                .iter()
                .filter(|t| row_seen.insert(*t))
                .cloned()
                .collect();
            total_track_entries += tracks.len();
            if !preview {
                let data = build_crate_bytes(&tracks);
                let crate_file = staging_dir.join(format!("{}.crate", crate_name));

                if let Ok(mut file) = fs::File::create(&crate_file) {
                    if file.write_all(&data).is_ok() {
                        crates_written += 1;
                    }
                }
            } else {
                crates_written += 1; // counted as "would-create" for preview summary
            }
        }
        if idx % 25 == 0 {
            emit_progress(Some(&app), "writing_crates", idx + 1, sorted_names.len(), "Writing crate files");
        }
    }

    // Commit phase: everything is staged — now swap old for new.
    if !preview {
        for name in &crates_to_delete {
            let _ = fs::remove_file(subcrates_dir.join(format!("{}.crate", name)));
        }
        if let Ok(entries) = fs::read_dir(&staging_dir) {
            for entry in entries.filter_map(|e| e.ok()) {
                let dest = subcrates_dir.join(entry.file_name());
                if fs::rename(entry.path(), &dest).is_err() {
                    // Cross-checkpoint fallback: copy then remove.
                    if fs::copy(entry.path(), &dest).is_ok() {
                        let _ = fs::remove_file(entry.path());
                    }
                }
            }
        }
        let _ = fs::remove_dir_all(&staging_dir);
    }

    // The fresh-arrival crates need a PLAYLISTS parent group to nest under.
    // If the PLAYLISTS folder had no tracks of its own, no parent crate was
    // written above — create an empty one (same trick as the SMART group).
    if !preview
        && new_this_month > 0
        && crates.get("PLAYLISTS").map(|t| t.is_empty()).unwrap_or(true)
    {
        let parent = subcrates_dir.join("PLAYLISTS.crate");
        if !parent.exists() {
            let _ = fs::write(&parent, build_crate_bytes(&[]));
        }
    }

    // Append new tracks to database V2 (skip in preview)
    let database_tracks = all_tracks.len();
    if !preview && !all_tracks.is_empty() {
        emit_progress(Some(&app), "writing_db", 0, all_tracks.len(), "Updating Serato database");
        // Read existing database
        let mut db_data = if db_path.exists() {
            fs::read(&db_path).unwrap_or_default()
        } else {
            // Create new database with version header
            let version = encode_utf16be("2.0/Serato Scratch LIVE Database");
            make_tlv("vrsn", &version)
        };

        // Append new track entries
        for (i, track) in all_tracks.iter().enumerate() {
            let entry = build_db_track_entry(track);
            db_data.extend(entry);
            if i % 50 == 0 {
                emit_progress(Some(&app), "writing_db", i + 1, all_tracks.len(), "Updating Serato database");
            }
        }

        // Write updated database atomically: temp file + rename (atomic on APFS),
        // so a crash mid-write can no longer leave a truncated database V2.
        let tmp_db = db_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(format!(
                "{}.tmp",
                db_path.file_name().unwrap_or_default().to_string_lossy()
            ));
        match fs::write(&tmp_db, &db_data).and_then(|_| fs::rename(&tmp_db, &db_path)) {
            Ok(_) => {}
            Err(e) => {
                let _ = fs::remove_file(&tmp_db);
                return Err(format!("database V2 write: {}", e));
            }
        }
    }

    // Write neworder.pref with top-level crates (skip in preview)
    if !preview {
        let neworder_path = serato_dir.join("neworder.pref");

        // Preserve everything except GENRES/PLAYLISTS (we re-append those at the end).
        // SMART is treated as "manual" so user-generated smart crates survive syncs.
        let mut manual_crates: Vec<String> = read_neworder_entries(&neworder_path)
            .into_iter()
            .filter(|name| name != "GENRES" && name != "PLAYLISTS")
            .collect();

        // If SMART.crate exists on disk but wasn't in neworder (e.g. old sync stripped it),
        // re-add it so Serato shows the SMART group.
        if subcrates_dir.join("SMART.crate").exists()
            && !manual_crates.iter().any(|e| e == "SMART")
        {
            manual_crates.push("SMART".to_string());
        }

        let mut all_crates = manual_crates;
        all_crates.push("GENRES".to_string());
        all_crates.push("PLAYLISTS".to_string());

        let _ = write_neworder_entries(&neworder_path, &all_crates);
    }

    // Build top-level summary
    let mut top_level_map: HashMap<String, (usize, usize)> = HashMap::new();
    for (name, tracks) in &crates {
        let top = name.split("%%").next().unwrap_or(name).to_string();
        let entry = top_level_map.entry(top).or_insert((0, 0));
        if !name.contains("%%") {
            entry.0 = tracks.len();
        } else {
            entry.1 += 1;
        }
    }

    let mut top_level: Vec<TopLevelCrate> = top_level_map
        .into_iter()
        .map(|(name, (track_count, subcrate_count))| TopLevelCrate {
            name,
            track_count,
            subcrate_count,
        })
        .collect();
    top_level.sort_by(|a, b| a.name.cmp(&b.name));

    // Write library cache for Smart Crates to consume (skip in preview — would be stale data)
    if !preview {
        emit_progress(Some(&app), "writing_cache", 0, 0, "Caching library metadata");
        if let Ok(json) = serde_json::to_vec(&full_library) {
            let _ = fs::write(library_cache_path(), json);
        }
    }

    // Collapse Serato's crate tree so folders aren't auto-expanded after sync.
    // Serato 3.x stores expansion state in master.sqlite → container.expanded.
    // Best-effort — no-op if Serato isn't installed or the schema has moved.
    if !preview {
        emit_progress(Some(&app), "collapse_tree", 0, 0, "Collapsing Serato crate tree");
        let home = std::env::var("HOME").unwrap_or_default();
        let sqlite_path = PathBuf::from(&home)
            .join("Library/Application Support/Serato/Library/master.sqlite");
        if sqlite_path.exists() {
            let _ = Command::new("/usr/bin/sqlite3")
                .arg(&sqlite_path)
                .arg("UPDATE container SET expanded = 0 WHERE expanded = 1;")
                .output();
        }
    }

    emit_progress(Some(&app), "done", total_files, total_files, if preview { "Preview complete" } else { "Sync complete" });

    Ok(SyncResult {
        preview,
        crates_to_create,
        crates_to_delete,
        crates_written,
        total_track_entries,
        bpm_adjusted,
        database_tracks_new: database_tracks,
        energy_analyzed,
        top_level,
        unsorted: unsorted_tracks,
        new_this_week,
        new_this_month,
    })
}

// ── Smart Crates ───────────────────────────────────────────────────────────

fn rules_match(track: &TrackMeta, rules: &SmartCrateRules) -> bool {
    // Year
    let year_num: Option<i32> = track.year.trim().parse().ok();
    if let Some(min) = rules.year_min {
        match year_num {
            Some(y) if y >= min => {}
            _ => return false,
        }
    }
    if let Some(max) = rules.year_max {
        match year_num {
            Some(y) if y <= max => {}
            _ => return false,
        }
    }

    // BPM
    if let Some(min) = rules.bpm_min {
        if track.bpm < min {
            return false;
        }
    }
    if let Some(max) = rules.bpm_max {
        if track.bpm > max {
            return false;
        }
    }

    // Genre (OR match, case-insensitive substring against genre, subgenre, and hierarchy)
    if !rules.genres.is_empty() {
        let hay = format!(
            "{} {} {}",
            track.genre.to_lowercase(),
            track.subgenre.to_lowercase(),
            track.hierarchy.join(" ").to_lowercase()
        );
        let any = rules
            .genres
            .iter()
            .any(|g| !g.is_empty() && hay.contains(&g.to_lowercase()));
        if !any {
            return false;
        }
    }

    // Subgenre (OR)
    if !rules.subgenres.is_empty() {
        let sub = track.subgenre.to_lowercase();
        let any = rules
            .subgenres
            .iter()
            .any(|s| !s.is_empty() && sub.contains(&s.to_lowercase()));
        if !any {
            return false;
        }
    }

    // Artists (OR, substring)
    if !rules.artists.is_empty() {
        let artist = track.artist.to_lowercase();
        let any = rules
            .artists
            .iter()
            .any(|a| !a.is_empty() && artist.contains(&a.to_lowercase()));
        if !any {
            return false;
        }
    }

    // Title contains
    if let Some(needle) = &rules.title_contains {
        if !needle.is_empty()
            && !track
                .title
                .to_lowercase()
                .contains(&needle.to_lowercase())
        {
            return false;
        }
    }

    // Popularity (Spotify 0-100). Tracks without popularity data fail popularity filters.
    if let Some(min) = rules.popularity_min {
        match track.popularity {
            Some(p) if p >= min => {}
            _ => return false,
        }
    }
    if let Some(max) = rules.popularity_max {
        match track.popularity {
            Some(p) if p <= max => {}
            _ => return false,
        }
    }

    true
}

#[tauri::command]
fn enrich_spotify_popularity(
    app: AppHandle,
    client_id: String,
    client_secret: String,
    force: bool,
) -> Result<EnrichResult, String> {
    if client_id.trim().is_empty() || client_secret.trim().is_empty() {
        return Err("Spotify Client ID and Secret are required (set them in Settings).".into());
    }

    let lib_path = library_cache_path();
    if !lib_path.exists() {
        return Err("Library cache not found. Run a full sync first.".into());
    }
    let lib_data = fs::read(&lib_path).map_err(|e| format!("Library read: {}", e))?;
    let library: Vec<TrackMeta> =
        serde_json::from_slice(&lib_data).map_err(|e| format!("Library parse: {}", e))?;

    let mut cache = load_spotify_cache();

    emit_progress(Some(&app), "spotify_auth", 0, 0, "Authenticating with Spotify");
    let token = get_spotify_token(&client_id, &client_secret)?;

    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| format!("HTTP client: {}", e))?;

    let total = library.len();
    let mut enriched = 0usize;
    let mut cached = 0usize;
    let mut not_found = 0usize;
    let mut errors = 0usize;

    emit_progress(Some(&app), "spotify_enriching", 0, total, "Enriching with Spotify");

    for (idx, track) in library.iter().enumerate() {
        if track.artist.is_empty() || track.title.is_empty() {
            not_found += 1;
        } else {
            let key = spotify_key(&track.artist, &track.title);

            if !force && cache.contains_key(&key) {
                cached += 1;
            } else {
                match search_spotify_popularity(&client, &token, &track.artist, &track.title) {
                    Ok(Some(pop)) => {
                        cache.insert(key, pop);
                        enriched += 1;
                    }
                    Ok(None) => {
                        cache.insert(key, -1); // -1 = not found, skip on future runs unless forced
                        not_found += 1;
                    }
                    Err(_) => {
                        errors += 1;
                    }
                }
                // Rate-limit padding: ~150 req/min
                std::thread::sleep(std::time::Duration::from_millis(400));
            }
        }

        if idx % 10 == 0 || idx + 1 == total {
            emit_progress(Some(&app), "spotify_enriching", idx + 1, total, "Enriching with Spotify");
            save_spotify_cache(&cache); // periodic save so progress isn't lost
        }
    }

    save_spotify_cache(&cache);
    emit_progress(Some(&app), "done", total, total, "Spotify enrichment complete");

    Ok(EnrichResult {
        total,
        enriched,
        cached,
        not_found,
        errors,
    })
}

#[tauri::command]
fn generate_smart_crate(
    app: AppHandle,
    crates_root: Option<String>,
    serato_path: String,
    name: String,
    rules: SmartCrateRules,
    preview: bool,
) -> Result<SmartCrateResult, String> {
    let cache_path = library_cache_path();

    let mut library: Vec<TrackMeta> = if cache_path.exists() {
        let data = fs::read(&cache_path).map_err(|e| format!("Cache read error: {}", e))?;
        serde_json::from_slice(&data).map_err(|e| format!("Cache parse error: {}", e))?
    } else if let Some(root) = crates_root.as_ref() {
        let root_path = PathBuf::from(root);
        if !root_path.exists() {
            return Err(format!("Crates directory not found: {}", root));
        }
        let lib = scan_library_read_only(&root_path, &app);
        if let Ok(json) = serde_json::to_vec(&lib) {
            let _ = fs::write(&cache_path, json);
        }
        lib
    } else {
        return Err(
            "Library cache not found. Run a full sync first so Smart Crates can read your library."
                .into(),
        );
    };

    // Merge in Spotify popularity from the cache (no-op if cache doesn't exist)
    let spotify_cache = load_spotify_cache();
    if !spotify_cache.is_empty() {
        for track in library.iter_mut() {
            if track.popularity.is_some() {
                continue;
            }
            let key = spotify_key(&track.artist, &track.title);
            if let Some(&pop) = spotify_cache.get(&key) {
                if pop >= 0 {
                    track.popularity = Some(pop);
                }
            }
        }
    }

    emit_progress(Some(&app), "smart_matching", 0, library.len(), "Matching tracks to rules");

    let mut matched: Vec<&TrackMeta> = library.iter().filter(|t| rules_match(t, &rules)).collect();
    if let Some(limit) = rules.limit {
        matched.truncate(limit);
    }

    let sample_tracks: Vec<String> = matched
        .iter()
        .take(10)
        .map(|t| {
            if !t.artist.is_empty() && !t.title.is_empty() {
                format!("{} — {}", t.artist, t.title)
            } else {
                t.serato_path.clone()
            }
        })
        .collect();

    let crate_name = format!("SMART%%{}", sanitize_crate_name(&name));

    if !preview {
        let serato_dir = PathBuf::from(&serato_path);
        let subcrates_dir = serato_dir.join("Subcrates");
        if !subcrates_dir.exists() {
            return Err(format!(
                "Serato Subcrates directory not found: {}",
                subcrates_dir.display()
            ));
        }

        let track_paths: Vec<String> = matched.iter().map(|t| t.serato_path.clone()).collect();
        let crate_bytes = build_crate_bytes(&track_paths);
        let crate_file = subcrates_dir.join(format!("{}.crate", crate_name));
        fs::write(&crate_file, crate_bytes).map_err(|e| format!("Write error: {}", e))?;

        // Also ensure a top-level SMART crate file exists (empty is fine) so Serato shows the group
        let top_level = subcrates_dir.join("SMART.crate");
        if !top_level.exists() {
            let _ = fs::write(&top_level, build_crate_bytes(&[]));
        }

        // Register SMART in neworder.pref so Serato actually displays it in the sidebar
        let neworder_path = serato_dir.join("neworder.pref");
        let mut entries = read_neworder_entries(&neworder_path);
        if !entries.iter().any(|e| e == "SMART") {
            // Insert SMART before GENRES/PLAYLISTS if present, otherwise append
            let insert_at = entries
                .iter()
                .position(|e| e == "GENRES" || e == "PLAYLISTS")
                .unwrap_or(entries.len());
            entries.insert(insert_at, "SMART".to_string());
            let _ = write_neworder_entries(&neworder_path, &entries);
        }
    }

    emit_progress(Some(&app), "done", library.len(), library.len(), "Smart crate complete");

    Ok(SmartCrateResult {
        preview,
        crate_name,
        matched_tracks: matched.len(),
        sample_tracks,
    })
}

fn sanitize_crate_name(name: &str) -> String {
    name.chars()
        .filter(|c| !matches!(c, '/' | '\\' | ':' | '%' | '*' | '?' | '"' | '<' | '>' | '|'))
        .collect::<String>()
        .trim()
        .to_string()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Detect --headless <subcommand> mode. When present, skip the UI and run
    // the Rust pipeline directly using a Tauri app whose setup() callback
    // spawns a worker thread, executes the pipeline, and exits. The default
    // window is hidden in setup so it never visibly flashes.
    let raw_args: Vec<String> = std::env::args().collect();
    let headless_subcmd: Option<String> = raw_args
        .iter()
        .position(|a| a == "--headless")
        .and_then(|i| raw_args.get(i + 1).cloned());

    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .invoke_handler(tauri::generate_handler![
            scan_library,
            run_python_script,
            sync_to_serato,
            run_full_pipeline,
            generate_smart_crate,
            enrich_spotify_popularity,
            dupes::find_duplicates,
            unsorted::scan_unsorted,
            unsorted::apply_unsorted_moves,
            tagging::normalize_bpms,
            tagging::label_moods,
            tagging::detect_bpms,
            tagging::clear_tags_library,
            tagging::sanitize_library,
            tagging::auto_cue_library,
            cleanup::cleanup_library,
            prune::prune_library,
            convert::convert_m4a_to_mp3,
            normalize::normalize_hierarchy,
        ]);

    let builder = if let Some(subcmd) = headless_subcmd {
        builder.setup(move |app| {
            // Manager trait provides get_webview_window() on the App type.
            use tauri::Manager;
            if let Some(win) = app.get_webview_window("main") {
                let _: Result<(), tauri::Error> = win.hide();
            }
            let handle = app.handle().clone();
            let subcmd = subcmd.clone();
            std::thread::spawn(move || {
                let rc = headless::run_subcommand(handle, &subcmd);
                std::process::exit(rc);
            });
            Ok(())
        })
    } else {
        builder
    };

    builder
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod fresh_crate_tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn within_and_outside_windows() {
        let now = SystemTime::now();
        let two_days_ago = now - Duration::from_secs(2 * 86_400);
        let ten_days_ago = now - Duration::from_secs(10 * 86_400);
        let forty_days_ago = now - Duration::from_secs(40 * 86_400);

        assert!(is_within_days(two_days_ago, now, FRESH_WEEK_DAYS));
        assert!(is_within_days(two_days_ago, now, FRESH_MONTH_DAYS));

        assert!(!is_within_days(ten_days_ago, now, FRESH_WEEK_DAYS));
        assert!(is_within_days(ten_days_ago, now, FRESH_MONTH_DAYS));

        assert!(!is_within_days(forty_days_ago, now, FRESH_MONTH_DAYS));
    }

    #[test]
    fn future_timestamps_count_as_fresh() {
        let now = SystemTime::now();
        let future = now + Duration::from_secs(3600);
        assert!(is_within_days(future, now, FRESH_WEEK_DAYS));
    }
}

#[cfg(test)]
mod db_update_tests {
    use super::*;

    fn tlv_bytes(tag: &str, data: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(tag.as_bytes());
        out.extend_from_slice(&(data.len() as u32).to_be_bytes());
        out.extend_from_slice(data);
        out
    }

    fn build_otrk_bytes(
        path: &str,
        title: &str,
        bpm: Option<&str>,
        lbl: Option<&str>,
    ) -> Vec<u8> {
        let mut inner = Vec::new();
        inner.extend(tlv_bytes("ttyp", &encode_utf16be("mp3")));
        inner.extend(tlv_bytes("pfil", &encode_utf16be(path)));
        inner.extend(tlv_bytes("tsng", &encode_utf16be(title)));
        if let Some(b) = bpm {
            inner.extend(tlv_bytes("tbpm", &encode_utf16be(b)));
        }
        if let Some(l) = lbl {
            inner.extend(tlv_bytes("tlbl", &encode_utf16be(l)));
        }
        // Unknown TLV — must be preserved byte-for-byte
        inner.extend(tlv_bytes("XYZA", b"\x01\x02\x03"));
        tlv_bytes("otrk", &inner)
    }

    fn parse_db_entries(data: &[u8]) -> Vec<(String, Option<String>, Option<String>)> {
        let mut out = Vec::new();
        let mut pos = 0;
        while pos + 8 <= data.len() {
            let tag = &data[pos..pos + 4];
            let length = u32::from_be_bytes([
                data[pos + 4],
                data[pos + 5],
                data[pos + 6],
                data[pos + 7],
            ]) as usize;
            let full_end = pos + 8 + length;
            if full_end > data.len() {
                break;
            }
            if tag == b"otrk" {
                let inner = &data[pos + 8..full_end];
                let mut path = String::new();
                let mut bpm = None;
                let mut lbl = None;
                let mut ipos = 0;
                while ipos + 8 <= inner.len() {
                    let itag = &inner[ipos..ipos + 4];
                    let ilen = u32::from_be_bytes([
                        inner[ipos + 4],
                        inner[ipos + 5],
                        inner[ipos + 6],
                        inner[ipos + 7],
                    ]) as usize;
                    ipos += 8;
                    if ipos + ilen > inner.len() {
                        break;
                    }
                    let slice = &inner[ipos..ipos + ilen];
                    let mut s = String::with_capacity(slice.len() / 2);
                    for i in (0..slice.len()).step_by(2) {
                        if i + 1 < slice.len() {
                            let ch = u16::from_be_bytes([slice[i], slice[i + 1]]);
                            if let Some(c) = char::from_u32(ch as u32) {
                                s.push(c);
                            }
                        }
                    }
                    match itag {
                        b"pfil" => path = s,
                        b"tbpm" => bpm = Some(s),
                        b"tlbl" => lbl = Some(s),
                        _ => {}
                    }
                    ipos += ilen;
                }
                out.push((path, bpm, lbl));
            }
            pos = full_end;
        }
        out
    }

    fn tmp_db(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join("dj-crates-tests");
        fs::create_dir_all(&dir).unwrap();
        let p = dir.join(name);
        let _ = fs::remove_file(&p);
        let _ = fs::remove_file(p.with_extension("V2.bak"));
        p
    }

    #[test]
    fn replaces_existing_tlbl_and_tbpm_in_place() {
        let path = tmp_db("replace.V2");
        let mut data = tlv_bytes("vrsn", &encode_utf16be("2.0/Serato Scratch LIVE Database"));
        data.extend(build_otrk_bytes(
            "Users/kory/Music/CRATES/GENRES/R&B/a.mp3",
            "song a",
            Some("123.00"),
            Some("Old Label"),
        ));
        data.extend(build_otrk_bytes(
            "Users/kory/Music/CRATES/GENRES/R&B/b.mp3",
            "song b",
            Some("95.00"),
            None,
        ));
        fs::write(&path, data).unwrap();

        let mut updates: HashMap<String, SeratoDbUpdate> = HashMap::new();
        updates.insert(
            "Users/kory/Music/CRATES/GENRES/R&B/a.mp3".to_string(),
            SeratoDbUpdate {
                tlbl: Some("Warmup".into()),
                tbpm: Some("67.50".into()),
                ..Default::default()
            },
        );
        updates.insert(
            "Users/kory/Music/CRATES/GENRES/R&B/b.mp3".to_string(),
            SeratoDbUpdate {
                tlbl: Some("Slowdown".into()),
                ..Default::default()
            },
        );

        let n = update_serato_db(&path, &updates).unwrap();
        assert_eq!(n, 2);

        let parsed = parse_db_entries(&fs::read(&path).unwrap());
        assert_eq!(parsed.len(), 2);

        let a = parsed.iter().find(|(p, _, _)| p.ends_with("a.mp3")).unwrap();
        assert_eq!(a.1.as_deref(), Some("67.50"), "BPM replaced");
        assert_eq!(a.2.as_deref(), Some("Warmup"), "Label replaced");

        let b = parsed.iter().find(|(p, _, _)| p.ends_with("b.mp3")).unwrap();
        assert_eq!(b.1.as_deref(), Some("95.00"), "BPM preserved");
        assert_eq!(b.2.as_deref(), Some("Slowdown"), "Label inserted");

        // Backups are now rotating + dated: <dbname>.bak.<stamp>
        let bak_prefix = format!("{}.bak.", path.file_name().unwrap().to_string_lossy());
        let has_backup = fs::read_dir(path.parent().unwrap())
            .unwrap()
            .filter_map(|e| e.ok())
            .any(|e| e.file_name().to_string_lossy().starts_with(&bak_prefix));
        assert!(has_backup, "rotating backup was created");
    }

    #[test]
    fn unchanged_tracks_are_byte_for_byte_preserved() {
        let path = tmp_db("preserve.V2");
        let mut data = tlv_bytes("vrsn", &encode_utf16be("2.0/Serato Scratch LIVE Database"));
        let keep_otrk = build_otrk_bytes(
            "Users/kory/Music/CRATES/GENRES/House/keep.mp3",
            "keep me",
            Some("124.00"),
            Some("Old"),
        );
        data.extend(&keep_otrk);
        data.extend(build_otrk_bytes(
            "Users/kory/Music/CRATES/GENRES/R&B/touch.mp3",
            "touch me",
            Some("85.00"),
            None,
        ));
        fs::write(&path, data).unwrap();

        let mut updates = HashMap::new();
        updates.insert(
            "Users/kory/Music/CRATES/GENRES/R&B/touch.mp3".to_string(),
            SeratoDbUpdate {
                tlbl: Some("Warmup".into()),
                ..Default::default()
            },
        );

        update_serato_db(&path, &updates).unwrap();

        // keep.mp3 otrk should appear identical in output
        let rewritten = fs::read(&path).unwrap();
        let keep_pos = rewritten
            .windows(keep_otrk.len())
            .position(|w| w == keep_otrk.as_slice())
            .expect("untouched otrk preserved verbatim");
        assert!(keep_pos > 0);
    }

    #[test]
    fn pfil_rename_moves_file_path_in_db() {
        let path = tmp_db("rename.V2");
        let mut data = tlv_bytes("vrsn", &encode_utf16be("2.0/Serato Scratch LIVE Database"));
        data.extend(build_otrk_bytes(
            "Users/kory/Music/CRATES/GENRES/Hip-Hop/a.mp3",
            "song a",
            Some("95.00"),
            None,
        ));
        data.extend(build_otrk_bytes(
            "Users/kory/Music/CRATES/GENRES/R&B/b.mp3",
            "song b",
            Some("85.00"),
            None,
        ));
        fs::write(&path, data).unwrap();

        let mut updates = HashMap::new();
        updates.insert(
            "Users/kory/Music/CRATES/GENRES/Hip-Hop/a.mp3".to_string(),
            SeratoDbUpdate {
                new_pfil: Some(
                    "Users/kory/Music/CRATES/GENRES/Hip-Hop:Rap/General Hip-Hop/a.mp3".into(),
                ),
                ..Default::default()
            },
        );

        update_serato_db(&path, &updates).unwrap();
        let parsed = parse_db_entries(&fs::read(&path).unwrap());

        // Moved entry is keyed by the NEW path
        let moved = parsed
            .iter()
            .find(|(p, _, _)| p.ends_with("Hip-Hop:Rap/General Hip-Hop/a.mp3"))
            .expect("renamed entry present under new path");
        assert_eq!(moved.1.as_deref(), Some("95.00"), "BPM carried across rename");

        // Other entries untouched
        assert!(parsed.iter().any(|(p, _, _)| p.ends_with("R&B/b.mp3")));
        // Old EXACT path no longer appears as its own pfil
        assert!(
            !parsed.iter().any(|(p, _, _)| p
                == "Users/kory/Music/CRATES/GENRES/Hip-Hop/a.mp3"),
            "old pfil replaced"
        );
    }

    #[test]
    fn delete_flag_removes_otrk_from_output() {
        let path = tmp_db("delete.V2");
        let mut data = tlv_bytes("vrsn", &encode_utf16be("2.0/Serato Scratch LIVE Database"));
        data.extend(build_otrk_bytes(
            "Users/kory/Music/CRATES/GENRES/Techno/doomed.mp3",
            "doomed",
            Some("128.00"),
            None,
        ));
        data.extend(build_otrk_bytes(
            "Users/kory/Music/CRATES/GENRES/R&B/keeper.mp3",
            "keeper",
            Some("85.00"),
            None,
        ));
        fs::write(&path, data).unwrap();

        let mut updates = HashMap::new();
        updates.insert(
            "Users/kory/Music/CRATES/GENRES/Techno/doomed.mp3".to_string(),
            SeratoDbUpdate {
                delete: true,
                ..Default::default()
            },
        );

        update_serato_db(&path, &updates).unwrap();
        let parsed = parse_db_entries(&fs::read(&path).unwrap());

        // Deleted track gone
        assert!(
            !parsed.iter().any(|(p, _, _)| p.ends_with("doomed.mp3")),
            "deleted otrk removed from db"
        );
        // Keeper track still present
        assert!(parsed.iter().any(|(p, _, _)| p.ends_with("keeper.mp3")));
    }

    #[test]
    fn empty_updates_is_noop() {
        let path = tmp_db("noop.V2");
        let data = tlv_bytes("vrsn", &encode_utf16be("v"));
        fs::write(&path, &data).unwrap();
        let n = update_serato_db(&path, &HashMap::new()).unwrap();
        assert_eq!(n, 0);
        assert_eq!(fs::read(&path).unwrap(), data);
        assert!(!path.with_extension("V2.bak").exists());
    }
}
