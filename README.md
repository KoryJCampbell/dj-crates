# DJ Crates

A lightweight desktop app that syncs your music folder structure into Serato DJ Pro crates — one button, one click.

## What It Does

Point it at your organized music library and it creates matching crates and subcrates inside Serato DJ Pro. Your folder hierarchy becomes your crate hierarchy.

```
CRATES/
├── GENRES/
│   ├── Hip-Hop:Rap/
│   │   ├── Boom Bap/
│   │   │   └── 1990s/
│   │   ├── Drill/
│   │   └── Trap/
│   ├── R&B/
│   ├── House/
│   └── ...
└── PLAYLISTS/
    ├── 90s Hip-Hop & R&B Hits/
    └── 2000s Hip-Hop & R&B Hits/
```

Becomes in Serato:

```
GENRES
├── Hip-Hop:Rap
│   ├── Boom Bap
│   │   └── 1990s
│   ├── Drill
│   └── Trap
├── R&B
├── House
└── ...
PLAYLISTS
├── 90s Hip-Hop & R&B Hits
└── 2000s Hip-Hop & R&B Hits
```

## Features

- **One-button sync** — press the button and your Serato crates match your folders
- **Full rebuild on every sync** — cleans old synced crates and writes fresh ones
- **Safe** — only touches crates prefixed with `GENRES` or `PLAYLISTS`, never deletes crates you created manually in Serato
- **Native Serato format** — writes `.crate` files directly using Serato's TLV binary format (no Serato API needed)
- **Fast** — built with Rust (Tauri) so scanning 18,000+ tracks takes seconds

## Tech Stack

- **Backend**: Rust + [Tauri v2](https://tauri.app/)
- **Frontend**: React + TypeScript + Tailwind CSS v4
- **Icons**: [Lucide](https://lucide.dev/)

## Development

### Prerequisites

- [Rust](https://rustup.rs/) (1.70+)
- [Node.js](https://nodejs.org/) (18+)

### Setup

```bash
git clone https://github.com/KoryJCampbell/dj-crates.git
cd dj-crates
npm install
npm run tauri dev
```

### Build for production

```bash
npm run tauri build
```

This outputs a `.dmg` installer in `src-tauri/target/release/bundle/dmg/`.

## Configuration

Paths are currently set in `src/App.tsx`:

```typescript
cratesRoot: "/Users/koryjcampbell/Music/CRATES"
seratoPath: "/Users/koryjcampbell/Music/_Serato_"
```

Update these to match your setup, or add a settings UI.

## How It Works

1. Recursively scans your `CRATES/` directory for audio files (mp3, flac, wav, aac, etc.)
2. Builds a crate hierarchy using Serato's `%%` subcrate naming convention
3. Writes binary `.crate` files with proper TLV encoding (version header, column definitions, track entries)
4. Every track appears at every level of its hierarchy (genre crate, subgenre crate, decade crate, year crate)

## License

MIT
