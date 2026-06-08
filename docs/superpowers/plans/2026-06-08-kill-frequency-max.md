# Kill-Frequency Max Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Per creature, compute and surface the highest kills-ever in a 24h calendar-day (fixed bins) and in a 2h sliding window, in both the GUI (KillsView columns) and the CLI (`amanuensis frequency`), backed by a new per-kill `kill_events` table.

**Architecture:** A new `kill_events` table records one timestamped row per kill during scan. A single core compute layer (`Database::kill_frequency_*`) derives the two max metrics — 24h via SQL `GROUP BY date(timestamp)`, 2h via a Rust two-pointer sliding sweep. Both the CLI subcommand and the Tauri command call that one layer, guaranteeing GUI/CLI parity. Idempotency reuses existing scan skip + `reset_log_data` machinery.

**Tech Stack:** Rust (rusqlite, chrono, clap, comfy_table) for `amanuensis-core` + `amanuensis-cli`; Tauri + React + TanStack Table + Zustand for the GUI.

**Spec:** `docs/superpowers/specs/2026-06-08-kill-frequency-max-design.md`

---

## File Structure

**Create:**
- `crates/amanuensis-core/src/db/queries/frequency.rs` — `CreatureFrequency` struct + `kill_frequency_*` compute (24h SQL + 2h sweep). Single source of truth.

**Modify:**
- `crates/amanuensis-core/src/db/schema.rs` — add `kill_events` table to `create_tables()`.
- `crates/amanuensis-core/src/db/queries/log_file.rs` — `reset_log_data()` clears `kill_events`.
- `crates/amanuensis-core/src/db/queries/kill.rs` — `insert_kill_event()` method.
- `crates/amanuensis-core/src/db/queries/mod.rs` — register `frequency` module, re-export `CreatureFrequency`.
- `crates/amanuensis-core/src/parser/mod.rs` — emit a `kill_events` row in the `SoloKill`/`AssistedKill` arms.
- `crates/amanuensis-cli/src/main.rs` — `Frequency` subcommand + `cmd_frequency()` handler.
- `crates/amanuensis-gui/src/commands/data.rs` — `get_kill_frequency` Tauri command.
- `crates/amanuensis-gui/src/main.rs` — register the command in `invoke_handler`.
- `crates/amanuensis-gui/ui/src/types.ts` — `CreatureFrequency` type.
- `crates/amanuensis-gui/ui/src/lib/commands.ts` — `getKillFrequency()` wrapper.
- `crates/amanuensis-gui/ui/src/lib/store.ts` — frequency cache keyed by char.
- `crates/amanuensis-gui/ui/src/components/views/KillsView.tsx` — Best day / Best 2h columns.
- `CLAUDE.md` — note backfill requirement.

---

## Task 1: `kill_events` table + reset wiring

**Files:**
- Modify: `crates/amanuensis-core/src/db/schema.rs` (inside `create_tables()` batch)
- Modify: `crates/amanuensis-core/src/db/queries/log_file.rs:57-80` (`reset_log_data`)
- Test: `crates/amanuensis-core/src/db/queries/kill.rs` (`#[cfg(test)] mod tests`)

- [ ] **Step 1: Write the failing test**

Add to the `tests` module at the bottom of `crates/amanuensis-core/src/db/queries/kill.rs`:

```rust
    #[test]
    fn kill_events_table_exists_and_reset_clears_it() {
        let db = Database::open_in_memory().unwrap();
        let char_id = db.get_or_create_character("Tester").unwrap();

        db.conn()
            .execute(
                "INSERT INTO kill_events (character_id, creature_name, verb, assisted, timestamp)
                 VALUES (?1, 'Rat', 'killed', 0, '2024-01-01 10:00:00')",
                rusqlite::params![char_id],
            )
            .unwrap();

        let count: i64 = db
            .conn()
            .query_row("SELECT COUNT(*) FROM kill_events", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1);

        db.reset_log_data().unwrap();

        let after: i64 = db
            .conn()
            .query_row("SELECT COUNT(*) FROM kill_events", [], |r| r.get(0))
            .unwrap();
        assert_eq!(after, 0, "reset_log_data must clear kill_events");
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p amanuensis-core kill_events_table_exists_and_reset_clears_it`
Expected: FAIL — `no such table: kill_events` (or reset assertion fails).

- [ ] **Step 3: Add the table in `create_tables()`**

In `crates/amanuensis-core/src/db/schema.rs`, inside the `execute_batch` string in `create_tables()` (after the `kills` table definition), add:

```sql
        CREATE TABLE IF NOT EXISTS kill_events (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            character_id INTEGER NOT NULL,
            creature_name TEXT NOT NULL,
            verb TEXT NOT NULL,
            assisted INTEGER NOT NULL DEFAULT 0,
            timestamp TEXT NOT NULL,
            FOREIGN KEY (character_id) REFERENCES characters(id)
        );
        CREATE INDEX IF NOT EXISTS idx_kill_events_char_creature_ts
            ON kill_events(character_id, creature_name, timestamp);
```

- [ ] **Step 4: Clear it in `reset_log_data()`**

In `crates/amanuensis-core/src/db/queries/log_file.rs`, inside the `execute_batch` in `reset_log_data()`, add a line alongside the existing `DELETE FROM kills;`:

```sql
         DELETE FROM kill_events;
```

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test -p amanuensis-core kill_events_table_exists_and_reset_clears_it`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add crates/amanuensis-core/src/db/schema.rs crates/amanuensis-core/src/db/queries/log_file.rs crates/amanuensis-core/src/db/queries/kill.rs
git commit -m "feat(core): add kill_events table, cleared on reset"
```

---

## Task 2: `insert_kill_event` DB method

**Files:**
- Modify: `crates/amanuensis-core/src/db/queries/kill.rs` (inside `impl Database`)
- Test: same file, `tests` module

- [ ] **Step 1: Write the failing test**

```rust
    #[test]
    fn insert_kill_event_persists_row() {
        let db = Database::open_in_memory().unwrap();
        let char_id = db.get_or_create_character("Tester").unwrap();

        db.insert_kill_event(char_id, "Rat", "slaughtered", false, "2024-01-01 10:00:00")
            .unwrap();
        db.insert_kill_event(char_id, "Rat", "killed", true, "2024-01-01 10:05:00")
            .unwrap();

        let total: i64 = db
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM kill_events WHERE character_id = ?1 AND creature_name = 'Rat'",
                rusqlite::params![char_id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(total, 2);

        let assisted: i64 = db
            .conn()
            .query_row(
                "SELECT assisted FROM kill_events WHERE verb = 'killed'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(assisted, 1);
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p amanuensis-core insert_kill_event_persists_row`
Expected: FAIL — `no method named insert_kill_event`.

- [ ] **Step 3: Implement the method**

In `crates/amanuensis-core/src/db/queries/kill.rs`, inside `impl Database` (e.g. right after `upsert_kill`):

```rust
    /// Append one immutable kill event for windowed-frequency analysis.
    /// `verb` is the lowercase KillVerb Display string ("killed"/"slaughtered"/...).
    pub fn insert_kill_event(
        &self,
        char_id: i64,
        creature_name: &str,
        verb: &str,
        assisted: bool,
        timestamp: &str,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT INTO kill_events (character_id, creature_name, verb, assisted, timestamp)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![char_id, creature_name, verb, assisted as i64, timestamp],
        )?;
        Ok(())
    }
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p amanuensis-core insert_kill_event_persists_row`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/amanuensis-core/src/db/queries/kill.rs
git commit -m "feat(core): add insert_kill_event"
```

---

## Task 3: Emit kill events during scan

**Files:**
- Modify: `crates/amanuensis-core/src/parser/mod.rs` (the `SoloKill` and `AssistedKill` match arms, ~lines 449–461)
- Test: same file, `tests` module (or wherever parser scan tests live; search for an existing `fn` that calls `scan_bytes`/`process_log` in `#[cfg(test)]` and follow its setup)

- [ ] **Step 1: Write the failing test**

Find the existing parser test module in `crates/amanuensis-core/src/parser/mod.rs` (search `#[cfg(test)]`). Mirror an existing scan test's setup to feed log text through the scanner. Add:

```rust
    #[test]
    fn scan_records_kill_events() {
        // Mirror the existing scan-test harness in this module for constructing a
        // parser bound to an in-memory Database. The two kill lines below must each
        // produce one kill_events row.
        let log = "\
1/1/24 10:00:00 Welcome to Clan Lord, Tester!
1/1/24 10:00:01 You slaughtered a Rat.
1/1/24 10:00:02 You helped kill a Rat.
";
        let (parser, char_id) = test_scan(log, "Tester"); // <-- use this module's existing helper name
        let db = parser.db();                              // <-- use this module's accessor

        let count: i64 = db
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM kill_events WHERE character_id = ?1",
                rusqlite::params![char_id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 2);

        let solo: i64 = db
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM kill_events WHERE assisted = 0 AND verb = 'slaughtered'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(solo, 1);
    }
```

> If this module has no reusable scan-test helper, instead write the test by
> constructing the parser exactly as the nearest existing `#[test]` in the file
> does (same constructor + `scan_bytes`/`process_log` call), then assert the two
> `kill_events` rows. Do not invent helper names that don't exist — copy the
> real setup.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p amanuensis-core scan_records_kill_events`
Expected: FAIL — count is 0 (no events inserted yet).

- [ ] **Step 3: Wire inserts into the kill arms**

In `crates/amanuensis-core/src/parser/mod.rs`, in the `LogEvent::SoloKill { creature, verb }` arm, after the existing `self.db.upsert_kill(...)?;` line add:

```rust
            self.db
                .insert_kill_event(char_id, &creature, &verb.to_string(), false, &date_str)?;
```

In the `LogEvent::AssistedKill { creature, verb }` arm, after its `upsert_kill`:

```rust
            self.db
                .insert_kill_event(char_id, &creature, &verb.to_string(), true, &date_str)?;
```

> `verb.to_string()` yields the lowercase string via `KillVerb`'s `Display` impl
> (`crates/amanuensis-core/src/parser/events.rs:12`). `char_id`, `creature`,
> `verb`, and `date_str` are all already in scope in these arms.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p amanuensis-core scan_records_kill_events`
Expected: PASS

- [ ] **Step 5: Run the full core suite for regressions**

Run: `cargo test -p amanuensis-core`
Expected: all pass (the 173 existing tests + the new ones).

- [ ] **Step 6: Commit**

```bash
git add crates/amanuensis-core/src/parser/mod.rs
git commit -m "feat(core): record kill_events during scan"
```

---

## Task 4: Core frequency compute layer

**Files:**
- Create: `crates/amanuensis-core/src/db/queries/frequency.rs`
- Modify: `crates/amanuensis-core/src/db/queries/mod.rs` (register + re-export)
- Test: in `frequency.rs` `#[cfg(test)] mod tests`

This is the single source of truth for both surfaces. 24h = fixed calendar-day max via SQL; 2h = sliding-window true max via a Rust two-pointer sweep. Per-verb breakdown of each winning bucket is included.

- [ ] **Step 1: Write the failing tests**

Create `crates/amanuensis-core/src/db/queries/frequency.rs` with the test module first (implementation added in Step 3):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::queries::Database;

    fn insert(db: &Database, char_id: i64, creature: &str, verb: &str, ts: &str) {
        db.insert_kill_event(char_id, creature, verb, false, ts).unwrap();
    }

    #[test]
    fn best_day_picks_peak_calendar_day() {
        let db = Database::open_in_memory().unwrap();
        let c = db.get_or_create_character("Tester").unwrap();
        // Day A: 2 rats. Day B: 3 rats (the peak).
        insert(&db, c, "Rat", "killed", "2024-01-01 09:00:00");
        insert(&db, c, "Rat", "killed", "2024-01-01 22:00:00");
        insert(&db, c, "Rat", "killed", "2024-01-02 08:00:00");
        insert(&db, c, "Rat", "slaughtered", "2024-01-02 09:00:00");
        insert(&db, c, "Rat", "killed", "2024-01-02 10:00:00");

        let freq = db.kill_frequency_for_char_ids(&[c], true).unwrap();
        let rat = freq.iter().find(|f| f.creature_name == "Rat").unwrap();
        assert_eq!(rat.best_day_count, 3);
        assert_eq!(rat.best_day_date.as_deref(), Some("2024-01-02"));
    }

    #[test]
    fn best_2h_sliding_window_catches_cross_bin_burst() {
        let db = Database::open_in_memory().unwrap();
        let c = db.get_or_create_character("Tester").unwrap();
        // 4 kills spanning 01:00..02:30 — splits across the midnight-aligned
        // 00:00-02:00 and 02:00-04:00 fixed bins (would be 3 + 1), but a sliding
        // 2h window starting 01:00 captures all 4.
        insert(&db, c, "Orga", "killed", "2024-01-01 01:00:00");
        insert(&db, c, "Orga", "killed", "2024-01-01 01:30:00");
        insert(&db, c, "Orga", "killed", "2024-01-01 01:59:00");
        insert(&db, c, "Orga", "killed", "2024-01-01 02:30:00");

        let freq = db.kill_frequency_for_char_ids(&[c], true).unwrap();
        let orga = freq.iter().find(|f| f.creature_name == "Orga").unwrap();
        assert_eq!(orga.best_2h_count, 4);
        assert_eq!(orga.best_2h_start.as_deref(), Some("2024-01-01 01:00:00"));
    }

    #[test]
    fn solo_only_excludes_assisted() {
        let db = Database::open_in_memory().unwrap();
        let c = db.get_or_create_character("Tester").unwrap();
        db.insert_kill_event(c, "Rat", "killed", false, "2024-01-01 09:00:00").unwrap();
        db.insert_kill_event(c, "Rat", "killed", true, "2024-01-01 09:30:00").unwrap();

        let with_assist = db.kill_frequency_for_char_ids(&[c], true).unwrap();
        assert_eq!(with_assist[0].best_day_count, 2);

        let solo = db.kill_frequency_for_char_ids(&[c], false).unwrap();
        assert_eq!(solo[0].best_day_count, 1);
    }

    #[test]
    fn per_verb_breakdown_of_best_day() {
        let db = Database::open_in_memory().unwrap();
        let c = db.get_or_create_character("Tester").unwrap();
        insert(&db, c, "Rat", "killed", "2024-01-02 08:00:00");
        insert(&db, c, "Rat", "killed", "2024-01-02 09:00:00");
        insert(&db, c, "Rat", "slaughtered", "2024-01-02 10:00:00");

        let freq = db.kill_frequency_for_char_ids(&[c], true).unwrap();
        let rat = freq.iter().find(|f| f.creature_name == "Rat").unwrap();
        assert_eq!(rat.best_day_count, 3);
        assert_eq!(rat.best_day_verbs.get("killed").copied(), Some(2));
        assert_eq!(rat.best_day_verbs.get("slaughtered").copied(), Some(1));
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p amanuensis-core --lib frequency`
Expected: FAIL — module/method not found (won't compile until Step 3 + 4).

- [ ] **Step 3: Implement the compute layer**

At the TOP of `crates/amanuensis-core/src/db/queries/frequency.rs` (above the test module):

```rust
use std::collections::BTreeMap;

use chrono::NaiveDateTime;
use rusqlite::params_from_iter;
use serde::Serialize;

use crate::error::Result;
use super::Database;

/// Window length for the sliding 2-hour max, in seconds.
const TWO_HOURS_SECS: i64 = 2 * 60 * 60;

/// Per-creature max-frequency stats.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct CreatureFrequency {
    pub creature_name: String,
    /// Highest kills in any single calendar day (fixed midnight-aligned bins).
    pub best_day_count: i64,
    /// Date ("YYYY-MM-DD") of the peak day, if any kills exist.
    pub best_day_date: Option<String>,
    /// Per-verb breakdown of the peak day.
    pub best_day_verbs: BTreeMap<String, i64>,
    /// Highest kills in any 2-hour sliding window (true max).
    pub best_2h_count: i64,
    /// Start timestamp ("YYYY-MM-DD HH:MM:SS") of the peak 2h window.
    pub best_2h_start: Option<String>,
    /// Per-verb breakdown of the peak 2h window.
    pub best_2h_verbs: BTreeMap<String, i64>,
}

#[derive(Clone)]
struct Event {
    verb: String,
    ts_raw: String,
    ts: NaiveDateTime,
}

/// Parse a stored timestamp. Real CL lines are full datetimes; if a line lacked a
/// time component the stored value is date-only, which we treat as midnight so the
/// 24h metric still works (the 2h metric collapses such events to one instant).
fn parse_ts(s: &str) -> NaiveDateTime {
    NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S")
        .or_else(|_| {
            chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d")
                .map(|d| d.and_hms_opt(0, 0, 0).unwrap())
        })
        .unwrap_or_else(|_| NaiveDateTime::parse_from_str("1970-01-01 00:00:00", "%Y-%m-%d %H:%M:%S").unwrap())
}

impl Database {
    /// Compute per-creature max-frequency stats across one or more character IDs
    /// (multiple IDs support merged characters). `include_assisted=false` counts
    /// solo kills only.
    pub fn kill_frequency_for_char_ids(
        &self,
        char_ids: &[i64],
        include_assisted: bool,
    ) -> Result<Vec<CreatureFrequency>> {
        if char_ids.is_empty() {
            return Ok(Vec::new());
        }
        let placeholders = char_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let assisted_clause = if include_assisted { "" } else { " AND assisted = 0" };
        let sql = format!(
            "SELECT creature_name, verb, timestamp FROM kill_events
             WHERE character_id IN ({placeholders}){assisted_clause}
             ORDER BY creature_name, timestamp",
        );

        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(params_from_iter(char_ids.iter()), |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?;

        // Group events by creature (rows already sorted by creature, timestamp).
        let mut by_creature: BTreeMap<String, Vec<Event>> = BTreeMap::new();
        for r in rows {
            let (creature, verb, ts_raw) = r?;
            let ts = parse_ts(&ts_raw);
            by_creature
                .entry(creature)
                .or_default()
                .push(Event { verb, ts_raw, ts });
        }

        let mut out = Vec::with_capacity(by_creature.len());
        for (creature_name, events) in by_creature {
            out.push(compute_one(creature_name, &events));
        }
        // Most-frequent first (by best day) for stable, useful default ordering.
        out.sort_by(|a, b| b.best_day_count.cmp(&a.best_day_count).then(a.creature_name.cmp(&b.creature_name)));
        Ok(out)
    }

    /// Merged-character convenience wrapper. Includes assisted kills.
    pub fn kill_frequency_merged(&self, char_id: i64) -> Result<Vec<CreatureFrequency>> {
        let ids = self.char_ids_for_merged(char_id)?;
        self.kill_frequency_for_char_ids(&ids, true)
    }
}

/// Compute both metrics for one creature's time-sorted events.
fn compute_one(creature_name: String, events: &[Event]) -> CreatureFrequency {
    // --- 24h: fixed calendar-day bins ---
    let mut day_counts: BTreeMap<String, BTreeMap<String, i64>> = BTreeMap::new();
    for e in events {
        let day = e.ts_raw.get(0..10).unwrap_or(&e.ts_raw).to_string();
        *day_counts.entry(day).or_default().entry(e.verb.clone()).or_default() += 1;
    }
    let mut best_day_count = 0i64;
    let mut best_day_date: Option<String> = None;
    let mut best_day_verbs: BTreeMap<String, i64> = BTreeMap::new();
    for (day, verbs) in &day_counts {
        let total: i64 = verbs.values().sum();
        // Strictly-greater keeps the earliest day on ties (deterministic).
        if total > best_day_count {
            best_day_count = total;
            best_day_date = Some(day.clone());
            best_day_verbs = verbs.clone();
        }
    }

    // --- 2h: sliding-window true max (two-pointer over sorted events) ---
    let mut best_2h_count = 0i64;
    let mut best_2h_start: Option<String> = None;
    let mut best_2h_verbs: BTreeMap<String, i64> = BTreeMap::new();
    let mut left = 0usize;
    for right in 0..events.len() {
        // Shrink from the left until the window is <= 2 hours.
        while (events[right].ts - events[left].ts).num_seconds() > TWO_HOURS_SECS {
            left += 1;
        }
        let count = (right - left + 1) as i64;
        if count > best_2h_count {
            best_2h_count = count;
            best_2h_start = Some(events[left].ts_raw.clone());
            let mut verbs: BTreeMap<String, i64> = BTreeMap::new();
            for e in &events[left..=right] {
                *verbs.entry(e.verb.clone()).or_default() += 1;
            }
            best_2h_verbs = verbs;
        }
    }

    CreatureFrequency {
        creature_name,
        best_day_count,
        best_day_date,
        best_day_verbs,
        best_2h_count,
        best_2h_start,
        best_2h_verbs,
    }
}
```

- [ ] **Step 4: Register the module**

In `crates/amanuensis-core/src/db/queries/mod.rs`, add the module declaration next to the others (e.g. after `mod kill;`):

```rust
mod frequency;
```

And add to the re-export line (next to `pub use kill::{KillsFilter, filter_kills};`):

```rust
pub use frequency::CreatureFrequency;
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p amanuensis-core --lib frequency`
Expected: PASS (all four tests).

- [ ] **Step 6: Commit**

```bash
git add crates/amanuensis-core/src/db/queries/frequency.rs crates/amanuensis-core/src/db/queries/mod.rs
git commit -m "feat(core): kill-frequency compute (24h day + 2h sliding window)"
```

---

## Task 5: CLI `frequency` subcommand

**Files:**
- Modify: `crates/amanuensis-cli/src/main.rs` (Commands enum, `run()` match, new `cmd_frequency`)

- [ ] **Step 1: Add the subcommand variant**

In the `enum Commands` in `crates/amanuensis-cli/src/main.rs` (near the `Kills` variant ~line 62), add:

```rust
    /// Show max kill-frequency per creature (24h day max + 2h sliding window).
    Frequency {
        /// Character name
        name: String,
        /// Which metric(s): day, 2h, both
        #[arg(long, default_value = "both")]
        bin: String,
        /// Count solo kills only (exclude assisted)
        #[arg(long)]
        solo: bool,
        /// Include per-verb breakdown columns
        #[arg(long)]
        by_verb: bool,
        /// Output format: table, csv, json
        #[arg(long, default_value = "table")]
        format: String,
        /// Limit number of rows
        #[arg(long)]
        limit: Option<usize>,
    },
```

- [ ] **Step 2: Add the dispatch arm**

In `run()` (the `match cli.command` block, ~line 330 near `Commands::Kills`), add:

```rust
        Commands::Frequency { name, bin, solo, by_verb, format, limit } => {
            cmd_frequency(&db_path, &name, &bin, solo, by_verb, &format, limit)
        }
```

- [ ] **Step 3: Implement the handler**

Add near `cmd_kills` (~line 707) in `crates/amanuensis-cli/src/main.rs`:

```rust
fn cmd_frequency(
    db_path: &str,
    name: &str,
    bin: &str,
    solo: bool,
    by_verb: bool,
    format: &str,
    limit: Option<usize>,
) -> amanuensis_core::Result<()> {
    let db = Database::open(db_path)?;
    let char = resolve_character(&db, name)?;
    let char_id = char.id.unwrap();
    let ids = db.char_ids_for_merged(char_id)?;

    let mut freq = db.kill_frequency_for_char_ids(&ids, !solo)?;
    if let Some(limit) = limit {
        freq.truncate(limit);
    }
    if freq.is_empty() {
        println!("No kill-frequency data for {}. (Run `scan --force` to backfill kill_events.)", name);
        return Ok(());
    }

    let show_day = bin == "day" || bin == "both";
    let show_2h = bin == "2h" || bin == "both";

    let verbs_str = |m: &std::collections::BTreeMap<String, i64>| {
        m.iter().map(|(v, n)| format!("{v}:{n}")).collect::<Vec<_>>().join(" ")
    };

    match format {
        "csv" => {
            let mut header = vec!["creature".to_string()];
            if show_day { header.push("best_day_count".into()); header.push("best_day_date".into()); }
            if show_2h { header.push("best_2h_count".into()); header.push("best_2h_start".into()); }
            if by_verb {
                if show_day { header.push("best_day_verbs".into()); }
                if show_2h { header.push("best_2h_verbs".into()); }
            }
            println!("{}", header.join(","));
            for f in &freq {
                let mut row = vec![f.creature_name.clone()];
                if show_day {
                    row.push(f.best_day_count.to_string());
                    row.push(f.best_day_date.clone().unwrap_or_default());
                }
                if show_2h {
                    row.push(f.best_2h_count.to_string());
                    row.push(f.best_2h_start.clone().unwrap_or_default());
                }
                if by_verb {
                    if show_day { row.push(verbs_str(&f.best_day_verbs)); }
                    if show_2h { row.push(verbs_str(&f.best_2h_verbs)); }
                }
                // Quote fields containing commas/spaces for safe CSV.
                let escaped: Vec<String> = row.iter().map(|c| {
                    if c.contains(',') || c.contains('"') || c.contains(' ') {
                        format!("\"{}\"", c.replace('"', "\"\""))
                    } else { c.clone() }
                }).collect();
                println!("{}", escaped.join(","));
            }
        }
        "json" => {
            println!("{}", serde_json::to_string_pretty(&freq)?);
        }
        _ => {
            let mut header = vec!["Creature"];
            if show_day { header.push("Best Day"); header.push("Day Date"); }
            if show_2h { header.push("Best 2h"); header.push("2h Start"); }
            let mut table = Table::new();
            table
                .load_preset(UTF8_FULL)
                .apply_modifier(UTF8_ROUND_CORNERS)
                .set_content_arrangement(ContentArrangement::Dynamic)
                .set_header(header);
            for f in &freq {
                let mut row = vec![f.creature_name.clone()];
                if show_day {
                    row.push(f.best_day_count.to_string());
                    row.push(f.best_day_date.clone().unwrap_or_default());
                }
                if show_2h {
                    row.push(f.best_2h_count.to_string());
                    row.push(f.best_2h_start.clone().unwrap_or_default());
                }
                table.add_row(row);
            }
            println!("Kill frequency for {}:", name);
            println!("{table}");
        }
    }
    Ok(())
}
```

> `serde_json` is already a dependency of `amanuensis-core`; confirm `amanuensis-cli`
> has it (check `crates/amanuensis-cli/Cargo.toml`). If absent, add `serde_json` to
> its `[dependencies]` (match the workspace version used elsewhere). `CreatureFrequency`
> already derives `Serialize`.

- [ ] **Step 4: Build and smoke-test**

Run: `cargo build -p amanuensis-cli`
Expected: compiles.

Run (against your real dev DB after a backfill scan):
```bash
cargo run -p amanuensis-cli -- scan --force <your-log-folder>
cargo run -p amanuensis-cli -- frequency Gandor --bin both --limit 10
cargo run -p amanuensis-cli -- frequency Gandor --format csv --by-verb --limit 5
```
Expected: a table of creatures with Best Day / Best 2h numbers and dates; CSV form prints comma-separated with verb breakdown.

- [ ] **Step 5: Commit**

```bash
git add crates/amanuensis-cli/src/main.rs crates/amanuensis-cli/Cargo.toml
git commit -m "feat(cli): add frequency subcommand (table/csv/json, 24h + 2h)"
```

---

## Task 6: Tauri `get_kill_frequency` command

**Files:**
- Modify: `crates/amanuensis-gui/src/commands/data.rs`
- Modify: `crates/amanuensis-gui/src/main.rs` (register in `invoke_handler`)

- [ ] **Step 1: Add the command**

In `crates/amanuensis-gui/src/commands/data.rs`, add the import and command. Update the top `use` for core types:

```rust
use amanuensis_core::db::queries::CreatureFrequency;
```

Then add (mirroring `get_kills`):

```rust
/// Per-creature max kill-frequency (24h day max + 2h sliding window), merged sources.
/// `include_assisted=false` counts solo kills only.
#[tauri::command]
pub fn get_kill_frequency(
    char_id: i64,
    include_assisted: bool,
    state: State<'_, AppState>,
) -> Result<Vec<CreatureFrequency>, String> {
    state.with_db(|db| {
        let ids = db.char_ids_for_merged(char_id).map_err(|e| e.to_string())?;
        db.kill_frequency_for_char_ids(&ids, include_assisted)
            .map_err(|e| e.to_string())
    })
}
```

> Confirm `char_ids_for_merged` is publicly reachable. In `kill.rs` the merged
> wrappers are public methods on `Database`; `char_ids_for_merged` is defined in
> `merge.rs`. If it is not `pub`, either make it `pub` or call the provided
> `db.kill_frequency_merged(char_id)` instead (which always includes assisted) and
> drop the `include_assisted` param's solo path for the GUI v1. Prefer making it
> `pub` so the GUI can offer the solo toggle.

- [ ] **Step 2: Register the command**

In `crates/amanuensis-gui/src/main.rs`, add to the `tauri::generate_handler![...]` list (next to `commands::get_kills`):

```rust
            commands::get_kill_frequency,
```

> If `data.rs` items are re-exported through `commands/mod.rs`, ensure
> `get_kill_frequency` is included there too (check how `get_kills` is surfaced as
> `commands::get_kills`).

- [ ] **Step 3: Build**

Run: `cargo build -p amanuensis-gui`
Expected: compiles.

- [ ] **Step 4: Commit**

```bash
git add crates/amanuensis-gui/src/commands/data.rs crates/amanuensis-gui/src/main.rs
git commit -m "feat(gui): get_kill_frequency Tauri command"
```

---

## Task 7: Frontend — type, command wrapper, store, columns

**Files:**
- Modify: `crates/amanuensis-gui/ui/src/types.ts`
- Modify: `crates/amanuensis-gui/ui/src/lib/commands.ts`
- Modify: `crates/amanuensis-gui/ui/src/lib/store.ts`
- Modify: `crates/amanuensis-gui/ui/src/components/views/KillsView.tsx`

- [ ] **Step 1: Add the TS type**

In `crates/amanuensis-gui/ui/src/types.ts`, add (match the Rust `CreatureFrequency` field names exactly — serde serializes snake_case):

```typescript
export interface CreatureFrequency {
  creature_name: string;
  best_day_count: number;
  best_day_date: string | null;
  best_day_verbs: Record<string, number>;
  best_2h_count: number;
  best_2h_start: string | null;
  best_2h_verbs: Record<string, number>;
}
```

- [ ] **Step 2: Add the command wrapper**

In `crates/amanuensis-gui/ui/src/lib/commands.ts` (mirror the existing `getKills`):

```typescript
export async function getKillFrequency(
  charId: number,
  includeAssisted: boolean,
): Promise<CreatureFrequency[]> {
  return invoke<CreatureFrequency[]>("get_kill_frequency", {
    charId,
    includeAssisted,
  });
}
```

> Import `CreatureFrequency` at the top of `commands.ts` from `../types` if that
> file uses explicit type imports (follow the existing import style there).

- [ ] **Step 3: Cache in the store**

In `crates/amanuensis-gui/ui/src/lib/store.ts`, add to the `AppStore` interface:

```typescript
  // Kill frequency for the active character (keyed by char id).
  killFrequency: Record<string, CreatureFrequency>;
  killFrequencyCharId: number | null;
  setKillFrequency: (charId: number, rows: CreatureFrequency[]) => void;
```

And in the `create<AppStore>` implementation:

```typescript
  killFrequency: {},
  killFrequencyCharId: null,
  setKillFrequency: (charId, rows) =>
    set(() => ({
      killFrequencyCharId: charId,
      killFrequency: rows.reduce(
        (acc, r) => ({ ...acc, [r.creature_name]: r }),
        {} as Record<string, CreatureFrequency>,
      ),
    })),
```

> Import `CreatureFrequency` into `store.ts` from `../types`.

- [ ] **Step 4: Load it where kills load, and add columns**

In `crates/amanuensis-gui/ui/src/components/views/KillsView.tsx`:

a) Read the frequency map + loader from the store and fetch it for the active character, mirroring how the view currently obtains `kills` for the selected character. Example (adapt to this file's existing hooks/props for the active char id):

```typescript
const killFrequency = useStore((s) => s.killFrequency);
const killFrequencyCharId = useStore((s) => s.killFrequencyCharId);
const setKillFrequency = useStore((s) => s.setKillFrequency);

useEffect(() => {
  if (charId == null) return;
  if (killFrequencyCharId === charId) return; // already loaded for this char
  getKillFrequency(charId, true)
    .then((rows) => setKillFrequency(charId, rows))
    .catch((err) => console.error("Failed to load kill frequency:", err));
}, [charId, killFrequencyCharId, setKillFrequency]);
```

b) Add two columns to the `columns` array (after the existing date columns), reading from the store map by creature name:

```typescript
  columnHelper.display({
    id: "best_day",
    header: "Best Day",
    cell: (info) => {
      const f = killFrequency[info.row.original.creature_name];
      if (!f || f.best_day_count === 0) return "-";
      return (
        <span title={f.best_day_date ? `Peak day: ${f.best_day_date}` : undefined}>
          {f.best_day_count.toLocaleString()}
        </span>
      );
    },
  }),
  columnHelper.display({
    id: "best_2h",
    header: "Best 2h",
    cell: (info) => {
      const f = killFrequency[info.row.original.creature_name];
      if (!f || f.best_2h_count === 0) return "-";
      return (
        <span title={f.best_2h_start ? `Peak 2h window start: ${f.best_2h_start}` : undefined}>
          {f.best_2h_count.toLocaleString()}
        </span>
      );
    },
  }),
```

> `columns` must be able to close over `killFrequency`. If the current `columns`
> is a module-level constant, move it inside the component (or wrap in
> `useMemo([killFrequency])`) so the cells re-render when the map loads. Follow
> whatever memoization the file already uses for other state-dependent columns.

- [ ] **Step 5: Build the frontend**

Run: `cd crates/amanuensis-gui/ui && npm run build` (or the project's configured typecheck/build, e.g. `npm run typecheck`).
Expected: no TypeScript errors.

- [ ] **Step 6: Commit**

```bash
git add crates/amanuensis-gui/ui/src/types.ts crates/amanuensis-gui/ui/src/lib/commands.ts crates/amanuensis-gui/ui/src/lib/store.ts crates/amanuensis-gui/ui/src/components/views/KillsView.tsx
git commit -m "feat(gui): Best Day / Best 2h columns in KillsView"
```

---

## Task 8: Manual end-to-end verification + docs

**Files:**
- Modify: `CLAUDE.md`

- [ ] **Step 1: Full app run**

Run the GUI (project's run command, e.g. `cargo tauri dev` from `crates/amanuensis-gui`, or the `run` skill). With a database scanned via `scan --force`:
- Open KillsView, confirm Best Day / Best 2h columns populate with numbers and date tooltips.
- Cross-check one creature against the CLI: `frequency <char> --bin both` — the GUI numbers must match the CLI numbers exactly (parity).

- [ ] **Step 2: Note the backfill requirement in CLAUDE.md**

In `CLAUDE.md`, under "Bestiary surface" / data sections, add a sentence:

```markdown
- **Kill frequency**: `kill_events` records one timestamped row per kill. Per-creature max stats (highest in any 24h calendar day, and any 2h sliding window) appear as Best Day / Best 2h columns in KillsView and via `amanuensis frequency <char> [--bin day|2h|both] [--solo] [--by-verb] [--format table|csv|json]`. **Databases created before this feature must run `amanuensis scan --force <folder>` once to backfill `kill_events`.**
```

- [ ] **Step 3: Commit**

```bash
git add CLAUDE.md
git commit -m "docs: document kill-frequency feature and backfill requirement"
```

---

## Self-Review Notes

- **Spec coverage:** kill_events table (Task 1), scan recording + idempotency via reset (Tasks 1, 3), 24h fixed-day max + 2h sliding window (Task 4), verb/assisted handling incl. solo toggle (Task 4 + surfaces), GUI KillsView columns (Task 7), CLI `frequency` with `--by-verb`/`--solo`/`--bin`/`--format` (Task 5), CLI/GUI parity via shared compute layer (Tasks 4/5/6, verified Task 8). All spec sections map to a task.
- **Parity:** both the CLI handler (Task 5) and the Tauri command (Task 6) call `kill_frequency_for_char_ids` — the single source of truth — so numbers cannot drift.
- **Type consistency:** `CreatureFrequency` fields are defined once in Task 4 and reused verbatim in Tasks 5–7 (`best_day_count`, `best_day_date`, `best_day_verbs`, `best_2h_count`, `best_2h_start`, `best_2h_verbs`). The TS interface mirrors serde's snake_case output.
- **Known edge (accepted):** timestamp-less kill lines store a date-only string → counted at midnight for the 2h sweep; 24h metric unaffected. Real CL logs always carry timestamps.
- **Adapt-to-codebase points flagged in-task:** the parser scan-test helper name (Task 3), `char_ids_for_merged` visibility (Task 6), and `columns` memoization in KillsView (Task 7) must follow the file's existing conventions — each is called out where it occurs.
