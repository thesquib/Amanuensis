use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LastyType {
    Befriend,
    Morph,
    Movements,
}

impl LastyType {
    pub fn as_str(&self) -> &'static str {
        match self {
            LastyType::Befriend => "Befriend",
            LastyType::Morph => "Morph",
            LastyType::Movements => "Movements",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "Befriend" => Some(LastyType::Befriend),
            "Morph" => Some(LastyType::Morph),
            "Movements" => Some(LastyType::Movements),
            _ => None,
        }
    }
}

impl std::fmt::Display for LastyType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
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
    pub first_seen_date: Option<String>,
    pub last_seen_date: Option<String>,
    pub completed_date: Option<String>,
    pub abandoned_date: Option<String>,
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
            first_seen_date: None,
            last_seen_date: None,
            completed_date: None,
            abandoned_date: None,
        }
    }
}
