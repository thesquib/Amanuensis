use rusqlite::params;

use crate::error::Result;
use crate::models::Pet;
use super::Database;

impl Database {
    /// Get pets for a character.
    pub fn get_pets(&self, char_id: i64) -> Result<Vec<Pet>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, character_id, pet_name, creature_name
             FROM pets WHERE character_id = ?1 ORDER BY pet_name",
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
}
