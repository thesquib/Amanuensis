# GUI Kills Export Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let a GUI user export the unified Kills table (kill counts + value + dates + Best Day / Best 2h) to a CSV or plain-text file, via an in-view `Export ▾` button, backed by one shared formatter in `amanuensis-core` that the CLI also uses.

**Architecture:** A new `amanuensis-core` `export` module holds a pure `format_kills_export(kills, freq, format) -> String` plus a `Database::export_kills_merged(char_id, format)` wrapper that fetches + joins + sorts. The CLI's `kills --format csv` and a new GUI `export_kills` Tauri command both call core. The frontend adds an `Export ▾` dropdown that runs the existing `save()` dialog and invokes the command.

**Tech Stack:** Rust (`comfy-table` for ASCII, added to core), Tauri (`tauri-plugin-dialog` `save()`/`message()`, already used), React/TS.

**Spec:** `docs/superpowers/specs/2026-06-10-kills-export-design.md`

---

## File Structure

**Create:**
- `crates/amanuensis-core/src/export.rs` — `ExportFormat`, the pure `format_kills_export`, the `Database::export_kills_merged` wrapper, small date/window/CSV-quote helpers, and unit tests. One clear responsibility: render the unified kills table to a string.

**Modify:**
- `crates/amanuensis-core/Cargo.toml` — add `comfy-table` dependency.
- `crates/amanuensis-core/src/lib.rs` — `pub mod export;` + re-export `ExportFormat`.
- `crates/amanuensis-cli/src/main.rs` — `kills` gains `--format table|csv`.
- `crates/amanuensis-gui/src/commands/data.rs` — new `export_kills` command.
- `crates/amanuensis-gui/src/main.rs` — register `export_kills` in `generate_handler!`.
- `crates/amanuensis-gui/ui/src/lib/commands.ts` — `exportKills` wrapper.
- `crates/amanuensis-gui/ui/src/components/views/KillsView.tsx` — `Export ▾` dropdown + save/invoke/message flow.

---

## Task 1: Core — `ExportFormat` + CSV formatter

**Files:**
- Create: `crates/amanuensis-core/src/export.rs`
- Modify: `crates/amanuensis-core/Cargo.toml`, `crates/amanuensis-core/src/lib.rs`

- [ ] **Step 1: Add the `comfy-table` dependency**

In `crates/amanuensis-core/Cargo.toml`, under `[dependencies]`, add `comfy-table` matching the version the CLI uses. Check `crates/amanuensis-cli/Cargo.toml` for the exact version string (the crate is `comfy-table`, imported in Rust as `comfy_table`) and copy it, e.g.:

```toml
comfy-table = "7"
```

- [ ] **Step 2: Register the module**

In `crates/amanuensis-core/src/lib.rs`, add after the other `pub mod` lines:

```rust
pub mod export;
```

and after the other `pub use` lines:

```rust
pub use export::ExportFormat;
```

- [ ] **Step 3: Write the failing CSV test**

Create `crates/amanuensis-core/src/export.rs` with ONLY this content for now (impl added next step):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::queries::CreatureFrequency;
    use crate::models::Kill;
    use std::collections::BTreeMap;

    fn lg_vermine() -> Kill {
        let mut k = Kill::new(0, "Large Vermine".into(), 70);
        k.killed_count = 5;
        k.assisted_kill_count = 2;
        k.slaughtered_count = 3;
        k.killed_by_count = 1;
        k.date_first = Some("2024-01-01 09:00:00".into());
        k.date_last = Some("2024-01-05 12:00:00".into());
        k
    }

    fn rat() -> Kill {
        let mut k = Kill::new(0, "Rat".into(), 2);
        k.killed_count = 8;
        k.date_first = Some("2024-02-01 09:00:00".into());
        k.date_last = Some("2024-02-02 09:00:00".into());
        k
    }

    fn lg_vermine_freq() -> CreatureFrequency {
        CreatureFrequency {
            creature_name: "Large Vermine".into(),
            best_day_count: 4,
            best_day_date: Some("2024-01-03".into()),
            best_day_verbs: BTreeMap::new(),
            best_2h_count: 3,
            best_2h_start: Some("2024-01-03 08:00".into()),
            best_2h_verbs: BTreeMap::new(),
        }
    }

    #[test]
    fn csv_has_header_and_rows_with_combined_totals_and_quoting() {
        let kills = vec![lg_vermine(), rat()];
        let freq = vec![lg_vermine_freq()]; // Rat has no frequency entry

        let out = format_kills_export(&kills, &freq, ExportFormat::Csv);
        let lines: Vec<&str> = out.lines().collect();

        assert_eq!(
            lines[0],
            "Creature,Vanquished,Killed,Dispatched,Slaughtered,Killed By,Value,First Kill,Last Kill,Best Day,Best Day Date,Best 2h,Best 2h Window"
        );
        // Large Vermine: killed 5+2=7, slaughtered 3, value 70, dates date-only,
        // freq best day 4 on 2024-01-03, best 2h 3 in the 08:00-10:00 window.
        // Name and window contain spaces -> quoted.
        assert_eq!(
            lines[1],
            r#""Large Vermine",0,7,0,3,1,70,2024-01-01,2024-01-05,4,2024-01-03,3,"2024-01-03 08:00–10:00""#
        );
        // Rat: killed 8, no frequency -> blank Best Day/2h cells.
        assert_eq!(lines[2], "Rat,0,8,0,0,0,2,2024-02-01,2024-02-02,,,,");
    }
}
```

- [ ] **Step 4: Run the test to verify it fails**

Run: `cargo test -p amanuensis-core --lib export`
Expected: FAIL — `format_kills_export` / `ExportFormat` not found (won't compile).

- [ ] **Step 5: Implement `ExportFormat`, helpers, and the CSV path**

At the TOP of `crates/amanuensis-core/src/export.rs` (above the test module):

```rust
use std::collections::HashMap;

use crate::db::queries::CreatureFrequency;
use crate::models::Kill;

/// Output format for the unified kills export.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    Csv,
    Text,
}

/// Column headers, in the Kills-view order.
const HEADERS: [&str; 13] = [
    "Creature", "Vanquished", "Killed", "Dispatched", "Slaughtered",
    "Killed By", "Value", "First Kill", "Last Kill",
    "Best Day", "Best Day Date", "Best 2h", "Best 2h Window",
];

/// One creature's cells, in HEADERS order. Frequency cells are "" when absent.
fn row_cells(k: &Kill, freq: Option<&CreatureFrequency>) -> Vec<String> {
    let (best_day, best_day_date, best_2h, best_2h_window) = match freq {
        Some(f) if f.best_day_count > 0 || f.best_2h_count > 0 => (
            num_or_blank(f.best_day_count),
            f.best_day_date.clone().unwrap_or_default(),
            num_or_blank(f.best_2h_count),
            f.best_2h_start.as_deref().map(two_hour_window).unwrap_or_default(),
        ),
        _ => (String::new(), String::new(), String::new(), String::new()),
    };
    vec![
        k.creature_name.clone(),
        (k.vanquished_count + k.assisted_vanquish_count).to_string(),
        (k.killed_count + k.assisted_kill_count).to_string(),
        (k.dispatched_count + k.assisted_dispatch_count).to_string(),
        (k.slaughtered_count + k.assisted_slaughter_count).to_string(),
        k.killed_by_count.to_string(),
        k.creature_value.to_string(),
        date_only(k.date_first.as_deref()),
        date_only(k.date_last.as_deref()),
        best_day,
        best_day_date,
        best_2h,
        best_2h_window,
    ]
}

fn num_or_blank(n: i64) -> String {
    if n > 0 { n.to_string() } else { String::new() }
}

/// Date portion of a "YYYY-MM-DD HH:MM:SS" timestamp (matches the GUI's date display).
fn date_only(s: Option<&str>) -> String {
    s.unwrap_or("").split(' ').next().unwrap_or("").to_string()
}

/// "YYYY-MM-DD HH:00" hour-bucket start -> "YYYY-MM-DD HH:00–HH:00" (start + 2h,
/// wrapping past midnight). Mirrors the GUI's formatTwoHourWindow tooltip.
fn two_hour_window(start: &str) -> String {
    let (date, time) = match start.split_once(' ') {
        Some(p) => p,
        None => return start.to_string(),
    };
    let start_hour: i64 = match time.split(':').next().and_then(|h| h.parse().ok()) {
        Some(h) => h,
        None => return start.to_string(),
    };
    let end_hour = (start_hour + 2) % 24;
    format!("{date} {start_hour:02}:00–{end_hour:02}:00")
}

/// Quote a CSV cell when it contains a comma, quote, or space; double inner quotes.
/// (Same rule the CLI's frequency export uses.)
fn csv_cell(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains(' ') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

/// Render the unified kills table to a string. Pure: callers decide ordering and
/// what to do with the result (write file / print). `freq` is joined onto `kills`
/// by creature name; kills are emitted in the order given.
pub fn format_kills_export(
    kills: &[Kill],
    freq: &[CreatureFrequency],
    format: ExportFormat,
) -> String {
    let freq_by_name: HashMap<&str, &CreatureFrequency> =
        freq.iter().map(|f| (f.creature_name.as_str(), f)).collect();

    match format {
        ExportFormat::Csv => {
            let mut out = String::new();
            out.push_str(&HEADERS.join(","));
            out.push('\n');
            for k in kills {
                let cells = row_cells(k, freq_by_name.get(k.creature_name.as_str()).copied());
                let line: Vec<String> = cells.iter().map(|c| csv_cell(c)).collect();
                out.push_str(&line.join(","));
                out.push('\n');
            }
            out
        }
        ExportFormat::Text => format_text(kills, &freq_by_name), // implemented in Task 2
    }
}
```

> Note: `format_text` is referenced but defined in Task 2. To keep Task 1 compiling on its own, add a temporary stub directly below `format_kills_export` and replace it in Task 2:
> ```rust
> fn format_text(_kills: &[Kill], _freq: &HashMap<&str, &CreatureFrequency>) -> String {
>     String::new()
> }
> ```

- [ ] **Step 6: Run the test to verify it passes**

Run: `cargo test -p amanuensis-core --lib export`
Expected: PASS (`csv_has_header_and_rows_with_combined_totals_and_quoting`).

- [ ] **Step 7: Commit**

```bash
git add crates/amanuensis-core/Cargo.toml crates/amanuensis-core/src/lib.rs crates/amanuensis-core/src/export.rs Cargo.lock
git commit -m "feat(core): kills export module with CSV formatter"
```

---

## Task 2: Core — ASCII (Text) formatter

**Files:**
- Modify: `crates/amanuensis-core/src/export.rs`

- [ ] **Step 1: Write the failing ASCII test**

Add to the `tests` module in `crates/amanuensis-core/src/export.rs`:

```rust
    #[test]
    fn text_render_has_headers_and_combined_values() {
        let kills = vec![lg_vermine(), rat()];
        let freq = vec![lg_vermine_freq()];

        let out = format_kills_export(&kills, &freq, ExportFormat::Text);

        // comfy_table draws a boxed table; assert key content is present.
        assert!(out.contains("Creature"));
        assert!(out.contains("Best 2h Window"));
        assert!(out.contains("Large Vermine"));
        // Combined killed total for Large Vermine is 7 (5 solo + 2 assisted).
        assert!(out.contains("7"));
        // The 2h window string is rendered.
        assert!(out.contains("2024-01-03 08:00–10:00"));
        // Rat present with no frequency cells.
        assert!(out.contains("Rat"));
    }
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p amanuensis-core --lib text_render_has_headers_and_combined_values`
Expected: FAIL — `format_text` stub returns empty string, so `contains("Creature")` fails.

- [ ] **Step 3: Implement `format_text`**

Replace the temporary `format_text` stub in `crates/amanuensis-core/src/export.rs` with:

```rust
fn format_text(kills: &[Kill], freq_by_name: &HashMap<&str, &CreatureFrequency>) -> String {
    use comfy_table::modifiers::UTF8_ROUND_CORNERS;
    use comfy_table::presets::UTF8_FULL;
    use comfy_table::{ContentArrangement, Table};

    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .apply_modifier(UTF8_ROUND_CORNERS)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(HEADERS.to_vec());

    for k in kills {
        let cells = row_cells(k, freq_by_name.get(k.creature_name.as_str()).copied());
        table.add_row(cells);
    }

    table.to_string()
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p amanuensis-core --lib export`
Expected: PASS (both export tests).

- [ ] **Step 5: Run clippy and the full core suite**

Run: `cargo clippy -p amanuensis-core --all-targets` (fix any warnings the new file introduces) and `cargo test -p amanuensis-core` (expected: all pass).

- [ ] **Step 6: Commit**

```bash
git add crates/amanuensis-core/src/export.rs
git commit -m "feat(core): ASCII table formatter for kills export"
```

---

## Task 3: Core — `Database::export_kills_merged`

**Files:**
- Modify: `crates/amanuensis-core/src/export.rs`

- [ ] **Step 1: Write the failing test**

Add to the `tests` module in `crates/amanuensis-core/src/export.rs`:

```rust
    #[test]
    fn export_kills_merged_sorts_by_total_and_joins_frequency() {
        use crate::db::queries::Database;

        let db = Database::open_in_memory().unwrap();
        let c = db.get_or_create_character("Tester").unwrap();
        // Rat: 8 kills. Wolf: 12 kills -> Wolf should sort first (total desc).
        // upsert_kill creates the row with count 1; bump to the target via UPDATE.
        db.upsert_kill(c, "Rat", "killed_count", 2, "2024-02-01 09:00:00").unwrap();
        db.conn().execute(
            "UPDATE kills SET killed_count = 8 WHERE creature_name = 'Rat'",
            [],
        ).unwrap();
        db.upsert_kill(c, "Wolf", "killed_count", 50, "2024-01-01 09:00:00").unwrap();
        db.conn().execute(
            "UPDATE kills SET killed_count = 12 WHERE creature_name = 'Wolf'",
            [],
        ).unwrap();

        let csv = db.export_kills_merged(c, ExportFormat::Csv).unwrap();
        let lines: Vec<&str> = csv.lines().collect();
        assert!(lines[0].starts_with("Creature,"));
        // Wolf (12) before Rat (8).
        assert!(lines[1].starts_with("Wolf,"));
        assert!(lines[2].starts_with("Rat,"));
    }
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p amanuensis-core --lib export_kills_merged_sorts_by_total_and_joins_frequency`
Expected: FAIL — no method `export_kills_merged`.

- [ ] **Step 3: Implement the wrapper**

Add to `crates/amanuensis-core/src/export.rs` (above the test module), with the imports it needs:

```rust
use crate::error::Result;
use crate::Database;

impl Database {
    /// Render the unified Kills table for a (possibly merged) character to a string.
    /// Fetches merged kills + frequency, sorts by total kills descending (the Kills
    /// view's default order), joins frequency by creature name, and formats.
    pub fn export_kills_merged(&self, char_id: i64, format: ExportFormat) -> Result<String> {
        let mut kills = self.get_kills_merged(char_id)?;
        kills.sort_by_key(|k| std::cmp::Reverse(k.total_all()));
        let freq = self.kill_frequency_merged_with(char_id, true)?;
        Ok(format_kills_export(&kills, &freq, format))
    }
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p amanuensis-core --lib export`
Expected: PASS (all three export tests).

- [ ] **Step 5: Commit**

```bash
git add crates/amanuensis-core/src/export.rs
git commit -m "feat(core): Database::export_kills_merged"
```

---

## Task 4: CLI — `kills --format table|csv`

**Files:**
- Modify: `crates/amanuensis-cli/src/main.rs`

- [ ] **Step 1: Add the `--format` arg to the `Kills` command**

In `enum Commands`, the `Kills { ... }` variant, add a field (place after `seasonal`):

```rust
        /// Output format: table, csv
        #[arg(long, default_value = "table")]
        format: String,
```

- [ ] **Step 2: Thread it through the dispatch arm**

In `run()`'s `match`, update the `Commands::Kills { .. }` arm to pass `format`:

```rust
        Commands::Kills { name, sort, limit, family, rarity, seasonal, format } => {
            cmd_kills(&db_path, &name, &sort, limit, family, rarity, seasonal, &format)
        }
```

- [ ] **Step 3: Add the parameter and CSV branch to `cmd_kills`**

In `cmd_kills` (`crates/amanuensis-cli/src/main.rs`), add `format: &str` to the signature (last param), and after the filtering + sort + `if kills.is_empty()` block, branch on format BEFORE building the existing `comfy_table`. Insert this just before the existing `let mut table = Table::new();` line:

```rust
    if format == "csv" {
        use amanuensis_core::export::{format_kills_export, ExportFormat};
        let freq = db.kill_frequency_merged_with(char_id, true)?;
        print!("{}", format_kills_export(&kills, &freq, ExportFormat::Csv));
        return Ok(());
    }
```

> The existing default (`table`) path below is unchanged — it keeps the current
> Solo/Assisted/Total ASCII layout. The CSV path reuses the shared core formatter
> (unified columns incl. frequency) and respects the `--sort/--limit/--family/...`
> filters already applied to `kills`.

- [ ] **Step 4: Build and smoke-test**

Run: `cargo build -p amanuensis-cli` (expected: compiles).

Run (against a scanned dev DB; substitute a real character):
```bash
GUIDB="/Users/thesquib/Library/Application Support/com.dfsw.Amanuensis/amanuensis.db"
cargo run -q -p amanuensis-cli -- --db "$GUIDB" kills Ruuk --format csv --limit 5
```
Expected: a CSV with the `Creature,Vanquished,Killed,...,Best 2h Window` header and up to 5 rows. Default `kills Ruuk` (no `--format`) still prints the original table.

- [ ] **Step 5: Commit**

```bash
git add crates/amanuensis-cli/src/main.rs
git commit -m "feat(cli): kills --format csv via shared core exporter"
```

---

## Task 5: GUI — `export_kills` Tauri command

**Files:**
- Modify: `crates/amanuensis-gui/src/commands/data.rs`, `crates/amanuensis-gui/src/main.rs`

- [ ] **Step 1: Add the command**

In `crates/amanuensis-gui/src/commands/data.rs`, add the import near the other `use amanuensis_core::...` lines:

```rust
use amanuensis_core::export::ExportFormat;
```

Then add (mirroring the existing `get_kills` command's `state.with_db` pattern):

```rust
/// Export the unified Kills table for a character to a file at `path`.
/// `format` is "csv" or "text".
#[tauri::command]
pub fn export_kills(
    char_id: i64,
    format: String,
    path: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let fmt = match format.as_str() {
        "csv" => ExportFormat::Csv,
        "text" => ExportFormat::Text,
        other => return Err(format!("Unknown export format: {other}")),
    };
    let contents = state.with_db(|db| {
        db.export_kills_merged(char_id, fmt).map_err(|e| e.to_string())
    })?;
    std::fs::write(&path, contents).map_err(|e| e.to_string())
}
```

> `ExportFormat` must be reachable as `amanuensis_core::export::ExportFormat`. The
> `export` module is `pub mod export;` in core's lib.rs (Task 1), so this path works.

- [ ] **Step 2: Register the command**

In `crates/amanuensis-gui/src/main.rs`, add to the `tauri::generate_handler![...]` list (next to `commands::get_kills`):

```rust
            commands::export_kills,
```

> If `data.rs` items are surfaced via a glob `pub use data::*;` in
> `commands/mod.rs`, nothing else is needed; confirm `commands::export_kills`
> resolves (the `generate_handler!` macro errors at compile time if not).

- [ ] **Step 3: Build**

Run: `cargo check -p amanuensis-gui`
Expected: compiles cleanly.

- [ ] **Step 4: Commit**

```bash
git add crates/amanuensis-gui/src/commands/data.rs crates/amanuensis-gui/src/main.rs
git commit -m "feat(gui): export_kills Tauri command"
```

---

## Task 6: Frontend — `Export ▾` dropdown

**Files:**
- Modify: `crates/amanuensis-gui/ui/src/lib/commands.ts`, `crates/amanuensis-gui/ui/src/components/views/KillsView.tsx`

- [ ] **Step 1: Add the command wrapper**

In `crates/amanuensis-gui/ui/src/lib/commands.ts`, mirror the existing `getKills` wrapper (camelCase invoke args):

```typescript
export async function exportKills(
  charId: number,
  format: "csv" | "text",
  path: string,
): Promise<void> {
  return invoke("export_kills", { charId, format, path });
}
```

- [ ] **Step 2: Wire the dropdown into KillsView**

In `crates/amanuensis-gui/ui/src/components/views/KillsView.tsx`:

a) Add imports at the top (the dialog helpers are the same ones `useDatabase.ts` uses):

```typescript
import { save, message } from "@tauri-apps/plugin-dialog";
import { exportKills } from "../../lib/commands";
```

b) Inside the `KillsView` component, read the active character id and name from the store (the store exposes `characters: Character[]` and `selectedCharacterId`), and add an export handler:

```typescript
const selectedCharacterId = useStore((s) => s.selectedCharacterId);
const characters = useStore((s) => s.characters);

const handleExport = useCallback(
  async (format: "csv" | "text") => {
    if (selectedCharacterId == null) return;
    const charName =
      characters.find((c) => c.id === selectedCharacterId)?.name ?? "character";
    const ext = format === "csv" ? "csv" : "txt";
    const path = await save({
      title: "Export Kills",
      defaultPath: `${charName}-kills.${ext}`,
      filters:
        format === "csv"
          ? [{ name: "CSV", extensions: ["csv"] }]
          : [{ name: "Text", extensions: ["txt"] }],
    });
    if (!path) return; // user cancelled
    try {
      await exportKills(selectedCharacterId, format, path);
      await message(`Exported kills to ${path}.`, { title: "Export Complete" });
    } catch (e) {
      await message(String(e), { title: "Export Failed", kind: "error" });
    }
  },
  [selectedCharacterId, characters],
);
```

> `selectedCharacterId` may already be read elsewhere in this file (the frequency
> loader added it). If so, reuse the existing binding rather than declaring it
> twice.

c) Render an `Export ▾` control in the header. Place it in the stat-card header row or just above the `DataTable` — a minimal native `<details>`/`<summary>` dropdown keeps it dependency-free and matches "friendly options":

```tsx
<details className="relative ml-auto inline-block">
  <summary className="cursor-pointer select-none rounded border border-[var(--color-border)] px-3 py-1 text-sm">
    Export ▾
  </summary>
  <div className="absolute right-0 z-10 mt-1 w-40 rounded border border-[var(--color-border)] bg-[var(--color-bg)] shadow">
    <button
      className="block w-full px-3 py-2 text-left text-sm hover:bg-[var(--color-bg-hover)]"
      onClick={() => handleExport("csv")}
    >
      CSV file (.csv)
    </button>
    <button
      className="block w-full px-3 py-2 text-left text-sm hover:bg-[var(--color-bg-hover)]"
      onClick={() => handleExport("text")}
    >
      Plain text (.txt)
    </button>
  </div>
</details>
```

> Match the file's existing styling tokens (look at how other buttons/borders in
> KillsView and shared components use `var(--color-...)` classes — copy the
> closest existing pattern rather than inventing class names). Disable or hide the
> control when `selectedCharacterId == null`. Closing the `<details>` after a click
> is optional polish.

- [ ] **Step 3: Typecheck the frontend**

Run from the repo root (NOT inside the ui tree if `cargo tauri dev` is running — it watches that tree):
```bash
cd crates/amanuensis-gui/ui && npm run build
```
Expected: no TypeScript errors. (`npm run build` runs `tsc -b && vite build`.)

- [ ] **Step 4: Commit**

```bash
git add crates/amanuensis-gui/ui/src/lib/commands.ts crates/amanuensis-gui/ui/src/components/views/KillsView.tsx
git commit -m "feat(gui): Export dropdown in Kills view (CSV / plain text)"
```

---

## Task 7: End-to-end verification + docs

**Files:**
- Modify: `CLAUDE.md`

- [ ] **Step 1: Full-stack manual check**

Build/run the GUI (`cd crates/amanuensis-gui && cargo tauri dev`). With a scanned DB and a character selected:
- Open Kills, click **Export ▾ → CSV file**, choose a path, confirm the success dialog.
- Open the file: header is `Creature,Vanquished,Killed,...,Best 2h Window`; verb columns are combined totals matching the on-screen numbers; a creature you know has a Best Day/2h shows those, and one without shows blanks.
- Repeat with **Plain text** and confirm a boxed ASCII table.
- Cross-check one creature against the CLI: `kills <char> --format csv` — same numbers (both go through the shared formatter).

- [ ] **Step 2: Document the feature in CLAUDE.md**

In `CLAUDE.md`, under the Kills/Kill-frequency area, add a sentence:

```markdown
- **Kills export**: the Kills view has an `Export ▾` button (CSV / plain text) that writes the unified kills table (counts + value + dates + Best Day / Best 2h) to a file via the `export_kills` Tauri command. The formatter lives once in `crates/amanuensis-core/src/export.rs` (`format_kills_export` / `Database::export_kills_merged`) and is shared with the CLI's `amanuensis kills --format csv`.
```

- [ ] **Step 3: Commit**

```bash
git add CLAUDE.md
git commit -m "docs: document GUI kills export"
```

---

## Self-Review Notes

- **Spec coverage:** shared core formatter (Tasks 1–3), unified WYSIWYG column set with combined verb totals (Task 1 `row_cells`/`HEADERS`), CSV + ASCII (Tasks 1–2), full-table sorted by total (Task 3), GUI `Export ▾` + save()/message flow (Tasks 5–6), CLI `kills --format csv` (Task 4), tests (Tasks 1–3 + manual Task 7). All spec sections map to a task.
- **Type/name consistency:** `ExportFormat { Csv, Text }`, `format_kills_export(kills, freq, format)`, `Database::export_kills_merged(char_id, format)`, and the Tauri `export_kills(char_id, format, path)` are used identically across tasks. The frontend passes `"csv"`/`"text"`; the Rust command maps those exact strings.
- **Known edge (accepted):** blank Best Day/2h cells for creatures with no frequency row; the CLI `--format csv` produces the unified columns while `--format table` keeps the old Solo/Assisted/Total layout (spec: default table unchanged).
- **Adapt-to-codebase points flagged in-task:** exact `comfy-table` version (Task 1), `selectedCharacterId` possibly already bound (Task 6), styling tokens for the dropdown (Task 6), `commands/mod.rs` re-export style (Task 5).
