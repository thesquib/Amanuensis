# Last Scan View Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let the user flip the Kills and Trainers (Ranks) tables between "All data" and "Last scan" (only what the most recent scan found), in the GUI and the CLI.

**Architecture:** The aggregated `kills` and `trainers` tables cannot be diffed after the fact, so each scan also writes its own contribution into two small shadow tables, `scan_kills` and `scan_trainers`, using the very same upsert SQL (parametrised by table name). Shadow tables hold only the latest scan that wrote anything: the first write of a new scan clears them. A scan that finds nothing leaves the previous "last scan" visible. Queries take a `ScanScope` (`All` | `LastScan`) and read from whichever table.

**Tech Stack:** Rust (rusqlite, `amanuensis-core`), clap CLI, Tauri 2 commands, React/Zustand UI.

**Spec:** None written. Origin is the Scribius 2.0 "All Data / Last Scan" toggle (`/Applications/Scribius.app/Contents/Resources/Scribius-Wiki.md`, changelog 2.0 and 2.0.2). Design decisions are fixed in this plan, see "Global Constraints".

## Global Constraints

- Scope is **kills and trainer ranks only**. Lastys, pets, coins, characters, checkpoints are not scoped.
- "Last scan" ranks view shows **scanned ranks only**: `modified_ranks` is 0 in that view (Scribius 2.0.2 fixed exactly this leak).
- `kills` / `trainers` / `kill_hourly` semantics and values must not change; shadow tables are additive.
- The shadow tables keep **only the latest scan** (no history, no growth).
- A scan that writes no kill/rank rows must NOT clear the previous last-scan data.
- `reset_log_data` and `delete_all_data` clear the shadow tables. A Rescan's own scan then repopulates them, so after Rescan "Last scan" == everything.
- Existing DBs: new tables are created via `CREATE TABLE IF NOT EXISTS` in `schema.rs`; no backfill (they start empty; the toggle shows an empty state until the first scan).
- GUI-first: GUI toggle is primary, CLI gets `--last-scan` on `kills` and `trainers` so the two stay in step.
- Test commands use the arm target (local rustup is x86 under Rosetta): `cargo test -p amanuensis-core --target aarch64-apple-darwin`.
- Do not run `tsc`/builds in `crates/amanuensis-gui/ui` while `cargo tauri dev` is running (it cycles the app).
- Do not push or cut a release without being asked. Commit messages follow repo style (`feat:`, `fix:`, `docs:`).

## Review Focus

1. Update Logs on an already-up-to-date DB (nothing found): Last scan must still show the previous scan, not go blank.
2. A scan spanning several source folders / several files: all of it is ONE "last scan"; only the first write clears the shadow tables.
3. Tail scan (grown file): only the appended tail's kills/ranks appear, not the whole file.
4. Characters with merge sources: Last scan view must sum shadow rows across the merged character ids, like `get_kills_merged`.
5. Rescan: shadow tables reset then repopulate, never double-count (`reset_log_data` must clear them).

## File Structure

- `crates/amanuensis-core/src/db/schema.rs` — add `scan_kills`, `scan_trainers` DDL.
- `crates/amanuensis-core/src/models/mod.rs` (or wherever `Kill` lives) — add `ScanScope` enum.
- `crates/amanuensis-core/src/db/queries/scan_scope.rs` — NEW. Scan-token lifecycle (`ensure_scan_started`) and the shadow-table plumbing; keeps `kill.rs`/`trainer.rs` from growing.
- `crates/amanuensis-core/src/db/queries/kill.rs` — parametrise `upsert_kill` / `get_kills` by table.
- `crates/amanuensis-core/src/db/queries/trainer.rs` — parametrise `upsert_trainer_rank` / `upsert_apply_learning` / `get_trainers`.
- `crates/amanuensis-core/src/db/queries/merge.rs` — `get_kills_merged_scoped`, `get_trainers_merged_scoped`.
- `crates/amanuensis-core/src/db/queries/log_file.rs` — clear shadow tables in `reset_log_data` / `delete_all_data`.
- `crates/amanuensis-core/src/parser/mod.rs:710-748,1017` — dual-write at the call sites; scan token created at scan entry points.
- `crates/amanuensis-gui/src/commands/data.rs` — `scope` arg on `get_kills`, `get_trainers`.
- `crates/amanuensis-gui/ui/src/lib/{commands,store}.ts`, `components/views/{KillsView,TrainersView}.tsx` — toggle.
- `crates/amanuensis-cli/src/main.rs` — `--last-scan` flags.
- `CLAUDE.md` — document the feature.

---

### Task 1: Shadow tables, `ScanScope`, dual-written upserts

**Files:**
- Modify: `crates/amanuensis-core/src/db/schema.rs` (next to `kill_hourly`, ~line 80)
- Create: `crates/amanuensis-core/src/db/queries/scan_scope.rs`
- Modify: `crates/amanuensis-core/src/db/queries/mod.rs` (declare module)
- Modify: `crates/amanuensis-core/src/db/queries/kill.rs:52-135`, `trainer.rs:36-110`
- Test: inline `#[cfg(test)]` in `scan_scope.rs`

**Interfaces:**
- Produces:
  - `pub enum ScanScope { All, LastScan }` (`Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug`, `Default = All`, serde `snake_case`) re-exported from `amanuensis_core`.
  - `Database::begin_scan_token(&self) -> Result<i64>` — returns a fresh token (monotonic, from `db_meta` key `scan_token_counter`).
  - `Database::mark_scan_write(&self, token: i64) -> Result<()>` — if `token` != `db_meta.last_scan_token`, runs `DELETE FROM scan_kills; DELETE FROM scan_trainers;` then stores `token` as `last_scan_token`. Idempotent for the same token.
  - `Database::upsert_kill_scan(...)` and `Database::upsert_trainer_rank_scan(...)`, `Database::upsert_apply_learning_scan(...)` with the same signatures as the originals; they write only to the shadow tables.

- [ ] **Step 1: Write failing tests** (in `scan_scope.rs`)

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::Database;

    fn count(db: &Database, table: &str) -> i64 {
        db.conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0)).unwrap()
    }

    #[test]
    fn same_token_accumulates_new_token_clears() {
        let db = Database::open_in_memory().unwrap();
        let id = db.get_or_create_character("Fen").unwrap();
        let t1 = db.begin_scan_token().unwrap();
        db.mark_scan_write(t1).unwrap();
        db.upsert_kill_scan(id, "Rat", "killed_count", 2, "2024-01-01").unwrap();
        db.mark_scan_write(t1).unwrap(); // same scan: must NOT clear
        db.upsert_kill_scan(id, "Rat", "killed_count", 2, "2024-01-01").unwrap();
        assert_eq!(db.get_kills_scoped(id, ScanScope::LastScan).unwrap()[0].killed_count, 2);

        let t2 = db.begin_scan_token().unwrap();
        assert!(t2 > t1);
        db.mark_scan_write(t2).unwrap(); // new scan's first write clears
        assert_eq!(count(&db, "scan_kills"), 0);
    }

    #[test]
    fn token_without_write_leaves_previous_scan() {
        let db = Database::open_in_memory().unwrap();
        let id = db.get_or_create_character("Fen").unwrap();
        let t1 = db.begin_scan_token().unwrap();
        db.mark_scan_write(t1).unwrap();
        db.upsert_kill_scan(id, "Rat", "killed_count", 2, "2024-01-01").unwrap();
        let _t2 = db.begin_scan_token().unwrap(); // scan that finds nothing
        assert_eq!(count(&db, "scan_kills"), 1);
    }

    #[test]
    fn last_scan_trainer_ranks_exclude_modified_ranks() {
        let db = Database::open_in_memory().unwrap();
        let id = db.get_or_create_character("Fen").unwrap();
        db.upsert_trainer_rank(id, "Atkus", "2024-01-01 10:00:00", 1.0).unwrap();
        db.conn.execute("UPDATE trainers SET modified_ranks = 7 WHERE trainer_name='Atkus'", []).unwrap();
        let t = db.begin_scan_token().unwrap();
        db.mark_scan_write(t).unwrap();
        db.upsert_trainer_rank_scan(id, "Atkus", "2024-01-01 10:00:00", 1.0).unwrap();
        let last = db.get_trainers_scoped(id, ScanScope::LastScan).unwrap();
        assert_eq!((last[0].ranks, last[0].modified_ranks), (1, 0));
        let all = db.get_trainers_scoped(id, ScanScope::All).unwrap();
        assert_eq!((all[0].ranks, all[0].modified_ranks), (1, 7));
    }
}
```

- [ ] **Step 2: Run to verify failure**
Run: `cargo test -p amanuensis-core --target aarch64-apple-darwin scan_scope`
Expected: compile errors (`begin_scan_token` etc. undefined).

- [ ] **Step 3: Implement**
  1. `schema.rs`: add `scan_kills` as a copy of the `kills` DDL (same columns incl. `date_first_*`, `date_last_*`, `best_loot_*`, `UNIQUE(character_id, creature_name)`) and `scan_trainers` as a copy of the `trainers` DDL (read the live DDL plus the `migrate_tables` ALTERs for `trainers`/`kills` and include every column those add, so the shared SQL works against both). Both `CREATE TABLE IF NOT EXISTS`. Also add them to the fresh-DB path and the legacy-migration path (schema.rs ~319-345 recreate pattern).
  2. `kill.rs`: rename the body of `upsert_kill` into private `fn upsert_kill_into(&self, table: &str, ...)`. Replace the literal `kills` in the INSERT target and every `kills.{c}` column qualifier with `{table}`. `upsert_kill` calls it with `"kills"`, `upsert_kill_scan` with `"scan_kills"`. Likewise make `get_kills` a thin wrapper over `fn get_kills_from(&self, table, char_id)` and add `get_kills_scoped(char_id, scope)` choosing the table.
  3. `trainer.rs`: same treatment for `upsert_trainer_rank` / `upsert_apply_learning` (table param) and `get_trainers` -> `get_trainers_from(table, ..)`, `get_trainers_scoped`. For `scan_trainers` the query must return `modified_ranks = 0` (it will, the column defaults to 0 and is never written).
  4. `scan_scope.rs`: `ScanScope`, `begin_scan_token`, `mark_scan_write` using `db_meta` (`INSERT ... ON CONFLICT(key) DO UPDATE`; confirm the `db_meta` column names from `schema.rs:179`).
- [ ] **Step 4: Run tests**
Run: `cargo test -p amanuensis-core --target aarch64-apple-darwin scan_scope kill trainer`
Expected: PASS, and all pre-existing kill/trainer tests still pass (the refactor must be behaviour-neutral).
- [ ] **Step 5: Commit** — `feat(core): shadow tables and scoped upserts for last-scan view`

---

### Task 2: Merged scoped queries and reset hooks

**Files:**
- Modify: `crates/amanuensis-core/src/db/queries/merge.rs:175-260`, `log_file.rs:84-130`
- Test: `merge.rs` tests module, `log_file.rs` tests

**Interfaces:**
- Consumes: `ScanScope`, `get_kills_from`, `get_trainers_from` (Task 1).
- Produces: `Database::get_kills_merged_scoped(char_id, ScanScope) -> Result<Vec<Kill>>`, `Database::get_trainers_merged_scoped(char_id, ScanScope) -> Result<Vec<Trainer>>`. Existing `get_kills_merged` / `get_trainers_merged` become `…_scoped(.., ScanScope::All)` wrappers so no caller breaks.

- [ ] **Step 1: Failing tests**
  - `merged_last_scan_sums_across_sources`: create characters A and B, set `B.merged_into = A` (use the helper the existing merge tests use), `upsert_kill_scan` one Rat kill on each, assert `get_kills_merged_scoped(A, LastScan)` shows `killed_count == 2`.
  - `reset_log_data_clears_scan_tables`: write scan rows, call `reset_log_data()`, assert both shadow tables empty; same for `delete_all_data()`.
- [ ] **Step 2: Run** — `cargo test -p amanuensis-core --target aarch64-apple-darwin merged_last_scan reset_log_data_clears_scan` — Expected: FAIL.
- [ ] **Step 3: Implement** — in `merge.rs` replace the hardcoded `FROM kills` / `FROM kills k2` with `FROM {table}` / `{table} k2` (table is only ever one of two string literals chosen by `ScanScope`, never user input). Same for trainers. In `reset_log_data` and `delete_all_data` add `DELETE FROM scan_kills; DELETE FROM scan_trainers; DELETE FROM db_meta WHERE key = 'last_scan_token';`.
- [ ] **Step 4: Run** the same tests plus `cargo test -p amanuensis-core --target aarch64-apple-darwin merge` — Expected: PASS.
- [ ] **Step 5: Commit** — `feat(core): merged last-scan queries; reset clears shadow tables`

---

### Task 3: Parser dual-write with one token per scan

**Files:**
- Modify: `crates/amanuensis-core/src/parser/mod.rs` — call sites at ~710, 719, 729 (kills), 748 (ranks), 1017 (apply-learning); scan entry points `scan_folder*`, `scan_recursive*`, `scan_files*`, `scan_sources` (~1719)
- Test: `parser/mod.rs` tests module (near `update_sources_picks_up_appends_without_resetting_or_double_counting`, ~4210)

**Interfaces:**
- Consumes: `begin_scan_token`, `mark_scan_write`, `upsert_*_scan` (Task 1).
- Produces: `LogParser` gains a field `scan_token: Cell<Option<i64>>` and a private `fn note_scan_write(&self) -> Result<()>` that lazily calls `begin_scan_token` (if `None`) and `mark_scan_write`. Public scan entry points reset `scan_token` to `None` on entry **only when called directly**; `scan_sources` resets it once, then its inner folder scans must not reset it (add an inner flag or have the public wrappers call a `_inner` variant, mirroring the existing `scan_folder_inner` pattern).

- [ ] **Step 1: Failing tests**
  1. `last_scan_contains_only_appended_tail`: build a temp char folder with one log containing 2 Rat kills, `update_sources`; append 1 more Rat kill line to the same file, `update_sources` again; assert `get_kills_merged_scoped(id, LastScan)` shows `killed_count == 1` while `All` shows 3. (Copy the file/line fixtures from the neighbouring `update_sources_picks_up_appends…` test.)
  2. `noop_update_keeps_previous_last_scan`: after the above, run `update_sources` a third time (nothing new); assert LastScan still shows 1.
  3. `multi_source_update_is_one_last_scan`: two source folders each with one new kill; one `update_sources` call; LastScan shows both.
  4. `rescan_last_scan_equals_all`: `rescan_sources` over the fixtures; LastScan totals equal All totals.
- [ ] **Step 2: Run** — `cargo test -p amanuensis-core --target aarch64-apple-darwin last_scan` — Expected: FAIL.
- [ ] **Step 3: Implement** — at each of the four sites, immediately after the existing `upsert_*` call add `self.note_scan_write()?; self.db.upsert_*_scan(<same args>)?;`. For the creature-kill sites keep the `upsert_kill_hourly` call untouched. Add the token field and `note_scan_write`. Ensure `scan_token` is cleared at the start of each top-level scan operation (one token per user-visible scan), not per file.
- [ ] **Step 4: Run** — `cargo test -p amanuensis-core --target aarch64-apple-darwin` — Expected: all PASS (existing count 402+ plus new).
- [ ] **Step 5: Commit** — `feat(parser): record each scan's kills and ranks for last-scan view`

---

### Task 4: Tauri commands and CLI flags

**Files:**
- Modify: `crates/amanuensis-gui/src/commands/data.rs:12-22`, `crates/amanuensis-gui/ui/src/lib/commands.ts` (the `getKills`/`getTrainers` wrappers)
- Modify: `crates/amanuensis-cli/src/main.rs` — `Kills` (~82) and `Trainers` (~105) variants, their handlers (~432 and the trainers handler, uses `get_trainers_merged` at ~994)
- Test: CLI clap smoke tests (the 9 existing parse tests, same module)

**Interfaces:**
- Consumes: `get_kills_merged_scoped`, `get_trainers_merged_scoped`.
- Produces: Tauri `get_kills(char_id, scope: Option<ScanScope>)`, `get_trainers(char_id, scope: Option<ScanScope>)` (None == All, so existing callers are unaffected); CLI `kills <name> --last-scan`, `trainers <name> --last-scan`. Table output gets a first line `(last scan only)` when the flag is set.

- [ ] **Step 1: Failing tests** — add clap tests: `kills Fen --last-scan` parses with `last_scan == true`; `trainers Fen --last-scan` likewise; default false.
- [ ] **Step 2: Run** — `cargo test -p amanuensis-cli --target aarch64-apple-darwin` — Expected: FAIL.
- [ ] **Step 3: Implement** — add `#[arg(long)] last_scan: bool` to both variants, map to `ScanScope::LastScan`/`All`, call the scoped getters. Do NOT change `kills --format csv` export or the GUI `export_kills` command; they stay full-table (document this). Update `commands.ts` signatures with an optional `scope` arg.
- [ ] **Step 4: Run** — `cargo test -p amanuensis-cli --target aarch64-apple-darwin && cargo check -p amanuensis-gui --target aarch64-apple-darwin` — Expected: PASS.
- [ ] **Step 5: Commit** — `feat: scope arg for get_kills/get_trainers and --last-scan CLI flag`

---

### Task 5: GUI toggle

**Files:**
- Modify: `crates/amanuensis-gui/ui/src/lib/store.ts` (state + persisted setting; follow the `TRAINERS_ALPHA_VIEW` localStorage pattern at ~303), `lib/constants.ts` (`STORAGE_KEYS`), `components/views/KillsView.tsx`, `components/views/TrainersView.tsx`, and the data loader in `lib/hooks/useDatabase.ts` / `SummaryView.tsx`'s `refresh` that populates `kills`/`trainers`.

**Interfaces:**
- Consumes: Tauri `get_kills`/`get_trainers` with `scope` (Task 4).
- Produces: store field `dataScope: "all" | "last_scan"` + `setDataScope`. A two-option segmented control "All data | Last scan" at the top of Kills and Trainers. Summary, Ranger, Fighter, CV Graph keep using ALL data (they must keep requesting `scope` undefined) so derived stats are never computed from a partial set.

- [ ] **Step 1:** Make the loader that fills the shared `kills`/`trainers` store always fetch ALL; add separate local state in `KillsView` and `TrainersView` that fetches scoped data only when `dataScope === "last_scan"` (re-fetch on character change and after `finishScan`). This keeps the whole app's derived stats on full data.
- [ ] **Step 2:** Add the segmented control and empty state ("Nothing found by the last scan yet. Run Update Logs.").
- [ ] **Step 3:** Typecheck with the dev watcher stopped: `cd crates/amanuensis-gui/ui && npx tsc --noEmit` — Expected: no errors.
- [ ] **Step 4:** Manual check via `/run`: scan a folder, append a kill line to a log, Update Logs, flip the toggle; Kills shows only the new kill, Trainers `modified_ranks` not leaking. Rescan: toggle shows everything.
- [ ] **Step 5: Commit** — `feat(ui): All data / Last scan toggle on Kills and Ranks`

---

### Task 6: Docs

**Files:** Modify `CLAUDE.md` (add item under Key Functional Areas), no code.

- [ ] **Step 1:** Add a short item: shadow tables, latest-scan-only, no-op scans keep previous, Rescan => last == all, ranks exclude modified, `--last-scan` flags, export intentionally unscoped, existing DBs start empty until next scan. Bump the test count line.
- [ ] **Step 2:** `cargo test -p amanuensis-core --target aarch64-apple-darwin && cargo test -p amanuensis-cli --target aarch64-apple-darwin` — Expected: PASS; update the counts in CLAUDE.md from the real output.
- [ ] **Step 3: Commit** — `docs: describe last-scan view`

---

## Self-Review

- **Coverage:** toggle for ranks + creatures (Tasks 4-5), CLI parity (4), modified-ranks leak (1), merge sources (2), reset (2), tail scans / no-op / multi-source / rescan (3), docs (6).
- **Placeholder scan:** none; the schema step intentionally says "copy live DDL + ALTER-added columns" because the exact column list must come from `schema.rs` at implementation time.
- **Type consistency:** `ScanScope`, `begin_scan_token`, `mark_scan_write`, `upsert_kill_scan`, `upsert_trainer_rank_scan`, `upsert_apply_learning_scan`, `get_*_scoped`, `get_*_merged_scoped` used identically across tasks.
- **Risk:** Task 1's table-parametrised refactor of `upsert_kill` touches the hottest scan path; the unchanged existing tests are the safety net. Table names are interpolated only from two internal literals.
