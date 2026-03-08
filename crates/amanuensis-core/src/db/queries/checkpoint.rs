use rusqlite::params;

use crate::error::Result;
use crate::models::TrainerCheckpoint;
use super::Database;

impl Database {
    /// Record a trainer rank checkpoint event.
    pub fn insert_trainer_checkpoint(
        &self,
        char_id: i64,
        trainer_name: &str,
        rank_min: i64,
        rank_max: Option<i64>,
        timestamp: &str,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT INTO trainer_checkpoints (character_id, trainer_name, rank_min, rank_max, timestamp, name_filtered)
             VALUES (?1, ?2, ?3, ?4, ?5, 1)",
            params![char_id, trainer_name, rank_min, rank_max, timestamp],
        )?;
        Ok(())
    }

    /// Get the most recent checkpoint for each trainer for a character.
    pub fn get_latest_trainer_checkpoints(&self, char_id: i64) -> Result<Vec<TrainerCheckpoint>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, character_id, trainer_name, rank_min, rank_max, timestamp
             FROM trainer_checkpoints
             WHERE character_id = ?1
               AND id = (
                 SELECT MAX(id) FROM trainer_checkpoints t2
                 WHERE t2.character_id = trainer_checkpoints.character_id
                   AND t2.trainer_name = trainer_checkpoints.trainer_name
               )
             ORDER BY trainer_name",
        )?;

        let rows = stmt.query_map(params![char_id], |row| {
            Ok(TrainerCheckpoint {
                id: Some(row.get(0)?),
                character_id: row.get(1)?,
                trainer_name: row.get(2)?,
                rank_min: row.get(3)?,
                rank_max: row.get(4)?,
                timestamp: row.get(5)?,
            })
        })?;

        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    /// Get all checkpoint events for a character, sorted by timestamp ascending.
    /// Used for the checkpoint progression timeline graph.
    pub fn get_all_trainer_checkpoints(&self, char_id: i64) -> Result<Vec<TrainerCheckpoint>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, character_id, trainer_name, rank_min, rank_max, timestamp
             FROM trainer_checkpoints
             WHERE character_id = ?1
             ORDER BY timestamp ASC, id ASC",
        )?;

        let rows = stmt.query_map(params![char_id], |row| {
            Ok(TrainerCheckpoint {
                id: Some(row.get(0)?),
                character_id: row.get(1)?,
                trainer_name: row.get(2)?,
                rank_min: row.get(3)?,
                rank_max: row.get(4)?,
                timestamp: row.get(5)?,
            })
        })?;

        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    /// Get full checkpoint history for a specific trainer and character.
    pub fn get_trainer_checkpoint_history(
        &self,
        char_id: i64,
        trainer_name: &str,
    ) -> Result<Vec<TrainerCheckpoint>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, character_id, trainer_name, rank_min, rank_max, timestamp
             FROM trainer_checkpoints
             WHERE character_id = ?1 AND trainer_name = ?2
             ORDER BY id ASC",
        )?;

        let rows = stmt.query_map(params![char_id, trainer_name], |row| {
            Ok(TrainerCheckpoint {
                id: Some(row.get(0)?),
                character_id: row.get(1)?,
                trainer_name: row.get(2)?,
                rank_min: row.get(3)?,
                rank_max: row.get(4)?,
                timestamp: row.get(5)?,
            })
        })?;

        Ok(rows.filter_map(|r| r.ok()).collect())
    }
}
