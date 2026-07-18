#!/usr/bin/env python3
"""
analyze_energy.py — Analyze energy/feel of every track in the library.

Three sources:
  1. Librosa audio analysis (local, from the actual waveform)
  2. Spotify audio features (API, matched by artist + title)
  3. Write results to ID3 comment tag (shows up in Serato's comment column)

The comment tag format:
  "Energy:7 | Feel:High | Dance:0.82 | Bright:Med"

Usage:
  python3 analyze_energy.py                    # analyze all untagged tracks
  python3 analyze_energy.py --force            # re-analyze all tracks
  python3 analyze_energy.py --spotify          # include Spotify features
  python3 analyze_energy.py --dry-run          # preview without writing tags
  python3 analyze_energy.py --limit 100        # only process first N tracks
"""

import json
import os
import re
import sys
import time
import warnings
from functools import partial
from pathlib import Path

# Force unbuffered output
print = partial(print, flush=True)

import librosa
import mutagen
import numpy as np
from mutagen.id3 import COMM, ID3, TBPM

warnings.filterwarnings("ignore")

# ── Config ──────────────────────────────────────────────────────────────────

GENRES_ROOT = Path("/Users/koryjcampbell/Music/CRATES/GENRES")
AUDIO_EXTENSIONS = {".mp3", ".flac", ".wav", ".ogg", ".aif", ".aiff", ".aac", ".m4a"}

# BPM normalization
BPM_MIN = 68.0
BPM_MAX = 135.0
LOW_BPM_SUBGENRES = {
    "trap", "general trap", "trap soul", "southern", "crunk",
    "cloud rap", "chopped & screwed", "chopped and screwed",
}

# Normalization ceilings (calibrated from library samples)
RMS_CEIL = 0.40        # RMS energy ceiling
CENTROID_CEIL = 5000.0  # Spectral centroid ceiling
ONSET_CEIL = 4.0        # Onset strength ceiling
CONTRAST_CEIL = 30.0    # Spectral contrast ceiling

# Analysis duration (seconds from middle of track)
ANALYSIS_DURATION = 30
ANALYSIS_OFFSET_RATIO = 0.3  # Start at 30% into the track

# Energy tag marker so we know we wrote it
ENERGY_TAG_PREFIX = "Energy:"


# ── BPM Normalization ──────────────────────────────────────────────────────

def normalize_bpm(bpm: float, subgenre: str) -> float:
    """Normalize BPM to 68-135 range with subgenre awareness."""
    if bpm <= 0:
        return 0.0

    normalized = bpm
    while normalized > BPM_MAX:
        normalized /= 2.0
    while normalized < BPM_MIN:
        normalized *= 2.0

    if subgenre.lower() in LOW_BPM_SUBGENRES and normalized > 90.0:
        halved = normalized / 2.0
        if halved >= BPM_MIN:
            normalized = halved

    return round(normalized, 2)


# ── Audio Analysis ──────────────────────────────────────────────────────────

def analyze_track(filepath: str) -> dict:
    """
    Analyze a track using librosa and return energy features.

    Returns dict with:
      energy (0-1), brightness (0-1), rhythm (0-1), punch (0-1),
      score (1-10), feel (Chill/Medium/High/Intense), tempo
    """
    try:
        # Get duration to calculate offset
        duration = librosa.get_duration(path=filepath)
        offset = max(0, duration * ANALYSIS_OFFSET_RATIO)
        # Don't exceed track length
        if offset + ANALYSIS_DURATION > duration:
            offset = max(0, duration - ANALYSIS_DURATION)

        y, sr = librosa.load(
            filepath,
            duration=ANALYSIS_DURATION,
            offset=offset,
            sr=22050,
        )

        if len(y) < sr:  # Less than 1 second of audio
            return None

        # RMS Energy — overall loudness/power
        rms = librosa.feature.rms(y=y)[0]
        energy = min(float(np.mean(rms)) / RMS_CEIL, 1.0)

        # Spectral Centroid — brightness/high frequency content
        cent = librosa.feature.spectral_centroid(y=y, sr=sr)[0]
        brightness = min(float(np.mean(cent)) / CENTROID_CEIL, 1.0)

        # Onset Strength — rhythmic intensity/drive
        onset_env = librosa.onset.onset_strength(y=y, sr=sr)
        rhythm = min(float(np.mean(onset_env)) / ONSET_CEIL, 1.0)

        # Spectral Contrast — dynamic range/punch
        contrast = librosa.feature.spectral_contrast(y=y, sr=sr)
        punch = min(float(np.mean(contrast)) / CONTRAST_CEIL, 1.0)

        # Danceability proxy — regularity of beat
        _, beats = librosa.beat.beat_track(y=y, sr=sr)
        if len(beats) > 2:
            beat_intervals = np.diff(beats)
            beat_regularity = 1.0 - min(float(np.std(beat_intervals)) / float(np.mean(beat_intervals) + 1e-6), 1.0)
        else:
            beat_regularity = 0.0

        # Tempo detection
        tempo = float(librosa.beat.tempo(y=y, sr=sr)[0])

        # Composite energy score (1-10)
        composite = (
            energy * 0.30
            + brightness * 0.15
            + rhythm * 0.30
            + punch * 0.10
            + beat_regularity * 0.15
        ) * 10
        score = max(1, min(10, round(composite)))

        # Feel classification
        if score >= 8:
            feel = "Intense"
        elif score >= 6:
            feel = "High"
        elif score >= 4:
            feel = "Medium"
        else:
            feel = "Chill"

        # Brightness label
        if brightness > 0.6:
            bright_label = "High"
        elif brightness > 0.3:
            bright_label = "Med"
        else:
            bright_label = "Low"

        return {
            "energy": round(energy, 2),
            "brightness": round(brightness, 2),
            "rhythm": round(rhythm, 2),
            "punch": round(punch, 2),
            "danceability": round(beat_regularity, 2),
            "tempo": round(tempo, 1),
            "score": score,
            "feel": feel,
            "bright_label": bright_label,
        }

    except Exception as e:
        return None


# ── Spotify Features ────────────────────────────────────────────────────────

def get_spotify_features(tracks_to_lookup: list[dict]) -> dict:
    """
    Look up Spotify audio features for tracks.

    Requires SPOTIPY_CLIENT_ID and SPOTIPY_CLIENT_SECRET env vars.
    Returns dict of filepath -> spotify features.
    """
    try:
        import spotipy
        from spotipy.oauth2 import SpotifyClientCredentials
    except ImportError:
        print("  spotipy not installed, skipping Spotify")
        return {}

    client_id = os.environ.get("SPOTIPY_CLIENT_ID")
    client_secret = os.environ.get("SPOTIPY_CLIENT_SECRET")

    if not client_id or not client_secret:
        print("  SPOTIPY_CLIENT_ID / SPOTIPY_CLIENT_SECRET not set, skipping Spotify")
        return {}

    try:
        sp = spotipy.Spotify(
            auth_manager=SpotifyClientCredentials(
                client_id=client_id,
                client_secret=client_secret,
            )
        )
    except Exception as e:
        print(f"  Spotify auth failed: {e}")
        return {}

    results = {}
    total = len(tracks_to_lookup)

    for i, track in enumerate(tracks_to_lookup):
        if i % 50 == 0 and i > 0:
            print(f"    Spotify: {i}/{total} looked up...")
            time.sleep(1)  # Rate limiting

        artist = track.get("artist", "")
        title = track.get("title", "")
        filepath = track["filepath"]

        if not artist or not title:
            continue

        # Clean up for search
        title_clean = re.sub(r"\(feat\..*?\)", "", title).strip()
        title_clean = re.sub(r"\[.*?\]", "", title_clean).strip()
        query = f"artist:{artist} track:{title_clean}"

        try:
            search = sp.search(q=query, type="track", limit=1)
            if search["tracks"]["items"]:
                track_id = search["tracks"]["items"][0]["id"]
                features = sp.audio_features([track_id])
                if features and features[0]:
                    f = features[0]
                    results[filepath] = {
                        "sp_energy": round(f.get("energy", 0), 2),
                        "sp_danceability": round(f.get("danceability", 0), 2),
                        "sp_valence": round(f.get("valence", 0), 2),
                        "sp_tempo": round(f.get("tempo", 0), 1),
                        "sp_loudness": round(f.get("loudness", 0), 1),
                    }
        except Exception:
            continue

        # Small delay to avoid rate limits
        if i % 10 == 0:
            time.sleep(0.1)

    print(f"    Spotify: matched {len(results)}/{total} tracks")
    return results


# ── ID3 Tag Writing ─────────────────────────────────────────────────────────

def write_energy_to_id3(filepath: str, analysis: dict, spotify: dict = None, dry_run: bool = False) -> bool:
    """
    Write energy analysis to ID3 comment tag and fix BPM.

    Comment format: "Energy:7 | Feel:High | Dance:0.82 | Bright:Med"
    """
    try:
        audio = mutagen.File(filepath)
        if audio is None:
            return False

        # Build comment string
        parts = [
            f"Energy:{analysis['score']}",
            f"Feel:{analysis['feel']}",
            f"Dance:{analysis['danceability']:.2f}",
            f"Bright:{analysis['bright_label']}",
        ]

        if spotify:
            parts.append(f"SpEnergy:{spotify.get('sp_energy', 'N/A')}")
            parts.append(f"SpDance:{spotify.get('sp_danceability', 'N/A')}")
            parts.append(f"SpVibe:{spotify.get('sp_valence', 'N/A')}")

        comment = " | ".join(parts)

        if dry_run:
            return True

        # Handle ID3 tags
        if hasattr(audio, "tags") and audio.tags is not None:
            # Remove existing energy comments
            keys_to_remove = []
            for key in audio.tags:
                if key.startswith("COMM") and isinstance(audio.tags[key], COMM):
                    if audio.tags[key].text and audio.tags[key].text[0].startswith(ENERGY_TAG_PREFIX):
                        keys_to_remove.append(key)
            for key in keys_to_remove:
                del audio.tags[key]

            # Add new energy comment
            audio.tags.add(
                COMM(encoding=3, lang="eng", desc="energy", text=[comment])
            )
        elif isinstance(audio, mutagen.mp3.MP3):
            audio.add_tags()
            audio.tags.add(
                COMM(encoding=3, lang="eng", desc="energy", text=[comment])
            )

        audio.save()
        return True

    except Exception:
        return False


def fix_bpm_in_id3(filepath: str, subgenre: str, dry_run: bool = False) -> tuple[bool, float, float]:
    """
    Read BPM from ID3, normalize it, write back if changed.

    Returns (changed, original_bpm, new_bpm)
    """
    try:
        audio = mutagen.File(filepath)
        if audio is None or audio.tags is None:
            return False, 0, 0

        # Read current BPM
        bpm_tag = audio.tags.get("TBPM")
        if bpm_tag is None:
            return False, 0, 0

        try:
            original = float(str(bpm_tag))
        except (ValueError, TypeError):
            return False, 0, 0

        if original <= 0:
            return False, 0, 0

        normalized = normalize_bpm(original, subgenre)

        if abs(original - normalized) < 0.01:
            return False, original, normalized

        if not dry_run:
            audio.tags.delall("TBPM")
            audio.tags.add(TBPM(encoding=3, text=[f"{normalized:.2f}"]))
            audio.save()

        return True, original, normalized

    except Exception:
        return False, 0, 0


# ── Main ────────────────────────────────────────────────────────────────────

def main():
    force = "--force" in sys.argv
    use_spotify = "--spotify" in sys.argv
    dry_run = "--dry-run" in sys.argv
    limit = 0
    for i, arg in enumerate(sys.argv):
        if arg == "--limit" and i + 1 < len(sys.argv):
            limit = int(sys.argv[i + 1])

    print("=" * 60)
    print("  DJ Crates — Energy Analysis")
    print("=" * 60)
    if dry_run:
        print("  Mode: DRY RUN")
    if use_spotify:
        print("  Spotify: enabled")
    print()

    # Collect all audio files
    print("Scanning library...")
    tracks = []
    for dirpath, dirnames, filenames in os.walk(GENRES_ROOT):
        dirnames[:] = [d for d in dirnames if not d.startswith(".")]
        for fname in filenames:
            filepath = os.path.join(dirpath, fname)
            ext = os.path.splitext(fname)[1].lower()
            if ext not in AUDIO_EXTENSIONS:
                continue

            # Extract genre/subgenre from path
            rel = os.path.relpath(filepath, GENRES_ROOT)
            parts = rel.split(os.sep)
            genre = parts[0] if len(parts) > 0 else ""
            subgenre = parts[1] if len(parts) > 1 else ""

            # Extract artist/title from ID3 for Spotify lookup
            artist, title = "", ""
            try:
                audio = mutagen.File(filepath)
                if audio and audio.tags:
                    artist = str(audio.tags.get("TPE1", ""))
                    title = str(audio.tags.get("TIT2", ""))
            except Exception:
                pass

            # Check if already analyzed (unless --force)
            already_tagged = False
            if not force:
                try:
                    audio = mutagen.File(filepath)
                    if audio and audio.tags:
                        for key in audio.tags:
                            if key.startswith("COMM") and isinstance(audio.tags[key], COMM):
                                if audio.tags[key].desc == "energy":
                                    already_tagged = True
                                    break
                except Exception:
                    pass

            if already_tagged:
                continue

            tracks.append({
                "filepath": filepath,
                "genre": genre,
                "subgenre": subgenre,
                "artist": artist,
                "title": title,
            })

    if limit > 0:
        tracks = tracks[:limit]

    print(f"  {len(tracks)} tracks to analyze\n")

    if not tracks:
        print("Nothing to do.")
        return

    # Step 1: Spotify lookup (optional)
    spotify_data = {}
    if use_spotify:
        print("Fetching Spotify features...")
        spotify_data = get_spotify_features(tracks)
        print()

    # Step 2: Librosa analysis + ID3 writing
    print("Analyzing audio & writing tags...")
    analyzed = 0
    bpm_fixed = 0
    errors = 0
    start = time.time()

    for i, track in enumerate(tracks):
        filepath = track["filepath"]
        subgenre = track["subgenre"]

        # Progress
        if i % 25 == 0:
            elapsed = time.time() - start
            rate = i / elapsed if elapsed > 0 else 0
            eta = (len(tracks) - i) / rate if rate > 0 else 0
            print(f"  [{i + 1}/{len(tracks)}] {rate:.1f} tracks/sec, ETA: {eta / 60:.0f}min")

        # Analyze with librosa
        result = analyze_track(filepath)
        if result is None:
            errors += 1
            continue

        # Get Spotify data if available
        sp = spotify_data.get(filepath)

        # Write to ID3
        if write_energy_to_id3(filepath, result, sp, dry_run=dry_run):
            analyzed += 1

        # Fix BPM
        changed, orig, new = fix_bpm_in_id3(filepath, subgenre, dry_run=dry_run)
        if changed:
            bpm_fixed += 1

    elapsed = time.time() - start

    print(f"\n{'=' * 60}")
    print(f"  Analyzed:    {analyzed}")
    print(f"  BPMs fixed:  {bpm_fixed}")
    print(f"  Errors:      {errors}")
    print(f"  Time:        {elapsed / 60:.1f} minutes")
    print(f"{'=' * 60}")


if __name__ == "__main__":
    main()
