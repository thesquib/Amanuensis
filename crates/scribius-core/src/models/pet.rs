use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pet {
    pub id: Option<i64>,
    pub character_id: i64,
    pub pet_name: String,
    pub creature_name: String,
}

impl Pet {
    pub fn new(character_id: i64, pet_name: String, creature_name: String) -> Self {
        Self {
            id: None,
            character_id,
            pet_name,
            creature_name,
        }
    }
}
