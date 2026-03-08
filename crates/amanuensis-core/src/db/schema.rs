use rusqlite::Connection;

use crate::error::Result;

pub fn create_tables(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS characters (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL UNIQUE,
            profession TEXT NOT NULL DEFAULT 'Unknown',
            logins INTEGER NOT NULL DEFAULT 0,
            departs INTEGER NOT NULL DEFAULT 0,
            deaths INTEGER NOT NULL DEFAULT 0,
            esteem INTEGER NOT NULL DEFAULT 0,
            armor TEXT NOT NULL DEFAULT '',
            coins_picked_up INTEGER NOT NULL DEFAULT 0,
            casino_won INTEGER NOT NULL DEFAULT 0,
            casino_lost INTEGER NOT NULL DEFAULT 0,
            chest_coins INTEGER NOT NULL DEFAULT 0,
            bounty_coins INTEGER NOT NULL DEFAULT 0,
            fur_coins INTEGER NOT NULL DEFAULT 0,
            mandible_coins INTEGER NOT NULL DEFAULT 0,
            blood_coins INTEGER NOT NULL DEFAULT 0,
            bells_used INTEGER NOT NULL DEFAULT 0,
            bells_broken INTEGER NOT NULL DEFAULT 0,
            chains_used INTEGER NOT NULL DEFAULT 0,
            chains_broken INTEGER NOT NULL DEFAULT 0,
            shieldstones_used INTEGER NOT NULL DEFAULT 0,
            shieldstones_broken INTEGER NOT NULL DEFAULT 0,
            ethereal_portals INTEGER NOT NULL DEFAULT 0,
            darkstone INTEGER NOT NULL DEFAULT 0,
            purgatory_pendant INTEGER NOT NULL DEFAULT 0,
            coin_level INTEGER NOT NULL DEFAULT 0,
            coin_level_interim INTEGER NOT NULL DEFAULT 0,
            good_karma INTEGER NOT NULL DEFAULT 0,
            bad_karma INTEGER NOT NULL DEFAULT 0,
            start_date TEXT,
            fur_worth INTEGER NOT NULL DEFAULT 0,
            mandible_worth INTEGER NOT NULL DEFAULT 0,
            blood_worth INTEGER NOT NULL DEFAULT 0,
            eps_broken INTEGER NOT NULL DEFAULT 0,
            untraining_count INTEGER NOT NULL DEFAULT 0,
            ore_found INTEGER NOT NULL DEFAULT 0,
            tin_ore_found INTEGER NOT NULL DEFAULT 0,
            copper_ore_found INTEGER NOT NULL DEFAULT 0,
            gold_ore_found INTEGER NOT NULL DEFAULT 0,
            iron_ore_found INTEGER NOT NULL DEFAULT 0,
            wood_taken INTEGER NOT NULL DEFAULT 0,
            wood_useless INTEGER NOT NULL DEFAULT 0,
            profession_override TEXT
        );

        CREATE TABLE IF NOT EXISTS kills (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            character_id INTEGER NOT NULL,
            creature_name TEXT NOT NULL,
            killed_count INTEGER NOT NULL DEFAULT 0,
            slaughtered_count INTEGER NOT NULL DEFAULT 0,
            vanquished_count INTEGER NOT NULL DEFAULT 0,
            dispatched_count INTEGER NOT NULL DEFAULT 0,
            assisted_kill_count INTEGER NOT NULL DEFAULT 0,
            assisted_slaughter_count INTEGER NOT NULL DEFAULT 0,
            assisted_vanquish_count INTEGER NOT NULL DEFAULT 0,
            assisted_dispatch_count INTEGER NOT NULL DEFAULT 0,
            killed_by_count INTEGER NOT NULL DEFAULT 0,
            date_first TEXT,
            date_first_killed TEXT,
            date_last TEXT,
            creature_value INTEGER NOT NULL DEFAULT 0,
            FOREIGN KEY (character_id) REFERENCES characters(id),
            UNIQUE(character_id, creature_name)
        );

        CREATE TABLE IF NOT EXISTS trainers (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            character_id INTEGER NOT NULL,
            trainer_name TEXT NOT NULL,
            ranks INTEGER NOT NULL DEFAULT 0,
            modified_ranks INTEGER NOT NULL DEFAULT 0,
            date_of_last_rank TEXT,
            effective_multiplier REAL NOT NULL DEFAULT 1.0,
            FOREIGN KEY (character_id) REFERENCES characters(id),
            UNIQUE(character_id, trainer_name)
        );

        CREATE TABLE IF NOT EXISTS lastys (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            character_id INTEGER NOT NULL,
            creature_name TEXT NOT NULL,
            lasty_type TEXT NOT NULL DEFAULT '',
            finished INTEGER NOT NULL DEFAULT 0,
            message_count INTEGER NOT NULL DEFAULT 0,
            FOREIGN KEY (character_id) REFERENCES characters(id),
            UNIQUE(character_id, creature_name)
        );

        CREATE TABLE IF NOT EXISTS pets (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            character_id INTEGER NOT NULL,
            pet_name TEXT NOT NULL,
            creature_name TEXT NOT NULL,
            FOREIGN KEY (character_id) REFERENCES characters(id),
            UNIQUE(character_id, pet_name)
        );

        CREATE TABLE IF NOT EXISTS log_files (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            character_id INTEGER NOT NULL,
            file_path TEXT NOT NULL UNIQUE,
            content_hash TEXT NOT NULL DEFAULT '',
            date_read TEXT NOT NULL,
            FOREIGN KEY (character_id) REFERENCES characters(id)
        );

        CREATE VIRTUAL TABLE IF NOT EXISTS log_lines USING fts5(
            content,
            character_id UNINDEXED,
            timestamp UNINDEXED,
            file_path UNINDEXED,
            tokenize='unicode61'
        );

        CREATE TABLE IF NOT EXISTS process_logs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            created_at TEXT NOT NULL,
            level TEXT NOT NULL,
            message TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS trainer_checkpoints (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            character_id INTEGER NOT NULL,
            trainer_name TEXT NOT NULL,
            rank_min INTEGER NOT NULL,
            rank_max INTEGER,
            timestamp TEXT NOT NULL,
            name_filtered INTEGER NOT NULL DEFAULT 0,
            FOREIGN KEY (character_id) REFERENCES characters(id)
        );
        ",
    )?;
    Ok(())
}

/// Migrate existing databases to add new columns.
/// Uses ALTER TABLE ADD COLUMN which is safe if columns already exist (we catch the error).
pub fn migrate_tables(conn: &Connection) -> Result<()> {
    let migrations = [
        "ALTER TABLE characters ADD COLUMN good_karma INTEGER NOT NULL DEFAULT 0",
        "ALTER TABLE characters ADD COLUMN bad_karma INTEGER NOT NULL DEFAULT 0",
        "ALTER TABLE characters ADD COLUMN start_date TEXT",
        "ALTER TABLE characters ADD COLUMN fur_worth INTEGER NOT NULL DEFAULT 0",
        "ALTER TABLE characters ADD COLUMN mandible_worth INTEGER NOT NULL DEFAULT 0",
        "ALTER TABLE characters ADD COLUMN blood_worth INTEGER NOT NULL DEFAULT 0",
        "ALTER TABLE characters ADD COLUMN eps_broken INTEGER NOT NULL DEFAULT 0",
        "ALTER TABLE trainers ADD COLUMN apply_learning_ranks INTEGER NOT NULL DEFAULT 0",
        "ALTER TABLE trainers ADD COLUMN apply_learning_unknown_count INTEGER NOT NULL DEFAULT 0",
        "ALTER TABLE lastys ADD COLUMN first_seen_date TEXT",
        "ALTER TABLE lastys ADD COLUMN last_seen_date TEXT",
        "ALTER TABLE lastys ADD COLUMN completed_date TEXT",
        "ALTER TABLE lastys ADD COLUMN abandoned_date TEXT",
        "ALTER TABLE characters ADD COLUMN merged_into INTEGER REFERENCES characters(id)",
        "ALTER TABLE characters ADD COLUMN untraining_count INTEGER NOT NULL DEFAULT 0",
        "ALTER TABLE kills ADD COLUMN date_last_killed TEXT",
        "ALTER TABLE kills ADD COLUMN date_last_slaughtered TEXT",
        "ALTER TABLE kills ADD COLUMN date_last_vanquished TEXT",
        "ALTER TABLE kills ADD COLUMN date_last_dispatched TEXT",
        "ALTER TABLE trainers ADD COLUMN rank_mode TEXT NOT NULL DEFAULT 'modifier'",
        "ALTER TABLE trainers ADD COLUMN override_date TEXT",
        "ALTER TABLE trainers ADD COLUMN effective_multiplier REAL NOT NULL DEFAULT 1.0",
        "ALTER TABLE kills ADD COLUMN best_loot_value INTEGER NOT NULL DEFAULT 0",
        "ALTER TABLE kills ADD COLUMN best_loot_item TEXT NOT NULL DEFAULT ''",
        "ALTER TABLE kills ADD COLUMN date_first_killed TEXT",
        "ALTER TABLE characters ADD COLUMN coin_level_interim INTEGER NOT NULL DEFAULT 0",
        "ALTER TABLE characters ADD COLUMN ore_found INTEGER NOT NULL DEFAULT 0",
        "ALTER TABLE characters ADD COLUMN tin_ore_found INTEGER NOT NULL DEFAULT 0",
        "ALTER TABLE characters ADD COLUMN copper_ore_found INTEGER NOT NULL DEFAULT 0",
        "ALTER TABLE characters ADD COLUMN gold_ore_found INTEGER NOT NULL DEFAULT 0",
        "ALTER TABLE characters ADD COLUMN iron_ore_found INTEGER NOT NULL DEFAULT 0",
        "ALTER TABLE characters ADD COLUMN wood_taken INTEGER NOT NULL DEFAULT 0",
        "ALTER TABLE characters ADD COLUMN wood_useless INTEGER NOT NULL DEFAULT 0",
        "ALTER TABLE characters ADD COLUMN profession_override TEXT",
        // Marks rows inserted after the character-name filter was added.
        // Existing rows (recorded before the filter) default to 0 and are purged below.
        "ALTER TABLE trainer_checkpoints ADD COLUMN name_filtered INTEGER NOT NULL DEFAULT 0",
    ];

    for sql in &migrations {
        // Ignore "duplicate column name" errors for idempotent migration
        match conn.execute(sql, []) {
            Ok(_) => {}
            Err(rusqlite::Error::SqliteFailure(err, _))
                if err.code == rusqlite::ffi::ErrorCode::Unknown
                    || err.extended_code == 1 =>
            {
                // Column already exists — that's fine
            }
            Err(e) => return Err(e.into()),
        }
    }

    // Create FTS5 table for full-text log search (idempotent via IF NOT EXISTS)
    // Also create trainer_checkpoints table (idempotent via IF NOT EXISTS)
    conn.execute_batch(
        "CREATE VIRTUAL TABLE IF NOT EXISTS log_lines USING fts5(
            content,
            character_id UNINDEXED,
            timestamp UNINDEXED,
            file_path UNINDEXED,
            tokenize='unicode61'
        );
        CREATE TABLE IF NOT EXISTS trainer_checkpoints (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            character_id INTEGER NOT NULL,
            trainer_name TEXT NOT NULL,
            rank_min INTEGER NOT NULL,
            rank_max INTEGER,
            timestamp TEXT NOT NULL,
            name_filtered INTEGER NOT NULL DEFAULT 0,
            FOREIGN KEY (character_id) REFERENCES characters(id)
        );
        -- Purge checkpoints that were recorded before the character-name filter existed.
        -- name_filtered=0 means the row was inserted by old code (no name check).
        -- This DELETE runs on every database open (migrate_tables is called at startup),
        -- but is a no-op once all pre-filter rows are gone, since all new inserts
        -- explicitly set name_filtered=1.
        DELETE FROM trainer_checkpoints WHERE name_filtered = 0;",
    )?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    #[test]
    fn test_create_tables() {
        let conn = Connection::open_in_memory().unwrap();
        create_tables(&conn).unwrap();

        // Verify tables exist (including virtual tables)
        let tables: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type IN ('table', 'shadow') ORDER BY name")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();

        assert!(tables.contains(&"characters".to_string()));
        assert!(tables.contains(&"kills".to_string()));
        assert!(tables.contains(&"trainers".to_string()));
        assert!(tables.contains(&"lastys".to_string()));
        assert!(tables.contains(&"pets".to_string()));
        assert!(tables.contains(&"log_files".to_string()));
        // FTS5 virtual table creates shadow tables (log_lines_content, etc.)
        assert!(tables.iter().any(|t| t.starts_with("log_lines")));
    }

    #[test]
    fn test_create_tables_idempotent() {
        let conn = Connection::open_in_memory().unwrap();
        create_tables(&conn).unwrap();
        create_tables(&conn).unwrap(); // Should not error
    }

    #[test]
    fn test_migrate_tables_idempotent() {
        let conn = Connection::open_in_memory().unwrap();
        create_tables(&conn).unwrap();
        // Migrate twice — should not error
        migrate_tables(&conn).unwrap();
        migrate_tables(&conn).unwrap();
    }

    #[test]
    fn test_migrate_purges_unfiltered_checkpoints() {
        let conn = Connection::open_in_memory().unwrap();
        create_tables(&conn).unwrap();
        migrate_tables(&conn).unwrap();

        // Insert a character so we can satisfy the FK
        conn.execute(
            "INSERT INTO characters (name) VALUES ('TestChar')",
            [],
        ).unwrap();
        let char_id: i64 = conn.last_insert_rowid();

        // Insert a row with name_filtered=0 (pre-filter, should be purged)
        conn.execute(
            "INSERT INTO trainer_checkpoints (character_id, trainer_name, rank_min, rank_max, timestamp, name_filtered)
             VALUES (?1, 'Histia', 0, 9, '2024-01-01 12:00:00', 0)",
            rusqlite::params![char_id],
        ).unwrap();

        // Insert a row with name_filtered=1 (new-style, should be kept)
        conn.execute(
            "INSERT INTO trainer_checkpoints (character_id, trainer_name, rank_min, rank_max, timestamp, name_filtered)
             VALUES (?1, 'Histia', 10, 19, '2024-01-02 12:00:00', 1)",
            rusqlite::params![char_id],
        ).unwrap();

        // Run migrate_tables again — should purge name_filtered=0 rows
        migrate_tables(&conn).unwrap();

        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM trainer_checkpoints WHERE name_filtered = 0",
            [],
            |row| row.get(0),
        ).unwrap();
        assert_eq!(count, 0, "Rows with name_filtered=0 should have been purged");

        let kept: i64 = conn.query_row(
            "SELECT COUNT(*) FROM trainer_checkpoints WHERE name_filtered = 1",
            [],
            |row| row.get(0),
        ).unwrap();
        assert_eq!(kept, 1, "Row with name_filtered=1 should still be present");
    }
}
