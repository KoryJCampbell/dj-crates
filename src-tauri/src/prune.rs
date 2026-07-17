// One-shot library prune + restructure tailored for an Urban AC / Black-music
// DJ library. Moves, merges, demotes, and archives based on a hardcoded
// plan — the logic isn't a generic engine, it's the specific cleanup we
// agreed on.

use crate::unsorted::ARTIST_ALIASES;
use crate::{emit_progress, to_serato_db_path, update_serato_db, SeratoDbUpdate};
use lofty::prelude::*;
use lofty::probe::Probe;
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::AppHandle;

const AUDIO_EXTENSIONS: &[&str] = &[
    "mp3", "flac", "wav", "ogg", "aif", "aiff", "aac", "m4a", "alac",
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
                out.push((p.clone(), rel));
            }
        }
    }
    let mut v = Vec::new();
    walk(root, root, &mut v);
    v
}

fn unique_dest(dir: &Path, filename: &str) -> PathBuf {
    let target = dir.join(filename);
    if !target.exists() {
        return target;
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
        let cand = if ext.is_empty() {
            dir.join(format!("{} ({})", stem, n))
        } else {
            dir.join(format!("{} ({}).{}", stem, n, ext))
        };
        if !cand.exists() {
            return cand;
        }
    }
    target
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

fn normalize_artist(s: &str) -> String {
    // Strip "feat./ft./x/vs./&" tails, collapse whitespace
    let s = s.to_lowercase();
    let s = s.replace(['-', '_'], " ");
    let cut = |s: &str| {
        for marker in [
            " feat ", " feat. ", " ft ", " ft. ", " & ", " and ", " x ", " vs ", " vs. ", "/",
        ] {
            if let Some(i) = s.find(marker) {
                return s[..i].to_string();
            }
        }
        s.to_string()
    };
    cut(&s).split_whitespace().collect::<Vec<_>>().join(" ")
}

fn read_artist(path: &Path) -> Option<String> {
    std::panic::catch_unwind(|| {
        Probe::open(path)
            .ok()?
            .read()
            .ok()?
            .primary_tag()
            .and_then(|t| t.artist())
            .map(|s| s.to_string())
    })
    .ok()
    .flatten()
}

/// Returns Some(target_genre) if artist matches an entry in ARTIST_ALIASES.
fn alias_genre_for_artist(artist: &str) -> Option<&'static str> {
    let n = normalize_artist(artist);
    if n.len() < 2 {
        return None;
    }
    for (genre, names) in ARTIST_ALIASES {
        for a in *names {
            if &n == a {
                return Some(genre);
            }
        }
    }
    None
}

// ── Plan ──────────────────────────────────────────────────────────────────

/// Artists we want to pluck out of a doomed folder and move to a better home
/// before the folder itself gets archived. (source_folder_name, artist_norm, dest_genre)
const RESCUES: &[(&str, &str, &str)] = &[
    ("Rock", "bootsy collins", "Funk"),
    ("Rock", "billy ocean", "R&B"),
    ("Rock", "tracy chapman", "Soul"),
    ("Breakbeat", "kenny dope", "House"),
    ("Breakbeat", "rich medina", "Hip-Hop:Rap"),
    ("Jungle", "flwr chyld", "Hip-Hop:Rap"),
    ("Jungle", "skins", "Hip-Hop:Rap"),
    ("Electronic", "ebo taylor", "Afrobeats"),
];

/// Whole-folder merges: every file from source moves into dest (destination
/// may be a nested path like `Hip-Hop:Rap/UK Music`).
const MERGES: &[(&str, &str)] = &[
    ("Alternative", "R&B"),
    ("Christian", "Gospel"),
    ("UK Music", "Hip-Hop:Rap/UK Music"),
    ("Garage", "House/UK Garage"),
    ("UK Garage", "House/UK Garage"),
];

/// Subgenre demotions: top-level `source` folder becomes a subfolder under
/// `parent` with name `subname`.
const DEMOTIONS: &[(&str, &str, &str)] = &[
    ("Neo-Soul", "Soul", "Neo-Soul"),
    ("Motown", "Soul", "Motown"),
    ("Afro House", "House", "Afro House"),
    ("Lo-Fi", "Hip-Hop:Rap", "Lo-Fi"),
];

/// Folders whose remaining files (after rescues) we archive wholesale.
const ARCHIVE_WHOLE: &[&str] = &[
    "Techno", "Trance", "Bassline", "Classical", "Lounge",
    "Latin", "Reggaeton", "Soca", "Country",
    "Rock", "Breakbeat", "Jungle", "Drum & Bass",
];

/// Folders we scan per-file: if artist matches ARTIST_ALIASES → move to
/// matched genre, else archive.
const PER_FILE_SCAN: &[&str] = &["Dance", "Electronic"];

// ── Reports ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct PruneReport {
    pub rescued: usize,
    pub merged: usize,
    pub demoted: usize,
    pub scanned_kept: usize,
    pub scanned_archived: usize,
    pub archived_whole: usize,
    pub serato_db_updated: usize,
    pub manifest_path: String,
    pub errors: Vec<String>,
}

#[derive(Serialize)]
struct MoveRecord {
    from: String,
    to: String,
    action: &'static str,
}

#[derive(Serialize)]
struct ArchiveRecord {
    from: String,
    to: String,
    source_folder: String,
}

// ── Command ───────────────────────────────────────────────────────────────

#[tauri::command]
pub fn prune_library(
    app: AppHandle,
    crates_root: String,
    serato_path: Option<String>,
) -> Result<PruneReport, String> {
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
        .join(format!("CRATES_removed_{}", ts));

    let mut moves: Vec<MoveRecord> = Vec::new();
    let mut archives: Vec<ArchiveRecord> = Vec::new();
    let mut db_updates: HashMap<String, SeratoDbUpdate> = HashMap::new();
    let mut report = PruneReport {
        rescued: 0,
        merged: 0,
        demoted: 0,
        scanned_kept: 0,
        scanned_archived: 0,
        archived_whole: 0,
        serato_db_updated: 0,
        manifest_path: String::new(),
        errors: Vec::new(),
    };

    // Track which (source_folder, path) pairs have already been rescued so the
    // per-folder archive phase doesn't try to archive them again.
    let mut rescued_paths: HashSet<PathBuf> = HashSet::new();

    // ── Phase 1: artist rescues ─────────────────────────────────────────
    emit_progress(Some(&app), "prune_rescue", 0, 0, "Rescuing mistagged artists");
    for (src_genre, artist_norm, dest_genre) in RESCUES {
        let src_dir = genres.join(src_genre);
        if !src_dir.is_dir() {
            continue;
        }
        for file in collect_audio(&src_dir) {
            let Some(artist) = read_artist(&file) else {
                continue;
            };
            if normalize_artist(&artist) == *artist_norm {
                let filename = file
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("track")
                    .to_string();
                let dest_dir = genres.join(dest_genre);
                let dest = unique_dest(&dest_dir, &filename);
                match move_file(&file, &dest) {
                    Ok(_) => {
                        db_updates.insert(
                            to_serato_db_path(&file),
                            SeratoDbUpdate {
                                new_pfil: Some(to_serato_db_path(&dest)),
                                ..Default::default()
                            },
                        );
                        moves.push(MoveRecord {
                            from: file.to_string_lossy().into(),
                            to: dest.to_string_lossy().into(),
                            action: "rescue",
                        });
                        rescued_paths.insert(file);
                        report.rescued += 1;
                    }
                    Err(e) => report.errors.push(format!("rescue {}: {}", artist, e)),
                }
            }
        }
    }

    // ── Phase 2: whole-folder merges ────────────────────────────────────
    emit_progress(Some(&app), "prune_merge", 0, 0, "Merging redundant folders");
    for (src_name, dest_rel) in MERGES {
        let src_dir = genres.join(src_name);
        if !src_dir.is_dir() {
            continue;
        }
        let dest_dir = genres.join(dest_rel);
        for (full, rel) in audio_with_relpath(&src_dir) {
            if rescued_paths.contains(&full) {
                continue;
            }
            // Preserve subfolder structure from source into dest
            let dest_parent = if rel.components().count() == 1 {
                dest_dir.clone()
            } else {
                let mut d = dest_dir.clone();
                for seg in rel.iter().take(rel.components().count() - 1) {
                    d.push(seg);
                }
                d
            };
            let filename = rel
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("track")
                .to_string();
            let dest = unique_dest(&dest_parent, &filename);
            match move_file(&full, &dest) {
                Ok(_) => {
                    db_updates.insert(
                        to_serato_db_path(&full),
                        SeratoDbUpdate {
                            new_pfil: Some(to_serato_db_path(&dest)),
                            ..Default::default()
                        },
                    );
                    moves.push(MoveRecord {
                        from: full.to_string_lossy().into(),
                        to: dest.to_string_lossy().into(),
                        action: "merge",
                    });
                    report.merged += 1;
                }
                Err(e) => report.errors.push(format!("merge {}: {}", full.display(), e)),
            }
        }
        remove_empty_dirs(&src_dir);
    }

    // ── Phase 3: subgenre demotions ─────────────────────────────────────
    emit_progress(Some(&app), "prune_demote", 0, 0, "Demoting top-level genres to subgenres");
    for (src_name, parent, subname) in DEMOTIONS {
        let src_dir = genres.join(src_name);
        if !src_dir.is_dir() {
            continue;
        }
        let dest_dir = genres.join(parent).join(subname);
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
                    moves.push(MoveRecord {
                        from: full.to_string_lossy().into(),
                        to: dest.to_string_lossy().into(),
                        action: "demote",
                    });
                    report.demoted += 1;
                }
                Err(e) => report.errors.push(format!("demote {}: {}", full.display(), e)),
            }
        }
        remove_empty_dirs(&src_dir);
    }

    // ── Phase 4: per-file scan of Dance / Electronic ────────────────────
    emit_progress(Some(&app), "prune_scan", 0, 0, "Scanning Dance / Electronic per-file");
    for src_name in PER_FILE_SCAN {
        let src_dir = genres.join(src_name);
        if !src_dir.is_dir() {
            continue;
        }
        let archive_subdir = archive_root.join(src_name);
        let files = collect_audio(&src_dir);
        for file in files {
            if rescued_paths.contains(&file) {
                continue;
            }
            let filename = file
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("track")
                .to_string();
            let artist = read_artist(&file).unwrap_or_default();
            match alias_genre_for_artist(&artist) {
                Some(g) => {
                    let dest = unique_dest(&genres.join(g), &filename);
                    match move_file(&file, &dest) {
                        Ok(_) => {
                            db_updates.insert(
                                to_serato_db_path(&file),
                                SeratoDbUpdate {
                                    new_pfil: Some(to_serato_db_path(&dest)),
                                    ..Default::default()
                                },
                            );
                            moves.push(MoveRecord {
                                from: file.to_string_lossy().into(),
                                to: dest.to_string_lossy().into(),
                                action: "scan_kept",
                            });
                            report.scanned_kept += 1;
                        }
                        Err(e) => {
                            report.errors.push(format!("scan keep {}: {}", file.display(), e))
                        }
                    }
                }
                None => {
                    let dest = unique_dest(&archive_subdir, &filename);
                    match move_file(&file, &dest) {
                        Ok(_) => {
                            db_updates.insert(
                                to_serato_db_path(&file),
                                SeratoDbUpdate {
                                    delete: true,
                                    ..Default::default()
                                },
                            );
                            archives.push(ArchiveRecord {
                                from: file.to_string_lossy().into(),
                                to: dest.to_string_lossy().into(),
                                source_folder: src_name.to_string(),
                            });
                            report.scanned_archived += 1;
                        }
                        Err(e) => {
                            report.errors.push(format!("scan archive {}: {}", file.display(), e))
                        }
                    }
                }
            }
        }
        remove_empty_dirs(&src_dir);
    }

    // ── Phase 5: archive whole folders (what's left post-rescue) ────────
    emit_progress(Some(&app), "prune_archive", 0, 0, "Archiving non-Urban AC folders");
    for src_name in ARCHIVE_WHOLE {
        let src_dir = genres.join(src_name);
        if !src_dir.is_dir() {
            continue;
        }
        let archive_subdir = archive_root.join(src_name);
        for (full, rel) in audio_with_relpath(&src_dir) {
            if rescued_paths.contains(&full) {
                continue;
            }
            let dest = archive_subdir.join(&rel);
            let dest = unique_dest(
                dest.parent().unwrap_or(&archive_subdir),
                dest.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("track"),
            );
            match move_file(&full, &dest) {
                Ok(_) => {
                    db_updates.insert(
                        to_serato_db_path(&full),
                        SeratoDbUpdate {
                            delete: true,
                            ..Default::default()
                        },
                    );
                    archives.push(ArchiveRecord {
                        from: full.to_string_lossy().into(),
                        to: dest.to_string_lossy().into(),
                        source_folder: src_name.to_string(),
                    });
                    report.archived_whole += 1;
                }
                Err(e) => {
                    report
                        .errors
                        .push(format!("archive {}: {}", full.display(), e))
                }
            }
        }
        remove_empty_dirs(&src_dir);
    }

    // ── Serato db update ────────────────────────────────────────────────
    if let Some(sp) = serato_path.as_deref() {
        let db_path = PathBuf::from(sp).join("database V2");
        if db_path.exists() && !db_updates.is_empty() {
            emit_progress(Some(&app), "prune_db", 0, 0, "Updating Serato database V2");
            match update_serato_db(&db_path, &db_updates) {
                Ok(n) => report.serato_db_updated = n,
                Err(e) => report.errors.push(format!("serato db: {}", e)),
            }
        }
    }

    // ── Manifest ────────────────────────────────────────────────────────
    let manifest_dir = std::env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(".dj-crates-tools");
    let _ = fs::create_dir_all(&manifest_dir);
    let manifest_path = manifest_dir.join(format!("prune_manifest_{}.json", ts));
    #[derive(Serialize)]
    struct Manifest<'a> {
        timestamp: u64,
        archive_root: String,
        moves: &'a [MoveRecord],
        archives: &'a [ArchiveRecord],
    }
    let manifest = Manifest {
        timestamp: ts,
        archive_root: archive_root.to_string_lossy().into(),
        moves: &moves,
        archives: &archives,
    };
    if let Err(e) = fs::write(
        &manifest_path,
        serde_json::to_string_pretty(&manifest).unwrap_or_default(),
    ) {
        report.errors.push(format!("manifest: {}", e));
    }
    report.manifest_path = manifest_path.to_string_lossy().into();

    emit_progress(Some(&app), "prune_done", 0, 0, "Prune complete");
    Ok(report)
}
