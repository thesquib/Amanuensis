use rusqlite::params;

use crate::data::{canonical_rarity, CreatureDb};
use crate::error::Result;
use crate::models::Kill;
use super::Database;

#[derive(Debug, Clone, Default)]
pub struct KillsFilter {
    pub family: Option<String>,
    pub rarity: Option<String>,
    pub seasonal: Option<bool>,
}

/// Filter a slice of kills against the bestiary using family / rarity / seasonal predicates.
/// Returns owned clones for the matched kills.
pub fn filter_kills(kills: &[Kill], db: &CreatureDb, filter: &KillsFilter) -> Vec<Kill> {
    if filter.family.is_none() && filter.rarity.is_none() && filter.seasonal.is_none() {
        return kills.to_vec();
    }
    kills
        .iter()
        .filter(|k| {
            let entry = db.get_entry(&k.creature_name);
            if let Some(want) = &filter.family {
                let raw = entry.and_then(|e| e.family.as_deref()).unwrap_or("");
                if !db.canonical_family(raw).eq_ignore_ascii_case(want) {
                    return false;
                }
            }
            if let Some(want) = &filter.rarity {
                let r = canonical_rarity(entry.and_then(|e| e.rarity.as_deref()));
                if !r.as_label().eq_ignore_ascii_case(want) {
                    return false;
                }
            }
            if let Some(want) = filter.seasonal {
                let s = entry.map(|e| e.is_seasonal).unwrap_or(false);
                if s != want {
                    return false;
                }
            }
            true
        })
        .cloned()
        .collect()
}

impl Database {
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

        // Determine the per-type date column to update (solo and assisted share the same date column
        // so that date_last_vanquished etc. reflect ANY vanquish, whether solo or assisted)
        let date_col = match field {
            "killed_count" | "assisted_kill_count" => Some("date_last_killed"),
            "slaughtered_count" | "assisted_slaughter_count" => Some("date_last_slaughtered"),
            "vanquished_count" | "assisted_vanquish_count" => Some("date_last_vanquished"),
            "dispatched_count" | "assisted_dispatch_count" => Some("date_last_dispatched"),
            _ => None,
        };

        let date_col_insert = date_col.map(|c| format!(", {c}")).unwrap_or_default();
        let date_col_value = if date_col.is_some() { ", ?4" } else { "" };
        let date_col_update = date_col
            .map(|c| format!(", {c} = NULLIF(MAX(COALESCE(kills.{c}, ''), COALESCE(excluded.{c}, '')), '')"))
            .unwrap_or_default();

        // Track the first-ever date for this verb (earliest), in its own per-verb column.
        // Uses MIN over the two non-empty candidates so out-of-order scans (or appended
        // tail scans) still settle on the truly-earliest date for each verb.
        let date_first_col = match field {
            "killed_count" | "assisted_kill_count" => Some("date_first_killed"),
            "slaughtered_count" | "assisted_slaughter_count" => Some("date_first_slaughtered"),
            "vanquished_count" | "assisted_vanquish_count" => Some("date_first_vanquished"),
            "dispatched_count" | "assisted_dispatch_count" => Some("date_first_dispatched"),
            _ => None,
        };
        let date_first_col_insert = date_first_col.map(|c| format!(", {c}")).unwrap_or_default();
        let date_first_col_value = if date_first_col.is_some() { ", NULLIF(?4, '')" } else { "" };
        let date_first_col_update = date_first_col
            .map(|c| format!(
                ", {c} = NULLIF(MIN(\
                    COALESCE(NULLIF(kills.{c}, ''), excluded.{c}), \
                    COALESCE(NULLIF(excluded.{c}, ''), kills.{c})\
                  ), '')"
            ))
            .unwrap_or_default();

        let is_death = field == "killed_by_count";

        if is_death {
            // Death events: insert NULL for date_first/date_last (these track kills only)
            let sql = format!(
                "INSERT INTO kills (character_id, creature_name, {field}, creature_value)
                 VALUES (?1, ?2, 1, ?3)
                 ON CONFLICT(character_id, creature_name) DO UPDATE SET
                    {field} = {field} + 1,
                    creature_value = MAX(kills.creature_value, excluded.creature_value)",
            );
            self.conn.execute(
                &sql,
                params![char_id, creature_name, creature_value],
            )?;
        } else {
            // Kill events: set dates, backfill date_first if NULL or empty string.
            // date_last uses MAX so that scan order never causes an older date to overwrite a newer one.
            // Dates are stored as "YYYY-MM-DD HH:MM:SS" which is lexicographically sortable.
            let date_update =
                ", date_first = COALESCE(NULLIF(kills.date_first, ''), NULLIF(excluded.date_first, '')), \
                   date_last = NULLIF(MAX(COALESCE(kills.date_last, ''), COALESCE(excluded.date_last, '')), '')";

            let sql = format!(
                "INSERT INTO kills (character_id, creature_name, {field}, creature_value, date_first, date_last{date_col_insert}{date_first_col_insert})
                 VALUES (?1, ?2, 1, ?3, NULLIF(?4, ''), NULLIF(?4, ''){date_col_value}{date_first_col_value})
                 ON CONFLICT(character_id, creature_name) DO UPDATE SET
                    {field} = {field} + 1,
                    creature_value = MAX(kills.creature_value, excluded.creature_value){date_update}{date_col_update}{date_first_col_update}",
            );
            self.conn.execute(
                &sql,
                params![char_id, creature_name, creature_value, date],
            )?;
        }
        Ok(())
    }

    /// Increment the per-hour verb count for a (character, creature, hour) bucket.
    /// `field` is one of the kill-verb count columns (same names as `kills`, minus killed_by).
    /// `hour` is "YYYY-MM-DD HH".
    pub fn upsert_kill_hourly(
        &self,
        char_id: i64,
        creature_name: &str,
        field: &str,
        hour: &str,
    ) -> Result<()> {
        let allowed = [
            "killed_count", "slaughtered_count", "vanquished_count", "dispatched_count",
            "assisted_kill_count", "assisted_slaughter_count", "assisted_vanquish_count",
            "assisted_dispatch_count",
        ];
        if !allowed.contains(&field) {
            return Err(crate::error::AmanuensisError::Data(format!(
                "Unknown kill_hourly field: {}",
                field
            )));
        }
        let sql = format!(
            "INSERT INTO kill_hourly (character_id, creature_name, hour, {field})
             VALUES (?1, ?2, ?3, 1)
             ON CONFLICT(character_id, creature_name, hour) DO UPDATE SET {field} = {field} + 1",
        );
        self.conn.execute(&sql, params![char_id, creature_name, hour])?;
        Ok(())
    }

    /// Get kills for a character, ordered by total count descending.
    pub fn get_kills(&self, char_id: i64) -> Result<Vec<Kill>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, character_id, creature_name,
                    killed_count, slaughtered_count, vanquished_count, dispatched_count,
                    assisted_kill_count, assisted_slaughter_count, assisted_vanquish_count, assisted_dispatch_count,
                    killed_by_count, date_first, date_last, creature_value,
                    date_first_killed, date_first_slaughtered, date_first_vanquished, date_first_dispatched,
                    date_last_killed, date_last_slaughtered, date_last_vanquished, date_last_dispatched,
                    COALESCE(best_loot_value, 0), COALESCE(best_loot_item, '')
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
                date_first_killed: row.get(15)?,
                date_first_slaughtered: row.get(16)?,
                date_first_vanquished: row.get(17)?,
                date_first_dispatched: row.get(18)?,
                date_last_killed: row.get(19)?,
                date_last_slaughtered: row.get(20)?,
                date_last_vanquished: row.get(21)?,
                date_last_dispatched: row.get(22)?,
                best_loot_value: row.get(23)?,
                best_loot_item: row.get(24)?,
            })
        })?;

        Ok(kills.filter_map(|r| r.ok()).collect())
    }

    /// Update the best single-loot recovery for a creature if the new value beats the existing one.
    /// Only updates if the creature already has a kills record (no-op otherwise).
    pub fn update_kill_best_loot(
        &self,
        char_id: i64,
        creature_name: &str,
        loot_value: i64,
        loot_item: &str,
    ) -> Result<()> {
        self.conn.execute(
            "UPDATE kills SET
                best_loot_item = CASE WHEN ?3 > best_loot_value THEN ?4 ELSE best_loot_item END,
                best_loot_value = MAX(best_loot_value, ?3)
             WHERE character_id = ?1 AND creature_name = ?2",
            params![char_id, creature_name, loot_value, loot_item],
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

    /// Minimum creature value that counts toward coin level.
    /// Excludes trivial low-level creatures (rats worth 2, jellyfish worth 8, etc.).
    const COIN_LEVEL_MIN_VALUE: i32 = 50;

    /// Minimum kill-verb count (solo + assisted) required for a creature to contribute to coin level.
    /// Prevents one-off GM spawns or unusual encounters from skewing the value.
    const COIN_LEVEL_MIN_KILLS: i64 = 5;

    /// Compute coin level as the highest creature_value among creatures killed (verb: kill/killed)
    /// at least COIN_LEVEL_MIN_KILLS times (solo + assisted). Returns 0 if none qualify.
    pub fn compute_coin_level_from_kills(&self, char_id: i64) -> Result<i64> {
        self.compute_coin_level_for_char_ids(&[char_id])
    }

    /// Compute interim coin level: best kill-verb creature with ≥1 kill when the
    /// reliable threshold (≥5) is not yet met. Returns 0 if coin_level is already > 0.
    pub fn compute_interim_coin_level_from_kills(&self, char_id: i64) -> Result<i64> {
        self.compute_interim_coin_level_for_char_ids(&[char_id])
    }

    pub fn compute_interim_coin_level_for_char_ids(&self, char_ids: &[i64]) -> Result<i64> {
        let placeholders = char_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        // No min_value floor — any kill-verb creature with a positive value qualifies,
        // so low-level characters like Helga aren't left showing 0.
        let sql = format!(
            "SELECT COALESCE(MAX(creature_value), 0) FROM kills
             WHERE character_id IN ({placeholders})
               AND (killed_count + assisted_kill_count) >= 1
               AND creature_value > 0",
        );
        let result: i64 = self.conn.query_row(
            &sql,
            rusqlite::params_from_iter(char_ids.iter()),
            |row| row.get(0),
        )?;
        Ok(result)
    }

    /// Compute coin level across a set of character IDs (for merged characters).
    pub fn compute_coin_level_for_char_ids(&self, char_ids: &[i64]) -> Result<i64> {
        let placeholders = char_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let sql = format!(
            "SELECT COALESCE(MAX(creature_value), 0) FROM kills
             WHERE character_id IN ({placeholders})
               AND (killed_count + assisted_kill_count) >= {min_kills}
               AND creature_value >= {min_val}",
            min_kills = Self::COIN_LEVEL_MIN_KILLS,
            min_val = Self::COIN_LEVEL_MIN_VALUE,
        );
        let result: i64 = self.conn.query_row(
            &sql,
            rusqlite::params_from_iter(char_ids.iter()),
            |row| row.get(0),
        )?;
        Ok(result)
    }

    /// Returns the set of creature names this character has encountered. A creature is
    /// "encountered" if it appears in `kills` with any positive solo/assisted/killed_by count,
    /// or in `lastys`.
    pub fn get_encountered_creatures(&self, char_id: i64) -> Result<std::collections::HashSet<String>> {
        let mut out = std::collections::HashSet::new();

        let mut kill_stmt = self.conn.prepare(
            "SELECT creature_name FROM kills WHERE character_id = ?1 AND \
             (killed_count + slaughtered_count + vanquished_count + dispatched_count + \
              assisted_kill_count + assisted_slaughter_count + assisted_vanquish_count + \
              assisted_dispatch_count + killed_by_count) > 0",
        )?;
        let kill_iter = kill_stmt.query_map([char_id], |row| row.get::<_, String>(0))?;
        for name in kill_iter {
            out.insert(name?);
        }

        let mut lasty_stmt = self
            .conn
            .prepare("SELECT creature_name FROM lastys WHERE character_id = ?1")?;
        let lasty_iter = lasty_stmt.query_map([char_id], |row| row.get::<_, String>(0))?;
        for name in lasty_iter {
            out.insert(name?);
        }

        Ok(out)
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
    fn test_kills_filter_helper() {
        use crate::data::CreatureDb;

        let db = CreatureDb::bundled().unwrap();

        // Build kills with known creatures.
        let kills = vec![
            Kill::new(0, "Rat".into(), 2),
            Kill::new(0, "Tesla".into(), 70),
            Kill::new(0, "Barracuda".into(), 250),
        ];

        // Family filter
        let filtered: Vec<_> = filter_kills(
            &kills,
            &db,
            &KillsFilter {
                family: Some("Vermine".into()),
                rarity: None,
                seasonal: None,
            },
        );
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].creature_name, "Rat");

        // Rarity filter
        let filtered: Vec<_> = filter_kills(
            &kills,
            &db,
            &KillsFilter {
                family: None,
                rarity: Some("Medium".into()),
                seasonal: None,
            },
        );
        assert!(filtered.iter().any(|k| k.creature_name == "Barracuda"));

        // Combined: family + rarity (Rat is Vermine + Common; expect Rat with family Vermine + Common rarity)
        let filtered: Vec<_> = filter_kills(
            &kills,
            &db,
            &KillsFilter {
                family: Some("Vermine".into()),
                rarity: Some("Common".into()),
                seasonal: None,
            },
        );
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].creature_name, "Rat");
    }

    #[test]
    fn filter_kills_matches_on_canonical_rarity_and_family() {
        use crate::data::{BestiaryEntry, BestiaryFile, CreatureDb};

        let file = BestiaryFile {
            version: "20260101".into(),
            entries: vec![
                BestiaryEntry {
                    name: "Punctus".into(),
                    family: Some("Extinct".into()),
                    rarity: Some("Common.".into()),
                    exp_taxidermy: 1,
                    ..BestiaryEntry::default()
                },
                BestiaryEntry {
                    name: "Wussy".into(),
                    family: Some("EXTINCT".into()),
                    rarity: Some("Extinct.".into()),
                    exp_taxidermy: 1,
                    ..BestiaryEntry::default()
                },
            ],
        };
        let bestiary_json = serde_json::to_vec(&file).unwrap();
        let db = CreatureDb::from_json_bytes(&bestiary_json, b"[]").unwrap();

        let kills = vec![
            Kill::new(0, "Punctus".into(), 1),
            Kill::new(0, "Wussy".into(), 1),
        ];

        // "Common." resolves to the canonical "Common" bucket.
        let common = filter_kills(
            &kills,
            &db,
            &KillsFilter {
                family: None,
                rarity: Some("Common".into()),
                seasonal: None,
            },
        );
        assert_eq!(common.len(), 1);
        assert_eq!(common[0].creature_name, "Punctus");

        // "Extinct." resolves to the canonical "Unique" bucket.
        let unique = filter_kills(
            &kills,
            &db,
            &KillsFilter {
                family: None,
                rarity: Some("Unique".into()),
                seasonal: None,
            },
        );
        assert_eq!(unique.len(), 1);
        assert_eq!(unique[0].creature_name, "Wussy");

        // "EXTINCT" and "Extinct" are the same canonical family.
        let extinct = filter_kills(
            &kills,
            &db,
            &KillsFilter {
                family: Some("Extinct".into()),
                rarity: None,
                seasonal: None,
            },
        );
        assert_eq!(extinct.len(), 2);
    }

    #[test]
    fn kill_hourly_table_exists_and_reset_clears_it() {
        let db = Database::open_in_memory().unwrap();
        let char_id = db.get_or_create_character("Tester").unwrap();

        db.conn()
            .execute(
                "INSERT INTO kill_hourly (character_id, creature_name, hour, killed_count)
                 VALUES (?1, 'Rat', '2024-01-01 10', 1)",
                rusqlite::params![char_id],
            )
            .unwrap();

        let count: i64 = db
            .conn()
            .query_row("SELECT COUNT(*) FROM kill_hourly", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1);

        db.reset_log_data().unwrap();

        let after: i64 = db
            .conn()
            .query_row("SELECT COUNT(*) FROM kill_hourly", [], |r| r.get(0))
            .unwrap();
        assert_eq!(after, 0, "reset_log_data must clear kill_hourly");
    }

    #[test]
    fn upsert_kill_hourly_accumulates() {
        let db = Database::open_in_memory().unwrap();
        let char_id = db.get_or_create_character("Tester").unwrap();

        db.upsert_kill_hourly(char_id, "Rat", "slaughtered_count", "2024-01-01 10")
            .unwrap();
        db.upsert_kill_hourly(char_id, "Rat", "slaughtered_count", "2024-01-01 10")
            .unwrap();
        db.upsert_kill_hourly(char_id, "Rat", "killed_count", "2024-01-01 10")
            .unwrap();

        let total: i64 = db
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM kill_hourly WHERE character_id = ?1 AND creature_name = 'Rat'",
                rusqlite::params![char_id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(total, 1, "expected a single accumulated bucket row");

        let (slaughtered, killed): (i64, i64) = db
            .conn()
            .query_row(
                "SELECT slaughtered_count, killed_count FROM kill_hourly WHERE creature_name = 'Rat'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(slaughtered, 2);
        assert_eq!(killed, 1);
    }

    #[test]
    fn upsert_kill_tracks_first_date_per_verb() {
        let db = Database::open_in_memory().unwrap();
        let char_id = db.get_or_create_character("Tester").unwrap();

        // Slaughter on a later date first, then an earlier one — first-date must be the MIN.
        db.upsert_kill(char_id, "Rat", "slaughtered_count", 2, "2024-03-01 10:00:00").unwrap();
        db.upsert_kill(char_id, "Rat", "slaughtered_count", 2, "2024-01-01 09:00:00").unwrap();
        db.upsert_kill(char_id, "Rat", "vanquished_count", 2, "2024-05-05 12:00:00").unwrap();
        db.upsert_kill(char_id, "Rat", "dispatched_count", 2, "2024-06-06 06:00:00").unwrap();
        db.upsert_kill(char_id, "Rat", "killed_count", 2, "2024-02-02 02:00:00").unwrap();

        let kills = db.get_kills(char_id).unwrap();
        let rat = kills.iter().find(|k| k.creature_name == "Rat").unwrap();
        assert_eq!(rat.date_first_killed.as_deref(), Some("2024-02-02 02:00:00"));
        assert_eq!(rat.date_first_slaughtered.as_deref(), Some("2024-01-01 09:00:00"));
        assert_eq!(rat.date_first_vanquished.as_deref(), Some("2024-05-05 12:00:00"));
        assert_eq!(rat.date_first_dispatched.as_deref(), Some("2024-06-06 06:00:00"));
    }

    #[test]
    fn test_get_encountered_creatures() {
        let db = Database::open_in_memory().unwrap();
        let char_id = db.get_or_create_character("Tester").unwrap();

        // Insert two kills with positive counts.
        db.upsert_kill(char_id, "Rat", "killed_count", 2, "2024-01-01").unwrap();
        db.upsert_kill(char_id, "Wolf", "assisted_kill_count", 50, "2024-01-02").unwrap();

        // Insert a zero-count kills row for "Bat" directly (no public API can create a zero row).
        db.conn().execute(
            "INSERT INTO kills (character_id, creature_name, creature_value) VALUES (?1, 'Bat', 0)",
            rusqlite::params![char_id],
        ).unwrap();

        // Insert one lasty for a creature not in kills.
        db.upsert_lasty(char_id, "Tesla", "Movements", "2024-01-01").unwrap();

        let encountered = db.get_encountered_creatures(char_id).unwrap();
        assert!(encountered.contains("Rat"));
        assert!(encountered.contains("Wolf"));
        assert!(encountered.contains("Tesla"));
        assert!(!encountered.contains("Bat"));
    }
}
