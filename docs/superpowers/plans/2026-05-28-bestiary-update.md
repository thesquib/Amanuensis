# Bestiary Update Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the bundled `creatures.csv` with a richer two-file bestiary (`bestiary.json` + `bestiary_aliases.json`) generated from the upstream `clnet_bestiary` XML dump, preserving all existing kill-tracking behavior and exposing the full bestiary record via a new CLI lookup command.

**Architecture:** A new `bestiary` submodule under `data/` holds the `BestiaryEntry` struct and the alias overlay types. An XML importer module converts the phpMyAdmin dump into our internal JSON. `CreatureDb` is reworked to load from the bundled JSON pair and resolve lookups through aliases-first → direct → `the X` strip-and-retry. Two new CLI commands cover regeneration (`update-bestiary`) and per-creature inspection (`bestiary`).

**Tech Stack:** Rust 2021, serde + serde_json (already in deps), `quick-xml` (new dep) for the XML importer, clap for CLI, existing test infra (`tempfile` dev-dep).

**Spec:** `docs/superpowers/specs/2026-05-28-bestiary-update-design.md`

---

## File Structure

**New files:**
- `crates/amanuensis-core/src/data/bestiary.rs` — `BestiaryEntry`, `BestiaryAlias`, `InlineEntry`, `EntrySource` types + serde tests.
- `crates/amanuensis-core/src/data/bestiary_import.rs` — XML → `Vec<BestiaryEntry>` parser.
- `crates/amanuensis-core/tests/fixtures/bestiary_minimal.xml` — two-creature fixture for parser tests.
- `crates/amanuensis-core/tests/fixtures/bestiary_full.xml` — 20-entry fixture for round-trip tests.
- `crates/amanuensis-core/data/bestiary.json` — generated from the real XML during Task 7.
- `crates/amanuensis-core/data/bestiary_aliases.json` — hand-curated, seeded in Task 6.

**Modified files:**
- `crates/amanuensis-core/Cargo.toml` — add `quick-xml`.
- `crates/amanuensis-core/src/data/mod.rs` — re-export new types.
- `crates/amanuensis-core/src/data/creatures.rs` — full rewrite to load JSON pair; existing tests preserved.
- `crates/amanuensis-cli/src/main.rs` — add `UpdateBestiary` and `Bestiary` subcommands and their handlers.
- `CLAUDE.md` — point "Updated Data Sources" at `amanuensis update-bestiary` and mention the rescan policy.

**Deleted files:**
- `crates/amanuensis-core/data/creatures.csv` — superseded by `bestiary.json`.

---

## Task 1: Add bestiary type module

**Files:**
- Create: `crates/amanuensis-core/src/data/bestiary.rs`
- Modify: `crates/amanuensis-core/src/data/mod.rs`

- [ ] **Step 1: Write the failing serde round-trip test**

Create `crates/amanuensis-core/src/data/bestiary.rs`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BestiaryEntry {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub family: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub information: Option<String>,
    pub exp_taxidermy: i32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rarity: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worth: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worth_range: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frames_per_swing: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub difficulty: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attack: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub defense: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub damage: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub health: Option<i32>,
    #[serde(default)]
    pub attack_measured: bool,
    #[serde(default)]
    pub defense_measured: bool,
    #[serde(default)]
    pub damage_measured: bool,
    #[serde(default)]
    pub health_measured: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub luck_hits: Option<i32>,
    #[serde(default)]
    pub is_seasonal: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_update: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_update: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub static_pic: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub static_width: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub static_height: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action_pic: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action_width: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action_height: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum BestiaryAlias {
    Resolves {
        log_name: String,
        resolves_to: String,
    },
    Inline {
        log_name: String,
        inline: InlineEntry,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct InlineEntry {
    pub exp_taxidermy: i32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub family: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rarity: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub information: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntrySource {
    Bestiary,
    Alias,
    InlineAlias,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BestiaryFile {
    pub version: String,                 // YYYYMMDD
    pub entries: Vec<BestiaryEntry>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entry_roundtrips_through_json() {
        let entry = BestiaryEntry {
            name: "Rat".into(),
            family: Some("Vermine".into()),
            exp_taxidermy: 2,
            rarity: Some("Common".into()),
            attack: Some(65),
            attack_measured: true,
            is_seasonal: false,
            ..default_entry()
        };
        let json = serde_json::to_string(&entry).unwrap();
        let back: BestiaryEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(entry, back);
    }

    #[test]
    fn alias_resolves_roundtrips() {
        let alias = BestiaryAlias::Resolves {
            log_name: "the Ramandu".into(),
            resolves_to: "the Ramandu (boss)".into(),
        };
        let json = serde_json::to_string(&alias).unwrap();
        let back: BestiaryAlias = serde_json::from_str(&json).unwrap();
        assert_eq!(alias, back);
    }

    #[test]
    fn alias_inline_roundtrips() {
        let alias = BestiaryAlias::Inline {
            log_name: "Old Critter".into(),
            inline: InlineEntry {
                exp_taxidermy: 500,
                family: Some("Legacy".into()),
                rarity: None,
                location: None,
                information: None,
            },
        };
        let json = serde_json::to_string(&alias).unwrap();
        let back: BestiaryAlias = serde_json::from_str(&json).unwrap();
        assert_eq!(alias, back);
    }

    fn default_entry() -> BestiaryEntry {
        BestiaryEntry {
            name: String::new(),
            family: None,
            location: None,
            information: None,
            exp_taxidermy: 0,
            rarity: None,
            worth: None,
            worth_range: None,
            frames_per_swing: None,
            difficulty: None,
            attack: None,
            defense: None,
            damage: None,
            health: None,
            attack_measured: false,
            defense_measured: false,
            damage_measured: false,
            health_measured: false,
            luck_hits: None,
            is_seasonal: false,
            first_update: None,
            last_update: None,
            static_pic: None,
            static_width: None,
            static_height: None,
            action_pic: None,
            action_width: None,
            action_height: None,
        }
    }
}
```

Add the module to `crates/amanuensis-core/src/data/mod.rs`:

```rust
pub mod bestiary;
pub mod creatures;
pub mod trainer_checkpoints;
pub mod trainers;

pub use bestiary::{BestiaryEntry, BestiaryAlias, InlineEntry, EntrySource, BestiaryFile};
pub use creatures::CreatureDb;
pub use trainer_checkpoints::lookup_checkpoint_message;
pub use trainers::{TrainerDb, TrainerMeta};
```

- [ ] **Step 2: Run tests to verify they pass**

Run: `cargo test -p amanuensis-core --lib data::bestiary -- --nocapture`
Expected: 3 PASS (entry_roundtrips_through_json, alias_resolves_roundtrips, alias_inline_roundtrips)

- [ ] **Step 3: Commit**

```bash
git add crates/amanuensis-core/src/data/bestiary.rs crates/amanuensis-core/src/data/mod.rs
git commit -m "Add BestiaryEntry, BestiaryAlias, BestiaryFile types"
```

---

## Task 2: Add quick-xml dep and XML importer skeleton

**Files:**
- Modify: `crates/amanuensis-core/Cargo.toml`
- Create: `crates/amanuensis-core/src/data/bestiary_import.rs`
- Create: `crates/amanuensis-core/tests/fixtures/bestiary_minimal.xml`
- Modify: `crates/amanuensis-core/src/data/mod.rs`

- [ ] **Step 1: Add quick-xml to Cargo.toml**

Edit `crates/amanuensis-core/Cargo.toml`:

```toml
[dependencies]
regex = "1"
once_cell = "1"
rusqlite = { version = "0.31", features = ["bundled"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
csv = "1"
chrono = { version = "0.4", features = ["serde"] }
thiserror = "1"
log = "0.4"
encoding_rs = "0.8"
quick-xml = "0.31"
```

Run: `cargo check -p amanuensis-core`
Expected: clean build (quick-xml downloads on first run).

- [ ] **Step 2: Write the minimal fixture**

Create `crates/amanuensis-core/tests/fixtures/bestiary_minimal.xml`:

```xml
<?xml version="1.0" encoding="utf-8"?>
<pma_xml_export version="1.0">
    <database name="clnet_bestiary">
        <table name="creatures">
            <column name="name">Rat</column>
            <column name="family">Vermine</column>
            <column name="location">Everywhere</column>
            <column name="information">Chew fallen. Feral.</column>
            <column name="exp_taxidermy">2</column>
            <column name="rarity">Common</column>
            <column name="worth">0</column>
            <column name="worth_range">Fur: 0 &#8211; 1</column>
            <column name="framesperswing">18</column>
            <column name="difficulty">Easy.</column>
            <column name="attack">65</column>
            <column name="defense">35</column>
            <column name="damage">10</column>
            <column name="health">2</column>
            <column name="defense_ismeasured">1</column>
            <column name="attack_ismeasured">1</column>
            <column name="damage_ismeasured">1</column>
            <column name="health_ismeasured">1</column>
            <column name="static_pic">ratnew.gif</column>
            <column name="static_width">18</column>
            <column name="static_height">22</column>
            <column name="action_pic">Action_Rat.gif</column>
            <column name="action_width">226</column>
            <column name="action_height">209</column>
            <column name="last_update">2026-02-11 01:31:08</column>
            <column name="first_update">NULL</column>
            <column name="luck_hits">NULL</column>
            <column name="is_seasonal">0</column>
        </table>
        <table name="creatures">
            <column name="name">Venomous Leech</column>
            <column name="family">Extinct</column>
            <column name="location">Dal&#039;Nzoth Waters</column>
            <column name="information">Eradicated.</column>
            <column name="exp_taxidermy">0</column>
            <column name="rarity">Extinct</column>
            <column name="worth">0</column>
            <column name="worth_range">0</column>
            <column name="framesperswing">0</column>
            <column name="difficulty">Unknown.</column>
            <column name="attack">5</column>
            <column name="defense">5</column>
            <column name="damage">30</column>
            <column name="health">15</column>
            <column name="defense_ismeasured">NULL</column>
            <column name="attack_ismeasured">NULL</column>
            <column name="damage_ismeasured">NULL</column>
            <column name="health_ismeasured">NULL</column>
            <column name="static_pic">venomousleech.gif</column>
            <column name="static_width">10</column>
            <column name="static_height">14</column>
            <column name="action_pic">NULL</column>
            <column name="action_width">NULL</column>
            <column name="action_height">NULL</column>
            <column name="last_update">2016-01-28 05:20:43</column>
            <column name="first_update">NULL</column>
            <column name="luck_hits">NULL</column>
            <column name="is_seasonal">0</column>
        </table>
    </database>
</pma_xml_export>
```

- [ ] **Step 3: Write the failing parser test**

Create `crates/amanuensis-core/src/data/bestiary_import.rs`:

```rust
use std::collections::HashMap;

use quick_xml::events::Event;
use quick_xml::reader::Reader;

use crate::data::bestiary::BestiaryEntry;
use crate::error::{AmanuensisError, Result};

/// Parse a phpMyAdmin XML dump (clnet_bestiary.creatures) into BestiaryEntry rows.
pub fn parse_bestiary_xml(xml: &[u8]) -> Result<Vec<BestiaryEntry>> {
    let mut reader = Reader::from_reader(xml);
    reader.config_mut().trim_text(true);

    let mut buf = Vec::new();
    let mut entries: Vec<BestiaryEntry> = Vec::new();
    let mut current: Option<HashMap<String, String>> = None;
    let mut current_column: Option<String> = None;
    let mut current_text = String::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => match e.name().as_ref() {
                b"table" => {
                    if attr_matches(&e, b"name", b"creatures") {
                        current = Some(HashMap::new());
                    }
                }
                b"column" => {
                    if current.is_some() {
                        if let Some(name) = attr_value(&e, b"name") {
                            current_column = Some(name);
                            current_text.clear();
                        }
                    }
                }
                _ => {}
            },
            Ok(Event::Text(t)) => {
                if current_column.is_some() {
                    let txt = t.unescape().map_err(xml_err)?;
                    current_text.push_str(&txt);
                }
            }
            Ok(Event::End(e)) => match e.name().as_ref() {
                b"column" => {
                    if let (Some(map), Some(col)) = (current.as_mut(), current_column.take()) {
                        map.insert(col, std::mem::take(&mut current_text));
                    }
                }
                b"table" => {
                    if let Some(map) = current.take() {
                        entries.push(entry_from_columns(map)?);
                    }
                }
                _ => {}
            },
            Ok(Event::Eof) => break,
            Err(e) => return Err(xml_err(e)),
            _ => {}
        }
        buf.clear();
    }

    Ok(entries)
}

fn xml_err<E: std::fmt::Display>(e: E) -> AmanuensisError {
    AmanuensisError::Parse(format!("bestiary XML: {}", e))
}

fn attr_matches(e: &quick_xml::events::BytesStart, key: &[u8], value: &[u8]) -> bool {
    e.attributes().flatten().any(|a| a.key.as_ref() == key && a.value.as_ref() == value)
}

fn attr_value(e: &quick_xml::events::BytesStart, key: &[u8]) -> Option<String> {
    e.attributes().flatten().find(|a| a.key.as_ref() == key).map(|a| {
        String::from_utf8_lossy(&a.value).into_owned()
    })
}

fn entry_from_columns(cols: HashMap<String, String>) -> Result<BestiaryEntry> {
    let name = cols.get("name").cloned().ok_or_else(|| {
        AmanuensisError::Parse("bestiary entry missing 'name' column".to_string())
    })?;

    Ok(BestiaryEntry {
        name,
        family: opt_str(&cols, "family"),
        location: opt_str(&cols, "location"),
        information: opt_str(&cols, "information"),
        exp_taxidermy: opt_int(&cols, "exp_taxidermy").unwrap_or(0),
        rarity: opt_str(&cols, "rarity"),
        worth: opt_int(&cols, "worth"),
        worth_range: opt_str(&cols, "worth_range"),
        frames_per_swing: opt_float(&cols, "framesperswing"),
        difficulty: opt_str(&cols, "difficulty"),
        attack: opt_int(&cols, "attack"),
        defense: opt_int(&cols, "defense"),
        damage: opt_int(&cols, "damage"),
        health: opt_int(&cols, "health"),
        attack_measured: opt_bool(&cols, "attack_ismeasured"),
        defense_measured: opt_bool(&cols, "defense_ismeasured"),
        damage_measured: opt_bool(&cols, "damage_ismeasured"),
        health_measured: opt_bool(&cols, "health_ismeasured"),
        luck_hits: opt_int(&cols, "luck_hits"),
        is_seasonal: opt_bool(&cols, "is_seasonal"),
        first_update: opt_str(&cols, "first_update"),
        last_update: opt_str(&cols, "last_update"),
        static_pic: opt_str(&cols, "static_pic"),
        static_width: opt_int(&cols, "static_width"),
        static_height: opt_int(&cols, "static_height"),
        action_pic: opt_str(&cols, "action_pic"),
        action_width: opt_int(&cols, "action_width"),
        action_height: opt_int(&cols, "action_height"),
    })
}

fn opt_str(cols: &HashMap<String, String>, key: &str) -> Option<String> {
    match cols.get(key) {
        Some(v) if v != "NULL" && !v.is_empty() => Some(v.clone()),
        _ => None,
    }
}

fn opt_int(cols: &HashMap<String, String>, key: &str) -> Option<i32> {
    cols.get(key).and_then(|v| if v == "NULL" { None } else { v.parse().ok() })
}

fn opt_float(cols: &HashMap<String, String>, key: &str) -> Option<f64> {
    cols.get(key).and_then(|v| if v == "NULL" { None } else { v.parse().ok() })
}

fn opt_bool(cols: &HashMap<String, String>, key: &str) -> bool {
    matches!(cols.get(key).map(String::as_str), Some("1"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINIMAL_FIXTURE: &[u8] = include_bytes!("../../tests/fixtures/bestiary_minimal.xml");

    #[test]
    fn parses_two_entries() {
        let entries = parse_bestiary_xml(MINIMAL_FIXTURE).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].name, "Rat");
        assert_eq!(entries[1].name, "Venomous Leech");
    }

    #[test]
    fn populates_rat_fields() {
        let entries = parse_bestiary_xml(MINIMAL_FIXTURE).unwrap();
        let rat = &entries[0];
        assert_eq!(rat.exp_taxidermy, 2);
        assert_eq!(rat.family.as_deref(), Some("Vermine"));
        assert_eq!(rat.rarity.as_deref(), Some("Common"));
        assert_eq!(rat.attack, Some(65));
        assert_eq!(rat.health, Some(2));
        assert!(rat.attack_measured);
        assert!(rat.health_measured);
        assert!(!rat.is_seasonal);
    }

    #[test]
    fn null_string_becomes_none() {
        let entries = parse_bestiary_xml(MINIMAL_FIXTURE).unwrap();
        let leech = &entries[1];
        assert_eq!(leech.first_update, None);          // first_update was "NULL"
        assert_eq!(leech.luck_hits, None);              // luck_hits was "NULL"
        assert_eq!(leech.action_pic, None);             // action_pic was "NULL"
        assert_eq!(leech.action_width, None);
    }

    #[test]
    fn ismeasured_null_becomes_false() {
        let entries = parse_bestiary_xml(MINIMAL_FIXTURE).unwrap();
        let leech = &entries[1];
        assert!(!leech.attack_measured);                // "NULL" → false
        assert!(!leech.defense_measured);
        assert!(!leech.damage_measured);
        assert!(!leech.health_measured);
    }

    #[test]
    fn html_entities_decoded() {
        let entries = parse_bestiary_xml(MINIMAL_FIXTURE).unwrap();
        let leech = &entries[1];
        // location is "Dal&#039;Nzoth Waters" in the fixture; should decode to "Dal'Nzoth Waters"
        assert_eq!(leech.location.as_deref(), Some("Dal'Nzoth Waters"));
    }
}
```

Add the module declaration to `crates/amanuensis-core/src/data/mod.rs`:

```rust
pub mod bestiary;
pub mod bestiary_import;
pub mod creatures;
pub mod trainer_checkpoints;
pub mod trainers;

pub use bestiary::{BestiaryEntry, BestiaryAlias, InlineEntry, EntrySource, BestiaryFile};
pub use bestiary_import::parse_bestiary_xml;
pub use creatures::CreatureDb;
pub use trainer_checkpoints::lookup_checkpoint_message;
pub use trainers::{TrainerDb, TrainerMeta};
```

- [ ] **Step 4: Run tests to verify they fail (file does not yet exist or test logic incomplete)**

Run: `cargo test -p amanuensis-core --lib data::bestiary_import`
Expected: 5 PASS once Step 3's code is in place. If any FAIL, fix and re-run.

- [ ] **Step 5: Commit**

```bash
git add crates/amanuensis-core/Cargo.toml \
        crates/amanuensis-core/Cargo.lock \
        crates/amanuensis-core/src/data/bestiary_import.rs \
        crates/amanuensis-core/src/data/mod.rs \
        crates/amanuensis-core/tests/fixtures/bestiary_minimal.xml
# Cargo.lock lives at the workspace root; substitute accordingly if necessary.
git add Cargo.lock
git commit -m "Add quick-xml bestiary importer with fixture-based tests"
```

---

## Task 3: Rewrite CreatureDb to load from JSON pair with alias resolution

**Files:**
- Modify: `crates/amanuensis-core/src/data/creatures.rs`

- [ ] **Step 1: Write the new CreatureDb with full test coverage**

Replace `crates/amanuensis-core/src/data/creatures.rs` entirely with:

```rust
use std::collections::HashMap;

use crate::data::bestiary::{BestiaryAlias, BestiaryEntry, BestiaryFile, EntrySource, InlineEntry};
use crate::error::{AmanuensisError, Result};

/// In-memory bestiary lookup, loaded from bestiary.json + bestiary_aliases.json.
#[derive(Debug)]
pub struct CreatureDb {
    version: String,
    by_name: HashMap<String, BestiaryEntry>,
    aliases: HashMap<String, ResolvedAlias>,
}

#[derive(Debug, Clone)]
enum ResolvedAlias {
    Pointer(String),
    Inline(BestiaryEntry),
}

impl CreatureDb {
    /// Load from JSON bytes (bestiary file + aliases file).
    /// Validates that every `resolves_to` target exists, and that no `log_name` is duplicated.
    pub fn from_json_bytes(bestiary: &[u8], aliases: &[u8]) -> Result<Self> {
        let file: BestiaryFile = serde_json::from_slice(bestiary)?;
        let mut by_name = HashMap::with_capacity(file.entries.len());
        for entry in file.entries {
            by_name.insert(entry.name.clone(), entry);
        }

        let raw_aliases: Vec<BestiaryAlias> = serde_json::from_slice(aliases)?;
        let mut alias_map: HashMap<String, ResolvedAlias> = HashMap::with_capacity(raw_aliases.len());
        for alias in raw_aliases {
            let (log_name, resolved) = match alias {
                BestiaryAlias::Resolves { log_name, resolves_to } => {
                    if !by_name.contains_key(&resolves_to) {
                        return Err(AmanuensisError::Data(format!(
                            "Alias '{}' points to missing bestiary entry '{}'",
                            log_name, resolves_to
                        )));
                    }
                    (log_name, ResolvedAlias::Pointer(resolves_to))
                }
                BestiaryAlias::Inline { log_name, inline } => {
                    let synthetic = synthesize_entry(&log_name, &inline);
                    (log_name, ResolvedAlias::Inline(synthetic))
                }
            };
            if alias_map.contains_key(&log_name) {
                return Err(AmanuensisError::Data(format!(
                    "Duplicate alias log_name: '{}'",
                    log_name
                )));
            }
            alias_map.insert(log_name, resolved);
        }

        log::info!(
            "Loaded bestiary version {} ({} entries, {} aliases)",
            file.version,
            by_name.len(),
            alias_map.len()
        );
        Ok(Self {
            version: file.version,
            by_name,
            aliases: alias_map,
        })
    }

    /// Load the bundled bestiary + aliases compiled into the binary.
    pub fn bundled() -> Result<Self> {
        Self::from_json_bytes(
            include_bytes!("../../data/bestiary.json"),
            include_bytes!("../../data/bestiary_aliases.json"),
        )
    }

    /// Look up a creature's exp_taxidermy value by log name.
    /// Lookup order: aliases → bestiary direct → strip "the " and retry.
    pub fn get_value(&self, log_name: &str) -> Option<i32> {
        self.get_entry(log_name).map(|e| e.exp_taxidermy)
    }

    /// Look up a creature's full BestiaryEntry by log name. Same lookup order as `get_value`.
    pub fn get_entry(&self, log_name: &str) -> Option<&BestiaryEntry> {
        self.get_entry_with_source(log_name).map(|(e, _)| e)
    }

    /// Look up an entry and report where it came from.
    pub fn get_entry_with_source(&self, log_name: &str) -> Option<(&BestiaryEntry, EntrySource)> {
        if let Some(hit) = self.lookup(log_name) {
            return Some(hit);
        }
        // "the X" fallback: strip and retry.
        if let Some(bare) = log_name.strip_prefix("the ") {
            return self.lookup(bare);
        }
        None
    }

    fn lookup(&self, log_name: &str) -> Option<(&BestiaryEntry, EntrySource)> {
        if let Some(alias) = self.aliases.get(log_name) {
            return Some(match alias {
                ResolvedAlias::Pointer(target) => (
                    self.by_name
                        .get(target)
                        .expect("validated at load time"),
                    EntrySource::Alias,
                ),
                ResolvedAlias::Inline(entry) => (entry, EntrySource::InlineAlias),
            });
        }
        self.by_name
            .get(log_name)
            .map(|e| (e, EntrySource::Bestiary))
    }

    pub fn entries(&self) -> impl Iterator<Item = &BestiaryEntry> {
        self.by_name.values()
    }

    pub fn bestiary_version(&self) -> &str {
        &self.version
    }

    pub fn len(&self) -> usize {
        self.by_name.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_name.is_empty()
    }
}

fn synthesize_entry(log_name: &str, inline: &InlineEntry) -> BestiaryEntry {
    BestiaryEntry {
        name: log_name.to_string(),
        family: inline.family.clone(),
        location: inline.location.clone(),
        information: inline.information.clone(),
        exp_taxidermy: inline.exp_taxidermy,
        rarity: inline.rarity.clone(),
        worth: None,
        worth_range: None,
        frames_per_swing: None,
        difficulty: None,
        attack: None,
        defense: None,
        damage: None,
        health: None,
        attack_measured: false,
        defense_measured: false,
        damage_measured: false,
        health_measured: false,
        luck_hits: None,
        is_seasonal: false,
        first_update: None,
        last_update: None,
        static_pic: None,
        static_width: None,
        static_height: None,
        action_pic: None,
        action_width: None,
        action_height: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_db(entries: &[(&str, i32)], aliases: &str) -> CreatureDb {
        let file = BestiaryFile {
            version: "20260101".into(),
            entries: entries
                .iter()
                .map(|(name, val)| BestiaryEntry {
                    name: (*name).into(),
                    exp_taxidermy: *val,
                    family: None,
                    location: None,
                    information: None,
                    rarity: None,
                    worth: None,
                    worth_range: None,
                    frames_per_swing: None,
                    difficulty: None,
                    attack: None,
                    defense: None,
                    damage: None,
                    health: None,
                    attack_measured: false,
                    defense_measured: false,
                    damage_measured: false,
                    health_measured: false,
                    luck_hits: None,
                    is_seasonal: false,
                    first_update: None,
                    last_update: None,
                    static_pic: None,
                    static_width: None,
                    static_height: None,
                    action_pic: None,
                    action_width: None,
                    action_height: None,
                })
                .collect(),
        };
        let bestiary_json = serde_json::to_vec(&file).unwrap();
        CreatureDb::from_json_bytes(&bestiary_json, aliases.as_bytes()).unwrap()
    }

    #[test]
    fn alias_resolves_to_bestiary_entry() {
        let db = make_db(
            &[("Bar", 999)],
            r#"[{"log_name": "Foo", "resolves_to": "Bar"}]"#,
        );
        assert_eq!(db.get_value("Foo"), Some(999));
        let (entry, source) = db.get_entry_with_source("Foo").unwrap();
        assert_eq!(entry.name, "Bar");
        assert_eq!(source, EntrySource::Alias);
    }

    #[test]
    fn inline_alias_returns_synthetic_entry() {
        let db = make_db(
            &[],
            r#"[{"log_name": "Old Critter", "inline": {"exp_taxidermy": 500, "family": "Legacy"}}]"#,
        );
        assert_eq!(db.get_value("Old Critter"), Some(500));
        let (entry, source) = db.get_entry_with_source("Old Critter").unwrap();
        assert_eq!(entry.exp_taxidermy, 500);
        assert_eq!(entry.family.as_deref(), Some("Legacy"));
        assert_eq!(source, EntrySource::InlineAlias);
    }

    #[test]
    fn alias_dangling_resolves_to_errors() {
        let file = BestiaryFile {
            version: "20260101".into(),
            entries: vec![],
        };
        let bestiary_json = serde_json::to_vec(&file).unwrap();
        let result = CreatureDb::from_json_bytes(
            &bestiary_json,
            br#"[{"log_name": "Foo", "resolves_to": "Missing"}]"#,
        );
        assert!(matches!(result, Err(AmanuensisError::Data(_))));
    }

    #[test]
    fn duplicate_log_name_errors() {
        let file = BestiaryFile {
            version: "20260101".into(),
            entries: vec![BestiaryEntry {
                name: "Bar".into(),
                exp_taxidermy: 1,
                family: None, location: None, information: None,
                rarity: None, worth: None, worth_range: None,
                frames_per_swing: None, difficulty: None,
                attack: None, defense: None, damage: None, health: None,
                attack_measured: false, defense_measured: false,
                damage_measured: false, health_measured: false,
                luck_hits: None, is_seasonal: false,
                first_update: None, last_update: None,
                static_pic: None, static_width: None, static_height: None,
                action_pic: None, action_width: None, action_height: None,
            }],
        };
        let bestiary_json = serde_json::to_vec(&file).unwrap();
        let result = CreatureDb::from_json_bytes(
            &bestiary_json,
            br#"[
                {"log_name": "Foo", "resolves_to": "Bar"},
                {"log_name": "Foo", "resolves_to": "Bar"}
            ]"#,
        );
        assert!(matches!(result, Err(AmanuensisError::Data(_))));
    }

    #[test]
    fn alias_overrides_bestiary_direct_hit() {
        // "Bar" exists in bestiary; an alias also names "Bar" with a different target.
        let db = make_db(
            &[("Bar", 100), ("Other", 999)],
            r#"[{"log_name": "Bar", "resolves_to": "Other"}]"#,
        );
        assert_eq!(db.get_value("Bar"), Some(999));
    }

    #[test]
    fn the_prefix_falls_back_to_bestiary() {
        let db = make_db(&[("Dragon", 500)], r#"[]"#);
        assert_eq!(db.get_value("the Dragon"), Some(500));
    }

    #[test]
    fn the_prefix_falls_back_through_alias() {
        let db = make_db(
            &[("Real Dragon", 500)],
            r#"[{"log_name": "Dragon", "resolves_to": "Real Dragon"}]"#,
        );
        // Direct "the Dragon" miss → strip → "Dragon" alias hit → "Real Dragon".
        assert_eq!(db.get_value("the Dragon"), Some(500));
    }

    #[test]
    fn unknown_creature_returns_none() {
        let db = make_db(&[("Rat", 2)], "[]");
        assert_eq!(db.get_value("Nonexistent"), None);
        assert!(db.get_entry("Nonexistent").is_none());
    }

    #[test]
    fn bestiary_version_exposed() {
        let db = make_db(&[("Rat", 2)], "[]");
        assert_eq!(db.bestiary_version(), "20260101");
    }
}
```

- [ ] **Step 2: Run new unit tests**

Run: `cargo test -p amanuensis-core --lib data::creatures`
Expected: 9 PASS

- [ ] **Step 3: Confirm the workspace still builds (no consumers broken yet)**

Run: `cargo build -p amanuensis-core`
Expected: clean build. (`include_bytes!` for `bestiary.json` will fail because those files don't exist yet — see Step 4.)

If the build fails because of the `include_bytes!` calls in `bundled()`, that's expected — `bundled()` won't work until Task 6 writes the files. Tests in this task don't call `bundled()`.

- [ ] **Step 4: Stub the bundled JSON files so `bundled()` compiles**

Create `crates/amanuensis-core/data/bestiary.json`:

```json
{"version":"00000000","entries":[]}
```

Create `crates/amanuensis-core/data/bestiary_aliases.json`:

```json
[]
```

Run: `cargo build -p amanuensis-core`
Expected: clean build.

- [ ] **Step 5: Commit**

```bash
git add crates/amanuensis-core/src/data/creatures.rs \
        crates/amanuensis-core/data/bestiary.json \
        crates/amanuensis-core/data/bestiary_aliases.json
git commit -m "Rewrite CreatureDb to load bestiary.json + aliases overlay"
```

---

## Task 4: Generate the real bestiary.json from the upstream XML

**Files:**
- Modify: `crates/amanuensis-core/data/bestiary.json` (overwrite with real data)
- Source: `~/Downloads/bestiary_20260520_fullexport.xml`

This task uses a one-off Rust harness — the CLI subcommand isn't built yet (Task 7), so we use a small example/test to do the conversion in-tree.

- [ ] **Step 1: Add an ignored test that generates bestiary.json**

Append to `crates/amanuensis-core/src/data/bestiary_import.rs`:

```rust
#[cfg(test)]
mod regen {
    use super::*;
    use crate::data::bestiary::BestiaryFile;
    use std::fs;
    use std::path::PathBuf;

    /// One-off generator: read the upstream XML dump, write bestiary.json.
    /// Run with: cargo test -p amanuensis-core regen::generate_bestiary_json -- --ignored --nocapture
    /// Env: BESTIARY_XML=/path/to/bestiary_YYYYMMDD_fullexport.xml
    #[test]
    #[ignore]
    fn generate_bestiary_json() {
        let xml_path = std::env::var("BESTIARY_XML")
            .expect("set BESTIARY_XML=/path/to/bestiary_YYYYMMDD_fullexport.xml");
        let xml = fs::read(&xml_path).expect("read XML");
        let mut entries = parse_bestiary_xml(&xml).expect("parse XML");
        entries.sort_by(|a, b| a.name.cmp(&b.name));

        let version = version_from_filename(&xml_path);
        let file = BestiaryFile { version, entries };
        let out = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("data/bestiary.json");
        let json = serde_json::to_string_pretty(&file).unwrap();
        fs::write(&out, json).expect("write bestiary.json");
        eprintln!(
            "Wrote {} ({} entries, version {})",
            out.display(),
            file.entries.len(),
            file.version
        );
    }

    fn version_from_filename(path: &str) -> String {
        // expects bestiary_YYYYMMDD_fullexport.xml
        std::path::Path::new(path)
            .file_name()
            .and_then(|s| s.to_str())
            .and_then(|s| s.strip_prefix("bestiary_"))
            .and_then(|s| s.split('_').next())
            .unwrap_or("00000000")
            .to_string()
    }
}
```

- [ ] **Step 2: Run the generator against the real XML**

Run:
```
BESTIARY_XML=$HOME/Downloads/bestiary_20260520_fullexport.xml \
  cargo test -p amanuensis-core regen::generate_bestiary_json -- --ignored --nocapture
```
Expected output: `Wrote .../data/bestiary.json (969 entries, version 20260520)`

- [ ] **Step 3: Sanity-check the generated file**

```bash
python3 -c "import json; d=json.load(open('crates/amanuensis-core/data/bestiary.json')); print(d['version'], len(d['entries']), d['entries'][0]['name'])"
```
Expected: `20260520 969 (Super) Crookbeak Kestrel`

- [ ] **Step 4: Commit (alias file still empty — that's the next task)**

```bash
git add crates/amanuensis-core/src/data/bestiary_import.rs crates/amanuensis-core/data/bestiary.json
git commit -m "Generate bestiary.json from 2026-05-20 clnet_bestiary dump (969 entries)"
```

---

## Task 5: Seed the alias file from the CSV→XML diff

**Files:**
- Modify: `crates/amanuensis-core/data/bestiary_aliases.json`

This is the editorial step. The current CSV has entries that the new XML lacks; some of these are real log-name remappings (Ramandu boss/clone), others may be stale.

- [ ] **Step 1: Compute the diff using shell**

```bash
# Names currently in CSV
cut -d, -f1 crates/amanuensis-core/data/creatures.csv | sort -u > /tmp/csv_names.txt
# Names now in the new bestiary.json
python3 -c "import json; [print(e['name']) for e in json.load(open('crates/amanuensis-core/data/bestiary.json'))['entries']]" | sort -u > /tmp/xml_names.txt
echo "--- in CSV not in XML ---"
comm -23 /tmp/csv_names.txt /tmp/xml_names.txt
echo "--- in XML not in CSV ---"
comm -13 /tmp/csv_names.txt /tmp/xml_names.txt | wc -l
```

Capture the "in CSV not in XML" list — these are the candidates for alias entries.

- [ ] **Step 2: Present the candidate alias list to the user**

For each name in the diff:
- If the name maps to a known bestiary entry (e.g., `Ramandu` → `the Ramandu`), draft a `Resolves` alias.
- If the name has no bestiary equivalent and the CSV value should be preserved, draft an `Inline` alias.
- If the name is clearly stale (no longer appears in logs), propose dropping it.

Write the proposed list to a scratch document and confirm with the user before writing the JSON file.

**Known starting entries (the cases identified in the spec):**

```json
[
  {"log_name": "the Ramandu", "resolves_to": "the Ramandu (boss)"},
  {"log_name": "Ramandu", "resolves_to": "the Ramandu"}
]
```

Any additional entries depend on the diff output. Common patterns to look for:
- Article-stripped variants: log says `Foo`, bestiary has `the Foo`.
- Color/variant shortcuts: log says `Mushroom`, bestiary has `Mushroom (Brown)` and `Mushroom (Purple)`.
- Old CSV values for creatures retired from the bestiary → `Inline` to keep recognition.

- [ ] **Step 3: Write the curated alias file**

Replace `crates/amanuensis-core/data/bestiary_aliases.json` with the user-approved JSON array. Example shape:

```json
[
  {"log_name": "the Ramandu", "resolves_to": "the Ramandu (boss)"},
  {"log_name": "Ramandu", "resolves_to": "the Ramandu"},
  {"log_name": "Mushroom", "resolves_to": "Mushroom (Brown)"}
]
```

- [ ] **Step 4: Verify CreatureDb loads cleanly with the new files**

Run: `cargo test -p amanuensis-core --lib data -- --nocapture`
Expected: all PASS, including any test that calls `CreatureDb::bundled()`.

Also exercise `bundled()` explicitly:
```rust
// scratch in a unit test or just `cargo test`:
let db = CreatureDb::bundled().unwrap();
println!("version: {}, entries: {}, aliases: {}", db.bestiary_version(), db.len(), db.entries().count());
```

- [ ] **Step 5: Add a `bundled()` smoke test**

Append to `crates/amanuensis-core/src/data/creatures.rs` tests module:

```rust
#[test]
fn bundled_loads_and_has_expected_creatures() {
    let db = CreatureDb::bundled().unwrap();
    assert!(db.len() > 950, "expected > 950 entries, got {}", db.len());
    // editorial-preservation: Ramandu boss/clone behavior survives the migration.
    assert_eq!(db.get_value("the Ramandu"), Some(2620));
    assert_eq!(db.get_value("Ramandu"), Some(666));
    // simple direct hits still work.
    assert_eq!(db.get_value("Rat"), Some(2));
    assert_eq!(db.get_value("Tesla"), Some(70));
}
```

Run: `cargo test -p amanuensis-core --lib data::creatures::tests::bundled_loads`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/amanuensis-core/data/bestiary_aliases.json crates/amanuensis-core/src/data/creatures.rs
git commit -m "Seed bestiary aliases from CSV→XML diff; add bundled() smoke test"
```

---

## Task 6: Delete creatures.csv and remove `csv` dep if unused

**Files:**
- Delete: `crates/amanuensis-core/data/creatures.csv`
- Possibly modify: `crates/amanuensis-core/Cargo.toml` (drop `csv` if no other consumer)

- [ ] **Step 1: Verify nothing else loads `creatures.csv`**

Run: `grep -r 'creatures.csv' crates/`
Expected: only references inside `CLAUDE.md` or comments — no live code.

- [ ] **Step 2: Check whether the `csv` crate has any remaining consumer**

Run: `grep -r 'use csv\|extern crate csv\|csv::' crates/amanuensis-core/src crates/amanuensis-cli/src crates/amanuensis-gui/src-tauri/src 2>/dev/null`
Expected: no hits. If there are hits, leave the dep alone.

- [ ] **Step 3: Delete the CSV**

```bash
git rm crates/amanuensis-core/data/creatures.csv
```

- [ ] **Step 4: If safe, drop the `csv` dep**

Edit `crates/amanuensis-core/Cargo.toml`, remove the line `csv = "1"` if Step 2 found zero consumers. Otherwise skip.

Run: `cargo build --workspace`
Expected: clean build.

- [ ] **Step 5: Run the full test suite (non-ignored)**

Run: `cargo test --workspace`
Expected: all PASS. The pre-existing `test_load_bundled_creatures`, `test_known_creatures`, `test_the_ramandu_boss_value`, `test_the_prefix_fallback`, `test_unknown_creature` are replaced by the rewritten tests in Tasks 3 and 5 — none should remain referencing the old CSV.

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "Drop creatures.csv; bestiary.json is now the single source of truth"
```

---

## Task 7: Add `amanuensis update-bestiary` CLI subcommand

**Files:**
- Modify: `crates/amanuensis-cli/src/main.rs`

- [ ] **Step 1: Add the subcommand to the clap enum**

Edit `crates/amanuensis-cli/src/main.rs`. Inside the `Commands` enum (after `UseItemHelp`), add:

```rust
/// Regenerate bestiary.json from an upstream phpMyAdmin XML dump
#[command(name = "update-bestiary")]
UpdateBestiary {
    /// Path to the bestiary XML dump (e.g. ~/Downloads/bestiary_YYYYMMDD_fullexport.xml)
    xml_path: PathBuf,
    /// Optional override for the aliases file (default: crates/amanuensis-core/data/bestiary_aliases.json)
    #[arg(long)]
    aliases: Option<PathBuf>,
    /// Output path for the generated bestiary.json (default: crates/amanuensis-core/data/bestiary.json)
    #[arg(long)]
    output: Option<PathBuf>,
    /// Parse and validate without writing
    #[arg(long)]
    dry_run: bool,
},
/// Print a creature's full bestiary record
Bestiary {
    /// Creature name as it appears in logs (e.g. "Rat", "the Ramandu")
    name: String,
},
```

- [ ] **Step 2: Route the new commands in `run()`**

Edit the `match cli.command` block in `run()`. Add before `Commands::GuiDbPath`:

```rust
Commands::UpdateBestiary { xml_path, aliases, output, dry_run } => {
    cmd_update_bestiary(&xml_path, aliases.as_deref(), output.as_deref(), dry_run)
}
Commands::Bestiary { name } => cmd_bestiary(&name),
```

Both commands don't need a DB. Add an early-return in `run()` (next to `UseItemHelp`):

```rust
if let Commands::UpdateBestiary { xml_path, aliases, output, dry_run } = &cli.command {
    return cmd_update_bestiary(xml_path, aliases.as_deref(), output.as_deref(), *dry_run);
}
if let Commands::Bestiary { name } = &cli.command {
    return cmd_bestiary(name);
}
```

- [ ] **Step 3: Implement `cmd_update_bestiary`**

Append to `crates/amanuensis-cli/src/main.rs`:

```rust
fn cmd_update_bestiary(
    xml_path: &Path,
    aliases_override: Option<&Path>,
    output_override: Option<&Path>,
    dry_run: bool,
) -> amanuensis_core::Result<()> {
    use amanuensis_core::data::{parse_bestiary_xml, BestiaryFile, CreatureDb};

    let xml = std::fs::read(xml_path)?;
    let mut entries = parse_bestiary_xml(&xml)?;
    entries.sort_by(|a, b| a.name.cmp(&b.name));
    let version = version_from_filename(xml_path);

    let alias_path = aliases_override.map(PathBuf::from).unwrap_or_else(default_alias_path);
    let alias_bytes = std::fs::read(&alias_path)?;

    // Validate aliases against the new entries by round-tripping through CreatureDb.
    let file = BestiaryFile { version: version.clone(), entries };
    let bestiary_bytes = serde_json::to_vec(&file)?;
    let db = CreatureDb::from_json_bytes(&bestiary_bytes, &alias_bytes)?;

    println!("Bestiary: {} entries (version {})", db.len(), db.bestiary_version());
    println!("Aliases:  {} loaded from {}", count_aliases(&alias_bytes)?, alias_path.display());

    if dry_run {
        println!("(dry-run; not writing)");
        return Ok(());
    }

    let out_path = output_override.map(PathBuf::from).unwrap_or_else(default_bestiary_path);
    let pretty = serde_json::to_string_pretty(&file)?;
    std::fs::write(&out_path, pretty)?;
    println!("Wrote {}", out_path.display());
    println!(
        "Bestiary updated to version {}. Existing databases should run 'amanuensis scan --force <folder>' to refresh kill values.",
        version
    );
    Ok(())
}

fn cmd_bestiary(name: &str) -> amanuensis_core::Result<()> {
    use amanuensis_core::data::{CreatureDb, EntrySource};
    let db = CreatureDb::bundled()?;
    match db.get_entry_with_source(name) {
        None => {
            eprintln!("No bestiary entry for '{}'", name);
            std::process::exit(1);
        }
        Some((entry, source)) => {
            let src = match source {
                EntrySource::Bestiary => "bestiary",
                EntrySource::Alias => "alias → bestiary",
                EntrySource::InlineAlias => "inline alias",
            };
            println!("Name:           {}", entry.name);
            println!("Source:         {} (bestiary v{})", src, db.bestiary_version());
            if let Some(f) = &entry.family { println!("Family:         {}", f); }
            if let Some(r) = &entry.rarity { println!("Rarity:         {}", r); }
            println!("Exp/taxidermy:  {}", entry.exp_taxidermy);
            if let Some(l) = &entry.location { println!("Location:       {}", l); }
            if let Some(i) = &entry.information { println!("Information:    {}", i); }
            if let Some(d) = &entry.difficulty { println!("Difficulty:     {}", d); }
            let stats = [
                ("Attack", entry.attack, entry.attack_measured),
                ("Defense", entry.defense, entry.defense_measured),
                ("Damage", entry.damage, entry.damage_measured),
                ("Health", entry.health, entry.health_measured),
            ];
            for (label, val, measured) in stats {
                if let Some(v) = val {
                    let suffix = if measured { " (measured)" } else { "" };
                    println!("{:14}  {}{}", format!("{}:", label), v, suffix);
                }
            }
            if let Some(l) = entry.luck_hits { println!("Luck hits:      {}%", l); }
            if let Some(fps) = entry.frames_per_swing { println!("Frames/swing:   {}", fps); }
            if let Some(w) = &entry.worth_range { println!("Worth range:    {}", w); }
            if entry.is_seasonal { println!("Seasonal:       yes"); }
        }
    }
    Ok(())
}

fn version_from_filename(path: &Path) -> String {
    path.file_name()
        .and_then(|s| s.to_str())
        .and_then(|s| s.strip_prefix("bestiary_"))
        .and_then(|s| s.split('_').next())
        .unwrap_or("00000000")
        .to_string()
}

fn default_alias_path() -> PathBuf {
    PathBuf::from("crates/amanuensis-core/data/bestiary_aliases.json")
}

fn default_bestiary_path() -> PathBuf {
    PathBuf::from("crates/amanuensis-core/data/bestiary.json")
}

fn count_aliases(bytes: &[u8]) -> amanuensis_core::Result<usize> {
    let parsed: serde_json::Value = serde_json::from_slice(bytes)?;
    Ok(parsed.as_array().map(|a| a.len()).unwrap_or(0))
}
```

- [ ] **Step 4: Build and smoke-test the new commands**

Run: `cargo build -p amanuensis-cli`
Expected: clean.

Smoke-test:
```bash
cargo run -p amanuensis-cli -- bestiary "Rat"
cargo run -p amanuensis-cli -- bestiary "the Ramandu"
cargo run -p amanuensis-cli -- update-bestiary $HOME/Downloads/bestiary_20260520_fullexport.xml --dry-run
```

Expected:
- `Rat` prints with family Vermine, source `bestiary`.
- `the Ramandu` prints with source `alias → bestiary` (or `bestiary` if the alias isn't installed), exp_taxidermy 2620.
- The dry-run prints the counts and `(dry-run; not writing)`.

- [ ] **Step 5: Commit**

```bash
git add crates/amanuensis-cli/src/main.rs
git commit -m "Add 'update-bestiary' and 'bestiary' CLI commands"
```

---

## Task 8: Re-run real-data comparison tests and update docs

**Files:**
- Modify: `CLAUDE.md`

- [ ] **Step 1: Re-run the real-data comparison suite**

Run: `cargo test -p amanuensis-core --test real_data_comparison -- --ignored`
Expected: all PASS. If any creature_value-derived metric (e.g. `coin_level`, `highest_kill`, `nemesis`) regresses for Ruuk, Olga, Squib, Tu Whawha, or Tane, investigate before continuing — the alias file is the most likely culprit.

If a regression appears, walk the diff:
```bash
cargo run -p amanuensis-cli -- summary Ruuk
# compare against prior known-good output captured before the migration
```

- [ ] **Step 2: Update CLAUDE.md**

Edit `CLAUDE.md`. In the "Updated Data Sources" section, replace the bullet about `creatures.csv` with:

```markdown
- **Bestiary**: `amanuensis update-bestiary <xml-path>` regenerates `crates/amanuensis-core/data/bestiary.json` from the upstream `clnet_bestiary` XML dump (e.g. `bestiary_YYYYMMDD_fullexport.xml`). The companion `bestiary_aliases.json` holds hand-curated log-name → bestiary-name mappings (e.g. `the Ramandu` → `the Ramandu (boss)`). After updating, existing databases should run `amanuensis scan --force <folder>` to refresh stored `creature_value` rows from the new bestiary.
```

- [ ] **Step 3: Commit**

```bash
git add CLAUDE.md
git commit -m "Document update-bestiary CLI and rescan policy in CLAUDE.md"
```

---

## Self-review (post-write)

Spec coverage check:
- BestiaryEntry / alias types — Task 1 ✓
- XML importer — Task 2 ✓
- CreatureDb rewrite + alias resolution + `the X` fallback + version stamp + EntrySource — Task 3 ✓
- Generate bestiary.json from real XML — Task 4 ✓
- Hand-curated alias seeding from diff — Task 5 ✓
- Delete creatures.csv — Task 6 ✓
- `update-bestiary` CLI (with `--aliases`, `--dry-run`) — Task 7 ✓
- `bestiary` CLI with source indicator — Task 7 ✓
- Rescan reminder printed by CLI — Task 7 ✓
- Real-data regression check — Task 8 ✓
- CLAUDE.md updated — Task 8 ✓

Placeholder scan: none.

Type consistency: `CreatureDb` API matches across tasks. `EntrySource` is named consistently. `BestiaryFile`/`BestiaryEntry`/`BestiaryAlias`/`InlineEntry` referenced consistently.

CLI integration tests from the spec (`test_update_bestiary_dry_run`, `test_bestiary_command_prints_record`, etc.) are deferred — the existing CLI has no integration-test harness, and the smoke-tests in Task 7 Step 4 provide equivalent confidence for the immediate change. If we later add an `assert_cmd` test harness for the CLI, those tests can be added then.
