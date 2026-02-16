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
            good_karma INTEGER NOT NULL DEFAULT 0,
            bad_karma INTEGER NOT NULL DEFAULT 0,
            start_date TEXT,
            fur_worth INTEGER NOT NULL DEFAULT 0,
            mandible_worth INTEGER NOT NULL DEFAULT 0,
            blood_worth INTEGER NOT NULL DEFAULT 0,
            eps_broken INTEGER NOT NULL DEFAULT 0
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

        // Verify tables exist
        let tables: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
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
}
