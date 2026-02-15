use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trainer {
    pub id: Option<i64>,
    pub character_id: i64,
    pub trainer_name: String,
    pub ranks: i64,
    pub modified_ranks: i64,
    pub date_of_last_rank: Option<String>,
}

impl Trainer {
    pub fn new(character_id: i64, trainer_name: String) -> Self {
        Self {
            id: None,
            character_id,
            trainer_name,
            ranks: 0,
            modified_ranks: 0,
            date_of_last_rank: None,
        }
    }
}
