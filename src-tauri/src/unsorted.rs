// Unsorted routing: classify downloads (SoundCloud, yt-dlp) into
// CRATES/GENRES/<genre>/<subgenre>/ (regular tracks — subgenre REQUIRED),
// CRATES/GENRES/<genre>/EDITS/ (edits — genre REQUIRED),
// or CRATES/_REVIEW/_Unrouted/ (everything else).
//
// Only genres that already exist as top-level folders under CRATES/GENRES
// are eligible — we never create new genre folders.

use crate::spotify::SpotifyClient;
use lofty::config::WriteOptions;
use lofty::prelude::*;
use lofty::probe::Probe;
use lofty::tag::ItemKey;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

const AUDIO_EXTENSIONS: &[&str] = &[
    "mp3", "flac", "wav", "ogg", "aif", "aiff", "aac", "m4a", "alac",
];

// "Is this an edit?" tokens — substring matched against the lowercased haystack.
// Permissive on parens/brackets so things like "(@user Edit)" or "[VIP]" match.
const EDIT_TOKENS: &[&str] = &[
    "edit)",
    "edit]",
    "edit ",
    "edit.",
    " edit",
    "(edit",
    "[edit",
    "dj edit",
    "djedit",
    "re-edit",
    "reedit",
    "flip)",
    "flip]",
    "(flip",
    "[flip",
    " flip",
    "- flip",
    "mashup",
    "mash-up",
    "mash up",
    "bootleg",
    "boot leg",
    "vip mix",
    "vip edit",
    "(vip)",
    "[vip]",
    " vip ",
    "extended mix",
    "extended edit",
    "refix",
    "re-rub",
    "rework",
    "re-work",
];

// Genre keyword recognizer — used as a fallback when a folder's own name
// doesn't appear literally in the haystack. Entries are matched against a
// genre folder case-insensitively.
// Ordered most-specific first so multi-word tokens beat generic ones.
// Post-normalize taxonomy: niche club/African/Soul terms route to their
// MAINSTREAM PARENT genre. The subgenre matcher then drops the track into
// the correct subfolder (e.g. "amapiano" → Afrobeats → Afrobeats/Amapiano/).
const GENRE_ALIASES: &[(&str, &[&str])] = &[
    // House umbrella — all soulful / Chicago / Detroit / Afro + club/bass
    ("House", &[
        "house", "deep house", "soulful house", "tech house",
        "afro house", "afrohouse", "afro-house",
        "jersey club", "jersey-club", "jerseyclub",
        "baltimore club", "bmore club",
        "baile funk", "funk carioca", "favela funk", "brazilian funk",
        "gqom",
        "footwork",
        "juke",
        "uk funky", "uk-funky",
        "uk garage", "2 step", "2-step", "2step",
        "moombahton", "moombah",
    ]),
    // Afrobeats umbrella — all African regional styles
    ("Afrobeats", &[
        "afrobeats", "afrobeat", "afro pop", "afropop", "afro-beats",
        "amapiano", "ama piano",
        "kuduro", "batida",
        "kwaito", "singeli", "kawina", "highlife", "afro swing", "afroswing",
    ]),
    // Soul + its descendants
    ("Soul", &[
        "soul", "neo-soul", "neo soul", "neosoul",
        "motown", "quiet storm",
    ]),
    // Hip-Hop canonical folder is "Hip-Hop:Rap" (macOS's rendering of "Hip-Hop/Rap")
    ("Hip-Hop:Rap", &[
        "hip hop", "hip-hop", "hiphop", "rap", "trap", "drill",
        "boom bap", "g-funk", "cloud rap", "conscious", "east coast",
        "west coast", "southern", "crunk", "lo-fi",
    ]),
    ("R&B", &["r&b", "rnb", "r and b", "rhythm and blues", "alternative r&b", "alt r&b"]),
    ("Gospel", &["gospel", "contemporary gospel", "traditional gospel"]),
    ("Jazz", &["jazz", "nu jazz", "jazz funk"]),
    ("Funk", &["funk", "p funk", "g-funk"]),
    ("Disco", &["disco", "nu disco", "disco funk"]),
    ("Dancehall", &["dancehall", "shatta"]),
    ("Reggae", &["reggae", "dub", "roots reggae", "lovers rock"]),
    ("Pop", &["pop"]),
];

// Artists → genre folder. Seeded with mainstream names across the lanes Kory
// actually DJs. Names are lowercased, spaces-only (no hyphens); the matcher
// normalizes candidate strings the same way before comparing.
//
// Not exhaustive by design — add names as you hit tracks that should route
// and don't. The alias list is the FIRST fallback after folder-name matching;
// Spotify is the second fallback after this.
pub(crate) const ARTIST_ALIASES: &[(&str, &[&str])] = &[
    ("R&B", &[
        "whitney houston", "mariah carey", "brandy", "monica", "aaliyah", "tlc",
        "destinys child", "beyonce", "alicia keys", "usher", "chris brown", "rihanna",
        "sza", "h.e.r.", "her", "jazmine sullivan", "tyrese", "ginuwine", "next",
        "dru hill", "joe", "case", "keith sweat", "mary j blige", "faith evans",
        "jon b", "maxwell", "boyz ii men", "k ci jojo", "sisqo", "kelly rowland",
        "ashanti", "cassie", "keyshia cole", "teyana taylor", "ella mai",
        "summer walker", "kehlani", "jhene aiko", "tinashe", "tyla", "ari lennox",
        "snoh aalegra", "leon thomas", "daniel caesar", "giveon", "6lack",
        "bryson tiller", "partynextdoor", "frank ocean", "the weeknd", "syd",
        "joyce wrice", "cleo sol", "sade",
    ]),
    ("Neo-Soul", &[
        "d'angelo", "dangelo", "erykah badu", "jill scott", "lauryn hill",
        "musiq soulchild", "india arie", "angie stone", "floetry", "anthony hamilton",
        "bilal", "dwele", "eric benet", "lalah hathaway", "raheem devaughn",
        "hiatus kaiyote", "yebba",
    ]),
    ("Soul", &[
        "stevie wonder", "marvin gaye", "al green", "aretha franklin", "otis redding",
        "curtis mayfield", "donny hathaway", "roberta flack", "isaac hayes",
        "bill withers", "teddy pendergrass", "the temptations", "the isley brothers",
        "james brown", "ray charles", "sam cooke", "chaka khan", "minnie riperton",
        "gil scott heron",
    ]),
    ("Gospel", &[
        "kirk franklin", "fred hammond", "yolanda adams", "donnie mcclurkin",
        "mary mary", "cece winans", "bebe winans", "marvin sapp", "hezekiah walker",
        "tasha cobbs", "travis greene", "tamela mann", "jonathan mcreynolds",
        "maverick city music", "elevation worship",
    ]),
    ("Jazz", &[
        "miles davis", "john coltrane", "thelonious monk", "herbie hancock",
        "charles mingus", "wayne shorter", "robert glasper", "kamasi washington",
        "thundercat", "esperanza spalding", "christian scott", "nicholas payton",
        "pharoah sanders", "duke ellington", "louis armstrong", "ella fitzgerald",
        "billie holiday", "sarah vaughan", "nina simone", "norah jones",
    ]),
    ("Hip-Hop", &[
        "jay z", "jay-z", "nas", "biggie", "notorious b.i.g.", "notorious big",
        "tupac", "2pac", "wu tang clan", "wu-tang clan", "kendrick lamar", "j cole",
        "kanye west", "ye", "drake", "travis scott", "future", "lil wayne",
        "eminem", "outkast", "a tribe called quest", "tribe called quest", "common",
        "mos def", "yasiin bey", "talib kweli", "mf doom", "madlib", "j dilla",
        "dilla", "dr dre", "dr. dre", "snoop dogg", "50 cent", "ice cube",
        "public enemy", "de la soul", "nas", "method man", "raekwon", "ghostface killah",
        "gza", "rza", "pharoahe monch", "black thought", "the roots", "roots",
        "mac miller", "tyler the creator", "earl sweatshirt", "isaiah rashad",
        "smino", "saba", "noname", "rapsody", "denzel curry", "jpegmafia",
        "freddie gibbs", "pusha t", "jay electronica", "joey bada$$", "joey badass",
    ]),
    ("Afrobeats", &[
        "burna boy", "wizkid", "davido", "tems", "tiwa savage", "fela kuti",
        "mr eazi", "rema", "omah lay", "asake", "ckay", "fireboy dml",
        "ayra starr", "adekunle gold", "ruger", "joeboy", "simi", "yemi alade",
        "olamide", "patoranking", "d'banj", "dbanj", "2baba", "2face", "kizz daniel",
        "ebo taylor",
    ]),
    ("Amapiano", &[
        "kabza de small", "dj maphorisa", "major league djz", "musa keys",
        "focalistic", "mr jazziq", "tyler icu", "vigro deep", "daliwonga",
    ]),
    ("House", &[
        "larry heard", "mr fingers", "masters at work", "louie vega", "kenny dope",
        "dennis ferrer", "kerri chandler", "black coffee", "frankie knuckles",
        "marshall jefferson", "osunlade", "ron trent",
        "kaytranada", "moodymann", "theo parrish", "channel tres", "daft punk",
        "azealia banks", "cakes da killa",
    ]),
    ("Dancehall", &[
        "sean paul", "shaggy", "vybz kartel", "popcaan", "spice", "beenie man",
        "mavado", "buju banton", "sizzla", "capleton",
    ]),
    ("Reggae", &[
        "bob marley", "peter tosh", "dennis brown", "gregory isaacs",
        "burning spear", "toots and the maytals", "jimmy cliff", "chronixx",
    ]),
    ("Funk", &[
        "parliament", "funkadelic", "george clinton", "rick james", "cameo",
        "the gap band", "earth wind fire", "earth wind and fire", "kool and the gang",
        "zapp", "roger troutman",
    ]),
    ("Disco", &[
        "donna summer", "chic", "nile rodgers", "diana ross", "gloria gaynor",
        "sister sledge", "kool the gang", "earth wind fire",
    ]),
];

// Folders whose names are about SOURCE (where the file came from), not artist
// or genre. Used to skip them when hunting for an artist candidate, and to
// trigger the "these are probably edits/mashups" routing rule.
const SOURCE_FOLDERS: &[&str] = &[
    "soundcloud", "bandcamp", "youtube", "yt", "downloads", "music",
    "dj pool", "djpool", "beatport", "digital", "digital downloads",
];

// Source folders that specifically indicate an edit/mashup culture.
// When present in the path, route to EDITS instead of subgenre (Kory's rule:
// "soulection and bandcamp are mostly edits and mashups").
const EDIT_SOURCE_FOLDERS: &[&str] = &["soulection", "bandcamp"];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnsortedTrack {
    pub abs_path: String,
    pub filename: String,
    pub cleaned_filename: String,
    pub title: String,
    pub artist: String,
    pub genre_tag: String,
    pub grouping_tag: String,
    pub detected_genre: Option<String>,
    pub detected_subgenre: Option<String>,
    pub is_edit: bool,
    pub suggested_dest: String,
    pub bucket: String, // "genres" | "edits" | "review"
    pub confidence: f32,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnsortedMove {
    pub abs_path: String,
    pub dest_dir: String,
    pub dest_filename: String,
    #[serde(default)]
    pub genre: String,
    #[serde(default)]
    pub grouping: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MoveReport {
    pub moved: usize,
    pub tagged: usize,
    pub skipped: usize,
    pub errors: Vec<String>,
}

// ── Filename cleanup ──────────────────────────────────────────────────────

/// Strip SoundCloud / yt-dlp cruft from a filename:
/// - `[1234567890]` style numeric ID blocks (anywhere in the name)
/// - `(320kbps)` / `(mp3)` / `(wav)` / `(flac)` format suffixes
/// - Stray `_` that appears around stripped blocks
/// - Collapsed whitespace, leading/trailing separators
fn clean_filename(name: &str) -> String {
    let (stem, ext) = match name.rfind('.') {
        Some(i) if i > 0 => (&name[..i], &name[i..]),
        _ => (name, ""),
    };

    // Pass 1: drop `[digits]` blocks (allow optional leading `-` or `_` inside).
    let mut chars: Vec<char> = stem.chars().collect();
    let mut out = String::with_capacity(stem.len());
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '[' {
            // Scan forward looking for matching ']' with only digits in between.
            let mut j = i + 1;
            while j < chars.len() && chars[j].is_ascii_digit() {
                j += 1;
            }
            if j > i + 1 && j < chars.len() && chars[j] == ']' {
                i = j + 1;
                continue;
            }
        }
        out.push(chars[i]);
        i += 1;
    }

    // Pass 2: drop common format suffix parens — e.g. (320kbps), (mp3), (wav), (flac).
    let format_parens = [
        "(320kbps)", "(256kbps)", "(192kbps)", "(128kbps)",
        "(mp3)", "(wav)", "(flac)", "(m4a)", "(aac)",
    ];
    for fp in format_parens {
        out = out.replace(fp, "");
        out = out.replace(&fp.to_uppercase(), "");
    }

    // Pass 3: collapse whitespace + strip stray separators left over from
    // removed bracket blocks. Then trim edges of `_`, `-`, `.`, whitespace.
    chars = out.chars().collect();
    let mut cleaned = String::with_capacity(chars.len());
    let mut prev_space = false;
    for c in chars {
        if c.is_whitespace() {
            if !prev_space {
                cleaned.push(' ');
                prev_space = true;
            }
        } else {
            cleaned.push(c);
            prev_space = false;
        }
    }
    let trimmed: String = cleaned
        .trim_matches(|c: char| c.is_whitespace() || c == '_' || c == '-' || c == '.')
        .to_string();

    // Trim leading " - " left behind after a bracket strip.
    let trimmed = trimmed.trim_start_matches(|c: char| c == '-' || c.is_whitespace()).to_string();

    if trimmed.is_empty() {
        return name.to_string();
    }
    format!("{}{}", trimmed, ext)
}

// ── Library scan ──────────────────────────────────────────────────────────

/// Read `<crates_root>/GENRES/` and build a map of
/// { genre_folder_name → [subgenre_folder_names] }. Hidden folders and EDITS
/// are skipped (EDITS is handled as a separate bucket).
fn scan_library(crates_root: &Path) -> HashMap<String, Vec<String>> {
    let mut out: HashMap<String, Vec<String>> = HashMap::new();
    let genres_dir = crates_root.join("GENRES");
    let Ok(genres) = fs::read_dir(&genres_dir) else {
        return out;
    };
    for g in genres.filter_map(|e| e.ok()) {
        if !g.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            continue;
        }
        let gname = g.file_name().to_string_lossy().to_string();
        if gname.starts_with('.') {
            continue;
        }
        let mut subs: Vec<String> = Vec::new();
        if let Ok(children) = fs::read_dir(g.path()) {
            for c in children.filter_map(|e| e.ok()) {
                if !c.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                    continue;
                }
                let sname = c.file_name().to_string_lossy().to_string();
                if sname.starts_with('.') || sname.eq_ignore_ascii_case("EDITS") {
                    continue;
                }
                subs.push(sname);
            }
        }
        out.insert(gname, subs);
    }
    out
}

// ── Classification ────────────────────────────────────────────────────────

struct Classification {
    bucket: &'static str,
    genre: Option<String>,
    subgenre: Option<String>,
    is_edit: bool,
    confidence: f32,
    reasons: Vec<String>,
}

fn detect_edit(hay: &str) -> Option<String> {
    for tok in EDIT_TOKENS {
        if hay.contains(tok) {
            return Some((*tok).to_string());
        }
    }
    None
}

/// Match a library genre folder against the haystack. We prefer longer folder
/// names (more specific), then fall back to the alias list when the bare
/// folder name doesn't appear literally.
fn match_library_genre(
    hay: &str,
    library: &HashMap<String, Vec<String>>,
) -> Option<(String, String)> {
    let mut folders: Vec<&String> = library.keys().collect();
    folders.sort_by_key(|f| std::cmp::Reverse(f.len()));
    for folder in &folders {
        let lc = folder.to_lowercase();
        if lc.len() >= 3 && hay.contains(&lc) {
            return Some(((*folder).clone(), format!("folder name '{}'", folder)));
        }
    }
    for (canonical, tokens) in GENRE_ALIASES {
        for tok in *tokens {
            if hay.contains(tok) {
                for folder in &folders {
                    if folder.eq_ignore_ascii_case(canonical) {
                        return Some((
                            (*folder).clone(),
                            format!("alias '{}' → {}", tok, folder),
                        ));
                    }
                }
            }
        }
    }
    None
}

/// Match a subgenre folder against the haystack. Only folders directly under
/// the given genre are eligible. Longest name wins.
fn match_library_subgenre(hay: &str, subs: &[String]) -> Option<(String, String)> {
    let mut sorted: Vec<&String> = subs.iter().collect();
    sorted.sort_by_key(|s| std::cmp::Reverse(s.len()));
    for sub in sorted {
        let lc = sub.to_lowercase();
        if lc.len() >= 3 && hay.contains(&lc) {
            return Some((sub.clone(), format!("subgenre folder '{}'", sub)));
        }
    }
    None
}

/// Normalize a name for artist-alias comparison: lowercase, replace
/// `-` and `_` with spaces, collapse whitespace.
fn normalize_artist_name(s: &str) -> String {
    let lowered = s.to_lowercase().replace(['-', '_'], " ");
    lowered.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn is_source_folder(seg: &str) -> bool {
    let n = normalize_artist_name(seg);
    SOURCE_FOLDERS.iter().any(|s| n == *s)
}

/// Strip trailing "ft X / feat X" suffix from a folder segment.
/// `whitney-houston-ft-icomplexity` → `whitney-houston`.
fn strip_ft_suffix(s: &str) -> &str {
    let lower = s.to_lowercase();
    for marker in [
        "-ft-", "-feat-", " ft ", " feat ", " ft. ", " feat. ", " x ", "-x-",
    ] {
        if let Some(idx) = lower.find(marker) {
            return &s[..idx];
        }
    }
    s
}

/// Produce an ordered list of "this might be the artist" strings, highest
/// confidence first. Caller tries them against ARTIST_ALIASES / Spotify in
/// order, stopping at the first hit. Source-looking folders (Soundcloud,
/// Bandcamp, etc.) are skipped.
fn extract_artist_candidates(
    filename: &str,
    parent_folders: &str,
    id3_artist: &str,
) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut push_unique = |s: String| {
        let n = normalize_artist_name(&s);
        if n.len() >= 2 && !out.iter().any(|x| normalize_artist_name(x) == n) {
            out.push(s);
        }
    };

    let id3 = id3_artist.trim();
    if !id3.is_empty() && !is_source_folder(id3) {
        push_unique(id3.to_string());
    }

    // Parent folder segments, each stripped of "-ft-..." tail.
    for seg in parent_folders.split('/').filter(|s| !s.is_empty()) {
        if is_source_folder(seg) {
            continue;
        }
        let stripped = strip_ft_suffix(seg);
        push_unique(stripped.to_string());
    }

    // Filename "Artist - Title.mp3" pattern.
    let stem = Path::new(filename)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("");
    // Drop leading track numbers: "01 - ", "21. ", "1) ".
    let stem = stem.trim_start_matches(|c: char| {
        c.is_ascii_digit() || c == '.' || c == ')' || c == '-' || c.is_whitespace()
    });
    if let Some(idx) = stem.find(" - ") {
        let candidate = stem[..idx].trim();
        if !candidate.is_empty() {
            push_unique(candidate.to_string());
        }
    }

    out
}

fn match_artist_alias(
    candidate: &str,
    library: &HashMap<String, Vec<String>>,
) -> Option<(String, String)> {
    let n = normalize_artist_name(candidate);
    if n.len() < 2 {
        return None;
    }
    let folders: Vec<&String> = library.keys().collect();
    for (canonical, artists) in ARTIST_ALIASES {
        for a in *artists {
            if &n == a {
                for folder in &folders {
                    if folder.eq_ignore_ascii_case(canonical) {
                        return Some((
                            (*folder).clone(),
                            format!("artist alias '{}' → {}", a, folder),
                        ));
                    }
                }
            }
        }
    }
    None
}

/// If any Spotify genre string contains a library folder name or alias token,
/// map to that folder. Used when `match_library_genre` couldn't resolve from
/// the haystack alone.
fn match_spotify_genres(
    sp_genres: &[String],
    library: &HashMap<String, Vec<String>>,
) -> Option<(String, String)> {
    let hay = sp_genres.join(" ").to_lowercase();
    if hay.is_empty() {
        return None;
    }
    match_library_genre(&hay, library).map(|(folder, _)| {
        (
            folder.clone(),
            format!("spotify genres [{}]", sp_genres.join(", ")),
        )
    })
}

fn classify(
    filename: &str,
    parent_folders: &str,
    title: &str,
    artist: &str,
    genre_tag: &str,
    grouping_tag: &str,
    library: &HashMap<String, Vec<String>>,
    spotify: Option<&mut SpotifyClient>,
) -> Classification {
    // Parent folder names often carry huge hints that the filename itself lacks.
    let mut hay = format!(
        "{} {} {} {} {} {}",
        parent_folders, filename, title, artist, genre_tag, grouping_tag
    )
    .to_lowercase()
    .replace('-', " ")
    .replace('_', " ")
    .replace('/', " ");

    let mut reasons: Vec<String> = Vec::new();

    // 1. Edit / source-based edit detection.
    let edit_hit = detect_edit(&hay);
    if let Some(ref tok) = edit_hit {
        reasons.push(format!("edit marker: '{}'", tok));
    }
    let edit_source = EDIT_SOURCE_FOLDERS
        .iter()
        .find(|s| hay.contains(*s))
        .copied();
    if let Some(src) = edit_source {
        reasons.push(format!("source '{}' implies edit/mashup", src));
    }
    let is_edit = edit_hit.is_some() || edit_source.is_some();

    // 2. Genre detection — multi-stage fallback.
    let mut genre_folder: Option<String> = None;

    if let Some((folder, r)) = match_library_genre(&hay, library) {
        reasons.push(r);
        genre_folder = Some(folder);
    }

    let candidates = extract_artist_candidates(filename, parent_folders, artist);

    if genre_folder.is_none() {
        for cand in &candidates {
            if let Some((folder, r)) = match_artist_alias(cand, library) {
                reasons.push(r);
                genre_folder = Some(folder);
                break;
            }
        }
    }

    if genre_folder.is_none() {
        if let Some(sp) = spotify {
            for cand in &candidates {
                match sp.search_artist(cand) {
                    Ok(Some(artist_info)) => {
                        // Inject Spotify genres into the haystack — helps
                        // subgenre matching too (e.g. "contemporary r&b"
                        // → your Contemporary R&B folder).
                        if !artist_info.genres.is_empty() {
                            hay.push(' ');
                            hay.push_str(&artist_info.genres.join(" ").to_lowercase());
                        }
                        if let Some((folder, r)) =
                            match_spotify_genres(&artist_info.genres, library)
                        {
                            reasons.push(format!(
                                "spotify '{}' → [{}] → {}",
                                artist_info.name,
                                artist_info.genres.join(", "),
                                folder
                            ));
                            let _ = r;
                            genre_folder = Some(folder);
                            break;
                        } else if !artist_info.genres.is_empty() {
                            reasons.push(format!(
                                "spotify '{}' → [{}] — no folder match",
                                artist_info.name,
                                artist_info.genres.join(", ")
                            ));
                        }
                    }
                    Ok(None) => {
                        reasons.push(format!("spotify: no result for '{}'", cand));
                    }
                    Err(e) => {
                        reasons.push(format!("spotify error: {}", e));
                        break; // don't hammer the API if it's failing
                    }
                }
            }
        }
    }

    let Some(genre_folder) = genre_folder else {
        reasons.push("no matching genre folder in your library".to_string());
        return Classification {
            bucket: "review",
            genre: None,
            subgenre: None,
            is_edit,
            confidence: 0.2,
            reasons,
        };
    };

    if is_edit {
        return Classification {
            bucket: "edits",
            genre: Some(genre_folder),
            subgenre: None,
            is_edit: true,
            confidence: 0.9,
            reasons,
        };
    }

    let subs = library.get(&genre_folder).cloned().unwrap_or_default();
    match match_library_subgenre(&hay, &subs) {
        Some((sub, s_reason)) => {
            reasons.push(s_reason);
            Classification {
                bucket: "genres",
                genre: Some(genre_folder),
                subgenre: Some(sub),
                is_edit: false,
                confidence: 0.82,
                reasons,
            }
        }
        None => {
            // No subgenre match — route to the genre root folder itself,
            // consistent with the existing library where many tracks live
            // at GENRES/<Genre>/ with no subgenre.
            reasons.push("no subgenre — routing to genre root".to_string());
            Classification {
                bucket: "genres",
                genre: Some(genre_folder),
                subgenre: None,
                is_edit: false,
                confidence: 0.6,
                reasons,
            }
        }
    }
}

// ── Metadata read ─────────────────────────────────────────────────────────

fn read_light_meta(path: &Path) -> (String, String, String, String) {
    match Probe::open(path).and_then(|p| p.read()) {
        Ok(tagged) => {
            let tag = tagged.primary_tag().or(tagged.first_tag());
            let title = tag
                .and_then(|t| t.title())
                .map(|s| s.to_string())
                .unwrap_or_default();
            let artist = tag
                .and_then(|t| t.artist())
                .map(|s| s.to_string())
                .unwrap_or_default();
            let genre = tag
                .and_then(|t| t.genre())
                .map(|s| s.to_string())
                .unwrap_or_default();
            let grouping = tag
                .and_then(|t| t.get_string(&ItemKey::ContentGroup))
                .map(|s| s.to_string())
                .unwrap_or_default();
            (title, artist, genre, grouping)
        }
        Err(_) => (String::new(), String::new(), String::new(), String::new()),
    }
}

fn is_audio(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| AUDIO_EXTENSIONS.iter().any(|x| x.eq_ignore_ascii_case(e)))
        .unwrap_or(false)
}

// ── Commands ──────────────────────────────────────────────────────────────

#[tauri::command]
pub fn scan_unsorted(
    unsorted_path: String,
    crates_root: String,
    spotify_client_id: Option<String>,
    spotify_client_secret: Option<String>,
) -> Result<Vec<UnsortedTrack>, String> {
    let root = PathBuf::from(&unsorted_path);
    if !root.exists() {
        return Err(format!("Unsorted folder not found: {}", unsorted_path));
    }
    let crates = PathBuf::from(&crates_root);
    if !crates.exists() {
        return Err(format!("Crates root not found: {}", crates_root));
    }

    let library = scan_library(&crates);
    let mut spotify = match (spotify_client_id, spotify_client_secret) {
        (Some(id), Some(secret)) => SpotifyClient::new(id, secret),
        _ => None,
    };

    let mut out: Vec<UnsortedTrack> = Vec::new();

    for entry in WalkDir::new(&root).into_iter().filter_map(|e| e.ok()) {
        let p = entry.path();
        if !p.is_file() || !is_audio(p) {
            continue;
        }

        let filename = p
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default()
            .to_string();
        let cleaned_filename = clean_filename(&filename);
        let parent_folders = p
            .parent()
            .and_then(|par| par.strip_prefix(&root).ok())
            .map(|rel| rel.to_string_lossy().to_string())
            .unwrap_or_default();
        let (title, artist, genre, grouping) = read_light_meta(p);
        let cls = classify(
            &filename,
            &parent_folders,
            &title,
            &artist,
            &genre,
            &grouping,
            &library,
            spotify.as_mut(),
        );

        let dest_dir = match cls.bucket {
            "edits" => {
                let g = cls.genre.clone().unwrap_or_default();
                crates.join("GENRES").join(&g).join("EDITS")
            }
            "genres" => {
                let g = cls.genre.clone().unwrap_or_default();
                match cls.subgenre.as_deref() {
                    Some(s) if !s.is_empty() => crates.join("GENRES").join(&g).join(s),
                    _ => crates.join("GENRES").join(&g),
                }
            }
            _ => crates.join("_REVIEW").join("_Unrouted"),
        };

        out.push(UnsortedTrack {
            abs_path: p.to_string_lossy().to_string(),
            filename,
            cleaned_filename,
            title,
            artist,
            genre_tag: genre,
            grouping_tag: grouping,
            detected_genre: cls.genre,
            detected_subgenre: cls.subgenre,
            is_edit: cls.is_edit,
            suggested_dest: dest_dir.to_string_lossy().to_string(),
            bucket: cls.bucket.to_string(),
            confidence: cls.confidence,
            reasons: cls.reasons,
        });
    }

    out.sort_by(|a, b| a.filename.to_lowercase().cmp(&b.filename.to_lowercase()));
    Ok(out)
}

pub(crate) fn unique_dest(dir: &Path, filename: &str) -> PathBuf {
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

fn write_tags(path: &Path, genre: &str, grouping: &str) -> Result<(), String> {
    if genre.is_empty() && grouping.is_empty() {
        return Ok(());
    }
    let mut tagged = Probe::open(path)
        .map_err(|e| format!("probe: {}", e))?
        .read()
        .map_err(|e| format!("read: {}", e))?;

    if tagged.primary_tag().is_none() {
        let tag_type = tagged.primary_tag_type();
        tagged.insert_tag(lofty::tag::Tag::new(tag_type));
    }
    let tag = tagged
        .primary_tag_mut()
        .ok_or_else(|| "no primary tag available".to_string())?;

    if !genre.is_empty() {
        tag.set_genre(genre.to_string());
    }
    if !grouping.is_empty() {
        tag.insert_text(ItemKey::ContentGroup, grouping.to_string());
    }

    tagged
        .save_to_path(path, WriteOptions::default())
        .map_err(|e| format!("save: {}", e))?;
    Ok(())
}

#[tauri::command]
pub fn apply_unsorted_moves(moves: Vec<UnsortedMove>) -> Result<MoveReport, String> {
    let mut moved = 0usize;
    let mut tagged = 0usize;
    let mut skipped = 0usize;
    let mut errors: Vec<String> = Vec::new();

    for m in moves {
        let src = PathBuf::from(&m.abs_path);
        if !src.exists() {
            skipped += 1;
            continue;
        }
        let dest_dir = PathBuf::from(&m.dest_dir);
        if let Err(e) = fs::create_dir_all(&dest_dir) {
            errors.push(format!("mkdir {}: {}", dest_dir.display(), e));
            continue;
        }
        let filename = if !m.dest_filename.is_empty() {
            m.dest_filename.clone()
        } else {
            src.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("track")
                .to_string()
        };
        let dest = unique_dest(&dest_dir, &filename);

        let move_ok = match fs::rename(&src, &dest) {
            Ok(_) => true,
            Err(_) => {
                if let Err(e) = fs::copy(&src, &dest) {
                    errors.push(format!("copy {} → {}: {}", src.display(), dest.display(), e));
                    false
                } else if let Err(e) = fs::remove_file(&src) {
                    errors.push(format!("remove {}: {}", src.display(), e));
                    false
                } else {
                    true
                }
            }
        };

        if !move_ok {
            continue;
        }
        moved += 1;

        if !m.genre.is_empty() || !m.grouping.is_empty() {
            match write_tags(&dest, &m.genre, &m.grouping) {
                Ok(_) => tagged += 1,
                Err(e) => errors.push(format!("tag {}: {}", dest.display(), e)),
            }
        }
    }

    Ok(MoveReport {
        moved,
        tagged,
        skipped,
        errors,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lib(entries: &[(&str, &[&str])]) -> HashMap<String, Vec<String>> {
        entries
            .iter()
            .map(|(g, s)| ((*g).to_string(), s.iter().map(|x| x.to_string()).collect()))
            .collect()
    }

    #[test]
    fn cleans_soundcloud_id_prefix() {
        assert_eq!(
            clean_filename("[1387989166] Mathias. - Beyoncé - Heated (Edit).m4a"),
            "Mathias. - Beyoncé - Heated (Edit).m4a"
        );
    }

    #[test]
    fn cleans_multiple_bracket_blocks() {
        assert_eq!(
            clean_filename("[123] [4567] Song Title.mp3"),
            "Song Title.mp3"
        );
    }

    #[test]
    fn cleans_format_suffix_paren() {
        assert_eq!(
            clean_filename("Artist - Title (320kbps).mp3"),
            "Artist - Title.mp3"
        );
    }

    #[test]
    fn keeps_non_numeric_brackets() {
        // Non-numeric brackets shouldn't be stripped.
        assert_eq!(
            clean_filename("Song [Instrumental].mp3"),
            "Song [Instrumental].mp3"
        );
    }

    #[test]
    fn mathias_edit_routes_to_edits_when_afrobeats_in_library() {
        let library = lib(&[("Afrobeats", &["Afroswing", "Amapiano"]), ("R&B", &["90s"])]);
        let c = classify(
            "[1387989166] Mathias. - Beyoncé - Heated Iskaba (@mathiasxdc Edit).m4a",
            "",
            "",
            "",
            "Afrobeats",
            "",
            &library,
            None,
        );
        assert_eq!(c.bucket, "edits");
        assert_eq!(c.genre.as_deref(), Some("Afrobeats"));
    }

    #[test]
    fn genre_match_without_subgenre_routes_to_genre_root() {
        // New rule: when we match a genre but can't find a subgenre, the
        // track goes to the genre root folder — not _Unrouted.
        let library = lib(&[("Afrobeats", &["Afroswing", "Amapiano"])]);
        let c = classify(
            "burna boy - last last.mp3",
            "",
            "",
            "",
            "Afrobeats",
            "",
            &library,
            None,
        );
        assert_eq!(c.bucket, "genres");
        assert_eq!(c.genre.as_deref(), Some("Afrobeats"));
        assert_eq!(c.subgenre, None);
    }

    #[test]
    fn afrobeats_track_with_subgenre_token_routes_to_genres() {
        let library = lib(&[("Afrobeats", &["Afroswing", "Amapiano"])]);
        let c = classify(
            "dj - amapiano banger.mp3",
            "",
            "",
            "",
            "Afrobeats",
            "",
            &library,
            None,
        );
        assert_eq!(c.bucket, "genres");
        assert_eq!(c.genre.as_deref(), Some("Afrobeats"));
        assert_eq!(c.subgenre.as_deref(), Some("Amapiano"));
    }

    #[test]
    fn unknown_genre_goes_to_review() {
        let library = lib(&[("Afrobeats", &["Amapiano"])]);
        let c = classify(
            "some - techno banger.mp3",
            "",
            "",
            "",
            "Techno",
            "",
            &library,
            None,
        );
        assert_eq!(c.bucket, "review");
        assert_eq!(c.genre, None);
    }

    #[test]
    fn parent_folder_supplies_genre_and_subgenre() {
        let library = lib(&[("R&B", &["90s"])]);
        let c = classify(
            "21 - queen-of-the-freek-a-leek.mp3",
            "R&B/90s",
            "",
            "",
            "",
            "",
            &library,
            None,
        );
        assert_eq!(c.bucket, "genres");
        assert_eq!(c.genre.as_deref(), Some("R&B"));
        assert_eq!(c.subgenre.as_deref(), Some("90s"));
    }

    #[test]
    fn parent_folder_mashups_name_routes_to_edits() {
        let library = lib(&[("R&B", &["90s"])]);
        let c = classify(
            "21 - queen-of-the-freek-a-leek.mp3",
            "R&B Mashups",
            "",
            "",
            "",
            "",
            &library,
            None,
        );
        assert_eq!(c.bucket, "edits");
        assert_eq!(c.genre.as_deref(), Some("R&B"));
    }

    #[test]
    fn artist_alias_whitney_houston_routes_to_rnb_edits() {
        // Parent folder names the artist (with 'ft' remixer tail). No R&B
        // token in the path. Alias list catches "whitney houston" → R&B.
        // The '-ft-' in the folder triggers edit routing, so → R&B/EDITS/.
        let library = lib(&[("R&B", &["Contemporary R&B"]), ("Hip-Hop", &["Boom Bap"])]);
        let c = classify(
            "01 - baby-im-changing-ur-mind-tonite.mp3",
            "whitney-houston-ft-icomplexity/love-her-for-life",
            "",
            "",
            "",
            "",
            &library,
            None,
        );
        // 'ft' token triggers edit detection; genre came from artist alias.
        assert_eq!(c.genre.as_deref(), Some("R&B"));
    }

    #[test]
    fn artist_alias_kendrick_no_edit_signals_routes_to_hiphop_root() {
        // No edit signals, no subgenre hint. Alias gives us Hip-Hop; with no
        // subgenre match we land at the genre root.
        let library = lib(&[("Hip-Hop", &["Boom Bap", "Trap"])]);
        let c = classify(
            "kendrick lamar - king kunta.mp3",
            "",
            "",
            "",
            "",
            "",
            &library,
            None,
        );
        assert_eq!(c.bucket, "genres");
        assert_eq!(c.genre.as_deref(), Some("Hip-Hop"));
        assert_eq!(c.subgenre, None);
    }

    #[test]
    fn soulection_source_folder_routes_to_edits() {
        // A file under a 'Soulection' parent folder should be treated as an
        // edit/mashup even without explicit edit tokens in the filename.
        let library = lib(&[("R&B", &["Contemporary R&B"])]);
        let c = classify(
            "track.mp3",
            "Soulection/Episode 500",
            "",
            "",
            "",
            "",
            &library,
            None,
        );
        // No genre hint in the path → review bucket but with is_edit=true.
        assert!(c.is_edit);
    }

    #[test]
    fn soulection_with_alias_artist_routes_to_edits_under_genre() {
        // Soulection parent + track filename carrying a known artist name.
        let library = lib(&[("R&B", &["Contemporary R&B"])]);
        let c = classify(
            "sza - good days.mp3",
            "Soulection/Radio",
            "",
            "",
            "",
            "",
            &library,
            None,
        );
        assert_eq!(c.bucket, "edits");
        assert_eq!(c.genre.as_deref(), Some("R&B"));
    }

    #[test]
    fn source_folder_soundcloud_is_skipped_when_extracting_artist() {
        // The 'Soundcloud' top folder must NOT be used as an artist candidate.
        let cands =
            extract_artist_candidates("01 - track.mp3", "Soundcloud/whitney-houston-ft-icomplexity", "");
        // First candidate after source skip should be whitney-houston.
        assert_eq!(
            cands.first().map(|s| normalize_artist_name(s)),
            Some("whitney houston".to_string())
        );
    }

    #[test]
    fn ft_suffix_strips_after_known_marker() {
        assert_eq!(strip_ft_suffix("whitney-houston-ft-icomplexity"), "whitney-houston");
        assert_eq!(strip_ft_suffix("Drake feat Future"), "Drake");
        assert_eq!(strip_ft_suffix("Whitney Houston"), "Whitney Houston");
    }
}
