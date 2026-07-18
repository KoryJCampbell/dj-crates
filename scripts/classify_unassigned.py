#!/usr/bin/env python3
"""classify_unassigned.py — pipeline "intake" step.

Files new arrivals out of GENRES/UNASSIGNED into genre folders using a
conservative cascade (never guesses):

  1. artist_match — the artist already has filed tracks; follow their modal
     genre/subfolder (requires >=60% agreement across their filed tracks).
  2. genre_tag   — the file's ID3 genre tag maps unambiguously onto a folder.
  3. otherwise   — leave in place; it shows up in the RESULT line as `left`.

Usage:
  classify_unassigned.py --crates-root /path/to/CRATES [--execute]

Dry-run by default. Every executed move is appended to
~/.dj-crates-tools/intake-moves.log (TSV: src<TAB>dst) for undo.
Prints a final line:  RESULT:moved=N,left=N,collisions=N
"""

import argparse
import os
import re
import shutil
import sys
import unicodedata
from collections import Counter, defaultdict
from pathlib import Path

try:
    import mutagen
except ImportError:  # tag reading is optional — cascade falls back to filenames
    mutagen = None

AUDIO_EXT = {".mp3", ".m4a", ".flac", ".wav", ".aif", ".aiff", ".aac", ".ogg"}
# Folders that are curated sets or dumps, not genre signals.
NON_GENRE = {"UNASSIGNED", "Kory Likes", "-Soundcloud", "Charts", "Party"}

TAG_MAP = {
    "hip hop": "Hip-Hop:Rap", "hip-hop": "Hip-Hop:Rap", "hip hop/rap": "Hip-Hop:Rap",
    "hip-hop/rap": "Hip-Hop:Rap", "rap": "Hip-Hop:Rap", "trap": "Hip-Hop:Rap",
    "drill": "Hip-Hop:Rap", "r&b": "R&B", "rnb": "R&B", "r&b/soul": "R&B",
    "contemporary r&b": "R&B", "neo-soul": "Soul", "neo soul": "Soul", "soul": "Soul",
    "funk": "Funk", "disco": "Disco", "jazz": "Jazz", "gospel": "Gospel",
    "christian & gospel": "Gospel", "afrobeats": "Afrobeats", "afrobeat": "Afrobeats",
    "afropop": "Afrobeats", "amapiano": "Amapiano", "dancehall": "Dancehall",
    "reggae": "Reggae", "reggaeton": "Latin", "latin": "Latin", "house": "House",
    "deep house": "House", "pop": "Pop", "dance pop": "Pop", "dance": "Dance",
    "edm": "Dance", "electronic": "Electronic", "rock": "Rock",
    "jungle": "Jungle", "drum & bass": "Jungle", "gogo": "GoGo", "go-go": "GoGo",
    "world": "World", "baile funk": "Baile", "funk carioca": "Baile",
}

FEAT_RE = re.compile(r"\s*[\(\[]?(?:feat\.?|ft\.?|featuring|with|w/)\s.+", re.I)
MULTI_RE = re.compile(r"\s*(?:,|&| x | X |;| and | vs\.? )\s*", re.I)


def norm_artist(s):
    s = unicodedata.normalize("NFKC", s)
    s = FEAT_RE.sub("", s)
    s = MULTI_RE.split(s)[0]
    s = re.sub(r"[^\w\s]", "", s.lower())
    return re.sub(r"\s+", " ", s).strip()


def artist_from_filename(name):
    stem = Path(name).stem
    stem = re.sub(r"^\d+[\s.\-_]+", "", stem)
    return stem.split(" - ")[0] if " - " in stem else ""


def read_tags(p):
    artist = genre = year = ""
    if mutagen is None:
        return artist, genre, year
    try:
        f = mutagen.File(p, easy=True)
        if f and f.tags:
            artist = (f.tags.get("artist") or [""])[0]
            genre = (f.tags.get("genre") or [""])[0]
            d = (f.tags.get("date") or f.tags.get("year") or [""])[0]
            m = re.search(r"\d{4}", str(d))
            year = m.group(0) if m else ""
    except Exception:
        pass
    return artist, genre, year


def build_artist_index(genres_root):
    index = defaultdict(Counter)
    for gdir in sorted(genres_root.iterdir()):
        if not gdir.is_dir() or gdir.name in NON_GENRE:
            continue
        for dirpath, dirnames, filenames in os.walk(gdir):
            dirnames[:] = [d for d in dirnames if not d.startswith(".")]
            for fn in filenames:
                if Path(fn).suffix.lower() not in AUDIO_EXT:
                    continue
                a = artist_from_filename(fn)
                if not a:
                    continue
                rel = Path(dirpath).relative_to(genres_root).parts
                sub = rel[1] if len(rel) > 1 else ""
                na = norm_artist(a)
                if na:
                    index[na][(rel[0], sub)] += 1
    return index


def decade_subfolder(genres_root, genre, year):
    if not year:
        return None
    dec = f"{year[:3]}0s"
    gdir = genres_root / genre
    if gdir.is_dir():
        for s in gdir.iterdir():
            if s.is_dir() and s.name.startswith(dec):
                return s.name
    return None


def resolve_sub(genres_root, genre, year):
    d = decade_subfolder(genres_root, genre, year)
    if d:
        return d
    base = genre.split(":")[0]
    return f"General {base}"


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--crates-root", required=True)
    ap.add_argument("--execute", action="store_true")
    args = ap.parse_args()

    genres_root = Path(args.crates_root) / "GENRES"
    unassigned = genres_root / "UNASSIGNED"
    if not unassigned.is_dir():
        print("RESULT:moved=0,left=0,collisions=0")
        return

    index = build_artist_index(genres_root)

    log_path = Path.home() / ".dj-crates-tools" / "intake-moves.log"
    log_path.parent.mkdir(parents=True, exist_ok=True)

    moved = left = collisions = 0
    with open(log_path, "a") as log:
        for dirpath, dirnames, filenames in os.walk(unassigned):
            dirnames[:] = [d for d in dirnames if not d.startswith(".")]
            for fn in sorted(filenames):
                p = Path(dirpath) / fn
                if p.suffix.lower() not in AUDIO_EXT:
                    continue
                t_artist, t_genre, year = read_tags(p)
                na = norm_artist(t_artist or artist_from_filename(fn) or "")

                genre = sub = None
                if na and na in index:
                    places = index[na]
                    total = sum(places.values())
                    by_genre = Counter()
                    for (g, s), c in places.items():
                        by_genre[g] += c
                    top, cnt = by_genre.most_common(1)[0]
                    if total == 1 or (total >= 2 and cnt / total >= 0.6):
                        genre = top
                        subs = Counter({s: c for (g, s), c in places.items() if g == top})
                        sub = subs.most_common(1)[0][0] or resolve_sub(genres_root, top, year)
                if genre is None and t_genre:
                    mapped = TAG_MAP.get(t_genre.strip().lower())
                    if mapped:
                        genre, sub = mapped, resolve_sub(genres_root, mapped, year)

                if genre is None:
                    left += 1
                    continue
                dst = genres_root / genre / sub / p.name
                if dst.exists():
                    collisions += 1
                    continue
                if args.execute:
                    dst.parent.mkdir(parents=True, exist_ok=True)
                    shutil.move(str(p), str(dst))
                    log.write(f"{p}\t{dst}\n")
                moved += 1

    mode = "" if args.execute else " (dry-run)"
    print(f"filed {moved}, left {left}, name-collisions {collisions}{mode}")
    print(f"RESULT:moved={moved},left={left},collisions={collisions}")


if __name__ == "__main__":
    main()
