#!/usr/bin/env python3
"""pool_sync.py — pull new tracks from a DJ record-pool FTP and file them.

A record pool has far more than you want, so this only ever looks at what's
NEW since the last run, keeps the tracks matching your filter rules, downloads
them into GENRES/UNASSIGNED, and then runs the same artist-match classifier the
one-button Sync uses — so filed tracks land straight in their genre folders.

Nothing here touches Deezer/Spotify/streaming — it's a plain FTP client for a
service you already pay for.

Config:  ~/.dj-crates-tools/pool.json   (created from --init if missing)
State:   ~/.dj-crates-tools/pool-seen.json      (remote files already seen)
Log:     ~/.dj-crates-tools/pool-downloads.log  (TSV: remote<TAB>local)

Usage:
  pool_sync.py --init                       # write a config template
  pool_sync.py --crates-root /path/CRATES   # dry-run: list what WOULD download
  pool_sync.py --crates-root /path/CRATES --execute
  pool_sync.py --crates-root /path/CRATES --execute --all   # ignore new-only
  pool_sync.py --crates-root /path/CRATES --execute --no-classify

Password resolution: env DJPOOL_PASSWORD  >  config "password".
Prints: RESULT:new=N,matched=N,downloaded=N,failed=N,filed=N
"""

import argparse
import ftplib
import json
import os
import ssl
import subprocess
import sys
from functools import partial
from pathlib import Path

print = partial(print, flush=True)

STATE_DIR = Path.home() / ".dj-crates-tools"
CONFIG_PATH = STATE_DIR / "pool.json"
SEEN_PATH = STATE_DIR / "pool-seen.json"
LOG_PATH = STATE_DIR / "pool-downloads.log"

AUDIO_EXT = {".mp3", ".m4a", ".flac", ".wav", ".aif", ".aiff", ".aac", ".ogg"}

CONFIG_TEMPLATE = {
    "host": "ftp.your-pool.com",
    "port": 21,
    "tls": True,
    "passive": True,
    "user": "your-username",
    "password": "",
    "_password_note": "Leave blank and export DJPOOL_PASSWORD instead (safer).",
    "remote_dirs": ["/"],
    "filters": {
        "extensions": [".mp3", ".m4a", ".flac", ".wav", ".aiff"],
        "include_any": [],
        "_include_note": "If non-empty, filename must contain at least one of these (case-insensitive).",
        "exclude_any": ["(acapella)", "(intro)", "(instrumental)"],
        "_exclude_note": "Drop if filename contains any of these.",
        "min_bytes": 500000,
        "max_bytes": 0,
        "_bytes_note": "max_bytes 0 = no cap.",
    },
}


# ── config / state ───────────────────────────────────────────────────────────

def load_json(path, default):
    try:
        return json.loads(path.read_text())
    except (OSError, json.JSONDecodeError):
        return default


def write_config_template():
    STATE_DIR.mkdir(parents=True, exist_ok=True)
    if CONFIG_PATH.exists():
        print(f"config already exists: {CONFIG_PATH}")
        return
    CONFIG_PATH.write_text(json.dumps(CONFIG_TEMPLATE, indent=2))
    os.chmod(CONFIG_PATH, 0o600)
    print(f"wrote {CONFIG_PATH} — fill in host/user, then set DJPOOL_PASSWORD.")


# ── FTP ──────────────────────────────────────────────────────────────────────

def connect(cfg):
    host, port = cfg["host"], int(cfg.get("port", 21))
    user = cfg.get("user", "")
    password = os.environ.get("DJPOOL_PASSWORD") or cfg.get("password", "")
    if not host or host == "ftp.your-pool.com":
        sys.exit("Configure host/user in ~/.dj-crates-tools/pool.json (--init).")

    if cfg.get("tls", True):
        ftp = ftplib.FTP_TLS(context=ssl.create_default_context())
        ftp.connect(host, port, timeout=30)
        ftp.login(user, password)
        ftp.prot_p()  # encrypt the data channel too
    else:
        ftp = ftplib.FTP()
        ftp.connect(host, port, timeout=30)
        ftp.login(user, password)
    ftp.set_pasv(cfg.get("passive", True))
    return ftp


def walk_remote(ftp, root):
    """Yield (path, size) for every file under root. Prefers MLSD; falls back
    to NLST + SIZE for servers that don't support MLSD."""
    try:
        entries = list(ftp.mlsd(root))
        have_mlsd = True
    except (ftplib.error_perm, ftplib.error_proto):
        have_mlsd = False
        entries = []

    if have_mlsd:
        for name, facts in entries:
            if name in (".", ".."):
                continue
            child = f"{root.rstrip('/')}/{name}"
            typ = facts.get("type", "")
            if typ == "dir":
                yield from walk_remote(ftp, child)
            elif typ == "file":
                yield child, int(facts.get("size", 0) or 0)
        return

    # Fallback: NLST then probe each entry.
    try:
        names = ftp.nlst(root)
    except ftplib.error_perm:
        return
    for entry in names:
        child = entry if entry.startswith("/") else f"{root.rstrip('/')}/{entry}"
        if child.rstrip("/") == root.rstrip("/"):
            continue
        size = None
        try:
            size = ftp.size(child)  # fails on directories
        except (ftplib.error_perm, ftplib.error_proto, OSError):
            size = None
        if size is None:
            yield from walk_remote(ftp, child)  # treat as directory
        else:
            yield child, int(size)


# ── filtering ────────────────────────────────────────────────────────────────

def passes_filters(name, size, f):
    ext = os.path.splitext(name)[1].lower()
    allowed = [e.lower() for e in f.get("extensions", list(AUDIO_EXT))]
    if ext not in allowed:
        return False
    lower = name.lower()
    inc = [s.lower() for s in f.get("include_any", []) if s]
    if inc and not any(s in lower for s in inc):
        return False
    exc = [s.lower() for s in f.get("exclude_any", []) if s]
    if any(s in lower for s in exc):
        return False
    if size and size < int(f.get("min_bytes", 0) or 0):
        return False
    mx = int(f.get("max_bytes", 0) or 0)
    if mx and size and size > mx:
        return False
    return True


# ── main ─────────────────────────────────────────────────────────────────────

def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--crates-root")
    ap.add_argument("--init", action="store_true")
    ap.add_argument("--execute", action="store_true", help="actually download (default: dry-run)")
    ap.add_argument("--all", action="store_true", help="consider all remote files, not just new")
    ap.add_argument("--no-classify", action="store_true", help="download only; skip auto-filing")
    ap.add_argument("--limit", type=int, default=0, help="cap downloads this run (0 = no cap)")
    args = ap.parse_args()

    if args.init:
        write_config_template()
        return
    if not args.crates_root:
        sys.exit("--crates-root is required")

    cfg = load_json(CONFIG_PATH, None)
    if cfg is None:
        sys.exit(f"no config — run: pool_sync.py --init  ({CONFIG_PATH})")

    dest_dir = Path(args.crates_root) / "GENRES" / "UNASSIGNED" / "_pool-intake"
    seen = load_json(SEEN_PATH, {})
    filters = cfg.get("filters", {})

    ftp = connect(cfg)
    try:
        remote = []
        for root in cfg.get("remote_dirs", ["/"]):
            remote.extend(walk_remote(ftp, root))
        # Dedup by path (a file can surface once).
        remote = {p: s for p, s in remote}.items()

        new_files = []
        for path, size in remote:
            key = path
            prev = seen.get(key)
            is_new = args.all or prev is None or prev.get("size") != size
            if is_new:
                new_files.append((path, size))

        matched = [(p, s) for p, s in new_files if passes_filters(os.path.basename(p), s, filters)]
        matched.sort()
        if args.limit:
            matched = matched[: args.limit]

        print(f"remote files: {len(list(remote))} · new: {len(new_files)} · matched filters: {len(matched)}")
        for p, s in matched[:40]:
            print(f"  {'DL' if args.execute else '—'} {p}  ({s/1e6:.1f} MB)")
        if len(matched) > 40:
            print(f"  … and {len(matched) - 40} more")

        downloaded = failed = 0
        if args.execute and matched:
            dest_dir.mkdir(parents=True, exist_ok=True)
            LOG_PATH.parent.mkdir(parents=True, exist_ok=True)
            with open(LOG_PATH, "a") as log:
                for path, size in matched:
                    local = dest_dir / os.path.basename(path)
                    tmp = local.with_suffix(local.suffix + ".part")
                    try:
                        with open(tmp, "wb") as fh:
                            ftp.retrbinary(f"RETR {path}", fh.write)
                        tmp.rename(local)
                        seen[path] = {"size": size}
                        log.write(f"{path}\t{local}\n")
                        downloaded += 1
                    except Exception as e:  # noqa: BLE001 — keep going past one bad file
                        failed += 1
                        if tmp.exists():
                            tmp.unlink()
                        print(f"  FAILED {path}: {e}")
            SEEN_PATH.write_text(json.dumps(seen, indent=0))
    finally:
        try:
            ftp.quit()
        except Exception:  # noqa: BLE001
            ftp.close()

    # Auto-file what we pulled, using the shared classifier.
    filed = 0
    if args.execute and downloaded and not args.no_classify:
        script = Path(__file__).with_name("classify_unassigned.py")
        try:
            out = subprocess.run(
                [sys.executable, str(script), "--crates-root", args.crates_root, "--execute"],
                capture_output=True, text=True, timeout=1800,
            )
            for line in out.stdout.splitlines():
                if line.startswith("RESULT:"):
                    for part in line[7:].split(","):
                        if part.startswith("moved="):
                            filed = int(part.split("=")[1])
            print(f"classifier: {out.stdout.strip().splitlines()[-1] if out.stdout.strip() else 'no output'}")
        except Exception as e:  # noqa: BLE001
            print(f"classify step failed: {e}")

    mode = "" if args.execute else " (dry-run — pass --execute)"
    print(f"RESULT:new={len(new_files)},matched={len(matched)},downloaded={downloaded},failed={failed},filed={filed}{mode}")


if __name__ == "__main__":
    main()
