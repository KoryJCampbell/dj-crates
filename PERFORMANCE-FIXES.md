# dj-crates-tools — Performance + Bug Punch List

Investigation date: 2026-04-28

## Context

The `--headless pipeline` run on a ~30K-track library takes **~4.5h** end-to-end. Daily cron split into:
- `dj-daily` (sync + sanitize, fast — minutes)
- `dj-weekly` (fix-bpms + enrich-popularity + downbeat-cue, ~3-4h, Sunday 2pm)
- `label-moods` is **disabled in cron** until issue #1 below is fixed.

The split buys time but the underlying issues are still real.

---

## #1 — `label-moods` UTF-8 char-boundary panic

### Symptom
Running `label-moods` repeatedly panics on tracks with non-ASCII artist names:

```
thread '<unnamed>' panicked at library/core/src/str/mod.rs:833:21:
byte index 30 is not a char boundary; it is inside '\u{82}' (bytes 29..31)
of `The MarÃÂÃÂÃÂÃÂ­as`
```

The string `MarÃÂÃÂÃÂÃÂ­as` is "Marías" double-encoded as UTF-8 → Latin-1 → UTF-8 (mojibake). It IS valid UTF-8 (multi-byte), so a literal `&s[..30]` slice can land inside a multi-byte char and panic.

### Impact
- `read_bpm_artist_title` and `write_single_tag` already wrap in `catch_unwind` (tagging.rs:201, 232), so the stage doesn't crash entirely.
- BUT: tracks with mojibake artist names **silently lose their Spotify popularity lookup** in `label_moods` (returns default `("","")` artist/title → no `popularity.get(&spotify_key(...))` hit → no Sing Along label).
- Default Rust panic handler still prints the panic message to stderr for every offender = log noise.

### Diagnosis steps

1. Re-run with backtrace to find the actual call site:
   ```bash
   RUST_BACKTRACE=1 /Applications/DJ\ Crates.app/Contents/MacOS/dj-crates-tools \
     --headless label-moods 2>&1 | head -200
   ```
2. The literal byte index (30) and the panic frame (`str/mod.rs:833`) point to a `<str as Index<Range<usize>>>::index` call — find it in the backtrace. Likely culprits:
   - `lofty` (tag reader) — check version in `Cargo.toml`, may be fixed in newer release
   - `id3` (transitive) — same
   - One of your own helpers that does `&artist[..N]` or `.get(..N)` for fixed-length truncation

### Fix options (pick one, don't ship all)

**A. If the panic is in your code:** swap byte slicing for char-aware truncation:
```rust
// Bad
let short = &name[..30];

// Good — take up to 30 chars, not 30 bytes
let short: String = name.chars().take(30).collect();

// Or — slice at the nearest char boundary <= 30 bytes
let cutoff = (0..=30).rev().find(|i| name.is_char_boundary(*i)).unwrap_or(0);
let short = &name[..cutoff];
```

**B. If the panic is in `lofty` or another dep:** check the project's CHANGELOG for char-boundary fixes; bump version. If unfixed, file an issue with this repro string: `"The MarÃÂÃÂÃÂÃÂ­as"`.

**C. Suppress noise even if root cause is in a dep:** install a panic hook at the top of `headless::run()` that logs once per file instead of letting Rust dump the default formatted panic to stderr:
```rust
std::panic::set_hook(Box::new(|info| {
    eprintln!("[panic suppressed] {}", info);
}));
```

### Verify
After fix, re-enable `label-moods` in `~/.local/bin/dj-weekly` (currently commented out) and run:
```bash
RUST_BACKTRACE=0 dj-crates-tools --headless label-moods 2>&1 | grep -c "panicked"
# Expected: 0
```

---

## #2 — Mojibake in `library.json` upstream

### Symptom
`spotify_artist_genres.json` and tag fields contain double-encoded names: `MarÃÂÃÂÃÂÃÂ­as`, `Lolo ZouaÃÂÃÂÃÂÃÂ¯` (should be "Marías", "Lolo Zouaï").

### Where it likely originates
- `crateup` shell pipeline → deemix downloads + tags via Python; if any step writes Latin-1-decoded bytes back as UTF-8 you get mojibake
- `sync_to_serato` reads ID3 tags via `lofty` — if the file was tagged with mojibake, lofty surfaces the mojibake faithfully
- `sanitize_library` (tagging.rs:773) reads + rewrites tags but doesn't currently detect-and-repair mojibake

### Fix: add a mojibake repair pass to `sanitize_library`

The signature is `MarÃÂÃÂÃÂÃÂ­as` = `Marías` re-encoded N times. Detection heuristic:

```rust
fn looks_mojibake(s: &str) -> bool {
    // Mojibake of UTF-8 read as Latin-1 produces lots of Ã, Â, Ÿ chars
    s.chars().filter(|c| matches!(*c, 'Ã' | 'Â' | 'Å' | 'Ÿ')).count() >= 2
}

fn try_demojibake(s: &str) -> Option<String> {
    // Reinterpret the UTF-8 bytes as Latin-1, then re-decode as UTF-8.
    // Repeat up to 4 times since some files are double/triple encoded.
    let mut current = s.to_string();
    for _ in 0..4 {
        let bytes: Vec<u8> = current.chars().filter_map(|c| {
            if (c as u32) < 256 { Some(c as u8) } else { None }
        }).collect();
        if bytes.len() != current.chars().count() { return None; } // not all latin-1
        match std::str::from_utf8(&bytes) {
            Ok(decoded) if !looks_mojibake(decoded) => return Some(decoded.to_string()),
            Ok(decoded) => current = decoded.to_string(),
            Err(_) => return None,
        }
    }
    None
}
```

Apply in `sanitize_library`'s per-file loop before writing the artist/title back. Bonus: after fix, also clear `~/.dj-crates-tools/spotify_cache.json` and `spotify_artist_genres.json` so the bad cached keys don't poison future Spotify lookups.

---

## #3 — Stages walk full library every run (no incremental mode)

### Symptom
`fix-bpms` and `downbeat-cue` are DSP-heavy (full audio decode + analysis per file). On 30K tracks at ~0.5-1 sec/track, that's 4-8 hours of CPU. Today, they re-process every file every run, even if 99% haven't changed.

### Impact
- Daily run took 4.5h before the cron split. Even weekly is a full CPU pin.
- Most of the work is wasted — only ~125 new tracks/day on average; the other 29,875 already have valid BPM and cues.

### Fix: add `--only-new` / `--since=<date>` flags to headless subcommands

In `headless.rs` (the subcommand dispatcher around line 79), parse trailing flags into an `IncrementalMode` enum:
```rust
enum IncrementalMode {
    All,
    OnlyNew,             // file mtime newer than last successful run
    Since(SystemTime),   // file mtime newer than this timestamp
}
```

Skip files in each stage's `WalkDir` iterator filter:
```rust
.filter(|e| {
    if let IncrementalMode::OnlyNew = mode {
        // Compare e.metadata()?.modified()? to last_run_timestamp
    }
    e.file_type().is_file() && is_audio(e.path())
})
```

Persist `last_run_timestamp` per stage in `~/.dj-crates-tools/<stage>_last_run.json` after a successful pass. Then update the cron scripts:

```zsh
# dj-weekly
open -gW "/Applications/DJ Crates.app" --args --headless fix-bpms --only-new
open -gW "/Applications/DJ Crates.app" --args --headless downbeat-cue --only-new
```

A weekly `--only-new` run on ~875 new tracks (125/day × 7) would drop from 4-5h to **<10 minutes**.

Keep a manual full-rebuild path:
```zsh
dj-crates-tools --headless fix-bpms --since=1970-01-01
```

### Sanity check before shipping
The DSP outputs (BPM, downbeat cue) need to be persistent in the file's tags or a sidecar so "already analyzed" is detectable. Verify:
- `fix-bpms` writes BPM to ID3 `TBPM` (or equivalent)
- `downbeat-cue` writes cue points to Serato's `database V2`

If yes, the "needs reprocessing" check is `tag.bpm.is_none() || tag.bpm == 0.0` (or equivalent) — even simpler than mtime tracking.

---

## #4 — `sync` is non-atomic — interrupted runs leave Subcrates wiped

### Symptom
`headless.rs:114-128` calls `sync_to_serato(..., clean: true, ...)`. The `clean: true` flag wipes `~/Music/_Serato_/Subcrates/` at the **start** of sync, then rebuilds. If the process is killed (Ctrl-C, power loss, OS kill, kill -9, app crash) between the wipe and the full rebuild, the user loses all crates until the next successful run.

This actually happened on 2026-04-28 — investigative probes of the binary killed `sync` ~2 seconds in, leaving Subcrates with 1 partial crate file. Recovery required restoring from `Export Backups/backup-20260428_210505-0400.zip`.

### Fix: atomic rebuild

In `sync_to_serato`, write the new crate files to `Subcrates.tmp/` (or `Subcrates.new-<pid>/`), then atomically swap once complete:

```rust
// Pseudocode
let tmp = subcrates_dir.with_extension("tmp");
fs::create_dir_all(&tmp)?;
write_all_crates_to(&tmp, ...)?;            // populate fresh
let bak = subcrates_dir.with_extension("bak");
let _ = fs::remove_dir_all(&bak);           // clear old .bak
let _ = fs::rename(&subcrates_dir, &bak);   // demote current → .bak
fs::rename(&tmp, &subcrates_dir)?;          // promote new → live
let _ = fs::remove_dir_all(&bak);           // clean up
```

If the process dies anywhere before the final rename, the original `Subcrates/` is still intact.

### Verify
- Run sync, hit Ctrl-C 2 seconds in. Subcrates should be unchanged.
- Run sync to completion. Subcrates should reflect current folder structure.

---

## #5 — `enrich-popularity` rate-limit handling

### Symptom
Spotify search API caps at ~180 req/min for client_credentials. With 30K tracks, even cached, a fresh run can hit the wall. Current code (`lib.rs:1079`) sleeps 5 sec on 429 then returns Err — caller probably retries the whole thing.

### Fix
- Honor the `Retry-After` header on 429
- Use a token-bucket rate limiter (e.g., `governor` crate) at ~150 req/min to stay under the cap
- Always batch through the cache first, only API on cache miss

Already mostly done via `spotify_cache.json` — but worth verifying the cache hit rate is >95% on a steady-state run.

---

## Verification plan after fixes

1. Pick one bad-mojibake track (e.g., a Marías song). Confirm `sanitize` repairs the artist tag in place.
2. Run `label-moods` end-to-end. Confirm:
   - 0 panic messages in stderr
   - `report.labeled` count went up vs. last successful run
3. Run `fix-bpms --only-new` after a fresh `crateup`. Confirm runtime drops from ~hours to ~minutes.
4. Re-enable `label-moods` in `~/.local/bin/dj-weekly`.
5. Time a full `dj-weekly` run. Target: under 30 minutes on a typical week.

---

## Cron status

After today's split (2026-04-28):

| Cron | Schedule | Stages | ETA |
|---|---|---|---|
| `dj-daily` | every day 10:00 | crateadd → crateup → build-edits → sync → sanitize | ~5 min + crateup time |
| `dj-weekly` | Sundays 14:00 | fix-bpms → enrich-popularity → downbeat-cue | ~3-4h |
| (disabled) | — | label-moods | until #1 fixed |
