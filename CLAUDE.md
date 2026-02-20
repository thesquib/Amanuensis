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

## Updated Data Sources

These files can be used to update the bundled Amanuensis data:

- **Bestiary**: `Bestiary_2026_Feb_Dump.xlsx` (in project root) — updated creature list to replace `creatures.csv`
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
- Tests cover: Ruuk (742 logs), Olga (120), Squib (134), Tu Whawha (71), Tane (65)
- Cross-source tests verify import-vs-scan agreement on logins, deaths, karma, chains

## Porting Notes

- The new implementation should be cross-platform (macOS, Linux, Windows)
- Must maintain backwards compatibility with the same Clan Lord text log format
- The `creatures.csv` and `trainers.plist` data should be preserved and reused
- Log messages use `¥` as a prefix character for system/trainer messages
