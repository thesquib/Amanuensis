//! Whether this database's scanned ranks predate the current trainer data
//! (`TRAINER_REVISION`), so the GUI and CLI can suggest one Rescan Logs.

use serde::Serialize;

use super::Database;
use crate::data::TRAINER_REVISION;
use crate::error::Result;

// ---- User-facing copy: the only place these strings live (GUI banner and CLI both show them). ----
pub const TRAINER_DATA_TITLE: &str = "Trainer ranks can now be read more accurately";
pub const TRAINER_DATA_BODY: &str = "This version recognises Higgrus and Hardia and fixes Rodnus. Run Rescan Logs to rebuild your ranks from your log folders. A rescan resets anything that is not in those folders, such as ranks imported from Scribius or logs on a drive that is not connected.";
pub const TRAINER_DATA_TITLE_AFTER: &str = "Rescan finished: please check your rank modifiers";
pub const TRAINER_DATA_BODY_AFTER: &str = "Your ranks were rebuilt with the new trainer data. These modifiers were entered before the rebuild, so check they are still what you want.";
pub const TRAINER_DATA_DISMISS_HINT: &str = "Run \"amanuensis trainer-status --ack\" to dismiss.";
const REASON_OVERRIDE: &str = "set to {n} by hand. Logged ranks are now available if you switch back.";
const REASON_MODIFIER: &str = "you entered {n}. After a rescan, logged ranks are added to this, so check it is still what you want.";

/// The trainers whose messages were shared with another trainer before this revision.
const REVIEW_TRAINERS: [&str; 4] = ["Histia", "Higgrus", "Master Bodrus", "Hardia"];

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ReviewItem {
    pub character: String,
    pub trainer: String,
    pub rank_mode: String,
    pub modified_ranks: i64,
    /// Plain-text line for this entry.
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TrainerDataStatus {
    pub stale: bool,
    pub acknowledged: bool,
    /// A rescan ran while stale and the modifier list has not been dismissed yet.
    pub review_pending: bool,
    /// Whether the banner / CLI notice should appear.
    pub show: bool,
    pub title: String,
    pub body: String,
    pub modifiers_to_review: Vec<ReviewItem>,
}

impl Database {
    fn meta_revision(&self, key: &str) -> Result<u32> {
        Ok(self.get_db_meta(key)?.and_then(|v| v.parse().ok()).unwrap_or(0))
    }

    /// Record that the scanned data here was built by the current trainer data.
    pub(crate) fn set_trainer_revision_current(&self) -> Result<()> {
        self.set_db_meta("trainer_revision", &TRAINER_REVISION.to_string())
    }

    /// An empty log index means nothing was scanned (fresh or import-only), so nothing is stale.
    pub(crate) fn mark_trainer_revision_if_unscanned(&self) -> Result<()> {
        let n: i64 = self.conn.query_row("SELECT COUNT(*) FROM log_files", [], |r| r.get(0))?;
        if n == 0 { self.set_trainer_revision_current()?; }
        Ok(())
    }

    pub fn trainer_data_status(&self) -> Result<TrainerDataStatus> {
        let scanned: i64 = self.conn.query_row("SELECT COUNT(*) FROM log_files", [], |r| r.get(0))?;
        let stale = scanned > 0 && self.meta_revision("trainer_revision")? < TRAINER_REVISION;
        let acknowledged = self.meta_revision("trainer_revision_ack")? >= TRAINER_REVISION;
        let review_pending = self.get_db_meta("trainer_review_pending")?.as_deref() == Some("1");
        let mut modifiers_to_review = Vec::new();
        if (stale || review_pending) && !acknowledged {
            // Modes that add logged ranks to a typed number, or hide logged ranks entirely.
            let mut stmt = self.conn.prepare(
                "SELECT c.name, t.trainer_name, t.rank_mode, t.modified_ranks
                 FROM trainers t JOIN characters c ON c.id = t.character_id
                 WHERE t.trainer_name IN (?1, ?2, ?3, ?4)
                   AND (t.rank_mode = 'override' OR t.modified_ranks != 0)
                 ORDER BY c.name, t.trainer_name",
            )?;
            let rows = stmt.query_map(rusqlite::params![
                REVIEW_TRAINERS[0], REVIEW_TRAINERS[1], REVIEW_TRAINERS[2], REVIEW_TRAINERS[3]
            ], |r| {
                let (character, trainer, rank_mode, n): (String, String, String, i64) =
                    (r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?);
                let tpl = if rank_mode == "override" { REASON_OVERRIDE } else { REASON_MODIFIER };
                let reason = format!("{trainer} ({character}): {}", tpl.replace("{n}", &n.to_string()));
                Ok(ReviewItem { character, trainer, rank_mode, modified_ranks: n, reason })
            })?;
            for r in rows { modifiers_to_review.push(r?); }
        }
        let show = !acknowledged && (stale || (review_pending && !modifiers_to_review.is_empty()));
        let after = !stale && review_pending;
        let (title, body) = if after { (TRAINER_DATA_TITLE_AFTER, TRAINER_DATA_BODY_AFTER) } else { (TRAINER_DATA_TITLE, TRAINER_DATA_BODY) };
        Ok(TrainerDataStatus {
            stale, acknowledged, review_pending, show,
            title: title.into(), body: body.into(), modifiers_to_review,
        })
    }

    pub fn acknowledge_trainer_data(&self) -> Result<()> {
        self.set_db_meta("trainer_revision_ack", &TRAINER_REVISION.to_string())?;
        self.conn.execute("DELETE FROM db_meta WHERE key = 'trainer_review_pending'", [])?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with_scanned_file(db: &Database) {
        let id = db.get_or_create_character("Scan").unwrap();
        db.conn.execute("INSERT INTO log_files (character_id, file_path, content_hash, date_read) VALUES (?1, 'a.txt', 'h', '2026-01-01')", [id]).unwrap();
    }

    fn set_override(db: &Database, ch: &str, trainer: &str, mode: &str, n: i64) {
        let id = db.get_or_create_character(ch).unwrap();
        db.set_rank_override(id, trainer, mode, n, if mode == "override_until_date" { Some("1/1/20") } else { None }).unwrap();
    }

    #[test]
    fn fresh_db_not_stale() {
        let db = Database::open_in_memory().unwrap();
        let s = db.trainer_data_status().unwrap();
        assert!(!s.stale && !s.acknowledged && s.modifiers_to_review.is_empty());
    }

    #[test]
    fn scanned_db_without_revision_is_stale() {
        let db = Database::open_in_memory().unwrap();
        with_scanned_file(&db);
        assert!(db.trainer_data_status().unwrap().stale);
    }

    #[test]
    fn reset_sets_current_revision() {
        let db = Database::open_in_memory().unwrap();
        with_scanned_file(&db);
        db.reset_log_data().unwrap();
        with_scanned_file(&db);
        assert!(!db.trainer_data_status().unwrap().stale);
    }

    #[test]
    fn reset_keeps_ack() {
        let db = Database::open_in_memory().unwrap();
        db.acknowledge_trainer_data().unwrap();
        db.reset_log_data().unwrap();
        assert!(db.trainer_data_status().unwrap().acknowledged);
    }

    #[test]
    fn acknowledge_hides_but_stays_stale() {
        let db = Database::open_in_memory().unwrap();
        with_scanned_file(&db);
        db.acknowledge_trainer_data().unwrap();
        let s = db.trainer_data_status().unwrap();
        assert!(s.stale && s.acknowledged);
    }

    #[test]
    fn delete_all_clears_ack() {
        let db = Database::open_in_memory().unwrap();
        db.acknowledge_trainer_data().unwrap();
        db.delete_all_data().unwrap();
        assert!(!db.trainer_data_status().unwrap().acknowledged);
    }

    #[test]
    fn import_only_db_not_stale() {
        let db = Database::open_in_memory().unwrap();
        set_override(&db, "Ruuk", "Histia", "modifier", 5);
        assert!(!db.trainer_data_status().unwrap().stale);
    }

    #[test]
    fn open_marks_unscanned_file_db_current() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.db");
        let p = path.to_str().unwrap();
        Database::open(p).unwrap();
        let db = Database::open(p).unwrap();
        assert_eq!(db.get_db_meta("trainer_revision").unwrap(), Some(TRAINER_REVISION.to_string()));
    }

    #[test]
    fn review_list_survives_rescan_until_acknowledged() {
        let db = Database::open_in_memory().unwrap();
        set_override(&db, "Ruuk", "Histia", "modifier", -2);
        with_scanned_file(&db);
        assert!(db.trainer_data_status().unwrap().show);
        db.reset_log_data().unwrap();
        let s = db.trainer_data_status().unwrap();
        assert!(!s.stale && s.review_pending && s.show);
        assert_eq!(s.modifiers_to_review.len(), 1);
        assert_eq!(s.title, TRAINER_DATA_TITLE_AFTER);
        db.acknowledge_trainer_data().unwrap();
        let s = db.trainer_data_status().unwrap();
        assert!(!s.show && s.modifiers_to_review.is_empty());
    }

    #[test]
    fn rescan_without_modifiers_shows_nothing() {
        let db = Database::open_in_memory().unwrap();
        with_scanned_file(&db);
        db.reset_log_data().unwrap();
        let s = db.trainer_data_status().unwrap();
        assert!(s.review_pending && !s.show);
    }

    #[test]
    fn rescan_of_current_db_sets_no_review_pending() {
        let db = Database::open_in_memory().unwrap();
        set_override(&db, "Ruuk", "Histia", "modifier", -2);
        with_scanned_file(&db);
        db.set_trainer_revision_current().unwrap();
        db.reset_log_data().unwrap();
        let s = db.trainer_data_status().unwrap();
        assert!(!s.review_pending && !s.show);
    }

    #[test]
    fn delete_all_clears_review_pending() {
        let db = Database::open_in_memory().unwrap();
        with_scanned_file(&db);
        db.reset_log_data().unwrap();
        db.delete_all_data().unwrap();
        assert!(!db.trainer_data_status().unwrap().review_pending);
    }

    #[test]
    fn reopen_with_scanned_files_stays_stale() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.db");
        let p = path.to_str().unwrap();
        {
            let db = Database::open(p).unwrap();
            with_scanned_file(&db);
            db.conn.execute("DELETE FROM db_meta WHERE key = 'trainer_revision'", []).unwrap();
        }
        assert!(Database::open(p).unwrap().trainer_data_status().unwrap().stale);
    }

    #[test]
    fn review_list_covers_all_four_trainers() {
        let db = Database::open_in_memory().unwrap();
        set_override(&db, "Ruuk", "Histia", "modifier", -20);
        set_override(&db, "Ruuk", "Master Bodrus", "override_until_date", 5);
        set_override(&db, "Ruuk", "Atkus", "modifier", 5); // moved elsewhere: not listed
        set_override(&db, "Ruuk", "Higgrus", "override", 3);
        set_override(&db, "Ruuk", "Hardia", "modifier", 0); // nothing entered: not listed
        with_scanned_file(&db);
        let got: Vec<(String, i64)> = db.trainer_data_status().unwrap().modifiers_to_review
            .into_iter().map(|i| (i.trainer, i.modified_ranks)).collect();
        assert_eq!(got, vec![
            ("Higgrus".to_string(), 3), ("Histia".to_string(), -20), ("Master Bodrus".to_string(), 5),
        ]);
    }
}
