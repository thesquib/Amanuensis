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
    pub rank_mode: String,
    pub override_date: Option<String>,
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
            rank_mode: "modifier".to_string(),
            override_date: None,
        }
    }

    /// Compute effective ranks based on the rank mode.
    ///
    /// - `modifier`: ranks + modified_ranks + apply_learning_ranks (default additive)
    /// - `override`: modified_ranks only (manual value replaces logs)
    /// - `override_until_date`: modified_ranks + ranks + apply_learning_ranks
    ///   (ranks/apply_learning_ranks only contain post-cutoff counts from parser)
    pub fn effective_ranks(&self) -> i64 {
        match self.rank_mode.as_str() {
            "override" => self.modified_ranks,
            "override_until_date" => self.modified_ranks + self.ranks + self.apply_learning_ranks,
            _ => self.ranks + self.modified_ranks + self.apply_learning_ranks,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_effective_ranks_modifier() {
        let mut t = Trainer::new(1, "Histia".to_string());
        t.ranks = 10;
        t.modified_ranks = 5;
        t.apply_learning_ranks = 3;
        assert_eq!(t.effective_ranks(), 18);
    }

    #[test]
    fn test_effective_ranks_override() {
        let mut t = Trainer::new(1, "Histia".to_string());
        t.rank_mode = "override".to_string();
        t.ranks = 10;
        t.modified_ranks = 50;
        t.apply_learning_ranks = 3;
        assert_eq!(t.effective_ranks(), 50);
    }

    #[test]
    fn test_effective_ranks_override_until_date() {
        let mut t = Trainer::new(1, "Histia".to_string());
        t.rank_mode = "override_until_date".to_string();
        t.ranks = 5; // post-cutoff ranks from parser
        t.modified_ranks = 45; // baseline
        t.apply_learning_ranks = 2; // post-cutoff apply learning
        assert_eq!(t.effective_ranks(), 52);
    }
}
