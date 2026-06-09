# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Amanuensis is a cross-platform Clan Lord log analysis tool, similar to `Scribius.app`. It parses text log files from **Clan Lord** and tracks player statistics including kills, trainer ranks, pets, and character data.

## Original Application Analysis

The original app is a native macOS Cocoa application (Objective-C, built with Xcode 15.4, macOS SDK 14.5). Key details:

- **Bundle ID**: `com.dfsw.Scribius` (version 0.4.2, build 42)
- **Architecture**: Universal binary (x86_64 + arm64)
- **Data layer**: Core Data with `NSPersistentContainer`, model entities:
  - `ModelCharacters` — player characters
  - `ModelKills` — creature kill records
  - `ModelLastys` — "lasty" tracking (last creature encounters)
  - `ModelLogs` — log file metadata
  - `ModelPets` — pet information
  - `ModelTrainers` — trainer rank progression
- **Frameworks**: Sparkle (auto-update), standard Cocoa
- **UI**: NIB-based (`MainMenu.nib`), uses `NSTableView` for kills/ranks/coins

## Bundled Data Files

- `creatures.csv` — Creature name-to-value mapping (CSV: `name,value`), ~858 entries
- `trainers.plist` — Maps trainer completion messages (¥-prefixed strings from logs) to trainer names

## Key Functional Areas

1. **Log scanning/parsing**: Reads Clan Lord text log files from a user-selected folder. Scans line-by-line using `NSScanner`. Tracks which logs have already been read.
2. **Character management**: Detects characters, professions, and levels from log content. Supports multiple characters.
3. **Kill tracking**: Parses kill events, maps creatures to values using `creatures.csv`, calculates statistics (highest kill, nemesis, etc.)
4. **Trainer tracking**: Detects trainer messages in logs, maps to trainer names via `trainers.plist`, tracks ranks (effective ranks, modified ranks, bulk ranks).
5. **Lasty tracking**: Tracks "lasty" events (last encounter data from reflect messages).
6. **Pet detection**: Parses pet information from logs.
7. **Coin/economy tracking**: Casino wins/losses, esteem, darkstone, chain breaks, bell usage.
8. **Bestiary surface**: clicking a row in KillsView opens a modal with the full creature record (family, rarity, location, attack/defense/damage/health with measured indicators, frames-per-swing, difficulty, luck-hits, seasonal). SummaryView shows a "Bestiary completion" card (`X / 969 encountered`) with a per-family table sorted by % complete, plus a "Bestiary breakdown" with per-family and per-rarity kill totals. KillsView has chip filters for family / rarity / seasonal. CLI `kills` supports `--family`, `--rarity`, `--seasonal` flags. The frontend bestiary data is loaded once at app boot via the `get_bestiary` Tauri command and cached in Zustand; sprites live in `crates/amanuensis-gui/ui/public/bestiary/`.
9. **Kill frequency**: the `kill_hourly` table holds one row per (character, creature, hour) with the 8 kill-verb count columns (killed/slaughtered/vanquished/dispatched + assisted variants), upsert-incremented during scan exactly like the aggregated `kills` table (which is unchanged). This hourly summary is ~10× smaller than a per-event table for heavy users (common grind monsters collapse most). Per creature, two max-ever metrics are derived in `crates/amanuensis-core/src/db/queries/frequency.rs` (the single source of truth shared by GUI and CLI): **best calendar day** (highest kills in any calendar day — exact, sum of that day's hour buckets) and **best 2 hours** (highest kills in any 2h *sliding* window — a two-pointer sweep over adjacent hour buckets, i.e. the densest pair of consecutive clock-hours; far better than fixed midnight-aligned bins, with the only precision loss being sub-hour bursts straddling three clock-hours — the GM/invasion-spawn outliers the bestiary author filters anyway). Surfaced as **Best Day / Best 2h** columns in KillsView (Tauri `get_kill_frequency`) and via the CLI `amanuensis frequency <char> [--bin day|2h|both] [--solo] [--by-verb] [--format table|csv|json] [--limit N]`. Both surfaces include assisted kills by default (`--solo` for solo-only). Origin: requested by the upstream bestiary-data author for quantitative spawn-frequency collation. **Databases scanned before this feature need a full re-scan to backfill `kill_hourly`.** Use the GUI's **Rescan Logs** action, or the CLI `amanuensis rescan <folder...>` — both call `reset_log_data` first then re-scan, so they repopulate `kill_hourly` without double-counting (pass ALL your log folders to `rescan`, since the reset wipes derived data first). Do **not** use `amanuensis scan --force <folder>` for backfill: `--force` only bypasses the already-scanned skip-guards and does not reset first, so re-scanning an already-scanned folder double-counts both `kill_hourly` and the aggregated `kills` totals (a pre-existing `--force` behavior).

## Updated Data Sources

These files can be used to update the bundled Amanuensis data:

- **Bestiary**: `amanuensis update-bestiary <xml-path>` regenerates `crates/amanuensis-core/data/bestiary.json` from the upstream `clnet_bestiary` phpMyAdmin XML dump (e.g. `bestiary_YYYYMMDD_fullexport.xml`). The companion `bestiary_aliases.json` holds hand-curated log-name → bestiary-name mappings (e.g. `the Ramandu` → `the Ramandu (boss)` for the 2620 boss value, `Ramandu` → `the Ramandu` for the 666 clone). Use `amanuensis bestiary <name>` to inspect a single record. **After updating, existing databases should run `amanuensis scan --force <folder>` to refresh stored `creature_value` rows from the new bestiary.**
- **Bestiary sprites**: `python3 tools/sync-bestiary-sprites.py` downloads each entry's `static_pic` from `https://bestiary.clanlord.net/images/creatures_static/<family>/<file>` into `crates/amanuensis-gui/ui/public/bestiary/`, writing only missing/changed files. Falls back to searching other family folders (extinct creatures' sprites live under their original family, not an "Extinct" one). Re-run after `update-bestiary` to pick up new/changed icons. A few extinct creatures (e.g. Captain of the Guard, Deadly Poppy, Tangleweed) have no sprite hosted anywhere; the UI renders no image for these rather than a broken-image glyph.
- **Rank messages**: https://raw.githubusercontent.com/maxtraxv3/Macros/refs/heads/main/clanlord%20apps/RankCounter/RankCounter27/rankmessages.txt
- **Trainer list**: https://raw.githubusercontent.com/maxtraxv3/Macros/refs/heads/main/clanlord%20apps/RankCounter/RankCounter27/trainers.txt
- **Special phrases**: https://raw.githubusercontent.com/maxtraxv3/Macros/refs/heads/main/clanlord%20apps/RankCounter/RankCounter27/specialphrases.txt

## Log Format Details

Discovered through real-data comparison testing against Scribius with 1,160 log files across 10 characters:

### Death/Fallen patterns
- `"X has fallen to a/an Y."` — standard death with article
- `"X has fallen to Y."` — death without article (e.g., "freezing ice barrier", "Coldy Fleas", "excessive drink", "Romanus")
- `"X has fallen."` — NOT a death; this is a login-while-fallen status message that appears when reconnecting while dead. Do not count these.
- Only count deaths where X matches the current character name (other players' deaths appear in logs too)

### Login counting
- Scribius counts each log file as exactly 1 login (742 files = 742 logins)
- Not all files contain "Welcome to Clan Lord" or "Welcome back" messages (mid-session starts, reconnects)
- Every scanned file should increment logins by exactly 1, regardless of welcome message presence

### Loot patterns
- Shared loot: `"* X recovers the {item} fur/blood/mandibles, worth Nc. Your share is Mc."`
- Solo recovery: `"* You recover the {item} fur/blood/mandibles, worth Nc."` (no "Your share" suffix — full worth goes to player)
- Mandibles are always plural ("mandibles") in real logs, never singular "mandible"

### Scribius import
- CLI: `amanuensis import <source.sqlite> [--output path] [--force]`
- GUI: `import_scribius_db` Tauri command
- Imports from Scribius Core Data `Model.sqlite` (at `~/Library/Application Support/Scribius/Model.sqlite`)

## Testing

- 173 unit tests in `amanuensis-core`
- 23 real data comparison tests (require local log files, run with `--ignored`):
  ```
  cargo test -p amanuensis-core --test real_data_comparison -- --ignored
  ```
- Tests cover: Gandor (742 logs), Helga (120), Squib (134), Da Bomba (71), Zephyr (65)
- Cross-source tests verify import-vs-scan agreement on logins, deaths, karma, chains

## Porting Notes

- The new implementation should be cross-platform (macOS, Linux, Windows)
- Must maintain backwards compatibility with the same Clan Lord text log format
- The `creatures.csv` and `trainers.plist` data should be preserved and reused
- Log messages use `¥` as a prefix character for system/trainer messages
