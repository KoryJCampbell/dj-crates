#!/usr/bin/env python3
"""
detect_bpms.py — Detect accurate BPMs from audio files using librosa,
with GENRE-AWARE halftime/doubletime disambiguation.

Approach:
  1. Load a 60-second slice from the middle of the track
  2. HPSS (Harmonic-Percussive Source Separation) — isolate the percussive
     signal so hi-hat subdivisions don't dominate the autocorrelation
  3. Run librosa.feature.tempo with a `start_bpm` hint equal to the
     genre's typical midpoint — this biases detection to the correct
     halftime/doubletime when a trap track could read as 70 or 140
  4. If detection falls outside the genre range, halve or double until
     it lands (or quarter for extreme cases)
  5. Write back to ID3 TBPM / MP4 tmpo — does NOT touch Comment

Genre is inferred from the path: CRATES/GENRES/<Genre>/...

Output is line-oriented for streaming to the Rust side:
  PROGRESS <idx> <total> <path>
  OK <path> <old_bpm> <new_bpm>
  SKIP <path> <reason>
  ERR <path> <reason>
  DONE <scanned> <written> <skipped> <errors>

Usage:
  detect_bpms.py --crates-root /Users/.../CRATES [--limit N] [--force]
"""

import argparse
import os
import sys
import warnings
from functools import partial
from pathlib import Path

# Force unbuffered output — each line flushes immediately for Rust streaming
print = partial(print, flush=True)
warnings.filterwarnings("ignore")


AUDIO_EXTENSIONS = {".mp3", ".flac", ".wav", ".ogg", ".aif", ".aiff", ".aac", ".m4a"}

# Per-genre (min, max, hint) BPM values. Hint biases librosa's tempo
# estimator toward this value, resolving halftime/doubletime ambiguity.
# Trap → 85 so a 140 detection halves cleanly to 70.
GENRE_BPM = {
    "R&B":              (68, 110, 85),
    "Neo-Soul":         (68, 110, 85),
    "Soul":             (68, 115, 90),
    "Gospel":           (68, 120, 95),
    "Jazz":             (68, 120, 95),
    "Hip-Hop":          (68, 105, 85),
    "Hip-Hop:Rap":      (68, 105, 85),
    "Afrobeats":        (95, 125, 110),
    "Amapiano":         (105, 125, 115),
    "House":            (115, 132, 124),
    "Afro House":       (115, 132, 124),
    "Dancehall":        (68, 110, 95),
    "Reggae":           (68, 95, 80),
    "Funk":             (95, 125, 110),
    "Disco":            (100, 130, 118),
    "Jersey Club":      (128, 135, 132),
    "Baltimore Club":   (128, 135, 132),
    "Baile Funk":       (125, 135, 130),
    "Moombahton":       (105, 115, 110),
    "Gqom":             (115, 130, 122),
    "Kwaito":           (100, 115, 108),
    "UK Funky":         (120, 135, 128),
    "UK Garage":        (128, 135, 132),
    "Techno":           (120, 135, 128),
    "Electronic":       (115, 135, 125),
    "Lo-Fi":            (68, 100, 85),
    "Lounge":           (68, 110, 90),
    "Motown":           (95, 130, 115),
    "Latin":            (85, 130, 110),
    "Reggaeton":        (88, 100, 95),
    "Soca":             (125, 135, 130),
}

DEFAULT_RANGE = (68, 135, 100)


def genre_from_path(path: Path, crates_root: Path):
    try:
        rel = path.relative_to(crates_root)
    except ValueError:
        return None
    parts = rel.parts
    if len(parts) < 2 or parts[0] != "GENRES":
        return None
    return parts[1]


def read_existing_bpm(path: Path):
    try:
        from mutagen import File
        f = File(str(path))
        if f is None:
            return 0.0
        # MP3: TBPM; M4A: tmpo
        for key in ("TBPM", "tmpo"):
            v = f.get(key)
            if v is None:
                continue
            if hasattr(v, "text"):  # ID3 frame
                s = str(v.text[0]) if v.text else ""
            else:
                s = str(v[0]) if isinstance(v, list) and v else str(v)
            try:
                return float(s)
            except ValueError:
                continue
        return 0.0
    except Exception:
        return 0.0


def write_bpm(path: Path, bpm: float):
    """Write BPM to the correct frame for the file's format."""
    from mutagen import File
    from mutagen.id3 import TBPM, ID3
    f = File(str(path))
    if f is None:
        raise RuntimeError("could not parse file")

    ext = path.suffix.lower()
    if ext == ".mp3":
        # ID3 TBPM — stored as text
        try:
            tags = ID3(str(path))
        except Exception:
            # no existing ID3 header; create one
            tags = ID3()
        tags.setall("TBPM", [TBPM(encoding=3, text=[f"{bpm:.2f}"])])
        tags.save(str(path))
    elif ext in (".m4a", ".aac", ".alac"):
        # MP4 tmpo — integer atom
        f["tmpo"] = [int(round(bpm))]
        f.save()
    else:
        # Flac/etc. — use mutagen generic
        f["BPM"] = f"{bpm:.2f}"
        f.save()


def normalize_to_range(bpm: float, lo: int, hi: int):
    """Try halve/double/quarter until in range. Returns best-fit BPM."""
    if bpm <= 0:
        return bpm
    if lo - 1 <= bpm <= hi + 1:
        return bpm
    for factor in (0.5, 2.0, 0.25, 4.0):
        candidate = bpm * factor
        if lo - 1 <= candidate <= hi + 1:
            return candidate
    return bpm


def detect_bpm_from_audio(path: Path, start_bpm: float):
    """
    Core detection: HPSS to isolate percussion, then tempo via
    onset autocorrelation biased by start_bpm.
    """
    import librosa
    import numpy as np

    # Load middle 60s at a reduced sample rate (plenty for tempo)
    duration_seconds = 60.0
    try:
        total = librosa.get_duration(path=str(path))
    except Exception:
        total = 240.0  # assume 4 min if we can't read
    offset = max(0.0, min(total * 0.3, max(0.0, total - duration_seconds)))
    y, sr = librosa.load(str(path), sr=22050, offset=offset, duration=duration_seconds, mono=True)
    if y.size == 0:
        return 0.0

    # Split into harmonic + percussive — focus on drums for beat tracking
    y_harm, y_perc = librosa.effects.hpss(y)

    # Tempo with genre prior — start_bpm biases the autocorrelation peak pick
    tempo = float(
        librosa.feature.tempo(y=y_perc, sr=sr, start_bpm=float(start_bpm))[0]
    )
    return tempo


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--crates-root", required=True)
    ap.add_argument("--limit", type=int, default=0, help="Process at most N files (0 = all)")
    ap.add_argument(
        "--force",
        action="store_true",
        help="Re-detect even files whose existing BPM is already in genre range",
    )
    args = ap.parse_args()

    crates_root = Path(args.crates_root)
    genres_dir = crates_root / "GENRES"
    if not genres_dir.is_dir():
        print(f"ERR - GENRES folder missing at {genres_dir}", file=sys.stderr)
        sys.exit(1)

    # Collect files
    files = []
    for dirpath, _, fnames in os.walk(genres_dir):
        for name in fnames:
            if Path(name).suffix.lower() in AUDIO_EXTENSIONS:
                files.append(Path(dirpath) / name)

    total = len(files)
    if args.limit > 0:
        files = files[: args.limit]
        total = len(files)

    scanned = 0
    written = 0
    skipped = 0
    errors = 0

    for idx, path in enumerate(files, 1):
        scanned += 1
        genre = genre_from_path(path, crates_root) or ""
        (lo, hi, hint) = GENRE_BPM.get(genre, DEFAULT_RANGE)

        existing = read_existing_bpm(path)
        if not args.force and existing > 0 and lo - 1 <= existing <= hi + 1:
            skipped += 1
            print(f"SKIP\t{path}\tin-range {existing:.2f}")
            if idx % 10 == 0 or idx == total:
                print(f"PROGRESS\t{idx}\t{total}\t{path.name}")
            continue

        try:
            detected = detect_bpm_from_audio(path, hint)
            if detected <= 0:
                errors += 1
                print(f"ERR\t{path}\tdetection returned 0")
                continue
            normalized = normalize_to_range(detected, lo, hi)
            write_bpm(path, normalized)
            written += 1
            print(f"OK\t{path}\t{existing:.2f}\t{normalized:.2f}")
        except Exception as e:
            errors += 1
            print(f"ERR\t{path}\t{type(e).__name__}: {e}")

        if idx % 10 == 0 or idx == total:
            print(f"PROGRESS\t{idx}\t{total}\t{path.name}")

    print(f"DONE\t{scanned}\t{written}\t{skipped}\t{errors}")


if __name__ == "__main__":
    main()
