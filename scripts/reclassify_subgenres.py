#!/usr/bin/env python3
"""
reclassify_subgenres.py — Use Spotify artist genres + librosa analysis to
improve subgenre classification. Creates new subgenre folders as needed.

Flow:
  1. Query Spotify for each unique artist's genres
  2. Map Spotify genres → our subgenre system
  3. Compare current placement vs recommended placement
  4. Move misplaced tracks, creating new subgenre folders as needed
  5. Clean up empty directories

Usage:
  python3 reclassify_subgenres.py               # full reclassification
  python3 reclassify_subgenres.py --dry-run      # preview moves without executing
  python3 reclassify_subgenres.py --report-only   # just show the report
"""

import json
import os
import re
import shutil
import sys
import time
from collections import defaultdict
from functools import partial
from pathlib import Path

import mutagen

print = partial(print, flush=True)

# ── Config ──────────────────────────────────────────────────────────────────

GENRES_ROOT = Path("/Users/koryjcampbell/Music/CRATES/GENRES")


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

# ── Spotify Genre → Our Genre/Subgenre Mapping ─────────────────────────────
# Spotify returns lowercase genre strings like "trap", "southern hip hop", etc.
# We map them to (Genre, Subgenre) tuples in our system.

SPOTIFY_GENRE_MAP = {
    # ── Hip-Hop ─────────────────────────────────────────────────
    "hip hop": ("Hip-Hop:Rap", "General Hip-Hop"),
    "rap": ("Hip-Hop:Rap", "General Hip-Hop"),
    "trap": ("Hip-Hop:Rap", "Trap"),
    "trap music": ("Hip-Hop:Rap", "Trap"),
    "southern hip hop": ("Hip-Hop:Rap", "Southern"),
    "dirty south rap": ("Hip-Hop:Rap", "Southern"),
    "crunk": ("Hip-Hop:Rap", "Crunk"),
    "east coast hip hop": ("Hip-Hop:Rap", "East Coast"),
    "nyc rap": ("Hip-Hop:Rap", "East Coast"),
    "new york rap": ("Hip-Hop:Rap", "East Coast"),
    "west coast rap": ("Hip-Hop:Rap", "West Coast"),
    "g-funk": ("Hip-Hop:Rap", "G-Funk"),
    "gangster rap": ("Hip-Hop:Rap", "Gangsta"),
    "conscious hip hop": ("Hip-Hop:Rap", "Conscious"),
    "underground hip hop": ("Hip-Hop:Rap", "Underground"),
    "alternative hip hop": ("Hip-Hop:Rap", "Alternative Hip-Hop"),
    "boom bap": ("Hip-Hop:Rap", "Boom Bap"),
    "old school hip hop": ("Hip-Hop:Rap", "Golden Age"),
    "golden age hip hop": ("Hip-Hop:Rap", "Golden Age"),
    "drill": ("Hip-Hop:Rap", "Drill"),
    "uk drill": ("Hip-Hop:Rap", "Drill"),
    "brooklyn drill": ("Hip-Hop:Rap", "Drill"),
    "chicago drill": ("Hip-Hop:Rap", "Drill"),
    "cloud rap": ("Hip-Hop:Rap", "Cloud Rap"),
    "emo rap": ("Hip-Hop:Rap", "Emo Rap"),
    "melodic rap": ("Hip-Hop:Rap", "Melodic Rap"),
    "plugg": ("Hip-Hop:Rap", "Plugg"),
    "rage rap": ("Hip-Hop:Rap", "Rage"),
    "memphis hip hop": ("Hip-Hop:Rap", "Memphis"),
    "houston rap": ("Hip-Hop:Rap", "Southern"),
    "atl hip hop": ("Hip-Hop:Rap", "Southern"),
    "atlanta hip hop": ("Hip-Hop:Rap", "Southern"),
    "miami hip hop": ("Hip-Hop:Rap", "Southern"),
    "detroit hip hop": ("Hip-Hop:Rap", "Detroit"),
    "chicago rap": ("Hip-Hop:Rap", "Chicago"),
    "pop rap": ("Hip-Hop:Rap", "Pop Rap"),
    "lgbtq+ hip hop": ("Hip-Hop:Rap", "General Hip-Hop"),
    "political hip hop": ("Hip-Hop:Rap", "Conscious"),
    "jazz rap": ("Hip-Hop:Rap", "Jazz Rap"),

    # ── R&B ─────────────────────────────────────────────────────
    "r&b": ("R&B", "Contemporary R&B"),
    "urban contemporary": ("R&B", "Contemporary R&B"),
    "contemporary r&b": ("R&B", "Contemporary R&B"),
    "neo soul": ("R&B", "Neo-Soul"),
    "neo-soul": ("R&B", "Neo-Soul"),
    "alternative r&b": ("R&B", "Alternative R&B"),
    "indie r&b": ("R&B", "Alternative R&B"),
    "new jack swing": ("R&B", "New Jack Swing"),
    "quiet storm": ("R&B", "Quiet Storm"),
    "trap soul": ("R&B", "Trap Soul"),
    "pnb": ("R&B", "Contemporary R&B"),

    # ── House ───────────────────────────────────────────────────
    "house": ("House", "General House"),
    "deep house": ("House", "Deep House"),
    "tech house": ("House", "Tech House"),
    "afro house": ("House", "Afro House"),
    "soulful house": ("House", "Soulful House"),
    "progressive house": ("House", "Progressive House"),
    "jackin house": ("House", "Jackin House"),
    "funky house": ("House", "Funky House"),
    "chicago house": ("House", "Chicago House"),
    "acid house": ("House", "Acid House"),
    "electro house": ("House", "Electro House"),
    "tropical house": ("House", "Tropical House"),
    "latin house": ("House", "Latin House"),
    "bass house": ("House", "Bass House"),
    "uk house": ("House", "UK House"),
    "disco house": ("House", "Disco House"),

    # ── Afrobeats ───────────────────────────────────────────────
    "afrobeats": ("Afrobeats", "General Afrobeats"),
    "afropop": ("Afrobeats", "Afropop"),
    "afro-pop": ("Afrobeats", "Afropop"),
    "afrofusion": ("Afrobeats", "Afrofusion"),
    "afroswing": ("Afrobeats", "Afroswing"),
    "afro-soul": ("Afrobeats", "Afro-Soul"),
    "highlife": ("Afrobeats", "Highlife"),
    "naija": ("Afrobeats", "General Afrobeats"),
    "nigerian pop": ("Afrobeats", "Afropop"),
    "ghanaian pop": ("Afrobeats", "Afropop"),
    "azonto": ("Afrobeats", "Afropop"),
    "amapiano": ("Amapiano", "General Amapiano"),

    # ── Pop ─────────────────────────────────────────────────────
    "pop": ("Pop", "General Pop"),
    "dance pop": ("Pop", "Dance Pop"),
    "synth-pop": ("Pop", "Synth Pop"),
    "synthpop": ("Pop", "Synth Pop"),
    "electropop": ("Pop", "Electropop"),
    "indie pop": ("Pop", "Indie Pop"),
    "art pop": ("Pop", "Art Pop"),
    "pop rock": ("Pop", "Pop Rock"),
    "bubblegum pop": ("Pop", "Bubblegum Pop"),
    "teen pop": ("Pop", "Teen Pop"),
    "k-pop": ("Pop", "K-Pop"),
    "j-pop": ("Pop", "J-Pop"),
    "latin pop": ("Latin", "Latin Pop"),

    # ── Dance/Electronic ────────────────────────────────────────
    "edm": ("Dance", "EDM"),
    "electronic": ("Dance", "Electronica"),
    "electronica": ("Dance", "Electronica"),
    "eurodance": ("Dance", "Eurodance"),
    "synthwave": ("Dance", "Synthwave"),
    "retrowave": ("Dance", "Synthwave"),
    "future bass": ("Dance", "Future Bass"),
    "uk garage": ("Garage", "UK Garage"),
    "speed garage": ("Garage", "Speed Garage"),
    "2-step garage": ("Garage", "2-Step"),
    "bassline": ("Bassline", "General Bassline"),
    "breakbeat": ("Breakbeat", "General Breakbeat"),

    # ── Techno ──────────────────────────────────────────────────
    "techno": ("Techno", "General Techno"),
    "detroit techno": ("Techno", "Detroit Techno"),
    "minimal techno": ("Techno", "Minimal Techno"),
    "industrial techno": ("Techno", "Industrial Techno"),
    "acid techno": ("Techno", "Acid Techno"),
    "melodic techno": ("Techno", "Melodic Techno"),
    "hard techno": ("Techno", "Hard Techno"),

    # ── Trance ──────────────────────────────────────────────────
    "trance": ("Trance", "General Trance"),
    "psytrance": ("Trance", "Psytrance"),
    "progressive trance": ("Trance", "Progressive Trance"),
    "uplifting trance": ("Trance", "Uplifting Trance"),
    "vocal trance": ("Trance", "Vocal Trance"),

    # ── Drum & Bass ─────────────────────────────────────────────
    "drum and bass": ("Drum & Bass", "General DnB"),
    "dnb": ("Drum & Bass", "General DnB"),
    "liquid dnb": ("Drum & Bass", "Liquid DnB"),
    "jungle": ("Jungle", "General Jungle"),

    # ── Reggae / Dancehall ──────────────────────────────────────
    "reggae": ("Reggae", "General Reggae"),
    "roots reggae": ("Reggae", "Roots Reggae"),
    "dub": ("Reggae", "Dub"),
    "lovers rock": ("Reggae", "Lovers Rock"),
    "dancehall": ("Dancehall", "General Dancehall"),
    "modern dancehall": ("Dancehall", "Modern Dancehall"),
    "ragga": ("Dancehall", "Ragga"),
    "reggaeton": ("Reggaeton", "General Reggaeton"),
    "perreo": ("Reggaeton", "General Reggaeton"),

    # ── Latin ───────────────────────────────────────────────────
    "latin": ("Latin", "General Latin"),
    "salsa": ("Latin", "Salsa"),
    "bachata": ("Latin", "Bachata"),
    "merengue": ("Latin", "Merengue"),
    "cumbia": ("Latin", "Cumbia"),
    "dembow": ("Latin", "Dembow"),
    "latin trap": ("Latin", "Latin Trap"),
    "urbano latino": ("Latin", "Urbano"),
    "latin hip hop": ("Latin", "Latin Hip-Hop"),

    # ── Rock ────────────────────────────────────────────────────
    "rock": ("Rock", "General Rock"),
    "classic rock": ("Rock", "Classic Rock"),
    "alternative rock": ("Rock", "Alternative"),
    "indie rock": ("Rock", "Indie Rock"),
    "punk": ("Rock", "Punk"),
    "punk rock": ("Rock", "Punk"),
    "hard rock": ("Rock", "Hard Rock"),
    "grunge": ("Rock", "Grunge"),
    "psychedelic rock": ("Rock", "Psychedelic"),
    "progressive rock": ("Rock", "Prog Rock"),
    "garage rock": ("Rock", "Garage Rock"),
    "post-punk": ("Rock", "Post-Punk"),
    "new wave": ("Rock", "New Wave"),

    # ── Soul ────────────────────────────────────────────────────
    "soul": ("Soul", "General Soul"),
    "northern soul": ("Soul", "Northern Soul"),
    "southern soul": ("Soul", "Southern Soul"),
    "philly soul": ("Soul", "Philly Soul"),
    "psychedelic soul": ("Soul", "Psychedelic Soul"),
    "classic soul": ("Soul", "Classic Soul"),
    "funk": ("Soul", "Funk"),
    "p-funk": ("Soul", "P-Funk"),

    # ── Jazz ────────────────────────────────────────────────────
    "jazz": ("Jazz", "General Jazz"),
    "smooth jazz": ("Jazz", "Smooth Jazz"),
    "jazz fusion": ("Jazz", "Jazz Fusion"),
    "acid jazz": ("Jazz", "Acid Jazz"),
    "cool jazz": ("Jazz", "Cool Jazz"),
    "latin jazz": ("Jazz", "Latin Jazz"),
    "bebop": ("Jazz", "Bebop"),

    # ── Disco ───────────────────────────────────────────────────
    "disco": ("Disco", "General Disco"),
    "nu-disco": ("Disco", "Nu-Disco"),
    "italo disco": ("Disco", "Italo Disco"),

    # ── Soca ────────────────────────────────────────────────────
    "soca": ("Soca", "General Soca"),
    "power soca": ("Soca", "Power Soca"),
    "groovy soca": ("Soca", "Groovy Soca"),

    # ── Gospel / Christian ──────────────────────────────────────
    "gospel": ("Gospel", "General Gospel"),
    "ccm": ("Christian", "General Christian"),
    "christian music": ("Christian", "General Christian"),
    "worship": ("Christian", "Worship"),

    # ── Country ─────────────────────────────────────────────────
    "country": ("Country", "General Country"),
    "country pop": ("Country", "Country Pop"),
    "country rap": ("Country", "Country Rap"),
    "bro-country": ("Country", "Bro-Country"),

    # ── Motown ──────────────────────────────────────────────────
    "motown": ("Motown", "General Motown"),

    # ── Lo-Fi ───────────────────────────────────────────────────
    "lo-fi beats": ("Lo-Fi", "General Lo-Fi"),
    "lo-fi": ("Lo-Fi", "General Lo-Fi"),
    "chillhop": ("Lo-Fi", "Chillhop"),

    # ── UK Music ────────────────────────────────────────────────
    "grime": ("UK Music", "Grime"),
    "uk hip hop": ("UK Music", "UK Hip-Hop"),
    "uk rap": ("UK Music", "UK Rap"),
    "uk funky": ("UK Funky", "General UK Funky"),

    # ── Baltimore Club ──────────────────────────────────────────
    "baltimore club": ("Baltimore Club", "General Baltimore Club"),

    # ── Jersey Club ─────────────────────────────────────────────
    "jersey club": ("Jersey Club", "General Jersey Club"),

    # ── Baile Funk ──────────────────────────────────────────────
    "baile funk": ("Baile Funk", "General Baile Funk"),
    "brazilian funk": ("Baile Funk", "General Baile Funk"),
    "funk carioca": ("Baile Funk", "General Baile Funk"),
}

# Priority order: prefer more specific genres over general ones
GENRE_PRIORITY = [
    "Trap", "Drill", "Boom Bap", "G-Funk", "Golden Age", "Crunk",
    "Southern", "East Coast", "West Coast", "Conscious", "Underground",
    "Cloud Rap", "Emo Rap", "Melodic Rap", "Memphis", "Detroit", "Chicago",
    "Jazz Rap", "Pop Rap", "Plugg", "Rage", "Alternative Hip-Hop",
    "Neo-Soul", "Alternative R&B", "Trap Soul", "New Jack Swing", "Quiet Storm",
    "Deep House", "Tech House", "Afro House", "Soulful House", "Progressive House",
    "Afropop", "Afrofusion", "Afroswing", "Highlife",
    "Grime", "UK Rap", "UK Hip-Hop",
    "Roots Reggae", "Dub", "Lovers Rock",
    "Modern Dancehall", "Ragga",
    "Nu-Disco", "Italo Disco",
    "Salsa", "Bachata", "Cumbia", "Dembow", "Latin Trap",
]


# ── Spotify ─────────────────────────────────────────────────────────────────

def init_spotify():
    import spotipy
    from spotipy.oauth2 import SpotifyClientCredentials
    client_id, client_secret = spotify_creds()
    return spotipy.Spotify(
        auth_manager=SpotifyClientCredentials(
            client_id=client_id,
            client_secret=client_secret,
        )
    )


def get_artist_genres_batch(sp, artists: list[str]) -> dict[str, list[str]]:
    """
    Look up Spotify genres for a batch of artists.
    Returns dict of artist_name -> [genres]
    """
    results = {}
    for i, artist in enumerate(artists):
        if i % 50 == 0 and i > 0:
            print(f"    Spotify artist lookup: {i}/{len(artists)}...")

        try:
            search = sp.search(q=f"artist:{artist}", type="artist", limit=1)
            items = search["artists"]["items"]
            if items:
                sp_artist = items[0]
                # Verify it's a reasonable match
                if artist.lower() in sp_artist["name"].lower() or sp_artist["name"].lower() in artist.lower():
                    results[artist] = sp_artist.get("genres", [])
        except Exception as e:
            err = str(e)
            if "rate" in err.lower() or "429" in err:
                print(f"    Rate limited at artist {i}, waiting 30s...")
                time.sleep(30)
                try:
                    search = sp.search(q=f"artist:{artist}", type="artist", limit=1)
                    items = search["artists"]["items"]
                    if items and (artist.lower() in items[0]["name"].lower() or items[0]["name"].lower() in artist.lower()):
                        results[artist] = items[0].get("genres", [])
                except Exception:
                    pass

        time.sleep(0.4)  # Rate limiting

    return results


def map_spotify_genres(spotify_genres: list[str]) -> tuple[str, str] | None:
    """
    Map a list of Spotify genre strings to our best (Genre, Subgenre) match.
    Prefers more specific matches over general ones.
    """
    matches = []
    for sg in spotify_genres:
        sg_lower = sg.lower().strip()
        if sg_lower in SPOTIFY_GENRE_MAP:
            genre, subgenre = SPOTIFY_GENRE_MAP[sg_lower]
            # Calculate priority (more specific = higher priority)
            priority = GENRE_PRIORITY.index(subgenre) if subgenre in GENRE_PRIORITY else 999
            matches.append((priority, genre, subgenre))

    if not matches:
        return None

    # Return the highest priority (lowest number) match
    matches.sort(key=lambda x: x[0])
    return (matches[0][1], matches[0][2])


# ── Main ────────────────────────────────────────────────────────────────────

def main():
    dry_run = "--dry-run" in sys.argv
    report_only = "--report-only" in sys.argv

    print("=" * 60)
    print("  DJ Crates — Subgenre Reclassification")
    print("=" * 60)
    if dry_run:
        print("  Mode: DRY RUN")
    if report_only:
        print("  Mode: REPORT ONLY")
    print()

    # Step 1: Collect all tracks and unique artists
    print("Scanning library...")
    tracks = []
    artists = set()

    for dirpath, dirnames, filenames in os.walk(GENRES_ROOT):
        dirnames[:] = [d for d in dirnames if not d.startswith(".")]
        for fname in filenames:
            ext = os.path.splitext(fname)[1].lower()
            if ext not in {".mp3", ".flac", ".wav", ".m4a", ".aac"}:
                continue

            filepath = os.path.join(dirpath, fname)
            rel = os.path.relpath(filepath, GENRES_ROOT)
            parts = rel.split(os.sep)

            if len(parts) < 3:
                continue

            genre = parts[0]
            subgenre = parts[1]

            # Read artist from ID3
            artist = ""
            try:
                audio = mutagen.File(filepath)
                if audio and audio.tags:
                    artist = str(audio.tags.get("TPE1", "")).strip()
            except Exception:
                pass

            # Fall back to filename parsing
            if not artist:
                name = os.path.splitext(fname)[0]
                name = re.sub(r"^\d{1,3}\s*[-\.]\s*", "", name)
                if " - " in name:
                    artist = name.split(" - ", 1)[0].strip()

            if artist:
                artists.add(artist)

            tracks.append({
                "filepath": filepath,
                "artist": artist,
                "genre": genre,
                "subgenre": subgenre,
                "rel_parts": parts,
            })

    print(f"  {len(tracks)} tracks, {len(artists)} unique artists\n")

    # Step 2: Look up artist genres from Spotify
    print("Looking up artist genres on Spotify...")
    try:
        sp = init_spotify()
        sp.search(q="test", type="artist", limit=1)  # Connection test
        print("  Connected!")
    except Exception as e:
        err = str(e)
        if "rate" in err.lower() or "limit" in err.lower():
            print(f"  Still rate limited by Spotify. Run again later.")
            print(f"  Falling back to filename/tag-based classification only.\n")
            sp = None
        else:
            print(f"  Spotify error: {e}")
            print(f"  Falling back to filename/tag-based classification only.\n")
            sp = None

    artist_genres = {}
    if sp:
        artist_list = sorted(artists)
        artist_genres = get_artist_genres_batch(sp, artist_list)
        print(f"  Got genres for {len(artist_genres)}/{len(artists)} artists\n")

    # Step 3: Determine recommended placement for each track
    print("Computing reclassifications...")
    moves = []
    kept = 0
    no_data = 0

    for track in tracks:
        artist = track["artist"]
        current_genre = track["genre"]
        current_sub = track["subgenre"]

        # Look up Spotify genres for this artist
        sp_genres = artist_genres.get(artist, [])
        recommended = map_spotify_genres(sp_genres)

        if recommended is None:
            no_data += 1
            continue

        rec_genre, rec_sub = recommended

        # Check if track needs moving
        # Only move if both genre AND subgenre differ, or if current sub is "General X"
        is_general = current_sub.startswith("General ")
        genre_matches = current_genre == rec_genre
        sub_matches = current_sub == rec_sub

        if genre_matches and sub_matches:
            kept += 1
            continue

        if genre_matches and not is_general:
            # Same genre, specific subgenre — only move if Spotify is more specific
            if rec_sub in GENRE_PRIORITY and current_sub not in GENRE_PRIORITY:
                pass  # Move — Spotify has better specificity
            elif not is_general:
                kept += 1
                continue

        # Build new path
        parts = track["rel_parts"]
        # Path: Genre/Subgenre/Decade/Year/file or Genre/Subgenre/UnknownYear/file
        remaining = parts[2:]  # Everything after genre/subgenre
        new_rel = os.path.join(rec_genre, rec_sub, *remaining)
        new_path = os.path.join(str(GENRES_ROOT), new_rel)

        moves.append({
            "filepath": track["filepath"],
            "artist": artist,
            "from_genre": current_genre,
            "from_sub": current_sub,
            "to_genre": rec_genre,
            "to_sub": rec_sub,
            "new_path": new_path,
            "spotify_genres": sp_genres[:5],
        })

    print(f"  Kept in place: {kept}")
    print(f"  No Spotify data: {no_data}")
    print(f"  Recommended moves: {len(moves)}\n")

    if not moves:
        print("Nothing to reclassify!")
        return

    # Step 4: Report
    # Group by move type
    move_groups = defaultdict(list)
    for m in moves:
        key = f"{m['from_genre']}/{m['from_sub']} → {m['to_genre']}/{m['to_sub']}"
        move_groups[key].append(m)

    print("Move Summary:")
    print("-" * 60)
    for key in sorted(move_groups.keys(), key=lambda k: -len(move_groups[k])):
        group = move_groups[key]
        print(f"  {key}: {len(group)} tracks")
        # Show a few examples
        for m in group[:3]:
            print(f"    {m['artist']}: {m['spotify_genres'][:3]}")
    print(f"\nTotal: {len(moves)} tracks to move\n")

    if report_only:
        return

    # Step 5: Execute moves
    if not dry_run:
        print("Moving tracks...")
    else:
        print("[DRY RUN] Would move tracks...")

    moved = 0
    errors = 0
    new_dirs = set()

    for m in moves:
        src = m["filepath"]
        dst = m["new_path"]

        # Create destination directory
        dst_dir = os.path.dirname(dst)
        if dst_dir not in new_dirs:
            if not dry_run:
                os.makedirs(dst_dir, exist_ok=True)
            new_dirs.add(dst_dir)

        # Handle filename collision
        if os.path.exists(dst) and not dry_run:
            base, ext = os.path.splitext(dst)
            i = 1
            while os.path.exists(f"{base} ({i}){ext}"):
                i += 1
            dst = f"{base} ({i}){ext}"

        if not dry_run:
            try:
                shutil.move(src, dst)
                moved += 1
            except Exception as e:
                errors += 1
        else:
            moved += 1

    # Step 6: Clean up empty directories
    if not dry_run:
        print("\nCleaning empty directories...")
        cleaned = 0
        for dirpath, dirnames, filenames in os.walk(str(GENRES_ROOT), topdown=False):
            # Remove .DS_Store in otherwise empty dirs
            real_files = [f for f in filenames if f != ".DS_Store"]
            if not real_files and not dirnames:
                for f in filenames:
                    os.remove(os.path.join(dirpath, f))
                try:
                    os.rmdir(dirpath)
                    cleaned += 1
                except OSError:
                    pass
        print(f"  Removed {cleaned} empty directories")

    print(f"\n{'=' * 60}")
    print(f"  Moved:        {moved}")
    print(f"  Errors:       {errors}")
    print(f"  New folders:  {len(new_dirs)}")
    print(f"{'=' * 60}")


if __name__ == "__main__":
    main()
