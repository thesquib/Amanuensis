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

#[cfg(test)]
mod tests {
    use super::super::Database;

    #[test]
    fn test_insert_and_get_latest() {
        let db = Database::open_in_memory().unwrap();
        let char_id = db.get_or_create_character("Fen").unwrap();

        db.insert_trainer_checkpoint(char_id, "Histia", 0, Some(9), "2024-01-01 12:00:00").unwrap();
        db.insert_trainer_checkpoint(char_id, "Histia", 10, Some(19), "2024-01-02 12:00:00").unwrap();

        let checkpoints = db.get_latest_trainer_checkpoints(char_id).unwrap();
        assert_eq!(checkpoints.len(), 1, "Should return exactly one row per trainer");
        assert_eq!(checkpoints[0].trainer_name, "Histia");
        assert_eq!(checkpoints[0].rank_min, 10, "Should return the most recent checkpoint");
    }

    #[test]
    fn test_get_latest_isolates_by_character() {
        let db = Database::open_in_memory().unwrap();
        let char_a = db.get_or_create_character("CharA").unwrap();
        let char_b = db.get_or_create_character("CharB").unwrap();

        db.insert_trainer_checkpoint(char_a, "Histia", 50, Some(99), "2024-01-01 12:00:00").unwrap();
        db.insert_trainer_checkpoint(char_b, "Histia", 100, Some(149), "2024-01-02 12:00:00").unwrap();

        let checkpoints_a = db.get_latest_trainer_checkpoints(char_a).unwrap();
        assert_eq!(checkpoints_a.len(), 1);
        assert_eq!(checkpoints_a[0].rank_min, 50, "CharA should only see their own checkpoint");

        let checkpoints_b = db.get_latest_trainer_checkpoints(char_b).unwrap();
        assert_eq!(checkpoints_b.len(), 1);
        assert_eq!(checkpoints_b[0].rank_min, 100, "CharB should only see their own checkpoint");
    }

    #[test]
    fn test_get_all_chronological() {
        let db = Database::open_in_memory().unwrap();
        let char_id = db.get_or_create_character("Fen").unwrap();

        db.insert_trainer_checkpoint(char_id, "Histia", 0, Some(9), "2024-01-01 12:00:00").unwrap();
        db.insert_trainer_checkpoint(char_id, "Histia", 10, Some(19), "2024-01-02 12:00:00").unwrap();
        db.insert_trainer_checkpoint(char_id, "Histia", 20, Some(29), "2024-01-03 12:00:00").unwrap();

        let checkpoints = db.get_all_trainer_checkpoints(char_id).unwrap();
        assert_eq!(checkpoints.len(), 3);
        assert_eq!(checkpoints[0].rank_min, 0);
        assert_eq!(checkpoints[1].rank_min, 10);
        assert_eq!(checkpoints[2].rank_min, 20, "Should be returned in ascending timestamp order");
    }

    #[test]
    fn test_rank_max_none_roundtrips() {
        let db = Database::open_in_memory().unwrap();
        let char_id = db.get_or_create_character("Fen").unwrap();

        db.insert_trainer_checkpoint(char_id, "Histia", 5750, None, "2024-01-01 12:00:00").unwrap();

        let checkpoints = db.get_all_trainer_checkpoints(char_id).unwrap();
        assert_eq!(checkpoints.len(), 1);
        assert_eq!(checkpoints[0].rank_min, 5750);
        assert_eq!(checkpoints[0].rank_max, None, "rank_max=None should roundtrip as None");
    }
}
