use rusqlite::params;

use crate::error::Result;
use crate::models::TrainerCheckpoint;
use super::Database;

/// Joined onto every checkpoint query so player-run trainers can be identified and filtered.
/// `known_players.name` is stored lowercased, hence the LOWER() on the join.
const PLAYER_JOIN: &str = "LEFT JOIN known_players kp ON kp.name = LOWER(tc.trainer_name)";

/// Appended to the WHERE clause unless players are being included.
const PLAYER_FILTER: &str = " AND kp.name IS NULL";

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
    /// "Most recent" is by log timestamp (the date/time from the log file), not insertion order.
    /// Player-run trainers are excluded unless `include_players` is set.
    pub fn get_latest_trainer_checkpoints(
        &self,
        char_id: i64,
        include_players: bool,
    ) -> Result<Vec<TrainerCheckpoint>> {
        let sql = format!(
            "SELECT tc.id, tc.character_id, tc.trainer_name, tc.rank_min, tc.rank_max,
                    tc.timestamp, kp.name IS NOT NULL AS is_player
             FROM trainer_checkpoints tc
             {PLAYER_JOIN}
             WHERE tc.character_id = ?1
               AND tc.rowid = (
                 SELECT t2.rowid FROM trainer_checkpoints t2
                 WHERE t2.character_id = tc.character_id
                   AND t2.trainer_name = tc.trainer_name
                 ORDER BY t2.timestamp DESC, t2.id DESC
                 LIMIT 1
               ){filter}
             ORDER BY tc.trainer_name",
            filter = if include_players { "" } else { PLAYER_FILTER },
        );

        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(params![char_id], map_checkpoint_row)?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    /// Get all checkpoint events for a character, sorted by timestamp ascending.
    /// Used for the checkpoint progression timeline graph.
    /// Player-run trainers are excluded unless `include_players` is set.
    pub fn get_all_trainer_checkpoints(
        &self,
        char_id: i64,
        include_players: bool,
    ) -> Result<Vec<TrainerCheckpoint>> {
        let sql = format!(
            "SELECT tc.id, tc.character_id, tc.trainer_name, tc.rank_min, tc.rank_max,
                    tc.timestamp, kp.name IS NOT NULL AS is_player
             FROM trainer_checkpoints tc
             {PLAYER_JOIN}
             WHERE tc.character_id = ?1{filter}
             ORDER BY tc.timestamp ASC, tc.id ASC",
            filter = if include_players { "" } else { PLAYER_FILTER },
        );

        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(params![char_id], map_checkpoint_row)?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    /// Get full checkpoint history for a specific trainer and character.
    /// Player-run trainers are excluded unless `include_players` is set.
    pub fn get_trainer_checkpoint_history(
        &self,
        char_id: i64,
        trainer_name: &str,
        include_players: bool,
    ) -> Result<Vec<TrainerCheckpoint>> {
        let sql = format!(
            "SELECT tc.id, tc.character_id, tc.trainer_name, tc.rank_min, tc.rank_max,
                    tc.timestamp, kp.name IS NOT NULL AS is_player
             FROM trainer_checkpoints tc
             {PLAYER_JOIN}
             WHERE tc.character_id = ?1 AND tc.trainer_name = ?2{filter}
             ORDER BY tc.id ASC",
            filter = if include_players { "" } else { PLAYER_FILTER },
        );

        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(params![char_id, trainer_name], map_checkpoint_row)?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }
}

/// Shared row -> TrainerCheckpoint mapping for the three checkpoint queries,
/// which all select the same seven columns in the same order.
fn map_checkpoint_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<TrainerCheckpoint> {
    Ok(TrainerCheckpoint {
        id: Some(row.get(0)?),
        character_id: row.get(1)?,
        trainer_name: row.get(2)?,
        rank_min: row.get(3)?,
        rank_max: row.get(4)?,
        timestamp: row.get(5)?,
        is_player: row.get(6)?,
    })
}

#[cfg(test)]
mod tests {
    use super::super::Database;
    use crate::parser::player_signals::PlayerSignal;

    #[test]
    fn player_checkpoints_are_hidden_by_default() {
        let db = Database::open_in_memory().unwrap();
        let char_id = db.get_or_create_character("Ruuk").unwrap();

        db.insert_trainer_checkpoint(char_id, "Histia", 100, Some(149), "2024-01-01 12:00:00").unwrap();
        db.insert_trainer_checkpoint(char_id, "Fenwick", 50, Some(99), "2024-01-02 12:00:00").unwrap();
        db.upsert_known_player("Fenwick", PlayerSignal::Clanning, "2024-01-02 12:00:00").unwrap();

        let hidden = db.get_all_trainer_checkpoints(char_id, false).unwrap();
        assert_eq!(hidden.len(), 1);
        assert_eq!(hidden[0].trainer_name, "Histia");
        assert!(!hidden[0].is_player);

        let shown = db.get_all_trainer_checkpoints(char_id, true).unwrap();
        assert_eq!(shown.len(), 2);
        let fenwick = shown.iter().find(|c| c.trainer_name == "Fenwick").unwrap();
        assert!(fenwick.is_player, "Fenwick should be marked as a player when included");
    }

    #[test]
    fn filtering_is_case_insensitive() {
        let db = Database::open_in_memory().unwrap();
        let char_id = db.get_or_create_character("Ruuk").unwrap();

        // Checkpoint stores the display casing; known_players stores lowercase.
        db.insert_trainer_checkpoint(char_id, "Bramwell Gorse", 0, Some(9), "2024-01-01 12:00:00").unwrap();
        db.upsert_known_player("bramwell gorse", PlayerSignal::Offer, "2024-01-01 12:00:00").unwrap();

        assert!(db.get_all_trainer_checkpoints(char_id, false).unwrap().is_empty());
        assert_eq!(db.get_all_trainer_checkpoints(char_id, true).unwrap().len(), 1);
    }

    #[test]
    fn latest_and_history_also_filter() {
        let db = Database::open_in_memory().unwrap();
        let char_id = db.get_or_create_character("Ruuk").unwrap();

        db.insert_trainer_checkpoint(char_id, "Fenwick", 50, Some(99), "2024-01-01 12:00:00").unwrap();
        db.insert_trainer_checkpoint(char_id, "Histia", 10, Some(19), "2024-01-01 12:00:00").unwrap();
        db.upsert_known_player("Fenwick", PlayerSignal::Ledger, "2024-01-01 12:00:00").unwrap();

        let latest = db.get_latest_trainer_checkpoints(char_id, false).unwrap();
        assert_eq!(latest.len(), 1);
        assert_eq!(latest[0].trainer_name, "Histia");
        assert_eq!(db.get_latest_trainer_checkpoints(char_id, true).unwrap().len(), 2);

        assert!(db.get_trainer_checkpoint_history(char_id, "Fenwick", false).unwrap().is_empty());
        assert_eq!(db.get_trainer_checkpoint_history(char_id, "Fenwick", true).unwrap().len(), 1);
    }

    #[test]
    fn retroactively_learning_a_player_hides_old_checkpoints() {
        let db = Database::open_in_memory().unwrap();
        let char_id = db.get_or_create_character("Ruuk").unwrap();
        db.insert_trainer_checkpoint(char_id, "Fenwick", 50, Some(99), "2017-11-22 23:07:16").unwrap();

        // Before detection: Fenwick looks like a trainer.
        assert_eq!(db.get_all_trainer_checkpoints(char_id, false).unwrap().len(), 1);

        // Learning it later must clean history without touching the rows.
        db.upsert_known_player("Fenwick", PlayerSignal::Clanning, "2017-11-22 23:07:16").unwrap();
        assert!(db.get_all_trainer_checkpoints(char_id, false).unwrap().is_empty());

        let stored: i64 = db
            .conn()
            .query_row("SELECT COUNT(*) FROM trainer_checkpoints", [], |r| r.get(0))
            .unwrap();
        assert_eq!(stored, 1, "checkpoint rows must never be deleted");
    }

    #[test]
    fn test_insert_and_get_latest() {
        let db = Database::open_in_memory().unwrap();
        let char_id = db.get_or_create_character("Fen").unwrap();

        db.insert_trainer_checkpoint(char_id, "Histia", 0, Some(9), "2024-01-01 12:00:00").unwrap();
        db.insert_trainer_checkpoint(char_id, "Histia", 10, Some(19), "2024-01-02 12:00:00").unwrap();

        let checkpoints = db.get_latest_trainer_checkpoints(char_id, true).unwrap();
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

        let checkpoints_a = db.get_latest_trainer_checkpoints(char_a, true).unwrap();
        assert_eq!(checkpoints_a.len(), 1);
        assert_eq!(checkpoints_a[0].rank_min, 50, "CharA should only see their own checkpoint");

        let checkpoints_b = db.get_latest_trainer_checkpoints(char_b, true).unwrap();
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

        let checkpoints = db.get_all_trainer_checkpoints(char_id, true).unwrap();
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

        let checkpoints = db.get_all_trainer_checkpoints(char_id, true).unwrap();
        assert_eq!(checkpoints.len(), 1);
        assert_eq!(checkpoints[0].rank_min, 5750);
        assert_eq!(checkpoints[0].rank_max, None, "rank_max=None should roundtrip as None");
    }
}
