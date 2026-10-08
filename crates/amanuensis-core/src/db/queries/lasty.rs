use rusqlite::params;

use crate::error::Result;
use crate::models::Lasty;
use super::Database;

impl Database {
    /// Upsert a lasty record. Increments message_count on subsequent encounters.
    /// `kills_left` is the milestone upper bound from progress-message wording;
    /// the stored value only ever tightens (MIN of known bounds), since progress
    /// decreases monotonically and rescans may replay messages out of order.
    /// Uses INSERT...ON CONFLICT for single-statement upsert performance.
    pub fn upsert_lasty(
        &self,
        char_id: i64,
        creature_name: &str,
        lasty_type: &str,
        date: &str,
        kills_left: Option<i64>,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT INTO lastys (character_id, creature_name, lasty_type, message_count, kills_left, first_seen_date, last_seen_date)
             VALUES (?1, ?2, ?3, 1, ?5, ?4, ?4)
             ON CONFLICT(character_id, creature_name, lasty_type) DO UPDATE SET
                message_count = message_count + 1,
                kills_left = COALESCE(MIN(lastys.kills_left, excluded.kills_left),
                                      lastys.kills_left, excluded.kills_left),
                last_seen_date = excluded.last_seen_date",
            params![char_id, creature_name, lasty_type, date, kills_left],
        )?;
        Ok(())
    }

    /// Mark a lasty as finished by creature name and type.
    /// INSERT with finished=1 if new, or UPDATE to set finished=1 and completed_date.
    pub fn finish_lasty(
        &self,
        char_id: i64,
        creature_name: &str,
        lasty_type: &str,
        date: &str,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT INTO lastys (character_id, creature_name, lasty_type, message_count, finished,
                                 first_seen_date, last_seen_date, completed_date)
             VALUES (?1, ?2, ?3, 1, 1, ?4, ?4, ?4)
             ON CONFLICT(character_id, creature_name, lasty_type) DO UPDATE SET
                message_count = message_count + 1,
                finished = 1,
                last_seen_date = excluded.last_seen_date,
                completed_date = excluded.completed_date",
            params![char_id, creature_name, lasty_type, date],
        )?;
        Ok(())
    }

    /// Mark a lasty as finished from reflect data.
    /// Unlike finish_lasty, this preserves an existing completed_date (only sets it if NULL),
    /// so a more precise log-derived date is never overwritten by the reflect timestamp.
    pub fn finish_lasty_from_reflect(
        &self,
        char_id: i64,
        creature_name: &str,
        lasty_type: &str,
        date: &str,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT INTO lastys (character_id, creature_name, lasty_type, message_count, finished,
                                 first_seen_date, last_seen_date, completed_date)
             VALUES (?1, ?2, ?3, 1, 1, ?4, ?4, ?4)
             ON CONFLICT(character_id, creature_name, lasty_type) DO UPDATE SET
                message_count = message_count + 1,
                finished = 1,
                last_seen_date = excluded.last_seen_date,
                completed_date = COALESCE(lastys.completed_date, excluded.completed_date)",
            params![char_id, creature_name, lasty_type, date],
        )?;
        Ok(())
    }

    /// Mark a lasty as completed (by trainer name — we find the most recent unfinished lasty).
    pub fn complete_lasty(&self, char_id: i64, _trainer: &str) -> Result<()> {
        // Mark the most recently updated unfinished lasty as complete
        self.conn.execute(
            "UPDATE lastys SET finished = 1
             WHERE id = (
                SELECT id FROM lastys
                WHERE character_id = ?1 AND finished = 0
                ORDER BY id DESC LIMIT 1
             )",
            params![char_id],
        )?;
        Ok(())
    }

    /// Record that a lasty study was abandoned. Sets abandoned_date on the matching record.
    pub fn abandon_lasty(
        &self,
        char_id: i64,
        creature_name: &str,
        date: &str,
    ) -> Result<()> {
        self.conn.execute(
            "UPDATE lastys SET abandoned_date = ?3
             WHERE character_id = ?1 AND creature_name = ?2",
            params![char_id, creature_name, date],
        )?;
        Ok(())
    }

    /// Clear the abandoned_date on a lasty (when study is resumed).
    pub fn clear_lasty_abandon(
        &self,
        char_id: i64,
        creature_name: &str,
    ) -> Result<()> {
        self.conn.execute(
            "UPDATE lastys SET abandoned_date = NULL
             WHERE character_id = ?1 AND creature_name = ?2",
            params![char_id, creature_name],
        )?;
        Ok(())
    }

    /// Zero the kills-since-message counter: called on each real study message
    /// (begin / progress), not on /reflect list lines.
    pub fn reset_lasty_kills(&self, char_id: i64, creature_name: &str, lasty_type: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE lastys SET kills_since_message = 0
             WHERE character_id = ?1 AND creature_name = ?2 AND lasty_type = ?3",
            params![char_id, creature_name, lasty_type],
        )?;
        Ok(())
    }

    /// Count a kill of `creature_name` toward every study of it still in progress.
    pub fn count_lasty_kill(&self, char_id: i64, creature_name: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE lastys SET kills_since_message = kills_since_message + 1
             WHERE character_id = ?1 AND creature_name = ?2
               AND finished = 0 AND abandoned_date IS NULL",
            params![char_id, creature_name],
        )?;
        Ok(())
    }

    /// Get lastys for a character.
    pub fn get_lastys(&self, char_id: i64) -> Result<Vec<Lasty>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, character_id, creature_name, lasty_type, finished, message_count,
                    kills_left, first_seen_date, last_seen_date, completed_date, abandoned_date,
                    kills_since_message
             FROM lastys WHERE character_id = ?1 ORDER BY creature_name",
        )?;

        let lastys = stmt.query_map(params![char_id], |row| {
            Ok(Lasty {
                id: Some(row.get(0)?),
                character_id: row.get(1)?,
                creature_name: row.get(2)?,
                lasty_type: row.get(3)?,
                finished: row.get::<_, i64>(4)? != 0,
                message_count: row.get(5)?,
                kills_left: row.get(6)?,
                first_seen_date: row.get(7)?,
                last_seen_date: row.get(8)?,
                completed_date: row.get(9)?,
                abandoned_date: row.get(10)?,
                kills_since_message: row.get(11)?,
            })
        })?;

        Ok(lastys.filter_map(|r| r.ok()).collect())
    }
}
