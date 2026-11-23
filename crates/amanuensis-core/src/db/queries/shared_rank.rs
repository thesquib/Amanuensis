//! Trainer sign-ups and shared rank message observations (internal bookkeeping).
use rusqlite::params;

use super::Database;
use crate::error::Result;

pub struct Signup {
    pub file_path: String,
    pub line_no: i64,
    pub at: String,
    /// None = a player teacher.
    pub trainer: Option<String>,
}

pub struct SharedRankObservation {
    pub file_path: String,
    pub line_no: i64,
    pub character_id: i64,
    pub at: String,
    pub default_trainer: String,
}

impl Database {
    /// Returns true only when a row was inserted (a re-read line adds nothing).
    pub fn insert_trainer_signup(
        &self, file_path: &str, line_no: i64, char_id: i64, at: &str, trainer: Option<&str>,
    ) -> Result<bool> {
        self.conn.execute(
            "INSERT OR IGNORE INTO trainer_signups (file_path, line_no, character_id, at, trainer_name)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![file_path, line_no, char_id, at, trainer],
        )?;
        Ok(self.conn.changes() == 1)
    }

    pub fn insert_shared_rank_observation(
        &self, file_path: &str, line_no: i64, char_id: i64, at: &str, default_trainer: &str,
    ) -> Result<bool> {
        self.conn.execute(
            "INSERT OR IGNORE INTO shared_rank_observations (file_path, line_no, character_id, at, default_trainer)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![file_path, line_no, char_id, at, default_trainer],
        )?;
        Ok(self.conn.changes() == 1)
    }

    pub fn signups_for(&self, char_id: i64) -> Result<Vec<Signup>> {
        let mut stmt = self.conn.prepare(
            "SELECT file_path, line_no, at, trainer_name FROM trainer_signups
             WHERE character_id = ?1 ORDER BY at, file_path, line_no",
        )?;
        let rows = stmt.query_map([char_id], |r| {
            Ok(Signup { file_path: r.get(0)?, line_no: r.get(1)?, at: r.get(2)?, trainer: r.get(3)? })
        })?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    pub fn all_shared_rank_observations(&self) -> Result<Vec<SharedRankObservation>> {
        let mut stmt = self.conn.prepare(
            "SELECT file_path, line_no, character_id, at, default_trainer FROM shared_rank_observations
             ORDER BY at, file_path, line_no",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(SharedRankObservation {
                file_path: r.get(0)?, line_no: r.get(1)?, character_id: r.get(2)?,
                at: r.get(3)?, default_trainer: r.get(4)?,
            })
        })?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    /// (character, trainer) pairs whose `ranks` currently include shared-message ranks.
    pub fn trainers_with_shared_ranks(&self) -> Result<Vec<(i64, String)>> {
        let mut stmt = self.conn.prepare(
            "SELECT character_id, trainer_name FROM trainers WHERE shared_ranks > 0",
        )?;
        let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    /// Replace the shared part of each trainer's `ranks` with a freshly derived count.
    /// `counts` is `(char_id, trainer, count, max_at)`; `max_at` is empty when count is 0.
    /// Touches only rows whose count changed or whose `ranks < shared_ranks` (the
    /// non-shared part was wiped: `MAX(..., 0)` heals it). Writes no user-entered column.
    pub fn apply_shared_counts(
        &self,
        counts: &[(i64, String, i64, String)],
        multiplier: impl Fn(&str) -> f64,
    ) -> Result<()> {
        let mut stmt = self.conn.prepare(
            "INSERT INTO trainers (character_id, trainer_name, ranks, shared_ranks, date_of_last_rank, effective_multiplier)
             VALUES (?1, ?2, ?3, ?3, NULLIF(?4, ''), ?5)
             ON CONFLICT(character_id, trainer_name) DO UPDATE SET
                ranks = MAX(ranks - shared_ranks, 0) + excluded.shared_ranks,
                shared_ranks = excluded.shared_ranks,
                date_of_last_rank = CASE
                    WHEN excluded.date_of_last_rank > COALESCE(date_of_last_rank, '')
                    THEN excluded.date_of_last_rank ELSE date_of_last_rank END,
                effective_multiplier = excluded.effective_multiplier
             WHERE shared_ranks != excluded.shared_ranks OR ranks < shared_ranks",
        )?;
        for (char_id, trainer, count, max_at) in counts {
            stmt.execute(params![char_id, trainer, count, max_at, multiplier(trainer)])?;
        }
        Ok(())
    }

    /// Part of `trainers.ranks` that came from shared rank observations (0 if no row).
    pub fn shared_ranks_of(&self, char_id: i64, trainer: &str) -> Result<i64> {
        Ok(self.conn.query_row(
            "SELECT COALESCE((SELECT shared_ranks FROM trainers WHERE character_id = ?1 AND trainer_name = ?2), 0)",
            params![char_id, trainer],
            |r| r.get(0),
        )?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn count(db: &Database, table: &str) -> i64 {
        db.conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0)).unwrap()
    }

    fn trainer_row(db: &Database, cid: i64, ranks: i64, shared: i64) {
        db.conn.execute(
            "INSERT INTO trainers (character_id, trainer_name, ranks, shared_ranks) VALUES (?1, 'Histia', ?2, ?3)",
            params![cid, ranks, shared],
        ).unwrap();
    }

    #[test]
    fn signup_insert_is_idempotent() {
        let db = Database::open_in_memory().unwrap();
        let id = db.get_or_create_character("Ruuk").unwrap();
        assert!(db.insert_trainer_signup("a.txt", 3, id, "2026-01-01 00:00:00", Some("Higgrus")).unwrap());
        assert!(!db.insert_trainer_signup("a.txt", 3, id, "2026-01-01 00:00:00", Some("Higgrus")).unwrap());
        assert!(db.insert_shared_rank_observation("a.txt", 4, id, "2026-01-01 00:00:01", "Histia").unwrap());
        assert!(!db.insert_shared_rank_observation("a.txt", 4, id, "2026-01-01 00:00:01", "Histia").unwrap());
        assert_eq!(db.signups_for(id).unwrap().len(), 1);
        assert_eq!(db.all_shared_rank_observations().unwrap().len(), 1);
    }

    #[test]
    fn apply_heals_ranks_wiped_below_shared() {
        // A writer that zeroed `ranks` but not `shared_ranks` must not leave 0 or go negative.
        let db = Database::open_in_memory().unwrap();
        let id = db.get_or_create_character("Ruuk").unwrap();
        trainer_row(&db, id, 0, 3);
        db.apply_shared_counts(&[(id, "Histia".into(), 3, "2026-01-01 00:00:00".into())], |_| 1.0).unwrap();
        let ranks: i64 = db.conn.query_row("SELECT ranks FROM trainers", [], |r| r.get(0)).unwrap();
        assert_eq!((ranks, db.shared_ranks_of(id, "Histia").unwrap()), (3, 3));
    }

    #[test]
    fn reset_clears_observations_and_shared_ranks() {
        let db = Database::open_in_memory().unwrap();
        let id = db.get_or_create_character("Ruuk").unwrap();
        db.insert_trainer_signup("a.txt", 3, id, "2026-01-01 00:00:00", None).unwrap();
        db.insert_shared_rank_observation("a.txt", 4, id, "2026-01-01 00:00:01", "Histia").unwrap();
        trainer_row(&db, id, 5, 3);
        db.reset_log_data().unwrap();
        assert_eq!(count(&db, "trainer_signups"), 0);
        assert_eq!(count(&db, "shared_rank_observations"), 0);
        let ranks: i64 = db.conn.query_row("SELECT ranks FROM trainers", [], |r| r.get(0)).unwrap();
        assert_eq!((ranks, db.shared_ranks_of(id, "Histia").unwrap()), (0, 0));
    }

    #[test]
    fn override_switch_zeroes_shared_ranks() {
        let db = Database::open_in_memory().unwrap();
        let id = db.get_or_create_character("Ruuk").unwrap();
        trainer_row(&db, id, 10, 10);
        db.conn.execute("UPDATE trainers SET notes = 'mine'", []).unwrap();
        db.set_rank_override(id, "Histia", "override_until_date", 4, Some("2026-01-01")).unwrap();
        let (ranks, modified, notes): (i64, i64, String) = db.conn.query_row(
            "SELECT ranks, modified_ranks, notes FROM trainers", [], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).unwrap();
        assert_eq!((ranks, modified, notes.as_str()), (0, 4, "mine"));
        assert_eq!(db.shared_ranks_of(id, "Histia").unwrap(), 0);
    }

    #[test]
    fn migrate_adds_shared_ranks_to_legacy_db() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        // Pre-change base trainers table, as in schema.rs create_tables.
        conn.execute_batch(
            "CREATE TABLE trainers (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                character_id INTEGER NOT NULL,
                trainer_name TEXT NOT NULL,
                ranks INTEGER NOT NULL DEFAULT 0,
                modified_ranks INTEGER NOT NULL DEFAULT 0,
                date_of_last_rank TEXT,
                effective_multiplier REAL NOT NULL DEFAULT 1.0,
                UNIQUE(character_id, trainer_name)
            );
            INSERT INTO trainers (character_id, trainer_name, ranks) VALUES (1, 'Histia', 7);",
        ).unwrap();
        crate::db::schema::create_tables(&conn).unwrap();
        crate::db::schema::migrate_tables(&conn).unwrap();
        let (ranks, shared): (i64, i64) = conn
            .query_row("SELECT ranks, shared_ranks FROM trainers", [], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap();
        assert_eq!((ranks, shared), (7, 0));
    }

    #[test]
    fn delete_all_clears_new_tables_and_ack() {
        let db = Database::open_in_memory().unwrap();
        let id = db.get_or_create_character("Ruuk").unwrap();
        db.insert_trainer_signup("a.txt", 3, id, "2026-01-01 00:00:00", None).unwrap();
        db.insert_shared_rank_observation("a.txt", 4, id, "2026-01-01 00:00:01", "Histia").unwrap();
        db.conn.execute("INSERT INTO db_meta (key, value) VALUES ('trainer_revision_ack', '1')", []).unwrap();
        db.delete_all_data().unwrap();
        assert_eq!(count(&db, "trainer_signups"), 0);
        assert_eq!(count(&db, "shared_rank_observations"), 0);
        let acks: i64 = db.conn.query_row(
            "SELECT COUNT(*) FROM db_meta WHERE key = 'trainer_revision_ack'", [], |r| r.get(0)).unwrap();
        assert_eq!(acks, 0);
    }
}
