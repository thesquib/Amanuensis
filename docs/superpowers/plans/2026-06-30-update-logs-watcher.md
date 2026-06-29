# Update Logs — Incremental Processing with File Watcher — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a lightweight "Update Logs (N)" action that incrementally processes new/grown logs across all configured sources without a full reset, plus a live filesystem watcher (toggleable, on by default) that keeps the badge count current.

**Architecture:** A new metadata-only core function `pending_files` mirrors the scanner's skip decisions to count what an incremental scan would touch. A new core method `update_sources` runs the existing incremental scan over all sources *without* `reset_log_data` (DRY-shared with the existing `rescan_sources` via a private `scan_sources` helper). Three new Tauri commands (`get_pending_log_count`, `update_logs`, `start_log_watcher`/`stop_log_watcher`) expose this. The watcher uses the `notify` crate on a background thread, debounces FS events, recomputes pending, and emits a `pending-changed` event. The frontend seeds the badge on DB-open (launch reconcile), updates it from the event, and processes via the new button.

**Tech Stack:** Rust (`amanuensis-core`, `amanuensis-gui` / Tauri 2), `notify` crate (new dep), `rusqlite`, React + Zustand + TypeScript, `@tauri-apps/api`.

## Global Constraints

- Spec: `docs/superpowers/specs/2026-06-30-update-logs-watcher-design.md`.
- **Key invariant:** badge `N` = number of files an incremental scan would touch *right now* (new files + grown files). Never overstate.
- **No auto-ingest.** Data only changes on explicit Update Logs / Rescan / Scan actions.
- **Do not change** `rescan_logs` / `rescan_sources` observable behavior (it must still reset then rescan).
- `pending_files` is **metadata-only** — it must NOT read file contents.
- Legacy `byte_len = 0` rows: appended content is NOT counted/scanned (mirrors existing scanner behavior). New files still count.
- Watcher toggle (`watchLogsEnabled`) defaults **on**; persisted in `localStorage` mirroring `indexLogLines` (`!== "false"` default-on pattern).
- **Placement refinement vs spec:** the spec said "below the deep-scan toggle in `SourcesDialog`"; the actual global Deep-scan checkbox lives in `Sidebar.tsx` (lines 59–62). Place the watcher toggle directly below it in `Sidebar.tsx`. This honors the user's "below Deep Scan" intent.
- Rust edition 2021. Match existing error style (`map_err(|e| e.to_string())` at the Tauri boundary, `crate::Result` in core).
- Run all core tests with: `cargo test -p amanuensis-core`. Build the GUI backend with: `cargo build -p amanuensis-gui`. Build the frontend with: `cd crates/amanuensis-gui/ui && npm run build`.

---

## Task 1: Core `pending_files` — metadata-only pending detector

**Files:**
- Modify: `crates/amanuensis-core/src/parser/mod.rs` (add free functions near `discover_log_folders`, ~line 1565; add tests in the existing `#[cfg(test)]` module, ~line 1660+)
- Modify: `crates/amanuensis-core/src/lib.rs:16` (re-export)

**Interfaces:**
- Consumes: `Database::get_log_scan_state(&self, file_path: &str) -> Result<Option<(i64, String)>>` (existing, `db/queries/log_file.rs`); private `find_log_files(dir: &Path) -> Result<Vec<PathBuf>>` and `pub fn discover_log_folders(root: &Path) -> Vec<PathBuf>` (same module).
- Produces: `pub fn pending_files(db: &Database, sources: &[(PathBuf, bool)]) -> Result<Vec<PathBuf>>` and private `fn source_log_files(root: &Path, recursive: bool) -> Vec<PathBuf>`. Re-exported as `amanuensis_core::parser::pending_files`.

- [ ] **Step 1: Write the failing test**

Add to the `#[cfg(test)] mod tests` block in `crates/amanuensis-core/src/parser/mod.rs` (it already has `create_test_log_dir`, `Database`, `LogParser`, `fs`, `PathBuf` in scope):

```rust
#[test]
fn pending_files_detects_new_grown_unchanged_shrank_and_legacy() {
    use super::pending_files;

    let (tmp, char_dir) = create_test_log_dir();
    let log_path = char_dir.join("CL Log 2024-01-01 13.00.00.txt");
    let initial = "\
1/1/24 1:00:00p Welcome to Clan Lord, TestChar!
1/1/24 1:01:00p You slaughtered a Rat.
";
    fs::write(&log_path, initial).unwrap();

    let db = Database::open_in_memory().unwrap();

    // Sources: the char_dir scanned non-recursively.
    let sources = vec![(char_dir.clone(), false)];

    // (a) Brand-new file, never scanned -> pending.
    let pend = pending_files(&db, &sources).unwrap();
    assert_eq!(pend, vec![log_path.clone()], "new file should be pending");

    // Scan it so it is recorded with byte_len > 0.
    let parser = LogParser::new(db).unwrap();
    parser.scan_folder(tmp.path(), false).unwrap();
    let db = parser.into_db();

    // (b) Unchanged -> not pending.
    assert!(
        pending_files(&db, &sources).unwrap().is_empty(),
        "unchanged file should not be pending"
    );

    // (c) Grown (appended) -> pending.
    let appended = format!("{initial}1/1/24 2:01:00p You slaughtered a Rat.\n");
    fs::write(&log_path, &appended).unwrap();
    assert_eq!(
        pending_files(&db, &sources).unwrap(),
        vec![log_path.clone()],
        "grown file should be pending"
    );

    // (d) Shrank/rotated (smaller than recorded byte_len) -> not pending.
    fs::write(&log_path, "1/1/24 1:00:00p Welcome to Clan Lord, TestChar!\n").unwrap();
    assert!(
        pending_files(&db, &sources).unwrap().is_empty(),
        "shrunk file should not be pending"
    );

    // (e) Legacy byte_len = 0 with a grown file -> not pending.
    fs::write(&log_path, &appended).unwrap();
    db.conn()
        .execute("UPDATE log_files SET byte_len = 0", [])
        .unwrap();
    assert!(
        pending_files(&db, &sources).unwrap().is_empty(),
        "legacy byte_len=0 file should not be pending"
    );
}

#[test]
fn pending_files_ignores_non_cl_log_files() {
    use super::pending_files;
    let (_tmp, char_dir) = create_test_log_dir();
    fs::write(char_dir.join("notes.txt"), "not a log").unwrap();
    let db = Database::open_in_memory().unwrap();
    assert!(
        pending_files(&db, &vec![(char_dir, false)]).unwrap().is_empty(),
        "non 'CL Log ' files must be ignored"
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p amanuensis-core pending_files`
Expected: FAIL — `cannot find function 'pending_files'` (compile error).

- [ ] **Step 3: Write minimal implementation**

In `crates/amanuensis-core/src/parser/mod.rs`, directly after the `discover_log_folders` function (the `pub fn discover_log_folders` at ~line 1565), add:

```rust
/// Return the log files across `sources` that an incremental scan would touch right now:
/// brand-new files plus files that have grown since their last scan. Metadata-only — file
/// contents are never read. Mirrors `plan_file_scan`'s skip decisions:
///   - unchanged size       -> skipped
///   - shrank/rotated        -> skipped (deferred to a full rescan)
///   - legacy `byte_len == 0` -> skipped (matches the scanner's legacy behavior)
/// Imprecision: a new path whose content was already scanned at another path is reported as
/// pending here (dedup needs the file's bytes, which we don't read); the post-scan recount
/// corrects the badge. `sources` is `(root, recursive)` exactly like `rescan_sources`.
pub fn pending_files(
    db: &crate::db::Database,
    sources: &[(PathBuf, bool)],
) -> Result<Vec<PathBuf>> {
    let mut pending = Vec::new();
    for (root, recursive) in sources {
        for file in source_log_files(root, *recursive) {
            let path_str = file.to_string_lossy();
            match db.get_log_scan_state(&path_str)? {
                None => pending.push(file), // never-seen path
                Some((byte_len, _)) if byte_len > 0 => {
                    let size = match std::fs::metadata(&file) {
                        Ok(m) => m.len() as i64,
                        Err(_) => continue, // unreadable -> ignore
                    };
                    if size > byte_len {
                        pending.push(file); // grew -> tail scan
                    }
                    // size == byte_len: unchanged; size < byte_len: rotated -> skip
                }
                Some(_) => {} // legacy byte_len == 0 -> skip
            }
        }
    }
    Ok(pending)
}

/// Expand one `(root, recursive)` source into its `CL Log` files, mirroring how
/// `scan_recursive_with_progress` discovers log roots.
fn source_log_files(root: &Path, recursive: bool) -> Vec<PathBuf> {
    if recursive {
        let folders = discover_log_folders(root);
        if folders.is_empty() {
            find_log_files(root).unwrap_or_default()
        } else {
            folders
                .iter()
                .flat_map(|f| find_log_files(f).unwrap_or_default())
                .collect()
        }
    } else {
        find_log_files(root).unwrap_or_default()
    }
}
```

Then add the re-export to `crates/amanuensis-core/src/lib.rs:16` — change:

```rust
pub use parser::LogParser;
```

to:

```rust
pub use parser::{LogParser, pending_files};
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p amanuensis-core pending_files`
Expected: PASS (both tests).

- [ ] **Step 5: Run the full core suite (no regressions)**

Run: `cargo test -p amanuensis-core`
Expected: PASS (existing tests + the 2 new ones).

- [ ] **Step 6: Commit**

```bash
git add crates/amanuensis-core/src/parser/mod.rs crates/amanuensis-core/src/lib.rs
git commit -m "feat(core): add pending_files metadata-only incremental-scan detector"
```

---

## Task 2: Core `update_sources` — incremental scan without reset

**Files:**
- Modify: `crates/amanuensis-core/src/parser/mod.rs` (refactor `rescan_sources` ~lines 1374-1404; add `update_sources` + private `scan_sources`; add a test in the `#[cfg(test)]` module)

**Interfaces:**
- Consumes (existing): `scan_recursive_with_progress`, `scan_folder_with_progress`, `finalize_characters`, `db.reset_log_data`, `db.list_characters`, `ScanResult`.
- Produces: `pub fn update_sources<F>(&self, sources: &[(PathBuf, bool)], index_lines: bool, progress: F) -> Result<ScanResult> where F: Fn(usize, usize, &str)`. (Behaviorally identical to `rescan_sources` but **without** the `reset_log_data` call.)

- [ ] **Step 1: Write the failing test**

Add to the `#[cfg(test)] mod tests` block:

```rust
#[test]
fn update_sources_picks_up_appends_without_resetting_or_double_counting() {
    let (tmp, char_dir) = create_test_log_dir();
    let log_path = char_dir.join("CL Log 2024-01-01 13.00.00.txt");
    let initial = "\
1/1/24 1:00:00p Welcome to Clan Lord, TestChar!
1/1/24 1:01:00p You slaughtered a Rat.
";
    fs::write(&log_path, initial).unwrap();

    let db = Database::open_in_memory().unwrap();
    let parser = LogParser::new(db).unwrap();
    let sources = vec![(tmp.path().to_path_buf(), true)];

    // First full scan via update_sources.
    parser.update_sources(&sources, false, |_, _, _| {}).unwrap();
    let char = parser.db().get_character("Testchar").unwrap().unwrap();
    assert_eq!(char.logins, 1);
    let char_id = char.id.unwrap();
    assert_eq!(
        parser.db().get_kills(char_id).unwrap().iter().map(|k| k.slaughtered_count).sum::<i64>(),
        1
    );

    // Append a kill; update again.
    let appended = format!("{initial}1/1/24 2:01:00p You slaughtered a Rat.\n");
    fs::write(&log_path, &appended).unwrap();
    parser.update_sources(&sources, false, |_, _, _| {}).unwrap();

    let char = parser.db().get_character("Testchar").unwrap().unwrap();
    assert_eq!(char.logins, 1, "tail scan must not re-count the login");
    assert_eq!(
        parser.db().get_kills(char_id).unwrap().iter().map(|k| k.slaughtered_count).sum::<i64>(),
        2,
        "appended kill should be counted exactly once"
    );

    // No-op update keeps totals stable (no reset, no double-count).
    parser.update_sources(&sources, false, |_, _, _| {}).unwrap();
    let char = parser.db().get_character("Testchar").unwrap().unwrap();
    assert_eq!(char.logins, 1);
    assert_eq!(
        parser.db().get_kills(char_id).unwrap().iter().map(|k| k.slaughtered_count).sum::<i64>(),
        2
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p amanuensis-core update_sources`
Expected: FAIL — `no method named 'update_sources'`.

- [ ] **Step 3: Refactor `rescan_sources` and add `update_sources` + `scan_sources`**

Replace the existing `rescan_sources` (lines ~1374-1404) with these three methods (same `impl LogParser` block):

```rust
pub fn rescan_sources<F>(
    &self,
    sources: &[(std::path::PathBuf, bool)],
    index_lines: bool,
    progress: F,
) -> Result<ScanResult>
where
    F: Fn(usize, usize, &str),
{
    // An empty source list is a no-op: do not reset (which would wipe the DB).
    if sources.is_empty() {
        return Ok(ScanResult::default());
    }
    self.db.reset_log_data()?;
    self.scan_sources(sources, index_lines, progress)
}

/// Incrementally process all sources (force=false) WITHOUT resetting first. New files are
/// scanned (login counted); grown files are tail-scanned (login not re-counted); unchanged
/// files are skipped. Safe to call repeatedly.
pub fn update_sources<F>(
    &self,
    sources: &[(std::path::PathBuf, bool)],
    index_lines: bool,
    progress: F,
) -> Result<ScanResult>
where
    F: Fn(usize, usize, &str),
{
    if sources.is_empty() {
        return Ok(ScanResult::default());
    }
    self.scan_sources(sources, index_lines, progress)
}

/// Shared body for `rescan_sources` / `update_sources`: scan every source, finalize
/// characters, and report the combined `ScanResult`. Does NOT reset.
fn scan_sources<F>(
    &self,
    sources: &[(std::path::PathBuf, bool)],
    index_lines: bool,
    progress: F,
) -> Result<ScanResult>
where
    F: Fn(usize, usize, &str),
{
    let mut combined = ScanResult::default();
    for (path, recursive) in sources {
        let r = if *recursive {
            self.scan_recursive_with_progress(path, false, index_lines, &progress)?
        } else {
            self.scan_folder_with_progress(path, false, index_lines, &progress)?
        };
        combined.files_scanned += r.files_scanned;
        combined.skipped += r.skipped;
        combined.lines_parsed += r.lines_parsed;
        combined.events_found += r.events_found;
        combined.errors += r.errors;
    }
    self.finalize_characters()?;
    combined.characters = self.db.list_characters()?.len();
    Ok(combined)
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p amanuensis-core update_sources`
Expected: PASS.

- [ ] **Step 5: Run the full core suite (rescan path unchanged)**

Run: `cargo test -p amanuensis-core`
Expected: PASS (all tests, including any existing rescan tests).

- [ ] **Step 6: Commit**

```bash
git add crates/amanuensis-core/src/parser/mod.rs
git commit -m "feat(core): add update_sources (incremental, no reset); share scan_sources with rescan"
```

---

## Task 3: Tauri commands `get_pending_log_count` + `update_logs`

**Files:**
- Modify: `crates/amanuensis-gui/src/commands/scanning.rs` (add two commands)
- Modify: `crates/amanuensis-gui/src/main.rs:53` (register)
- Modify: `crates/amanuensis-gui/ui/src/lib/commands.ts` (add TS wrappers)

**Interfaces:**
- Consumes: `amanuensis_core::parser::pending_files` (Task 1), `LogParser::update_sources` (Task 2), `AppState::with_db` / `take_db`, `SourceSpec`, `ScanProgress`, `ScanResult` (all existing in scope of `scanning.rs` / `commands/mod.rs`).
- Produces (Rust): `get_pending_log_count(sources: Vec<SourceSpec>, state) -> Result<usize, String>`, `update_logs(sources: Vec<SourceSpec>, index_lines: bool, app, state) -> Result<ScanResult, String>`.
- Produces (TS): `getPendingLogCount(sources): Promise<number>`, `updateLogs(sources, indexLines?): Promise<ScanResult>`.

- [ ] **Step 1: Add the two commands to `scanning.rs`**

Append to `crates/amanuensis-gui/src/commands/scanning.rs` (it already imports `Emitter`, `State`, `LogParser`, `ScanProgress`, `ScanResult`, `SourceSpec`, `PathBuf`, `AppState` via the module — match the imports `rescan_logs` already uses; add `use std::path::PathBuf;` at the top only if not already present):

```rust
/// Count the log files an incremental Update would touch right now (new + grown), without
/// scanning. Metadata-only. Returns 0 when no database is open is treated as an error by
/// `with_db`; callers should only invoke this once a DB is open.
#[tauri::command]
pub async fn get_pending_log_count(
    sources: Vec<SourceSpec>,
    state: State<'_, AppState>,
) -> Result<usize, String> {
    let folders: Vec<(PathBuf, bool)> = sources
        .into_iter()
        .map(|s| (PathBuf::from(s.path), s.recursive))
        .collect();
    state.with_db(|db| {
        amanuensis_core::parser::pending_files(db, &folders)
            .map(|v| v.len())
            .map_err(|e| e.to_string())
    })
}

/// Incrementally process new/grown logs across all configured sources WITHOUT resetting.
#[tauri::command]
pub async fn update_logs(
    sources: Vec<SourceSpec>,
    index_lines: bool,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<ScanResult, String> {
    let db = state.take_db()?;
    let state_db = state.db.clone();
    let folders: Vec<(PathBuf, bool)> = sources
        .into_iter()
        .map(|s| (PathBuf::from(s.path), s.recursive))
        .collect();

    let result = tauri::async_runtime::spawn_blocking(move || {
        let parser = LogParser::new(db).map_err(|e| e.to_string())?;
        let app_handle = app.clone();
        let progress_cb = |current: usize, total: usize, filename: &str| {
            let _ = app_handle.emit(
                "scan-progress",
                ScanProgress {
                    current_file: current,
                    total_files: total,
                    filename: filename.to_string(),
                },
            );
        };
        let result = parser
            .update_sources(&folders, index_lines, progress_cb)
            .map_err(|e| e.to_string())?;
        *state_db.lock().map_err(|e| format!("Lock poisoned: {e}"))? = Some(parser.into_db());
        Ok(result)
    })
    .await
    .map_err(|e| e.to_string())?;

    result
}
```

- [ ] **Step 2: Register the commands**

In `crates/amanuensis-gui/src/main.rs`, inside `tauri::generate_handler![ ... ]`, after `commands::scan_files,` (line 30) add:

```rust
            commands::update_logs,
            commands::get_pending_log_count,
```

- [ ] **Step 3: Verify the backend compiles**

Run: `cargo build -p amanuensis-gui`
Expected: builds with no errors. (If `PathBuf` is reported unresolved in `scanning.rs`, add `use std::path::PathBuf;` at the top of that file.)

- [ ] **Step 4: Add TS wrappers**

In `crates/amanuensis-gui/ui/src/lib/commands.ts`, after the existing `rescanLogs` wrapper (~line 153), add:

```typescript
export async function getPendingLogCount(
  sources: { path: string; recursive: boolean }[],
): Promise<number> {
  return invoke("get_pending_log_count", { sources });
}

export async function updateLogs(
  sources: { path: string; recursive: boolean }[],
  indexLines: boolean = true,
): Promise<ScanResult> {
  return invoke("update_logs", { sources, indexLines });
}
```

- [ ] **Step 5: Verify the frontend type-checks**

Run: `cd crates/amanuensis-gui/ui && npm run build`
Expected: build succeeds (TypeScript compiles).

- [ ] **Step 6: Commit**

```bash
git add crates/amanuensis-gui/src/commands/scanning.rs crates/amanuensis-gui/src/main.rs crates/amanuensis-gui/ui/src/lib/commands.ts
git commit -m "feat(gui): add update_logs + get_pending_log_count commands and TS wrappers"
```

---

## Task 4: File watcher backend (`notify`) + start/stop commands

**Files:**
- Modify: `crates/amanuensis-gui/Cargo.toml` (add `notify` dep)
- Modify: `crates/amanuensis-gui/src/state.rs` (add `log_watcher` field)
- Create: `crates/amanuensis-gui/src/commands/watcher.rs`
- Modify: `crates/amanuensis-gui/src/commands/mod.rs` (declare + re-export module)
- Modify: `crates/amanuensis-gui/src/main.rs` (register commands)
- Modify: `crates/amanuensis-gui/ui/src/lib/commands.ts` (TS wrappers)

**Interfaces:**
- Consumes: `amanuensis_core::parser::pending_files`, `AppState.db` (`Arc<Mutex<Option<Database>>>`), `SourceSpec`.
- Produces (Rust): `start_log_watcher(sources: Vec<SourceSpec>, app, state) -> Result<(), String>`, `stop_log_watcher(state) -> Result<(), String>`. Emits Tauri event `"pending-changed"` with a `usize` payload.
- Produces (TS): `startLogWatcher(sources): Promise<void>`, `stopLogWatcher(): Promise<void>`.

- [ ] **Step 1: Add the `notify` dependency**

In `crates/amanuensis-gui/Cargo.toml`, under `[dependencies]`, add:

```toml
notify = "6"
```

- [ ] **Step 2: Add the watcher slot to `AppState`**

In `crates/amanuensis-gui/src/state.rs`:

Add the field to the struct (after `db_path`):

```rust
    /// Live filesystem watcher for log sources; `Some` while watching. Dropping it stops
    /// the watcher and its worker thread (the event channel closes).
    pub log_watcher: Mutex<Option<notify::RecommendedWatcher>>,
```

And initialize it in `AppState::new()`:

```rust
            log_watcher: Mutex::new(None),
```

- [ ] **Step 3: Create the watcher command module**

Create `crates/amanuensis-gui/src/commands/watcher.rs`:

```rust
use std::path::PathBuf;
use std::time::Duration;

use notify::{RecursiveMode, Watcher};
use tauri::{Emitter, State};

use super::SourceSpec;
use crate::state::AppState;

/// Debounce window for coalescing bursts of filesystem events before recomputing.
const DEBOUNCE: Duration = Duration::from_millis(1500);

/// Start (or restart) the live filesystem watcher over `sources`. On each debounced batch of
/// events it recomputes the pending count and emits `"pending-changed"` (usize). Replacing
/// the stored watcher drops the previous one, which closes its channel and ends its worker.
#[tauri::command]
pub fn start_log_watcher(
    sources: Vec<SourceSpec>,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let folders: Vec<(PathBuf, bool)> = sources
        .into_iter()
        .map(|s| (PathBuf::from(s.path), s.recursive))
        .collect();

    let (tx, rx) = std::sync::mpsc::channel();
    let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        let _ = tx.send(res);
    })
    .map_err(|e| e.to_string())?;

    for (path, recursive) in &folders {
        let mode = if *recursive {
            RecursiveMode::Recursive
        } else {
            RecursiveMode::NonRecursive
        };
        if let Err(e) = watcher.watch(path, mode) {
            log::warn!("log watcher: failed to watch {}: {e}", path.display());
        }
    }

    let db_arc = state.db.clone();
    let folders_for_thread = folders.clone();
    std::thread::spawn(move || {
        // Block until an event; then drain the burst within the debounce window.
        while rx.recv().is_ok() {
            while rx.recv_timeout(DEBOUNCE).is_ok() {}
            let count = match db_arc.lock() {
                Ok(guard) => match guard.as_ref() {
                    // A scan in progress takes the DB out of state (None) — skip this tick.
                    Some(db) => amanuensis_core::parser::pending_files(db, &folders_for_thread)
                        .map(|v| v.len())
                        .ok(),
                    None => None,
                },
                Err(_) => None,
            };
            if let Some(c) = count {
                let _ = app.emit("pending-changed", c);
            }
        }
    });

    *state
        .log_watcher
        .lock()
        .map_err(|e| format!("Lock poisoned: {e}"))? = Some(watcher);
    Ok(())
}

/// Stop the live filesystem watcher (drops it; its worker thread exits when the channel
/// closes). No-op if not running.
#[tauri::command]
pub fn stop_log_watcher(state: State<'_, AppState>) -> Result<(), String> {
    *state
        .log_watcher
        .lock()
        .map_err(|e| format!("Lock poisoned: {e}"))? = None;
    Ok(())
}
```

- [ ] **Step 4: Declare and re-export the module**

In `crates/amanuensis-gui/src/commands/mod.rs`:

Add after `mod bestiary;` (line 8):

```rust
mod watcher;
```

Add after `pub use bestiary::*;` (line 18):

```rust
pub use watcher::*;
```

- [ ] **Step 5: Register the commands**

In `crates/amanuensis-gui/src/main.rs`, inside `generate_handler![ ... ]`, after the two commands added in Task 3, add:

```rust
            commands::start_log_watcher,
            commands::stop_log_watcher,
```

- [ ] **Step 6: Verify the backend compiles**

Run: `cargo build -p amanuensis-gui`
Expected: builds with no errors (downloads `notify` on first build).

- [ ] **Step 7: Add TS wrappers**

In `crates/amanuensis-gui/ui/src/lib/commands.ts`, after the wrappers from Task 3, add:

```typescript
export async function startLogWatcher(
  sources: { path: string; recursive: boolean }[],
): Promise<void> {
  return invoke("start_log_watcher", { sources });
}

export async function stopLogWatcher(): Promise<void> {
  return invoke("stop_log_watcher");
}
```

- [ ] **Step 8: Verify the frontend type-checks**

Run: `cd crates/amanuensis-gui/ui && npm run build`
Expected: build succeeds.

- [ ] **Step 9: Commit**

```bash
git add crates/amanuensis-gui/Cargo.toml crates/amanuensis-gui/Cargo.lock crates/amanuensis-gui/src/state.rs crates/amanuensis-gui/src/commands/watcher.rs crates/amanuensis-gui/src/commands/mod.rs crates/amanuensis-gui/src/main.rs crates/amanuensis-gui/ui/src/lib/commands.ts
git commit -m "feat(gui): add notify-based log file watcher with start/stop commands"
```

---

## Task 5: Frontend store — `watchLogsEnabled` setting + `pendingLogCount` state

**Files:**
- Modify: `crates/amanuensis-gui/ui/src/lib/constants.ts` (add storage key)
- Modify: `crates/amanuensis-gui/ui/src/lib/store.ts` (add slices)

**Interfaces:**
- Produces (Zustand store): `watchLogsEnabled: boolean`, `setWatchLogsEnabled: (v: boolean) => void`, `pendingLogCount: number`, `setPendingLogCount: (n: number) => void`.

- [ ] **Step 1: Add the storage key**

In `crates/amanuensis-gui/ui/src/lib/constants.ts`, inside the `STORAGE_KEYS` object (after `LOG_SOURCES`), add:

```typescript
  WATCH_LOGS: "amanuensis_watch_logs",
```

- [ ] **Step 2: Declare the store slices (interface)**

In `crates/amanuensis-gui/ui/src/lib/store.ts`, in the store state interface near `indexLogLines: boolean; setIndexLogLines: ...` (lines ~123-124), add:

```typescript
  watchLogsEnabled: boolean;
  setWatchLogsEnabled: (v: boolean) => void;
  pendingLogCount: number;
  setPendingLogCount: (n: number) => void;
```

- [ ] **Step 3: Implement the slices**

In the `create(...)` body near the `indexLogLines:` / `setIndexLogLines:` implementation (lines ~241-245), add (mirroring the `!== "false"` default-on pattern):

```typescript
  watchLogsEnabled: localStorage.getItem(STORAGE_KEYS.WATCH_LOGS) !== "false",
  setWatchLogsEnabled: (v) => {
    localStorage.setItem(STORAGE_KEYS.WATCH_LOGS, String(v));
    set({ watchLogsEnabled: v });
  },
  pendingLogCount: 0,
  setPendingLogCount: (n) => set({ pendingLogCount: n }),
```

- [ ] **Step 4: Verify the frontend type-checks**

Run: `cd crates/amanuensis-gui/ui && npm run build`
Expected: build succeeds.

- [ ] **Step 5: Commit**

```bash
git add crates/amanuensis-gui/ui/src/lib/constants.ts crates/amanuensis-gui/ui/src/lib/store.ts
git commit -m "feat(gui): add watchLogsEnabled setting and pendingLogCount store state"
```

---

## Task 6: `useLogWatcher` hook — launch reconcile + watcher lifecycle + event

**Files:**
- Create: `crates/amanuensis-gui/ui/src/lib/hooks/useLogWatcher.ts`
- Modify: `crates/amanuensis-gui/ui/src/components/layout/AppShell.tsx` (mount the hook)

**Interfaces:**
- Consumes: store (`dbPath`, `sources`, `watchLogsEnabled`, `setPendingLogCount`, `isScanning`), `getPendingLogCount`, `startLogWatcher`, `stopLogWatcher` (Tasks 3-4), `listen` from `@tauri-apps/api/event`.
- Produces: `useLogWatcher(): void` — side-effect hook, no return.

- [ ] **Step 1: Create the hook**

Create `crates/amanuensis-gui/ui/src/lib/hooks/useLogWatcher.ts`:

```typescript
import { useEffect } from "react";
import { listen } from "@tauri-apps/api/event";
import { useStore } from "../store";
import { getPendingLogCount, startLogWatcher, stopLogWatcher } from "../commands";

/**
 * Keeps the "pending logs" badge accurate:
 *  - on DB open / source change: one cheap reconcile pass (catches changes made while
 *    the app was closed),
 *  - while the watcher toggle is on: a live `pending-changed` event stream.
 * Never ingests data — only counts.
 */
export function useLogWatcher() {
  const dbPath = useStore((s) => s.dbPath);
  const sources = useStore((s) => s.sources);
  const watchLogsEnabled = useStore((s) => s.watchLogsEnabled);
  const setPendingLogCount = useStore((s) => s.setPendingLogCount);

  // Live event from the backend watcher.
  useEffect(() => {
    const unlisten = listen<number>("pending-changed", (event) => {
      setPendingLogCount(event.payload);
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, [setPendingLogCount]);

  // Launch reconcile: recompute whenever DB or sources change.
  useEffect(() => {
    if (!dbPath || sources.length === 0) {
      setPendingLogCount(0);
      return;
    }
    let cancelled = false;
    getPendingLogCount(sources)
      .then((n) => {
        if (!cancelled) setPendingLogCount(n);
      })
      .catch((e) => console.error("pending count failed:", e));
    return () => {
      cancelled = true;
    };
  }, [dbPath, sources, setPendingLogCount]);

  // Watcher lifecycle: start when enabled + DB + sources; stop otherwise.
  useEffect(() => {
    if (watchLogsEnabled && dbPath && sources.length > 0) {
      startLogWatcher(sources).catch((e) => console.error("start watcher failed:", e));
      return () => {
        stopLogWatcher().catch(() => {});
      };
    }
    stopLogWatcher().catch(() => {});
  }, [watchLogsEnabled, dbPath, sources]);
}
```

- [ ] **Step 2: Mount the hook in `AppShell`**

In `crates/amanuensis-gui/ui/src/components/layout/AppShell.tsx`:

Add the import near the other lib imports (after line 4):

```typescript
import { useLogWatcher } from "../../lib/hooks/useLogWatcher";
```

Call it inside the `AppShell` component body, immediately after the `useStore(...)` destructure (~line 98):

```typescript
  useLogWatcher();
```

- [ ] **Step 3: Verify the frontend type-checks**

Run: `cd crates/amanuensis-gui/ui && npm run build`
Expected: build succeeds.

- [ ] **Step 4: Commit**

```bash
git add crates/amanuensis-gui/ui/src/lib/hooks/useLogWatcher.ts crates/amanuensis-gui/ui/src/components/layout/AppShell.tsx
git commit -m "feat(gui): add useLogWatcher hook (launch reconcile + live pending-changed badge)"
```

---

## Task 7: `handleUpdateLogs` in `useScan`

**Files:**
- Modify: `crates/amanuensis-gui/ui/src/lib/hooks/useScan.ts`

**Interfaces:**
- Consumes: `updateLogs`, `getPendingLogCount` (Task 3), store (`sources`, `indexLogLines`, `setPendingLogCount`, `setIsScanning`, `setScanProgress`), `finishScan` (existing).
- Produces: `handleUpdateLogs: () => Promise<void>` added to the hook's return object.

- [ ] **Step 1: Import the new commands**

In `crates/amanuensis-gui/ui/src/lib/hooks/useScan.ts`, extend the import on line 5:

```typescript
import { scanLogs, rescanLogs, scanFiles, updateLogs, getPendingLogCount, listCharacters, getScannedLogCount, getLogLineCount, getProcessLogs } from "../commands";
```

- [ ] **Step 2: Add the handler**

Add `setPendingLogCount` to the `useStore()` destructure (near `setProcessLogs`), then add this `useCallback` after `handleRescanLogs` (before the `return {`):

```typescript
  const handleUpdateLogs = useCallback(async () => {
    if (sources.length === 0) return;
    setIsScanning(true);
    setScanProgress(null);
    try {
      await updateLogs(sources, indexLogLines);
      await finishScan();
      const pending = await getPendingLogCount(sources);
      setPendingLogCount(pending);
    } catch (e) {
      console.error("Update logs failed:", e);
    } finally {
      setIsScanning(false);
      setScanProgress(null);
    }
  }, [sources, indexLogLines, setIsScanning, setScanProgress, finishScan, setPendingLogCount]);
```

- [ ] **Step 3: Export it from the hook**

Add `handleUpdateLogs,` to the returned object (the `return { ... }` near line 113).

- [ ] **Step 4: Verify the frontend type-checks**

Run: `cd crates/amanuensis-gui/ui && npm run build`
Expected: build succeeds.

- [ ] **Step 5: Commit**

```bash
git add crates/amanuensis-gui/ui/src/lib/hooks/useScan.ts
git commit -m "feat(gui): add handleUpdateLogs (incremental process + pending recount)"
```

---

## Task 8: Sidebar UI — "Update Logs (N)" button + watcher toggle

**Files:**
- Modify: `crates/amanuensis-gui/ui/src/components/layout/Sidebar.tsx`

**Interfaces:**
- Consumes: store (`pendingLogCount`, `watchLogsEnabled`, `setWatchLogsEnabled`, `sources`), `handleUpdateLogs` (Task 7), `isScanning`.

- [ ] **Step 1: Pull the new store values + handler**

In `Sidebar.tsx`, extend the `useStore()` destructure on line 14 to include `pendingLogCount, watchLogsEnabled, setWatchLogsEnabled`:

```typescript
  const { dbPath, sources, scannedLogCount, recursiveScan, setRecursiveScan, indexLogLines, setIndexLogLines, theme, setTheme, characters, setCharacters, pendingLogCount, watchLogsEnabled, setWatchLogsEnabled } = useStore();
```

Extend the `useScan(...)` destructure on line 22 to include `handleUpdateLogs`:

```typescript
  const { scanProgress, handleScanFolder, handleScanFiles, handleRescanLogs, handleUpdateLogs } = useScan(
```

- [ ] **Step 2: Add the "Update Logs" button**

In `Sidebar.tsx`, immediately after the Deep-scan `<label>` block (after line 62, before the Advanced section comment), add the primary Update button with its count badge:

```typescript
        <button
          onClick={handleUpdateLogs}
          disabled={isScanning || sources.length === 0}
          className="flex items-center justify-center gap-2 rounded bg-[var(--color-accent)]/90 px-3 py-1.5 text-sm font-medium text-white hover:bg-[var(--color-accent)]/70 disabled:opacity-50"
          title={sources.length > 0 ? "Process new and updated logs from your sources (incremental — no reset)" : "No log sources yet — scan a folder first"}
        >
          Update Logs
          {pendingLogCount > 0 && (
            <span className="rounded-full bg-white/25 px-1.5 py-0.5 text-xs font-semibold leading-none">
              {pendingLogCount}
            </span>
          )}
        </button>
```

- [ ] **Step 3: Add the watcher toggle below Deep scan**

In `Sidebar.tsx`, immediately after the Deep-scan `<label>...</label>` (after line 62) and after the Update button from Step 2, add:

```typescript
        <label className="flex items-center gap-1.5 text-xs text-[var(--color-text-muted)]">
          <input type="checkbox" checked={watchLogsEnabled} onChange={(e) => setWatchLogsEnabled(e.target.checked)} className="accent-[var(--color-accent)]" />
          Watch log folders for changes
        </label>
```

- [ ] **Step 4: Verify the frontend type-checks**

Run: `cd crates/amanuensis-gui/ui && npm run build`
Expected: build succeeds.

- [ ] **Step 5: Commit**

```bash
git add crates/amanuensis-gui/ui/src/components/layout/Sidebar.tsx
git commit -m "feat(gui): add Update Logs button with pending badge and watcher toggle"
```

---

## Task 9: Manual end-to-end verification + docs

**Files:**
- Modify: `CLAUDE.md` (document the feature under Key Functional Areas)

**Interfaces:** none (verification + docs).

- [ ] **Step 1: Build everything**

Run: `cargo build -p amanuensis-gui && cargo test -p amanuensis-core`
Expected: backend builds; all core tests pass.

- [ ] **Step 2: Manual smoke test (Tauri dev)**

> NOTE (from project memory): do not run `tsc`/`npm run build` in the `ui` tree while `cargo tauri dev` is running — it cycles the app. Stop the dev server before any build step.

Run the app (`cargo tauri dev` from `crates/amanuensis-gui`, or the project's usual run skill). Verify:
  1. Open a DB with at least one configured source → "Update Logs" appears; badge reflects any already-pending files (launch reconcile).
  2. With "Watch log folders for changes" ON, append a line to a `CL Log …` file in a source (e.g. `echo ... >> file`) → within ~2s the badge increments (the `pending-changed` event fired).
  3. Click "Update Logs" → progress bar runs, data refreshes (kills/logins updated), badge returns to 0. Logins NOT double-counted.
  4. Toggle the watcher OFF → appending no longer updates the badge live; toggle ON again → live updates resume. Setting persists across app restart.
  5. "Rescan Logs" still performs the full reset+rebuild (unchanged).

- [ ] **Step 3: Document the feature in `CLAUDE.md`**

Add a new numbered item under "## Key Functional Areas" (after item 10, "Kills export"):

```markdown
11. **Update Logs (incremental) + file watcher**: the sidebar has an **"Update Logs (N)"** button that incrementally processes new and grown logs across all configured sources WITHOUT a reset — it calls the core `update_sources` (which shares `scan_sources` with `rescan_sources` but skips `reset_log_data`), relying on offset-resume to tail-scan appended files and skip unchanged ones cheaply. The badge `N` is the count of files an incremental scan would touch right now (new + grown), computed by the metadata-only `pending_files` (`parser/mod.rs`) — it never reads file contents and mirrors `plan_file_scan`'s skip decisions (unchanged/shrank/legacy `byte_len=0` are not counted). The count is seeded on DB-open (launch reconcile via `get_pending_log_count`) and kept live by a `notify`-based filesystem watcher (`commands/watcher.rs`) that debounces FS events, recomputes pending, and emits `pending-changed`. A **"Watch log folders for changes"** toggle (sidebar, default on, persisted in `localStorage` under `amanuensis_watch_logs`) gates only the live watcher thread — launch reconcile and the Update button still work when it is off. No auto-ingest: data changes only when the user clicks Update Logs (or Scan/Rescan). Legacy DBs with `byte_len=0` rows do not count/scan appended content on those files until one full Rescan Logs (matches existing scanner behavior).
```

- [ ] **Step 4: Commit**

```bash
git add CLAUDE.md
git commit -m "docs: document Update Logs incremental processing + file watcher"
```

---

## Self-Review Notes

- **Spec coverage:** §1 pending predicate → Task 1; §2 launch reconcile + live watcher → Tasks 4 (backend) + 6 (hook); §3 watcher toggle → Tasks 5 + 8; §4 command surface → Tasks 3 + 4; §5 frontend button/badge → Tasks 7 + 8; legacy caveat → covered in `pending_files` logic (Task 1) + docs (Task 9); testing → Tasks 1-2 unit tests + Task 9 manual.
- **Placeholder scan:** none — all code blocks are complete.
- **Type/name consistency:** `pending_files(db, sources) -> Vec<PathBuf>`, `update_sources`, `get_pending_log_count`, `update_logs`, `start_log_watcher`/`stop_log_watcher`, event `"pending-changed"`, store `pendingLogCount`/`watchLogsEnabled`, storage key `WATCH_LOGS` — used identically across all tasks.
- **Known limitation surfaced:** `get_pending_log_count` does its fs-stat pass while holding (briefly) the DB lock / on the command thread; fine for local disks. Network-drive perf is a future optimization, not in scope.
</content>
</invoke>
