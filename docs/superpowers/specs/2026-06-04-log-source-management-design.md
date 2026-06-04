# Log Source Management — Design

Date: 2026-06-04
Status: Approved (pending spec review)

## Problem

Amanuensis remembers only a single in-memory `logFolder`, set when the user runs
"Scan Log Folder(s)" and lost on app restart. Consequences:

- **"Rescan Logs" is greyed out on every launch** (its disabled condition is
  `isScanning || !logFolder`, and `logFolder` is `null` until a scan happens this
  session). Users who keep a populated database can't rescan without first
  re-picking a folder.
- Users who scan **multiple** folders (the real workflow) have no record of which
  folders feed the database, and rescanning replays only the last one.

## Goals

- Persist the set of log source folders so they survive restarts.
- Support **multiple** source folders, each with its own recursive ("deep scan") setting.
- "Rescan Logs" replays **all** sources.
- A settings window lets users see and remove sources.
- "Rescan Logs" is enabled on launch whenever at least one source is remembered.

## Non-goals (YAGNI)

- Storing sources in the database (decided: per-machine app settings instead).
- Adding folders from inside the settings window (scanning auto-adds; the window is
  for review/cleanup only).
- Detecting/flagging source folders that no longer exist (a missing folder simply
  scans to 0 files; a "missing source" indicator is a possible future addition).
- Treating individual files ("Scan Specific Log Files") as sources — sources are folders.
- Per-source dedup beyond exact-path matching (no parent/child folder reconciliation).

## Decisions (from brainstorming)

1. **Persistence:** per-machine `localStorage` (app-install concern), not the DB.
2. **Adding sources:** scanning a folder auto-adds it; the settings window only views/removes.
3. **Recursive flag:** stored **per source**, captured at scan time, honored on rescan.

## Design

### Data model & persistence

Frontend store (`lib/store.ts`) gains:

```ts
interface LogSource { path: string; recursive: boolean; }

sources: LogSource[];
addSource: (path: string, recursive: boolean) => void;  // dedupe by path; update recursive if re-scanned
removeSource: (path: string) => void;
```

- Persisted to `localStorage` key `amanuensis-log-sources` as a JSON array, hydrated on
  store init (same pattern as `theme`, `index-logs`).
- The existing single `logFolder` field is **removed**; all call sites move to `sources`.
- The backend stays **stateless** about sources — the frontend passes the list into
  scan/rescan commands.

### Scan behavior

- **"Scan Log Folder(s)"** — UX unchanged. On a successful scan, call
  `addSource(folderPath, recursiveScan)`. Re-scanning a path already present updates its
  `recursive` flag rather than duplicating.
- **"Scan Specific Log Files"** — unchanged; does not modify `sources`.

### Rescan behavior

New Tauri command replaces the current single-folder `rescan_logs`:

```
rescan_logs(sources: Vec<SourceSpec>, index_lines: bool) -> ScanResult
  where SourceSpec = { path: String, recursive: bool }
```

Backend sequence (one parser instance, one DB):

1. `reset_log_data()` once (clears kills/lastys/pets/log_files/log_lines, zeroes character
   log-derived columns and trainer ranks; preserves rank modifiers/overrides — unchanged).
2. For each source: `scan_recursive_with_progress` or `scan_folder_with_progress` per its
   `recursive` flag, accumulating a combined `ScanResult`.
3. `finalize_characters()` once.

Implemented by generalizing the existing `run_scan` helper (which already supports
`reset_first`) to take an ordered list of `ScanOp::Folder` ops instead of a single op,
resetting once before the loop and finalizing once after.

### Settings window (Sources)

- New **"Sources…"** button in the sidebar's Advanced section.
- Modal (styled like the existing `MergeDialog`) containing:
  - One row per source: path text, a "deep" badge when `recursive`, and a **Remove** button.
  - A **"Rescan all"** button (invokes the same rescan path).
  - Empty state: "No log folders yet — use Scan Log Folder(s) to add one."

### Enable-state & banner fixes (side effects)

- "Rescan Logs" disabled condition becomes `isScanning || sources.length === 0`.
- The ranger befriend/morph rescan banner's "click Rescan Logs" guidance now holds on a
  fresh launch (the button is enabled whenever sources exist).

## Testing

- **Core:** a test for the multi-folder rescan path — two temp source folders, one
  recursive and one not, each containing logs; after rescan the combined result includes
  both folders' data and the per-source recursive flag is honored (e.g. a nested log is
  found only under the recursive source).
- **Frontend:** `tsc -b` typecheck + `vite build` (no UI test runner configured).

## Affected files (anticipated)

- `crates/amanuensis-gui/ui/src/lib/store.ts` — sources state + persistence; remove `logFolder`.
- `crates/amanuensis-gui/ui/src/lib/hooks/useScan.ts` — auto-add on scan; multi-source rescan.
- `crates/amanuensis-gui/ui/src/lib/commands.ts` — updated `rescan_logs` signature.
- `crates/amanuensis-gui/ui/src/components/layout/Sidebar.tsx` — "Sources…" button, enable-state.
- `crates/amanuensis-gui/ui/src/components/shared/SourcesDialog.tsx` — new modal.
- `crates/amanuensis-gui/src/commands/scanning.rs` + `commands/mod.rs` — multi-source rescan command + `run_scan` generalization.
