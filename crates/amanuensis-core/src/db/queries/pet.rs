use rusqlite::params;

use crate::error::{AmanuensisError, Result};
use crate::models::Pet;
use super::Database;

impl Database {
    /// Get pets for a character, excluding ones the user deleted or merged away.
    pub fn get_pets(&self, char_id: i64) -> Result<Vec<Pet>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, character_id, pet_name, creature_name
             FROM pets WHERE character_id = ?1 AND hidden = 0 AND merged_into IS NULL
             ORDER BY pet_name",
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

    /// Upsert a pet record. Uses creature_name as both pet_name and creature_name.
    pub fn upsert_pet(&self, char_id: i64, creature_name: &str) -> Result<()> {
        self.conn.execute(
            "INSERT OR IGNORE INTO pets (character_id, pet_name, creature_name)
             VALUES (?1, ?2, ?2)",
            params![char_id, creature_name],
        )?;
        Ok(())
    }

    /// Delete a pet from a character's view (including its merge sources).
    /// The row is kept with `hidden = 1` so a rescan doesn't bring it back.
    pub fn delete_pet(&self, char_id: i64, pet_name: &str) -> Result<()> {
        let ids = self.char_ids_for_merged(char_id)?;
        let placeholders = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let sql = format!(
            "UPDATE pets SET hidden = 1 WHERE pet_name = ? AND character_id IN ({placeholders})"
        );
        let mut args: Vec<rusqlite::types::Value> = vec![pet_name.to_string().into()];
        args.extend(ids.iter().map(|&i| i.into()));
        self.conn.execute(&sql, rusqlite::params_from_iter(args))?;
        Ok(())
    }

    /// Merge `sources` into the pet named `target` (same pet logged under several
    /// names). Sources are kept with `merged_into = target` so a rescan doesn't
    /// bring them back. The target must be a visible pet of this character.
    pub fn merge_pets(&self, char_id: i64, sources: &[String], target: &str) -> Result<()> {
        if sources.iter().any(|s| s == target) {
            return Err(AmanuensisError::Data(format!("Cannot merge pet '{target}' into itself")));
        }
        let visible = self.get_pets_merged(char_id)?;
        if !visible.iter().any(|p| p.pet_name == target) {
            return Err(AmanuensisError::Data(format!("Target pet '{target}' not found")));
        }
        let ids = self.char_ids_for_merged(char_id)?;
        let id_ph = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let src_ph = sources.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let sql = format!(
            "UPDATE pets SET merged_into = ?
             WHERE pet_name IN ({src_ph}) AND character_id IN ({id_ph})"
        );
        let mut args: Vec<rusqlite::types::Value> = vec![target.to_string().into()];
        args.extend(sources.iter().map(|s| s.clone().into()));
        args.extend(ids.iter().map(|&i| i.into()));
        self.conn.execute(&sql, rusqlite::params_from_iter(args))?;
        Ok(())
    }
}
