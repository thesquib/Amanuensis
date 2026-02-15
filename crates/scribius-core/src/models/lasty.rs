use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LastyType {
    Befriend,
    Morph,
    Ability,
}

impl std::fmt::Display for LastyType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LastyType::Befriend => write!(f, "Befriend"),
            LastyType::Morph => write!(f, "Morph"),
            LastyType::Ability => write!(f, "Ability"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Lasty {
    pub id: Option<i64>,
    pub character_id: i64,
    pub creature_name: String,
    pub lasty_type: String,
    pub finished: bool,
    pub message_count: i64,
}

impl Lasty {
    pub fn new(character_id: i64, creature_name: String, lasty_type: String) -> Self {
        Self {
            id: None,
            character_id,
            creature_name,
            lasty_type,
            finished: false,
            message_count: 0,
        }
    }
}
