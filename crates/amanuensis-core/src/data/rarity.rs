//! Canonical rarity bucketing.
//!
//! The upstream bestiary stores `rarity` as free text (65 distinct values across
//! 969 entries). This module collapses any raw string into one of seven canonical
//! buckets using a "lowest common denominator" rule: a creature that is common
//! somewhere is treated as common.

/// Canonical rarity bucket. Variants are declared lowest (most common) to highest
/// (rarest), so deriving `Ord` lets `.min()` pick the lowest-common-denominator.
/// `Unknown` sorts last.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Rarity {
    Common,
    Medium,
    Rare,
    Unique,
    Exotic,
    GmOnly,
    Unknown,
}

impl Rarity {
    /// Display label for tables, chips, and the detail modal.
    pub fn as_label(self) -> &'static str {
        match self {
            Rarity::Common => "Common",
            Rarity::Medium => "Medium",
            Rarity::Rare => "Rare",
            Rarity::Unique => "Unique",
            Rarity::Exotic => "Exotic",
            Rarity::GmOnly => "GM Only",
            Rarity::Unknown => "Unknown",
        }
    }
}

/// Collapse a raw bestiary rarity string into a canonical bucket.
///
/// Scans for rarity keywords and returns the *lowest* (most common) bucket found.
/// `extinct` with no other keyword maps to `Unique`; anything else unresolvable
/// (including `None`) maps to `Unknown`.
pub fn canonical_rarity(raw: Option<&str>) -> Rarity {
    let Some(raw) = raw else {
        return Rarity::Unknown;
    };
    // Drop "uncommon" first so it does not false-match the "common" substring.
    let text = raw.to_ascii_lowercase().replace("uncommon", "");

    let mut found: Option<Rarity> = None;
    let mut consider = |r: Rarity| {
        found = Some(found.map_or(r, |cur| cur.min(r)));
    };
    if text.contains("common") {
        consider(Rarity::Common);
    }
    if text.contains("medium") {
        consider(Rarity::Medium);
    }
    if text.contains("rare") {
        consider(Rarity::Rare);
    }
    if text.contains("unique") || text.contains("boss") {
        consider(Rarity::Unique);
    }
    if text.contains("exotic") {
        consider(Rarity::Exotic);
    }
    if text.contains("gm only") {
        consider(Rarity::GmOnly);
    }

    found.unwrap_or_else(|| {
        if text.contains("extinct") {
            Rarity::Unique
        } else {
            Rarity::Unknown
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordering_is_low_to_high() {
        assert!(Rarity::Common < Rarity::Medium);
        assert!(Rarity::Medium < Rarity::Rare);
        assert!(Rarity::Rare < Rarity::Unique);
        assert!(Rarity::Unique < Rarity::Exotic);
        assert!(Rarity::Exotic < Rarity::GmOnly);
        assert!(Rarity::GmOnly < Rarity::Unknown);
    }

    #[test]
    fn labels_render_for_display() {
        assert_eq!(Rarity::Common.as_label(), "Common");
        assert_eq!(Rarity::Medium.as_label(), "Medium");
        assert_eq!(Rarity::Rare.as_label(), "Rare");
        assert_eq!(Rarity::Unique.as_label(), "Unique");
        assert_eq!(Rarity::Exotic.as_label(), "Exotic");
        assert_eq!(Rarity::GmOnly.as_label(), "GM Only");
        assert_eq!(Rarity::Unknown.as_label(), "Unknown");
    }

    #[test]
    fn maps_clean_values() {
        assert_eq!(canonical_rarity(Some("Common")), Rarity::Common);
        assert_eq!(canonical_rarity(Some("Medium")), Rarity::Medium);
        assert_eq!(canonical_rarity(Some("Rare")), Rarity::Rare);
        assert_eq!(canonical_rarity(Some("Exotic")), Rarity::Exotic);
        assert_eq!(canonical_rarity(Some("GM Only")), Rarity::GmOnly);
    }

    #[test]
    fn strips_trailing_punctuation_noise() {
        assert_eq!(canonical_rarity(Some("Common.")), Rarity::Common);
        assert_eq!(canonical_rarity(Some("Common. (Obviously.)")), Rarity::Common);
        assert_eq!(canonical_rarity(Some("Medium.")), Rarity::Medium);
        assert_eq!(canonical_rarity(Some("Rare.")), Rarity::Rare);
        assert_eq!(canonical_rarity(Some("Medium-Rare?")), Rarity::Medium);
    }

    #[test]
    fn lowest_common_denominator_wins() {
        assert_eq!(
            canonical_rarity(Some("Rare (Melabrion's), Common (Wendecka Breeding Grounds)")),
            Rarity::Common
        );
        assert_eq!(canonical_rarity(Some("Medium-Rare")), Rarity::Medium);
        assert_eq!(canonical_rarity(Some("Medium-Common")), Rarity::Common);
        assert_eq!(
            canonical_rarity(Some("Exotic (Melabrion's), Common (Midpass spring)")),
            Rarity::Common
        );
        assert_eq!(
            canonical_rarity(Some("Unique (Boss), Ultra Common (Clones)")),
            Rarity::Common
        );
    }

    #[test]
    fn uncommon_does_not_count_as_common() {
        // "Medium (or 'uncommon')" must resolve to Medium, not Common.
        assert_eq!(canonical_rarity(Some("Medium (or 'uncommon')")), Rarity::Medium);
    }

    #[test]
    fn boss_and_unique_map_to_unique() {
        assert_eq!(canonical_rarity(Some("Unique (Boss)")), Rarity::Unique);
        assert_eq!(canonical_rarity(Some("Boss")), Rarity::Unique);
        assert_eq!(canonical_rarity(Some("Unique (Mini-Boss)")), Rarity::Unique);
        assert_eq!(canonical_rarity(Some("Unique")), Rarity::Unique);
    }

    #[test]
    fn exotic_outranks_gm_only_in_lcd() {
        assert_eq!(canonical_rarity(Some("Exotic (GM only?)")), Rarity::Exotic);
        assert_eq!(canonical_rarity(Some("Exotic or GM only")), Rarity::Exotic);
    }

    #[test]
    fn extinct_maps_to_unique() {
        assert_eq!(canonical_rarity(Some("Extinct")), Rarity::Unique);
        assert_eq!(canonical_rarity(Some("Extinct.")), Rarity::Unique);
    }

    #[test]
    fn unresolvable_maps_to_unknown() {
        assert_eq!(canonical_rarity(None), Rarity::Unknown);
        assert_eq!(canonical_rarity(Some("Once per year!")), Rarity::Unknown);
        assert_eq!(canonical_rarity(Some("Not Applicable")), Rarity::Unknown);
        assert_eq!(canonical_rarity(Some("")), Rarity::Unknown);
    }

    #[test]
    fn every_bundled_entry_resolves_to_a_known_bucket() {
        use crate::data::CreatureDb;

        let known = [
            "Common", "Medium", "Rare", "Unique", "Exotic", "GM Only", "Unknown",
        ];
        let db = CreatureDb::bundled().unwrap();
        let mut common = 0usize;
        for entry in db.entries() {
            let label = canonical_rarity(entry.rarity.as_deref()).as_label();
            assert!(known.contains(&label), "unexpected bucket {label:?}");
            if label == "Common" {
                common += 1;
            }
        }
        // Common is by far the largest bucket in the real bestiary.
        assert!(common > 300, "expected Common to dominate, got {common}");
    }
}
