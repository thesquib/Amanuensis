use rusqlite::params;

use crate::error::Result;
use crate::models::{RankMode, Trainer};
use super::Database;

impl Database {
    /// Upsert a trainer rank.
    /// Uses INSERT...ON CONFLICT for single-statement upsert performance.
    pub fn upsert_trainer_rank(
        &self,
        char_id: i64,
        trainer_name: &str,
        date: &str,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT INTO trainers (character_id, trainer_name, ranks, date_of_last_rank)
             VALUES (?1, ?2, 1, ?3)
             ON CONFLICT(character_id, trainer_name) DO UPDATE SET
                ranks = ranks + 1,
                date_of_last_rank = excluded.date_of_last_rank",
            params![char_id, trainer_name, date],
        )?;
        Ok(())
    }

    /// Get trainers for a character, ordered by ranks descending.
    pub fn get_trainers(&self, char_id: i64) -> Result<Vec<Trainer>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, character_id, trainer_name, ranks, modified_ranks, date_of_last_rank,
                    apply_learning_ranks, apply_learning_unknown_count, rank_mode, override_date
             FROM trainers WHERE character_id = ?1 ORDER BY ranks DESC",
        )?;

        let trainers = stmt.query_map(params![char_id], |row| {
            Ok(Trainer {
                id: Some(row.get(0)?),
                character_id: row.get(1)?,
                trainer_name: row.get(2)?,
                ranks: row.get(3)?,
                modified_ranks: row.get(4)?,
                date_of_last_rank: row.get(5)?,
                apply_learning_ranks: row.get(6)?,
                apply_learning_unknown_count: row.get(7)?,
                rank_mode: row.get(8)?,
                override_date: row.get(9)?,
            })
        })?;

        Ok(trainers.filter_map(|r| r.ok()).collect())
    }

    /// Upsert apply-learning confirmed ranks (10 per "much more" event).
    pub fn upsert_apply_learning(
        &self,
        char_id: i64,
        trainer_name: &str,
        date: &str,
        amount: i64,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT INTO trainers (character_id, trainer_name, apply_learning_ranks, date_of_last_rank)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(character_id, trainer_name) DO UPDATE SET
                apply_learning_ranks = apply_learning_ranks + ?3,
                date_of_last_rank = excluded.date_of_last_rank",
            params![char_id, trainer_name, amount, date],
        )?;
        Ok(())
    }

    /// Upsert apply-learning unknown count (1 per "more" event).
    pub fn upsert_apply_learning_unknown(
        &self,
        char_id: i64,
        trainer_name: &str,
        date: &str,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT INTO trainers (character_id, trainer_name, apply_learning_unknown_count, date_of_last_rank)
             VALUES (?1, ?2, 1, ?3)
             ON CONFLICT(character_id, trainer_name) DO UPDATE SET
                apply_learning_unknown_count = apply_learning_unknown_count + 1,
                date_of_last_rank = excluded.date_of_last_rank",
            params![char_id, trainer_name, date],
        )?;
        Ok(())
    }

    /// Set the modified_ranks for a specific trainer record.
    /// Creates the trainer record if it doesn't exist (for pre-log baseline ranks).
    /// Recalculates coin_level after the update.
    pub fn set_modified_ranks(
        &self,
        char_id: i64,
        trainer_name: &str,
        modified_ranks: i64,
    ) -> Result<()> {
        self.set_rank_override(char_id, trainer_name, RankMode::Modifier.as_str(), modified_ranks, None)
    }

    /// Set rank override mode for a specific trainer record.
    /// Creates the trainer record if it doesn't exist.
    /// When switching TO override or override_until_date, zeros ranks and apply_learning_ranks
    /// so the parser can rebuild only post-cutoff counts on next scan.
    /// Recalculates coin_level after the update.
    pub fn set_rank_override(
        &self,
        char_id: i64,
        trainer_name: &str,
        rank_mode: &str,
        modified_ranks: i64,
        override_date: Option<&str>,
    ) -> Result<()> {
        // Validate rank_mode
        let parsed_mode = RankMode::parse(rank_mode).ok_or_else(|| {
            crate::error::AmanuensisError::Data(format!(
                "Invalid rank_mode: {}. Must be one of: modifier, override, override_until_date",
                rank_mode
            ))
        })?;

        // Check if we're switching to a non-modifier mode that needs rank zeroing
        let current_mode: Option<String> = self.conn.query_row(
            "SELECT rank_mode FROM trainers WHERE character_id = ?1 AND trainer_name = ?2",
            params![char_id, trainer_name],
            |row| row.get(0),
        ).ok();

        let switching_to_override = parsed_mode.is_override_mode()
            && current_mode.as_deref() != Some(rank_mode);

        // Upsert the trainer record
        self.conn.execute(
            "INSERT INTO trainers (character_id, trainer_name, ranks, modified_ranks, rank_mode, override_date)
             VALUES (?1, ?2, 0, ?3, ?4, ?5)
             ON CONFLICT(character_id, trainer_name) DO UPDATE SET
                modified_ranks = excluded.modified_ranks,
                rank_mode = excluded.rank_mode,
                override_date = excluded.override_date",
            params![char_id, trainer_name, modified_ranks, rank_mode, override_date],
        )?;

        // When switching TO override or override_until_date, zero out log-derived ranks
        // so the next scan rebuilds only post-cutoff counts correctly
        if switching_to_override {
            self.conn.execute(
                "UPDATE trainers SET ranks = 0, apply_learning_ranks = 0
                 WHERE character_id = ?1 AND trainer_name = ?2",
                params![char_id, trainer_name],
            )?;
        }

        // Recalculate coin level using effective_ranks
        let coin_level = self.compute_effective_coin_level(char_id)?;
        self.update_coin_level(char_id, coin_level)?;

        Ok(())
    }

    /// Compute coin level using mode-aware effective_ranks for each trainer.
    pub fn compute_effective_coin_level(&self, char_id: i64) -> Result<i64> {
        let trainers = self.get_trainers(char_id)?;
        Ok(trainers.iter().map(|t| t.effective_ranks()).sum())
    }
}
