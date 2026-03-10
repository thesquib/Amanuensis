use serde::{Deserialize, Serialize};

/// The three rank-tracking modes for a trainer record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RankMode {
    /// Additive modifier: log ranks + modified_ranks + apply_learning_ranks (default).
    Modifier,
    /// Full manual override: only modified_ranks counts.
    Override,
    /// Baseline + post-cutoff: modified_ranks + log ranks after the override_date.
    OverrideUntilDate,
}

impl RankMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            RankMode::Modifier => "modifier",
            RankMode::Override => "override",
            RankMode::OverrideUntilDate => "override_until_date",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "modifier" => Some(RankMode::Modifier),
            "override" => Some(RankMode::Override),
            "override_until_date" => Some(RankMode::OverrideUntilDate),
            _ => None,
        }
    }

    /// Returns true for modes that require resetting log-derived ranks on activation.
    pub fn is_override_mode(&self) -> bool {
        matches!(self, RankMode::Override | RankMode::OverrideUntilDate)
    }
}

impl std::fmt::Display for RankMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

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
    pub effective_multiplier: f64,
    pub notes: Option<String>,
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
            rank_mode: RankMode::Modifier.as_str().to_string(),
            override_date: None,
            effective_multiplier: 1.0,
            notes: None,
        }
    }

    /// Compute effective ranks based on the rank mode.
    ///
    /// - `modifier`: ranks + modified_ranks + apply_learning_ranks (default additive)
    /// - `override`: modified_ranks only (manual value replaces logs)
    /// - `override_until_date`: modified_ranks + ranks + apply_learning_ranks
    ///   (ranks/apply_learning_ranks only contain post-cutoff counts from parser)
    pub fn effective_ranks(&self) -> i64 {
        match RankMode::parse(&self.rank_mode) {
            Some(RankMode::Override) => self.modified_ranks,
            Some(RankMode::OverrideUntilDate) => {
                self.modified_ranks + self.ranks + self.apply_learning_ranks
            }
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
        t.rank_mode = RankMode::Override.as_str().to_string();
        t.ranks = 10;
        t.modified_ranks = 50;
        t.apply_learning_ranks = 3;
        assert_eq!(t.effective_ranks(), 50);
    }

    #[test]
    fn test_effective_ranks_override_until_date() {
        let mut t = Trainer::new(1, "Histia".to_string());
        t.rank_mode = RankMode::OverrideUntilDate.as_str().to_string();
        t.ranks = 5; // post-cutoff ranks from parser
        t.modified_ranks = 45; // baseline
        t.apply_learning_ranks = 2; // post-cutoff apply learning
        assert_eq!(t.effective_ranks(), 52);
    }
}
