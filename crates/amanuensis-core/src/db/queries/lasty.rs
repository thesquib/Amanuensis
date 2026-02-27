use rusqlite::params;

use crate::error::Result;
use crate::models::Lasty;
use super::Database;

impl Database {
    /// Upsert a lasty record. Increments message_count on subsequent encounters.
    /// Uses INSERT...ON CONFLICT for single-statement upsert performance.
    pub fn upsert_lasty(
        &self,
        char_id: i64,
        creature_name: &str,
        lasty_type: &str,
        date: &str,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT INTO lastys (character_id, creature_name, lasty_type, message_count, first_seen_date, last_seen_date)
             VALUES (?1, ?2, ?3, 1, ?4, ?4)
             ON CONFLICT(character_id, creature_name) DO UPDATE SET
                message_count = message_count + 1,
                last_seen_date = excluded.last_seen_date",
            params![char_id, creature_name, lasty_type, date],
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
             ON CONFLICT(character_id, creature_name) DO UPDATE SET
                message_count = message_count + 1,
                finished = 1,
                last_seen_date = excluded.last_seen_date,
                completed_date = excluded.completed_date",
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

    /// Get lastys for a character.
    pub fn get_lastys(&self, char_id: i64) -> Result<Vec<Lasty>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, character_id, creature_name, lasty_type, finished, message_count,
                    first_seen_date, last_seen_date, completed_date, abandoned_date
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
                first_seen_date: row.get(6)?,
                last_seen_date: row.get(7)?,
                completed_date: row.get(8)?,
                abandoned_date: row.get(9)?,
            })
        })?;

        Ok(lastys.filter_map(|r| r.ok()).collect())
    }
}
