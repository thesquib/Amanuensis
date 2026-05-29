# Bestiary Surface — Design

**Date**: 2026-05-29
**Status**: Approved (design phase complete; awaiting user spec review)
**Predecessor spec**: `docs/superpowers/specs/2026-05-28-bestiary-update-design.md`

## Goal

Now that the backend bundles a 969-entry richer bestiary (`bestiary.json`), expose that data through the rest of the application: consolidate the parallel frontend bestiary, surface family/rarity/sprite/stats in the kill list, add family + rarity aggregates and a completion badge (overall + per family) to the summary view, and add family/rarity/seasonal filters to the existing kill/search surfaces.

## Background

The previous spec replaced `creatures.csv` with the richer two-file bestiary model. That work is complete on `main`.

Independently, the GUI has been carrying its own bestiary at `crates/amanuensis-gui/data/bestiary_images.json` (932 entries: name → `{family, rarity, pic, w, h, atk, def, dmg, hp, fps}`). It is consumed by:

- `lib/bestiary.ts` — `getCreatureImageUrl`, `getCreatureFamily`, `isStuffable`, `NON_STUFFABLE_FAMILIES` (= `{Ethereal, Insubstantial Undine}`)
- `lib/killStats.ts`
- `lib/rangerStats.ts`
- `components/views/RangerStatsView.tsx`
- `components/views/CVGraphView.tsx`
- `components/shared/CreatureImage.tsx`

The 813 `.gif` sprite files live in `crates/amanuensis-gui/ui/public/bestiary/` and are served by Vite at `/bestiary/<filename>.gif`. Spot-check confirms backend `static_pic` field names match the frontend `pic` field names exactly; 920/969 backend entries resolve to a sprite file we already have.

## Scope

In scope:

- **Consolidation.** Backend becomes the single source of bestiary truth. Frontend loads the bestiary once via Tauri command into Zustand. `bestiary_images.json` is deleted. `lib/bestiary.ts` rewrites against the unified shape. The six consumers stay functionally identical; only their data accessor changes.
- **Kill detail modal.** Clicking a row in `KillsView` opens a centered modal (matching the `MergeDialog` style) showing the full `BestiaryEntry` for that creature: sprite (when available), family, rarity, location, exp/taxidermy, attack/defense/damage/health with `measured` indicator, frames-per-swing, difficulty blurb, luck-hits, seasonal flag, source (bestiary / alias / inline alias).
- **SummaryView additions.**
  - "Bestiary completion" KPI card: `X / 969 encountered (~Y%)`.
  - Collapsible per-family completion table sorted by **% complete** desc, then by family name.
  - "Bestiary breakdown" section: per-family kill totals + per-rarity kill totals (two adjacent small tables).
- **Filters.** CLI `kills` subcommand grows `--family`, `--rarity`, `--seasonal` flags. GUI `KillsView` grows family / rarity filter chips (multi-select). The existing `Search` command stays scoped to log-text FTS5 — it does NOT learn bestiary filters; the user-facing surface for those is the kills filter row.

Out of scope (deferred or rejected):

- Sprite migration to backend assets. Sprites stay in `public/bestiary/`.
- A "what can I kill at my rank" predictor (would need rank → attack/defense logic; tangential).
- New-in-bestiary diff against prior versions (cute, but no upstream consumer asked for it yet).
- A separate "Bestiary" top-level view. Everything bestiary-related lives inside KillsView (detail modal) and SummaryView (aggregates + completion).

## Architecture

### Data flow

```
crates/amanuensis-core/data/bestiary.json
        │
        ▼  include_bytes! → CreatureDb::bundled()
   amanuensis-core CreatureDb
        │
        ▼  tauri::command get_bestiary()
   Vec<BestiaryEntry> over IPC, once at app start
        │
        ▼
   Zustand store slice: bestiary: BestiaryEntry[]  +  byName: Map<string, BestiaryEntry>
        │
        ▼
   lib/bestiary.ts public API (unchanged shape):
     - getCreatureImageUrl(name)
     - getCreatureFamily(name)
     - getBestiaryEntry(name)        ← new
     - isStuffable(name)              ← reuses NON_STUFFABLE_FAMILIES
        │
        ▼
   Six existing consumers + new kill detail modal + new SummaryView sections + new filter chips
```

### Tauri command surface

One new command, added to `crates/amanuensis-gui/src/commands/bestiary.rs`:

```rust
#[tauri::command]
pub fn get_bestiary() -> Result<BestiaryPayload, String> {
    let db = CreatureDb::bundled().map_err(|e| e.to_string())?;
    Ok(BestiaryPayload {
        version: db.bestiary_version().to_string(),
        entries: db.entries().cloned().collect(),
    })
}

#[derive(Serialize)]
pub struct BestiaryPayload {
    version: String,
    entries: Vec<BestiaryEntry>,
}
```

`BestiaryEntry` already derives `Serialize`. The payload is roughly 600–800 KB; the IPC roundtrip on app start is acceptable.

The command is registered in `crates/amanuensis-gui/src/main.rs` alongside the existing handlers.

### Zustand store slice

In `lib/store.ts`:

```ts
interface BestiaryState {
    bestiaryLoaded: boolean;
    bestiaryVersion: string;
    bestiary: BestiaryEntry[];
    bestiaryByName: Map<string, BestiaryEntry>;
}

// Setter at app boot (in App.tsx or equivalent root):
//   await invoke<BestiaryPayload>("get_bestiary")
//      .then(({ version, entries }) => set({ bestiaryLoaded: true, bestiaryVersion: version, bestiary: entries, bestiaryByName: new Map(entries.map(e => [e.name, e])) }))
```

While the bestiary is loading, the GUI renders normally; consumers fall back gracefully (`getCreatureImageUrl` returns `null` on miss, same as today).

### lib/bestiary.ts rewrite

The public function signatures stay the same so the six consumer files don't change behavior. Internals switch from static JSON to a store accessor:

```ts
import { useStore } from "./store";
import type { BestiaryEntry } from "../types";

export type { BestiaryEntry } from "../types";

function lookup(name: string): BestiaryEntry | undefined {
    const map = useStore.getState().bestiaryByName;
    return map.get(name);
}

export function getCreatureImageUrl(name: string): string | null {
    const lookupName = name.startsWith("Captured ")
        ? name.slice("Captured ".length)
        : name;
    const entry = lookup(lookupName);
    return entry?.static_pic ? `/bestiary/${entry.static_pic}` : null;
}

export function getCreatureFamily(name: string): string {
    return lookup(name)?.family ?? "";
}

export function getBestiaryEntry(name: string): BestiaryEntry | undefined {
    return lookup(name);
}

export const NON_STUFFABLE_FAMILIES = new Set([
    "Ethereal",
    "Insubstantial Undine",
]);

export function isStuffable(name: string): boolean {
    const family = getCreatureFamily(name);
    return family.length > 0 && !NON_STUFFABLE_FAMILIES.has(family);
}
```

`bestiaryMap` (currently exported as `Record<string, BestiaryEntry>`) is removed. Two consumers (`killStats.ts`, `rangerStats.ts`) import it directly — they switch to `getBestiaryEntry`. CVGraphView and RangerStatsView use the function helpers only.

`crates/amanuensis-gui/data/bestiary_images.json` is deleted.

### Backend query: bestiary completion

In `crates/amanuensis-core/src/db/queries/`:

```rust
/// Returns (encountered_creature_names) for a character. A creature is "encountered"
/// if it appears in the kills table for this character (any solo/assisted/killed_by count > 0)
/// OR appears in the lastys table for this character.
pub fn get_encountered_creatures(db: &Database, character_id: i64) -> Result<HashSet<String>>;
```

The frontend then computes completion stats client-side using the bundled bestiary. Per-family stats are derived in the GUI (one pass over `bestiary` + `encountered`). No new SQL beyond the encountered query.

### Backend query: family/rarity aggregates for SummaryView

The aggregation is over existing `kills` rows joined against the bestiary (which lives in-memory, not in the DB). Implementation: load `Vec<Kill>` for the character via existing `get_kills`, then aggregate in the frontend:

```ts
// in SummaryView or a derived selector
const kills = useStore(s => s.activeCharacterKills);
const bestiary = useStore(s => s.bestiaryByName);
const byFamily = aggregateBy(kills, k => bestiary.get(k.creature_name)?.family ?? "Unknown");
const byRarity = aggregateBy(kills, k => bestiary.get(k.creature_name)?.rarity ?? "Unknown");
```

No new backend query needed.

### Backend query: kill filters

Existing `get_kills` becomes parameterised:

```rust
pub struct KillsFilter {
    pub family: Option<String>,        // exact match against bestiary entry's family
    pub rarity: Option<String>,
    pub seasonal: Option<bool>,
}

pub fn get_kills_filtered(db: &Database, character_id: i64, filter: KillsFilter) -> Result<Vec<Kill>>;
```

Filtering happens after fetch (small data, in-memory join against bestiary). The existing `get_kills` stays as a thin wrapper with an empty filter.

### CLI flags

`kills` subcommand:
```
amanuensis kills <name> [--family <FAMILY>] [--rarity <RARITY>] [--seasonal]
```

The `--seasonal` flag, when present, restricts to `is_seasonal == true`. Filter combinations AND together. No filter = current behavior.

### GUI filter chips

In `KillsView`, above the existing tanstack-react-table:

- A "Family" chip dropdown (multi-select). Options derived from the union of families present in this character's kills, sorted alphabetically. Selecting one or more chips filters the table.
- A "Rarity" chip dropdown. Same shape.
- An optional "Seasonal" toggle chip.

Chip state is per-character (kept in the existing per-view persisted state in Zustand). No new persistence schema.

### Kill detail modal

A new component `components/shared/KillDetailModal.tsx`:

```tsx
interface KillDetailModalProps {
    kill: Kill;
    onClose: () => void;
}
```

Layout: centered, `fixed inset-0 z-50 flex items-center justify-center bg-black/60` (same as `MergeDialog`). Inner card is wider than `MergeDialog` (`max-w-2xl`) to accommodate stats.

Content:
- Creature image (via existing `CreatureImage`) at the top.
- Name + source indicator (`bestiary` / `alias → bestiary` / `inline alias`).
- Two-column stat grid:
  - Family, Rarity, Location, Difficulty (left column)
  - Exp/taxidermy, Attack, Defense, Damage, Health, Frames/swing, Luck hits, Seasonal (right column)
- Each measured stat (`attack_measured` etc.) shows a small "✓ measured" badge next to the value.
- Footer: "Killed N times" (echoing the kill totals from the row that opened it) and a Close button.

KillsView wires `onRowClick` on the tanstack table to open the modal with that row's `Kill`.

### SummaryView additions

Two new sections appended after the existing summary:

1. **Bestiary completion** card — KPI tile showing `X / TOTAL encountered` and the % rounded. Below it a collapsible `<details>` block: per-family table with columns `Family`, `Encountered`, `Total`, `%`. Sort default: % desc, ties broken by family name asc.

2. **Bestiary breakdown** — two side-by-side tables: "Kills by family" and "Kills by rarity", each with `Name`, `Kills`, `% of total`. Sorted by Kills desc.

## Migration

This pass deletes `crates/amanuensis-gui/data/bestiary_images.json`. The build step that copies it (if any) is removed. The 813 sprite files in `public/bestiary/` are untouched.

The Tauri payload is 600–800 KB. We do not pay this cost on every nav — it's a one-time fetch into the Zustand store, then in-memory thereafter. If profiling later shows the IPC blocks first paint noticeably, the fallback is moving to static fetch (Q5 option C) — a one-line change in the bootstrap.

## Testing

### Backend

- `get_bestiary` Tauri command: smoke test that builds a `BestiaryPayload`, asserts `entries.len() > 950` and `version` matches the bundled value. Lives next to the command in `commands/bestiary.rs`.
- `get_encountered_creatures` unit test: seed kills + lastys for two characters, assert the returned set matches expected names.
- `get_kills_filtered` unit tests: seed kills against a small bestiary fixture, assert family / rarity / seasonal filters narrow correctly (single + combined).

### Frontend

- `lib/bestiary.test.ts` (new) — exercises the rewritten public API against a seeded Zustand store: `getCreatureImageUrl`, `getCreatureFamily`, `isStuffable`, fallback when entry not in store.
- `KillDetailModal` snapshot/render test — given a fixture `Kill` + bestiary entry, asserts measured badges render only for measured stats, source indicator matches alias type.
- SummaryView additions: assertion that with a small fixture (3 kills across 2 families), the family table sorts by % desc.
- Filter chips: render KillsView with a fixture of 5 kills across 3 families, click a family chip, assert the table shows 2 rows (or whatever the fixture math is).

### CLI

- `kills --family Vermine` integration smoke (via assert_cmd if the project has it, otherwise a manual `cargo run` line in the plan).

## Risks

- **First-paint delay.** 600–800 KB IPC at boot. Mitigation: render the UI optimistically; consumers gracefully handle empty bestiary by returning `null` / `""`. Worst case the bestiary appears after 100–300 ms; not enough to be visible.
- **Drift between frontend type and backend struct.** `BestiaryEntry` is defined in `crates/amanuensis-core/src/data/bestiary.rs` and needs a matching TypeScript type in `ui/src/types.ts`. This pass adds it by hand; future-proofing via `ts-rs` or schemars is out of scope. Risk: a backend field rename silently breaks the frontend. Mitigation: the existing consumer tests catch the most common breakages, and CI builds the GUI.
- **NON_STUFFABLE_FAMILIES staying hand-coded.** This list is editorial domain knowledge that doesn't live in the bestiary. Leaving it inline in `lib/bestiary.ts` is the lowest-friction option; a future pass could move it to `bestiary_aliases.json` if it grows.

## Build sequence (preview for the implementation plan)

1. Backend: add `get_bestiary` Tauri command + `BestiaryPayload`. Register in `main.rs`.
2. Frontend: add `BestiaryEntry` TS type mirroring the Rust struct.
3. Frontend: add bestiary Zustand slice + boot-time loader.
4. Rewrite `lib/bestiary.ts` against the store, keeping public API stable.
5. Sweep `killStats.ts`, `rangerStats.ts`, `CVGraphView.tsx`, `RangerStatsView.tsx`, `CreatureImage.tsx` for `bestiaryMap` direct imports → swap to function helpers.
6. Delete `crates/amanuensis-gui/data/bestiary_images.json` and any build refs.
7. Backend: `get_encountered_creatures` query + tests.
8. Backend: `get_kills_filtered` + thin wrapper, CLI flags.
9. Build `KillDetailModal`, wire `onRowClick` in `KillsView`.
10. Add Bestiary completion KPI + per-family table to `SummaryView`.
11. Add Bestiary breakdown (family + rarity aggregate tables) to `SummaryView`.
12. Add Family / Rarity / Seasonal filter chips above the `KillsView` table.
13. Run `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, build the GUI, smoke-test in `cargo tauri dev`.
14. Update CLAUDE.md "Key Functional Areas" with the new surfaces.
