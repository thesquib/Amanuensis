use rusqlite::{params, Connection};

use crate::error::Result;
use crate::models::*;

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
                    fur_worth, mandible_worth, blood_worth, eps_broken
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
                })
            },
        );

        match result {
            Ok(c) => Ok(Some(c)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// List all characters.
    pub fn list_characters(&self) -> Result<Vec<Character>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, profession, logins, departs, deaths, esteem, armor,
                    coins_picked_up, casino_won, casino_lost, chest_coins, bounty_coins,
                    fur_coins, mandible_coins, blood_coins,
                    bells_used, bells_broken, chains_used, chains_broken,
                    shieldstones_used, shieldstones_broken, ethereal_portals, darkstone, purgatory_pendant,
                    coin_level, good_karma, bad_karma, start_date,
                    fur_worth, mandible_worth, blood_worth, eps_broken
             FROM characters ORDER BY name",
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
        ];
        if !allowed.contains(&field) {
            return Err(crate::error::ScribiusError::Data(format!(
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
            return Err(crate::error::ScribiusError::Data(format!(
                "Unknown kill field: {}",
                field
            )));
        }

        // Try insert first
        let existing: Option<i64> = self
            .conn
            .query_row(
                "SELECT id FROM kills WHERE character_id = ?1 AND creature_name = ?2",
                params![char_id, creature_name],
                |row| row.get(0),
            )
            .ok();

        if let Some(kill_id) = existing {
            let sql = format!(
                "UPDATE kills SET {} = {} + 1, date_last = ?1 WHERE id = ?2",
                field, field
            );
            self.conn.execute(&sql, params![date, kill_id])?;
        } else {
            let sql = format!(
                "INSERT INTO kills (character_id, creature_name, {}, creature_value, date_first, date_last)
                 VALUES (?1, ?2, 1, ?3, ?4, ?4)",
                field
            );
            self.conn.execute(
                &sql,
                params![char_id, creature_name, creature_value, date],
            )?;
        }
        Ok(())
    }

    /// Get kills for a character, ordered by total count descending.
    pub fn get_kills(&self, char_id: i64) -> Result<Vec<Kill>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, character_id, creature_name,
                    killed_count, slaughtered_count, vanquished_count, dispatched_count,
                    assisted_kill_count, assisted_slaughter_count, assisted_vanquish_count, assisted_dispatch_count,
                    killed_by_count, date_first, date_last, creature_value
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
            })
        })?;

        Ok(kills.filter_map(|r| r.ok()).collect())
    }

    // === Trainers ===

    /// Upsert a trainer rank.
    pub fn upsert_trainer_rank(
        &self,
        char_id: i64,
        trainer_name: &str,
        date: &str,
    ) -> Result<()> {
        let existing: Option<i64> = self
            .conn
            .query_row(
                "SELECT id FROM trainers WHERE character_id = ?1 AND trainer_name = ?2",
                params![char_id, trainer_name],
                |row| row.get(0),
            )
            .ok();

        if let Some(trainer_id) = existing {
            self.conn.execute(
                "UPDATE trainers SET ranks = ranks + 1, date_of_last_rank = ?1 WHERE id = ?2",
                params![date, trainer_id],
            )?;
        } else {
            self.conn.execute(
                "INSERT INTO trainers (character_id, trainer_name, ranks, date_of_last_rank)
                 VALUES (?1, ?2, 1, ?3)",
                params![char_id, trainer_name, date],
            )?;
        }
        Ok(())
    }

    /// Get trainers for a character, ordered by ranks descending.
    pub fn get_trainers(&self, char_id: i64) -> Result<Vec<Trainer>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, character_id, trainer_name, ranks, modified_ranks, date_of_last_rank
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
            })
        })?;

        Ok(trainers.filter_map(|r| r.ok()).collect())
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
        let existing: Option<i64> = self
            .conn
            .query_row(
                "SELECT id FROM trainers WHERE character_id = ?1 AND trainer_name = ?2",
                params![char_id, trainer_name],
                |row| row.get(0),
            )
            .ok();

        if let Some(trainer_id) = existing {
            self.conn.execute(
                "UPDATE trainers SET modified_ranks = ?1 WHERE id = ?2",
                params![modified_ranks, trainer_id],
            )?;
        } else {
            self.conn.execute(
                "INSERT INTO trainers (character_id, trainer_name, ranks, modified_ranks)
                 VALUES (?1, ?2, 0, ?3)",
                params![char_id, trainer_name, modified_ranks],
            )?;
        }

        // Recalculate coin level from all trainer ranks
        let coin_level: i64 = self.conn.query_row(
            "SELECT COALESCE(SUM(ranks + modified_ranks), 0) FROM trainers WHERE character_id = ?1",
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
    pub fn upsert_lasty(
        &self,
        char_id: i64,
        creature_name: &str,
        lasty_type: &str,
    ) -> Result<()> {
        let existing: Option<i64> = self
            .conn
            .query_row(
                "SELECT id FROM lastys WHERE character_id = ?1 AND creature_name = ?2",
                params![char_id, creature_name],
                |row| row.get(0),
            )
            .ok();

        if let Some(lasty_id) = existing {
            self.conn.execute(
                "UPDATE lastys SET message_count = message_count + 1 WHERE id = ?1",
                params![lasty_id],
            )?;
        } else {
            self.conn.execute(
                "INSERT INTO lastys (character_id, creature_name, lasty_type, message_count)
                 VALUES (?1, ?2, ?3, 1)",
                params![char_id, creature_name, lasty_type],
            )?;
        }
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

    /// Get lastys for a character.
    pub fn get_lastys(&self, char_id: i64) -> Result<Vec<Lasty>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, character_id, creature_name, lasty_type, finished, message_count
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
        db.upsert_lasty(id, "Maha Ruknee", "Befriend").unwrap();
        db.upsert_lasty(id, "Maha Ruknee", "Befriend").unwrap();
        db.upsert_lasty(id, "Orga Anger", "Morph").unwrap();

        let lastys = db.get_lastys(id).unwrap();
        assert_eq!(lastys.len(), 2);

        let maha = lastys.iter().find(|l| l.creature_name == "Maha Ruknee").unwrap();
        assert_eq!(maha.lasty_type, "Befriend");
        assert_eq!(maha.message_count, 2);
        assert!(!maha.finished);

        let orga = lastys.iter().find(|l| l.creature_name == "Orga Anger").unwrap();
        assert_eq!(orga.lasty_type, "Morph");
        assert_eq!(orga.message_count, 1);
    }

    #[test]
    fn test_complete_lasty() {
        let db = Database::open_in_memory().unwrap();
        let id = db.get_or_create_character("Fen").unwrap();
        db.upsert_lasty(id, "Maha Ruknee", "Befriend").unwrap();
        db.complete_lasty(id, "Sespus").unwrap();

        let lastys = db.get_lastys(id).unwrap();
        assert_eq!(lastys.len(), 1);
        assert!(lastys[0].finished);
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
}
