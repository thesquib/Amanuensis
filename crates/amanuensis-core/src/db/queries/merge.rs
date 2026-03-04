use rusqlite::params;

use crate::error::Result;
use crate::models::{Character, Kill, Lasty, Pet, Trainer};
use super::{CHARACTER_COLUMNS, map_character_row, Database};

impl Database {
    /// Get all character IDs that have been merged into the given target.
    fn merged_source_ids(&self, target_id: i64) -> Result<Vec<i64>> {
        let mut stmt = self.conn.prepare(
            "SELECT id FROM characters WHERE merged_into = ?1",
        )?;
        let ids = stmt
            .query_map(params![target_id], |row| row.get::<_, i64>(0))?
            .filter_map(|r| r.ok())
            .collect();
        Ok(ids)
    }

    /// Build a list containing the target character ID plus all merged source IDs.
    fn char_ids_for_merged(&self, char_id: i64) -> Result<Vec<i64>> {
        let mut ids = vec![char_id];
        ids.extend(self.merged_source_ids(char_id)?);
        Ok(ids)
    }

    /// Merge one or more source characters into a target character.
    /// Sets `merged_into = target_id` for each source. Recalculates target's profession and coin_level.
    /// Runs in a transaction for atomicity.
    pub fn merge_characters(&self, source_ids: &[i64], target_id: i64) -> Result<()> {
        self.begin_transaction()?;
        match self.merge_characters_inner(source_ids, target_id) {
            Ok(()) => { self.commit_transaction()?; Ok(()) }
            Err(e) => { let _ = self.rollback_transaction(); Err(e) }
        }
    }

    fn merge_characters_inner(&self, source_ids: &[i64], target_id: i64) -> Result<()> {
        // Validate target exists and is not itself merged
        let target_merged: Option<Option<i64>> = self.conn.query_row(
            "SELECT merged_into FROM characters WHERE id = ?1",
            params![target_id],
            |row| row.get(0),
        ).ok();
        let target_merged = target_merged.ok_or_else(|| {
            crate::error::AmanuensisError::Data(format!("Target character {} not found", target_id))
        })?;
        if target_merged.is_some() {
            return Err(crate::error::AmanuensisError::Data(
                "Target character is itself merged into another character".to_string(),
            ));
        }

        // Block merge if either target or any source has non-modifier rank overrides
        let target_overrides = self.get_non_modifier_trainers(target_id)?;
        if !target_overrides.is_empty() {
            return Err(crate::error::AmanuensisError::Data(format!(
                "Cannot merge: target character has rank overrides on: {}",
                target_overrides.join(", ")
            )));
        }

        for &source_id in source_ids {
            if source_id == target_id {
                return Err(crate::error::AmanuensisError::Data(
                    "Cannot merge a character into itself".to_string(),
                ));
            }

            let source_overrides = self.get_non_modifier_trainers(source_id)?;
            if !source_overrides.is_empty() {
                return Err(crate::error::AmanuensisError::Data(format!(
                    "Cannot merge: source character {} has rank overrides on: {}",
                    source_id, source_overrides.join(", ")
                )));
            }

            // Verify source exists and is not already merged
            let source_merged: Option<Option<i64>> = self.conn.query_row(
                "SELECT merged_into FROM characters WHERE id = ?1",
                params![source_id],
                |row| row.get(0),
            ).ok();
            let source_merged = source_merged.ok_or_else(|| {
                crate::error::AmanuensisError::Data(format!(
                    "Source character {} not found", source_id
                ))
            })?;
            if source_merged.is_some() {
                return Err(crate::error::AmanuensisError::Data(format!(
                    "Source character {} is already merged into another character", source_id
                )));
            }
            self.conn.execute(
                "UPDATE characters SET merged_into = ?1 WHERE id = ?2",
                params![target_id, source_id],
            )?;
        }

        // Recalculate target's aggregated coin_level and profession
        self.recalculate_merged_stats(target_id)?;

        Ok(())
    }

    /// Unmerge a character (clear its merged_into). Recalculates the former target's stats.
    /// Runs in a transaction for atomicity.
    pub fn unmerge_character(&self, source_id: i64) -> Result<()> {
        self.begin_transaction()?;
        match self.unmerge_character_inner(source_id) {
            Ok(()) => { self.commit_transaction()?; Ok(()) }
            Err(e) => { let _ = self.rollback_transaction(); Err(e) }
        }
    }

    fn unmerge_character_inner(&self, source_id: i64) -> Result<()> {
        let former_target: Option<i64> = self.conn.query_row(
            "SELECT merged_into FROM characters WHERE id = ?1",
            params![source_id],
            |row| row.get(0),
        ).map_err(|_| {
            crate::error::AmanuensisError::Data(format!("Character {} not found", source_id))
        })?;

        let former_target = former_target.ok_or_else(|| {
            crate::error::AmanuensisError::Data(format!("Character {} is not merged", source_id))
        })?;

        self.conn.execute(
            "UPDATE characters SET merged_into = NULL WHERE id = ?1",
            params![source_id],
        )?;

        // Recalculate the former target's stats
        self.recalculate_merged_stats(former_target)?;

        Ok(())
    }

    /// Get all characters that have been merged into the given target.
    pub fn get_merge_sources(&self, target_id: i64) -> Result<Vec<Character>> {
        let sql = format!(
            "SELECT {CHARACTER_COLUMNS} FROM characters WHERE merged_into = ?1 ORDER BY name"
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let chars = stmt.query_map(params![target_id], map_character_row)?;
        Ok(chars.filter_map(|r| r.ok()).collect())
    }

    /// Get trainer names with non-modifier rank modes for a character.
    fn get_non_modifier_trainers(&self, char_id: i64) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare(
            "SELECT trainer_name FROM trainers WHERE character_id = ?1 AND rank_mode != 'modifier'",
        )?;
        let names = stmt
            .query_map(params![char_id], |row| row.get::<_, String>(0))?
            .filter_map(|r| r.ok())
            .collect();
        Ok(names)
    }

    /// Recalculate a target character's coin_level after merge/unmerge.
    /// Coin level = max creature_value across all personal kills for this character and its sources.
    fn recalculate_merged_stats(&self, target_id: i64) -> Result<()> {
        let all_ids = self.char_ids_for_merged(target_id)?;
        let coin_level = self.compute_coin_level_for_char_ids(&all_ids)?;
        self.update_coin_level(target_id, coin_level)?;
        Ok(())
    }

    /// Get kills aggregated across a character and all its merge sources.
    /// For the same creature, counts are summed; dates take min(first) and max(last).
    pub fn get_kills_merged(&self, char_id: i64) -> Result<Vec<Kill>> {
        let all_ids = self.char_ids_for_merged(char_id)?;
        if all_ids.len() == 1 {
            return self.get_kills(char_id);
        }
        let placeholders = all_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let sql = format!(
            "SELECT NULL, {}, creature_name,
                    SUM(killed_count), SUM(slaughtered_count), SUM(vanquished_count), SUM(dispatched_count),
                    SUM(assisted_kill_count), SUM(assisted_slaughter_count), SUM(assisted_vanquish_count), SUM(assisted_dispatch_count),
                    SUM(killed_by_count), MIN(date_first), MAX(date_last), MAX(creature_value),
                    MIN(date_first_killed), MAX(date_last_killed), MAX(date_last_slaughtered), MAX(date_last_vanquished), MAX(date_last_dispatched),
                    COALESCE(MAX(best_loot_value), 0),
                    COALESCE((SELECT best_loot_item FROM kills k2 WHERE k2.character_id IN ({}) AND k2.creature_name = kills.creature_name ORDER BY best_loot_value DESC LIMIT 1), '')
             FROM kills WHERE character_id IN ({})
             GROUP BY creature_name
             ORDER BY (SUM(killed_count) + SUM(slaughtered_count) + SUM(vanquished_count) + SUM(dispatched_count) +
                       SUM(assisted_kill_count) + SUM(assisted_slaughter_count) + SUM(assisted_vanquish_count) + SUM(assisted_dispatch_count)) DESC",
            char_id, placeholders, placeholders
        );
        let mut stmt = self.conn.prepare(&sql)?;
        // The SQL has two IN (?) clauses: one for the best_loot_item subquery and one for the
        // main WHERE. Supply all_ids twice so both sets of ? placeholders are bound.
        let all_params: Vec<i64> = all_ids.iter().chain(all_ids.iter()).copied().collect();
        let kills = stmt.query_map(rusqlite::params_from_iter(all_params.iter()), |row| {
            Ok(Kill {
                id: row.get(0)?,
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
                date_last_killed: row.get(16)?,
                date_last_slaughtered: row.get(17)?,
                date_last_vanquished: row.get(18)?,
                date_last_dispatched: row.get(19)?,
                best_loot_value: row.get(20)?,
                best_loot_item: row.get(21)?,
            })
        })?;
        Ok(kills.filter_map(|r| r.ok()).collect())
    }

    /// Get trainers aggregated across a character and all its merge sources.
    /// For the same trainer name: sum ranks, take max date.
    /// rank_mode and override_date come from the primary character's record.
    pub fn get_trainers_merged(&self, char_id: i64) -> Result<Vec<Trainer>> {
        let all_ids = self.char_ids_for_merged(char_id)?;
        if all_ids.len() == 1 {
            return self.get_trainers(char_id);
        }
        let placeholders = all_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let sql = format!(
            "SELECT NULL, {cid}, trainer_name,
                    SUM(ranks), SUM(modified_ranks), MAX(date_of_last_rank),
                    SUM(apply_learning_ranks), SUM(apply_learning_unknown_count),
                    MAX(CASE WHEN character_id = {cid} THEN rank_mode ELSE 'modifier' END),
                    MAX(CASE WHEN character_id = {cid} THEN override_date ELSE NULL END),
                    MAX(effective_multiplier)
             FROM trainers WHERE character_id IN ({placeholders})
             GROUP BY trainer_name
             ORDER BY SUM(ranks) DESC",
            cid = char_id, placeholders = placeholders
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let trainers = stmt.query_map(rusqlite::params_from_iter(all_ids.iter()), |row| {
            Ok(Trainer {
                id: row.get(0)?,
                character_id: row.get(1)?,
                trainer_name: row.get(2)?,
                ranks: row.get(3)?,
                modified_ranks: row.get(4)?,
                date_of_last_rank: row.get(5)?,
                apply_learning_ranks: row.get(6)?,
                apply_learning_unknown_count: row.get(7)?,
                rank_mode: row.get(8)?,
                override_date: row.get(9)?,
                effective_multiplier: row.get(10)?,
            })
        })?;
        Ok(trainers.filter_map(|r| r.ok()).collect())
    }

    /// Get pets aggregated across a character and all its merge sources (distinct by pet_name).
    pub fn get_pets_merged(&self, char_id: i64) -> Result<Vec<Pet>> {
        let all_ids = self.char_ids_for_merged(char_id)?;
        if all_ids.len() == 1 {
            return self.get_pets(char_id);
        }
        let placeholders = all_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let sql = format!(
            "SELECT MIN(id), {}, pet_name, creature_name
             FROM pets WHERE character_id IN ({})
             GROUP BY pet_name
             ORDER BY pet_name",
            char_id, placeholders
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let pets = stmt.query_map(rusqlite::params_from_iter(all_ids.iter()), |row| {
            Ok(Pet {
                id: Some(row.get(0)?),
                character_id: row.get(1)?,
                pet_name: row.get(2)?,
                creature_name: row.get(3)?,
            })
        })?;
        Ok(pets.filter_map(|r| r.ok()).collect())
    }

    /// Get lastys aggregated across a character and all its merge sources.
    /// For the same creature: keep the one with higher message_count, prefer finished=1.
    pub fn get_lastys_merged(&self, char_id: i64) -> Result<Vec<Lasty>> {
        let all_ids = self.char_ids_for_merged(char_id)?;
        if all_ids.len() == 1 {
            return self.get_lastys(char_id);
        }
        let placeholders = all_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let sql = format!(
            "SELECT MIN(id), {}, creature_name, lasty_type,
                    MAX(finished), SUM(message_count),
                    MIN(first_seen_date), MAX(last_seen_date),
                    MAX(completed_date), MAX(abandoned_date)
             FROM lastys WHERE character_id IN ({})
             GROUP BY creature_name
             ORDER BY creature_name",
            char_id, placeholders
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let lastys = stmt.query_map(rusqlite::params_from_iter(all_ids.iter()), |row| {
            Ok(Lasty {
                id: Some(row.get(0)?),
                character_id: row.get(1)?,
                creature_name: row.get(2)?,
                lasty_type: row.get(3)?,
                finished: row.get::<_, i64>(4)? != 0,
                message_count: row.get(5)?,
                first_seen_date: row.get(6)?,
                last_seen_date: row.get(7)?,
                completed_date: row.get(8)?,
                abandoned_date: row.get(9)?,
            })
        })?;
        Ok(lastys.filter_map(|r| r.ok()).collect())
    }

    /// Get a character with aggregated stats from all its merge sources.
    /// Sums numeric fields, takes MIN start_date.
    pub fn get_character_merged(&self, char_id: i64) -> Result<Option<Character>> {
        let source_ids = self.merged_source_ids(char_id)?;
        if source_ids.is_empty() {
            return self.get_character_by_id(char_id);
        }

        // Get the target character as a base
        let target = match self.get_character_by_id(char_id)? {
            Some(c) => c,
            None => return Ok(None),
        };

        // Get all source characters and sum their stats
        let mut merged = target;
        for &sid in &source_ids {
            if let Some(source) = self.get_character_by_id(sid)? {
                merged.logins += source.logins;
                merged.departs += source.departs;
                merged.deaths += source.deaths;
                merged.esteem += source.esteem;
                merged.coins_picked_up += source.coins_picked_up;
                merged.casino_won += source.casino_won;
                merged.casino_lost += source.casino_lost;
                merged.chest_coins += source.chest_coins;
                merged.bounty_coins += source.bounty_coins;
                merged.fur_coins += source.fur_coins;
                merged.mandible_coins += source.mandible_coins;
                merged.blood_coins += source.blood_coins;
                merged.bells_used += source.bells_used;
                merged.bells_broken += source.bells_broken;
                merged.chains_used += source.chains_used;
                merged.chains_broken += source.chains_broken;
                merged.shieldstones_used += source.shieldstones_used;
                merged.shieldstones_broken += source.shieldstones_broken;
                merged.ethereal_portals += source.ethereal_portals;
                merged.darkstone += source.darkstone;
                merged.purgatory_pendant += source.purgatory_pendant;
                merged.good_karma += source.good_karma;
                merged.bad_karma += source.bad_karma;
                merged.fur_worth += source.fur_worth;
                merged.mandible_worth += source.mandible_worth;
                merged.blood_worth += source.blood_worth;
                merged.eps_broken += source.eps_broken;
                merged.untraining_count += source.untraining_count;
                // Take earlier start_date
                if let Some(ref source_date) = source.start_date {
                    if merged.start_date.is_none() || merged.start_date.as_ref().unwrap() > source_date {
                        merged.start_date = Some(source_date.clone());
                    }
                }
            }
        }

        // Coin level = max creature_value across all personal kills
        let all_ids = self.char_ids_for_merged(char_id)?;
        merged.coin_level = self.compute_coin_level_for_char_ids(&all_ids)?;

        Ok(Some(merged))
    }
}
