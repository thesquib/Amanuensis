use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trainer {
    pub id: Option<i64>,
    pub character_id: i64,
    pub trainer_name: String,
    pub ranks: i64,
    pub modified_ranks: i64,
    pub date_of_last_rank: Option<String>,
    pub apply_learning_ranks: i64,
    pub apply_learning_unknown_count: i64,
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
            apply_learning_ranks: 0,
            apply_learning_unknown_count: 0,
        }
    }
}
