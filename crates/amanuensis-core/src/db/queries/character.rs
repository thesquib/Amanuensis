use rusqlite::params;

use crate::error::Result;
use crate::models::*;
use super::{CHARACTER_COLUMNS, map_character_row, Database};

impl Database {
    /// Get or create a character by name. Returns the character ID.
    pub fn get_or_create_character(&self, name: &str) -> Result<i64> {
        // Try to find existing
        let existing: Option<i64> = self
            .conn
            .query_row(
                "SELECT id FROM characters WHERE name = ?1",
                params![name],
                |row| row.get(0),
            )
            .ok();

        if let Some(id) = existing {
            return Ok(id);
        }

        // Create new
        self.conn.execute(
            "INSERT INTO characters (name) VALUES (?1)",
            params![name],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// Get a character by name.
    pub fn get_character(&self, name: &str) -> Result<Option<Character>> {
        let sql = format!("SELECT {CHARACTER_COLUMNS} FROM characters WHERE name = ?1");
        let result = self.conn.query_row(&sql, params![name], map_character_row);
        match result {
            Ok(c) => Ok(Some(c)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// List all characters (excludes merged-into characters and unscanned ghost rows).
    /// Characters with logins=0 are hidden — they are empty placeholder rows kept only
    /// to preserve rank overrides across rescans, and should not appear in the UI.
    pub fn list_characters(&self) -> Result<Vec<Character>> {
        let sql = format!(
            "SELECT {CHARACTER_COLUMNS}, \
             (SELECT COALESCE(SUM(ranks + apply_learning_ranks + modified_ranks), 0) \
              FROM trainers WHERE character_id = characters.id) as total_ranks \
             FROM characters WHERE merged_into IS NULL AND logins > 0 ORDER BY name"
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let chars = stmt.query_map([], |row| {
            let mut c = map_character_row(row)?;
            c.total_ranks = row.get(45)?;
            Ok(c)
        })?;
        Ok(chars.filter_map(|r| r.ok()).collect())
    }

    /// Increment a character counter field.
    pub fn increment_character_field(&self, char_id: i64, field: &str, amount: i64) -> Result<()> {
        // Only allow known fields to prevent SQL injection
        let allowed = [
            "logins", "departs", "deaths", "esteem",
            "coins_picked_up", "casino_won", "casino_lost",
            "chest_coins", "bounty_coins", "fur_coins", "mandible_coins", "blood_coins",
            "bells_used", "bells_broken", "chains_used", "chains_broken",
            "shieldstones_used", "shieldstones_broken", "ethereal_portals",
            "darkstone", "purgatory_pendant", "coin_level",
            "good_karma", "bad_karma", "gave_good_karma", "gave_bad_karma",
            "fur_worth", "mandible_worth", "blood_worth", "eps_broken",
            "untraining_count", "ore_found",
            "tin_ore_found", "copper_ore_found", "gold_ore_found", "iron_ore_found",
            "wood_taken", "wood_useless",
        ];
        if !allowed.contains(&field) {
            return Err(crate::error::AmanuensisError::Data(format!(
                "Unknown character field: {}",
                field
            )));
        }

        let sql = format!(
            "UPDATE characters SET {} = {} + ?1 WHERE id = ?2",
            field, field
        );
        self.conn.execute(&sql, params![amount, char_id])?;
        Ok(())
    }

    /// Set the departs counter to an absolute value (it's cumulative in logs).
    pub fn set_departs(&self, char_id: i64, count: i64) -> Result<()> {
        self.conn.execute(
            "UPDATE characters SET departs = ?1 WHERE id = ?2",
            params![count, char_id],
        )?;
        Ok(())
    }

    /// Update a character's profession.
    pub fn update_character_profession(&self, char_id: i64, profession: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE characters SET profession = ?1 WHERE id = ?2",
            params![profession, char_id],
        )?;
        Ok(())
    }

    /// Update a character's coin level.
    pub fn update_coin_level(&self, char_id: i64, coin_level: i64) -> Result<()> {
        self.conn.execute(
            "UPDATE characters SET coin_level = ?1 WHERE id = ?2",
            params![coin_level, char_id],
        )?;
        Ok(())
    }

    /// Update a character's interim coin level (best kill-verb value when threshold not met).
    pub fn update_coin_level_interim(&self, char_id: i64, interim: i64) -> Result<()> {
        self.conn.execute(
            "UPDATE characters SET coin_level_interim = ?1 WHERE id = ?2",
            params![interim, char_id],
        )?;
        Ok(())
    }

    /// Set a character's start_date to the earlier of the existing value and the new value.
    pub fn update_start_date(&self, char_id: i64, date: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE characters SET start_date = ?1
             WHERE id = ?2 AND (start_date IS NULL OR start_date > ?1)",
            params![date, char_id],
        )?;
        Ok(())
    }

    /// Set (or clear) a manual profession override for a character.
    /// Pass `None` to clear the override and revert to auto-detection on next scan.
    /// When setting a non-null value, also immediately updates the `profession` column.
    pub fn set_profession_override(&self, char_id: i64, profession: Option<&str>) -> Result<()> {
        self.conn.execute(
            "UPDATE characters SET profession_override = ?1 WHERE id = ?2",
            params![profession, char_id],
        )?;
        if let Some(prof) = profession {
            self.conn.execute(
                "UPDATE characters SET profession = ?1 WHERE id = ?2",
                params![prof, char_id],
            )?;
        }
        Ok(())
    }

    /// Get a character by ID (internal helper).
    pub fn get_character_by_id(&self, char_id: i64) -> Result<Option<Character>> {
        let sql = format!("SELECT {CHARACTER_COLUMNS} FROM characters WHERE id = ?1");
        let result = self.conn.query_row(&sql, params![char_id], map_character_row);
        match result {
            Ok(c) => Ok(Some(c)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Check if a character is merged, returning the target character's name if so.
    pub fn get_merged_into_name(&self, char_id: i64) -> Result<Option<String>> {
        let result = self.conn.query_row(
            "SELECT c2.name FROM characters c1
             JOIN characters c2 ON c1.merged_into = c2.id
             WHERE c1.id = ?1",
            params![char_id],
            |row| row.get::<_, String>(0),
        );
        match result {
            Ok(name) => Ok(Some(name)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Get a character by name, including merged characters (not filtered by merged_into).
    /// Useful for finding a character that might be hidden due to merge.
    pub fn get_character_including_merged(&self, name: &str) -> Result<Option<Character>> {
        let sql = format!("SELECT {CHARACTER_COLUMNS} FROM characters WHERE name = ?1");
        let result = self.conn.query_row(&sql, params![name], map_character_row);
        match result {
            Ok(c) => Ok(Some(c)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }
}
