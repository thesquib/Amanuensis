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
