use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use walkdir::WalkDir;

const AUDIO_EXTENSIONS: &[&str] = &[
    "mp3", "flac", "wav", "ogg", "aif", "aiff", "aac", "m4a", "alac",
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
pub struct CrateSyncResult {
    pub crates_written: usize,
    pub total_track_entries: usize,
    pub top_level: Vec<TopLevelCrate>,
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

// ── Serato Crate Binary Writer ─────────────────────────────────────────────

fn encode_utf16be(text: &str) -> Vec<u8> {
    let mut bytes = Vec::new();
    for c in text.encode_utf16() {
        bytes.push((c >> 8) as u8);
        bytes.push(c as u8);
    }
    bytes
}

fn make_tlv(tag: &str, data: &[u8]) -> Vec<u8> {
    let mut result = Vec::new();
    result.extend_from_slice(tag.as_bytes());
    result.extend_from_slice(&(data.len() as u32).to_be_bytes());
    result.extend_from_slice(data);
    result
}

fn build_crate_bytes(track_paths: &[String]) -> Vec<u8> {
    let mut parts = Vec::new();

    // Version header
    let version = encode_utf16be("1.0/Serato ScratchLive Crate");
    parts.extend(make_tlv("vrsn", &version));

    // Column definitions
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

    // Track entries (sorted)
    let mut sorted_paths = track_paths.to_vec();
    sorted_paths.sort();
    for path in &sorted_paths {
        let ptrk_data = make_tlv("ptrk", &encode_utf16be(path));
        parts.extend(make_tlv("otrk", &ptrk_data));
    }

    parts
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

        // Extract genre and subgenre from path
        if let Ok(rel) = path.strip_prefix(&root) {
            let parts: Vec<&str> = rel.components()
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

#[tauri::command]
fn sync_to_serato(crates_root: String, serato_path: String, clean: bool) -> Result<CrateSyncResult, String> {
    let crates_dir = PathBuf::from(&crates_root);
    let subcrates_dir = PathBuf::from(&serato_path).join("Subcrates");

    if !crates_dir.exists() {
        return Err(format!("Crates directory not found: {}", crates_root));
    }
    if !subcrates_dir.exists() {
        return Err(format!("Serato Subcrates directory not found: {}", subcrates_dir.display()));
    }

    // Clean existing synced crates if requested
    if clean {
        if let Ok(entries) = fs::read_dir(&subcrates_dir) {
            for entry in entries.filter_map(|e| e.ok()) {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.ends_with(".crate")
                    && (name.starts_with("GENRES%%") || name.starts_with("PLAYLISTS%%")
                        || name == "GENRES.crate" || name == "PLAYLISTS.crate")
                {
                    let _ = fs::remove_file(entry.path());
                }
            }
        }
    }

    // Scan directory tree and build crate hierarchy
    let mut crates: HashMap<String, Vec<String>> = HashMap::new();

    for entry in WalkDir::new(&crates_dir)
        .into_iter()
        .filter_entry(|e| !e.file_name().to_string_lossy().starts_with('.'))
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if !path.is_file() || !is_audio_file(path) {
            continue;
        }

        if let Ok(rel) = path.strip_prefix(&crates_dir) {
            let parts: Vec<String> = rel
                .parent()
                .map(|p| {
                    p.components()
                        .filter_map(|c| c.as_os_str().to_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default();

            let track_path = to_serato_path(path);

            // Add track to every hierarchy level
            for i in 1..=parts.len() {
                let crate_name = parts[..i].join("%%");
                crates
                    .entry(crate_name)
                    .or_default()
                    .push(track_path.clone());
            }
        }
    }

    // Write crate files
    let mut crates_written = 0;
    let mut total_track_entries = 0;

    let mut sorted_names: Vec<String> = crates.keys().cloned().collect();
    sorted_names.sort_by_key(|k| k.matches("%%").count());

    for crate_name in &sorted_names {
        if let Some(tracks) = crates.get(crate_name) {
            if tracks.is_empty() {
                continue;
            }
            let data = build_crate_bytes(tracks);
            let crate_file = subcrates_dir.join(format!("{}.crate", crate_name));

            if let Ok(mut file) = fs::File::create(&crate_file) {
                if file.write_all(&data).is_ok() {
                    crates_written += 1;
                    total_track_entries += tracks.len();
                }
            }
        }
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

    Ok(CrateSyncResult {
        crates_written,
        total_track_entries,
        top_level,
    })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            scan_library,
            run_python_script,
            sync_to_serato,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
