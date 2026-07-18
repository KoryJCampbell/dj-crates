#!/usr/bin/env python3
"""
analyze_new_tracks.py — Quick pass: only analyze tracks missing energy tags.
Called by the Tauri app during sync. Exits fast if nothing to do.
"""

import os
import sys
import time
import warnings
from functools import partial
from pathlib import Path

import librosa
import mutagen
import numpy as np
from mutagen.id3 import COMM, TBPM

warnings.filterwarnings("ignore")
print = partial(print, flush=True)

GENRES_ROOT = Path("/Users/koryjcampbell/Music/CRATES/GENRES")
AUDIO_EXTENSIONS = {".mp3", ".flac", ".wav", ".ogg", ".aif", ".aiff", ".aac", ".m4a"}

BPM_MIN, BPM_MAX = 65.0, 135.0
LOW_BPM_SUBGENRES = {
    "trap", "general trap", "trap soul", "southern", "crunk",
    "cloud rap", "chopped & screwed",
}
RMS_CEIL, CENTROID_CEIL, ONSET_CEIL, CONTRAST_CEIL = 0.40, 5000.0, 4.0, 30.0


def normalize_bpm(bpm, subgenre):
    if bpm <= 0:
        return 0.0
    n = bpm
    while n > BPM_MAX:
        n /= 2.0
    while n < BPM_MIN:
        n *= 2.0
    if subgenre.lower() in LOW_BPM_SUBGENRES and n > 90.0:
        h = n / 2.0
        if h >= BPM_MIN:
            n = h
    return round(n, 2)


def analyze_track(filepath):
    try:
        duration = librosa.get_duration(path=filepath)
        offset = max(0, duration * 0.3)
        if offset + 30 > duration:
            offset = max(0, duration - 30)
        y, sr = librosa.load(filepath, duration=30, offset=offset, sr=22050)
        if len(y) < sr:
            return None

        rms = min(float(np.mean(librosa.feature.rms(y=y)[0])) / RMS_CEIL, 1.0)
        brightness = min(float(np.mean(librosa.feature.spectral_centroid(y=y, sr=sr)[0])) / CENTROID_CEIL, 1.0)
        onset = librosa.onset.onset_strength(y=y, sr=sr)
        rhythm = min(float(np.mean(onset)) / ONSET_CEIL, 1.0)
        contrast = librosa.feature.spectral_contrast(y=y, sr=sr)
        punch = min(float(np.mean(contrast)) / CONTRAST_CEIL, 1.0)

        _, beats = librosa.beat.beat_track(y=y, sr=sr)
        if len(beats) > 2:
            bi = np.diff(beats)
            dance = 1.0 - min(float(np.std(bi)) / float(np.mean(bi) + 1e-6), 1.0)
        else:
            dance = 0.0

        score = (rms * 0.30 + brightness * 0.15 + rhythm * 0.30 + punch * 0.10 + dance * 0.15) * 10
        score = max(1, min(10, round(score)))

        feel = "Intense" if score >= 8 else "High" if score >= 6 else "Medium" if score >= 4 else "Chill"
        blabel = "High" if brightness > 0.6 else "Med" if brightness > 0.3 else "Low"

        return {
            "score": score, "feel": feel, "dance": dance, "bright": blabel,
        }
    except Exception:
        return None


def main():
    start = time.time()

    # Find untagged tracks
    untagged = []
    for dirpath, dirnames, filenames in os.walk(GENRES_ROOT):
        dirnames[:] = [d for d in dirnames if not d.startswith(".")]
        for f in filenames:
            ext = os.path.splitext(f)[1].lower()
            if ext not in AUDIO_EXTENSIONS:
                continue
            filepath = os.path.join(dirpath, f)
            try:
                audio = mutagen.File(filepath, easy=False)
                if audio and audio.tags:
                    tagged = False
                    for key in audio.tags:
                        if key.startswith("COMM") and isinstance(audio.tags[key], COMM):
                            if audio.tags[key].desc in ("energy", "") and "Energy:" in str(audio.tags[key].text):
                                tagged = True
                                break
                    if tagged:
                        continue

                rel = os.path.relpath(filepath, GENRES_ROOT)
                parts = rel.split(os.sep)
                subgenre = parts[1] if len(parts) > 1 else ""
                untagged.append((filepath, subgenre))
            except Exception:
                continue

    if not untagged:
        print(f"RESULT:analyzed=0,bpm_fixed=0,errors=0")
        return

    print(f"Analyzing {len(untagged)} new tracks...")
    analyzed = 0
    bpm_fixed = 0
    errors = 0

    for i, (filepath, subgenre) in enumerate(untagged):
        if i % 25 == 0 and i > 0:
            elapsed = time.time() - start
            rate = i / elapsed if elapsed > 0 else 0
            eta = (len(untagged) - i) / rate if rate > 0 else 0
            print(f"  [{i}/{len(untagged)}] {rate:.1f}/sec ETA:{eta/60:.0f}min")

        result = analyze_track(filepath)
        if result is None:
            errors += 1
            continue

        # Write energy tag
        try:
            audio = mutagen.File(filepath)
            if audio is None:
                continue
            if audio.tags is None and hasattr(audio, "add_tags"):
                audio.add_tags()

            comment = f"Energy:{result['score']} | Feel:{result['feel']} | Dance:{result['dance']:.2f} | Bright:{result['bright']}"
            # Write to default comment (empty desc) so Serato shows it in Comments column
            audio.tags.delall("COMM")
            audio.tags.add(COMM(encoding=3, lang="eng", desc="", text=[comment]))

            # Fix BPM
            bpm_tag = audio.tags.get("TBPM")
            if bpm_tag:
                try:
                    orig = float(str(bpm_tag))
                    normed = normalize_bpm(orig, subgenre)
                    if abs(orig - normed) > 0.01:
                        audio.tags.delall("TBPM")
                        audio.tags.add(TBPM(encoding=3, text=[f"{normed:.2f}"]))
                        bpm_fixed += 1
                except (ValueError, TypeError):
                    pass

            audio.save()
            analyzed += 1
        except Exception:
            errors += 1

    print(f"RESULT:analyzed={analyzed},bpm_fixed={bpm_fixed},errors={errors}")


if __name__ == "__main__":
    main()
