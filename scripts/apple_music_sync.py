#!/usr/bin/env python3
"""apple_music_sync.py — pipeline "apple-music" step.

Mirrors CRATES/GENRES and CRATES/PLAYLISTS into Music.app as nested playlist
folders:  <ROOT folder> → <one folder per top-level dir> → <one playlist per
subfolder> (deeper nesting is aggregated into the subfolder's playlist).

Incremental: a leaf playlist is only (re)built when its Music track count
differs from the number of audio files in its folder. Rebuilds use single
`add` calls — Music.app's batch add silently drops files under sustained
load (verified 2026-07-18) — and re-verify the count afterwards.

Usage:  apple_music_sync.py --crates-root /path/to/CRATES
Prints: RESULT:leaves=N,rebuilt=N,added=N,failed=N
"""

import argparse
import os
import subprocess
import sys
import tempfile
import time
from pathlib import Path

AUDIO_EXT = {".mp3", ".m4a", ".flac", ".wav", ".aif", ".aiff", ".aac", ".ogg"}
ROOTS = ["GENRES", "PLAYLISTS"]

COUNT_SCRIPT = """
on run argv
  set rootName to item 1 of argv
  tell application "Music"
    with timeout of 900 seconds
      if not (exists folder playlist rootName) then return ""
      set gl to folder playlist rootName
      set out to ""
      repeat with thePL in (every user playlist)
        try
          set myPar to parent of thePL
          set myGpar to parent of myPar
          if (id of myGpar) is (id of gl) then
            set out to out & (name of myPar) & "/" & (name of thePL) & tab & (count of tracks of thePL) & linefeed
          end if
        end try
      end repeat
      return out
    end timeout
  end tell
end run
"""

REBUILD_SCRIPT = """
on toFile(p)
  return POSIX file p as alias
end toFile

on run argv
  set rootName to item 1 of argv
  set genreName to item 2 of argv
  set leafName to item 3 of argv
  set listPath to item 4 of argv
  tell application "Music"
    with timeout of 3600 seconds
      if not (exists folder playlist rootName) then
        make new folder playlist with properties {name:rootName}
      end if
      set gl to folder playlist rootName
      set gf to missing value
      repeat with theFP in (every folder playlist)
        try
          if (name of theFP) is genreName and (id of parent of theFP) is (id of gl) then
            set gf to theFP
            exit repeat
          end if
        end try
      end repeat
      if gf is missing value then
        set gf to make new folder playlist at gl with properties {name:genreName}
      end if
      repeat with thePL in (every user playlist)
        try
          if (name of thePL) is leafName and (id of parent of thePL) is (id of gf) then delete thePL
        end try
      end repeat
      set pl to make new user playlist at gf with properties {name:leafName}
      set fileLines to paragraphs of (read POSIX file listPath as «class utf8»)
      set added to 0
      set failed to 0
      repeat with fl in fileLines
        set p to contents of fl
        if p is not "" then
          try
            add (my toFile(p)) to pl
            set added to added + 1
          on error
            set failed to failed + 1
          end try
        end if
      end repeat
      return (added as text) & " " & (failed as text) & " " & ((count of tracks of pl) as text)
    end timeout
  end tell
end run
"""


def osascript(script, *args):
    with tempfile.NamedTemporaryFile("w", suffix=".applescript", delete=False) as f:
        f.write(script)
        path = f.name
    try:
        out = subprocess.run(
            ["osascript", path, *args],
            capture_output=True, text=True, timeout=3600, stdin=subprocess.DEVNULL,
        )
        if out.returncode != 0:
            raise RuntimeError(out.stderr.strip() or "osascript failed")
        return out.stdout.rstrip("\n")
    finally:
        os.unlink(path)


def disk_leaves(crates_root):
    """{(root, top, leaf): [files]} for every subfolder with audio files."""
    leaves = {}
    for root in ROOTS:
        rdir = crates_root / root
        if not rdir.is_dir():
            continue
        for top in sorted(rdir.iterdir()):
            if not top.is_dir() or top.name.startswith("."):
                continue
            for leaf in sorted(top.iterdir()):
                if not leaf.is_dir() or leaf.name.startswith("."):
                    continue
                files = sorted(
                    str(f) for f in leaf.rglob("*")
                    if f.is_file() and f.suffix.lower() in AUDIO_EXT
                )
                if files:
                    leaves[(root, top.name, leaf.name)] = files
    return leaves


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--crates-root", required=True)
    args = ap.parse_args()
    crates_root = Path(args.crates_root)

    subprocess.run(["open", "-g", "-a", "Music"], check=False)
    time.sleep(3)

    leaves = disk_leaves(crates_root)

    # Current counts in Music, per root folder.
    music_counts = {}
    for root in ROOTS:
        try:
            dump = osascript(COUNT_SCRIPT, root)
        except RuntimeError as e:
            print(f"count query failed for {root}: {e}")
            dump = ""
        for line in dump.splitlines():
            if "\t" in line:
                key, n = line.rsplit("\t", 1)
                try:
                    music_counts[(root, *key.split("/", 1))] = int(n)
                except ValueError:
                    pass

    rebuilt = added_total = failed_total = 0
    for (root, top, leaf), files in leaves.items():
        if music_counts.get((root, top, leaf)) == len(files):
            continue
        with tempfile.NamedTemporaryFile("w", suffix=".txt", delete=False) as f:
            f.write("\n".join(files))
            list_path = f.name
        try:
            out = osascript(REBUILD_SCRIPT, root, top, leaf, list_path)
            added, failed, now = (int(x) for x in out.split())
            rebuilt += 1
            added_total += added
            failed_total += failed
            status = "ok" if now == len(files) else f"MISMATCH now={now} want={len(files)}"
            print(f"{root}/{top}/{leaf}: added={added} failed={failed} [{status}]")
            if now != len(files):
                failed_total += abs(len(files) - now)
        except (RuntimeError, ValueError) as e:
            print(f"{root}/{top}/{leaf}: rebuild failed: {e}")
            failed_total += 1
        finally:
            os.unlink(list_path)

    print(f"RESULT:leaves={len(leaves)},rebuilt={rebuilt},added={added_total},failed={failed_total}")
    sys.exit(1 if failed_total else 0)


if __name__ == "__main__":
    main()
