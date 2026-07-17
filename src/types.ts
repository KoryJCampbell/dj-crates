export type Config = {
  cratesRoot: string;
  seratoPath: string;
  unsortedPath: string;
  pythonVenv: string;
  scriptsDir: string;
  spotifyClientId: string;
  spotifyClientSecret: string;
};

export type UnsortedTrack = {
  abs_path: string;
  filename: string;
  cleaned_filename: string;
  title: string;
  artist: string;
  genre_tag: string;
  grouping_tag: string;
  detected_genre: string | null;
  detected_subgenre: string | null;
  is_edit: boolean;
  suggested_dest: string;
  bucket: "genres" | "edits" | "review";
  confidence: number;
  reasons: string[];
};

export type UnsortedMove = {
  abs_path: string;
  dest_dir: string;
  dest_filename: string;
  genre: string;
  grouping: string;
};

export type MoveReport = {
  moved: number;
  tagged: number;
  skipped: number;
  errors: string[];
};

export type TopLevelCrate = {
  name: string;
  track_count: number;
  subcrate_count: number;
};

export type SyncResult = {
  preview: boolean;
  crates_to_create: string[];
  crates_to_delete: string[];
  crates_written: number;
  total_track_entries: number;
  bpm_adjusted: number;
  database_tracks_new: number;
  energy_analyzed: number;
  top_level: TopLevelCrate[];
  unsorted: UnsortedTrack[];
  new_this_week: number;
  new_this_month: number;
};

export type ProgressPayload = {
  stage: string;
  current: number;
  total: number;
  message: string;
};

export type SmartCrateRules = {
  year_min?: number | null;
  year_max?: number | null;
  bpm_min?: number | null;
  bpm_max?: number | null;
  genres: string[];
  subgenres: string[];
  artists: string[];
  title_contains?: string | null;
  popularity_min?: number | null;
  popularity_max?: number | null;
  limit?: number | null;
};

export type EnrichResult = {
  total: number;
  enriched: number;
  cached: number;
  not_found: number;
  errors: number;
};

export type BpmFixReport = {
  scanned: number;
  halved: number;
  doubled: number;
  quartered: number;
  unchanged: number;
  errors: string[];
  serato_db_updated: number;
};

export type BpmDetectReport = {
  scanned: number;
  written: number;
  skipped: number;
  errors: string[];
};

export type CleanupReport = {
  hip_hop_merged: number;
  orphans_classified: number;
  orphans_left: number;
  loose_to_general: number;
  serato_db_updated: number;
  manifest_path: string;
  errors: string[];
};

export type PruneReport = {
  rescued: number;
  merged: number;
  demoted: number;
  scanned_kept: number;
  scanned_archived: number;
  archived_whole: number;
  serato_db_updated: number;
  manifest_path: string;
  errors: string[];
};

export type ConvertReport = {
  total_m4a: number;
  converted: number;
  skipped_existing_mp3: number;
  failed: number;
  serato_db_updated: number;
  archive_root: string;
  errors: string[];
};

export type NormalizeReport = {
  demoted: number;
  orphans_classified: number;
  orphans_archived: number;
  serato_db_updated: number;
  archive_root: string;
  errors: string[];
};

export type ClearTagsReport = {
  scanned: number;
  cleared: number;
  serato_db_updated: number;
  errors: string[];
};

export type SanitizeReport = {
  scanned: number;
  updated: number;
  comments_written: number;
  serato_db_updated: number;
  errors: string[];
};

export type AutoCueReport = {
  scanned: number;
  written: number;
  skipped: number;
  errors: string[];
};

export type MoodReport = {
  scanned: number;
  labeled: number;
  blank: number;
  errors: string[];
  by_mood: [string, number][];
  serato_db_updated: number;
};

export type SmartCratePreset = {
  id: string;
  name: string;
  description: string;
  rules: SmartCrateRules;
  builtin?: boolean;
};

export type SmartCrateResult = {
  preview: boolean;
  crate_name: string;
  matched_tracks: number;
  sample_tracks: string[];
};

export type DupesResult = {
  scanned: number;
  groups: number;
  extra_copies: number;
  wasted_bytes: number;
  report_path: string;
  crate_name: string | null;
  sample: string[];
  used_cache: boolean;
};

