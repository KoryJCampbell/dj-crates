#!/usr/bin/env python3
"""
enrich_metadata.py — Fill in missing ID3 metadata from Spotify's catalog.

Searches Spotify by artist + title (from existing tags or filename parsing),
then writes back missing fields: album, year, genre, artist, title.

Usage:
  python3 enrich_metadata.py                # enrich tracks with missing metadata
  python3 enrich_metadata.py --dry-run      # preview without writing
  python3 enrich_metadata.py --limit 100    # only process first N tracks
  python3 enrich_metadata.py --all          # re-enrich all tracks (overwrite existing)
"""

import json
import os
import re
import sys
import time
from functools import partial
from pathlib import Path

# Force unbuffered output
print = partial(print, flush=True)

import mutagen
from mutagen.id3 import (
    TALB, TCON, TDRC, TIT2, TPE1, TPE2,
    ID3, ID3NoHeaderError,
)

# ── Config ──────────────────────────────────────────────────────────────────

GENRES_ROOT = Path("/Users/koryjcampbell/Music/CRATES/GENRES")
AUDIO_EXTENSIONS = {".mp3"}

def spotify_creds() -> tuple[str, str]:
    """SPOTIPY_CLIENT_ID/SECRET env vars first, then the DJ Crates app settings."""
    cid = os.environ.get("SPOTIPY_CLIENT_ID", "")
    secret = os.environ.get("SPOTIPY_CLIENT_SECRET", "")
    if cid and secret:
        return cid, secret
    settings = (Path.home() / "Library/Application Support"
                / "com.koryjcampbell.dj-crates" / "settings.json")
    try:
        cfg = json.loads(settings.read_text()).get("config", {})
        cid, secret = cfg.get("spotifyClientId", ""), cfg.get("spotifyClientSecret", "")
    except (OSError, json.JSONDecodeError):
        pass
    if not (cid and secret):
        sys.exit("Spotify credentials not found. Set SPOTIPY_CLIENT_ID / "
                 "SPOTIPY_CLIENT_SECRET or configure them in DJ Crates settings.")
    return cid, secret


# ── Filename Parsing ────────────────────────────────────────────────────────

def parse_filename(filename: str) -> tuple[str, str]:
    """
    Try to extract artist and title from common filename patterns:
      "123 - Artist - Title.mp3"
      "Artist - Title.mp3"
      "Artist_-_Title.mp3"
    """
    name = os.path.splitext(filename)[0]

    # Remove leading track number: "123 - " or "01. "
    name = re.sub(r"^\d{1,3}\s*[-\.]\s*", "", name)

    # Split on " - "
    if " - " in name:
        parts = name.split(" - ", 1)
        artist = parts[0].strip()
        title = parts[1].strip()
        # Clean feat/ft from title
        title = re.sub(r"\s*\(feat\..*?\)", "", title, flags=re.IGNORECASE)
        return artist, title

    return "", name.strip()


# ── Spotify Client ──────────────────────────────────────────────────────────

def init_spotify():
    """Initialize Spotify client with client credentials."""
    import spotipy
    from spotipy.oauth2 import SpotifyClientCredentials

    client_id, client_secret = spotify_creds()
    return spotipy.Spotify(
        auth_manager=SpotifyClientCredentials(
            client_id=client_id,
            client_secret=client_secret,
        )
    )


def search_spotify(sp, artist: str, title: str) -> dict | None:
    """
    Search Spotify for a track and return metadata.

    Returns dict with: artist, title, album, year, album_artist, genres
    """
    if not artist and not title:
        return None

    # Clean up search terms
    title_clean = re.sub(r"\(feat\..*?\)", "", title, flags=re.IGNORECASE).strip()
    title_clean = re.sub(r"\[.*?\]", "", title_clean).strip()
    title_clean = re.sub(r"\(.*?remix.*?\)", "", title_clean, flags=re.IGNORECASE).strip()
    title_clean = re.sub(r"\(.*?edit.*?\)", "", title_clean, flags=re.IGNORECASE).strip()

    # Try with artist + title first
    queries = []
    if artist and title_clean:
        queries.append(f"artist:{artist} track:{title_clean}")
    if artist and title:
        queries.append(f"{artist} {title_clean}")
    if title_clean:
        queries.append(f"track:{title_clean}")

    for query in queries:
        try:
            results = sp.search(q=query, type="track", limit=3)
            items = results["tracks"]["items"]

            if not items:
                continue

            # Find best match
            for item in items:
                sp_artist = item["artists"][0]["name"]
                sp_title = item["name"]

                # Loose match: check if artist name appears in our artist or vice versa
                artist_lower = artist.lower() if artist else ""
                sp_artist_lower = sp_artist.lower()

                artist_match = (
                    not artist
                    or artist_lower in sp_artist_lower
                    or sp_artist_lower in artist_lower
                    or any(
                        a["name"].lower() in artist_lower
                        for a in item["artists"]
                    )
                )

                if artist_match:
                    album = item["album"]
                    release_date = album.get("release_date", "")
                    year = release_date[:4] if release_date else ""

                    return {
                        "artist": sp_artist,
                        "title": sp_title,
                        "album": album.get("name", ""),
                        "album_artist": item["artists"][0]["name"],
                        "year": year,
                        "genres": [],
                        "all_artists": ", ".join(a["name"] for a in item["artists"]),
                    }

        except Exception as e:
            err = str(e)
            if "rate" in err.lower() or "429" in err or "limit" in err.lower():
                print(f"    Rate limited, waiting 30s...")
                time.sleep(30)
                # Retry this query
                try:
                    results = sp.search(q=query, type="track", limit=3)
                    items = results["tracks"]["items"]
                    if items:
                        item = items[0]
                        album = item["album"]
                        release_date = album.get("release_date", "")
                        year = release_date[:4] if release_date else ""
                        return {
                            "artist": item["artists"][0]["name"],
                            "title": item["name"],
                            "album": album.get("name", ""),
                            "album_artist": item["artists"][0]["name"],
                            "year": year,
                            "genres": [],
                            "all_artists": ", ".join(a["name"] for a in item["artists"]),
                        }
                except Exception:
                    pass
            else:
                time.sleep(0.5)
            continue

    return None


# ── ID3 Writing ─────────────────────────────────────────────────────────────

def enrich_track(filepath: str, spotify_data: dict, overwrite_all: bool = False, dry_run: bool = False) -> dict:
    """
    Write missing metadata from Spotify to ID3 tags.

    Returns dict of fields that were written.
    """
    written = {}

    try:
        audio = mutagen.File(filepath)
        if audio is None:
            return written

        if audio.tags is None:
            if isinstance(audio, mutagen.mp3.MP3):
                audio.add_tags()
            else:
                return written

        tags = audio.tags

        # Artist
        current_artist = str(tags.get("TPE1", "")).strip()
        if (not current_artist or overwrite_all) and spotify_data.get("artist"):
            written["artist"] = spotify_data["artist"]
            if not dry_run:
                tags.delall("TPE1")
                tags.add(TPE1(encoding=3, text=[spotify_data["artist"]]))

        # Title
        current_title = str(tags.get("TIT2", "")).strip()
        if (not current_title or overwrite_all) and spotify_data.get("title"):
            written["title"] = spotify_data["title"]
            if not dry_run:
                tags.delall("TIT2")
                tags.add(TIT2(encoding=3, text=[spotify_data["title"]]))

        # Album (always fill if missing — this is our biggest gap)
        current_album = str(tags.get("TALB", "")).strip()
        if (not current_album or overwrite_all) and spotify_data.get("album"):
            written["album"] = spotify_data["album"]
            if not dry_run:
                tags.delall("TALB")
                tags.add(TALB(encoding=3, text=[spotify_data["album"]]))

        # Album Artist
        current_aa = str(tags.get("TPE2", "")).strip()
        if (not current_aa or overwrite_all) and spotify_data.get("album_artist"):
            written["album_artist"] = spotify_data["album_artist"]
            if not dry_run:
                tags.delall("TPE2")
                tags.add(TPE2(encoding=3, text=[spotify_data["album_artist"]]))

        # Year
        current_year = str(tags.get("TDRC", "")).strip()
        if not current_year:
            current_year = str(tags.get("TYER", "")).strip()
        if (not current_year or overwrite_all) and spotify_data.get("year"):
            written["year"] = spotify_data["year"]
            if not dry_run:
                tags.delall("TDRC")
                tags.add(TDRC(encoding=3, text=[spotify_data["year"]]))

        # Genre (only if missing — don't overwrite our classification)
        current_genre = str(tags.get("TCON", "")).strip()
        if not current_genre and spotify_data.get("genres"):
            genre = spotify_data["genres"][0].title()
            written["genre"] = genre
            if not dry_run:
                tags.delall("TCON")
                tags.add(TCON(encoding=3, text=[genre]))

        if written and not dry_run:
            audio.save()

    except Exception as e:
        written["_error"] = str(e)

    return written


# ── Main ────────────────────────────────────────────────────────────────────

def main():
    dry_run = "--dry-run" in sys.argv
    overwrite_all = "--all" in sys.argv
    limit = 0
    for i, arg in enumerate(sys.argv):
        if arg == "--limit" and i + 1 < len(sys.argv):
            limit = int(sys.argv[i + 1])

    print("=" * 60)
    print("  DJ Crates — Spotify Metadata Enrichment")
    print("=" * 60)
    if dry_run:
        print("  Mode: DRY RUN")
    print()

    # Init Spotify
    print("Connecting to Spotify...")
    try:
        sp = init_spotify()
        # Quick test
        sp.search(q="test", type="track", limit=1)
        print("  Connected!\n")
    except Exception as e:
        print(f"  Failed: {e}")
        return

    # Scan for tracks with missing metadata
    print("Scanning for tracks with missing metadata...")
    tracks = []

    for dirpath, dirnames, filenames in os.walk(GENRES_ROOT):
        dirnames[:] = [d for d in dirnames if not d.startswith(".")]
        for fname in filenames:
            ext = os.path.splitext(fname)[1].lower()
            if ext not in AUDIO_EXTENSIONS:
                continue

            filepath = os.path.join(dirpath, fname)

            try:
                audio = mutagen.File(filepath)
                if audio is None:
                    continue

                tags = audio.tags or {}
                artist = str(tags.get("TPE1", "")).strip()
                title = str(tags.get("TIT2", "")).strip()
                album = str(tags.get("TALB", "")).strip()
                year = str(tags.get("TDRC", str(tags.get("TYER", "")))).strip()
                genre = str(tags.get("TCON", "")).strip()

                needs_enrichment = overwrite_all or not album or not year or not artist or not title or not genre

                if not needs_enrichment:
                    continue

                # If no artist/title in tags, try parsing from filename
                if not artist or not title:
                    fn_artist, fn_title = parse_filename(fname)
                    if not artist:
                        artist = fn_artist
                    if not title:
                        title = fn_title

                tracks.append({
                    "filepath": filepath,
                    "artist": artist,
                    "title": title,
                    "has_album": bool(album),
                    "has_year": bool(year),
                    "has_artist": bool(str(tags.get("TPE1", "")).strip()),
                    "has_title": bool(str(tags.get("TIT2", "")).strip()),
                })

            except Exception:
                continue

    if limit > 0:
        tracks = tracks[:limit]

    print(f"  {len(tracks)} tracks need enrichment\n")

    if not tracks:
        print("Nothing to do — all tracks have complete metadata.")
        return

    # Process tracks
    print("Enriching metadata from Spotify...")
    enriched = 0
    not_found = 0
    errors = 0
    field_counts = {"artist": 0, "title": 0, "album": 0, "year": 0, "genre": 0, "album_artist": 0}
    start = time.time()

    for i, track in enumerate(tracks):
        # Progress
        if i % 50 == 0:
            elapsed = time.time() - start
            rate = i / elapsed if elapsed > 0 else 0
            eta = (len(tracks) - i) / rate if rate > 0 else 0
            print(f"  [{i + 1}/{len(tracks)}] enriched: {enriched}, not found: {not_found}, "
                  f"rate: {rate:.1f}/sec, ETA: {eta / 60:.0f}min")

        # Search Spotify
        result = search_spotify(sp, track["artist"], track["title"])

        if result is None:
            not_found += 1
            continue

        # Write to ID3
        written = enrich_track(track["filepath"], result, overwrite_all=overwrite_all, dry_run=dry_run)

        if "_error" in written:
            errors += 1
            continue

        if written:
            enriched += 1
            for field in written:
                if field in field_counts:
                    field_counts[field] += 1

        # Rate limiting — ~1 request per second to stay safe
        time.sleep(0.5)

    elapsed = time.time() - start

    print(f"\n{'=' * 60}")
    print(f"  Enriched:    {enriched} tracks")
    print(f"  Not found:   {not_found}")
    print(f"  Errors:      {errors}")
    print(f"  Time:        {elapsed / 60:.1f} minutes")
    print(f"\n  Fields filled:")
    for field, count in sorted(field_counts.items()):
        if count > 0:
            print(f"    {field}: {count}")
    print(f"{'=' * 60}")


if __name__ == "__main__":
    main()
