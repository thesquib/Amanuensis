use std::collections::HashMap;

use crate::error::{Result, ScribiusError};

/// In-memory creature name → value lookup, loaded from creatures.csv.
#[derive(Debug)]
pub struct CreatureDb {
    creatures: HashMap<String, i32>,
}

impl CreatureDb {
    /// Load from CSV bytes (name,value per line, no header).
    pub fn from_csv_bytes(data: &[u8]) -> Result<Self> {
        let mut creatures = HashMap::new();
        let mut rdr = csv::ReaderBuilder::new()
            .has_headers(false)
            .from_reader(data);

        for result in rdr.records() {
            let record = result?;
            if record.len() < 2 {
                continue;
            }
            let name = record[0].trim().to_string();
            let value: i32 = record[1]
                .trim()
                .parse()
                .map_err(|e| ScribiusError::Data(format!("Bad creature value for '{}': {}", name, e)))?;
            if !name.is_empty() {
                creatures.insert(name, value);
            }
        }

        log::info!("Loaded {} creatures", creatures.len());
        Ok(Self { creatures })
    }

    /// Load from the bundled creatures.csv (compiled into the binary).
    pub fn bundled() -> Result<Self> {
        Self::from_csv_bytes(include_bytes!("../../data/creatures.csv"))
    }

    /// Look up a creature's value by name.
    pub fn get_value(&self, name: &str) -> Option<i32> {
        self.creatures.get(name).copied()
    }

    /// Get all creature names.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.creatures.keys().map(|s| s.as_str())
    }

    pub fn len(&self) -> usize {
        self.creatures.len()
    }

    pub fn is_empty(&self) -> bool {
        self.creatures.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_bundled_creatures() {
        let db = CreatureDb::bundled().unwrap();
        assert!(db.len() > 800, "Expected 800+ creatures, got {}", db.len());
    }

    #[test]
    fn test_known_creatures() {
        let db = CreatureDb::bundled().unwrap();
        assert_eq!(db.get_value("Rat"), Some(2));
        assert_eq!(db.get_value("Leech"), Some(5));
        assert_eq!(db.get_value("Tesla"), Some(35));
        assert_eq!(db.get_value("Barracuda"), Some(250));
    }

    #[test]
    fn test_unknown_creature() {
        let db = CreatureDb::bundled().unwrap();
        assert_eq!(db.get_value("Nonexistent Creature XYZ"), None);
    }

    #[test]
    fn test_from_csv_bytes() {
        let csv = b"Goblin,10\nDragon,500\n";
        let db = CreatureDb::from_csv_bytes(csv).unwrap();
        assert_eq!(db.len(), 2);
        assert_eq!(db.get_value("Goblin"), Some(10));
        assert_eq!(db.get_value("Dragon"), Some(500));
    }
}
