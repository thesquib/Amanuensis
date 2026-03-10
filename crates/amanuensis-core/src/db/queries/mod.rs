use rusqlite::{Connection, Row};
use serde::Serialize;

use crate::error::Result;
use crate::models::*;

mod character;
mod checkpoint;
mod kill;
pub mod trainer;
mod lasty;
mod pet;
mod log_file;
mod merge;
mod process_log;

// ---------------------------------------------------------------------------
// Shared character projection
// ---------------------------------------------------------------------------

/// Column list used in every full-character SELECT.
const CHARACTER_COLUMNS: &str =
    "id, name, profession, logins, departs, deaths, esteem, armor,
     coins_picked_up, casino_won, casino_lost, chest_coins, bounty_coins,
     fur_coins, mandible_coins, blood_coins,
     bells_used, bells_broken, chains_used, chains_broken,
     shieldstones_used, shieldstones_broken, ethereal_portals, darkstone, purgatory_pendant,
     coin_level, coin_level_interim, good_karma, bad_karma, gave_good_karma, gave_bad_karma, start_date,
     fur_worth, mandible_worth, blood_worth, eps_broken, untraining_count, ore_found,
     tin_ore_found, copper_ore_found, gold_ore_found, iron_ore_found,
     wood_taken, wood_useless, profession_override";

/// Map a rusqlite row (from a CHARACTER_COLUMNS projection) to a Character.
fn map_character_row(row: &Row<'_>) -> rusqlite::Result<Character> {
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
        coin_level_interim: row.get(26)?,
        good_karma: row.get(27)?,
        bad_karma: row.get(28)?,
        gave_good_karma: row.get(29)?,
        gave_bad_karma: row.get(30)?,
        start_date: row.get(31)?,
        fur_worth: row.get(32)?,
        mandible_worth: row.get(33)?,
        blood_worth: row.get(34)?,
        eps_broken: row.get(35)?,
        untraining_count: row.get(36)?,
        ore_found: row.get(37)?,
        tin_ore_found: row.get(38)?,
        copper_ore_found: row.get(39)?,
        gold_ore_found: row.get(40)?,
        iron_ore_found: row.get(41)?,
        wood_taken: row.get(42)?,
        wood_useless: row.get(43)?,
        profession_override: row.get(44)?,
        total_ranks: 0,
    })
}

/// A single search result from the FTS5 log_lines table.
#[derive(Debug, Serialize)]
pub struct LogSearchResult {
    pub content: String,
    pub character_id: i64,
    pub timestamp: String,
    pub file_path: String,
    pub snippet: String,
    pub character_name: String,
    pub context_before: Vec<String>,
    pub context_after: Vec<String>,
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
    fn test_death_does_not_set_dates() {
        // date_first/date_last should only reflect kill events, not deaths
        let db = Database::open_in_memory().unwrap();
        let id = db.get_or_create_character("Fen").unwrap();

        // First encounter is a death — dates should be NULL
        db.upsert_kill(id, "Orga Fury", "killed_by_count", 10, "2024-01-01")
            .unwrap();
        let kills = db.get_kills(id).unwrap();
        let orga = kills.iter().find(|k| k.creature_name == "Orga Fury").unwrap();
        assert_eq!(orga.killed_by_count, 1);
        assert_eq!(orga.date_first, None, "Death should not set date_first");
        assert_eq!(orga.date_last, None, "Death should not set date_last");

        // Now kill the creature — dates should be set
        db.upsert_kill(id, "Orga Fury", "killed_count", 10, "2024-02-15")
            .unwrap();
        let kills = db.get_kills(id).unwrap();
        let orga = kills.iter().find(|k| k.creature_name == "Orga Fury").unwrap();
        assert_eq!(orga.killed_count, 1);
        assert_eq!(orga.killed_by_count, 1);
        assert_eq!(orga.date_first, Some("2024-02-15".to_string()), "First kill should backfill date_first");
        assert_eq!(orga.date_last, Some("2024-02-15".to_string()));

        // Another death — dates should NOT change
        db.upsert_kill(id, "Orga Fury", "killed_by_count", 10, "2024-03-01")
            .unwrap();
        let kills = db.get_kills(id).unwrap();
        let orga = kills.iter().find(|k| k.creature_name == "Orga Fury").unwrap();
        assert_eq!(orga.killed_by_count, 2);
        assert_eq!(orga.date_first, Some("2024-02-15".to_string()), "Death should not change date_first");
        assert_eq!(orga.date_last, Some("2024-02-15".to_string()), "Death should not change date_last");

        // Another kill — date_last should update but date_first stays
        db.upsert_kill(id, "Orga Fury", "slaughtered_count", 10, "2024-04-01")
            .unwrap();
        let kills = db.get_kills(id).unwrap();
        let orga = kills.iter().find(|k| k.creature_name == "Orga Fury").unwrap();
        assert_eq!(orga.date_first, Some("2024-02-15".to_string()), "date_first should stay at first kill");
        assert_eq!(orga.date_last, Some("2024-04-01".to_string()), "date_last should update to latest kill");
    }

    #[test]
    fn test_upsert_trainer_rank() {
        let db = Database::open_in_memory().unwrap();
        let id = db.get_or_create_character("Fen").unwrap();
        db.upsert_trainer_rank(id, "Bangus Anmash", "2024-01-01", 1.0)
            .unwrap();
        db.upsert_trainer_rank(id, "Bangus Anmash", "2024-01-02", 1.0)
            .unwrap();
        db.upsert_trainer_rank(id, "Regia", "2024-01-03", 1.0).unwrap();

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
        let fen_id = db.get_or_create_character("Fen").unwrap();
        let pip_id = db.get_or_create_character("pip").unwrap();

        // Characters with logins=0 are hidden (ghost rows from reset)
        let chars = db.list_characters().unwrap();
        assert_eq!(chars.len(), 0);

        // Once logged in, they appear
        db.increment_character_field(fen_id, "logins", 1).unwrap();
        db.increment_character_field(pip_id, "logins", 1).unwrap();
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
        db.upsert_trainer_rank(id_a, "Histia", "2024-01-01", 1.0).unwrap();
        db.upsert_trainer_rank(id_a, "Histia", "2024-01-02", 1.0).unwrap();
        db.upsert_trainer_rank(id_b, "Histia", "2024-01-03", 1.0).unwrap();
        db.upsert_trainer_rank(id_b, "Regia", "2024-01-04", 1.0).unwrap();
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
        db.increment_character_field(id_a, "logins", 1).unwrap();
        db.increment_character_field(id_b, "logins", 1).unwrap();

        // Merge B into A
        db.merge_characters(&[id_b], id_a).unwrap();

        // list_characters should NOT return CharB (merged) nor zero-login chars
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
        let results = db.search_log_lines("Rat", None, 10, true, 0, 0).unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0].snippet.contains("<mark>"));
        assert_eq!(results[0].character_name, "Fen");

        // Search with character filter
        let results = db.search_log_lines("Rat", Some(id), 10, true, 0, 0).unwrap();
        assert_eq!(results.len(), 1);

        // Search with wrong character
        let id2 = db.get_or_create_character("Pip").unwrap();
        let results = db.search_log_lines("Rat", Some(id2), 10, true, 0, 0).unwrap();
        assert_eq!(results.len(), 0);

        // Search no match
        let results = db.search_log_lines("Dragon", None, 10, true, 0, 0).unwrap();
        assert_eq!(results.len(), 0);
    }
}
