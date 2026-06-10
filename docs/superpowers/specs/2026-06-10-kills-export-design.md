# Export Kills from the GUI — Design Spec

**Date:** 2026-06-10
**Status:** Design approved (pending spec review)

## Motivation

GUI users want to get the Kills view data out of the app — into a spreadsheet
(CSV) or a readable text block (ASCII table) to share or analyse. Today only the
CLI can emit this kind of output, and even there only `frequency` does CSV; the
`kills` table is ASCII-only. The export logic should be written **once** and
shared, so the GUI and CLI never drift. A GUI user should never need to know the
CLI exists.

## Decisions (locked)

- **Scope:** one **unified** export = exactly the Kills view table (kill counts +
  value + dates + Best Day + Best 2h), not separate kills/frequency exports.
- **Fidelity:** **full table, core-formatted.** Every creature for the character,
  sorted like the view's default (total kills descending). On-screen chip filters
  / search / sort are **not** applied — the file is the whole table.
- **Backend:** shared formatter in **`amanuensis-core`** (a new `export` module),
  called by both the GUI Tauri command and the CLI. No subprocess, no duplication.
- **Verb columns:** **combined totals** (solo + assisted per verb), matching the
  on-screen numbers (WYSIWYG). No solo/assisted split.
- **UI:** an in-view **`Export ▾`** dropdown in the Kills header (CSV / Plain
  text), not a native OS menu (the app has none today).

## Architecture

### Core: `amanuensis-core` export module
New file `crates/amanuensis-core/src/export.rs` plus a `Database` method:

```rust
pub enum ExportFormat { Csv, Text }

impl Database {
    /// Render the unified Kills table for a (possibly merged) character to a
    /// string. Fetches merged kills + frequency, joins by creature name, formats.
    pub fn export_kills_merged(&self, char_id: i64, format: ExportFormat) -> Result<String>;
}
```

- Internally calls the existing `get_kills_merged(char_id)` and
  `kill_frequency_merged_with(char_id, true)`, joins the frequency rows onto the
  kills by `creature_name` (HashMap lookup), and sorts by total kills desc.
- Pure string output; the caller decides what to do with it (write file / print).
- `comfy_table` is added to `amanuensis-core`'s dependencies for the ASCII render
  (same library and visual style the CLI already uses). This puts presentation
  code in the core crate — an accepted, deliberate trade for a single shared
  implementation; a separate `amanuensis-export` crate was considered and rejected
  as over-scaffolding for one feature.

### Column set (view order)

| Column | Source |
|---|---|
| Creature | `creature_name` |
| Vanquished | `vanquished_count + assisted_vanquish_count` |
| Killed | `killed_count + assisted_kill_count` |
| Dispatched | `dispatched_count + assisted_dispatch_count` |
| Slaughtered | `slaughtered_count + assisted_slaughter_count` |
| Killed By | `killed_by_count` |
| Value | `creature_value` |
| First Kill | `date_first` (date portion, as shown in the view) |
| Last Kill | `date_last` (date portion) |
| Best Day | `best_day_count` (blank/`-` if none) |
| Best Day (date) | `best_day_date` |
| Best 2h | `best_2h_count` |
| Best 2h (window) | 2-hour window string from `best_2h_start`, e.g. `2024-01-02 08:00–10:00` (start hour + 2h, wrapping midnight) — mirrors the GUI tooltip |

- **CSV:** comma-separated, header row, quote any cell containing comma / quote /
  space, escape inner quotes by doubling (`"` → `""`). Reuse the exact quoting
  rule already in the CLI's `cmd_frequency`.
- **Text:** `comfy_table` boxed table (UTF8_FULL + round corners), same as the CLI.
- Creatures with no frequency data render blank in the Best Day / Best 2h columns.

### GUI

- **`crates/amanuensis-gui/src/commands/`**: new command
  ```rust
  #[tauri::command]
  pub fn export_kills(char_id: i64, format: String, path: String, state) -> Result<(), String>;
  ```
  `format` is `"csv"` | `"text"`; it calls `db.export_kills_merged(...)` and writes
  the string to `path`. Registered in `generate_handler!`.
- **Frontend:** an `Export ▾` dropdown in the KillsView header (next to the search
  /filters). Choosing CSV or Plain text:
  1. calls `tauri-plugin-dialog` `save()` (already used for Scribius import) with a
     friendly default name — `<Character>-kills.csv` / `.txt` — and the matching
     file-type filter;
  2. on a chosen path, invokes `export_kills(charId, format, path)`;
  3. shows a success toast (or error), following the import flow's pattern.
- Disabled / hidden when no character is selected.

### CLI

- Extend `cmd_kills`: add `--format table|csv` (default `table`, unchanged
  behaviour). `--format csv` renders via the **same** core formatter, so the CLI
  gains the unified CSV (now including the frequency columns) for free. The
  existing `--sort/--limit/--family/--rarity/--seasonal` flags continue to apply to
  the table path; for v1, `--format csv` exports the full unified table (filters
  still narrow the row set before formatting, consistent with how `kills` already
  filters).

## Error handling

- Core: propagate `Result` (DB errors). Writing is the caller's job.
- GUI command: map core/IO errors to `String`; the frontend surfaces a toast. A
  cancelled save dialog (no path) is a no-op, not an error.

## Testing

- **Core (`export.rs` unit tests):**
  - A fixture character with a couple of creatures (one with frequency, one
    without) → assert the **exact CSV string** (header + rows, including quoting of
    a creature name containing a space, and blank frequency cells).
  - ASCII render smoke test: contains the expected column headers and a known
    creature's combined totals.
  - Combined-total correctness: solo + assisted summed per verb.
- **CLI:** `kills --format csv` produces the same header as the core CSV (a thin
  assertion that the wiring is in place).
- **GUI command:** light — it's a thin wrapper over the core fn + a file write;
  one test that it writes a non-empty file for a known DB.

## Out of scope (v1)

- Exporting other views (Trainers, Coins, etc.) — this is Kills-only. The core
  `export` module is structured so similar exporters can be added later, but we
  build only Kills now.
- Respecting on-screen filters/search/sort in the file (decided: full table).
- Solo/assisted split columns.
- A native OS menu bar.
