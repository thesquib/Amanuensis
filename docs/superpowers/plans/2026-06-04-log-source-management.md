# Log Source Management Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Persist a per-machine list of log source folders (each with its own recursive flag), rescan all of them from one action, and let users review/remove them in a settings window.

**Architecture:** The source list lives in the frontend Zustand store, persisted to `localStorage`; the backend stays stateless about sources. Scanning a folder auto-adds it to the list. A new core method `LogParser::rescan_sources` does the multi-folder reset→scan→finalize; a thin Tauri `rescan_logs` command wraps it. A `SourcesDialog` modal manages the list.

**Tech Stack:** Rust (`amanuensis-core`, `amanuensis-gui` Tauri backend), React + TypeScript + Zustand (Tauri frontend).

**Notes for the engineer:**
- The frontend has **no test runner**. For frontend tasks, "verify" means `npx tsc -b` (typecheck) and, for the final UI task, `npm run build`, plus a manual app run. TDD red/green applies to the Rust core task only.
- Run all `cargo`/`npx`/`npm` commands from the repo root unless stated. Frontend dir: `crates/amanuensis-gui/ui`.

---

## File Structure

- `crates/amanuensis-core/src/parser/mod.rs` — new `LogParser::rescan_sources` + tests (core orchestration).
- `crates/amanuensis-gui/src/commands/scanning.rs` — rewrite `rescan_logs` Tauri command (multi-source).
- `crates/amanuensis-gui/src/commands/mod.rs` — `SourceSpec` deserialize struct.
- `crates/amanuensis-gui/ui/src/lib/constants.ts` — `LOG_SOURCES` storage key.
- `crates/amanuensis-gui/ui/src/lib/store.ts` — `LogSource` type, `sources` state + persistence; later remove `logFolder`.
- `crates/amanuensis-gui/ui/src/lib/commands.ts` — `rescanLogs` signature.
- `crates/amanuensis-gui/ui/src/lib/hooks/useScan.ts` — auto-add on scan; multi-source rescan.
- `crates/amanuensis-gui/ui/src/components/shared/SourcesDialog.tsx` — new modal.
- `crates/amanuensis-gui/ui/src/components/layout/Sidebar.tsx` — "Sources…" button, rescan enable-state.

---

## Task 1: Core `LogParser::rescan_sources`

**Files:**
- Modify: `crates/amanuensis-core/src/parser/mod.rs` (add method near `finalize_characters`, ~line 1293; add tests in the `#[cfg(test)] mod tests` block)

- [ ] **Step 1: Write the failing tests**

Add inside `mod tests` (after `test_reflect_movements_befriend_morph_coexist`). These build two source folders; `Beta`'s log sits one level deeper, so only a recursive scan finds it.

```rust
    #[test]
    fn test_rescan_sources_clears_previous_data() {
        // Pre-seed Alpha from folder_a, then rescan only folder_b (recursive).
        // The reset must drop Alpha; the recursive scan must find the nested Beta.
        let tmp = tempfile::tempdir().unwrap();
        let folder_a = tmp.path().join("a");
        let folder_b = tmp.path().join("b");
        fs::create_dir_all(folder_a.join("Alpha")).unwrap();
        fs::create_dir_all(folder_b.join("nested").join("Beta")).unwrap();
        fs::write(
            folder_a.join("Alpha").join("CL Log 2024-01-01 10.00.00.txt"),
            "1/1/24 1:00:00p Welcome to Clan Lord, Alpha!\n1/1/24 1:01:00p You slaughtered a Rat.\n",
        )
        .unwrap();
        fs::write(
            folder_b.join("nested").join("Beta").join("CL Log 2024-01-01 10.00.00.txt"),
            "1/1/24 1:00:00p Welcome to Clan Lord, Beta!\n1/1/24 1:01:00p You slaughtered a Rat.\n",
        )
        .unwrap();

        let db = Database::open_in_memory().unwrap();
        let parser = LogParser::new(db).unwrap();
        parser.scan_folder(&folder_a, false).unwrap(); // stale data

        parser
            .rescan_sources(&[(folder_b.clone(), true)], false, |_, _, _| {})
            .unwrap();

        let names: Vec<String> = parser
            .db()
            .list_characters()
            .unwrap()
            .into_iter()
            .map(|c| c.name)
            .collect();
        assert!(!names.contains(&"Alpha".to_string()), "reset should have dropped Alpha");
        assert!(names.contains(&"Beta".to_string()), "recursive source should find nested Beta");
    }

    #[test]
    fn test_rescan_sources_combines_per_source_recursive() {
        // folder_a (non-recursive) holds Alpha at its top level; folder_b (recursive)
        // holds Beta nested one level down. Both should be present after one rescan.
        let tmp = tempfile::tempdir().unwrap();
        let folder_a = tmp.path().join("a");
        let folder_b = tmp.path().join("b");
        fs::create_dir_all(folder_a.join("Alpha")).unwrap();
        fs::create_dir_all(folder_b.join("nested").join("Beta")).unwrap();
        fs::write(
            folder_a.join("Alpha").join("CL Log 2024-01-01 10.00.00.txt"),
            "1/1/24 1:00:00p Welcome to Clan Lord, Alpha!\n1/1/24 1:01:00p You slaughtered a Rat.\n",
        )
        .unwrap();
        fs::write(
            folder_b.join("nested").join("Beta").join("CL Log 2024-01-01 10.00.00.txt"),
            "1/1/24 1:00:00p Welcome to Clan Lord, Beta!\n1/1/24 1:01:00p You slaughtered a Rat.\n",
        )
        .unwrap();

        let db = Database::open_in_memory().unwrap();
        let parser = LogParser::new(db).unwrap();

        let result = parser
            .rescan_sources(
                &[(folder_a.clone(), false), (folder_b.clone(), true)],
                false,
                |_, _, _| {},
            )
            .unwrap();

        let names: Vec<String> = parser
            .db()
            .list_characters()
            .unwrap()
            .into_iter()
            .map(|c| c.name)
            .collect();
        assert!(names.contains(&"Alpha".to_string()), "non-recursive source Alpha missing");
        assert!(names.contains(&"Beta".to_string()), "recursive source Beta missing");
        assert_eq!(result.characters, names.len(), "characters count should be distinct DB total");
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p amanuensis-core rescan_sources 2>&1 | tail -20`
Expected: compile error — `no method named rescan_sources found for ... LogParser` (the method does not exist yet).

- [ ] **Step 3: Implement `rescan_sources`**

Add this method to `impl LogParser` immediately before `finalize_characters` (~line 1293). `std::path::PathBuf` is already imported in this file.

```rust
    /// Rescan a set of source folders: clear log-derived data, scan each folder
    /// (honoring its own recursive flag), then finalize. Used by the GUI "Rescan Logs"
    /// action when the user has multiple remembered source folders. Rank overrides are
    /// preserved (reset_log_data keeps them). The returned ScanResult sums the additive
    /// per-folder counters; `characters` is the distinct character total after finalize.
    pub fn rescan_sources<F>(
        &self,
        sources: &[(std::path::PathBuf, bool)],
        index_lines: bool,
        progress: F,
    ) -> Result<ScanResult>
    where
        F: Fn(usize, usize, &str),
    {
        self.db.reset_log_data()?;
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

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p amanuensis-core rescan_sources 2>&1 | tail -10`
Expected: `test result: ok. 2 passed`

- [ ] **Step 5: Full core suite + clippy stay green**

Run: `cargo test -p amanuensis-core 2>&1 | grep "test result:" | head -1 && cargo clippy -p amanuensis-core 2>&1 | grep -E "warning|error" || echo "clippy clean"`
Expected: `test result: ok. 328 passed; 0 failed; 1 ignored` and `clippy clean`

- [ ] **Step 6: Commit**

```bash
git add crates/amanuensis-core/src/parser/mod.rs
git commit -m "Add LogParser::rescan_sources for multi-folder rescan"
```

---

## Task 2: Backend `rescan_logs` command (multi-source)

**Files:**
- Modify: `crates/amanuensis-gui/src/commands/mod.rs` (add `SourceSpec`)
- Modify: `crates/amanuensis-gui/src/commands/scanning.rs` (rewrite `rescan_logs`)

- [ ] **Step 1: Add the `SourceSpec` deserialize struct**

In `crates/amanuensis-gui/src/commands/mod.rs`, directly after the `ScanOp` enum (~line 52), add:

```rust
#[derive(serde::Deserialize)]
pub(super) struct SourceSpec {
    pub path: String,
    pub recursive: bool,
}
```

- [ ] **Step 2: Rewrite the `rescan_logs` command**

In `crates/amanuensis-gui/src/commands/scanning.rs`, replace the entire existing `rescan_logs` function (the `#[tauri::command] pub async fn rescan_logs(...)` block) with:

```rust
/// Rescan logs: clear all log-derived data (preserving rank overrides) then rescan
/// every remembered source folder (each with its own recursive flag).
#[tauri::command]
pub async fn rescan_logs(
    sources: Vec<SourceSpec>,
    index_lines: bool,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<ScanResult, String> {
    let db = state.take_db()?;
    let state_db = state.db.clone();
    let folders: Vec<(std::path::PathBuf, bool)> = sources
        .into_iter()
        .map(|s| (std::path::PathBuf::from(s.path), s.recursive))
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
            .rescan_sources(&folders, index_lines, progress_cb)
            .map_err(|e| e.to_string())?;
        *state_db.lock().map_err(|e| format!("Lock poisoned: {e}"))? = Some(parser.into_db());
        Ok(result)
    })
    .await
    .map_err(|e| e.to_string())?;

    result
}
```

- [ ] **Step 3: Fix imports in `scanning.rs`**

`rescan_logs` no longer uses `run_scan` or `ScanOp`, but `scan_logs`/`scan_files` still do. It now needs `SourceSpec`, `LogParser`, `ScanProgress`, and `tauri::Emitter` (for `app.emit`). Update the `use super::...` line near the top of `scanning.rs` to:

```rust
use super::{run_scan, ScanOp, ScanProgress, SourceSpec};
use amanuensis_core::parser::LogParser;
use tauri::Emitter;
```

(Keep the existing `use tauri::State;` and `use amanuensis_core::parser::ScanResult;` lines.)

- [ ] **Step 4: Verify the workspace builds**

Run: `cargo build 2>&1 | tail -5`
Expected: `Finished` with no errors. If the compiler reports `ScanProgress`/`Emitter`/`run_scan` as unused or missing, adjust the `use` lines to match what `scan_logs`/`scan_files` and the new function actually reference (the compiler message names the exact item).

- [ ] **Step 5: Commit**

```bash
git add crates/amanuensis-gui/src/commands/mod.rs crates/amanuensis-gui/src/commands/scanning.rs
git commit -m "Make rescan_logs command accept multiple sources"
```

---

## Task 3: Store — `sources` state + persistence (alongside `logFolder`)

**Files:**
- Modify: `crates/amanuensis-gui/ui/src/lib/constants.ts`
- Modify: `crates/amanuensis-gui/ui/src/lib/store.ts`

Keep `logFolder` for now so consumers still compile; it is removed in Task 6.

- [ ] **Step 1: Add the storage key**

In `crates/amanuensis-gui/ui/src/lib/constants.ts`, add a line inside the `STORAGE_KEYS` object (after `TRAINERS_ALPHA_VIEW`):

```ts
  LOG_SOURCES: "amanuensis_log_sources",
```

- [ ] **Step 2: Add the `LogSource` type and a loader helper**

In `crates/amanuensis-gui/ui/src/lib/store.ts`, add near the top (after the existing imports, before `interface AppStore`):

```ts
export interface LogSource {
  path: string;
  recursive: boolean;
}

function loadSources(): LogSource[] {
  try {
    const raw = localStorage.getItem(STORAGE_KEYS.LOG_SOURCES);
    if (!raw) return [];
    const parsed = JSON.parse(raw);
    if (!Array.isArray(parsed)) return [];
    return parsed.filter(
      (s): s is LogSource =>
        s && typeof s.path === "string" && typeof s.recursive === "boolean",
    );
  } catch {
    return [];
  }
}
```

- [ ] **Step 3: Add `sources` to the `AppStore` interface**

In the `interface AppStore` block, immediately after the `logFolder`/`setLogFolder` lines (~line 48), add:

```ts
  // Log sources (persisted list of scanned folders)
  sources: LogSource[];
  addSource: (path: string, recursive: boolean) => void;
  removeSource: (path: string) => void;
```

- [ ] **Step 4: Implement `sources` in the store body**

In the `create<AppStore>((set) => ({ ... }))` body, immediately after the `setLogFolder` line (~line 158), add:

```ts
  sources: loadSources(),
  addSource: (path, recursive) =>
    set((s) => {
      const others = s.sources.filter((src) => src.path !== path);
      const next = [...others, { path, recursive }];
      localStorage.setItem(STORAGE_KEYS.LOG_SOURCES, JSON.stringify(next));
      return { sources: next };
    }),
  removeSource: (path) =>
    set((s) => {
      const next = s.sources.filter((src) => src.path !== path);
      localStorage.setItem(STORAGE_KEYS.LOG_SOURCES, JSON.stringify(next));
      return { sources: next };
    }),
```

- [ ] **Step 5: Typecheck**

Run: `cd crates/amanuensis-gui/ui && npx tsc -b 2>&1 | tail -5; cd ../../..`
Expected: no output after the npm notice (clean typecheck).

- [ ] **Step 6: Commit**

```bash
git add crates/amanuensis-gui/ui/src/lib/constants.ts crates/amanuensis-gui/ui/src/lib/store.ts
git commit -m "Add persisted log sources list to store"
```

---

## Task 4: `useScan` — auto-add on scan, multi-source rescan

**Files:**
- Modify: `crates/amanuensis-gui/ui/src/lib/commands.ts`
- Modify: `crates/amanuensis-gui/ui/src/lib/hooks/useScan.ts`

- [ ] **Step 1: Update the `rescanLogs` command wrapper**

In `crates/amanuensis-gui/ui/src/lib/commands.ts`, replace the entire existing `rescanLogs` function with:

```ts
export async function rescanLogs(
  sources: { path: string; recursive: boolean }[],
  indexLines: boolean = true,
): Promise<ScanResult> {
  return invoke("rescan_logs", { sources, indexLines });
}
```

- [ ] **Step 2: Update `useScan` to use `sources`**

In `crates/amanuensis-gui/ui/src/lib/hooks/useScan.ts`:

(a) Change the store destructuring (~lines 9-22) — replace `logFolder` and `setLogFolder` with `sources` and `addSource`:

```ts
  const {
    sources,
    addSource,
    isScanning,
    setIsScanning,
    scanProgress,
    setScanProgress,
    setCharacters,
    setScannedLogCount,
    setLogLineCount,
    setProcessLogs,
    recursiveScan,
    indexLogLines,
  } = useStore();
```

(b) In `handleScanFolder`, replace `setLogFolder(folderPath);` (~line 53) with nothing (remove that line), and after the successful `await scanLogs(...)` call (right before `await finishScan();`, ~line 59) add:

```ts
      addSource(folderPath, recursiveScan);
```

Then update that callback's dependency array (~line 66) to:

```ts
  }, [addSource, setIsScanning, setScanProgress, finishScan, recursiveScan, indexLogLines]);
```

(c) Replace the entire `handleRescanLogs` callback with:

```ts
  const handleRescanLogs = useCallback(async () => {
    if (sources.length === 0) return;
    const confirmed = await confirm(
      "This will clear all scanned data and rescan every remembered log source from scratch. Your rank modifier settings will be preserved. Continue?",
      { title: "Rescan Logs", kind: "warning" },
    );
    if (!confirmed) return;
    setIsScanning(true);
    setScanProgress(null);
    try {
      await rescanLogs(sources, indexLogLines);
      await finishScan();
    } catch (e) {
      console.error("Rescan failed:", e);
    } finally {
      setIsScanning(false);
      setScanProgress(null);
    }
  }, [sources, indexLogLines, setIsScanning, setScanProgress, finishScan]);
```

(d) In the hook's return object (~lines 113-120), replace `logFolder,` with `sources,`.

- [ ] **Step 3: Typecheck**

Run: `cd crates/amanuensis-gui/ui && npx tsc -b 2>&1 | tail -8; cd ../../..`
Expected: error(s) only in `Sidebar.tsx` (it still references the old `logFolder`/`handleRescanLogs` shape) — that is fixed in Task 5. `useScan.ts` and `commands.ts` themselves must be error-free. If `useScan.ts` reports an unused `confirm` import, leave it (still used) — confirm the only errors are in `Sidebar.tsx`.

- [ ] **Step 4: Commit**

```bash
git add crates/amanuensis-gui/ui/src/lib/commands.ts crates/amanuensis-gui/ui/src/lib/hooks/useScan.ts
git commit -m "Wire useScan to sources list and multi-source rescan"
```

---

## Task 5: Sidebar "Sources…" button + `SourcesDialog`

**Files:**
- Create: `crates/amanuensis-gui/ui/src/components/shared/SourcesDialog.tsx`
- Modify: `crates/amanuensis-gui/ui/src/components/layout/Sidebar.tsx`

- [ ] **Step 1: Create the `SourcesDialog` component**

Create `crates/amanuensis-gui/ui/src/components/shared/SourcesDialog.tsx`:

```tsx
import { useStore } from "../../lib/store";

interface SourcesDialogProps {
  onClose: () => void;
  onRescan: () => void;
  isScanning: boolean;
}

export function SourcesDialog({ onClose, onRescan, isScanning }: SourcesDialogProps) {
  const { sources, removeSource } = useStore();

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/60"
      onClick={onClose}
    >
      <div
        className="w-full max-w-md rounded-lg border border-[var(--color-border)] bg-[var(--color-bg)] p-5 shadow-xl"
        onClick={(e) => e.stopPropagation()}
      >
        <h2 className="mb-1 text-lg font-bold">Log Sources</h2>
        <p className="mb-4 text-xs text-[var(--color-text-muted)]">
          Folders scanned into this app. Rescan replays all of them. Removing a source only
          forgets the folder; it does not delete already-scanned data until the next rescan.
        </p>

        {sources.length === 0 ? (
          <div className="mb-4 rounded border border-[var(--color-border)] px-3 py-6 text-center text-sm text-[var(--color-text-muted)]">
            No log folders yet — use Scan Log Folder(s) to add one.
          </div>
        ) : (
          <div className="mb-4 max-h-64 overflow-y-auto rounded border border-[var(--color-border)]">
            {sources.map((src) => (
              <div
                key={src.path}
                className="flex items-center gap-3 border-b border-[var(--color-border)] px-3 py-2 last:border-b-0"
              >
                <div className="min-w-0 flex-1">
                  <div className="truncate text-sm" title={src.path}>
                    {src.path}
                  </div>
                  {src.recursive && (
                    <span className="text-xs text-[var(--color-text-muted)]">deep scan</span>
                  )}
                </div>
                <button
                  type="button"
                  onClick={() => removeSource(src.path)}
                  disabled={isScanning}
                  className="shrink-0 text-xs text-red-400 hover:text-red-300 disabled:opacity-50"
                >
                  Remove
                </button>
              </div>
            ))}
          </div>
        )}

        <div className="flex justify-between">
          <button
            type="button"
            onClick={onRescan}
            disabled={isScanning || sources.length === 0}
            className="rounded bg-[var(--color-accent)] px-3 py-1.5 text-sm font-medium text-white hover:bg-[var(--color-accent)]/80 disabled:opacity-50"
          >
            {isScanning ? "Rescanning..." : "Rescan all"}
          </button>
          <button
            type="button"
            onClick={onClose}
            className="rounded border border-[var(--color-border)] bg-[var(--color-btn-secondary)] px-3 py-1.5 text-sm font-medium hover:opacity-80"
          >
            Close
          </button>
        </div>
      </div>
    </div>
  );
}
```

- [ ] **Step 2: Wire the dialog into the Sidebar**

In `crates/amanuensis-gui/ui/src/components/layout/Sidebar.tsx`:

(a) Add the import near the other shared imports (after the `MergeDialog` import, ~line 9):

```tsx
import { SourcesDialog } from "../shared/SourcesDialog";
```

(b) Change the store destructuring (~line 13) — replace `logFolder` with `sources`:

```tsx
  const { dbPath, sources, scannedLogCount, recursiveScan, setRecursiveScan, indexLogLines, setIndexLogLines, theme, setTheme, characters, setCharacters } = useStore();
```

(c) Add a dialog-open state next to `showMergeDialog` (~line 16):

```tsx
  const [showSourcesDialog, setShowSourcesDialog] = useState(false);
```

(d) Replace the existing "Rescan Logs" `<button>` block (the one with `onClick={handleRescanLogs}` and `disabled={isScanning || !logFolder}`) with:

```tsx
            <button
              onClick={handleRescanLogs}
              disabled={isScanning || sources.length === 0}
              className="rounded border border-[var(--color-border)] bg-[var(--color-btn-secondary)] px-3 py-1.5 text-sm font-medium hover:opacity-80 disabled:opacity-50"
              title={sources.length > 0 ? "Clear all scanned data and rescan every source from scratch (preserves rank modifiers)" : "No log sources yet — scan a folder first"}
            >
              Rescan Logs
            </button>
            <button
              onClick={() => setShowSourcesDialog(true)}
              className="rounded border border-[var(--color-border)] bg-[var(--color-btn-secondary)] px-3 py-1.5 text-sm font-medium hover:opacity-80"
            >
              Sources… ({sources.length})
            </button>
```

(e) Find where `MergeDialog` is rendered (search for `showMergeDialog &&`) and add, immediately after that block:

```tsx
      {showSourcesDialog && (
        <SourcesDialog
          onClose={() => setShowSourcesDialog(false)}
          onRescan={handleRescanLogs}
          isScanning={isScanning}
        />
      )}
```

- [ ] **Step 3: Typecheck + full build**

Run: `cd crates/amanuensis-gui/ui && npx tsc -b 2>&1 | tail -5 && npm run build 2>&1 | tail -4; cd ../../..`
Expected: clean typecheck and `✓ built` (the pre-existing 500 kB chunk-size warning is fine).

- [ ] **Step 4: Commit**

```bash
git add crates/amanuensis-gui/ui/src/components/shared/SourcesDialog.tsx crates/amanuensis-gui/ui/src/components/layout/Sidebar.tsx
git commit -m "Add Sources settings dialog and multi-source rescan button"
```

---

## Task 6: Remove the obsolete `logFolder` from the store

**Files:**
- Modify: `crates/amanuensis-gui/ui/src/lib/store.ts`

- [ ] **Step 1: Confirm nothing else references `logFolder`**

Run: `grep -rn "logFolder\|setLogFolder" crates/amanuensis-gui/ui/src`
Expected: only the definition lines in `store.ts`. If any other file appears, it was missed in an earlier task — migrate it to `sources` before continuing.

- [ ] **Step 2: Delete the `logFolder` declarations**

In `crates/amanuensis-gui/ui/src/lib/store.ts`, remove the interface lines:

```ts
  // Log folder
  logFolder: string | null;
  setLogFolder: (folder: string | null) => void;
```

and the store-body lines:

```ts
  logFolder: null,
  setLogFolder: (folder) => set({ logFolder: folder }),
```

- [ ] **Step 3: Typecheck**

Run: `cd crates/amanuensis-gui/ui && npx tsc -b 2>&1 | tail -5; cd ../../..`
Expected: clean (no errors).

- [ ] **Step 4: Commit**

```bash
git add crates/amanuensis-gui/ui/src/lib/store.ts
git commit -m "Remove obsolete single logFolder store field"
```

---

## Task 7: Manual end-to-end verification

**No code changes — verify the feature in the running app.**

- [ ] **Step 1: Launch the app**

Run (background): `cargo tauri dev` (from repo root; it locates `crates/amanuensis-gui/tauri.conf.json`). Wait for the window.

- [ ] **Step 2: Verify scan auto-adds a source**

Click **Scan Log Folder(s)**, pick a folder (toggle Deep scan as desired). After it completes, open **Advanced → Sources…** and confirm the folder appears (with a "deep scan" label if recursive).

- [ ] **Step 3: Verify multi-source rescan + persistence**

Scan a second folder. Confirm both appear in Sources…. Click **Rescan all** (or **Rescan Logs**); confirm it completes and characters from both folders are present. Close and relaunch the app; confirm **Rescan Logs** is **enabled** on launch (not greyed out) and Sources… still lists both.

- [ ] **Step 4: Verify remove**

In Sources…, **Remove** one source; confirm it disappears and persists after relaunch.

- [ ] **Step 5: Stop the dev app**

Stop the background `cargo tauri dev` process.

---

## Self-review notes

- **Spec coverage:** persistence in localStorage (Task 3), multiple sources + per-source recursive (Tasks 1,3,4), auto-add on scan (Task 4), rescan-all (Tasks 1,2,4), settings window view/remove (Task 5), enable-on-launch + banner correctness (Task 5 enable-state). All covered.
- **Type consistency:** `rescan_sources(&[(PathBuf,bool)], bool, F)` (core) ↔ `rescan_logs(Vec<SourceSpec>, index_lines)` (backend) ↔ `rescanLogs(sources, indexLines)` (frontend) ↔ `addSource(path, recursive)` / `LogSource{path,recursive}` (store). Names align across tasks.
- **No placeholders:** every code step is complete.
