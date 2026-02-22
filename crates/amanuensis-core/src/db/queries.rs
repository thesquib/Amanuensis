use rusqlite::{params, Connection};
use serde::Serialize;

use crate::error::Result;
use crate::models::*;

/// A single search result from the FTS5 log_lines table.
#[derive(Debug, Serialize)]
pub struct LogSearchResult {
    pub content: String,
    pub character_id: i64,
    pub timestamp: String,
    pub file_path: String,
    pub snippet: String,
    pub character_name: String,
}

/// Database wrapper with CRUD operations.
pub struct Database {
    conn: Connection,
}

impl Database {
    /// Open (or create) a SQLite database at the given path.
    pub fn open(path: &str) -> Result<Self> {
        let conn = Connection::open(path)?;
        crate::db::schema::create_tables(&conn)?;
        crate::db::schema::migrate_tables(&conn)?;
        Ok(Self { conn })
    }

    /// Open an in-memory database (for testing).
    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        crate::db::schema::create_tables(&conn)?;
        crate::db::schema::migrate_tables(&conn)?;
        Ok(Self { conn })
    }

    pub fn conn(&self) -> &Connection {
        &self.conn
    }

    /// Begin a transaction for batch operations.
    pub fn begin_transaction(&self) -> Result<()> {
        self.conn.execute_batch("BEGIN")?;
        Ok(())
    }

    /// Commit the current transaction.
    pub fn commit_transaction(&self) -> Result<()> {
        self.conn.execute_batch("COMMIT")?;
        Ok(())
    }

    /// Rollback the current transaction.
    pub fn rollback_transaction(&self) -> Result<()> {
        self.conn.execute_batch("ROLLBACK")?;
        Ok(())
    }

    /// Set performance PRAGMAs for bulk scanning operations.
    pub fn set_scan_pragmas(&self) -> Result<()> {
        self.conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             PRAGMA cache_size = -64000;
             PRAGMA temp_store = MEMORY;
             PRAGMA mmap_size = 268435456;",
        )?;
        Ok(())
    }

    /// Reset PRAGMAs to safe defaults after scanning.
    pub fn reset_pragmas(&self) -> Result<()> {
        self.conn.execute_batch(
            "PRAGMA synchronous = FULL;
             PRAGMA cache_size = -2000;",
        )?;
        Ok(())
    }

    // === Characters ===

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
        let result = self.conn.query_row(
            "SELECT id, name, profession, logins, departs, deaths, esteem, armor,
                    coins_picked_up, casino_won, casino_lost, chest_coins, bounty_coins,
                    fur_coins, mandible_coins, blood_coins,
                    bells_used, bells_broken, chains_used, chains_broken,
                    shieldstones_used, shieldstones_broken, ethereal_portals, darkstone, purgatory_pendant,
                    coin_level, good_karma, bad_karma, start_date,
                    fur_worth, mandible_worth, blood_worth, eps_broken, untraining_count
             FROM characters WHERE name = ?1",
            params![name],
            |row| {
                Ok(Character {
                    id: Some(row.get(0)?),
                    name: row.get(1)?,
                    profession: Profession::parse(&row.get::<_, String>(2)?),
                    logins: row.get(3)?,
                    departs: row.get(4)?,
                    deaths: row.get(5)?,
                    esteem: row.get(6)?,
                    armor: row.get(7)?,
                    coins_picked_up: row.get(8)?,
                    casino_won: row.get(9)?,
                    casino_lost: row.get(10)?,
                    chest_coins: row.get(11)?,
                    bounty_coins: row.get(12)?,
                    fur_coins: row.get(13)?,
                    mandible_coins: row.get(14)?,
                    blood_coins: row.get(15)?,
                    bells_used: row.get(16)?,
                    bells_broken: row.get(17)?,
                    chains_used: row.get(18)?,
                    chains_broken: row.get(19)?,
                    shieldstones_used: row.get(20)?,
                    shieldstones_broken: row.get(21)?,
                    ethereal_portals: row.get(22)?,
                    darkstone: row.get(23)?,
                    purgatory_pendant: row.get(24)?,
                    coin_level: row.get(25)?,
                    good_karma: row.get(26)?,
                    bad_karma: row.get(27)?,
                    start_date: row.get(28)?,
                    fur_worth: row.get(29)?,
                    mandible_worth: row.get(30)?,
                    blood_worth: row.get(31)?,
                    eps_broken: row.get(32)?,
                    untraining_count: row.get(33)?,
                })
            },
        );

        match result {
            Ok(c) => Ok(Some(c)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// List all characters (excludes characters that have been merged into another).
    pub fn list_characters(&self) -> Result<Vec<Character>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, profession, logins, departs, deaths, esteem, armor,
                    coins_picked_up, casino_won, casino_lost, chest_coins, bounty_coins,
                    fur_coins, mandible_coins, blood_coins,
                    bells_used, bells_broken, chains_used, chains_broken,
                    shieldstones_used, shieldstones_broken, ethereal_portals, darkstone, purgatory_pendant,
                    coin_level, good_karma, bad_karma, start_date,
                    fur_worth, mandible_worth, blood_worth, eps_broken, untraining_count
             FROM characters WHERE merged_into IS NULL ORDER BY name",
        )?;

        let chars = stmt.query_map([], |row| {
            Ok(Character {
                id: Some(row.get(0)?),
                name: row.get(1)?,
                profession: Profession::parse(&row.get::<_, String>(2)?),
                logins: row.get(3)?,
                departs: row.get(4)?,
                deaths: row.get(5)?,
                esteem: row.get(6)?,
                armor: row.get(7)?,
                coins_picked_up: row.get(8)?,
                casino_won: row.get(9)?,
                casino_lost: row.get(10)?,
                chest_coins: row.get(11)?,
                bounty_coins: row.get(12)?,
                fur_coins: row.get(13)?,
                mandible_coins: row.get(14)?,
                blood_coins: row.get(15)?,
                bells_used: row.get(16)?,
                bells_broken: row.get(17)?,
                chains_used: row.get(18)?,
                chains_broken: row.get(19)?,
                shieldstones_used: row.get(20)?,
                shieldstones_broken: row.get(21)?,
                ethereal_portals: row.get(22)?,
                darkstone: row.get(23)?,
                purgatory_pendant: row.get(24)?,
                coin_level: row.get(25)?,
                good_karma: row.get(26)?,
                bad_karma: row.get(27)?,
                start_date: row.get(28)?,
                fur_worth: row.get(29)?,
                mandible_worth: row.get(30)?,
                blood_worth: row.get(31)?,
                eps_broken: row.get(32)?,
                untraining_count: row.get(33)?,
            })
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
            "good_karma", "bad_karma",
            "fur_worth", "mandible_worth", "blood_worth", "eps_broken",
            "untraining_count",
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

    // === Kills ===

    /// Upsert a kill record. Increments the appropriate count field.
    /// Uses INSERT...ON CONFLICT for single-statement upsert performance.
    pub fn upsert_kill(
        &self,
        char_id: i64,
        creature_name: &str,
        field: &str,
        creature_value: i32,
        date: &str,
    ) -> Result<()> {
        let allowed = [
            "killed_count", "slaughtered_count", "vanquished_count", "dispatched_count",
            "assisted_kill_count", "assisted_slaughter_count", "assisted_vanquish_count",
            "assisted_dispatch_count", "killed_by_count",
        ];
        if !allowed.contains(&field) {
            return Err(crate::error::AmanuensisError::Data(format!(
                "Unknown kill field: {}",
                field
            )));
        }

        // Determine the per-type date column to update (solo kill types only)
        let date_col = match field {
            "killed_count" => Some("date_last_killed"),
            "slaughtered_count" => Some("date_last_slaughtered"),
            "vanquished_count" => Some("date_last_vanquished"),
            "dispatched_count" => Some("date_last_dispatched"),
            _ => None,
        };

        let date_col_insert = date_col.map(|c| format!(", {c}")).unwrap_or_default();
        let date_col_value = if date_col.is_some() { ", ?4" } else { "" };
        let date_col_update = date_col
            .map(|c| format!(", {c} = excluded.{c}"))
            .unwrap_or_default();

        let sql = format!(
            "INSERT INTO kills (character_id, creature_name, {field}, creature_value, date_first, date_last{date_col_insert})
             VALUES (?1, ?2, 1, ?3, ?4, ?4{date_col_value})
             ON CONFLICT(character_id, creature_name) DO UPDATE SET
                {field} = {field} + 1,
                date_last = excluded.date_last{date_col_update}",
        );
        self.conn.execute(
            &sql,
            params![char_id, creature_name, creature_value, date],
        )?;
        Ok(())
    }

    /// Get kills for a character, ordered by total count descending.
    pub fn get_kills(&self, char_id: i64) -> Result<Vec<Kill>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, character_id, creature_name,
                    killed_count, slaughtered_count, vanquished_count, dispatched_count,
                    assisted_kill_count, assisted_slaughter_count, assisted_vanquish_count, assisted_dispatch_count,
                    killed_by_count, date_first, date_last, creature_value,
                    date_last_killed, date_last_slaughtered, date_last_vanquished, date_last_dispatched
             FROM kills WHERE character_id = ?1
             ORDER BY (killed_count + slaughtered_count + vanquished_count + dispatched_count +
                       assisted_kill_count + assisted_slaughter_count + assisted_vanquish_count + assisted_dispatch_count) DESC",
        )?;

        let kills = stmt.query_map(params![char_id], |row| {
            Ok(Kill {
                id: Some(row.get(0)?),
                character_id: row.get(1)?,
                creature_name: row.get(2)?,
                killed_count: row.get(3)?,
                slaughtered_count: row.get(4)?,
                vanquished_count: row.get(5)?,
                dispatched_count: row.get(6)?,
                assisted_kill_count: row.get(7)?,
                assisted_slaughter_count: row.get(8)?,
                assisted_vanquish_count: row.get(9)?,
                assisted_dispatch_count: row.get(10)?,
                killed_by_count: row.get(11)?,
                date_first: row.get(12)?,
                date_last: row.get(13)?,
                creature_value: row.get(14)?,
                date_last_killed: row.get(15)?,
                date_last_slaughtered: row.get(16)?,
                date_last_vanquished: row.get(17)?,
                date_last_dispatched: row.get(18)?,
            })
        })?;

        Ok(kills.filter_map(|r| r.ok()).collect())
    }

    // === Trainers ===

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
                    apply_learning_ranks, apply_learning_unknown_count
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
        self.conn.execute(
            "INSERT INTO trainers (character_id, trainer_name, ranks, modified_ranks)
             VALUES (?1, ?2, 0, ?3)
             ON CONFLICT(character_id, trainer_name) DO UPDATE SET
                modified_ranks = excluded.modified_ranks",
            params![char_id, trainer_name, modified_ranks],
        )?;

        // Recalculate coin level from all trainer ranks (including apply-learning)
        let coin_level: i64 = self.conn.query_row(
            "SELECT COALESCE(SUM(ranks + modified_ranks + apply_learning_ranks), 0) FROM trainers WHERE character_id = ?1",
            params![char_id],
            |row| row.get(0),
        )?;
        self.update_coin_level(char_id, coin_level)?;

        Ok(())
    }

    // === Log files ===

    /// Check if a log file has already been scanned (by path or content hash).
    pub fn is_log_scanned(&self, file_path: &str) -> Result<bool> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM log_files WHERE file_path = ?1",
            params![file_path],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    /// Check if a content hash has already been scanned (catches duplicate files at different paths).
    pub fn is_hash_scanned(&self, content_hash: &str) -> Result<bool> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM log_files WHERE content_hash = ?1",
            params![content_hash],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    /// Mark a log file as scanned with its content hash.
    pub fn mark_log_scanned(
        &self,
        char_id: i64,
        file_path: &str,
        content_hash: &str,
        date_read: &str,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT OR IGNORE INTO log_files (character_id, file_path, content_hash, date_read)
             VALUES (?1, ?2, ?3, ?4)",
            params![char_id, file_path, content_hash, date_read],
        )?;
        Ok(())
    }

    /// Get count of scanned log files.
    pub fn scanned_log_count(&self) -> Result<i64> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM log_files",
            [],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    // === Pets ===

    /// Get pets for a character.
    pub fn get_pets(&self, char_id: i64) -> Result<Vec<Pet>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, character_id, pet_name, creature_name
             FROM pets WHERE character_id = ?1 ORDER BY pet_name",
        )?;

        let pets = stmt.query_map(params![char_id], |row| {
            Ok(Pet {
                id: Some(row.get(0)?),
                character_id: row.get(1)?,
                pet_name: row.get(2)?,
                creature_name: row.get(3)?,
            })
        })?;

        Ok(pets.filter_map(|r| r.ok()).collect())
    }

    /// Upsert a pet record. Uses creature_name as both pet_name and creature_name.
    pub fn upsert_pet(&self, char_id: i64, creature_name: &str) -> Result<()> {
        self.conn.execute(
            "INSERT OR IGNORE INTO pets (character_id, pet_name, creature_name)
             VALUES (?1, ?2, ?2)",
            params![char_id, creature_name],
        )?;
        Ok(())
    }

    // === Lastys ===

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

    // === Profession ===

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

    /// Set a character's start_date to the earlier of the existing value and the new value.
    pub fn update_start_date(&self, char_id: i64, date: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE characters SET start_date = ?1
             WHERE id = ?2 AND (start_date IS NULL OR start_date > ?1)",
            params![date, char_id],
        )?;
        Ok(())
    }

    // === Character Merging ===

    /// Get all character IDs that have been merged into the given target.
    fn merged_source_ids(&self, target_id: i64) -> Result<Vec<i64>> {
        let mut stmt = self.conn.prepare(
            "SELECT id FROM characters WHERE merged_into = ?1",
        )?;
        let ids = stmt
            .query_map(params![target_id], |row| row.get::<_, i64>(0))?
            .filter_map(|r| r.ok())
            .collect();
        Ok(ids)
    }

    /// Build a list containing the target character ID plus all merged source IDs.
    fn char_ids_for_merged(&self, char_id: i64) -> Result<Vec<i64>> {
        let mut ids = vec![char_id];
        ids.extend(self.merged_source_ids(char_id)?);
        Ok(ids)
    }

    /// Merge one or more source characters into a target character.
    /// Sets `merged_into = target_id` for each source. Recalculates target's profession and coin_level.
    /// Runs in a transaction for atomicity.
    pub fn merge_characters(&self, source_ids: &[i64], target_id: i64) -> Result<()> {
        self.begin_transaction()?;
        match self.merge_characters_inner(source_ids, target_id) {
            Ok(()) => { self.commit_transaction()?; Ok(()) }
            Err(e) => { let _ = self.rollback_transaction(); Err(e) }
        }
    }

    fn merge_characters_inner(&self, source_ids: &[i64], target_id: i64) -> Result<()> {
        // Validate target exists and is not itself merged
        let target_merged: Option<Option<i64>> = self.conn.query_row(
            "SELECT merged_into FROM characters WHERE id = ?1",
            params![target_id],
            |row| row.get(0),
        ).ok();
        let target_merged = target_merged.ok_or_else(|| {
            crate::error::AmanuensisError::Data(format!("Target character {} not found", target_id))
        })?;
        if target_merged.is_some() {
            return Err(crate::error::AmanuensisError::Data(
                "Target character is itself merged into another character".to_string(),
            ));
        }

        for &source_id in source_ids {
            if source_id == target_id {
                return Err(crate::error::AmanuensisError::Data(
                    "Cannot merge a character into itself".to_string(),
                ));
            }
            // Verify source exists and is not already merged
            let source_merged: Option<Option<i64>> = self.conn.query_row(
                "SELECT merged_into FROM characters WHERE id = ?1",
                params![source_id],
                |row| row.get(0),
            ).ok();
            let source_merged = source_merged.ok_or_else(|| {
                crate::error::AmanuensisError::Data(format!(
                    "Source character {} not found", source_id
                ))
            })?;
            if source_merged.is_some() {
                return Err(crate::error::AmanuensisError::Data(format!(
                    "Source character {} is already merged into another character", source_id
                )));
            }
            self.conn.execute(
                "UPDATE characters SET merged_into = ?1 WHERE id = ?2",
                params![target_id, source_id],
            )?;
        }

        // Recalculate target's aggregated coin_level and profession
        self.recalculate_merged_stats(target_id)?;

        Ok(())
    }

    /// Unmerge a character (clear its merged_into). Recalculates the former target's stats.
    /// Runs in a transaction for atomicity.
    pub fn unmerge_character(&self, source_id: i64) -> Result<()> {
        self.begin_transaction()?;
        match self.unmerge_character_inner(source_id) {
            Ok(()) => { self.commit_transaction()?; Ok(()) }
            Err(e) => { let _ = self.rollback_transaction(); Err(e) }
        }
    }

    fn unmerge_character_inner(&self, source_id: i64) -> Result<()> {
        let former_target: Option<i64> = self.conn.query_row(
            "SELECT merged_into FROM characters WHERE id = ?1",
            params![source_id],
            |row| row.get(0),
        ).map_err(|_| {
            crate::error::AmanuensisError::Data(format!("Character {} not found", source_id))
        })?;

        let former_target = former_target.ok_or_else(|| {
            crate::error::AmanuensisError::Data(format!("Character {} is not merged", source_id))
        })?;

        self.conn.execute(
            "UPDATE characters SET merged_into = NULL WHERE id = ?1",
            params![source_id],
        )?;

        // Recalculate the former target's stats
        self.recalculate_merged_stats(former_target)?;

        Ok(())
    }

    /// Get all characters that have been merged into the given target.
    pub fn get_merge_sources(&self, target_id: i64) -> Result<Vec<Character>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, profession, logins, departs, deaths, esteem, armor,
                    coins_picked_up, casino_won, casino_lost, chest_coins, bounty_coins,
                    fur_coins, mandible_coins, blood_coins,
                    bells_used, bells_broken, chains_used, chains_broken,
                    shieldstones_used, shieldstones_broken, ethereal_portals, darkstone, purgatory_pendant,
                    coin_level, good_karma, bad_karma, start_date,
                    fur_worth, mandible_worth, blood_worth, eps_broken, untraining_count
             FROM characters WHERE merged_into = ?1 ORDER BY name",
        )?;

        let chars = stmt.query_map(params![target_id], |row| {
            Ok(Character {
                id: Some(row.get(0)?),
                name: row.get(1)?,
                profession: Profession::parse(&row.get::<_, String>(2)?),
                logins: row.get(3)?,
                departs: row.get(4)?,
                deaths: row.get(5)?,
                esteem: row.get(6)?,
                armor: row.get(7)?,
                coins_picked_up: row.get(8)?,
                casino_won: row.get(9)?,
                casino_lost: row.get(10)?,
                chest_coins: row.get(11)?,
                bounty_coins: row.get(12)?,
                fur_coins: row.get(13)?,
                mandible_coins: row.get(14)?,
                blood_coins: row.get(15)?,
                bells_used: row.get(16)?,
                bells_broken: row.get(17)?,
                chains_used: row.get(18)?,
                chains_broken: row.get(19)?,
                shieldstones_used: row.get(20)?,
                shieldstones_broken: row.get(21)?,
                ethereal_portals: row.get(22)?,
                darkstone: row.get(23)?,
                purgatory_pendant: row.get(24)?,
                coin_level: row.get(25)?,
                good_karma: row.get(26)?,
                bad_karma: row.get(27)?,
                start_date: row.get(28)?,
                fur_worth: row.get(29)?,
                mandible_worth: row.get(30)?,
                blood_worth: row.get(31)?,
                eps_broken: row.get(32)?,
                untraining_count: row.get(33)?,
            })
        })?;

        Ok(chars.filter_map(|r| r.ok()).collect())
    }

    /// Recalculate a target character's coin_level after merge/unmerge.
    fn recalculate_merged_stats(&self, target_id: i64) -> Result<()> {
        let all_ids = self.char_ids_for_merged(target_id)?;
        let placeholders = all_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");

        // Recalculate coin level from merged trainers
        let sql = format!(
            "SELECT COALESCE(SUM(ranks + modified_ranks + apply_learning_ranks), 0) FROM trainers WHERE character_id IN ({})",
            placeholders
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let coin_level: i64 = stmt.query_row(
            rusqlite::params_from_iter(all_ids.iter()),
            |row| row.get(0),
        )?;
        self.update_coin_level(target_id, coin_level)?;

        Ok(())
    }

    // === Merged Aggregation Queries ===

    /// Get kills aggregated across a character and all its merge sources.
    /// For the same creature, counts are summed; dates take min(first) and max(last).
    pub fn get_kills_merged(&self, char_id: i64) -> Result<Vec<Kill>> {
        let all_ids = self.char_ids_for_merged(char_id)?;
        if all_ids.len() == 1 {
            return self.get_kills(char_id);
        }
        let placeholders = all_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let sql = format!(
            "SELECT NULL, {}, creature_name,
                    SUM(killed_count), SUM(slaughtered_count), SUM(vanquished_count), SUM(dispatched_count),
                    SUM(assisted_kill_count), SUM(assisted_slaughter_count), SUM(assisted_vanquish_count), SUM(assisted_dispatch_count),
                    SUM(killed_by_count), MIN(date_first), MAX(date_last), MAX(creature_value),
                    MAX(date_last_killed), MAX(date_last_slaughtered), MAX(date_last_vanquished), MAX(date_last_dispatched)
             FROM kills WHERE character_id IN ({})
             GROUP BY creature_name
             ORDER BY (SUM(killed_count) + SUM(slaughtered_count) + SUM(vanquished_count) + SUM(dispatched_count) +
                       SUM(assisted_kill_count) + SUM(assisted_slaughter_count) + SUM(assisted_vanquish_count) + SUM(assisted_dispatch_count)) DESC",
            char_id, placeholders
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let kills = stmt.query_map(rusqlite::params_from_iter(all_ids.iter()), |row| {
            Ok(Kill {
                id: row.get(0)?,
                character_id: row.get(1)?,
                creature_name: row.get(2)?,
                killed_count: row.get(3)?,
                slaughtered_count: row.get(4)?,
                vanquished_count: row.get(5)?,
                dispatched_count: row.get(6)?,
                assisted_kill_count: row.get(7)?,
                assisted_slaughter_count: row.get(8)?,
                assisted_vanquish_count: row.get(9)?,
                assisted_dispatch_count: row.get(10)?,
                killed_by_count: row.get(11)?,
                date_first: row.get(12)?,
                date_last: row.get(13)?,
                creature_value: row.get(14)?,
                date_last_killed: row.get(15)?,
                date_last_slaughtered: row.get(16)?,
                date_last_vanquished: row.get(17)?,
                date_last_dispatched: row.get(18)?,
            })
        })?;
        Ok(kills.filter_map(|r| r.ok()).collect())
    }

    /// Get trainers aggregated across a character and all its merge sources.
    /// For the same trainer name: sum ranks, take max date.
    pub fn get_trainers_merged(&self, char_id: i64) -> Result<Vec<Trainer>> {
        let all_ids = self.char_ids_for_merged(char_id)?;
        if all_ids.len() == 1 {
            return self.get_trainers(char_id);
        }
        let placeholders = all_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let sql = format!(
            "SELECT NULL, {}, trainer_name,
                    SUM(ranks), SUM(modified_ranks), MAX(date_of_last_rank),
                    SUM(apply_learning_ranks), SUM(apply_learning_unknown_count)
             FROM trainers WHERE character_id IN ({})
             GROUP BY trainer_name
             ORDER BY SUM(ranks) DESC",
            char_id, placeholders
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let trainers = stmt.query_map(rusqlite::params_from_iter(all_ids.iter()), |row| {
            Ok(Trainer {
                id: row.get(0)?,
                character_id: row.get(1)?,
                trainer_name: row.get(2)?,
                ranks: row.get(3)?,
                modified_ranks: row.get(4)?,
                date_of_last_rank: row.get(5)?,
                apply_learning_ranks: row.get(6)?,
                apply_learning_unknown_count: row.get(7)?,
            })
        })?;
        Ok(trainers.filter_map(|r| r.ok()).collect())
    }

    /// Get pets aggregated across a character and all its merge sources (distinct by pet_name).
    pub fn get_pets_merged(&self, char_id: i64) -> Result<Vec<Pet>> {
        let all_ids = self.char_ids_for_merged(char_id)?;
        if all_ids.len() == 1 {
            return self.get_pets(char_id);
        }
        let placeholders = all_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let sql = format!(
            "SELECT MIN(id), {}, pet_name, creature_name
             FROM pets WHERE character_id IN ({})
             GROUP BY pet_name
             ORDER BY pet_name",
            char_id, placeholders
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let pets = stmt.query_map(rusqlite::params_from_iter(all_ids.iter()), |row| {
            Ok(Pet {
                id: Some(row.get(0)?),
                character_id: row.get(1)?,
                pet_name: row.get(2)?,
                creature_name: row.get(3)?,
            })
        })?;
        Ok(pets.filter_map(|r| r.ok()).collect())
    }

    /// Get lastys aggregated across a character and all its merge sources.
    /// For the same creature: keep the one with higher message_count, prefer finished=1.
    pub fn get_lastys_merged(&self, char_id: i64) -> Result<Vec<Lasty>> {
        let all_ids = self.char_ids_for_merged(char_id)?;
        if all_ids.len() == 1 {
            return self.get_lastys(char_id);
        }
        let placeholders = all_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let sql = format!(
            "SELECT MIN(id), {}, creature_name, lasty_type,
                    MAX(finished), SUM(message_count),
                    MIN(first_seen_date), MAX(last_seen_date),
                    MAX(completed_date), MAX(abandoned_date)
             FROM lastys WHERE character_id IN ({})
             GROUP BY creature_name
             ORDER BY creature_name",
            char_id, placeholders
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let lastys = stmt.query_map(rusqlite::params_from_iter(all_ids.iter()), |row| {
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

    /// Get a character with aggregated stats from all its merge sources.
    /// Sums numeric fields, takes MIN start_date.
    pub fn get_character_merged(&self, char_id: i64) -> Result<Option<Character>> {
        let source_ids = self.merged_source_ids(char_id)?;
        if source_ids.is_empty() {
            return self.get_character_by_id(char_id);
        }

        // Get the target character as a base
        let target = match self.get_character_by_id(char_id)? {
            Some(c) => c,
            None => return Ok(None),
        };

        // Get all source characters and sum their stats
        let mut merged = target;
        for &sid in &source_ids {
            if let Some(source) = self.get_character_by_id(sid)? {
                merged.logins += source.logins;
                merged.departs += source.departs;
                merged.deaths += source.deaths;
                merged.esteem += source.esteem;
                merged.coins_picked_up += source.coins_picked_up;
                merged.casino_won += source.casino_won;
                merged.casino_lost += source.casino_lost;
                merged.chest_coins += source.chest_coins;
                merged.bounty_coins += source.bounty_coins;
                merged.fur_coins += source.fur_coins;
                merged.mandible_coins += source.mandible_coins;
                merged.blood_coins += source.blood_coins;
                merged.bells_used += source.bells_used;
                merged.bells_broken += source.bells_broken;
                merged.chains_used += source.chains_used;
                merged.chains_broken += source.chains_broken;
                merged.shieldstones_used += source.shieldstones_used;
                merged.shieldstones_broken += source.shieldstones_broken;
                merged.ethereal_portals += source.ethereal_portals;
                merged.darkstone += source.darkstone;
                merged.purgatory_pendant += source.purgatory_pendant;
                merged.good_karma += source.good_karma;
                merged.bad_karma += source.bad_karma;
                merged.fur_worth += source.fur_worth;
                merged.mandible_worth += source.mandible_worth;
                merged.blood_worth += source.blood_worth;
                merged.eps_broken += source.eps_broken;
                merged.untraining_count += source.untraining_count;
                // Take earlier start_date
                if let Some(ref source_date) = source.start_date {
                    if merged.start_date.is_none() || merged.start_date.as_ref().unwrap() > source_date {
                        merged.start_date = Some(source_date.clone());
                    }
                }
            }
        }

        // Coin level is from the merged trainer totals (already set in recalculate_merged_stats)
        // but recompute here for accuracy
        let all_ids = self.char_ids_for_merged(char_id)?;
        let placeholders = all_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let sql = format!(
            "SELECT COALESCE(SUM(ranks + modified_ranks + apply_learning_ranks), 0) FROM trainers WHERE character_id IN ({})",
            placeholders
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let coin_level: i64 = stmt.query_row(
            rusqlite::params_from_iter(all_ids.iter()),
            |row| row.get(0),
        )?;
        merged.coin_level = coin_level;

        Ok(Some(merged))
    }

    /// Get a character by ID (internal helper).
    pub fn get_character_by_id(&self, char_id: i64) -> Result<Option<Character>> {
        let result = self.conn.query_row(
            "SELECT id, name, profession, logins, departs, deaths, esteem, armor,
                    coins_picked_up, casino_won, casino_lost, chest_coins, bounty_coins,
                    fur_coins, mandible_coins, blood_coins,
                    bells_used, bells_broken, chains_used, chains_broken,
                    shieldstones_used, shieldstones_broken, ethereal_portals, darkstone, purgatory_pendant,
                    coin_level, good_karma, bad_karma, start_date,
                    fur_worth, mandible_worth, blood_worth, eps_broken, untraining_count
             FROM characters WHERE id = ?1",
            params![char_id],
            |row| {
                Ok(Character {
                    id: Some(row.get(0)?),
                    name: row.get(1)?,
                    profession: Profession::parse(&row.get::<_, String>(2)?),
                    logins: row.get(3)?,
                    departs: row.get(4)?,
                    deaths: row.get(5)?,
                    esteem: row.get(6)?,
                    armor: row.get(7)?,
                    coins_picked_up: row.get(8)?,
                    casino_won: row.get(9)?,
                    casino_lost: row.get(10)?,
                    chest_coins: row.get(11)?,
                    bounty_coins: row.get(12)?,
                    fur_coins: row.get(13)?,
                    mandible_coins: row.get(14)?,
                    blood_coins: row.get(15)?,
                    bells_used: row.get(16)?,
                    bells_broken: row.get(17)?,
                    chains_used: row.get(18)?,
                    chains_broken: row.get(19)?,
                    shieldstones_used: row.get(20)?,
                    shieldstones_broken: row.get(21)?,
                    ethereal_portals: row.get(22)?,
                    darkstone: row.get(23)?,
                    purgatory_pendant: row.get(24)?,
                    coin_level: row.get(25)?,
                    good_karma: row.get(26)?,
                    bad_karma: row.get(27)?,
                    start_date: row.get(28)?,
                    fur_worth: row.get(29)?,
                    mandible_worth: row.get(30)?,
                    blood_worth: row.get(31)?,
                    eps_broken: row.get(32)?,
                    untraining_count: row.get(33)?,
                })
            },
        );

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
        let result = self.conn.query_row(
            "SELECT id, name, profession, logins, departs, deaths, esteem, armor,
                    coins_picked_up, casino_won, casino_lost, chest_coins, bounty_coins,
                    fur_coins, mandible_coins, blood_coins,
                    bells_used, bells_broken, chains_used, chains_broken,
                    shieldstones_used, shieldstones_broken, ethereal_portals, darkstone, purgatory_pendant,
                    coin_level, good_karma, bad_karma, start_date,
                    fur_worth, mandible_worth, blood_worth, eps_broken, untraining_count
             FROM characters WHERE name = ?1",
            params![name],
            |row| {
                Ok(Character {
                    id: Some(row.get(0)?),
                    name: row.get(1)?,
                    profession: Profession::parse(&row.get::<_, String>(2)?),
                    logins: row.get(3)?,
                    departs: row.get(4)?,
                    deaths: row.get(5)?,
                    esteem: row.get(6)?,
                    armor: row.get(7)?,
                    coins_picked_up: row.get(8)?,
                    casino_won: row.get(9)?,
                    casino_lost: row.get(10)?,
                    chest_coins: row.get(11)?,
                    bounty_coins: row.get(12)?,
                    fur_coins: row.get(13)?,
                    mandible_coins: row.get(14)?,
                    blood_coins: row.get(15)?,
                    bells_used: row.get(16)?,
                    bells_broken: row.get(17)?,
                    chains_used: row.get(18)?,
                    chains_broken: row.get(19)?,
                    shieldstones_used: row.get(20)?,
                    shieldstones_broken: row.get(21)?,
                    ethereal_portals: row.get(22)?,
                    darkstone: row.get(23)?,
                    purgatory_pendant: row.get(24)?,
                    coin_level: row.get(25)?,
                    good_karma: row.get(26)?,
                    bad_karma: row.get(27)?,
                    start_date: row.get(28)?,
                    fur_worth: row.get(29)?,
                    mandible_worth: row.get(30)?,
                    blood_worth: row.get(31)?,
                    eps_broken: row.get(32)?,
                    untraining_count: row.get(33)?,
                })
            },
        );

        match result {
            Ok(c) => Ok(Some(c)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Get the highest-value killed creature for a character.
    /// Returns (creature_name, total_solo_kills * creature_value).
    pub fn get_highest_kill(&self, char_id: i64) -> Result<Option<(String, i64)>> {
        let result = self.conn.query_row(
            "SELECT creature_name,
                    (killed_count + slaughtered_count + vanquished_count + dispatched_count) * creature_value AS score
             FROM kills WHERE character_id = ?1 AND creature_value > 0
             ORDER BY score DESC LIMIT 1",
            params![char_id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
        );
        match result {
            Ok(r) => Ok(Some(r)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Get the nemesis (creature that killed the character the most).
    /// Returns (creature_name, killed_by_count).
    pub fn get_nemesis(&self, char_id: i64) -> Result<Option<(String, i64)>> {
        let result = self.conn.query_row(
            "SELECT creature_name, killed_by_count
             FROM kills WHERE character_id = ?1 AND killed_by_count > 0
             ORDER BY killed_by_count DESC LIMIT 1",
            params![char_id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
        );
        match result {
            Ok(r) => Ok(Some(r)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    // === Log Lines (FTS5 full-text search) ===

    /// Batch-insert log lines into the FTS5 table.
    /// Each tuple is (character_id, content, timestamp, file_path).
    pub fn insert_log_lines(&self, lines: &[(i64, &str, &str, &str)]) -> Result<()> {
        let mut stmt = self.conn.prepare_cached(
            "INSERT INTO log_lines (content, character_id, timestamp, file_path)
             VALUES (?1, ?2, ?3, ?4)",
        )?;
        for &(char_id, content, timestamp, file_path) in lines {
            stmt.execute(params![content, char_id, timestamp, file_path])?;
        }
        Ok(())
    }

    /// Search log lines using FTS5 full-text search.
    /// Returns results with highlighted snippets.
    pub fn search_log_lines(
        &self,
        query: &str,
        char_id: Option<i64>,
        limit: i64,
    ) -> Result<Vec<LogSearchResult>> {
        // Escape double quotes in the query and wrap for literal matching
        let escaped = query.replace('"', "\"\"");
        let fts_query = format!("\"{}\"", escaped);

        let row_mapper = |row: &rusqlite::Row| -> rusqlite::Result<LogSearchResult> {
            // character_id may be stored as integer or text depending on how it was inserted
            let character_id: i64 = row.get::<_, i64>(1).or_else(|_| {
                row.get::<_, String>(1).map(|s| s.parse().unwrap_or(0))
            })?;
            Ok(LogSearchResult {
                content: row.get(0)?,
                character_id,
                timestamp: row.get(2)?,
                file_path: row.get(3)?,
                snippet: row.get(4)?,
                character_name: row.get(5)?,
            })
        };

        if let Some(cid) = char_id {
            let mut stmt = self.conn.prepare(
                "SELECT l.content, l.character_id, l.timestamp, l.file_path,
                        snippet(log_lines, 0, '<mark>', '</mark>', '...', 64) AS snippet,
                        COALESCE(c.name, 'Unknown') AS character_name
                 FROM log_lines l
                 LEFT JOIN characters c ON CAST(l.character_id AS INTEGER) = c.id
                 WHERE log_lines MATCH ?1 AND CAST(l.character_id AS INTEGER) = ?2
                 ORDER BY rank
                 LIMIT ?3",
            )?;
            let mut results = Vec::new();
            for row in stmt.query_map(params![fts_query, cid, limit], row_mapper)? {
                match row {
                    Ok(r) => results.push(r),
                    Err(e) => log::warn!("FTS5 row error: {}", e),
                }
            }
            Ok(results)
        } else {
            let mut stmt = self.conn.prepare(
                "SELECT l.content, l.character_id, l.timestamp, l.file_path,
                        snippet(log_lines, 0, '<mark>', '</mark>', '...', 64) AS snippet,
                        COALESCE(c.name, 'Unknown') AS character_name
                 FROM log_lines l
                 LEFT JOIN characters c ON CAST(l.character_id AS INTEGER) = c.id
                 WHERE log_lines MATCH ?1
                 ORDER BY rank
                 LIMIT ?2",
            )?;
            let mut results = Vec::new();
            for row in stmt.query_map(params![fts_query, limit], row_mapper)? {
                match row {
                    Ok(r) => results.push(r),
                    Err(e) => log::warn!("FTS5 row error: {}", e),
                }
            }
            Ok(results)
        }
    }

    /// Get the total number of indexed log lines.
    pub fn log_line_count(&self) -> Result<i64> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM log_lines",
            [],
            |row| row.get(0),
        )?;
        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_or_create_character() {
        let db = Database::open_in_memory().unwrap();
        let id1 = db.get_or_create_character("Fen").unwrap();
        let id2 = db.get_or_create_character("Fen").unwrap();
        assert_eq!(id1, id2, "Same name should return same ID");

        let id3 = db.get_or_create_character("pip").unwrap();
        assert_ne!(id1, id3, "Different names should return different IDs");
    }

    #[test]
    fn test_get_character() {
        let db = Database::open_in_memory().unwrap();
        db.get_or_create_character("Fen").unwrap();
        let char = db.get_character("Fen").unwrap().unwrap();
        assert_eq!(char.name, "Fen");
        assert_eq!(char.profession, Profession::Unknown);
        assert_eq!(char.logins, 0);
    }

    #[test]
    fn test_increment_character_field() {
        let db = Database::open_in_memory().unwrap();
        let id = db.get_or_create_character("Fen").unwrap();
        db.increment_character_field(id, "logins", 1).unwrap();
        db.increment_character_field(id, "logins", 1).unwrap();
        db.increment_character_field(id, "deaths", 3).unwrap();
        let char = db.get_character("Fen").unwrap().unwrap();
        assert_eq!(char.logins, 2);
        assert_eq!(char.deaths, 3);
    }

    #[test]
    fn test_increment_invalid_field_rejected() {
        let db = Database::open_in_memory().unwrap();
        let id = db.get_or_create_character("Fen").unwrap();
        let result = db.increment_character_field(id, "name; DROP TABLE characters;--", 1);
        assert!(result.is_err());
    }

    #[test]
    fn test_upsert_kill() {
        let db = Database::open_in_memory().unwrap();
        let id = db.get_or_create_character("Fen").unwrap();
        db.upsert_kill(id, "Rat", "slaughtered_count", 2, "2024-01-01")
            .unwrap();
        db.upsert_kill(id, "Rat", "slaughtered_count", 2, "2024-01-02")
            .unwrap();
        db.upsert_kill(id, "Rat", "killed_count", 2, "2024-01-03")
            .unwrap();

        let kills = db.get_kills(id).unwrap();
        assert_eq!(kills.len(), 1);
        assert_eq!(kills[0].creature_name, "Rat");
        assert_eq!(kills[0].slaughtered_count, 2);
        assert_eq!(kills[0].killed_count, 1);
        assert_eq!(kills[0].date_first, Some("2024-01-01".to_string()));
        assert_eq!(kills[0].date_last, Some("2024-01-03".to_string()));
    }

    #[test]
    fn test_upsert_trainer_rank() {
        let db = Database::open_in_memory().unwrap();
        let id = db.get_or_create_character("Fen").unwrap();
        db.upsert_trainer_rank(id, "Bangus Anmash", "2024-01-01")
            .unwrap();
        db.upsert_trainer_rank(id, "Bangus Anmash", "2024-01-02")
            .unwrap();
        db.upsert_trainer_rank(id, "Regia", "2024-01-03").unwrap();

        let trainers = db.get_trainers(id).unwrap();
        assert_eq!(trainers.len(), 2);
        // Bangus should be first (2 ranks)
        assert_eq!(trainers[0].trainer_name, "Bangus Anmash");
        assert_eq!(trainers[0].ranks, 2);
        assert_eq!(trainers[1].trainer_name, "Regia");
        assert_eq!(trainers[1].ranks, 1);
    }

    #[test]
    fn test_log_scanning() {
        let db = Database::open_in_memory().unwrap();
        let id = db.get_or_create_character("Fen").unwrap();
        assert!(!db.is_log_scanned("/logs/test.txt").unwrap());
        db.mark_log_scanned(id, "/logs/test.txt", "abc123hash", "2024-01-01")
            .unwrap();
        assert!(db.is_log_scanned("/logs/test.txt").unwrap());
        assert_eq!(db.scanned_log_count().unwrap(), 1);
    }

    #[test]
    fn test_hash_dedup() {
        let db = Database::open_in_memory().unwrap();
        let id = db.get_or_create_character("Fen").unwrap();
        let hash = "deadbeef12345678";
        assert!(!db.is_hash_scanned(hash).unwrap());
        db.mark_log_scanned(id, "/logs/a.txt", hash, "2024-01-01")
            .unwrap();
        assert!(db.is_hash_scanned(hash).unwrap());
        // Same hash at different path should be detected as duplicate
        assert!(!db.is_log_scanned("/logs/b.txt").unwrap());
        assert!(db.is_hash_scanned(hash).unwrap());
    }

    #[test]
    fn test_list_characters() {
        let db = Database::open_in_memory().unwrap();
        db.get_or_create_character("Fen").unwrap();
        db.get_or_create_character("pip").unwrap();
        let chars = db.list_characters().unwrap();
        assert_eq!(chars.len(), 2);
    }

    #[test]
    fn test_coin_tracking() {
        let db = Database::open_in_memory().unwrap();
        let id = db.get_or_create_character("Fen").unwrap();
        db.increment_character_field(id, "coins_picked_up", 50).unwrap();
        db.increment_character_field(id, "fur_coins", 10).unwrap();
        db.increment_character_field(id, "blood_coins", 15).unwrap();
        let char = db.get_character("Fen").unwrap().unwrap();
        assert_eq!(char.coins_picked_up, 50);
        assert_eq!(char.fur_coins, 10);
        assert_eq!(char.blood_coins, 15);
    }

    #[test]
    fn test_upsert_pet() {
        let db = Database::open_in_memory().unwrap();
        let id = db.get_or_create_character("Fen").unwrap();
        db.upsert_pet(id, "Maha Ruknee").unwrap();
        db.upsert_pet(id, "Maha Ruknee").unwrap(); // duplicate should be ignored
        let pets = db.get_pets(id).unwrap();
        assert_eq!(pets.len(), 1);
        assert_eq!(pets[0].creature_name, "Maha Ruknee");
        assert_eq!(pets[0].pet_name, "Maha Ruknee");
    }

    #[test]
    fn test_upsert_lasty() {
        let db = Database::open_in_memory().unwrap();
        let id = db.get_or_create_character("Fen").unwrap();
        db.upsert_lasty(id, "Maha Ruknee", "Befriend", "2024-01-01").unwrap();
        db.upsert_lasty(id, "Maha Ruknee", "Befriend", "2024-01-02").unwrap();
        db.upsert_lasty(id, "Orga Anger", "Morph", "2024-01-03").unwrap();

        let lastys = db.get_lastys(id).unwrap();
        assert_eq!(lastys.len(), 2);

        let maha = lastys.iter().find(|l| l.creature_name == "Maha Ruknee").unwrap();
        assert_eq!(maha.lasty_type, "Befriend");
        assert_eq!(maha.message_count, 2);
        assert!(!maha.finished);
        assert_eq!(maha.first_seen_date, Some("2024-01-01".to_string()));
        assert_eq!(maha.last_seen_date, Some("2024-01-02".to_string()));

        let orga = lastys.iter().find(|l| l.creature_name == "Orga Anger").unwrap();
        assert_eq!(orga.lasty_type, "Morph");
        assert_eq!(orga.message_count, 1);
    }

    #[test]
    fn test_complete_lasty() {
        let db = Database::open_in_memory().unwrap();
        let id = db.get_or_create_character("Fen").unwrap();
        db.upsert_lasty(id, "Maha Ruknee", "Befriend", "2024-01-01").unwrap();
        db.complete_lasty(id, "Sespus").unwrap();

        let lastys = db.get_lastys(id).unwrap();
        assert_eq!(lastys.len(), 1);
        assert!(lastys[0].finished);
    }

    #[test]
    fn test_finish_lasty() {
        let db = Database::open_in_memory().unwrap();
        let id = db.get_or_create_character("Fen").unwrap();
        db.upsert_lasty(id, "Maha Ruknee", "Befriend", "2024-01-01").unwrap();
        db.finish_lasty(id, "Maha Ruknee", "Befriend", "2024-01-05").unwrap();

        let lastys = db.get_lastys(id).unwrap();
        assert_eq!(lastys.len(), 1);
        assert!(lastys[0].finished);
        assert_eq!(lastys[0].message_count, 2);
        assert_eq!(lastys[0].completed_date, Some("2024-01-05".to_string()));
        assert_eq!(lastys[0].first_seen_date, Some("2024-01-01".to_string()));
    }

    #[test]
    fn test_finish_lasty_new_creature() {
        let db = Database::open_in_memory().unwrap();
        let id = db.get_or_create_character("Fen").unwrap();
        // finish_lasty on a creature with no prior record should still work
        db.finish_lasty(id, "Rat", "Movements", "2024-01-01").unwrap();

        let lastys = db.get_lastys(id).unwrap();
        assert_eq!(lastys.len(), 1);
        assert!(lastys[0].finished);
        assert_eq!(lastys[0].message_count, 1);
        assert_eq!(lastys[0].completed_date, Some("2024-01-01".to_string()));
    }

    #[test]
    fn test_abandon_lasty() {
        let db = Database::open_in_memory().unwrap();
        let id = db.get_or_create_character("Fen").unwrap();
        db.upsert_lasty(id, "Maha Ruknee", "Befriend", "2024-01-01").unwrap();
        db.abandon_lasty(id, "Maha Ruknee", "2024-01-02").unwrap();

        let lastys = db.get_lastys(id).unwrap();
        assert_eq!(lastys[0].abandoned_date, Some("2024-01-02".to_string()));

        // Clear abandon
        db.clear_lasty_abandon(id, "Maha Ruknee").unwrap();
        let lastys = db.get_lastys(id).unwrap();
        assert_eq!(lastys[0].abandoned_date, None);
    }

    #[test]
    fn test_update_profession() {
        let db = Database::open_in_memory().unwrap();
        let id = db.get_or_create_character("Fen").unwrap();
        db.update_character_profession(id, "Ranger").unwrap();
        let char = db.get_character("Fen").unwrap().unwrap();
        assert_eq!(char.profession, Profession::Ranger);
    }

    #[test]
    fn test_update_coin_level() {
        let db = Database::open_in_memory().unwrap();
        let id = db.get_or_create_character("Fen").unwrap();
        db.update_coin_level(id, 42).unwrap();
        let char = db.get_character("Fen").unwrap().unwrap();
        assert_eq!(char.coin_level, 42);
    }

    #[test]
    fn test_merge_characters() {
        let db = Database::open_in_memory().unwrap();
        let id_a = db.get_or_create_character("CharA").unwrap();
        let id_b = db.get_or_create_character("CharB").unwrap();

        // Add some data to both
        db.increment_character_field(id_a, "logins", 10).unwrap();
        db.increment_character_field(id_b, "logins", 5).unwrap();
        db.increment_character_field(id_a, "deaths", 2).unwrap();
        db.increment_character_field(id_b, "deaths", 3).unwrap();
        db.upsert_kill(id_a, "Rat", "killed_count", 2, "2024-01-01").unwrap();
        db.upsert_kill(id_b, "Rat", "killed_count", 2, "2024-01-05").unwrap();
        db.upsert_kill(id_b, "Wolf", "killed_count", 5, "2024-01-03").unwrap();
        db.upsert_trainer_rank(id_a, "Histia", "2024-01-01").unwrap();
        db.upsert_trainer_rank(id_a, "Histia", "2024-01-02").unwrap();
        db.upsert_trainer_rank(id_b, "Histia", "2024-01-03").unwrap();
        db.upsert_trainer_rank(id_b, "Regia", "2024-01-04").unwrap();
        db.upsert_pet(id_a, "Cat").unwrap();
        db.upsert_pet(id_b, "Cat").unwrap(); // duplicate pet
        db.upsert_pet(id_b, "Dog").unwrap();
        db.upsert_lasty(id_a, "Maha Ruknee", "Befriend", "2024-01-01").unwrap();
        db.upsert_lasty(id_b, "Maha Ruknee", "Befriend", "2024-01-05").unwrap();
        db.upsert_lasty(id_b, "Orga Anger", "Morph", "2024-01-03").unwrap();

        // Merge B into A
        db.merge_characters(&[id_b], id_a).unwrap();

        // B should be hidden from list
        let chars = db.list_characters().unwrap();
        assert_eq!(chars.len(), 1);
        assert_eq!(chars[0].name, "CharA");

        // Merge sources should return B
        let sources = db.get_merge_sources(id_a).unwrap();
        assert_eq!(sources.len(), 1);
        assert_eq!(sources[0].name, "CharB");

        // Merged character should have aggregated stats
        let merged = db.get_character_merged(id_a).unwrap().unwrap();
        assert_eq!(merged.logins, 15); // 10 + 5
        assert_eq!(merged.deaths, 5); // 2 + 3

        // Merged kills should combine
        let kills = db.get_kills_merged(id_a).unwrap();
        assert_eq!(kills.len(), 2); // Rat (combined) + Wolf
        let rat = kills.iter().find(|k| k.creature_name == "Rat").unwrap();
        assert_eq!(rat.killed_count, 2); // 1 + 1

        // Merged trainers should combine
        let trainers = db.get_trainers_merged(id_a).unwrap();
        let histia = trainers.iter().find(|t| t.trainer_name == "Histia").unwrap();
        assert_eq!(histia.ranks, 3); // 2 + 1
        let regia = trainers.iter().find(|t| t.trainer_name == "Regia").unwrap();
        assert_eq!(regia.ranks, 1);

        // Merged pets should be distinct
        let pets = db.get_pets_merged(id_a).unwrap();
        assert_eq!(pets.len(), 2); // Cat + Dog

        // Merged lastys should combine
        let lastys = db.get_lastys_merged(id_a).unwrap();
        assert_eq!(lastys.len(), 2); // Maha Ruknee + Orga Anger
        let maha = lastys.iter().find(|l| l.creature_name == "Maha Ruknee").unwrap();
        assert_eq!(maha.message_count, 2); // 1 + 1
    }

    #[test]
    fn test_unmerge_character() {
        let db = Database::open_in_memory().unwrap();
        let id_a = db.get_or_create_character("CharA").unwrap();
        let id_b = db.get_or_create_character("CharB").unwrap();
        db.increment_character_field(id_a, "logins", 10).unwrap();
        db.increment_character_field(id_b, "logins", 5).unwrap();

        // Merge then unmerge
        db.merge_characters(&[id_b], id_a).unwrap();
        assert_eq!(db.list_characters().unwrap().len(), 1);

        db.unmerge_character(id_b).unwrap();
        assert_eq!(db.list_characters().unwrap().len(), 2);

        // Merged stats should revert to original
        let char_a = db.get_character_merged(id_a).unwrap().unwrap();
        assert_eq!(char_a.logins, 10); // back to original
    }

    #[test]
    fn test_merge_validation() {
        let db = Database::open_in_memory().unwrap();
        let id_a = db.get_or_create_character("CharA").unwrap();

        // Cannot merge into self
        assert!(db.merge_characters(&[id_a], id_a).is_err());

        // Cannot merge nonexistent character
        assert!(db.merge_characters(&[9999], id_a).is_err());

        // Cannot merge into nonexistent target
        assert!(db.merge_characters(&[id_a], 9999).is_err());
    }

    #[test]
    fn test_get_character_by_id() {
        let db = Database::open_in_memory().unwrap();
        let id = db.get_or_create_character("Fen").unwrap();
        let char = db.get_character_by_id(id).unwrap().unwrap();
        assert_eq!(char.name, "Fen");
        assert!(db.get_character_by_id(9999).unwrap().is_none());
    }

    #[test]
    fn test_get_merged_into_name() {
        let db = Database::open_in_memory().unwrap();
        let id_a = db.get_or_create_character("CharA").unwrap();
        let id_b = db.get_or_create_character("CharB").unwrap();

        // Not merged — should return None
        assert!(db.get_merged_into_name(id_b).unwrap().is_none());

        // Merge B into A
        db.merge_characters(&[id_b], id_a).unwrap();

        // B is merged into A — should return "CharA"
        assert_eq!(db.get_merged_into_name(id_b).unwrap(), Some("CharA".to_string()));

        // A is not merged — should return None
        assert!(db.get_merged_into_name(id_a).unwrap().is_none());

        // Nonexistent ID — should return None
        assert!(db.get_merged_into_name(9999).unwrap().is_none());
    }

    #[test]
    fn test_get_character_including_merged() {
        let db = Database::open_in_memory().unwrap();
        let id_a = db.get_or_create_character("CharA").unwrap();
        let id_b = db.get_or_create_character("CharB").unwrap();

        // Merge B into A
        db.merge_characters(&[id_b], id_a).unwrap();

        // list_characters should NOT return CharB
        let chars = db.list_characters().unwrap();
        assert_eq!(chars.len(), 1);
        assert_eq!(chars[0].name, "CharA");

        // get_character_including_merged SHOULD still find CharB
        let found = db.get_character_including_merged("CharB").unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().name, "CharB");

        // Also finds non-merged characters
        let found_a = db.get_character_including_merged("CharA").unwrap();
        assert!(found_a.is_some());

        // Nonexistent returns None
        assert!(db.get_character_including_merged("Nobody").unwrap().is_none());
    }

    #[test]
    fn test_merge_already_merged_source_rejected() {
        let db = Database::open_in_memory().unwrap();
        let id_a = db.get_or_create_character("CharA").unwrap();
        let id_b = db.get_or_create_character("CharB").unwrap();
        let id_c = db.get_or_create_character("CharC").unwrap();

        // Merge B into A
        db.merge_characters(&[id_b], id_a).unwrap();

        // Trying to merge B into C should fail — B is already merged
        let result = db.merge_characters(&[id_b], id_c);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("already merged"));
    }

    #[test]
    fn test_fts5_insert_and_search() {
        let db = Database::open_in_memory().unwrap();
        let id = db.get_or_create_character("Fen").unwrap();

        // Insert some log lines
        let lines = vec![
            (id, "You slaughtered a Rat.", "2024-01-01 13:00:00", "/logs/test.txt"),
            (id, "You helped vanquish a Large Vermine.", "2024-01-01 13:01:00", "/logs/test.txt"),
            (id, "Welcome to Clan Lord, Fen!", "2024-01-01 13:00:00", "/logs/test.txt"),
        ];
        db.insert_log_lines(&lines).unwrap();

        assert_eq!(db.log_line_count().unwrap(), 3);

        // Search all
        let results = db.search_log_lines("Rat", None, 10).unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0].snippet.contains("<mark>"));
        assert_eq!(results[0].character_name, "Fen");

        // Search with character filter
        let results = db.search_log_lines("Rat", Some(id), 10).unwrap();
        assert_eq!(results.len(), 1);

        // Search with wrong character
        let id2 = db.get_or_create_character("Pip").unwrap();
        let results = db.search_log_lines("Rat", Some(id2), 10).unwrap();
        assert_eq!(results.len(), 0);

        // Search no match
        let results = db.search_log_lines("Dragon", None, 10).unwrap();
        assert_eq!(results.len(), 0);
    }
}
