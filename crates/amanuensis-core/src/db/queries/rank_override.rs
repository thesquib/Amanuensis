//! Rank overrides keyed by character and trainer *name*, not row id, so they can be
//! exported, imported into another database, and kept in a backup file that outlives
//! any one database (a Delete All Data, a new database from a Scribius import, a
//! rescan that credits a different-case character row).

use std::path::Path;

use rusqlite::params;
use serde::{Deserialize, Serialize};

use crate::error::{AmanuensisError, Result};
use crate::models::RankMode;
use super::Database;

/// One trainer's user-entered override.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RankOverride {
    pub character: String,
    pub trainer: String,
    pub modified_ranks: i64,
    pub rank_mode: String,
    pub override_date: Option<String>,
}

impl RankOverride {
    /// Character names are unique in Clan Lord ignoring case; trainer names are exact.
    fn same_key(&self, character: &str, trainer: &str) -> bool {
        self.character.eq_ignore_ascii_case(character) && self.trainer == trainer
    }
}

#[derive(Debug, Default, Serialize)]
pub struct ApplyOverridesResult {
    pub applied: usize,
    /// Characters named in the file that this database doesn't have (skipped).
    pub unknown_characters: Vec<String>,
    /// True when any applied override uses an override mode, which needs a rescan.
    pub needs_rescan: bool,
}

#[derive(Serialize, Deserialize)]
struct OverridesFile {
    version: u32,
    overrides: Vec<RankOverride>,
}

/// Read an overrides file. A missing file is an empty list.
pub fn read_overrides_file(path: &Path) -> Result<Vec<RankOverride>> {
    match std::fs::read_to_string(path) {
        Ok(s) => Ok(serde_json::from_str::<OverridesFile>(&s)?.overrides),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(e) => Err(e.into()),
    }
}

/// Write an overrides file atomically (temp file + rename), so a crash mid-write
/// can't leave the backup truncated.
pub fn write_overrides_file(path: &Path, overrides: &[RankOverride]) -> Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let json = serde_json::to_string_pretty(&OverridesFile { version: 1, overrides: overrides.to_vec() })?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, json)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

/// Replace `entry`'s key in `list` (or add it).
fn upsert(list: &mut Vec<RankOverride>, entry: RankOverride) {
    list.retain(|o| !o.same_key(&entry.character, &entry.trainer));
    list.push(entry);
}

impl Database {
    /// Every trainer row carrying a user override (non-zero modifier or a non-modifier
    /// mode), optionally limited to one character row.
    pub fn rank_overrides(&self, char_id: Option<i64>) -> Result<Vec<RankOverride>> {
        let mut stmt = self.conn.prepare(
            "SELECT c.name, t.trainer_name, t.modified_ranks, t.rank_mode, t.override_date
             FROM trainers t JOIN characters c ON c.id = t.character_id
             WHERE (t.modified_ranks != 0 OR t.rank_mode != 'modifier')
               AND (?1 IS NULL OR t.character_id = ?1)
             ORDER BY c.name, t.trainer_name",
        )?;
        let rows = stmt.query_map(params![char_id], |row| {
            Ok(RankOverride {
                character: row.get(0)?,
                trainer: row.get(1)?,
                modified_ranks: row.get(2)?,
                rank_mode: row.get(3)?,
                override_date: row.get(4)?,
            })
        })?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    /// The character row a name refers to, ignoring case. A row that has logins wins
    /// over an empty leftover, and a capitalised row over an all-lowercase one.
    fn find_character_id(&self, name: &str) -> Option<i64> {
        self.conn.query_row(
            "SELECT id FROM characters WHERE name = ?1 COLLATE NOCASE
             ORDER BY logins = 0, name = lower(name), id LIMIT 1",
            params![name],
            |row| row.get(0),
        ).ok()
    }

    /// Apply overrides, overwriting those trainers' current values. With `char_id`, all
    /// entries go to that character whatever their name (restore); otherwise each entry
    /// goes to the character it names (import). Validated up front and applied in one
    /// transaction, so a bad file changes nothing.
    pub fn apply_rank_overrides(
        &self,
        char_id: Option<i64>,
        overrides: &[RankOverride],
    ) -> Result<ApplyOverridesResult> {
        for o in overrides {
            if RankMode::parse(&o.rank_mode).is_none() {
                return Err(AmanuensisError::Data(format!(
                    "Invalid rank mode '{}' for {} / {}", o.rank_mode, o.character, o.trainer
                )));
            }
        }
        self.begin_transaction()?;
        match self.apply_rank_overrides_inner(char_id, overrides) {
            Ok(r) => { self.commit_transaction()?; Ok(r) }
            Err(e) => { let _ = self.rollback_transaction(); Err(e) }
        }
    }

    fn apply_rank_overrides_inner(
        &self,
        char_id: Option<i64>,
        overrides: &[RankOverride],
    ) -> Result<ApplyOverridesResult> {
        let mut result = ApplyOverridesResult::default();
        for o in overrides {
            let Some(id) = char_id.or_else(|| self.find_character_id(&o.character)) else {
                if !result.unknown_characters.iter().any(|n| n.eq_ignore_ascii_case(&o.character)) {
                    result.unknown_characters.push(o.character.clone());
                }
                continue;
            };
            self.set_rank_override(id, &o.trainer, &o.rank_mode, o.modified_ranks, o.override_date.as_deref())?;
            result.applied += 1;
            result.needs_rescan |= o.rank_mode != RankMode::Modifier.as_str();
        }
        Ok(result)
    }

    /// Move a trainer's override to another trainer name (e.g. after a trainer was
    /// renamed in trainers.json), for the character and its merge sources. Refuses if
    /// the target already has an override, so nothing is silently overwritten.
    pub fn move_rank_override(&self, char_id: i64, from: &str, to: &str) -> Result<()> {
        let ids = self.char_ids_for_merged(char_id)?;
        let mut moves = Vec::new();
        for id in ids {
            let overrides = self.rank_overrides(Some(id))?;
            if let Some(src) = overrides.iter().find(|o| o.trainer == from) {
                if let Some(dst) = overrides.iter().find(|o| o.trainer == to) {
                    return Err(AmanuensisError::Data(format!(
                        "{} already has a rank override on {}; clear it first", dst.character, to
                    )));
                }
                moves.push((id, src.clone()));
            }
        }
        self.begin_transaction()?;
        let r = moves.iter().try_for_each(|(id, o)| {
            self.set_rank_override(*id, to, &o.rank_mode, o.modified_ranks, o.override_date.as_deref())?;
            self.set_rank_override(*id, from, RankMode::Modifier.as_str(), 0, None)
        });
        match r {
            Ok(()) => { self.commit_transaction()?; Ok(()) }
            Err(e) => { let _ = self.rollback_transaction(); Err(e) }
        }
    }

    /// Add every override in this database to the backup, replacing older values for
    /// the same character/trainer. Never removes anything, so opening a database that
    /// lost its overrides doesn't erase them from the backup.
    pub fn backup_all_rank_overrides(&self, backup: &Path) -> Result<()> {
        let mut list = read_overrides_file(backup)?;
        for o in self.rank_overrides(None)? {
            upsert(&mut list, o);
        }
        write_overrides_file(backup, &list)
    }

    /// Bring the backup in line with the database for these trainers of these
    /// characters, after the user edited them: a trainer set back to defaults is
    /// removed from the backup on purpose.
    pub fn sync_rank_override_backup(&self, backup: &Path, char_ids: &[i64], trainers: &[&str]) -> Result<()> {
        let mut list = read_overrides_file(backup)?;
        for &id in char_ids {
            let Ok(name) = self.conn.query_row(
                "SELECT name FROM characters WHERE id = ?1", params![id], |row| row.get::<_, String>(0),
            ) else { continue };
            let current = self.rank_overrides(Some(id))?;
            for t in trainers {
                list.retain(|o| !o.same_key(&name, t));
                if let Some(o) = current.iter().find(|o| o.trainer == *t) {
                    list.push(o.clone());
                }
            }
        }
        write_overrides_file(backup, &list)
    }

    /// Backed-up overrides for this character and its merge sources (matched by name,
    /// ignoring case) whose trainer has no override on that row: the ones that were lost.
    /// Paired with the row each one belongs to.
    fn missing_with_ids(&self, backup: &Path, char_id: i64) -> Result<Vec<(i64, RankOverride)>> {
        let backed_up = read_overrides_file(backup)?;
        let mut out: Vec<(i64, RankOverride)> = Vec::new();
        for id in self.char_ids_for_merged(char_id)? {
            let name: String = self.conn.query_row(
                "SELECT name FROM characters WHERE id = ?1", params![id], |row| row.get(0),
            )?;
            let current = self.rank_overrides(Some(id))?;
            for o in &backed_up {
                if o.character.eq_ignore_ascii_case(&name)
                    && !current.iter().any(|c| c.trainer == o.trainer)
                    && !out.iter().any(|(_, m)| m == o)
                {
                    out.push((id, o.clone()));
                }
            }
        }
        Ok(out)
    }

    /// Backed-up overrides this character (or a merge source) no longer has.
    pub fn missing_rank_overrides(&self, backup: &Path, char_id: i64) -> Result<Vec<RankOverride>> {
        Ok(self.missing_with_ids(backup, char_id)?.into_iter().map(|(_, o)| o).collect())
    }

    /// Put each missing backed-up override back on the row it came from.
    pub fn restore_missing_rank_overrides(&self, backup: &Path, char_id: i64) -> Result<ApplyOverridesResult> {
        let missing = self.missing_with_ids(backup, char_id)?;
        self.begin_transaction()?;
        let r = missing.iter().try_fold(ApplyOverridesResult::default(), |mut acc, (id, o)| {
            self.apply_rank_overrides_inner(Some(*id), std::slice::from_ref(o)).map(|one| {
                acc.applied += one.applied;
                acc.needs_rescan |= one.needs_rescan;
                acc
            })
        });
        match r {
            Ok(r) => { self.commit_transaction()?; Ok(r) }
            Err(e) => { let _ = self.rollback_transaction(); Err(e) }
        }
    }

    /// Drop this character's missing overrides from the backup (the user doesn't want them).
    pub fn discard_missing_rank_overrides(&self, backup: &Path, char_id: i64) -> Result<()> {
        let missing = self.missing_rank_overrides(backup, char_id)?;
        let mut list = read_overrides_file(backup)?;
        list.retain(|o| !missing.contains(o));
        write_overrides_file(backup, &list)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn db_with(names: &[&str]) -> (Database, Vec<i64>) {
        let db = Database::open(":memory:").unwrap();
        let ids = names.iter().map(|n| db.get_or_create_character(n).unwrap()).collect();
        (db, ids)
    }

    #[test]
    fn export_import_roundtrip_by_name() {
        let (a, ids) = db_with(&["Ruuk"]);
        a.set_rank_override(ids[0], "Bangus Anmash", "override", 40, None).unwrap();
        a.set_rank_override(ids[0], "Atkus", "modifier", 5, None).unwrap();
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("o.json");
        write_overrides_file(&file, &a.rank_overrides(None).unwrap()).unwrap();

        let (b, bids) = db_with(&["ruuk", "Other"]);
        let mut entries = read_overrides_file(&file).unwrap();
        entries.push(RankOverride { character: "Nobody".into(), trainer: "Atkus".into(), modified_ranks: 1, rank_mode: "modifier".into(), override_date: None });
        let r = b.apply_rank_overrides(None, &entries).unwrap();
        assert_eq!(r.applied, 2);
        assert_eq!(r.unknown_characters, vec!["Nobody".to_string()]);
        assert!(r.needs_rescan);
        let got = b.rank_overrides(Some(bids[0])).unwrap();
        assert_eq!(got.len(), 2);
        assert!(got.iter().any(|o| o.trainer == "Bangus Anmash" && o.rank_mode == "override" && o.modified_ranks == 40));
    }

    #[test]
    fn bad_mode_applies_nothing() {
        let (db, ids) = db_with(&["Ruuk"]);
        let entries = vec![
            RankOverride { character: "Ruuk".into(), trainer: "Atkus".into(), modified_ranks: 5, rank_mode: "modifier".into(), override_date: None },
            RankOverride { character: "Ruuk".into(), trainer: "Detha".into(), modified_ranks: 5, rank_mode: "bogus".into(), override_date: None },
        ];
        assert!(db.apply_rank_overrides(None, &entries).is_err());
        assert!(db.rank_overrides(Some(ids[0])).unwrap().is_empty());
    }

    #[test]
    fn clear_one_character_keeps_others() {
        let (db, ids) = db_with(&["Ruuk", "Tane"]);
        db.set_rank_override(ids[0], "Atkus", "modifier", 5, None).unwrap();
        db.set_rank_override(ids[1], "Atkus", "modifier", 7, None).unwrap();
        db.clear_rank_overrides(Some(ids[0])).unwrap();
        assert!(db.rank_overrides(Some(ids[0])).unwrap().is_empty());
        assert_eq!(db.rank_overrides(Some(ids[1])).unwrap().len(), 1);
    }

    #[test]
    fn backup_survives_wipe_and_offers_restore() {
        let (db, ids) = db_with(&["Ruuk"]);
        let dir = tempfile::tempdir().unwrap();
        let backup = dir.path().join("b.json");
        db.set_rank_override(ids[0], "Atkus", "modifier", 5, None).unwrap();
        db.set_rank_override(ids[0], "Detha", "modifier", 3, None).unwrap();
        db.backup_all_rank_overrides(&backup).unwrap();

        // Bulk clear doesn't touch the backup, so the overrides show as missing.
        db.clear_rank_overrides(None).unwrap();
        db.backup_all_rank_overrides(&backup).unwrap();
        assert_eq!(db.missing_rank_overrides(&backup, ids[0]).unwrap().len(), 2);

        db.restore_missing_rank_overrides(&backup, ids[0]).unwrap();
        assert!(db.missing_rank_overrides(&backup, ids[0]).unwrap().is_empty());

        // An explicit single edit back to default removes it from the backup.
        db.set_rank_override(ids[0], "Atkus", "modifier", 0, None).unwrap();
        db.sync_rank_override_backup(&backup, &ids, &["Atkus"]).unwrap();
        assert!(db.missing_rank_overrides(&backup, ids[0]).unwrap().is_empty());
        assert_eq!(read_overrides_file(&backup).unwrap().len(), 1);

        // Discard drops what's missing.
        db.clear_rank_overrides(None).unwrap();
        db.discard_missing_rank_overrides(&backup, ids[0]).unwrap();
        assert!(read_overrides_file(&backup).unwrap().is_empty());
    }

    #[test]
    fn clear_and_restore_merged_character() {
        let (db, ids) = db_with(&["Ruuk", "Ruukalt"]);
        let dir = tempfile::tempdir().unwrap();
        let backup = dir.path().join("b.json");
        db.set_rank_override(ids[0], "Atkus", "modifier", 5, None).unwrap();
        db.set_rank_override(ids[1], "Detha", "modifier", 3, None).unwrap();
        db.merge_characters(&[ids[1]], ids[0]).unwrap();
        db.backup_all_rank_overrides(&backup).unwrap();

        db.clear_rank_overrides(Some(ids[0])).unwrap();
        assert_eq!(db.missing_rank_overrides(&backup, ids[0]).unwrap().len(), 2);
        db.restore_missing_rank_overrides(&backup, ids[0]).unwrap();
        assert_eq!(db.rank_overrides(Some(ids[0])).unwrap()[0].trainer, "Atkus");
        assert_eq!(db.rank_overrides(Some(ids[1])).unwrap()[0].trainer, "Detha");
        assert!(db.missing_rank_overrides(&backup, ids[0]).unwrap().is_empty());
    }

    #[test]
    fn move_override_to_renamed_trainer() {
        let (db, ids) = db_with(&["Ruuk"]);
        db.set_rank_override(ids[0], "Fellblade", "override", 12, None).unwrap();
        db.move_rank_override(ids[0], "Fellblade", "Champion Blade").unwrap();
        let got = db.rank_overrides(Some(ids[0])).unwrap();
        assert_eq!(got.len(), 1);
        assert_eq!((got[0].trainer.as_str(), got[0].modified_ranks), ("Champion Blade", 12));

        db.set_rank_override(ids[0], "Fellblade", "modifier", 1, None).unwrap();
        assert!(db.move_rank_override(ids[0], "Fellblade", "Champion Blade").is_err());
    }
}
