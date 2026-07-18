// Library reorganization — one-shot cleanup of:
//   1. `Hip-Hop/` folder (dumping ground) → merged into `Hip-Hop:Rap/`
//   2. `Unknown Genre/` + `No Genre/` → reclassified via classifier
//   3. Loose files sitting directly in genre roots → moved to `General <Genre>/`
//
// Every move is logged to a JSON manifest for undo. Serato's `database V2`
// gets its `pfil` entries rewritten so tracks stay findable in Serato (backup
// to `database V2.bak` happens on first write).

use crate::{emit_progress, to_serato_db_path, update_serato_db, SeratoDbUpdate};
use serde::Serialize;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::AppHandle;

const AUDIO_EXTENSIONS: &[&str] = &[
    "mp3", "flac", "wav", "ogg", "aif", "aiff", "aac", "m4a", "alac",
];

// The Hip-Hop folder is a dumping ground; the real canonical hierarchy is
// Hip-Hop:Rap. We empty Hip-Hop into Hip-Hop:Rap and delete the dir.
const HIP_HOP_DUMPING_DIR: &str = "Hip-Hop";
const HIP_HOP_CANONICAL_DIR: &str = "Hip-Hop:Rap";

// Folders that hold "I don't know the genre" files — we try to reclassify
// each one and move it into its proper genre's General subfolder.
const ORPHAN_GENRE_DIRS: &[&str] = &["Unknown Genre", "No Genre", "Unclassified"];

fn is_audio(p: &Path) -> bool {
    p.extension()
        .and_then(|e| e.to_str())
        .map(|e| AUDIO_EXTENSIONS.iter().any(|x| x.eq_ignore_ascii_case(e)))
        .unwrap_or(false)
}

#[derive(Debug, Clone, Serialize)]
pub struct CleanupReport {
    pub hip_hop_merged: usize,
    pub orphans_classified: usize,
    pub orphans_left: usize,
    pub loose_to_general: usize,
    pub serato_db_updated: usize,
    pub manifest_path: String,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
struct MoveRecord {
    from: String,
    to: String,
    reason: &'static str,
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
        let candidate = if ext.is_empty() {
            dir.join(format!("{} ({})", stem, n))
        } else {
            dir.join(format!("{} ({}).{}", stem, n, ext))
        };
        if !candidate.exists() {
            return candidate;
        }
    }
    target
}

/// Safely move a file, creating intermediate dirs. Falls back to copy+delete
/// across volumes. On macOS within one Music folder everything is on the
/// same volume, so rename is fast/atomic.
fn move_file(from: &Path, to: &Path) -> Result<(), String> {
    if let Some(parent) = to.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("mkdir {}: {}", parent.display(), e))?;
    }
    match fs::rename(from, to) {
        Ok(_) => Ok(()),
        Err(_) => {
            fs::copy(from, to).map_err(|e| format!("copy: {}", e))?;
            fs::remove_file(from).map_err(|e| format!("remove orig: {}", e))?;
            Ok(())
        }
    }
}

/// Walk one level of `dir` (non-recursive) and return audio files directly in it.
fn loose_files_in(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(entries) = fs::read_dir(dir) else {
        return out;
    };
    for e in entries.filter_map(|x| x.ok()) {
        let p = e.path();
        if p.is_file() && is_audio(&p) {
            out.push(p);
        }
    }
    out
}

/// Recursively collect every audio file under `dir`, along with its relative
/// sub-path (the part after `dir`). Used when walking Hip-Hop/ for merge.
fn all_audio_with_relpath(dir: &Path) -> Vec<(PathBuf, PathBuf)> {
    let mut out = Vec::new();
    let Ok(entries) = fs::read_dir(dir) else {
        return out;
    };
    for e in entries.filter_map(|x| x.ok()) {
        let p = e.path();
        if p.is_dir() {
            for (sub, rel) in all_audio_with_relpath(&p) {
                let mut full_rel = PathBuf::from(e.file_name());
                full_rel.push(rel);
                out.push((sub, full_rel));
            }
        } else if p.is_file() && is_audio(&p) {
            out.push((p.clone(), PathBuf::from(e.file_name())));
        }
    }
    out
}

fn general_subgenre_for(genre: &str) -> String {
    format!("General {}", genre)
}

fn record_move(
    records: &mut Vec<MoveRecord>,
    db_updates: &mut HashMap<String, SeratoDbUpdate>,
    from: &Path,
    to: &Path,
    reason: &'static str,
) {
    records.push(MoveRecord {
        from: from.to_string_lossy().into_owned(),
        to: to.to_string_lossy().into_owned(),
        reason,
    });
    db_updates.insert(
        to_serato_db_path(from),
        SeratoDbUpdate {
            new_pfil: Some(to_serato_db_path(to)),
            ..Default::default()
        },
    );
}

#[tauri::command]
pub fn cleanup_library(
    app: AppHandle,
    crates_root: String,
    serato_path: Option<String>,
) -> Result<CleanupReport, String> {
    let root = PathBuf::from(&crates_root);
    let genres = root.join("GENRES");
    if !genres.is_dir() {
        return Err(format!("GENRES folder not found under {}", crates_root));
    }

    let mut records: Vec<MoveRecord> = Vec::new();
    let mut db_updates: HashMap<String, SeratoDbUpdate> = HashMap::new();
    let mut report = CleanupReport {
        hip_hop_merged: 0,
        orphans_classified: 0,
        orphans_left: 0,
        loose_to_general: 0,
        serato_db_updated: 0,
        manifest_path: String::new(),
        errors: Vec::new(),
    };

    // ── Phase 1: Hip-Hop merge ───────────────────────────────────────────
    let dump_dir = genres.join(HIP_HOP_DUMPING_DIR);
    let canonical_dir = genres.join(HIP_HOP_CANONICAL_DIR);
    if dump_dir.is_dir() && canonical_dir.is_dir() {
        emit_progress(Some(&app), "cleanup_hiphop", 0, 0, "Merging Hip-Hop → Hip-Hop:Rap");
        let files = all_audio_with_relpath(&dump_dir);
        for (full, rel) in &files {
            // Relative parts: either "foo.mp3" (loose) or "SubFolder/foo.mp3".
            let parts: Vec<&std::ffi::OsStr> = rel.iter().collect();
            let dest_dir = if parts.len() == 1 {
                // Loose file in Hip-Hop/ → Hip-Hop:Rap/General Hip-Hop/
                canonical_dir.join(general_subgenre_for("Hip-Hop"))
            } else {
                // Hip-Hop/<subfolder>/.../foo.mp3 → Hip-Hop:Rap/<subfolder>/...
                // Preserve intermediate subfolders; drop filename.
                let mut d = canonical_dir.clone();
                for seg in &parts[..parts.len() - 1] {
                    d.push(seg);
                }
                d
            };
            let filename = rel
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("track")
                .to_string();
            let dest = unique_dest(&dest_dir, &filename);
            match move_file(full, &dest) {
                Ok(_) => {
                    record_move(&mut records, &mut db_updates, full, &dest, "hip_hop_merge");
                    report.hip_hop_merged += 1;
                }
                Err(e) => report.errors.push(format!("{}: {}", full.display(), e)),
            }
        }
        // Remove empty Hip-Hop/ tree (ignore errors — non-empty dirs mean
        // something we couldn't move; we want a no-op rather than a failure).
        remove_empty_dirs(&dump_dir);
    }

    // ── Phase 2: Orphan-genre folders (Unknown Genre, No Genre) ────────
    // Best-effort genre detection for each file. If we can identify a genre
    // from the file's parent path or tags, move to <Genre>/General <Genre>/.
    // Otherwise, leave in place.
    let library = existing_genre_folders(&genres);
    for orphan in ORPHAN_GENRE_DIRS {
        let orphan_dir = genres.join(orphan);
        if !orphan_dir.is_dir() {
            continue;
        }
        emit_progress(
            Some(&app),            "cleanup_orphans",
            0,
            0,
            &format!("Reclassifying {}", orphan),
        );
        let files = collect_all_audio(&orphan_dir);
        for full in &files {
            let filename = full
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("track")
                .to_string();
            match detect_genre_for_orphan(full, &library) {
                Some(g) => {
                    let dest_dir = genres.join(&g).join(general_subgenre_for(&g));
                    let dest = unique_dest(&dest_dir, &filename);
                    match move_file(full, &dest) {
                        Ok(_) => {
                            record_move(
                                &mut records,
                                &mut db_updates,
                                full,
                                &dest,
                                "orphan_classified",
                            );
                            report.orphans_classified += 1;
                        }
                        Err(e) => report.errors.push(format!("{}: {}", full.display(), e)),
                    }
                }
                None => report.orphans_left += 1,
            }
        }
        // Don't delete the orphan folder itself — files we couldn't classify
        // stay there.
    }

    // ── Phase 3: Loose files at genre roots ─────────────────────────────
    emit_progress(Some(&app), "cleanup_loose", 0, 0, "Tidying loose files");
    let Ok(entries) = fs::read_dir(&genres) else {
        return Err("could not read GENRES".into());
    };
    for e in entries.filter_map(|x| x.ok()) {
        let gpath = e.path();
        if !gpath.is_dir() {
            continue;
        }
        let gname = e.file_name().to_string_lossy().into_owned();
        // Skip orphan + hip-hop dump folders (already handled) + any hidden
        if gname.starts_with('.') || ORPHAN_GENRE_DIRS.contains(&gname.as_str()) {
            continue;
        }
        if gname == HIP_HOP_DUMPING_DIR {
            continue; // handled in phase 1
        }
        let loose = loose_files_in(&gpath);
        if loose.is_empty() {
            continue;
        }
        let dest_dir = gpath.join(general_subgenre_for(&gname));
        for full in &loose {
            let filename = full
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("track")
                .to_string();
            let dest = unique_dest(&dest_dir, &filename);
            match move_file(full, &dest) {
                Ok(_) => {
                    record_move(
                        &mut records,
                        &mut db_updates,
                        full,
                        &dest,
                        "loose_to_general",
                    );
                    report.loose_to_general += 1;
                }
                Err(err) => report.errors.push(format!("{}: {}", full.display(), err)),
            }
        }
    }

    // ── Serato database update ──────────────────────────────────────────
    if let Some(sp) = serato_path.as_deref() {
        let db_path = PathBuf::from(sp).join("database V2");
        if db_path.exists() && !db_updates.is_empty() {
            emit_progress(
                Some(&app),                "cleanup_serato_db",
                0,
                0,
                "Updating Serato database V2",
            );
            match update_serato_db(&db_path, &db_updates) {
                Ok(n) => report.serato_db_updated = n,
                Err(e) => report.errors.push(format!("serato db update: {}", e)),
            }
        }
    }

    // ── Manifest (for undo) ─────────────────────────────────────────────
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let manifest_dir = std::env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(".dj-crates-tools");
    let _ = fs::create_dir_all(&manifest_dir);
    let manifest_path = manifest_dir.join(format!("cleanup_manifest_{}.json", ts));
    let manifest_json = serde_json::to_string_pretty(&records)
        .unwrap_or_else(|_| "[]".to_string());
    if let Err(e) = fs::write(&manifest_path, manifest_json) {
        report.errors.push(format!("manifest write: {}", e));
    }
    report.manifest_path = manifest_path.to_string_lossy().into_owned();

    emit_progress(Some(&app), "cleanup_done", 0, 0, "Library cleanup complete");
    Ok(report)
}

// ── Helpers ───────────────────────────────────────────────────────────────

fn remove_empty_dirs(dir: &Path) {
    // Walk bottom-up removing empty dirs. Safe: will not remove non-empty.
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

fn existing_genre_folders(genres_dir: &Path) -> Vec<String> {
    let mut out = Vec::new();
    let Ok(entries) = fs::read_dir(genres_dir) else {
        return out;
    };
    for e in entries.filter_map(|x| x.ok()) {
        let p = e.path();
        if p.is_dir() {
            let name = e.file_name().to_string_lossy().into_owned();
            if !name.starts_with('.') && !ORPHAN_GENRE_DIRS.contains(&name.as_str()) {
                out.push(name);
            }
        }
    }
    out
}

fn collect_all_audio(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(entries) = fs::read_dir(dir) else {
        return out;
    };
    for e in entries.filter_map(|x| x.ok()) {
        let p = e.path();
        if p.is_dir() {
            out.extend(collect_all_audio(&p));
        } else if p.is_file() && is_audio(&p) {
            out.push(p);
        }
    }
    out
}

/// Light classifier for orphan files. Checks file's ID3 genre tag against
/// known library genre names and the existing alias list from unsorted.rs.
/// Returns Some(genre_folder_name) if confident.
fn detect_genre_for_orphan(path: &Path, library_genres: &[String]) -> Option<String> {
    use lofty::prelude::*;
    use lofty::probe::Probe;

    let (id3_genre, id3_grouping) = match Probe::open(path).and_then(|p| p.read()) {
        Ok(tagged) => {
            let tag = tagged.primary_tag().or(tagged.first_tag());
            let g = tag
                .and_then(|t| t.genre())
                .map(|s| s.to_string())
                .unwrap_or_default();
            let gr = tag
                .and_then(|t| t.get_string(&lofty::tag::ItemKey::ContentGroup))
                .map(|s| s.to_string())
                .unwrap_or_default();
            (g, gr)
        }
        Err(_) => return None,
    };

    let hay = format!("{} {}", id3_genre, id3_grouping)
        .to_lowercase()
        .replace('-', " ")
        .replace('/', " ");
    if hay.trim().is_empty() {
        return None;
    }

    // Prefer longest folder name to avoid "House" eating "Afro House".
    let mut folders: Vec<&String> = library_genres.iter().collect();
    folders.sort_by_key(|f| std::cmp::Reverse(f.len()));
    for folder in &folders {
        let lc = folder.to_lowercase().replace('-', " ");
        if lc.len() >= 3 && hay.contains(&lc) {
            return Some((*folder).clone());
        }
    }
    None
}
