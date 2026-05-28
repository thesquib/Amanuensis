# Bestiary Update — Design

**Date**: 2026-05-28
**Status**: Approved (design phase complete; awaiting user spec review)
**Source data**: `~/Downloads/bestiary_20260520_fullexport.xml` (phpMyAdmin XML dump of `clnet_bestiary.creatures`, 969 rows)

## Goal

Replace the bundled `creatures.csv` (943 rows, `name,value` only) with a richer bestiary database derived from the upstream `clnet_bestiary` dump, preserving all current kill-tracking behavior and exposing the additional fields (family, rarity, location, attack/defense/etc., sprite metadata) for future surfaces. Update is reproducible via a CLI command.

## Background

`crates/amanuensis-core/data/creatures.csv` is loaded via `include_bytes!` into `CreatureDb`, which exposes `get_value(name) -> Option<i32>`. The parser uses that value to populate `Kill.creature_value`, which drives `coin_level`, `highest_kill`, and `nemesis`.

The current CSV's `value` column corresponds to the bestiary's `exp_taxidermy` field (verified against Rat=2, Leech=5, Tesla=70, Barracuda=250).

The bestiary uses **parenthetical disambiguation** in its primary key (e.g. `the Ramandu (boss)`, `Mushroom (Brown)`, `Mushroom (Purple)`, `(Super) Crookbeak Kestrel`). Log messages use shorter natural forms (`the Ramandu`, `Mushroom`). The existing CSV flattens both naming conventions into a single key space with editorial choices (e.g., bare `the Ramandu` → 2620, the boss value). Those editorial choices must be preserved.

## Scope

In scope:
- New `BestiaryEntry` struct holding the full 27-column bestiary record.
- New bundled `bestiary.json` (generated from XML, sorted by name).
- New bundled `bestiary_aliases.json` (hand-curated overlay, checked in).
- `CreatureDb` reworked to load from the JSON pair; `get_value` behavior preserved; new `get_entry` method added.
- `creatures.csv` deleted.
- CLI: `amanuensis update-bestiary <xml-path> [--aliases <path>] [--dry-run]` regenerates `bestiary.json` and validates aliases.
- CLI: `amanuensis bestiary <name>` prints the full record (with alias resolution).
- Tests: unit coverage for all alias resolution paths plus a fixture-based XML parse round-trip.

Out of scope (future passes):
- GUI changes (kill-detail drawer, family aggregates, search-by-family). Data will be available; UI consumes incrementally.
- Sprite asset bundling (only metadata is retained).

## User impact / rescan policy

Bestiary updates are treated as a reason to rescan. Existing stored `Kill.creature_value` rows are *not* automatically migrated; instead the user is encouraged to run `amanuensis scan --force` after the binary update to bring all kill rows onto the new bestiary values. This is "getting the user better data" — not just preserving the old totals.

To make the decision visible:
- `bestiary.json` carries a top-level `version` field (the XML's dump date in `YYYYMMDD` form, e.g. `"20260520"`). This becomes the canonical bestiary version stamp.
- `CreatureDb` exposes `bestiary_version() -> &str`.
- `Log` (or a sibling `bestiary_version_at_scan` column on `Kill`) is *not* added in this pass — that's a follow-up if we want per-row epoch tracking. For now, the version is exposed and the user is told to rescan.
- The `update-bestiary` CLI prints a one-line reminder after a successful write: `Bestiary updated to version 20260520. Existing databases should run 'amanuensis scan --force <folder>' to refresh kill values.`
- The release notes / CLAUDE.md update mentions the rescan recommendation alongside the new command.

## Architecture

### Data shape

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BestiaryEntry {
    pub name: String,                    // canonical bestiary primary key
    pub family: Option<String>,
    pub location: Option<String>,
    pub information: Option<String>,
    pub exp_taxidermy: i32,              // == the current CSV "value"
    pub rarity: Option<String>,
    pub worth: Option<i32>,
    pub worth_range: Option<String>,
    pub frames_per_swing: Option<f64>,
    pub difficulty: Option<String>,
    pub attack: Option<i32>,
    pub defense: Option<i32>,
    pub damage: Option<i32>,
    pub health: Option<i32>,
    pub attack_measured: bool,           // from attack_ismeasured tinyint
    pub defense_measured: bool,
    pub damage_measured: bool,
    pub health_measured: bool,
    pub luck_hits: Option<i32>,
    pub is_seasonal: bool,
    pub first_update: Option<String>,    // ISO date string, as-is
    pub last_update: Option<String>,
    pub static_pic: Option<String>,
    pub static_width: Option<i32>,
    pub static_height: Option<i32>,
    pub action_pic: Option<String>,
    pub action_width: Option<i32>,
    pub action_height: Option<i32>,
}
```

Full 27-column fidelity. NULL in the XML maps to `None`. The `*_ismeasured` tinyint columns coerce `1` → `true`, `0`/`NULL` → `false`. The `source` indicator (bestiary vs. alias) is tracked separately on lookup rather than per-entry.

### Alias overlay shape

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum BestiaryAlias {
    Resolves {
        log_name: String,
        resolves_to: String,    // must match a BestiaryEntry.name
    },
    Inline {
        log_name: String,
        inline: InlineEntry,    // for log-only names with no bestiary record
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InlineEntry {
    pub exp_taxidermy: i32,
    // other BestiaryEntry fields optional; default to None
    pub family: Option<String>,
    pub rarity: Option<String>,
    // ...
}
```

Stored as a JSON array. Load-time validation: every `resolves_to` target must exist in `bestiary.json`; duplicate `log_name` entries are an error.

### CreatureDb

```rust
pub struct CreatureDb {
    by_name: HashMap<String, BestiaryEntry>,        // canonical bestiary names
    aliases: HashMap<String, ResolvedAlias>,         // log name → resolution
}

enum ResolvedAlias {
    Pointer(String),         // canonical name in by_name
    Inline(BestiaryEntry),   // synthetic entry from InlineEntry
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntrySource {
    Bestiary,           // direct hit on bestiary.json
    Alias,              // resolved via a `resolves_to` alias
    InlineAlias,        // synthesized from an `inline` alias entry
}

impl CreatureDb {
    pub fn bundled() -> Result<Self>;                                            // includes both JSON files
    pub fn from_json_bytes(bestiary: &[u8], aliases: &[u8]) -> Result<Self>;
    pub fn get_value(&self, log_name: &str) -> Option<i32>;                     // returns exp_taxidermy
    pub fn get_entry(&self, log_name: &str) -> Option<&BestiaryEntry>;
    pub fn get_entry_with_source(&self, log_name: &str) -> Option<(&BestiaryEntry, EntrySource)>;
    pub fn entries(&self) -> impl Iterator<Item = &BestiaryEntry>;
    pub fn bestiary_version(&self) -> &str;                                      // YYYYMMDD stamp
    pub fn len(&self) -> usize;
    pub fn is_empty(&self) -> bool;
}
```

**Lookup order** (same for `get_value` and `get_entry`):
1. Aliases — if `log_name` is in `aliases`, follow the pointer or return the inline entry.
2. Bestiary direct — `by_name.get(log_name)`.
3. Strip leading `"the "` from `log_name` and retry steps 1+2 (preserves existing boss-prefix fallback behavior).

### CLI commands

#### `amanuensis update-bestiary <xml-path> [--aliases <path>] [--dry-run]`

1. Parse the phpMyAdmin XML dump using `quick-xml` (already-acceptable dep or to be added).
2. Convert each `<table name="creatures">` block into a `BestiaryEntry`.
3. Load the alias file (default `crates/amanuensis-core/data/bestiary_aliases.json`).
4. Validate aliases: every `resolves_to` must resolve in the new bestiary; no duplicate `log_name`. Errors abort the write.
5. Print summary:
   ```
   XML:         969 entries
   Aliases:     26 resolves, 0 inline
   Overrides:   3 aliases hide a same-named bestiary entry
   Written:     crates/amanuensis-core/data/bestiary.json (sorted by name)
   ```
6. `--dry-run` skips the write; everything else runs.

#### `amanuensis bestiary <name>`

Prints a single creature's full record. Uses the same alias-aware lookup as the parser. Output includes a source line indicating whether the hit came from the bestiary, an alias pointer, or an inline alias.

## Data flow

```
~/Downloads/bestiary_*.xml
        │
        ▼  amanuensis update-bestiary
   parse XML ───────► validate against aliases
        │                       │
        ▼                       ▼
  bestiary.json     bestiary_aliases.json  (hand-edited, committed)
        │                       │
        └────► include_bytes! ──┘
                     │
                     ▼
              CreatureDb::bundled
                     │
                     ▼
              parser get_value()
                     │
                     ▼
              Kill.creature_value
```

## Migration / seeding the alias file

Symmetric diff between current CSV and new XML is computed during implementation. Each non-matching name becomes a proposed alias entry, presented to the user before commit:

- **CSV name not in XML** → candidate for `Inline` alias (log name still in use, bestiary lacks the record) or deletion (if the name is obsolete).
- **XML name not in CSV** → no alias needed (it's a new entry the parser will pick up directly).
- **Editorial pairs** (e.g., the Ramandu boss vs clone) → explicit `Resolves` aliases.

Known seed entries:
- `{ "log_name": "the Ramandu", "resolves_to": "the Ramandu (boss)" }` — preserves current 2620 behavior.
- `{ "log_name": "Ramandu", "resolves_to": "the Ramandu" }` — preserves current 666 behavior.
- Mushroom and `(Super) Crookbeak Kestrel` cases reviewed during the symmetric-diff pass.

The full proposed alias file is reviewed by the user before being written.

## Testing

### Unit tests in `crates/amanuensis-core/src/data/creatures.rs`

Reused (must continue to pass):
- `test_load_bundled_creatures` — assert `db.len() > 800` (will be > 990 after refresh).
- `test_known_creatures` — `Rat=2`, `Leech=5`, `Tesla=70`, `Barracuda=250`.
- `test_unknown_creature` — `Nonexistent Creature XYZ` returns `None`.
- `test_the_ramandu_boss_value` — `the Ramandu` → 2620, `Ramandu` → 666 (both must continue to pass; this is the editorial-preservation test).
- `test_the_prefix_fallback` — fallback for entries without an alias.

New:
- `test_get_entry_returns_full_record` — `db.get_entry("Tesla")` returns family `"Annelida"`, rarity `"Common"`, etc.
- `test_alias_resolves_to` — fixture alias `{ log_name: "Foo", resolves_to: "Bar" }` plus bestiary entry `Bar` → `get_value("Foo") == Bar.exp_taxidermy`, `get_entry("Foo")` returns the `Bar` entry.
- `test_alias_inline` — fixture inline alias with `exp_taxidermy: 500` → `get_value("Foo") == Some(500)`, `get_entry("Foo")` returns a synthetic entry with that value.
- `test_alias_dangling_resolves_to` — alias `resolves_to: "Missing"` → `from_json_bytes` returns `AmanuensisError::Data`.
- `test_alias_duplicate_log_name` — two aliases with the same `log_name` → load error.
- `test_alias_overrides_bestiary` — if `log_name` matches both an alias and a bestiary entry, the alias wins.
- `test_alias_lookup_with_the_prefix_fallback` — `the Foo` → alias miss → strip-and-retry hits `Foo` alias.

### XML parser tests in `crates/amanuensis-core/src/data/bestiary_import.rs` (new file)

- `test_parse_minimal_xml` — small two-creature fixture in a `tests/fixtures/` file → two correctly-populated `BestiaryEntry` values.
- `test_parse_null_handling` — fixture with `NULL` columns → fields become `None` / `false`.
- `test_parse_html_entities` — `&#039;` → `'`, `&amp;` → `&`.
- `test_parse_ismeasured_tinyint` — `1` → `true`, `0` → `false`, `NULL` → `false`.
- `test_parse_full_fixture` — a 20-entry hand-trimmed fixture from the real XML → exact field-by-field comparison.

### CLI integration tests

- `test_update_bestiary_dry_run` — invokes the command on a fixture XML, asserts no file is written, summary is printed.
- `test_update_bestiary_writes_sorted` — writes to a temp path, asserts entries appear sorted by name in the output.
- `test_update_bestiary_alias_validation_fails` — alias with dangling `resolves_to` exits non-zero, file is not written.
- `test_bestiary_command_prints_record` — `amanuensis bestiary Rat` includes family, rarity, exp_taxidermy.
- `test_bestiary_command_via_alias` — `amanuensis bestiary "the Ramandu"` shows the boss entry plus an alias-source indicator.

### Real-data comparison tests

Existing `--ignored` tests in `crates/amanuensis-core/tests/real_data_comparison.rs` (Ruuk, Olga, Squib, Tu Whawha, Tane) are rerun manually after the regeneration. No new tests added at this layer; they exist to catch regressions in `creature_value`-derived stats (`coin_level`, `highest_kill`, `nemesis`).

## Open questions

None remain at design time. The symmetric-diff alias seeding is presented to the user at implementation time, not design time.

## Risks

- **Alias file rot**: someone refreshes the bestiary later, an entry referenced by an alias gets renamed, and the load validation breaks. Mitigation: `update-bestiary` runs the same validation before writing, so the failure surfaces at refresh time, not parse time.
- **Editorial drift**: a future bestiary edit changes `exp_taxidermy` for a creature that's been the subject of long-term player stat tracking. Stored `Kill.creature_value` rows aren't migrated automatically; the rescan policy in "User impact" addresses this — running `amanuensis scan --force` after a bestiary update rewrites all kill values from the new data.
- **JSON file size**: `bestiary.json` is estimated at 600-800KB. Within `include_bytes!` tolerance, but increases binary size by roughly that amount.

## Build sequence (preview for the implementation plan)

1. Add `BestiaryEntry` struct and the alias enum types.
2. Build the XML importer (`bestiary_import.rs`) with a small fixture-based test suite first (TDD).
3. Build `CreatureDb` against the new types, with the alias-resolution lookup. Reuse existing tests verbatim where possible; add the new ones.
4. Compute the CSV→XML symmetric diff. Present the proposed alias file to the user for review.
5. Run the importer over the real XML. Commit `bestiary.json` and `bestiary_aliases.json`.
6. Delete `creatures.csv`.
7. Add the `update-bestiary` and `bestiary` CLI subcommands.
8. Run the full unit suite + the `--ignored` real-data tests. Confirm no `creature_value`-derived metric regresses across the five tracked characters.
9. Update `CLAUDE.md`'s "Updated Data Sources" section to point at `amanuensis update-bestiary` and mention the post-update rescan recommendation.
