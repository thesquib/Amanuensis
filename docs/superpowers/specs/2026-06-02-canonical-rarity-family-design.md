# Canonical Rarity & Family Normalization

**Date:** 2026-06-02
**Status:** Approved, ready for implementation

## Problem

The bestiary surface groups and filters kills by the raw `rarity` and `family`
strings stored in `bestiary.json`. Both come from the upstream `clnet_bestiary`
dump and are inconsistent:

- **Rarity** is free text — 65 distinct values across 969 entries. Beyond the
  clean `Common`/`Medium`/`Rare`/`Exotic`, there are descriptive strings like
  `"Medium for pink snakes, common for green snakes"`,
  `"Rare (Melabrion's), Common (Wendecka Breeding Grounds)"`, `"Common."`,
  `"Medium-Rare?"`, `"Unique (Boss)"`, etc. The "By rarity" breakdown
  (SummaryView) and the Rarity chip row (KillsView filter bar) show every one
  of these as a separate row/chip.
- **Family** is a clean categorical token (66 distinct values) with one true
  case-duplicate: `EXTINCT` (1 entry) vs `Extinct` (31).

## Goal

Collapse rarity into a small canonical set and de-duplicate family casing, so the
breakdown tables, filter chips, kill filtering, and detail modal all use tidy,
consistent labels. The normalization logic lives **only** in `amanuensis-core`
(single source of truth); the GUI consumes pre-computed canonical labels rather
than reimplementing the logic in TypeScript.

## Rarity — canonical buckets

Seven buckets, ordered lowest (most common) → highest (rarest):

```
Common < Medium < Rare < Unique < Exotic < GM Only
```

plus **Unknown** for anything unresolvable.

### Lowest-common-denominator rule

A pure per-string function `canonical_rarity(Option<&str>) -> Rarity`:

1. `None` → `Unknown`.
2. Lowercase the string. Remove the substring `uncommon` first, so it does not
   false-match `common`.
3. Scan for keywords and collect matched buckets:
   - `common` → Common
   - `medium` → Medium
   - `rare` → Rare
   - `unique` **or** `boss` → Unique
   - `exotic` → Exotic
   - `gm only` → GM Only
4. If any matched, return the **lowest** (the "lowest common denominator" — a
   creature common somewhere is treated as common).
5. If none matched: contains `extinct` → **Unique**; otherwise → **Unknown**.

`Rarity` is an enum deriving `Ord` with variants declared in rank order
(Common … GM Only, Unknown last), so `.min()` yields the LCD. It exposes
`as_label()` returning the display string (`"GM Only"`, `"Unknown"`, …).

### Verified distribution (969 entries)

| Common | Medium | Rare | Unique | Exotic | GM Only | Unknown |
|--------|--------|------|--------|--------|---------|---------|
| 448    | 355    | 69   | 42     | 26     | 4       | 25      |

Spot-checks (encoded as unit tests):

| Raw                                               | Canonical |
|---------------------------------------------------|-----------|
| `"Common"` / `"Common."` / `"Common. (Obviously.)"` | Common  |
| `"Rare (Melabrion's), Common (Wendecka…)"`         | Common    |
| `"Medium-Rare"` / `"Medium-Rare?"`                 | Medium    |
| `"Medium (or 'uncommon')"`                         | Medium    |
| `"Exotic (GM only?)"` / `"Exotic or GM only"`      | Exotic    |
| `"GM Only"`                                         | GM Only   |
| `"Unique (Boss)"` / `"Boss"`                        | Unique    |
| `"Extinct"` / `"Extinct."`                          | Unique    |
| `"Once per year!"` / `"Not Applicable"` / `None`    | Unknown   |

## Family — case-fold de-duplication

Family is not free text, so no bucketing. Instead, group all family values
case-insensitively and pick the **most-common casing** in each group as the
canonical label. Data-driven, so any future case-dup is handled automatically.

Current effect: `EXTINCT` (1) folds into `Extinct` (31). No typo merging —
`Org` (the Org-Cave beasts: Org, Mammoth Org, Young Org) and `Orga` (the orc-like
humanoids) are genuinely distinct families and stay separate. `Uncategorized`
(32) is a legitimate catch-all and stays.

Because this needs the whole entry set, the case-fold map is built once at
bestiary load.

## Architecture

### `amanuensis-core`

1. New `Rarity` enum + `canonical_rarity(Option<&str>) -> Rarity` (pure function,
   with `.as_label()`). Lives in a new `data/rarity.rs` re-exported from the
   `data` module.
2. `CreatureDb::from_json_bytes` builds a family case-fold map
   (`HashMap<lowercased, canonical>`, canonical = most-common casing, ties broken
   deterministically — e.g. by descending count then the lexicographically
   smallest variant). New method `canonical_family(&self, raw: &str) -> &str`
   returning the canonical casing (or the input unchanged if unseen).
3. `BestiaryEntry` gains two transport fields:
   - `rarity_canonical: Option<String>`
   - `family_canonical: Option<String>`

   Both `#[serde(default, skip_serializing_if = "Option::is_none")]`, so neither
   is written to `bestiary.json` and both default to `None` when the file is
   loaded. They are populated only when serving the bestiary to the GUI.
4. `filter_kills`: rarity predicate compares `canonical_rarity(entry.rarity)`
   (case-insensitive on the label); family predicate compares
   `db.canonical_family(entry.family)`.

### `amanuensis-gui` backend

`get_bestiary` command sets `rarity_canonical` and `family_canonical` on each
entry (via `canonical_rarity` and `db.canonical_family`) before returning the
payload.

### Frontend (TypeScript)

5. `types.ts`: add `rarity_canonical?: string` and `family_canonical?: string`
   to `BestiaryEntry`.
6. `BestiaryBreakdown.tsx`: "By rarity" groups on `rarity_canonical`; "By family"
   on `family_canonical`.
7. `KillsFilterBar.tsx`: rarity chips built from `rarity_canonical`, sorted by
   rarity rank (Common → … → Unknown); family chips from `family_canonical`
   (the `EXTINCT`/`Extinct` duplicate collapses to a single chip).
8. `KillsView.tsx`: `visibleKills` filter matches on `rarity_canonical` /
   `family_canonical`.
9. `KillDetailModal.tsx`: shows canonical rarity and canonical family.
10. Remove the now-unused `normalizeBestiaryLabel` from `lib/bestiary.ts` (its
    only consumer was the rarity grouping).

### CLI

- `kills --rarity` / `--family` already route through `filter_kills`, so they
  pick up canonical matching automatically. Update the `--rarity` help text to
  list the seven buckets.
- `bestiary <name>` prints the canonical rarity and family.

## Testing (TDD)

Write tests first for each unit:

- **`canonical_rarity`** — a table-driven unit test covering every row of the
  spot-check table above, plus a guard that all 969 bundled entries resolve to
  one of the seven buckets.
- **`CreatureDb::canonical_family`** — `EXTINCT` → `Extinct`; `Orga` stays
  `Orga`; an unseen value returns unchanged.
- **`filter_kills`** — filtering by `"Common"` includes a creature whose raw
  rarity is `"Common."`; filtering by `"Unique"` includes an `Extinct` creature;
  family filter treats `EXTINCT` and `Extinct` as the same bucket.
- **`get_bestiary`** — returned entries have `rarity_canonical` and
  `family_canonical` populated.
- Existing `filter_kills` and bestiary tests must continue to pass.

## Out of scope

- Family typo merging beyond case (e.g. `Org`/`Orga` stay separate).
- Family `Uncategorized` stays.
- No change to `bestiary.json` on disk or to `update-bestiary`.
