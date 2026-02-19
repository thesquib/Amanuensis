# Amanuensis

A cross-platform [Clan Lord](https://www.deltatao.com/clanlord/) log analyzer. Parses text log files and tracks kills, trainer ranks, professions, coins, pets, lastys, equipment, karma, and more.

Built in Rust with a Tauri + React desktop GUI and a standalone CLI. Runs on macOS (Apple Silicon + Intel back to High Sierra), Linux, and Windows.

> **Alpha** - works well but needs more logs thrown at it. Bug reports and log donations welcome.

## Features

- **Kill tracking** - solo/assisted counts per creature, kill verbs, creature values from the Bestiary, nemesis, highest kill
- **Trainer ranks** - parsed from rank-up messages, grouped by profession, editable baselines for pre-log ranks
- **Profession detection** - automatically determined from trainer history (Fighter, Healer, Mystic, Ranger, Bloodmage, Champion)
- **Coin tracking** - picked up, fur/blood/mandible shares with loot worth, chest deposits
- **Lastys** - befriend, morph, and movements progress with completion tracking
- **Pets** - detected from befriend messages
- **Equipment** - bells, chains, shieldstones, ethereal portal stones (used + broken)
- **Karma & esteem** - good/bad karma, esteem gains
- **Multi-character** - detects characters from welcome messages, tracks each independently
- **Dedup** - content-hash based, safe to re-scan overlapping log folders
- **Encoding** - handles Windows-1252/ISO-8859-1 log files (the `0xA5 = ¥` prefix)

## Download

Grab the latest build from [GitHub Actions](https://github.com/thesquib/Amanuensis/actions) (artifacts attached to each run) or from [Releases](https://github.com/thesquib/Amanuensis/releases) once tagged.

| Platform | File |
|----------|------|
| macOS (Apple Silicon) | `Amanuensis_aarch64.dmg` |
| macOS (Intel, 10.13+) | `Amanuensis_x64.dmg` |
| Linux | `.deb` or `.AppImage` |
| Windows | `.msi`, `.exe` installer, or portable `.zip` |

## Usage - GUI

1. Launch Amanuensis
2. Click **Open Database** (or it auto-opens the last one)
3. Click **Scan Logs** and select your Clan Lord `Text Logs` folder
4. Browse characters and stats via the sidebar

The GUI auto-detects character subdirectories and handles recursive folder discovery.

## Usage - CLI

The CLI is a standalone binary with no GUI dependencies.

```
amanuensis [--db path.db] <command>
```

### Scan logs

```sh
# Scan a log folder (expects character subdirectories with "CL Log *.txt" files)
amanuensis scan /path/to/Text\ Logs

# Force re-scan of already-read files
amanuensis scan --force /path/to/Text\ Logs

# Use a specific database file
amanuensis --db mydata.db scan /path/to/Text\ Logs
```

### View data

```sh
# List all characters
amanuensis characters

# Character summary
amanuensis summary Ruuk

# Kill table (sortable by: total, solo, assisted, value, name)
amanuensis kills Ruuk --sort value --limit 20

# Trainer ranks
amanuensis trainers Ruuk

# Pets
amanuensis pets Ruuk

# Lastys
amanuensis lastys Ruuk
```

The default database file is `amanuensis.db` in the current directory.

## Building from source

### Prerequisites

- [Rust](https://rustup.rs/) (stable)
- [Node.js](https://nodejs.org/) 20+ and npm (for the GUI frontend)
- Platform dependencies for Tauri v2:
  - **macOS**: Xcode Command Line Tools
  - **Linux**: `sudo apt install libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf`
  - **Windows**: [Microsoft C++ Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/), WebView2 (bundled with Windows 10+)

### Build the CLI only

```sh
cargo build --release -p amanuensis-cli
# Binary at target/release/amanuensis
```

### Build the GUI app

```sh
# Install frontend dependencies
cd crates/amanuensis-gui/ui && npm install && cd -

# Development mode (hot-reload)
cd crates/amanuensis-gui && cargo tauri dev

# Production build
cd crates/amanuensis-gui && cargo tauri build
```

### Run tests

```sh
cargo test
cargo clippy -- -D warnings
```

## Project structure

```
crates/
  amanuensis-core/     # Library: parser, database, models, data files
  amanuensis-cli/      # CLI binary
  amanuensis-gui/      # Tauri v2 desktop app
    ui/                # React + TypeScript + Tailwind frontend
```

## Kudos & inspiration

- Soul Hunter's Scribius
- Gorvin's DPS Calculator
- Maxtraxv3's RankCounter

## License

[MIT](LICENSE) - free forever, do what you want with it.
