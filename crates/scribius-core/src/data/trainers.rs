use std::collections::HashMap;

use crate::error::Result;

/// In-memory trainer message → trainer name lookup, loaded from trainers.json.
/// The JSON format is: { "¥message text": { "trainer": "Name", "profession": "Fighter" }, ... }
#[derive(Debug)]
pub struct TrainerDb {
    /// Map from message text (with ¥ prefix stripped) to trainer name
    trainers: HashMap<String, String>,
    /// Map from trainer name to profession string
    professions: HashMap<String, String>,
}

impl TrainerDb {
    /// Load from JSON bytes. The JSON has ¥-prefixed keys mapping to {"trainer": "Name", "profession": "..."}.
    /// We strip the ¥ prefix from keys for easier matching.
    pub fn from_json_bytes(data: &[u8]) -> Result<Self> {
        let raw: HashMap<String, serde_json::Value> = serde_json::from_slice(data)?;
        let mut trainers = HashMap::new();
        let mut professions = HashMap::new();

        for (key, value) in raw {
            if let Some(trainer_name) = value.get("trainer").and_then(|v| v.as_str()) {
                // Strip ¥ prefix if present for matching
                let message = key.strip_prefix('¥').unwrap_or(&key).to_string();
                trainers.insert(message, trainer_name.to_string());

                // Store profession mapping if present
                if let Some(profession) = value.get("profession").and_then(|v| v.as_str()) {
                    professions.insert(trainer_name.to_string(), profession.to_string());
                }
            }
        }

        log::info!("Loaded {} trainer messages, {} profession mappings", trainers.len(), professions.len());
        Ok(Self { trainers, professions })
    }

    /// Load from the bundled trainers.json (compiled into the binary).
    pub fn bundled() -> Result<Self> {
        Self::from_json_bytes(include_bytes!("../../data/trainers.json"))
    }

    /// Look up a trainer name by message text (without ¥ prefix).
    pub fn get_trainer(&self, message: &str) -> Option<&str> {
        self.trainers.get(message).map(|s| s.as_str())
    }

    /// Look up a profession by trainer name.
    pub fn get_profession(&self, trainer_name: &str) -> Option<&str> {
        self.professions.get(trainer_name).map(|s| s.as_str())
    }

    pub fn len(&self) -> usize {
        self.trainers.len()
    }

    pub fn is_empty(&self) -> bool {
        self.trainers.is_empty()
    }

    /// Return all unique trainer names with their profession (if known).
    /// Used for the "Show Zero Trainers" toggle in the GUI.
    pub fn all_trainers_with_professions(&self) -> Vec<(String, Option<String>)> {
        let mut seen = std::collections::HashSet::new();
        let mut result = Vec::new();
        for trainer_name in self.trainers.values() {
            if seen.insert(trainer_name.clone()) {
                let profession = self.professions.get(trainer_name).cloned();
                result.push((trainer_name.clone(), profession));
            }
        }
        result.sort_by(|a, b| a.0.cmp(&b.0));
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
        assert_eq!(db.get_profession("Aktur"), Some("Bloodmage"));

        // Champion
        assert_eq!(db.get_profession("Channel Master"), Some("Champion"));
        assert_eq!(db.get_profession("Corsetta"), Some("Champion"));
        assert_eq!(db.get_profession("Ittum"), Some("Champion"));
        assert_eq!(db.get_profession("Toomeria"), Some("Champion"));
        assert_eq!(db.get_profession("Vala Loack"), Some("Champion"));

        // Trainers without profession (language, craft, etc.)
        assert_eq!(db.get_profession("ParTroon"), None);
        assert_eq!(db.get_profession("Zeucros"), None);
    }
}
