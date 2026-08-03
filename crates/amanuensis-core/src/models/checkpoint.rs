use serde::{Deserialize, Serialize};

/// A trainer rank checkpoint: a point-in-time reading of a character's rank
/// range with a specific trainer, derived from the trainer's greeting message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainerCheckpoint {
    pub id: Option<i64>,
    pub character_id: i64,
    pub trainer_name: String,
    pub rank_min: i64,
    /// None = maxed (no upper bound)
    pub rank_max: Option<i64>,
    pub timestamp: String,
    /// True when the greeting came from a player-run ("ledger") trainer rather than an NPC.
    /// Always false in results fetched with `include_players = false`, since those are excluded.
    pub is_player: bool,
}
