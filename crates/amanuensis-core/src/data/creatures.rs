use std::collections::HashMap;

use crate::data::bestiary::{BestiaryAlias, BestiaryEntry, BestiaryFile, EntrySource, InlineEntry};
use crate::error::{AmanuensisError, Result};

/// In-memory bestiary lookup, loaded from bestiary.json + bestiary_aliases.json.
#[derive(Debug)]
pub struct CreatureDb {
    version: String,
    by_name: HashMap<String, BestiaryEntry>,
    aliases: HashMap<String, ResolvedAlias>,
    /// Lowercased family name -> canonical (most-common) casing. Collapses casing
    /// duplicates like `EXTINCT`/`Extinct` to a single label.
    family_canonical: HashMap<String, String>,
}

#[derive(Debug, Clone)]
enum ResolvedAlias {
    Pointer(String),
    Inline(Box<BestiaryEntry>),
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
                    (log_name, ResolvedAlias::Inline(Box::new(synthetic)))
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

        let family_canonical = build_family_canonical(by_name.values());

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
            family_canonical,
        })
    }

    /// Canonical (most-common-cased) form of a family name. Folds casing
    /// duplicates (e.g. `EXTINCT` -> `Extinct`). Unknown values pass through
    /// unchanged.
    pub fn canonical_family<'a>(&'a self, raw: &'a str) -> &'a str {
        self.family_canonical
            .get(&raw.to_ascii_lowercase())
            .map(String::as_str)
            .unwrap_or(raw)
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
                ResolvedAlias::Inline(entry) => (entry.as_ref(), EntrySource::InlineAlias),
            });
        }
        self.by_name
            .get(log_name)
            .map(|e| (e, EntrySource::Bestiary))
    }

    /// Iterate over all bestiary entries. Inline-alias synthetic entries are NOT included —
    /// they exist only to satisfy lookups for log names with no bestiary equivalent.
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

/// Build a lowercased-family -> canonical-casing map. For each case-insensitive
/// group, the canonical casing is the most frequent variant; ties are broken by
/// the lexicographically smallest variant for determinism.
fn build_family_canonical<'a>(
    entries: impl Iterator<Item = &'a BestiaryEntry>,
) -> HashMap<String, String> {
    // lowercase -> (exact casing -> count)
    let mut groups: HashMap<String, HashMap<String, usize>> = HashMap::new();
    for entry in entries {
        if let Some(family) = entry.family.as_deref() {
            *groups
                .entry(family.to_ascii_lowercase())
                .or_default()
                .entry(family.to_string())
                .or_insert(0) += 1;
        }
    }
    groups
        .into_iter()
        .map(|(lower, variants)| {
            let canonical = variants
                .into_iter()
                .max_by(|(a_name, a_count), (b_name, b_count)| {
                    a_count
                        .cmp(b_count)
                        .then_with(|| b_name.cmp(a_name)) // smaller name wins ties
                })
                .map(|(name, _)| name)
                .unwrap_or_else(|| lower.clone());
            (lower, canonical)
        })
        .collect()
}

fn synthesize_entry(log_name: &str, inline: &InlineEntry) -> BestiaryEntry {
    BestiaryEntry {
        name: log_name.to_string(),
        family: inline.family.clone(),
        location: inline.location.clone(),
        information: inline.information.clone(),
        exp_taxidermy: inline.exp_taxidermy,
        rarity: inline.rarity.clone(),
        ..BestiaryEntry::default()
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
                    ..BestiaryEntry::default()
                })
                .collect(),
        };
        let bestiary_json = serde_json::to_vec(&file).unwrap();
        CreatureDb::from_json_bytes(&bestiary_json, aliases.as_bytes()).unwrap()
    }

    fn make_db_with_families(families: &[&str]) -> CreatureDb {
        let file = BestiaryFile {
            version: "20260101".into(),
            entries: families
                .iter()
                .enumerate()
                .map(|(i, fam)| BestiaryEntry {
                    name: format!("c{i}"),
                    family: Some((*fam).into()),
                    exp_taxidermy: 1,
                    ..BestiaryEntry::default()
                })
                .collect(),
        };
        let bestiary_json = serde_json::to_vec(&file).unwrap();
        CreatureDb::from_json_bytes(&bestiary_json, b"[]").unwrap()
    }

    #[test]
    fn canonical_family_folds_case_to_most_common() {
        // "Extinct" (2) vs "EXTINCT" (1): the majority casing wins.
        let db = make_db_with_families(&["Extinct", "Extinct", "EXTINCT", "Orga"]);
        assert_eq!(db.canonical_family("EXTINCT"), "Extinct");
        assert_eq!(db.canonical_family("Extinct"), "Extinct");
        // Distinct families are not merged.
        assert_eq!(db.canonical_family("Orga"), "Orga");
    }

    #[test]
    fn canonical_family_passthrough_for_unseen() {
        let db = make_db_with_families(&["Feline"]);
        assert_eq!(db.canonical_family("Nonexistent"), "Nonexistent");
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
                ..BestiaryEntry::default()
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

    #[test]
    fn bundled_get_entry_returns_full_record() {
        let db = CreatureDb::bundled().unwrap();
        let tesla = db.get_entry("Tesla").expect("Tesla should be in the bestiary");
        assert_eq!(tesla.family.as_deref(), Some("Annelida"));
        assert_eq!(tesla.rarity.as_deref(), Some("Common"));
        assert_eq!(tesla.exp_taxidermy, 70);
        assert_eq!(tesla.attack, Some(115));
    }

    #[test]
    fn bundled_loads_and_has_expected_creatures() {
        let db = CreatureDb::bundled().unwrap();
        assert!(db.len() > 950, "expected > 950 entries, got {}", db.len());
        // Editorial preservation: Ramandu boss/clone behavior survives the migration.
        assert_eq!(db.get_value("the Ramandu"), Some(2620));
        assert_eq!(db.get_value("Ramandu"), Some(666));
        // Direct hits.
        assert_eq!(db.get_value("Rat"), Some(2));
        assert_eq!(db.get_value("Tesla"), Some(70));
        // Alias-resolved hits.
        assert_eq!(db.get_value("Mushroom"), Some(5));
        assert_eq!(db.get_value("Seasylvan"), Some(865));
        // Inline alias.
        assert_eq!(db.get_value("Fumehorn Colossus"), Some(1510));
    }
}
