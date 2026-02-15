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
        Ok(Self { conn })
    }

    /// Open an in-memory database (for testing).
    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        crate::db::schema::create_tables(&conn)?;
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
                    shieldstones_used, shieldstones_broken, ethereal_portals, darkstone, purgatory_pendant
             FROM characters WHERE name = ?1",
            params![name],
            |row| {
                Ok(Character {
                    id: Some(row.get(0)?),
                    name: row.get(1)?,
                    profession: match row.get::<_, String>(2)?.as_str() {
                        "Fighter" => Profession::Fighter,
                        "Healer" => Profession::Healer,
                        "Mystic" => Profession::Mystic,
                        "Ranger" => Profession::Ranger,
                        _ => Profession::Unknown,
                    },
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
                    shieldstones_used, shieldstones_broken, ethereal_portals, darkstone, purgatory_pendant
             FROM characters ORDER BY name",
        )?;

        let chars = stmt.query_map([], |row| {
            Ok(Character {
                id: Some(row.get(0)?),
                name: row.get(1)?,
                profession: match row.get::<_, String>(2)?.as_str() {
                    "Fighter" => Profession::Fighter,
                    "Healer" => Profession::Healer,
                    "Mystic" => Profession::Mystic,
                    "Ranger" => Profession::Ranger,
                    _ => Profession::Unknown,
                },
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
            "darkstone", "purgatory_pendant",
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

    // === Log files ===

    /// Check if a log file has already been scanned.
    pub fn is_log_scanned(&self, file_path: &str) -> Result<bool> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM log_files WHERE file_path = ?1",
            params![file_path],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    /// Mark a log file as scanned.
    pub fn mark_log_scanned(&self, char_id: i64, file_path: &str, date_read: &str) -> Result<()> {
        self.conn.execute(
            "INSERT OR IGNORE INTO log_files (character_id, file_path, date_read)
             VALUES (?1, ?2, ?3)",
            params![char_id, file_path, date_read],
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_or_create_character() {
        let db = Database::open_in_memory().unwrap();
        let id1 = db.get_or_create_character("Ruuk").unwrap();
        let id2 = db.get_or_create_character("Ruuk").unwrap();
        assert_eq!(id1, id2, "Same name should return same ID");

        let id3 = db.get_or_create_character("squib").unwrap();
        assert_ne!(id1, id3, "Different names should return different IDs");
    }

    #[test]
    fn test_get_character() {
        let db = Database::open_in_memory().unwrap();
        db.get_or_create_character("Ruuk").unwrap();
        let char = db.get_character("Ruuk").unwrap().unwrap();
        assert_eq!(char.name, "Ruuk");
        assert_eq!(char.profession, Profession::Unknown);
        assert_eq!(char.logins, 0);
    }

    #[test]
    fn test_increment_character_field() {
        let db = Database::open_in_memory().unwrap();
        let id = db.get_or_create_character("Ruuk").unwrap();
        db.increment_character_field(id, "logins", 1).unwrap();
        db.increment_character_field(id, "logins", 1).unwrap();
        db.increment_character_field(id, "deaths", 3).unwrap();
        let char = db.get_character("Ruuk").unwrap().unwrap();
        assert_eq!(char.logins, 2);
        assert_eq!(char.deaths, 3);
    }

    #[test]
    fn test_increment_invalid_field_rejected() {
        let db = Database::open_in_memory().unwrap();
        let id = db.get_or_create_character("Ruuk").unwrap();
        let result = db.increment_character_field(id, "name; DROP TABLE characters;--", 1);
        assert!(result.is_err());
    }

    #[test]
    fn test_upsert_kill() {
        let db = Database::open_in_memory().unwrap();
        let id = db.get_or_create_character("Ruuk").unwrap();
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
        let id = db.get_or_create_character("Ruuk").unwrap();
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
        let id = db.get_or_create_character("Ruuk").unwrap();
        assert!(!db.is_log_scanned("/logs/test.txt").unwrap());
        db.mark_log_scanned(id, "/logs/test.txt", "2024-01-01")
            .unwrap();
        assert!(db.is_log_scanned("/logs/test.txt").unwrap());
        assert_eq!(db.scanned_log_count().unwrap(), 1);
    }

    #[test]
    fn test_list_characters() {
        let db = Database::open_in_memory().unwrap();
        db.get_or_create_character("Ruuk").unwrap();
        db.get_or_create_character("squib").unwrap();
        let chars = db.list_characters().unwrap();
        assert_eq!(chars.len(), 2);
    }

    #[test]
    fn test_coin_tracking() {
        let db = Database::open_in_memory().unwrap();
        let id = db.get_or_create_character("Ruuk").unwrap();
        db.increment_character_field(id, "coins_picked_up", 50).unwrap();
        db.increment_character_field(id, "fur_coins", 10).unwrap();
        db.increment_character_field(id, "blood_coins", 15).unwrap();
        let char = db.get_character("Ruuk").unwrap().unwrap();
        assert_eq!(char.coins_picked_up, 50);
        assert_eq!(char.fur_coins, 10);
        assert_eq!(char.blood_coins, 15);
    }
}
