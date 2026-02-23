use std::collections::HashMap;

use serde::Serialize;

use crate::error::Result;

/// Metadata about a trainer, including effective rank multiplier and combo info.
#[derive(Debug, Clone, Serialize)]
pub struct TrainerMeta {
    pub name: String,
    pub profession: Option<String>,
    pub multiplier: f64,
    pub is_combo: bool,
    pub combo_components: Vec<String>,
}

/// In-memory trainer message -> trainer name lookup, loaded from trainers.json.
/// The JSON format is: { "¥message text": { "trainer": "Name", "profession": "Fighter", ... }, ... }
#[derive(Debug)]
pub struct TrainerDb {
    /// Map from message text (with ¥ prefix stripped) to trainer name
    trainers: HashMap<String, String>,
    /// Map from trainer name to profession string
    professions: HashMap<String, String>,
    /// Map from trainer name to effective rank multiplier (only non-1.0 values stored)
    multipliers: HashMap<String, f64>,
    /// Map from trainer name to combo component trainer names
    combo_components: HashMap<String, Vec<String>>,
}

impl TrainerDb {
    /// Load from JSON bytes. The JSON has ¥-prefixed keys mapping to {"trainer": "Name", "profession": "...", ...}.
    /// We strip the ¥ prefix from keys for easier matching.
    pub fn from_json_bytes(data: &[u8]) -> Result<Self> {
        let raw: HashMap<String, serde_json::Value> = serde_json::from_slice(data)?;
        let mut trainers = HashMap::new();
        let mut professions = HashMap::new();
        let mut multipliers = HashMap::new();
        let mut combo_components = HashMap::new();

        for (key, value) in raw {
            if let Some(trainer_name) = value.get("trainer").and_then(|v| v.as_str()) {
                // Strip ¥ prefix if present for matching, and trim whitespace
                let message = key.strip_prefix('¥').unwrap_or(&key).trim().to_string();
                trainers.insert(message, trainer_name.to_string());

                // Store profession mapping if present
                if let Some(profession) = value.get("profession").and_then(|v| v.as_str()) {
                    professions.insert(trainer_name.to_string(), profession.to_string());
                }

                // Store effective rank multiplier if present and not 1.0
                if let Some(mult) = value
                    .get("effective_rank_multiplier")
                    .and_then(|v| v.as_f64())
                {
                    multipliers.insert(trainer_name.to_string(), mult);
                }

                // Store combo components if present
                if let Some(components) = value.get("combo_components").and_then(|v| v.as_array())
                {
                    let names: Vec<String> = components
                        .iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect();
                    if !names.is_empty() {
                        combo_components.insert(trainer_name.to_string(), names);
                    }
                }
            }
        }

        log::info!(
            "Loaded {} trainer messages, {} profession mappings, {} multipliers, {} combos",
            trainers.len(),
            professions.len(),
            multipliers.len(),
            combo_components.len()
        );
        Ok(Self {
            trainers,
            professions,
            multipliers,
            combo_components,
        })
    }

    /// Load from the bundled trainers.json (compiled into the binary).
    pub fn bundled() -> Result<Self> {
        Self::from_json_bytes(include_bytes!("../../data/trainers.json"))
    }

    /// Look up a trainer name by message text (without ¥ prefix).
    /// Tries exact match first, then with/without trailing period, for robustness.
    pub fn get_trainer(&self, message: &str) -> Option<&str> {
        let trimmed = message.trim();
        if let Some(name) = self.trainers.get(trimmed) {
            return Some(name.as_str());
        }
        // Try adding/removing trailing period for edge cases
        if let Some(without_period) = trimmed.strip_suffix('.') {
            if let Some(name) = self.trainers.get(without_period) {
                return Some(name.as_str());
            }
        } else {
            let with_period = format!("{}.", trimmed);
            if let Some(name) = self.trainers.get(&with_period) {
                return Some(name.as_str());
            }
        }
        None
    }

    /// Look up a profession by trainer name.
    pub fn get_profession(&self, trainer_name: &str) -> Option<&str> {
        self.professions.get(trainer_name).map(|s| s.as_str())
    }

    /// Get the effective rank multiplier for a trainer (defaults to 1.0).
    pub fn get_multiplier(&self, name: &str) -> f64 {
        self.multipliers.get(name).copied().unwrap_or(1.0)
    }

    /// Check if a trainer is a combo trainer (trains multiple stats).
    pub fn is_combo(&self, name: &str) -> bool {
        self.combo_components.contains_key(name)
    }

    /// Get the combo component trainer names for a combo trainer.
    pub fn get_combo_components(&self, name: &str) -> &[String] {
        self.combo_components
            .get(name)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    pub fn len(&self) -> usize {
        self.trainers.len()
    }

    pub fn is_empty(&self) -> bool {
        self.trainers.is_empty()
    }

    /// Return all unique trainer names with full metadata.
    /// Used for the GUI trainer catalog (zero-trainers toggle, effective ranks, etc.).
    pub fn all_trainer_metadata(&self) -> Vec<TrainerMeta> {
        let mut seen = std::collections::HashSet::new();
        let mut result = Vec::new();
        for trainer_name in self.trainers.values() {
            if seen.insert(trainer_name.clone()) {
                let components = self
                    .combo_components
                    .get(trainer_name)
                    .cloned()
                    .unwrap_or_default();
                result.push(TrainerMeta {
                    name: trainer_name.clone(),
                    profession: self.professions.get(trainer_name).cloned(),
                    multiplier: self.get_multiplier(trainer_name),
                    is_combo: !components.is_empty(),
                    combo_components: components,
                });
            }
        }
        result.sort_by(|a, b| a.name.cmp(&b.name));
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_bundled_trainers() {
        let db = TrainerDb::bundled().unwrap();
        assert!(db.len() > 90, "Expected 90+ trainer messages, got {}", db.len());
    }

    #[test]
    fn test_known_trainers() {
        let db = TrainerDb::bundled().unwrap();
        assert_eq!(db.get_trainer("Your combat ability improves."), Some("Bangus Anmash"));
        assert_eq!(db.get_trainer("You notice your balance recovering more quickly."), Some("Regia"));
        assert_eq!(db.get_trainer("You notice yourself healing others faster."), Some("Faustus"));
        assert_eq!(db.get_trainer("You seem to fight more effectively now."), Some("Evus"));
    }

    #[test]
    fn test_unknown_message() {
        let db = TrainerDb::bundled().unwrap();
        assert_eq!(db.get_trainer("This is not a trainer message."), None);
    }

    #[test]
    fn test_seel_no_yen_prefix() {
        // "Things appear a bit more clearly, now." has no ¥ prefix in the plist
        let db = TrainerDb::bundled().unwrap();
        assert_eq!(db.get_trainer("Things appear a bit more clearly, now."), Some("Seel"));
    }

    #[test]
    fn test_from_json_bytes() {
        let json = r#"{"¥You feel tougher.": {"trainer": "Farly Buff", "profession": "Ranger"}}"#;
        let db = TrainerDb::from_json_bytes(json.as_bytes()).unwrap();
        assert_eq!(db.get_trainer("You feel tougher."), Some("Farly Buff"));
        assert_eq!(db.get_profession("Farly Buff"), Some("Ranger"));
    }

    #[test]
    fn test_profession_mappings() {
        let db = TrainerDb::bundled().unwrap();
        // Fighter trainers
        assert_eq!(db.get_profession("Evus"), Some("Fighter"));
        assert_eq!(db.get_profession("Atkus"), Some("Fighter"));
        assert_eq!(db.get_profession("Darkus"), Some("Fighter"));
        assert_eq!(db.get_profession("Detha"), Some("Fighter"));
        assert_eq!(db.get_profession("Knox"), Some("Fighter"));
        assert_eq!(db.get_profession("Regia"), Some("Fighter"));
        assert_eq!(db.get_profession("Swengus"), Some("Fighter"));
        assert_eq!(db.get_profession("Aktur"), Some("Fighter"));

        // Healer trainers
        assert_eq!(db.get_profession("Eva"), Some("Healer"));
        assert_eq!(db.get_profession("Faustus"), Some("Healer"));
        assert_eq!(db.get_profession("Horus"), Some("Healer"));
        assert_eq!(db.get_profession("Proximus"), Some("Healer"));
        assert_eq!(db.get_profession("Respia"), Some("Healer"));
        assert_eq!(db.get_profession("Sespus"), Some("Healer"));
        assert_eq!(db.get_profession("Sprite"), Some("Healer"));

        // Mystic trainers
        assert_eq!(db.get_profession("Sespos"), Some("Mystic"));
        assert_eq!(db.get_profession("Respos"), Some("Mystic"));
        assert_eq!(db.get_profession("Quantos"), Some("Mystic"));
        assert_eq!(db.get_profession("Pontifen"), Some("Mystic"));
        assert_eq!(db.get_profession("Radia"), Some("Mystic"));
        assert_eq!(db.get_profession("Skryss"), Some("Mystic"));
        assert_eq!(db.get_profession("Alaenos"), Some("Mystic"));
        assert_eq!(db.get_profession("Histuvia"), Some("Mystic"));
        assert_eq!(db.get_profession("Hardio"), Some("Mystic"));
        assert_eq!(db.get_profession("Bouste"), Some("Mystic"));
        assert_eq!(db.get_profession("Seel"), Some("Mystic"));

        // Ranger trainers
        assert_eq!(db.get_profession("Bangus Anmash"), Some("Ranger"));
        assert_eq!(db.get_profession("Farly Buff"), Some("Ranger"));
        assert_eq!(db.get_profession("Respin Verminebane"), Some("Ranger"));
        assert_eq!(db.get_profession("Ranger 2nd Slot"), Some("Ranger"));
        assert_eq!(db.get_profession("Spleisha'Sul"), Some("Ranger"));

        // Bloodmage
        assert_eq!(db.get_profession("Posuhm"), Some("Bloodmage"));
        assert_eq!(db.get_profession("Disabla"), Some("Bloodmage"));
        assert_eq!(db.get_profession("Cryptus"), Some("Bloodmage"));
        assert_eq!(db.get_profession("Dantus"), Some("Bloodmage"));

        // Champion
        assert_eq!(db.get_profession("Forvyola"), Some("Champion"));
        assert_eq!(db.get_profession("Channel Master"), Some("Champion"));
        assert_eq!(db.get_profession("Corsetta"), Some("Champion"));
        assert_eq!(db.get_profession("Ittum"), Some("Champion"));
        assert_eq!(db.get_profession("Toomeria"), Some("Champion"));
        assert_eq!(db.get_profession("Vala Loack"), Some("Champion"));

        // General trainers now categorized
        assert_eq!(db.get_profession("Histia"), Some("Fighter"));
        assert_eq!(db.get_profession("Balthus"), Some("Fighter"));
        assert_eq!(db.get_profession("Darktur"), Some("Fighter"));
        assert_eq!(db.get_profession("Angilsa"), Some("Fighter"));
        assert_eq!(db.get_profession("Atkia"), Some("Fighter"));
        assert_eq!(db.get_profession("Master Bodrus"), Some("Fighter"));
        assert_eq!(db.get_profession("Rodnus"), Some("Healer"));
        assert_eq!(db.get_profession("Master Spirtus"), Some("Healer"));
        assert_eq!(db.get_profession("Master Mentus"), Some("Mystic"));
        assert_eq!(db.get_profession("Gossamer"), Some("Ranger"));

        // Language trainers
        assert_eq!(db.get_profession("ParTroon"), Some("Language"));
        assert_eq!(db.get_profession("Sylvan"), Some("Language"));

        // Arts trainers
        assert_eq!(db.get_profession("Dark Blue Paint"), Some("Arts"));

        // Trades trainers
        assert_eq!(db.get_profession("Zeucros"), Some("Trades"));
        assert_eq!(db.get_profession("Forgus"), Some("Trades"));
        assert_eq!(db.get_profession("Sartorio"), Some("Trades"));
    }

    #[test]
    fn test_multiplier_evus() {
        let db = TrainerDb::bundled().unwrap();
        let mult = db.get_multiplier("Evus");
        assert!((mult - 1.1436).abs() < 0.0001, "Evus multiplier should be 1.1436, got {mult}");
    }

    #[test]
    fn test_multiplier_default() {
        let db = TrainerDb::bundled().unwrap();
        assert!((db.get_multiplier("Histia") - 1.0).abs() < f64::EPSILON);
        assert!((db.get_multiplier("Regia") - 1.0).abs() < f64::EPSILON);
        assert!((db.get_multiplier("NonExistent") - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_combo_detection() {
        let db = TrainerDb::bundled().unwrap();
        assert!(db.is_combo("Evus"));
        assert!(db.is_combo("Atkus"));
        assert!(db.is_combo("Darkus"));
        assert!(!db.is_combo("Histia"));
        assert!(!db.is_combo("Knox"));
    }

    #[test]
    fn test_combo_components() {
        let db = TrainerDb::bundled().unwrap();
        let evus = db.get_combo_components("Evus");
        assert_eq!(evus.len(), 6);
        assert!(evus.contains(&"Aktur".to_string()));
        assert!(evus.contains(&"Histia".to_string()));
        assert!(evus.contains(&"Darktur".to_string()));
    }

    #[test]
    fn test_all_trainer_metadata() {
        let db = TrainerDb::bundled().unwrap();
        let meta = db.all_trainer_metadata();
        assert!(!meta.is_empty());

        // Check Evus entry
        let evus = meta.iter().find(|m| m.name == "Evus").unwrap();
        assert_eq!(evus.profession.as_deref(), Some("Fighter"));
        assert!((evus.multiplier - 1.1436).abs() < 0.0001);
        assert!(evus.is_combo);
        assert_eq!(evus.combo_components.len(), 6);

        // Check a base trainer (now categorized as Fighter)
        let histia = meta.iter().find(|m| m.name == "Histia").unwrap();
        assert_eq!(histia.profession.as_deref(), Some("Fighter"));
        assert!((histia.multiplier - 1.0).abs() < f64::EPSILON);
        assert!(!histia.is_combo);
        assert!(histia.combo_components.is_empty());
    }

    #[test]
    fn test_from_json_with_multiplier_and_combo() {
        let json = r#"{
            "¥Test msg.": {
                "trainer": "TestCombo",
                "profession": "Fighter",
                "effective_rank_multiplier": 1.5,
                "combo_components": ["A", "B"]
            }
        }"#;
        let db = TrainerDb::from_json_bytes(json.as_bytes()).unwrap();
        assert!((db.get_multiplier("TestCombo") - 1.5).abs() < f64::EPSILON);
        assert!(db.is_combo("TestCombo"));
        assert_eq!(db.get_combo_components("TestCombo"), &["A", "B"]);
    }
}
