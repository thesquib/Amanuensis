use std::collections::HashMap;

use crate::error::Result;

/// In-memory trainer message → trainer name lookup, loaded from trainers.json.
/// The JSON format is: { "¥message text": { "trainer": "Name" }, ... }
#[derive(Debug)]
pub struct TrainerDb {
    /// Map from message text (with ¥ prefix stripped) to trainer name
    trainers: HashMap<String, String>,
}

impl TrainerDb {
    /// Load from JSON bytes. The JSON has ¥-prefixed keys mapping to {"trainer": "Name"}.
    /// We strip the ¥ prefix from keys for easier matching.
    pub fn from_json_bytes(data: &[u8]) -> Result<Self> {
        let raw: HashMap<String, serde_json::Value> = serde_json::from_slice(data)?;
        let mut trainers = HashMap::new();

        for (key, value) in raw {
            if let Some(trainer_name) = value.get("trainer").and_then(|v| v.as_str()) {
                // Strip ¥ prefix if present for matching
                let message = key.strip_prefix('¥').unwrap_or(&key).to_string();
                trainers.insert(message, trainer_name.to_string());
            }
        }

        log::info!("Loaded {} trainer messages", trainers.len());
        Ok(Self { trainers })
    }

    /// Load from the bundled trainers.json (compiled into the binary).
    pub fn bundled() -> Result<Self> {
        Self::from_json_bytes(include_bytes!("../../data/trainers.json"))
    }

    /// Look up a trainer name by message text (without ¥ prefix).
    pub fn get_trainer(&self, message: &str) -> Option<&str> {
        self.trainers.get(message).map(|s| s.as_str())
    }

    pub fn len(&self) -> usize {
        self.trainers.len()
    }

    pub fn is_empty(&self) -> bool {
        self.trainers.is_empty()
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
        let json = r#"{"¥You feel tougher.": {"trainer": "Farly Buff"}}"#;
        let db = TrainerDb::from_json_bytes(json.as_bytes()).unwrap();
        assert_eq!(db.get_trainer("You feel tougher."), Some("Farly Buff"));
    }
}
