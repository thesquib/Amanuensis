# Update Logs — incremental processing with change detection

**Date:** 2026-06-30
**Status:** Design approved, pending spec review

## Problem

Active Clan Lord players generate new log files (and append to today's file) constantly.
Today the only GUI affordance is **"Scan All Logs"**, which calls `rescan_logs` — it
**wipes all log-derived data and rebuilds from scratch**. That is slow and overkill for
the common daily case of "I just played, pull in what's new."

The core scanner is already capable of cheap incremental work: `plan_file_scan` /
`ScanPlan` (offset-resume) picks up brand-new files, tail-scans files that grew (parsing
only appended bytes, without re-counting the login), skips unchanged files by a size
fast-path, and dedupes by content hash. That capability is simply not surfaced as its own
GUI action — `scan_logs(folder, force=false, recursive=true)` already does the right thing.

We want to (a) surface a lightweight **"Update Logs"** action, and (b) tell the user when
there is something to process, **without auto-ingesting**.

## Goals

- A new **"Update Logs (N)"** button that incrementally processes new/grown logs across
  all configured sources, without resetting existing data.
- A live filesystem watcher that keeps the badge count current while the app is open.
- An accurate badge the moment the app opens, reflecting changes made while it was closed.
- A user toggle to enable/disable the live watcher (default **on**).
- No auto-ingest: data only changes when the user clicks Update Logs (or Scan All Logs).

## Non-goals

- Auto-scanning / auto-ingesting on change.
- Fixing the legacy `byte_len = 0` appended-content limitation (documented caveat below).
- Changing the existing "Scan All Logs" (rescan/rebuild) behavior.
- CLI changes (incremental scan already exists via `scan --force=false`; out of scope).

## Key invariant

**The badge `N` equals the number of files an incremental scan would actually touch right
now** (new files + files that have grown). The badge never overstates what the button does.
After a successful Update Logs run the badge returns to 0.

## Design

### 1. Shared "what's pending?" predicate (core)

New core function, e.g. in a `pending` module under `amanuensis-core`:

```
fn pending_files(db: &Database, sources: &[LogSource]) -> Result<Vec<PathBuf>>
```

It performs a **metadata-only** pass (no byte reads): for each source, discover `CL Log …`
files using the **same discovery as scanning** (`find_log_files` / `discover_log_folders`,
honoring each source's `recursive` flag), then for each file decide via `fs::metadata` size
vs the stored `log_files.byte_len`:

| File state | Counted? | Rationale |
|---|---|---|
| Not in `log_files` (new) | yes | incremental scan will full-scan it (counts a login) |
| Known `byte_len > 0`, size **grew** | yes | tail-scan of appended bytes |
| size **unchanged** | no | fast-path skip |
| size **shrank** (rotated/truncated) | no | scanner defers to full rescan |
| legacy `byte_len = 0` | no | mirrors scanner's legacy skip (see caveat) |

This is the metadata-only mirror of `plan_file_scan`'s decision, so the count matches what
the scan touches. Known imprecision: a grown file whose already-scanned **prefix** changed
reads as "grew" here but resolves to `SkipChanged` on the real scan — badge may be off by
one until the post-scan recount corrects it. Acceptable.

This single function backs **both** launch-reconcile and the live watcher.

### 2. Detection: launch reconcile + live watcher

- **Launch reconcile (always runs):** on app boot, and whenever the source list changes,
  call `pending_files` once over all sources to seed the badge. This catches everything
  that changed while the app was closed — the dominant real-world case.
- **Live watcher (toggle-gated):** a Rust `notify` recommended-watcher on the configured
  source roots. On a **debounced** (~1–2s) batch of FS events, **recompute `pending_files`
  and `emit("pending-changed", count)`** to the frontend. No per-path dirty-set to drift —
  we just re-run the cheap pass. The watcher is (re)initialized when the source list
  changes. Unreadable/missing folders are logged and skipped.

### 3. Watcher toggle

- New persisted setting `watchLogsEnabled`, **default on**, mirroring `indexLogLines`:
  stored in `localStorage` under a new `STORAGE_KEYS.WATCH_LOGS` key, read with the same
  default-on pattern (`!== "false"`), with a `setWatchLogsEnabled` store action.
- UI: a toggle placed **directly below the "deep scan" toggle** in
  `SourcesDialog.tsx` (the deep-scan span is at `SourcesDialog.tsx:43`).
- **Semantics:** the toggle gates the **live background watcher thread only**.
  Launch-reconcile, manual refresh, and the Update Logs button still function when it is
  off — the badge stays accurate on open and after user actions; only real-time updates
  while the app sits idle stop. Toggling it on/off starts/stops the watcher thread without
  restarting the app.

### 4. Command surface (Tauri)

- `get_pending_log_count(sources) -> usize` — wraps `pending_files().len()`; used for boot
  reconcile and manual refresh.
- `update_logs(sources) -> ScanResult` — loops the configured sources calling the existing
  incremental scan path with `force=false` (reusing `scan_logs`/`scan_recursive`),
  **without any reset**, aggregating into one `ScanResult`. Emits the existing
  `scan-progress` events so the progress bar works unchanged.
- Watcher lifecycle commands as needed (e.g. `start_log_watcher(sources)` /
  `stop_log_watcher()`), or managed via Tauri state initialized at startup and reconfigured
  when sources/toggle change.

### 5. Frontend

- On boot: call `get_pending_log_count(sources)` → set badge.
- Subscribe to `pending-changed` → update badge (only meaningful while the watcher runs).
- **"Update Logs (N)"** button (new, alongside the unchanged "Scan All Logs"):
  - calls `update_logs(sources)`,
  - on success refreshes the active views/stats (same refresh path as a scan),
  - recomputes pending → badge returns to 0.
- Badge hidden / shows no number when `N == 0`.
- When `watchLogsEnabled` is toggled, (re)start or stop the watcher via the lifecycle
  command and re-run the launch-reconcile count.

### 6. Buttons & labels

- Keep **both** buttons. "Scan All Logs" remains the heavy rebuild (`rescan_logs`);
  "Update Logs" is the new daily driver.
- Label: **"Update Logs"** (with `(N)` badge).

## Known caveat (documented, not fixed)

Legacy DBs scanned before offset-resume have `byte_len = 0`. For those specific files,
**appended** content is neither counted nor tail-scanned by Update Logs — exactly matching
today's documented scanner behavior. New files in those folders still count. One
"Scan All Logs" rebuild repopulates `byte_len` and resolves it going forward. Surface this
as a one-line note near the button rather than special-casing it.

## Error handling

- Missing / unreadable source folders: skipped with a logged warning in both the pending
  pass and the watcher; never crash the badge or the watcher thread.
- `update_logs` surfaces scan errors through the existing `ScanResult` reporting.
- Watcher initialization failure (e.g. OS limits): log and continue with reconcile-only
  behavior; the badge still works via boot/manual recompute.

## Testing

- Unit-test `pending_files` for every row of the decision table: new file, grown file,
  unchanged file, shrank/rotated file, legacy `byte_len = 0`, and non-`CL Log` files
  ignored. Cover `recursive` vs non-recursive source discovery.
- Incremental-scan correctness (new + appended, no double-count, login counted once) is
  already covered by existing scan tests; add a focused test that `update_logs` does not
  reset and does not double-count on repeated invocation.
- Keep the watcher thin (all logic lives in the tested `pending_files`); do not attempt
  brittle cross-platform FS-event unit tests.

---

## Design revision (2026-06-30, post-implementation)

The live filesystem watcher was **removed** after smoke testing. Root cause: macOS
FSEvents (used by the `notify` crate's recommended watcher) silently accepts a watch on
external/USB-mounted APFS volumes but never delivers events — and the test user (like many
players) keeps logs on a USB volume (`/Volumes/Aux`). The watcher registered without error
yet no events ever fired, so the live badge update never happened.

Rather than fight FSEvents (poll-watcher fallback, etc.), the watcher is dropped entirely:

- **Removed:** the `notify` dependency, `commands/watcher.rs` (`start_log_watcher` /
  `stop_log_watcher`), the `AppState.log_watcher` field, the `pending-changed` event, the
  `watchLogsEnabled` setting, and the "Watch log folders for changes" toggle.
- **Kept (works on every volume — no FSEvents):** the stat-based pending badge. The
  `usePendingLogCount` hook recomputes the count on DB-open / source-change, on **window
  focus**, and after every scan. This keeps the badge usefully fresh without a watcher.
- **Added:** a post-update **`UpdateResultDialog`** confirmation showing the `ScanResult`
  counts plus a per-character logins/deaths delta (before/after `listCharacters` diff in
  `handleUpdateLogs`), or an "Already up to date" message when nothing was found.

Net effect: the feature is purely on-demand (click → process → confirmation), with a
stat-based hint badge, and no platform-fragile event source.

### Pending-count correctness (post-verification fixes)

Manual verification surfaced two enumeration/decision mismatches between `pending_files` and
the actual scanner that left the badge stuck (counting files Update would never clear):

1. **Wrong depth.** The scanner scans `CL Log` files inside each log root's *character
   subfolders* (`scan_folder_inner` iterates subdirs, `find_log_files` each). `pending_files`
   was instead reading files *directly in the log root*, so it both missed real character
   logs and counted a stray loose file that the scanner can never reach. Fixed by `char_log_files`,
   which mirrors the scanner's subfolder enumeration.
2. **`SkipDuplicate` not modelled.** A new-path file whose content was already scanned
   elsewhere is a `SkipDuplicate` — the scanner never records it, so a metadata-only check
   counted it forever (the user has two source folders with overlapping logs). Fixed by
   `would_scan`, the read-only twin of `plan_file_scan` that reads the candidate and applies
   the content-hash dedup. Consequence: `pending_files` is no longer metadata-only — it reads
   candidate (new/grown) files. In steady state candidates are few; with overlapping source
   folders the duplicate set is re-read on each refresh (acceptable; optimise if it lags).

Invariant restated: **the badge must count a file iff `plan_file_scan` would `Scan` it.**
Regression tests cover the loose-file and duplicate-content cases.
