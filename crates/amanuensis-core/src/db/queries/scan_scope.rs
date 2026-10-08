use rusqlite::params;
use serde::{Deserialize, Serialize};

use crate::error::Result;
use super::Database;

/// Which counters a kills/trainers query reads: lifetime totals or only the most recent scan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScanScope {
    #[default]
    All,
    LastScan,
}

const COUNTER_KEY: &str = "scan_token_counter";
const LAST_TOKEN_KEY: &str = "last_scan_token";

impl Database {
    fn meta_i64(&self, key: &str) -> Result<Option<i64>> {
        let v: Option<String> = match self.conn.query_row(
            "SELECT value FROM db_meta WHERE key = ?1",
            params![key],
            |r| r.get(0),
        ) {
            Ok(v) => Some(v),
            Err(rusqlite::Error::QueryReturnedNoRows) => None,
            Err(e) => return Err(e.into()),
        };
        Ok(v.and_then(|s| s.parse().ok()))
    }

    fn set_meta_i64(&self, key: &str, value: i64) -> Result<()> {
        self.conn.execute(
            "INSERT INTO db_meta (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value.to_string()],
        )?;
        Ok(())
    }

    /// Allocate a fresh, monotonically increasing scan token.
    pub fn begin_scan_token(&self) -> Result<i64> {
        let next = self.meta_i64(COUNTER_KEY)?.unwrap_or(0) + 1;
        self.set_meta_i64(COUNTER_KEY, next)?;
        Ok(next)
    }

    /// Called before a scan's writes to the shadow tables. The first call with a new token
    /// clears the previous scan's shadow data; further calls with the same token are no-ops.
    pub fn mark_scan_write(&self, token: i64) -> Result<()> {
        if self.meta_i64(LAST_TOKEN_KEY)? == Some(token) {
            return Ok(());
        }
        // SAVEPOINT (not BEGIN) because scans already run inside an outer transaction.
        self.conn.execute_batch("SAVEPOINT mark_scan_write")?;
        let res = self
            .conn
            .execute_batch("DELETE FROM scan_kills; DELETE FROM scan_trainers;")
            .map_err(Into::into)
            .and_then(|_| self.set_meta_i64(LAST_TOKEN_KEY, token));
        match res {
            Ok(()) => self.conn.execute_batch("RELEASE mark_scan_write")?,
            Err(e) => {
                let _ = self.conn.execute_batch("ROLLBACK TO mark_scan_write; RELEASE mark_scan_write");
                return Err(e);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Database;

    fn count(db: &Database, table: &str) -> i64 {
        db.conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0)).unwrap()
    }

    #[test]
    fn same_token_accumulates_new_token_clears() {
        let db = Database::open_in_memory().unwrap();
        let id = db.get_or_create_character("Fen").unwrap();
        let t1 = db.begin_scan_token().unwrap();
        db.mark_scan_write(t1).unwrap();
        db.upsert_kill_scan(id, "Rat", "killed_count", 2, "2024-01-01").unwrap();
        db.mark_scan_write(t1).unwrap(); // same scan: must NOT clear
        db.upsert_kill_scan(id, "Rat", "killed_count", 2, "2024-01-01").unwrap();
        assert_eq!({ let k = db.get_kills_scoped(id, ScanScope::LastScan).unwrap(); assert_eq!(k.len(), 1); k[0].killed_count }, 2);

        let t2 = db.begin_scan_token().unwrap();
        assert!(t2 > t1);
        db.mark_scan_write(t2).unwrap(); // new scan's first write clears
        assert_eq!(count(&db, "scan_kills"), 0);
    }

    #[test]
    fn token_without_write_leaves_previous_scan() {
        let db = Database::open_in_memory().unwrap();
        let id = db.get_or_create_character("Fen").unwrap();
        let t1 = db.begin_scan_token().unwrap();
        db.mark_scan_write(t1).unwrap();
        db.upsert_kill_scan(id, "Rat", "killed_count", 2, "2024-01-01").unwrap();
        let _t2 = db.begin_scan_token().unwrap(); // scan that finds nothing
        assert_eq!(count(&db, "scan_kills"), 1);
    }

    #[test]
    fn last_scan_trainer_ranks_exclude_modified_ranks() {
        let db = Database::open_in_memory().unwrap();
        let id = db.get_or_create_character("Fen").unwrap();
        db.upsert_trainer_rank(id, "Atkus", "2024-01-01 10:00:00", 1.0).unwrap();
        db.conn.execute("UPDATE trainers SET modified_ranks = 7 WHERE trainer_name='Atkus'", []).unwrap();
        let t = db.begin_scan_token().unwrap();
        db.mark_scan_write(t).unwrap();
        db.upsert_trainer_rank_scan(id, "Atkus", "2024-01-01 10:00:00", 1.0).unwrap();
        let last = db.get_trainers_scoped(id, ScanScope::LastScan).unwrap();
        assert_eq!(last.len(), 1);
        assert_eq!((last[0].ranks, last[0].modified_ranks), (1, 0));
        let all = db.get_trainers_scoped(id, ScanScope::All).unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!((all[0].ranks, all[0].modified_ranks), (1, 7));
    }

    #[test]
    fn scan_upserts_do_not_touch_lifetime_tables() {
        let db = Database::open_in_memory().unwrap();
        let id = db.get_or_create_character("Fen").unwrap();
        db.upsert_kill_scan(id, "Rat", "killed_count", 2, "2024-01-01").unwrap();
        db.upsert_trainer_rank_scan(id, "Atkus", "2024-01-01", 1.0).unwrap();
        db.upsert_apply_learning_scan(id, "Atkus", "2024-01-01", 10, 1.0).unwrap();
        assert_eq!(count(&db, "kills"), 0);
        assert_eq!(count(&db, "trainers"), 0);
        assert_eq!(count(&db, "scan_kills"), 1);
        assert_eq!(count(&db, "scan_trainers"), 1);
    }

    #[test]
    fn apply_learning_scan_writes_shadow_only() {
        let db = Database::open_in_memory().unwrap();
        let id = db.get_or_create_character("Fen").unwrap();
        db.upsert_apply_learning_scan(id, "Atkus", "2024-01-01", 10, 1.0).unwrap();
        db.upsert_apply_learning_scan(id, "Atkus", "2024-01-02", 10, 1.0).unwrap();
        assert!(db.get_trainers_scoped(id, ScanScope::All).unwrap().is_empty());
        let t = db.get_trainers_scoped(id, ScanScope::LastScan).unwrap();
        assert_eq!(t.len(), 1);
        assert_eq!(t[0].apply_learning_ranks, 20);
    }

    #[test]
    fn scan_verb_dates_min_first_max_last() {
        let db = Database::open_in_memory().unwrap();
        let id = db.get_or_create_character("Fen").unwrap();
        db.upsert_kill_scan(id, "Rat", "killed_count", 2, "2024-05-01 10:00:00").unwrap();
        db.upsert_kill_scan(id, "Rat", "killed_count", 2, "2024-01-01 10:00:00").unwrap();
        let k = db.get_kills_scoped(id, ScanScope::LastScan).unwrap();
        assert_eq!(k.len(), 1);
        assert_eq!(k[0].date_first_killed.as_deref(), Some("2024-01-01 10:00:00"));
        assert_eq!(k[0].date_last_killed.as_deref(), Some("2024-05-01 10:00:00"));
    }

    #[test]
    fn legacy_db_gains_shadow_tables_on_migrate() {
        let db = Database::open_in_memory().unwrap();
        let id = db.get_or_create_character("Fen").unwrap();
        db.conn.execute_batch("DROP TABLE scan_kills; DROP TABLE scan_trainers;").unwrap();
        crate::db::schema::migrate_tables(&db.conn).unwrap();
        db.upsert_kill_scan(id, "Rat", "killed_count", 2, "2024-01-01").unwrap();
        db.upsert_trainer_rank_scan(id, "Atkus", "2024-01-01", 1.0).unwrap();
        assert_eq!(count(&db, "scan_kills"), 1);
        assert_eq!(count(&db, "scan_trainers"), 1);
    }
}
