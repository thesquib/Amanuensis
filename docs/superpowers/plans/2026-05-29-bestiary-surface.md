# Bestiary Surface Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Consolidate the parallel frontend bestiary onto the backend's bundled `bestiary.json`, then add kill-detail modal, summary completion + breakdown sections, and family/rarity/seasonal filters to KillsView/CLI.

**Architecture:** A new Tauri command `get_bestiary` serves the backend bestiary into a Zustand store slice at app boot. `lib/bestiary.ts` is rewritten to read from the store while keeping its public API stable, so the six existing consumers swap data sources without behavioral change. New features (kill modal, completion stats, breakdown, filters) build on the unified store.

**Tech Stack:** Rust 2021, Tauri 2, React 19, TypeScript, Zustand 5, tanstack/react-table 8, Tailwind 4. Frontend has no test runner; Rust uses standard `cargo test`. Frontend changes are verified via `cargo tauri dev` smoke tests called out explicitly per task.

**Spec:** `docs/superpowers/specs/2026-05-29-bestiary-surface-design.md`

**Branch:** All work lands on a single branch `bestiary-surface`. Branch is created before Task 1 (see Pre-Task 0).

---

## File Structure

**New files:**
- `crates/amanuensis-gui/src/commands/bestiary.rs` — `get_bestiary` Tauri command + `BestiaryPayload`.
- `crates/amanuensis-gui/ui/src/components/shared/KillDetailModal.tsx` — kill detail modal.
- `crates/amanuensis-gui/ui/src/components/shared/BestiaryCompletion.tsx` — completion KPI + per-family table.
- `crates/amanuensis-gui/ui/src/components/shared/BestiaryBreakdown.tsx` — per-family + per-rarity kill aggregate tables.
- `crates/amanuensis-gui/ui/src/components/shared/KillsFilterBar.tsx` — family/rarity/seasonal chip filter row.

**Modified files:**
- `crates/amanuensis-core/src/db/queries/kill.rs` — add `get_encountered_creatures`, `KillsFilter`, `get_kills_filtered`.
- `crates/amanuensis-core/src/db/queries/mod.rs` — re-export new query types.
- `crates/amanuensis-gui/src/commands/mod.rs` — register `bestiary` module.
- `crates/amanuensis-gui/src/commands/data.rs` — add `get_encountered_creatures` command.
- `crates/amanuensis-gui/src/main.rs` — register new handlers.
- `crates/amanuensis-cli/src/main.rs` — `kills` subcommand gains `--family`, `--rarity`, `--seasonal`.
- `crates/amanuensis-gui/ui/src/types.ts` — add `BestiaryEntry` interface.
- `crates/amanuensis-gui/ui/src/lib/store.ts` — bestiary slice + boot loader.
- `crates/amanuensis-gui/ui/src/lib/bestiary.ts` — rewrite to consume store.
- `crates/amanuensis-gui/ui/src/lib/commands.ts` — add `getBestiary()` invoke wrapper.
- `crates/amanuensis-gui/ui/src/lib/killStats.ts` — sweep field names.
- `crates/amanuensis-gui/ui/src/lib/rangerStats.ts` — sweep field names + map shape.
- `crates/amanuensis-gui/ui/src/components/views/CVGraphView.tsx` — sweep field names.
- `crates/amanuensis-gui/ui/src/components/views/RangerStatsView.tsx` — sweep field names + map shape.
- `crates/amanuensis-gui/ui/src/components/shared/CreatureImage.tsx` — sweep field names.
- `crates/amanuensis-gui/ui/src/components/views/KillsView.tsx` — onRowClick, filter bar.
- `crates/amanuensis-gui/ui/src/components/views/SummaryView.tsx` — completion + breakdown sections.
- `crates/amanuensis-gui/ui/src/components/layout/AppShell.tsx` — boot-time bestiary load.
- `CLAUDE.md` — update Key Functional Areas with new surfaces.

**Deleted files:**
- `crates/amanuensis-gui/data/bestiary_images.json` — superseded by the unified backend bestiary.

---

## Pre-Task 0: Branch

- [ ] **Step 1: Create branch from main**

```bash
git checkout -b bestiary-surface
git status
```
Expected: `On branch bestiary-surface`, working tree clean.

---

## Task 1: Backend `get_bestiary` Tauri command

**Files:**
- Create: `crates/amanuensis-gui/src/commands/bestiary.rs`
- Modify: `crates/amanuensis-gui/src/commands/mod.rs`
- Modify: `crates/amanuensis-gui/src/main.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/amanuensis-gui/src/commands/bestiary.rs`:

```rust
use serde::Serialize;

use amanuensis_core::data::{BestiaryEntry, CreatureDb};

#[derive(Debug, Serialize)]
pub struct BestiaryPayload {
    pub version: String,
    pub entries: Vec<BestiaryEntry>,
}

#[tauri::command]
pub fn get_bestiary() -> Result<BestiaryPayload, String> {
    let db = CreatureDb::bundled().map_err(|e| e.to_string())?;
    Ok(BestiaryPayload {
        version: db.bestiary_version().to_string(),
        entries: db.entries().cloned().collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_bestiary_returns_bundled_payload() {
        let payload = get_bestiary().expect("bundled bestiary should load");
        assert!(
            payload.entries.len() > 950,
            "expected > 950 entries, got {}",
            payload.entries.len()
        );
        assert_eq!(payload.version.len(), 8, "version should be YYYYMMDD");
        // sanity: a known creature should be present
        assert!(payload.entries.iter().any(|e| e.name == "Rat"));
    }
}
```

- [ ] **Step 2: Register the module**

Edit `crates/amanuensis-gui/src/commands/mod.rs`. Add `mod bestiary;` next to the other `mod` declarations and `pub use bestiary::*;` next to the other re-exports.

After edits the top of the file looks like:
```rust
mod database;
mod scanning;
mod characters;
mod data;
mod rank;
mod portraits;
mod updates;
mod bestiary;

pub use database::*;
pub use scanning::*;
pub use characters::*;
pub use data::*;
pub use rank::*;
pub use portraits::*;
pub use updates::*;
pub use bestiary::*;
```

- [ ] **Step 3: Register the handler in main.rs**

Edit `crates/amanuensis-gui/src/main.rs`. Inside the `tauri::generate_handler![ ... ]` block, add `commands::get_bestiary,` next to the other commands (any position is fine; group with `data.rs`-style commands for readability).

- [ ] **Step 4: Run the test**

```
cargo test -p amanuensis-gui --lib commands::bestiary -- --nocapture
```
Expected: 1 passed.

- [ ] **Step 5: Build the GUI crate to confirm registration compiles**

```
cargo build -p amanuensis-gui
```
Expected: clean build (no warnings about unused commands).

- [ ] **Step 6: Commit**

```bash
git add crates/amanuensis-gui/src/commands/bestiary.rs \
        crates/amanuensis-gui/src/commands/mod.rs \
        crates/amanuensis-gui/src/main.rs
git commit -m "Add get_bestiary Tauri command serving bundled BestiaryEntry list"
```

---

## Task 2: Frontend BestiaryEntry type + commands wrapper

**Files:**
- Modify: `crates/amanuensis-gui/ui/src/types.ts`
- Modify: `crates/amanuensis-gui/ui/src/lib/commands.ts`

- [ ] **Step 1: Add BestiaryEntry interface to types.ts**

Append to `crates/amanuensis-gui/ui/src/types.ts`:

```typescript
/** Mirrors Rust `BestiaryEntry` struct (data::bestiary). */
export interface BestiaryEntry {
  name: string;
  family?: string;
  location?: string;
  information?: string;
  exp_taxidermy: number;
  rarity?: string;
  worth?: number;
  worth_range?: string;
  frames_per_swing?: number;
  difficulty?: string;
  attack?: number;
  defense?: number;
  damage?: number;
  health?: number;
  attack_measured: boolean;
  defense_measured: boolean;
  damage_measured: boolean;
  health_measured: boolean;
  luck_hits?: number;
  is_seasonal: boolean;
  first_update?: string;
  last_update?: string;
  static_pic?: string;
  static_width?: number;
  static_height?: number;
  action_pic?: string;
  action_width?: number;
  action_height?: number;
}

export interface BestiaryPayload {
  version: string;
  entries: BestiaryEntry[];
}
```

- [ ] **Step 2: Add getBestiary() wrapper**

Append to `crates/amanuensis-gui/ui/src/lib/commands.ts` (after the existing exports):

```typescript
import type { BestiaryPayload } from "../types";

export async function getBestiary(): Promise<BestiaryPayload> {
  return invoke<BestiaryPayload>("get_bestiary");
}
```

Verify the file already imports `invoke` from `@tauri-apps/api/core` at the top. If `getBestiary` ends up being the only consumer of the `BestiaryPayload` type import, leave the import next to the function for locality.

- [ ] **Step 3: Confirm TS compiles**

```
cd crates/amanuensis-gui/ui && npx tsc -b
```
Expected: no errors.

- [ ] **Step 4: Commit**

```bash
git add crates/amanuensis-gui/ui/src/types.ts crates/amanuensis-gui/ui/src/lib/commands.ts
git commit -m "Add BestiaryEntry/BestiaryPayload TS types and getBestiary() wrapper"
```

---

## Task 3: Zustand bestiary slice + boot loader

**Files:**
- Modify: `crates/amanuensis-gui/ui/src/lib/store.ts`
- Modify: `crates/amanuensis-gui/ui/src/components/layout/AppShell.tsx`

- [ ] **Step 1: Add bestiary slice to store.ts**

Inside the `AppStore` interface in `crates/amanuensis-gui/ui/src/lib/store.ts`, add the following fields (alongside the existing slices):

```typescript
  // Bestiary (loaded once at boot from get_bestiary)
  bestiaryLoaded: boolean;
  bestiaryVersion: string;
  bestiary: BestiaryEntry[];
  bestiaryByName: Record<string, BestiaryEntry>;
  setBestiary: (payload: { version: string; entries: BestiaryEntry[] }) => void;
```

Add the `BestiaryEntry` import next to the other type imports at the top:
```typescript
import type {
  Character,
  Kill,
  Trainer,
  Pet,
  Lasty,
  ScanProgress,
  ViewType,
  ProcessLog,
  BestiaryEntry,
} from "../types";
```

In the `create<AppStore>((set) => ({ ... }))` initializer body, add the bestiary defaults next to the other initial state and the `setBestiary` setter:

```typescript
  bestiaryLoaded: false,
  bestiaryVersion: "",
  bestiary: [],
  bestiaryByName: {},
  setBestiary: ({ version, entries }) =>
    set({
      bestiaryLoaded: true,
      bestiaryVersion: version,
      bestiary: entries,
      bestiaryByName: Object.fromEntries(entries.map((e) => [e.name, e])),
    }),
```

Note: `bestiaryByName` is a plain object (`Record<string, BestiaryEntry>`), not a `Map`. This matches the existing `bestiaryMap` shape consumed by `rangerStats.ts` and minimises consumer churn.

- [ ] **Step 2: Boot loader in AppShell**

Edit `crates/amanuensis-gui/ui/src/components/layout/AppShell.tsx`. Add a `useEffect` near the top of the component body that loads the bestiary once and writes it into the store:

```tsx
import { useEffect } from "react";
import { getBestiary } from "../../lib/commands";
import { useStore } from "../../lib/store";
// ...

const setBestiary = useStore((s) => s.setBestiary);
const bestiaryLoaded = useStore((s) => s.bestiaryLoaded);

useEffect(() => {
  if (bestiaryLoaded) return;
  let cancelled = false;
  getBestiary()
    .then((payload) => {
      if (!cancelled) setBestiary(payload);
    })
    .catch((err) => {
      console.error("Failed to load bestiary:", err);
    });
  return () => {
    cancelled = true;
  };
}, [bestiaryLoaded, setBestiary]);
```

If `AppShell` is currently `function AppShell() { return <...> }` and has no hooks, add the imports and hook calls inside the function body before the `return`.

- [ ] **Step 3: TypeScript compile check**

```
cd crates/amanuensis-gui/ui && npx tsc -b
```
Expected: clean.

- [ ] **Step 4: Manual smoke test**

```
cd crates/amanuensis-gui && cargo tauri dev
```
Open the app. Open the in-app dev tools (Cmd-Option-I on macOS). In the console:

```javascript
useStore.getState().bestiaryLoaded
useStore.getState().bestiary.length
useStore.getState().bestiaryByName["Rat"]
```
Expected: `true`, `> 950`, and an object with `exp_taxidermy: 2, family: "Vermine"`.

Close the dev build.

- [ ] **Step 5: Commit**

```bash
git add crates/amanuensis-gui/ui/src/lib/store.ts \
        crates/amanuensis-gui/ui/src/components/layout/AppShell.tsx
git commit -m "Load bundled bestiary into Zustand store at app boot"
```

---

## Task 4: Rewrite lib/bestiary.ts against the store

**Files:**
- Modify: `crates/amanuensis-gui/ui/src/lib/bestiary.ts`

The legacy `bestiaryMap` and the legacy short-field `BestiaryEntry` (with `pic`, `atk`, etc.) are removed. The new accessor reads from the store. Public function names stay; their return types now use the long backend field names (`static_pic`, `attack`, ...).

- [ ] **Step 1: Replace bestiary.ts entirely**

Overwrite `crates/amanuensis-gui/ui/src/lib/bestiary.ts` with:

```typescript
import { useStore } from "./store";
import type { BestiaryEntry } from "../types";

export type { BestiaryEntry } from "../types";

/** Returns a snapshot of the entire bestiary name -> entry map. */
export function getBestiaryMap(): Record<string, BestiaryEntry> {
  return useStore.getState().bestiaryByName;
}

/** Look up a creature by exact name from the loaded bestiary. */
export function getBestiaryEntry(name: string): BestiaryEntry | undefined {
  return useStore.getState().bestiaryByName[name];
}

/** Resolve a sprite URL relative to the public/bestiary folder. */
export function getCreatureImageUrl(name: string): string | null {
  const lookupName = name.startsWith("Captured ")
    ? name.slice("Captured ".length)
    : name;
  const entry = getBestiaryEntry(lookupName);
  return entry?.static_pic ? `/bestiary/${entry.static_pic}` : null;
}

/** Convenience: family of the creature, or "" if not in the bestiary. */
export function getCreatureFamily(name: string): string {
  return getBestiaryEntry(name)?.family ?? "";
}

/**
 * Families excluded from coin-level and CV graph because their bestiary values are
 * averaged across multiple population strengths (e.g. Ghastly Presence appears in
 * weak and strong variants, averaged to ~650). Demonic Undine (e.g. Ancient Darshak
 * Liche) is NOT excluded — these are specific enemies with reliable, consistent values.
 */
export const NON_STUFFABLE_FAMILIES = new Set<string>([
  "Ethereal",
  "Insubstantial Undine",
]);

/** Returns false for creatures whose bestiary values are unreliable for CV tracking. */
export function isStuffable(name: string): boolean {
  const family = getCreatureFamily(name);
  return family.length > 0 && !NON_STUFFABLE_FAMILIES.has(family);
}
```

This intentionally removes the previous `import bestiaryData from "../../../data/bestiary_images.json"` statement and the legacy `bestiaryMap` export. Consumers that imported `bestiaryMap` will get a TypeScript error in Task 5/6, which is the cue to migrate them.

- [ ] **Step 2: TypeScript compile (expect failures in consumers)**

```
cd crates/amanuensis-gui/ui && npx tsc -b 2>&1 | head -40
```
Expected: errors complaining about `bestiaryMap` no longer being exported, and errors in `rangerStats.ts` / `RangerStatsView.tsx` about the missing import. These are addressed in Tasks 5 and 6.

- [ ] **Step 3: Commit**

```bash
git add crates/amanuensis-gui/ui/src/lib/bestiary.ts
git commit -m "Rewrite lib/bestiary.ts to read from Zustand store"
```

---

## Task 5: Sweep killStats, CVGraphView, CreatureImage

These three consumers each call only `isStuffable` or `getCreatureImageUrl` / `getCreatureFamily`. They need no functional change — they should already compile against the rewritten lib. This task verifies that and updates `CreatureImage` for any short-name leftovers.

**Files:**
- Modify: `crates/amanuensis-gui/ui/src/components/shared/CreatureImage.tsx` (if any short-field access remains)
- Verify: `crates/amanuensis-gui/ui/src/lib/killStats.ts`
- Verify: `crates/amanuensis-gui/ui/src/components/views/CVGraphView.tsx`

- [ ] **Step 1: Verify killStats.ts and CVGraphView.tsx**

```
cd crates/amanuensis-gui/ui && npx tsc -b 2>&1 | grep -E '(killStats|CVGraphView|CreatureImage)' | head -20
```
Expected: NO errors mentioning these three files.

If errors appear in `killStats.ts` or `CVGraphView.tsx`, fix the field name(s) involved (`atk` -> `attack` etc.) inline.

- [ ] **Step 2: Update CreatureImage.tsx**

Open `crates/amanuensis-gui/ui/src/components/shared/CreatureImage.tsx`. The current implementation imports `getCreatureImageUrl` (already verified) and probably accesses entry fields directly only via that helper. If it accesses `bestiaryMap` or any short field directly, replace with `getBestiaryEntry(...).static_pic`, `.static_width`, `.static_height`.

After edits the file should look approximately like:

```tsx
import { getBestiaryEntry, getCreatureImageUrl } from "../../lib/bestiary";

interface CreatureImageProps {
  creatureName: string;
  className?: string;
}

export function CreatureImage({ creatureName, className }: CreatureImageProps) {
  const url = getCreatureImageUrl(creatureName);
  if (!url) return null;
  const entry = getBestiaryEntry(creatureName);
  return (
    <img
      src={url}
      alt={creatureName}
      width={entry?.static_width ?? undefined}
      height={entry?.static_height ?? undefined}
      className={className}
    />
  );
}
```

(If the existing file doesn't use width/height attributes at all, leave it alone — the only change required is making sure no removed export is referenced.)

- [ ] **Step 3: TypeScript compile**

```
cd crates/amanuensis-gui/ui && npx tsc -b 2>&1 | head -40
```
Expected: only errors remaining are in `rangerStats.ts` and `RangerStatsView.tsx`.

- [ ] **Step 4: Commit**

```bash
git add crates/amanuensis-gui/ui/src/components/shared/CreatureImage.tsx
git commit -m "Sweep CreatureImage to use renamed bestiary fields"
```

---

## Task 6: Sweep rangerStats.ts and RangerStatsView.tsx

The biggest consumer. `rangerStats.ts` takes a `bestiaryMap: Record<string, BestiaryEntry>` parameter and uses fields `family`, `atk`, `def`, `dmg`, `hp`, `fps`. All field names switch to the long backend versions. The parameter type stays the same (we kept `bestiaryByName` as a `Record` in the store).

**Files:**
- Modify: `crates/amanuensis-gui/ui/src/lib/rangerStats.ts`
- Modify: `crates/amanuensis-gui/ui/src/components/views/RangerStatsView.tsx`

- [ ] **Step 1: Update rangerStats.ts function signatures and field accesses**

In `crates/amanuensis-gui/ui/src/lib/rangerStats.ts`:

1. Remove any local `BestiaryEntry` interface declaration. Import the unified type:
   ```typescript
   import type { BestiaryEntry } from "../types";
   ```
2. The function parameters `bestiaryMap: Record<string, BestiaryEntry>` keep their shape but the field accesses change. Apply these renames everywhere in this file:
   - `.atk` -> `.attack ?? 0`
   - `.def` -> `.defense ?? 0`
   - `.dmg` -> `.damage ?? 0`
   - `.hp` -> `.health ?? 0`
   - `.fps` -> `.frames_per_swing ?? 0`
   - `.family` stays
   - `.rarity` stays (was already long-name in the old file too if used)
3. Where the existing code reads e.g. `entry.atk`, it expected `number`. The new type is `number | undefined`. The `?? 0` keeps semantics.

After the edits, run a self-grep:

```
grep -n '\.atk\|\.def\|\.dmg\|\.hp\b\|\.fps' crates/amanuensis-gui/ui/src/lib/rangerStats.ts
```
Expected: no matches.

- [ ] **Step 2: Update RangerStatsView.tsx**

In `crates/amanuensis-gui/ui/src/components/views/RangerStatsView.tsx`:

1. Replace the static `bestiaryMap` import:
   ```typescript
   // before:
   import { bestiaryMap } from "../../lib/bestiary";
   // after:
   import { getBestiaryMap } from "../../lib/bestiary";
   ```
2. Where the component reads `bestiaryMap`, call `getBestiaryMap()` inside the render so it reads current store state. If `bestiaryMap` is passed straight into `computeRangerStats(lastys, trainers, creatureValues, bestiaryMap)`, change to `getBestiaryMap()`.

- [ ] **Step 3: TypeScript compile**

```
cd crates/amanuensis-gui/ui && npx tsc -b 2>&1 | head -40
```
Expected: clean.

- [ ] **Step 4: Manual smoke test**

```
cd crates/amanuensis-gui && cargo tauri dev
```
Open the app, switch to the Ranger Stats view for a character that has lastys. Confirm the page renders without console errors. Compare numbers (eligible targets, family rows) against pre-change behavior if possible.

Close the dev build.

- [ ] **Step 5: Commit**

```bash
git add crates/amanuensis-gui/ui/src/lib/rangerStats.ts \
        crates/amanuensis-gui/ui/src/components/views/RangerStatsView.tsx
git commit -m "Sweep rangerStats and RangerStatsView to use unified bestiary fields"
```

---

## Task 7: Delete the legacy frontend bestiary

**Files:**
- Delete: `crates/amanuensis-gui/data/bestiary_images.json`

- [ ] **Step 1: Confirm no live references remain**

```
grep -rn 'bestiary_images\.json' crates/amanuensis-gui/ 2>/dev/null | grep -v node_modules
```
Expected: no hits. (If hits appear, fix them by switching to `lib/bestiary.ts` helpers or the store.)

- [ ] **Step 2: Delete the file**

```
git rm crates/amanuensis-gui/data/bestiary_images.json
```

- [ ] **Step 3: Confirm GUI build still works**

```
cd crates/amanuensis-gui/ui && npx tsc -b
```
Expected: clean.

- [ ] **Step 4: Manual smoke test**

```
cd crates/amanuensis-gui && cargo tauri dev
```
- Open the app on a character with kills.
- Open the Kills view — creature images visible where sprites exist.
- Open the Ranger Stats view — page renders.
- Open dev tools console — no errors about missing modules or undefined fields.
- Close the dev build.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "Delete legacy bestiary_images.json (consolidated into backend bestiary)"
```

---

## Task 8: Backend `get_encountered_creatures` query

**Files:**
- Modify: `crates/amanuensis-core/src/db/queries/kill.rs`

Encountered = a creature appears in `kills` (any solo/assisted/killed_by count > 0) OR in `lastys` for this character. We include lasty creatures because some are encountered but never killed.

- [ ] **Step 1: Write the failing test**

Append to `crates/amanuensis-core/src/db/queries/kill.rs`'s `#[cfg(test)] mod tests`:

```rust
#[test]
fn test_get_encountered_creatures() {
    let db = setup_test_db();
    let char_id = db
        .upsert_character(&Character::new("Tester".to_string()))
        .unwrap();

    // Insert two kills with positive counts and one zero-only row.
    let mut killed = Kill::new(char_id, "Rat".into(), 2);
    killed.killed_count = 5;
    db.upsert_kill(&killed).unwrap();

    let mut assisted = Kill::new(char_id, "Wolf".into(), 50);
    assisted.assisted_kill_count = 1;
    db.upsert_kill(&assisted).unwrap();

    let mut zero = Kill::new(char_id, "Bat".into(), 10);
    db.upsert_kill(&zero).unwrap(); // no counts → not encountered via kills

    // Insert one lasty for a creature not in kills.
    db.upsert_lasty(&Lasty::new(char_id, "Tesla".into())).unwrap();

    let encountered = db.get_encountered_creatures(char_id).unwrap();
    assert!(encountered.contains("Rat"));
    assert!(encountered.contains("Wolf"));
    assert!(encountered.contains("Tesla"));
    assert!(!encountered.contains("Bat"));
}
```

(If the existing test module already uses different helper names like `Lasty::new`, adapt the calls — read the file before writing.)

- [ ] **Step 2: Implement `get_encountered_creatures`**

Add to the `Database` impl block (the same impl block that contains `get_kills`):

```rust
/// Returns the set of creature names this character has encountered. A creature is
/// "encountered" if it appears in `kills` with any positive solo/assisted/killed_by count,
/// or in `lastys`.
pub fn get_encountered_creatures(&self, char_id: i64) -> Result<std::collections::HashSet<String>> {
    let mut out = std::collections::HashSet::new();

    let mut kill_stmt = self.conn.prepare(
        "SELECT creature_name FROM kills WHERE character_id = ?1 AND \
         (killed_count + slaughtered_count + vanquished_count + dispatched_count + \
          assisted_kill_count + assisted_slaughter_count + assisted_vanquish_count + \
          assisted_dispatch_count + killed_by_count) > 0",
    )?;
    let kill_iter = kill_stmt.query_map([char_id], |row| row.get::<_, String>(0))?;
    for name in kill_iter {
        out.insert(name?);
    }

    let mut lasty_stmt = self
        .conn
        .prepare("SELECT creature_name FROM lastys WHERE character_id = ?1")?;
    let lasty_iter = lasty_stmt.query_map([char_id], |row| row.get::<_, String>(0))?;
    for name in lasty_iter {
        out.insert(name?);
    }

    Ok(out)
}
```

- [ ] **Step 3: Run tests**

```
cargo test -p amanuensis-core --lib db::queries::kill -- --nocapture
```
Expected: all existing tests still pass, plus 1 new pass.

- [ ] **Step 4: Commit**

```bash
git add crates/amanuensis-core/src/db/queries/kill.rs
git commit -m "Add get_encountered_creatures query (kills with positive counts + lastys)"
```

---

## Task 9: Tauri command for encountered creatures + frontend wiring

**Files:**
- Modify: `crates/amanuensis-gui/src/commands/data.rs`
- Modify: `crates/amanuensis-gui/src/main.rs`
- Modify: `crates/amanuensis-gui/ui/src/lib/commands.ts`
- Modify: `crates/amanuensis-gui/ui/src/types.ts` (no — no new type needed; encountered is just `string[]`)

- [ ] **Step 1: Add Tauri command**

Append to `crates/amanuensis-gui/src/commands/data.rs`:

```rust
#[tauri::command]
pub fn get_encountered_creatures(
    char_id: i64,
    state: State<'_, AppState>,
) -> Result<Vec<String>, String> {
    state.with_db(|db| {
        db.get_encountered_creatures(char_id)
            .map(|set| {
                let mut v: Vec<String> = set.into_iter().collect();
                v.sort();
                v
            })
            .map_err(|e| e.to_string())
    })
}
```

- [ ] **Step 2: Register in main.rs**

Add `commands::get_encountered_creatures,` to the `tauri::generate_handler!` list.

- [ ] **Step 3: Build to confirm**

```
cargo build -p amanuensis-gui
```
Expected: clean.

- [ ] **Step 4: Add TS wrapper**

Append to `crates/amanuensis-gui/ui/src/lib/commands.ts`:

```typescript
export async function getEncounteredCreatures(charId: number): Promise<string[]> {
  return invoke<string[]>("get_encountered_creatures", { charId });
}
```

- [ ] **Step 5: Commit**

```bash
git add crates/amanuensis-gui/src/commands/data.rs \
        crates/amanuensis-gui/src/main.rs \
        crates/amanuensis-gui/ui/src/lib/commands.ts
git commit -m "Expose get_encountered_creatures Tauri command"
```

---

## Task 10: SummaryView bestiary completion section

**Files:**
- Create: `crates/amanuensis-gui/ui/src/components/shared/BestiaryCompletion.tsx`
- Modify: `crates/amanuensis-gui/ui/src/components/views/SummaryView.tsx`

- [ ] **Step 1: Create BestiaryCompletion.tsx**

```tsx
import { useEffect, useMemo, useState } from "react";
import { getEncounteredCreatures } from "../../lib/commands";
import { useStore } from "../../lib/store";

interface BestiaryCompletionProps {
  characterId: number;
}

interface FamilyRow {
  family: string;
  encountered: number;
  total: number;
  pct: number;
}

export function BestiaryCompletion({ characterId }: BestiaryCompletionProps) {
  const bestiary = useStore((s) => s.bestiary);
  const [encountered, setEncountered] = useState<Set<string>>(new Set());
  const [open, setOpen] = useState(false);

  useEffect(() => {
    let cancelled = false;
    getEncounteredCreatures(characterId)
      .then((names) => {
        if (!cancelled) setEncountered(new Set(names));
      })
      .catch((err) => console.error("Failed to load encountered creatures", err));
    return () => {
      cancelled = true;
    };
  }, [characterId]);

  const total = bestiary.length;
  const encCount = useMemo(
    () => bestiary.reduce((acc, e) => acc + (encountered.has(e.name) ? 1 : 0), 0),
    [bestiary, encountered],
  );
  const pct = total > 0 ? Math.round((encCount / total) * 1000) / 10 : 0;

  const families: FamilyRow[] = useMemo(() => {
    const rows = new Map<string, { encountered: number; total: number }>();
    for (const entry of bestiary) {
      const fam = entry.family ?? "Unknown";
      const row = rows.get(fam) ?? { encountered: 0, total: 0 };
      row.total += 1;
      if (encountered.has(entry.name)) row.encountered += 1;
      rows.set(fam, row);
    }
    return Array.from(rows.entries())
      .map(([family, { encountered, total }]) => ({
        family,
        encountered,
        total,
        pct: total > 0 ? (encountered / total) * 100 : 0,
      }))
      .sort((a, b) => b.pct - a.pct || a.family.localeCompare(b.family));
  }, [bestiary, encountered]);

  if (total === 0) return null;

  return (
    <section className="rounded-lg border border-[var(--color-border)] bg-[var(--color-bg-elevated)] p-4">
      <h3 className="mb-2 text-sm font-semibold uppercase tracking-wide text-[var(--color-text-muted)]">
        Bestiary completion
      </h3>
      <div className="flex items-baseline gap-3">
        <div className="text-2xl font-bold">
          {encCount} / {total}
        </div>
        <div className="text-sm text-[var(--color-text-muted)]">{pct}% encountered</div>
      </div>
      <button
        type="button"
        onClick={() => setOpen((v) => !v)}
        className="mt-3 text-xs text-[var(--color-accent)] underline"
      >
        {open ? "Hide" : "Show"} per-family breakdown
      </button>
      {open && (
        <table className="mt-3 w-full text-xs">
          <thead>
            <tr className="border-b border-[var(--color-border)] text-[var(--color-text-muted)]">
              <th className="py-1 text-left">Family</th>
              <th className="py-1 text-right">Encountered</th>
              <th className="py-1 text-right">Total</th>
              <th className="py-1 text-right">%</th>
            </tr>
          </thead>
          <tbody>
            {families.map((r) => (
              <tr key={r.family} className="border-b border-[var(--color-border)]/40">
                <td className="py-1">{r.family}</td>
                <td className="py-1 text-right">{r.encountered}</td>
                <td className="py-1 text-right">{r.total}</td>
                <td className="py-1 text-right">{Math.round(r.pct * 10) / 10}%</td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </section>
  );
}
```

- [ ] **Step 2: Wire into SummaryView.tsx**

Open `crates/amanuensis-gui/ui/src/components/views/SummaryView.tsx`. Import the new component:

```tsx
import { BestiaryCompletion } from "../shared/BestiaryCompletion";
```

Insert `<BestiaryCompletion characterId={character.id} />` after the existing summary content but before the closing wrapper. Pick the position that matches the visual flow (typically after the KPI cards section).

If the SummaryView accesses the current character via a prop named differently (e.g. `selectedCharacter`), use that property — read the file before writing.

- [ ] **Step 3: TypeScript compile**

```
cd crates/amanuensis-gui/ui && npx tsc -b
```
Expected: clean.

- [ ] **Step 4: Manual smoke test**

```
cd crates/amanuensis-gui && cargo tauri dev
```
- Open a character's Summary view.
- Confirm "Bestiary completion" card shows `N / 969`, percentage, and the toggle.
- Toggle the per-family table; verify rows sorted with highest % first.
- Pick a creature you've never killed → its family % is < 100 (sanity).

Close the dev build.

- [ ] **Step 5: Commit**

```bash
git add crates/amanuensis-gui/ui/src/components/shared/BestiaryCompletion.tsx \
        crates/amanuensis-gui/ui/src/components/views/SummaryView.tsx
git commit -m "Add bestiary completion KPI + per-family table to SummaryView"
```

---

## Task 11: SummaryView family + rarity kill breakdown

**Files:**
- Create: `crates/amanuensis-gui/ui/src/components/shared/BestiaryBreakdown.tsx`
- Modify: `crates/amanuensis-gui/ui/src/components/views/SummaryView.tsx`

- [ ] **Step 1: Create BestiaryBreakdown.tsx**

```tsx
import { useMemo } from "react";
import type { Kill } from "../../types";
import { useStore } from "../../lib/store";

interface BestiaryBreakdownProps {
  kills: Kill[];
}

interface AggRow {
  key: string;
  kills: number;
  pct: number;
}

function aggregate(kills: Kill[], group: (k: Kill) => string): AggRow[] {
  const counts = new Map<string, number>();
  let total = 0;
  for (const k of kills) {
    const totalForKill =
      k.killed_count +
      k.slaughtered_count +
      k.vanquished_count +
      k.dispatched_count +
      k.assisted_kill_count +
      k.assisted_slaughter_count +
      k.assisted_vanquish_count +
      k.assisted_dispatch_count;
    if (totalForKill === 0) continue;
    const key = group(k) || "Unknown";
    counts.set(key, (counts.get(key) ?? 0) + totalForKill);
    total += totalForKill;
  }
  return Array.from(counts.entries())
    .map(([key, count]) => ({
      key,
      kills: count,
      pct: total > 0 ? (count / total) * 100 : 0,
    }))
    .sort((a, b) => b.kills - a.kills || a.key.localeCompare(b.key));
}

export function BestiaryBreakdown({ kills }: BestiaryBreakdownProps) {
  const byName = useStore((s) => s.bestiaryByName);

  const byFamily = useMemo(
    () => aggregate(kills, (k) => byName[k.creature_name]?.family ?? ""),
    [kills, byName],
  );
  const byRarity = useMemo(
    () => aggregate(kills, (k) => byName[k.creature_name]?.rarity ?? ""),
    [kills, byName],
  );

  if (byFamily.length === 0) return null;

  return (
    <section className="rounded-lg border border-[var(--color-border)] bg-[var(--color-bg-elevated)] p-4">
      <h3 className="mb-3 text-sm font-semibold uppercase tracking-wide text-[var(--color-text-muted)]">
        Bestiary breakdown
      </h3>
      <div className="grid grid-cols-1 gap-4 md:grid-cols-2">
        <BreakdownTable title="By family" rows={byFamily} />
        <BreakdownTable title="By rarity" rows={byRarity} />
      </div>
    </section>
  );
}

function BreakdownTable({ title, rows }: { title: string; rows: AggRow[] }) {
  return (
    <div>
      <h4 className="mb-1 text-xs font-semibold text-[var(--color-text-muted)]">{title}</h4>
      <table className="w-full text-xs">
        <thead>
          <tr className="border-b border-[var(--color-border)] text-[var(--color-text-muted)]">
            <th className="py-1 text-left">{title.replace("By ", "")}</th>
            <th className="py-1 text-right">Kills</th>
            <th className="py-1 text-right">%</th>
          </tr>
        </thead>
        <tbody>
          {rows.map((r) => (
            <tr key={r.key} className="border-b border-[var(--color-border)]/40">
              <td className="py-1">{r.key}</td>
              <td className="py-1 text-right">{r.kills}</td>
              <td className="py-1 text-right">{Math.round(r.pct * 10) / 10}%</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
```

- [ ] **Step 2: Wire into SummaryView.tsx**

Find where the existing SummaryView loads the character's kills (it likely already calls `getKills` or reads from store). Pass that array into `<BestiaryBreakdown kills={kills} />`. Place the component below `<BestiaryCompletion />`.

If SummaryView does not currently hold the kills, add a `useEffect` next to the existing data loads:

```tsx
import { useState, useEffect } from "react";
import { getKills } from "../../lib/commands";
// ...
const [kills, setKills] = useState<Kill[]>([]);
useEffect(() => {
  let cancelled = false;
  getKills(character.id).then((k) => { if (!cancelled) setKills(k); });
  return () => { cancelled = true; };
}, [character.id]);
```

- [ ] **Step 3: TypeScript compile**

```
cd crates/amanuensis-gui/ui && npx tsc -b
```
Expected: clean.

- [ ] **Step 4: Manual smoke test**

```
cd crates/amanuensis-gui && cargo tauri dev
```
- Open a character's Summary view.
- Confirm "Bestiary breakdown" shows two adjacent tables (family + rarity).
- Family table totals match the visible kill counts for that character.

Close.

- [ ] **Step 5: Commit**

```bash
git add crates/amanuensis-gui/ui/src/components/shared/BestiaryBreakdown.tsx \
        crates/amanuensis-gui/ui/src/components/views/SummaryView.tsx
git commit -m "Add bestiary family + rarity kill breakdown to SummaryView"
```

---

## Task 12: KillDetailModal

**Files:**
- Create: `crates/amanuensis-gui/ui/src/components/shared/KillDetailModal.tsx`
- Modify: `crates/amanuensis-gui/ui/src/components/views/KillsView.tsx`

- [ ] **Step 1: Create KillDetailModal.tsx**

```tsx
import type { Kill } from "../../types";
import { useStore } from "../../lib/store";
import { getCreatureImageUrl } from "../../lib/bestiary";

interface KillDetailModalProps {
  kill: Kill;
  onClose: () => void;
}

function sourceLabel(kill: Kill, alias: boolean, inline: boolean): string {
  if (inline) return "inline alias";
  if (alias) return "alias → bestiary";
  return "bestiary";
}

export function KillDetailModal({ kill, onClose }: KillDetailModalProps) {
  const entry = useStore((s) => s.bestiaryByName[kill.creature_name]);
  const aliases = useStore((s) => s.bestiary); // dummy dep to trigger if store updates
  void aliases;
  const imgUrl = getCreatureImageUrl(kill.creature_name);

  const totalKills =
    kill.killed_count +
    kill.slaughtered_count +
    kill.vanquished_count +
    kill.dispatched_count +
    kill.assisted_kill_count +
    kill.assisted_slaughter_count +
    kill.assisted_vanquish_count +
    kill.assisted_dispatch_count;

  return (
    <div
      role="dialog"
      aria-modal="true"
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/60"
      onClick={onClose}
    >
      <div
        className="w-full max-w-2xl rounded-lg border border-[var(--color-border)] bg-[var(--color-bg)] p-5 shadow-xl"
        onClick={(e) => e.stopPropagation()}
      >
        <header className="mb-3 flex items-start gap-3">
          {imgUrl && (
            <img
              src={imgUrl}
              alt={kill.creature_name}
              width={entry?.static_width ?? undefined}
              height={entry?.static_height ?? undefined}
              className="rounded border border-[var(--color-border)]"
            />
          )}
          <div className="min-w-0 flex-1">
            <h2 className="text-lg font-bold">{kill.creature_name}</h2>
            {entry && (
              <p className="text-xs text-[var(--color-text-muted)]">
                {entry.family ?? "Unknown family"} · {entry.rarity ?? "Unknown rarity"}
              </p>
            )}
          </div>
        </header>

        {entry ? (
          <div className="grid grid-cols-1 gap-x-6 gap-y-1 text-sm md:grid-cols-2">
            <Field label="Exp / taxidermy" value={`${entry.exp_taxidermy}`} />
            {entry.location && <Field label="Location" value={entry.location} />}
            {entry.difficulty && <Field label="Difficulty" value={entry.difficulty} long />}
            <Stat label="Attack" value={entry.attack} measured={entry.attack_measured} />
            <Stat label="Defense" value={entry.defense} measured={entry.defense_measured} />
            <Stat label="Damage" value={entry.damage} measured={entry.damage_measured} />
            <Stat label="Health" value={entry.health} measured={entry.health_measured} />
            {entry.frames_per_swing != null && (
              <Field label="Frames / swing" value={`${entry.frames_per_swing}`} />
            )}
            {entry.luck_hits != null && (
              <Field label="Luck hits" value={`${entry.luck_hits}%`} />
            )}
            {entry.is_seasonal && <Field label="Seasonal" value="yes" />}
          </div>
        ) : (
          <p className="text-sm text-[var(--color-text-muted)]">
            No bestiary record for "{kill.creature_name}".
          </p>
        )}

        <footer className="mt-4 flex items-center justify-between">
          <p className="text-xs text-[var(--color-text-muted)]">
            Killed {totalKills} times total
          </p>
          <button
            type="button"
            onClick={onClose}
            className="rounded border border-[var(--color-border)] bg-[var(--color-bg-elevated)] px-3 py-1 text-sm hover:bg-[var(--color-bg-hover)]"
          >
            Close
          </button>
        </footer>
      </div>
    </div>
  );
}

function Field({ label, value, long }: { label: string; value: string; long?: boolean }) {
  return (
    <div className={long ? "md:col-span-2" : ""}>
      <span className="text-xs text-[var(--color-text-muted)]">{label}: </span>
      <span>{value}</span>
    </div>
  );
}

function Stat({
  label,
  value,
  measured,
}: {
  label: string;
  value: number | undefined;
  measured: boolean;
}) {
  if (value == null) return null;
  return (
    <div>
      <span className="text-xs text-[var(--color-text-muted)]">{label}: </span>
      <span>{value}</span>
      {measured && (
        <span className="ml-1 text-[10px] text-[var(--color-accent)]">✓ measured</span>
      )}
    </div>
  );
}
```

Note: the `sourceLabel` helper above is unused in this implementation — the alias source isn't readily available from the frontend bestiary state (it's not exposed on `BestiaryEntry`). Remove the helper and the unused `alias`/`inline` parameters to keep the file clean. Leaving it documented here in case a future iteration exposes source.

The cleaner version omits `sourceLabel` entirely:

```tsx
// Drop the sourceLabel function above; nothing references it.
```

- [ ] **Step 2: Wire `onRowClick` in KillsView.tsx**

Open `crates/amanuensis-gui/ui/src/components/views/KillsView.tsx`. It uses `@tanstack/react-table` via `createColumnHelper` and likely renders rows through a `DataTable` component or a manual `<tr>` loop.

Add:

```tsx
import { useState } from "react";
import type { Kill } from "../../types";
import { KillDetailModal } from "../shared/KillDetailModal";
// ...

const [selectedKill, setSelectedKill] = useState<Kill | null>(null);
```

Inside the row render (find where `<tr>` is emitted; if using a `DataTable` shared component, pass an `onRowClick` prop or wrap the row), add `onClick={() => setSelectedKill(row.original)}` on each row.

Render at the bottom of the component:

```tsx
{selectedKill && (
  <KillDetailModal kill={selectedKill} onClose={() => setSelectedKill(null)} />
)}
```

If `DataTable` is a shared component that doesn't support row click handlers today, the smallest change is to add a `onRowClick?: (row: T) => void` prop. Look at `crates/amanuensis-gui/ui/src/components/shared/DataTable.tsx` first; if the prop already exists, just pass it.

- [ ] **Step 3: TypeScript compile**

```
cd crates/amanuensis-gui/ui && npx tsc -b
```
Expected: clean.

- [ ] **Step 4: Manual smoke test**

```
cd crates/amanuensis-gui && cargo tauri dev
```
- Open a character's Kills view.
- Click a row. Modal appears centered with backdrop.
- Confirm sprite (when available), family, rarity, location, stats, measured badges, total kills, Close button.
- Click backdrop → closes. Click Close → closes.
- Click a row for an obscure creature with no bestiary entry → modal shows the "No bestiary record" message gracefully.

Close.

- [ ] **Step 5: Commit**

```bash
git add crates/amanuensis-gui/ui/src/components/shared/KillDetailModal.tsx \
        crates/amanuensis-gui/ui/src/components/views/KillsView.tsx
git commit -m "Add KillDetailModal with bestiary info on row click"
```

---

## Task 13: Backend kill filters + CLI flags

**Files:**
- Modify: `crates/amanuensis-core/src/db/queries/kill.rs`
- Modify: `crates/amanuensis-cli/src/main.rs`

The frontend filters client-side (it has the kills + bestiary already), so no Tauri command change is needed. The backend gains a filter helper purely for CLI use.

- [ ] **Step 1: Write the failing test**

Append to `crates/amanuensis-core/src/db/queries/kill.rs` tests:

```rust
#[test]
fn test_kills_filter_helper() {
    use crate::data::CreatureDb;

    let db = CreatureDb::bundled().unwrap();

    // Build kills with known creatures.
    let kills = vec![
        Kill::new(0, "Rat".into(), 2),
        Kill::new(0, "Tesla".into(), 70),
        Kill::new(0, "Barracuda".into(), 250),
    ];

    // Family filter
    let filtered: Vec<_> = filter_kills(
        &kills,
        &db,
        &KillsFilter {
            family: Some("Vermine".into()),
            rarity: None,
            seasonal: None,
        },
    );
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].creature_name, "Rat");

    // Rarity filter
    let filtered: Vec<_> = filter_kills(
        &kills,
        &db,
        &KillsFilter {
            family: None,
            rarity: Some("Medium".into()),
            seasonal: None,
        },
    );
    assert!(filtered.iter().any(|k| k.creature_name == "Barracuda"));

    // Combined: family + rarity (Rat is Vermine + Common; expect Rat with family Vermine + Common rarity)
    let filtered: Vec<_> = filter_kills(
        &kills,
        &db,
        &KillsFilter {
            family: Some("Vermine".into()),
            rarity: Some("Common".into()),
            seasonal: None,
        },
    );
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].creature_name, "Rat");
}
```

- [ ] **Step 2: Implement KillsFilter and filter_kills**

Add to the same file (outside the `impl Database` block, since this helper operates on a slice plus a `CreatureDb` and doesn't need the database):

```rust
use crate::data::CreatureDb;

#[derive(Debug, Clone, Default)]
pub struct KillsFilter {
    pub family: Option<String>,
    pub rarity: Option<String>,
    pub seasonal: Option<bool>,
}

/// Filter a slice of kills against the bestiary using family / rarity / seasonal predicates.
/// Returns owned clones for the matched kills.
pub fn filter_kills(kills: &[Kill], db: &CreatureDb, filter: &KillsFilter) -> Vec<Kill> {
    if filter.family.is_none() && filter.rarity.is_none() && filter.seasonal.is_none() {
        return kills.to_vec();
    }
    kills
        .iter()
        .filter(|k| {
            let entry = db.get_entry(&k.creature_name);
            if let Some(want) = &filter.family {
                let f = entry.and_then(|e| e.family.as_deref()).unwrap_or("");
                if !f.eq_ignore_ascii_case(want) {
                    return false;
                }
            }
            if let Some(want) = &filter.rarity {
                let r = entry.and_then(|e| e.rarity.as_deref()).unwrap_or("");
                if !r.eq_ignore_ascii_case(want) {
                    return false;
                }
            }
            if let Some(want) = filter.seasonal {
                let s = entry.map(|e| e.is_seasonal).unwrap_or(false);
                if s != want {
                    return false;
                }
            }
            true
        })
        .cloned()
        .collect()
}
```

If `crate::data::CreatureDb` import is already present in this file, skip the duplicate `use`.

- [ ] **Step 3: Re-export from queries/mod.rs**

In `crates/amanuensis-core/src/db/queries/mod.rs`, add `pub use kill::{KillsFilter, filter_kills};` (or whatever the module-level re-export style is — read the file first).

- [ ] **Step 4: Run tests**

```
cargo test -p amanuensis-core --lib db::queries::kill::tests::test_kills_filter_helper -- --nocapture
```
Expected: 1 pass.

- [ ] **Step 5: Add CLI flags**

In `crates/amanuensis-cli/src/main.rs`, edit the `Commands::Kills` variant:

```rust
/// Show kill statistics
Kills {
    /// Character name
    name: String,
    /// Sort by: total, solo, assisted, value, name
    #[arg(long, default_value = "total")]
    sort: String,
    /// Limit number of results
    #[arg(long)]
    limit: Option<usize>,
    /// Filter by bestiary family (case-insensitive)
    #[arg(long)]
    family: Option<String>,
    /// Filter by bestiary rarity (case-insensitive)
    #[arg(long)]
    rarity: Option<String>,
    /// Only show creatures flagged is_seasonal
    #[arg(long)]
    seasonal: bool,
},
```

Update the dispatch in `run()`:

```rust
Commands::Kills { name, sort, limit, family, rarity, seasonal } => {
    cmd_kills(&db_path, &name, &sort, limit, family, rarity, seasonal)
}
```

Update `cmd_kills` signature and body. After fetching kills:

```rust
fn cmd_kills(
    db_path: &str,
    name: &str,
    sort: &str,
    limit: Option<usize>,
    family: Option<String>,
    rarity: Option<String>,
    seasonal: bool,
) -> amanuensis_core::Result<()> {
    use amanuensis_core::data::CreatureDb;
    use amanuensis_core::db::queries::{filter_kills, KillsFilter};

    let db = Database::open(db_path)?;
    let char = resolve_character(&db, name)?;
    let mut kills = db.get_kills_merged(char.id.unwrap())?;

    if family.is_some() || rarity.is_some() || seasonal {
        let creature_db = CreatureDb::bundled()?;
        kills = filter_kills(
            &kills,
            &creature_db,
            &KillsFilter {
                family,
                rarity,
                seasonal: if seasonal { Some(true) } else { None },
            },
        );
    }

    // ...existing sort / limit / print code...
}
```

(Read the existing `cmd_kills` body before editing; the sort/limit/print path stays the same.)

- [ ] **Step 6: Smoke-test CLI**

```
cargo run -p amanuensis-cli -- kills <CharacterName> --family Vermine
cargo run -p amanuensis-cli -- kills <CharacterName> --rarity Common --limit 5
```
Replace `<CharacterName>` with one that exists in the local DB. Expected: filtered output.

- [ ] **Step 7: Commit**

```bash
git add crates/amanuensis-core/src/db/queries/kill.rs \
        crates/amanuensis-core/src/db/queries/mod.rs \
        crates/amanuensis-cli/src/main.rs
git commit -m "Add KillsFilter + filter_kills helper; CLI kills gains --family/--rarity/--seasonal"
```

---

## Task 14: Frontend kill filter chips

**Files:**
- Create: `crates/amanuensis-gui/ui/src/components/shared/KillsFilterBar.tsx`
- Modify: `crates/amanuensis-gui/ui/src/components/views/KillsView.tsx`

Frontend filters happen in-memory; no new Tauri command needed.

- [ ] **Step 1: Create KillsFilterBar.tsx**

```tsx
import { useMemo } from "react";
import type { Kill } from "../../types";
import { useStore } from "../../lib/store";

export interface KillsFilterState {
  families: Set<string>;
  rarities: Set<string>;
  seasonal: boolean;
}

interface KillsFilterBarProps {
  kills: Kill[];
  value: KillsFilterState;
  onChange: (next: KillsFilterState) => void;
}

export function KillsFilterBar({ kills, value, onChange }: KillsFilterBarProps) {
  const byName = useStore((s) => s.bestiaryByName);

  const { families, rarities } = useMemo(() => {
    const fam = new Set<string>();
    const rar = new Set<string>();
    for (const k of kills) {
      const e = byName[k.creature_name];
      if (e?.family) fam.add(e.family);
      if (e?.rarity) rar.add(e.rarity);
    }
    return {
      families: Array.from(fam).sort(),
      rarities: Array.from(rar).sort(),
    };
  }, [kills, byName]);

  const toggle = (set: Set<string>, key: string): Set<string> => {
    const next = new Set(set);
    if (next.has(key)) next.delete(key);
    else next.add(key);
    return next;
  };

  return (
    <div className="mb-3 flex flex-wrap items-center gap-2 text-xs">
      <span className="text-[var(--color-text-muted)]">Family:</span>
      {families.map((f) => (
        <Chip
          key={f}
          label={f}
          active={value.families.has(f)}
          onClick={() => onChange({ ...value, families: toggle(value.families, f) })}
        />
      ))}
      <span className="ml-3 text-[var(--color-text-muted)]">Rarity:</span>
      {rarities.map((r) => (
        <Chip
          key={r}
          label={r}
          active={value.rarities.has(r)}
          onClick={() => onChange({ ...value, rarities: toggle(value.rarities, r) })}
        />
      ))}
      <Chip
        label="Seasonal"
        active={value.seasonal}
        onClick={() => onChange({ ...value, seasonal: !value.seasonal })}
      />
      {(value.families.size > 0 || value.rarities.size > 0 || value.seasonal) && (
        <button
          type="button"
          className="ml-2 text-[var(--color-accent)] underline"
          onClick={() =>
            onChange({ families: new Set(), rarities: new Set(), seasonal: false })
          }
        >
          Clear
        </button>
      )}
    </div>
  );
}

function Chip({
  label,
  active,
  onClick,
}: {
  label: string;
  active: boolean;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={`rounded-full border px-2 py-0.5 transition ${
        active
          ? "border-[var(--color-accent)] bg-[var(--color-accent)]/15 text-[var(--color-accent)]"
          : "border-[var(--color-border)] text-[var(--color-text-muted)] hover:bg-[var(--color-bg-hover)]"
      }`}
    >
      {label}
    </button>
  );
}
```

- [ ] **Step 2: Wire into KillsView.tsx**

In `crates/amanuensis-gui/ui/src/components/views/KillsView.tsx`:

```tsx
import { useMemo, useState } from "react";
import { KillsFilterBar, type KillsFilterState } from "../shared/KillsFilterBar";
import { useStore } from "../../lib/store";
// ...

const [filter, setFilter] = useState<KillsFilterState>({
  families: new Set(),
  rarities: new Set(),
  seasonal: false,
});

const byName = useStore((s) => s.bestiaryByName);

const visibleKills = useMemo(() => {
  if (filter.families.size === 0 && filter.rarities.size === 0 && !filter.seasonal) {
    return kills;
  }
  return kills.filter((k) => {
    const e = byName[k.creature_name];
    if (filter.families.size > 0) {
      if (!e?.family || !filter.families.has(e.family)) return false;
    }
    if (filter.rarities.size > 0) {
      if (!e?.rarity || !filter.rarities.has(e.rarity)) return false;
    }
    if (filter.seasonal) {
      if (!e?.is_seasonal) return false;
    }
    return true;
  });
}, [kills, filter, byName]);
```

Replace the existing `kills` reference in the table data prop with `visibleKills`. Render `<KillsFilterBar kills={kills} value={filter} onChange={setFilter} />` above the table.

- [ ] **Step 3: TypeScript compile**

```
cd crates/amanuensis-gui/ui && npx tsc -b
```
Expected: clean.

- [ ] **Step 4: Manual smoke test**

```
cd crates/amanuensis-gui && cargo tauri dev
```
- Kills view shows chip rows for Family and Rarity, plus a Seasonal chip.
- Click "Vermine" → table narrows to Vermine creatures.
- Click a second family → both included (OR).
- Click Clear → all chips reset.
- Filter combinations work as expected.

Close.

- [ ] **Step 5: Commit**

```bash
git add crates/amanuensis-gui/ui/src/components/shared/KillsFilterBar.tsx \
        crates/amanuensis-gui/ui/src/components/views/KillsView.tsx
git commit -m "Add family/rarity/seasonal filter chips to KillsView"
```

---

## Task 15: Documentation and final sweep

**Files:**
- Modify: `CLAUDE.md`

- [ ] **Step 1: Update CLAUDE.md**

In `CLAUDE.md`'s "Key Functional Areas" section, append a bullet about the bestiary surface:

```markdown
8. **Bestiary surface**: clicking a row in KillsView opens a modal with the full creature record (family, rarity, location, attack/defense/damage/health with measured indicators, frames-per-swing, difficulty, luck-hits, seasonal). SummaryView shows a "Bestiary completion" card (`X / 969 encountered`) with a per-family table sorted by % complete, plus a "Bestiary breakdown" with per-family and per-rarity kill totals. KillsView has chip filters for family / rarity / seasonal. CLI `kills` supports `--family`, `--rarity`, `--seasonal` flags. The frontend bestiary data is loaded once at app boot via the `get_bestiary` Tauri command and cached in Zustand; sprites live in `crates/amanuensis-gui/ui/public/bestiary/`.
```

- [ ] **Step 2: Run full workspace sweep**

```
cargo test --workspace --lib
cargo clippy --workspace --all-targets -- -D warnings
cd crates/amanuensis-gui/ui && npx tsc -b
```
Expected: all green.

- [ ] **Step 3: Final dev smoke test**

```
cd crates/amanuensis-gui && cargo tauri dev
```
Run through:
- SummaryView shows completion + breakdown.
- KillsView shows chip filters; click-row opens the modal.
- Ranger Stats view still works (the consolidation didn't break it).
- CV Graph view still works.

Close.

- [ ] **Step 4: Commit**

```bash
git add CLAUDE.md
git commit -m "Document bestiary surface in CLAUDE.md Key Functional Areas"
```

---

## Self-Review

**Spec coverage:**
- Consolidation (Tauri command + store + lib rewrite + sweeps + delete) — Tasks 1–7 ✓
- Kill detail modal — Task 12 ✓
- Bestiary completion KPI + per-family table — Task 10 ✓
- Bestiary breakdown (per-family + per-rarity kills) — Task 11 ✓
- Family / rarity / seasonal filters (GUI + CLI) — Tasks 13 (CLI), 14 (GUI) ✓
- Encountered creatures query — Tasks 8, 9 ✓
- Docs — Task 15 ✓

**Placeholder scan:** none.

**Type consistency:**
- `BestiaryEntry` field names match across spec, types.ts (Task 2), store (Task 3), rewritten lib (Task 4), consumer sweeps (Tasks 5–6), modal/breakdown/completion (Tasks 10–12).
- `KillsFilterState` is defined once in `KillsFilterBar.tsx` (Task 14) and re-exported via the named export.
- Backend `KillsFilter` field shape (`family`, `rarity`, `seasonal`) matches the CLI flag names (Task 13).
- `getBestiary()` → `BestiaryPayload` (Task 1 Rust, Task 2 TS) — version + entries shape matches.
- `bestiaryByName` is `Record<string, BestiaryEntry>` everywhere — store (Task 3), bestiary.ts (Task 4), consumer code (Task 6), breakdown (Task 11), modal (Task 12), filter bar (Task 14).
