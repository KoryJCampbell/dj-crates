// Second-pass library normalizer — fixes what Prune (Urban AC) left behind:
//   1. Demotes niche electronic/club genres to subfolders under their proper
//      mainstream parent (Gqom → House/Gqom, Amapiano → Afrobeats/Amapiano, etc.)
//   2. Aggressively classifies Unknown Genre / No Genre files via ID3 genre
//      tag + artist alias table. Unclassifiable → archive.

use crate::unsorted::ARTIST_ALIASES;
use crate::{emit_progress, to_serato_db_path, update_serato_db, SeratoDbUpdate};
use lofty::prelude::*;
use lofty::probe::Probe;
use lofty::tag::ItemKey;
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::AppHandle;

const AUDIO_EXTENSIONS: &[&str] = &[
    "mp3", "flac", "wav", "ogg", "aif", "aiff", "aac", "m4a", "alac",
];

const ORPHAN_FOLDERS: &[&str] = &["Unknown Genre", "No Genre", "Unclassified"];

/// Mainstream top-level genres (Discogs / Serato-style). Anything NOT in this
/// list that has a sensible parent gets demoted. Order mirrors what we expect
/// to stay at top level after normalization.
const MAINSTREAM_TOPLEVEL: &[&str] = &[
    "Hip-Hop:Rap",
    "R&B",
    "Soul",
    "Gospel",
    "Funk",
    "Disco",
    "Jazz",
    "Pop",
    "Afrobeats",
    "Dancehall",
    "Reggae",
    "House",
];

/// Niche genres to demote into their mainstream parent.
/// (source, parent_genre, subgenre_name_under_parent)
const DEMOTIONS: &[(&str, &str, &str)] = &[
    ("Amapiano", "Afrobeats", "Amapiano"),
    ("Baile Funk", "House", "Baile Funk"),
    ("Baltimore Club", "House", "Baltimore Club"),
    ("Batida", "Afrobeats", "Batida"),
    ("Footwork", "House", "Footwork"),
    ("Gqom", "House", "Gqom"),
    ("Jersey Club", "House", "Jersey Club"),
    ("Juke", "House", "Juke"),
    ("Kuduro", "Afrobeats", "Kuduro"),
    ("Neo-Soul", "Soul", "Neo-Soul"),
    ("UK Funky", "House", "UK Funky"),
];

fn is_audio(p: &Path) -> bool {
    p.extension()
        .and_then(|e| e.to_str())
        .map(|e| AUDIO_EXTENSIONS.iter().any(|x| x.eq_ignore_ascii_case(e)))
        .unwrap_or(false)
}

fn collect_audio(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(entries) = fs::read_dir(dir) else {
        return out;
    };
    for e in entries.filter_map(|x| x.ok()) {
        let p = e.path();
        if p.is_dir() {
            out.extend(collect_audio(&p));
        } else if p.is_file() && is_audio(&p) {
            out.push(p);
        }
    }
    out
}

fn audio_with_relpath(root: &Path) -> Vec<(PathBuf, PathBuf)> {
    fn walk(root: &Path, cur: &Path, out: &mut Vec<(PathBuf, PathBuf)>) {
        let Ok(entries) = fs::read_dir(cur) else {
            return;
        };
        for e in entries.filter_map(|x| x.ok()) {
            let p = e.path();
            if p.is_dir() {
                walk(root, &p, out);
            } else if p.is_file() && is_audio(&p) {
                let rel = p.strip_prefix(root).unwrap_or(&p).to_path_buf();
                out.push((p, rel));
            }
        }
    }
    let mut v = Vec::new();
    walk(root, root, &mut v);
    v
}

fn unique_dest(dir: &Path, filename: &str) -> PathBuf {
    let t = dir.join(filename);
    if !t.exists() {
        return t;
    }
    let stem = Path::new(filename)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(filename);
    let ext = Path::new(filename)
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("");
    for n in 1..1000 {
        let c = if ext.is_empty() {
            dir.join(format!("{} ({})", stem, n))
        } else {
            dir.join(format!("{} ({}).{}", stem, n, ext))
        };
        if !c.exists() {
            return c;
        }
    }
    t
}

fn move_file(from: &Path, to: &Path) -> Result<(), String> {
    if let Some(parent) = to.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("mkdir {}: {}", parent.display(), e))?;
    }
    match fs::rename(from, to) {
        Ok(_) => Ok(()),
        Err(_) => {
            fs::copy(from, to).map_err(|e| format!("copy: {}", e))?;
            fs::remove_file(from).map_err(|e| format!("rm orig: {}", e))?;
            Ok(())
        }
    }
}

fn remove_empty_dirs(dir: &Path) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for e in entries.filter_map(|x| x.ok()) {
        let p = e.path();
        if p.is_dir() {
            remove_empty_dirs(&p);
        }
    }
    let _ = fs::remove_dir(dir);
}

fn read_tags(path: &Path) -> (String, String) {
    std::panic::catch_unwind(|| {
        let tagged = Probe::open(path).ok()?.read().ok()?;
        let tag = tagged.primary_tag().or(tagged.first_tag())?;
        let artist = tag.artist().map(|s| s.to_string()).unwrap_or_default();
        let genre = tag.genre().map(|s| s.to_string()).unwrap_or_default();
        Some((artist, genre))
    })
    .ok()
    .flatten()
    .unwrap_or_default()
}

fn normalize(s: &str) -> String {
    s.to_lowercase()
        .replace(['-', '_', '/'], " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Try to resolve a file's true genre. Checks, in order:
///   1. ID3 genre tag matched against existing genre folders
///   2. Artist alias table (our Black-music name list)
/// Returns Some(matched_folder_name) or None.
fn classify_orphan(path: &Path, existing_genres: &HashSet<String>) -> Option<String> {
    let (artist, genre_tag) = read_tags(path);

    // Genre-tag match first (strongest signal)
    if !genre_tag.is_empty() {
        let g_norm = normalize(&genre_tag);
        for g in existing_genres {
            if normalize(g) == g_norm {
                return Some(g.clone());
            }
        }
        // Alias check — "hip hop" / "rap" / "r&b" variations
        for (canonical, tokens) in ARTIST_ALIASES {
            // Skip — this is the artist alias table. Use it for artists below.
            let _ = (canonical, tokens);
        }
    }

    // Artist alias match
    if !artist.is_empty() {
        let a_norm = normalize(&artist);
        for (canonical, names) in ARTIST_ALIASES {
            for n in *names {
                if a_norm == *n {
                    // Ensure the target genre exists in the user's library
                    if existing_genres.contains(*canonical) {
                        return Some((*canonical).to_string());
                    }
                }
            }
        }
    }

    // Filename keyword match against existing genre folder names
    if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
        let hay = normalize(stem);
        let mut sorted: Vec<&String> = existing_genres.iter().collect();
        sorted.sort_by_key(|s| std::cmp::Reverse(s.len()));
        for g in sorted {
            let n = normalize(g);
            if n.len() >= 3 && hay.contains(&n) {
                return Some(g.clone());
            }
        }
    }

    None
}

#[derive(Debug, Clone, Serialize)]
pub struct NormalizeReport {
    pub demoted: usize,
    pub orphans_classified: usize,
    pub orphans_archived: usize,
    pub serato_db_updated: usize,
    pub archive_root: String,
    pub errors: Vec<String>,
}

#[tauri::command]
pub fn normalize_hierarchy(
    app: AppHandle,
    crates_root: String,
    serato_path: Option<String>,
) -> Result<NormalizeReport, String> {
    let root = PathBuf::from(&crates_root);
    let genres = root.join("GENRES");
    if !genres.is_dir() {
        return Err(format!("GENRES folder not found under {}", crates_root));
    }

    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let archive_root = std::env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("Music")
        .join("_Archive")
        .join(format!("CRATES_unknown_{}", ts));

    let mut report = NormalizeReport {
        demoted: 0,
        orphans_classified: 0,
        orphans_archived: 0,
        serato_db_updated: 0,
        archive_root: archive_root.to_string_lossy().into(),
        errors: Vec::new(),
    };
    let mut db_updates: HashMap<String, SeratoDbUpdate> = HashMap::new();

    // ── Phase 1: demote niche genres ─────────────────────────────────────
    emit_progress(Some(&app), "norm_demote", 0, 0, "Demoting niche genres");
    for (src, parent, sub) in DEMOTIONS {
        let src_dir = genres.join(src);
        if !src_dir.is_dir() {
            continue;
        }
        let dest_dir = genres.join(parent).join(sub);
        for (full, rel) in audio_with_relpath(&src_dir) {
            let filename = rel
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("track")
                .to_string();
            let dest = unique_dest(&dest_dir, &filename);
            match move_file(&full, &dest) {
                Ok(_) => {
                    db_updates.insert(
                        to_serato_db_path(&full),
                        SeratoDbUpdate {
                            new_pfil: Some(to_serato_db_path(&dest)),
                            ..Default::default()
                        },
                    );
                    report.demoted += 1;
                }
                Err(e) => report.errors.push(format!("demote {}: {}", full.display(), e)),
            }
        }
        remove_empty_dirs(&src_dir);
    }

    // ── Phase 2: orphan classification ───────────────────────────────────
    // Build the canonical genre set AFTER demotion so orphans can land in
    // the now-correct top-level genres.
    let mut existing_genres: HashSet<String> = HashSet::new();
    if let Ok(entries) = fs::read_dir(&genres) {
        for e in entries.filter_map(|x| x.ok()) {
            if e.path().is_dir() {
                let name = e.file_name().to_string_lossy().to_string();
                if !name.starts_with('.') && !ORPHAN_FOLDERS.contains(&name.as_str()) {
                    existing_genres.insert(name);
                }
            }
        }
    }
    // Only accept mainstream top-level as classification targets (avoids
    // routing to another niche subgenre that we just tried to get rid of).
    let mainstream: HashSet<String> = MAINSTREAM_TOPLEVEL.iter().map(|s| s.to_string()).collect();
    let classification_targets: HashSet<String> =
        existing_genres.intersection(&mainstream).cloned().collect();

    emit_progress(Some(&app), "norm_orphans", 0, 0, "Classifying orphan files");
    for orphan_name in ORPHAN_FOLDERS {
        let orphan_dir = genres.join(orphan_name);
        if !orphan_dir.is_dir() {
            continue;
        }
        let files = collect_audio(&orphan_dir);
        for file in files {
            let filename = file
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("track")
                .to_string();
            match classify_orphan(&file, &classification_targets) {
                Some(g) => {
                    let dest = unique_dest(&genres.join(&g), &filename);
                    match move_file(&file, &dest) {
                        Ok(_) => {
                            db_updates.insert(
                                to_serato_db_path(&file),
                                SeratoDbUpdate {
                                    new_pfil: Some(to_serato_db_path(&dest)),
                                    ..Default::default()
                                },
                            );
                            report.orphans_classified += 1;
                        }
                        Err(e) => report.errors.push(format!("orphan move: {}", e)),
                    }
                }
                None => {
                    // Archive — can't reliably classify
                    let archive_dir = archive_root.join(orphan_name);
                    let dest = unique_dest(&archive_dir, &filename);
                    match move_file(&file, &dest) {
                        Ok(_) => {
                            db_updates.insert(
                                to_serato_db_path(&file),
                                SeratoDbUpdate {
                                    delete: true,
                                    ..Default::default()
                                },
                            );
                            report.orphans_archived += 1;
                        }
                        Err(e) => {
                            report.errors.push(format!("archive orphan: {}", e));
                        }
                    }
                }
            }
        }
        remove_empty_dirs(&orphan_dir);
    }

    // ── Serato db update ────────────────────────────────────────────────
    if let Some(sp) = serato_path.as_deref() {
        let db_path = PathBuf::from(sp).join("database V2");
        if db_path.exists() && !db_updates.is_empty() {
            emit_progress(Some(&app), "norm_db", 0, 0, "Updating Serato database V2");
            match update_serato_db(&db_path, &db_updates) {
                Ok(n) => report.serato_db_updated = n,
                Err(e) => report.errors.push(format!("serato db: {}", e)),
            }
        }
    }

    emit_progress(Some(&app), "norm_done", 0, 0, "Hierarchy normalized");
    Ok(report)
}
