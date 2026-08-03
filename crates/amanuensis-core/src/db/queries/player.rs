use rusqlite::{params, OptionalExtension};

use crate::error::Result;
use crate::parser::player_signals::PlayerSignal;
use super::Database;

/// SQL fragment mapping an evidence string to its precedence rank.
/// Must stay in sync with `PlayerSignal::rank`.
const EVIDENCE_RANK: &str =
    "CASE ? WHEN 'ledger' THEN 4 WHEN 'offer' THEN 3 WHEN 'thinks' THEN 2 WHEN 'sharing' THEN 1 ELSE 0 END";

impl Database {
    /// Record `name` as a known player character.
    ///
    /// Names are stored lowercased so the primary key doubles as a case-insensitive index.
    /// `evidence` is only overwritten by a strictly stronger signal; `first_seen` is never
    /// overwritten, so it keeps the earliest observation.
    pub fn upsert_known_player(
        &self,
        name: &str,
        signal: PlayerSignal,
        first_seen: &str,
    ) -> Result<()> {
        let key = name.trim().to_lowercase();
        if key.is_empty() {
            return Ok(());
        }
        let sql = format!(
            "INSERT INTO known_players (name, evidence, first_seen)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(name) DO UPDATE SET evidence = ?2
             WHERE ({new_rank}) > ({old_rank})",
            new_rank = EVIDENCE_RANK.replacen('?', "?2", 1),
            old_rank = EVIDENCE_RANK.replacen('?', "known_players.evidence", 1),
        );
        self.conn
            .execute(&sql, params![key, signal.as_str(), first_seen])?;
        Ok(())
    }

    /// True if `name` has been observed doing something only a player can do.
    pub fn is_known_player(&self, name: &str) -> Result<bool> {
        let key = name.trim().to_lowercase();
        if key.is_empty() {
            return Ok(false);
        }
        let found: Option<i64> = self
            .conn
            .query_row(
                "SELECT 1 FROM known_players WHERE name = ?1",
                params![key],
                |row| row.get(0),
            )
            .optional()?;
        Ok(found.is_some())
    }

    /// All known players as `(lowercased_name, evidence)`, sorted by name.
    pub fn list_known_players(&self) -> Result<Vec<(String, String)>> {
        let mut stmt = self
            .conn
            .prepare("SELECT name, evidence FROM known_players ORDER BY name")?;
        let rows = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    /// Store a small key/value fact about this database (migration/backfill markers).
    pub fn set_db_meta(&self, key: &str, value: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO db_meta (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = ?2",
            params![key, value],
        )?;
        Ok(())
    }

    /// One-time recovery of `known_players` from the already-indexed `log_lines` table.
    ///
    /// Existing databases store every log line, so historic players can be identified
    /// without asking the user to Rescan. Guarded by a `db_meta` marker so it runs once.
    /// If log indexing was disabled the sweep finds nothing and `known_players` instead
    /// fills during the next scan.
    pub fn backfill_known_players(&self) -> Result<usize> {
        if self.get_db_meta("known_players_backfilled")?.is_some() {
            return Ok(0);
        }

        // (lowercased name) -> (strongest signal, first_seen for that observation)
        let mut found: std::collections::HashMap<String, (PlayerSignal, String)> =
            std::collections::HashMap::new();

        {
            let mut stmt = self.conn.prepare("SELECT content, timestamp FROM log_lines")?;
            let rows = stmt.query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?;

            let mut scratch: Vec<(String, PlayerSignal)> = Vec::new();
            for row in rows {
                let (content, timestamp) = match row {
                    Ok(v) => v,
                    Err(_) => continue,
                };
                // log_lines.content keeps the raw "MM/DD/YY H:MM:SSa " prefix.
                let message = crate::parser::strip_log_timestamp(&content);

                scratch.clear();
                crate::parser::player_signals::detect_player_signals(message, &mut scratch);
                for (name, signal) in scratch.drain(..) {
                    if crate::data::is_known_npc_trainer(&name) {
                        continue;
                    }
                    let key = name.to_lowercase();
                    match found.get_mut(&key) {
                        Some((existing, _)) if existing.rank() >= signal.rank() => {}
                        Some(slot) => slot.0 = signal,
                        None => {
                            found.insert(key, (signal, timestamp.clone()));
                        }
                    }
                }
            }
        }

        for (name, (signal, first_seen)) in &found {
            self.upsert_known_player(name, *signal, first_seen)?;
        }

        self.set_db_meta("known_players_backfilled", "1")?;
        Ok(found.len())
    }

    /// Read a `db_meta` value, or `None` if unset.
    pub fn get_db_meta(&self, key: &str) -> Result<Option<String>> {
        let value: Option<String> = self
            .conn
            .query_row(
                "SELECT value FROM db_meta WHERE key = ?1",
                params![key],
                |row| row.get(0),
            )
            .optional()?;
        Ok(value)
    }
}

#[cfg(test)]
mod tests {
    use super::super::Database;
    use crate::parser::player_signals::PlayerSignal;

    #[test]
    fn upsert_and_lookup_is_case_insensitive() {
        let db = Database::open_in_memory().unwrap();
        db.upsert_known_player("Fenwick", PlayerSignal::Clanning, "2017-11-22 23:07:16").unwrap();

        assert!(db.is_known_player("Fenwick").unwrap());
        assert!(db.is_known_player("fenwick").unwrap());
        assert!(db.is_known_player("  FENWICK ").unwrap());
        assert!(!db.is_known_player("Histia").unwrap());
    }

    #[test]
    fn stronger_evidence_overwrites_weaker() {
        let db = Database::open_in_memory().unwrap();
        db.upsert_known_player("Fenwick", PlayerSignal::Clanning, "2017-01-01 00:00:00").unwrap();
        db.upsert_known_player("Fenwick", PlayerSignal::Ledger, "2017-06-01 00:00:00").unwrap();

        let players = db.list_known_players().unwrap();
        assert_eq!(players, vec![("fenwick".to_string(), "ledger".to_string())]);
    }

    #[test]
    fn weaker_evidence_does_not_overwrite_stronger() {
        let db = Database::open_in_memory().unwrap();
        db.upsert_known_player("Fenwick", PlayerSignal::Ledger, "2017-01-01 00:00:00").unwrap();
        db.upsert_known_player("Fenwick", PlayerSignal::Clanning, "2017-06-01 00:00:00").unwrap();

        let players = db.list_known_players().unwrap();
        assert_eq!(players, vec![("fenwick".to_string(), "ledger".to_string())]);
    }

    #[test]
    fn first_seen_is_not_overwritten() {
        let db = Database::open_in_memory().unwrap();
        db.upsert_known_player("Fenwick", PlayerSignal::Clanning, "2017-01-01 00:00:00").unwrap();
        db.upsert_known_player("Fenwick", PlayerSignal::Ledger, "2019-06-01 00:00:00").unwrap();

        let first_seen: String = db
            .conn()
            .query_row("SELECT first_seen FROM known_players WHERE name='fenwick'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(first_seen, "2017-01-01 00:00:00");
    }

    #[test]
    fn db_meta_roundtrips() {
        let db = Database::open_in_memory().unwrap();
        assert_eq!(db.get_db_meta("missing").unwrap(), None);
        db.set_db_meta("k", "v").unwrap();
        assert_eq!(db.get_db_meta("k").unwrap(), Some("v".to_string()));
        db.set_db_meta("k", "v2").unwrap();
        assert_eq!(db.get_db_meta("k").unwrap(), Some("v2".to_string()));
    }

    fn insert_log_line(db: &Database, char_id: i64, content: &str) {
        db.conn()
            .execute(
                "INSERT INTO log_lines (content, character_id, timestamp, file_path)
                 VALUES (?1, ?2, ?3, ?4)",
                rusqlite::params![content, char_id, "2017-11-22 23:07:16", "CL Log 11.22.17"],
            )
            .unwrap();
    }

    #[test]
    fn backfill_finds_players_in_stored_log_lines() {
        let db = Database::open_in_memory().unwrap();
        let char_id = db.get_or_create_character("Ruuk").unwrap();

        insert_log_line(&db, char_id, "11/22/17 11:07:16p Fenwick is now Clanning.");
        insert_log_line(&db, char_id, "11/1/18 1:17:31p Bramwell Gorse writes your name in her training ledger.");
        insert_log_line(&db, char_id, r#"11/22/17 11:07:16p Fenwick says, "Hail, Ruuk. You are one of my better pupils.""#);
        insert_log_line(&db, char_id, r#"11/22/17 11:11:59p Duvin Beastlore says, "You show great devotion to your studies.""#);

        let inserted = db.backfill_known_players().unwrap();
        assert_eq!(inserted, 2);

        assert!(db.is_known_player("Fenwick").unwrap());
        assert!(db.is_known_player("Bramwell Gorse").unwrap());
        assert!(!db.is_known_player("Duvin Beastlore").unwrap());
    }

    #[test]
    fn backfill_never_flags_a_guarded_npc_trainer() {
        let db = Database::open_in_memory().unwrap();
        let char_id = db.get_or_create_character("Ruuk").unwrap();
        // Contrived: even if an NPC name appeared in a player-only line, the guard wins.
        insert_log_line(&db, char_id, "11/22/17 11:07:16p Higgrus is now Clanning.");

        assert_eq!(db.backfill_known_players().unwrap(), 0);
        assert!(!db.is_known_player("Higgrus").unwrap());
    }

    #[test]
    fn backfill_is_idempotent() {
        let db = Database::open_in_memory().unwrap();
        let char_id = db.get_or_create_character("Ruuk").unwrap();
        insert_log_line(&db, char_id, "11/22/17 11:07:16p Fenwick is now Clanning.");

        assert_eq!(db.backfill_known_players().unwrap(), 1);
        assert_eq!(db.backfill_known_players().unwrap(), 0, "marker should short-circuit");
        assert_eq!(db.list_known_players().unwrap().len(), 1);
    }

    #[test]
    fn backfill_keeps_strongest_evidence_across_lines() {
        let db = Database::open_in_memory().unwrap();
        let char_id = db.get_or_create_character("Ruuk").unwrap();
        insert_log_line(&db, char_id, "11/22/17 11:07:16p Fenwick is now Clanning.");
        insert_log_line(&db, char_id, "12/8/17 9:11:48a Fenwick writes Thistledown's name in her training ledger.");

        db.backfill_known_players().unwrap();
        assert_eq!(db.list_known_players().unwrap(), vec![("fenwick".to_string(), "ledger".to_string())]);
    }

    #[test]
    fn reset_log_data_clears_players_and_checkpoints() {
        let db = Database::open_in_memory().unwrap();
        let char_id = db.get_or_create_character("Ruuk").unwrap();
        db.insert_trainer_checkpoint(char_id, "Histia", 0, Some(9), "2024-01-01 12:00:00").unwrap();
        db.upsert_known_player("Fenwick", PlayerSignal::Clanning, "2024-01-01 12:00:00").unwrap();
        db.set_db_meta("known_players_backfilled", "1").unwrap();

        db.reset_log_data().unwrap();

        assert!(db.list_known_players().unwrap().is_empty());
        assert_eq!(db.get_db_meta("known_players_backfilled").unwrap(), None);

        // Asserted via raw SQL rather than get_all_trainer_checkpoints, which does not
        // gain its `include_players` parameter until Task 6.
        let remaining: i64 = db
            .conn()
            .query_row("SELECT COUNT(*) FROM trainer_checkpoints", [], |r| r.get(0))
            .unwrap();
        assert_eq!(remaining, 0);
    }
}
