#!/usr/bin/env python3
"""
autocue.py — beat-grid auto hot cues for Serato DJ Pro.

For every track in CRATES/GENRES, detect the first downbeat via librosa,
then place hot cues at N-bar intervals starting from that beat. Cues are
written as Serato's `Serato Markers2` GEOB frame (MP3) or
`----:com.serato.dj:markers_v2` atom (M4A) using the reverse-engineered
binary format, so they show up in Serato natively.

Usage:
  autocue.py --crates-root /path/to/CRATES [--bars 16] [--cues 8] [--limit N] [--force] [--dry-run]

Output (one line per file, space-separated, for the Rust wrapper to parse):
  OK   <path>  <bpm>  <first_beat_s>
  SKIP <path>  <reason>
  ERR  <path>  <reason>
  DONE <total> <ok> <skip> <err>
"""

import argparse
import base64
import os
import re
import struct
import sys
import warnings
from functools import partial
from pathlib import Path

print = partial(print, flush=True)
warnings.filterwarnings("ignore")

AUDIO_EXTS = {".mp3", ".m4a", ".flac", ".wav", ".aiff", ".aif"}

# Genre → (min_bpm, max_bpm, hint_bpm). Walks the track's path segments
# most-specific-first so 'Jersey Club' matches before 'House'.
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
    "Deep House":       (118, 128, 123),
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
    "Motown":           (95, 130, 115),
    "Reggaeton":        (88, 100, 95),
    "Soca":             (125, 135, 130),
}
DEFAULT_BPM_RANGE = (68, 135, 110)


def genre_bpm_for_path(filepath, crates_root):
    """Walk the path segments (deepest first) and return the hint BPM for
    the most specific matching genre folder. Falls back to a wide default."""
    try:
        rel = Path(filepath).resolve().relative_to(Path(crates_root).resolve())
    except ValueError:
        return DEFAULT_BPM_RANGE
    parts = list(rel.parts)
    # Drop "GENRES" prefix if present
    if parts and parts[0] == "GENRES":
        parts = parts[1:]
    # Drop filename
    parts = parts[:-1]
    # Try deepest first so subgenre wins over parent
    for seg in reversed(parts):
        if seg in GENRE_BPM:
            return GENRE_BPM[seg]
        # Loose match: allow "Jersey Club [Soundcloud]" → "Jersey Club"
        # Sort keys longest-first so "Hip-Hop:Rap" beats "Hip-Hop"
        for key in sorted(GENRE_BPM.keys(), key=len, reverse=True):
            if seg.startswith(key) or seg.startswith(f"{key} ") or seg.startswith(f"{key}["):
                return GENRE_BPM[key]
    return DEFAULT_BPM_RANGE


def normalize_bpm_to_range(bpm, lo, hi, hint=None):
    """Halve or double the BPM until it fits the genre's expected range.
    If nothing lands in range and a hint was given, snap to the hint — this
    catches cases like Jersey Club where librosa's syncopated detection comes
    back at 90 (neither half nor double of 132 fits 128-135)."""
    if bpm <= 0:
        return hint if hint is not None else bpm
    if lo - 1 <= bpm <= hi + 1:
        return bpm
    for factor in (0.5, 2.0, 0.25, 4.0):
        c = bpm * factor
        if lo - 1 <= c <= hi + 1:
            return c
    if hint is not None:
        return hint
    return bpm

# Serato's default hot-cue colors (index 0-7)
DEFAULT_COLORS = [
    (0xCC, 0x00, 0x00),  # red
    (0xCC, 0x88, 0x00),  # orange
    (0xCC, 0xCC, 0x00),  # yellow
    (0x00, 0xCC, 0x00),  # green
    (0x00, 0xCC, 0xCC),  # cyan
    (0x00, 0x00, 0xCC),  # blue
    (0xCC, 0x00, 0xCC),  # magenta
    (0xCC, 0xCC, 0xCC),  # white
]


# ── Serato Markers2 binary format ─────────────────────────────────────────
# Reverse-engineered reference: https://github.com/Holzhaus/serato-tags

def _encode_cue_entry(index, position_ms, color=(0xCC, 0x00, 0x00), name=""):
    body = b"\x00"
    body += bytes([index])
    body += struct.pack(">I", int(position_ms))
    body += b"\x00"
    body += bytes(color)
    body += b"\x00\x00"
    body += name.encode("utf-8") + b"\x00"
    entry = b"CUE\x00"
    entry += struct.pack(">I", len(body))
    entry += body
    return entry


def _encode_markers2_payload(cues):
    out = b"\x01\x01"  # version header
    for c in cues:
        out += _encode_cue_entry(**c)
    return out


def _encode_markers2_geob(cues):
    payload = _encode_markers2_payload(cues)
    b64 = base64.b64encode(payload).decode("ascii")
    wrapped = "\n".join(b64[i : i + 72] for i in range(0, len(b64), 72))
    return b"\x01\x01" + wrapped.encode("ascii") + b"\x00"


# ── Tag I/O ───────────────────────────────────────────────────────────────

def get_existing_bpm(filepath):
    from mutagen import File
    try:
        f = File(filepath)
        if not f:
            return 0.0
        v = f.get("TBPM")
        if v:
            s = str(v.text[0]) if hasattr(v, "text") else str(v[0])
            try:
                return float(s)
            except ValueError:
                pass
        v = f.get("tmpo")
        if v:
            try:
                return float(v[0])
            except (TypeError, ValueError):
                pass
    except Exception:
        pass
    return 0.0


def has_existing_cues(filepath):
    """True if the file already has Serato Markers2 data."""
    from mutagen import File
    ext = Path(filepath).suffix.lower()
    try:
        f = File(filepath)
        if not f:
            return False
        if ext == ".mp3":
            for key in f.tags.keys():
                if key.startswith("GEOB:Serato Markers2"):
                    return True
        else:
            return "----:com.serato.dj:markers_v2" in f
    except Exception:
        return False
    return False


def set_cues(filepath, cues):
    ext = Path(filepath).suffix.lower()
    data = _encode_markers2_geob(cues)
    if ext == ".mp3":
        from mutagen.id3 import ID3, GEOB, ID3NoHeaderError
        try:
            tags = ID3(filepath)
        except ID3NoHeaderError:
            tags = ID3()
        # purge any previous Markers2 frame
        for k in list(tags.keys()):
            if k.startswith("GEOB:Serato Markers2"):
                del tags[k]
        tags.add(
            GEOB(
                encoding=3,
                mime="application/octet-stream",
                filename="",
                desc="Serato Markers2",
                data=data,
            )
        )
        tags.save(filepath)
    elif ext in (".m4a", ".aac"):
        from mutagen.mp4 import MP4, MP4FreeForm
        f = MP4(filepath)
        f["----:com.serato.dj:markers_v2"] = [MP4FreeForm(data)]
        f.save()
    else:
        raise ValueError(f"unsupported format: {ext}")


# ── Beat detection ────────────────────────────────────────────────────────

def detect_first_beat_ms(filepath, hint_bpm=None, trust_bpm=False):
    """Return (first_strong_beat_ms, bpm). When trust_bpm=True (BPM is
    already tagged in the file), skip HPSS and shorten the audio window —
    we only need to locate the first strong beat, not estimate tempo."""
    import librosa
    import numpy as np
    # Shorter window when we're not estimating tempo — first beat lands in
    # the first ~10s of any real track. 60s window when BPM is unknown so
    # tempo estimation is stable.
    duration = 30 if trust_bpm else 60
    y, sr = librosa.load(filepath, sr=22050, duration=duration, offset=0.0, mono=True)
    if y.size == 0:
        return None, 0.0
    # HPSS (harmonic/percussive separation) is the most expensive step in
    # this pipeline. Only run it when we actually need clean drums for
    # tempo estimation. When BPM is tagged, the raw signal is fine.
    y_beat = y if trust_bpm else librosa.effects.hpss(y)[1]
    kwargs = {"y": y_beat, "sr": sr, "tightness": 100}
    if hint_bpm and hint_bpm > 0:
        kwargs["start_bpm"] = float(hint_bpm)
    tempo, beats = librosa.beat.beat_track(**kwargs)
    if len(beats) == 0:
        return None, float(tempo) if np.isscalar(tempo) else float(tempo[0])
    try:
        tempo_f = float(tempo)
    except (TypeError, ValueError):
        tempo_f = float(tempo[0]) if hasattr(tempo, "__len__") else 0.0

    # Among the first 8 beats, pick the one with strongest onset energy.
    # Real downbeats tend to be kick-heavy, pickup beats are quieter.
    onset_env = librosa.onset.onset_strength(y=y_beat, sr=sr)
    candidate_beats = beats[: min(8, len(beats))]
    best_beat = candidate_beats[0]
    best_strength = 0.0
    for b in candidate_beats:
        if b < len(onset_env):
            s = float(onset_env[b])
            if s > best_strength:
                best_strength = s
                best_beat = b
    first_beat_s = float(librosa.frames_to_time(best_beat, sr=sr))
    return first_beat_s * 1000.0, tempo_f


def compute_cue_positions(first_beat_ms, bpm, num_cues, bars_per_cue, beats_per_bar=4):
    ms_per_beat = 60_000.0 / bpm
    ms_per_cue = bars_per_cue * beats_per_bar * ms_per_beat
    return [first_beat_ms + i * ms_per_cue for i in range(num_cues)]


# ── Main ──────────────────────────────────────────────────────────────────

def process_file(path, bars, n_cues, force, dry, crates_root=None, cue_index=0):
    # Skip broken symlinks (target missing) or unreadable paths before librosa hits them
    try:
        if not Path(path).is_file():
            return "SKIP", "missing or broken symlink"
    except OSError:
        return "SKIP", "unreadable path"
    if not force and has_existing_cues(str(path)):
        return "SKIP", "already has cues"
    # Genre-aware BPM hint: picks 132 for Jersey Club, 85 for Hip-Hop, etc.
    lo, hi, hint = (
        genre_bpm_for_path(path, crates_root) if crates_root else DEFAULT_BPM_RANGE
    )
    bpm = get_existing_bpm(str(path))
    # If file has a tagged BPM, trust it and take the HPSS-skip fast path;
    # else use the genre hint to bias librosa's tempo search on clean drums.
    first_beat_ms, detected_bpm = detect_first_beat_ms(
        str(path),
        hint_bpm=bpm if bpm > 0 else hint,
        trust_bpm=bpm > 0,
    )
    use_bpm = bpm if bpm > 0 else detected_bpm
    # Snap to genre range. Falls back to the hint if no halve/double fits.
    use_bpm = normalize_bpm_to_range(use_bpm, lo, hi, hint=hint)
    if first_beat_ms is None or use_bpm <= 0:
        return "ERR", "couldn't detect beats"
    positions = compute_cue_positions(first_beat_ms, use_bpm, n_cues, bars)
    # cue_index is the starting Serato hot-cue slot (0-7). For multi-cue runs
    # subsequent cues fill the next slots mod 8.
    cues = [
        {
            "index": (cue_index + i) % 8,
            "position_ms": int(p),
            "color": DEFAULT_COLORS[(cue_index + i) % 8],
            "name": "",
        }
        for i, p in enumerate(positions)
    ]
    if not dry:
        try:
            set_cues(str(path), cues)
        except Exception as e:
            return "ERR", f"{type(e).__name__}: {e}"
    return "OK", f"{use_bpm:.2f} {first_beat_ms/1000:.3f}"


def _worker(payload):
    """Top-level worker for multiprocessing.Pool (must be picklable)."""
    path, bars, n_cues, force, dry, crates_root, cue_index = payload
    return str(path), process_file(
        path, bars, n_cues, force, dry, crates_root, cue_index
    )


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--crates-root", required=True)
    ap.add_argument("--bars", type=int, default=16, help="bars between cues (default 16)")
    ap.add_argument("--cues", type=int, default=4, help="number of cues to set (default 4)")
    ap.add_argument("--cue-index", type=int, default=0,
                    help="starting Serato hot-cue slot 0-7 (default 0)")
    ap.add_argument("--limit", type=int, default=0, help="process at most N files (0=all)")
    ap.add_argument("--force", action="store_true", help="overwrite existing cues")
    ap.add_argument("--dry-run", action="store_true", help="compute but don't write")
    ap.add_argument("--workers", type=int, default=0,
                    help="parallel workers (0=auto, cpu_count-2)")
    args = ap.parse_args()

    root = Path(args.crates_root) / "GENRES"
    if not root.is_dir():
        print(f"ERR - GENRES folder missing at {root}", file=sys.stderr)
        sys.exit(1)

    files = []
    for dp, _, fs in os.walk(root):
        for f in fs:
            if Path(f).suffix.lower() in AUDIO_EXTS:
                files.append(Path(dp) / f)

    if args.limit > 0:
        files = files[: args.limit]
    total = len(files)

    workers = args.workers if args.workers > 0 else max(1, (os.cpu_count() or 4) - 2)

    ok = skip = err = 0

    def tally(status, path_name, idx):
        nonlocal ok, skip, err
        if status == "OK":
            ok += 1
        elif status == "SKIP":
            skip += 1
        else:
            err += 1
        if idx % 10 == 0 or idx == total:
            print(f"PROGRESS\t{idx}\t{total}\t{path_name}")

    if workers <= 1 or total <= 4:
        for idx, p in enumerate(files, 1):
            status, detail = process_file(
                p, args.bars, args.cues, args.force, args.dry_run,
                args.crates_root, args.cue_index,
            )
            print(f"{status}\t{p}\t{detail}")
            tally(status, p.name, idx)
    else:
        from multiprocessing import Pool
        payload = [
            (p, args.bars, args.cues, args.force, args.dry_run,
             args.crates_root, args.cue_index)
            for p in files
        ]
        # chunksize=1 so we stream results promptly; librosa per-file is slow
        # enough that the pickle round-trip overhead is negligible.
        with Pool(workers) as pool:
            for idx, (path, (status, detail)) in enumerate(
                pool.imap_unordered(_worker, payload, chunksize=1), 1
            ):
                print(f"{status}\t{path}\t{detail}")
                tally(status, Path(path).name, idx)

    print(f"DONE\t{total}\t{ok}\t{skip}\t{err}")


if __name__ == "__main__":
    main()
