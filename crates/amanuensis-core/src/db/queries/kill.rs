use rusqlite::params;

use crate::error::Result;
use crate::models::Kill;
use super::Database;

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
            // NULLIF ensures empty strings are treated as NULL so valid dates always win.
            let date_update =
                ", date_first = COALESCE(NULLIF(kills.date_first, ''), NULLIF(excluded.date_first, '')), \
                   date_last = NULLIF(COALESCE(NULLIF(excluded.date_last, ''), kills.date_last), '')";

            let sql = format!(
                "INSERT INTO kills (character_id, creature_name, {field}, creature_value, date_first, date_last{date_col_insert})
                 VALUES (?1, ?2, 1, ?3, NULLIF(?4, ''), NULLIF(?4, ''){date_col_value})
                 ON CONFLICT(character_id, creature_name) DO UPDATE SET
                    {field} = {field} + 1,
                    creature_value = MAX(kills.creature_value, excluded.creature_value){date_update}{date_col_update}",
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
                    killed_by_count, date_first, date_last, creature_value,
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
                date_last_killed: row.get(15)?,
                date_last_slaughtered: row.get(16)?,
                date_last_vanquished: row.get(17)?,
                date_last_dispatched: row.get(18)?,
                best_loot_value: row.get(19)?,
                best_loot_item: row.get(20)?,
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

    /// Compute coin level as the highest creature_value among all personal kills
    /// (killed, slaughtered, vanquished, or dispatched — not assists or deaths).
    /// Returns 0 if no personal kills of meaningful value recorded.
    pub fn compute_coin_level_from_kills(&self, char_id: i64) -> Result<i64> {
        self.compute_coin_level_for_char_ids(&[char_id])
    }

    /// Compute coin level across a set of character IDs (for merged characters).
    pub fn compute_coin_level_for_char_ids(&self, char_ids: &[i64]) -> Result<i64> {
        let placeholders = char_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let sql = format!(
            "SELECT COALESCE(MAX(creature_value), 0) FROM kills
             WHERE character_id IN ({placeholders})
               AND (killed_count + slaughtered_count + vanquished_count + dispatched_count) > 0
               AND creature_value >= {min}",
            min = Self::COIN_LEVEL_MIN_VALUE,
        );
        let result: i64 = self.conn.query_row(
            &sql,
            rusqlite::params_from_iter(char_ids.iter()),
            |row| row.get(0),
        )?;
        Ok(result)
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
