use rusqlite::params;
use serde::Serialize;

use crate::error::Result;
use super::Database;

/// Depart totals derived from every "Your spirit has departed your body N times."
/// line seen. The game's counter can go down (Ruuk's went 59 → 40 between 2023 and
/// 2026), so the latest count is not the lifetime total, and a lifetime total that
/// predates the logs can't be compared with deaths counted from the logs.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct DepartSummary {
    /// First count seen, plus every later depart: an increase adds its size, a drop
    /// adds one (the line itself is printed by a depart).
    pub lifetime: i64,
    /// The game's own counter at the latest observation.
    pub current: i64,
    /// Departs whose line appears in the logs: the first observation plus each later
    /// one whose count changed. Same span as the deaths counted from the logs.
    pub in_logs: i64,
}

/// Summarise observations already sorted by time. A repeated count (the same line
/// seen again) is not a new depart.
fn summarize(counts: &[i64]) -> Option<DepartSummary> {
    let (&first, rest) = counts.split_first()?;
    let mut s = DepartSummary { lifetime: first, current: first, in_logs: 1 };
    for &c in rest {
        let delta = c - s.current;
        if delta != 0 {
            s.lifetime += if delta > 0 { delta } else { 1 };
            s.in_logs += 1;
        }
        s.current = c;
    }
    Some(s)
}

impl Database {
    /// Record one depart line. Duplicate copies of a log (backups in several folders)
    /// collapse onto the same (time, count) row.
    pub fn record_depart(&self, char_id: i64, observed_at: &str, count: i64) -> Result<()> {
        self.conn.execute(
            "INSERT OR IGNORE INTO depart_observations (character_id, observed_at, count)
             VALUES (?1, ?2, ?3)",
            params![char_id, observed_at, count],
        )?;
        Ok(())
    }

    /// Depart summary for a character and all its merge sources, or None when no depart
    /// line was ever scanned (e.g. a Scribius import, whose `departs` stands alone).
    /// Ties on time (lines without a timestamp take the file's date) order by count.
    pub fn depart_summary_merged(&self, char_id: i64) -> Result<Option<DepartSummary>> {
        let ids = self.char_ids_for_merged(char_id)?;
        let placeholders = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let sql = format!(
            "SELECT DISTINCT observed_at, count FROM depart_observations
             WHERE character_id IN ({placeholders})
             ORDER BY observed_at, count"
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let counts = stmt
            .query_map(rusqlite::params_from_iter(ids.iter()), |row| row.get::<_, i64>(1))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(summarize(&counts))
    }

    /// Write each scanned character's lifetime departs into `characters.departs`, so
    /// the stored total no longer depends on the order files were scanned in.
    pub fn refresh_departs(&self) -> Result<()> {
        let ids: Vec<i64> = {
            let mut stmt = self.conn.prepare("SELECT DISTINCT character_id FROM depart_observations")?;
            let rows = stmt.query_map([], |row| row.get(0))?;
            rows.collect::<std::result::Result<_, _>>()?
        };
        for id in ids {
            let mut stmt = self.conn.prepare(
                "SELECT count FROM depart_observations WHERE character_id = ?1
                 ORDER BY observed_at, count",
            )?;
            let counts = stmt
                .query_map(params![id], |row| row.get::<_, i64>(0))?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            if let Some(s) = summarize(&counts) {
                self.conn.execute(
                    "UPDATE characters SET departs = ?1 WHERE id = ?2",
                    params![s.lifetime, id],
                )?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summarize_handles_rises_drops_and_repeats() {
        // Ruuk's real counter: 46 (2017) … 57, 58, 59 (2023), then 40 (2026).
        let s = summarize(&[46, 48, 57, 58, 59, 40]).unwrap();
        assert_eq!(s.current, 40);
        assert_eq!(s.lifetime, 46 + 2 + 9 + 1 + 1 + 1);
        assert_eq!(s.in_logs, 6);
        // A repeated line is not another depart (Olga's log repeats "5 times").
        let s = summarize(&[4, 5, 5, 6]).unwrap();
        assert_eq!((s.lifetime, s.current, s.in_logs), (6, 6, 3));
        assert!(summarize(&[]).is_none());
    }

    #[test]
    fn summary_is_independent_of_insert_order_and_duplicates() {
        let db = Database::open_in_memory().unwrap();
        let id = db.get_or_create_character("Ruuk").unwrap();
        // Inserted out of order, with one duplicate from a backup copy of a log.
        for (t, c) in [
            ("2026-07-13 10:03:50", 40),
            ("2017-02-06 12:00:00", 46),
            ("2023-02-19 07:21:04", 58),
            ("2023-02-19 07:21:04", 59), // no timestamp: both lines take the file date
            ("2020-12-10 19:01:00", 57),
            ("2020-12-10 19:01:00", 57),
        ] {
            db.record_depart(id, t, c).unwrap();
        }
        let s = db.depart_summary_merged(id).unwrap().unwrap();
        assert_eq!((s.lifetime, s.current, s.in_logs), (46 + 11 + 1 + 1 + 1, 40, 5));

        db.refresh_departs().unwrap();
        assert_eq!(db.get_character("Ruuk").unwrap().unwrap().departs, 60);
    }

    #[test]
    fn merged_summary_unions_sources_and_imports_have_none() {
        let db = Database::open_in_memory().unwrap();
        let a = db.get_or_create_character("A").unwrap();
        let b = db.get_or_create_character("B").unwrap();
        db.record_depart(a, "2020-01-01 00:00:00", 3).unwrap();
        db.record_depart(b, "2021-01-01 00:00:00", 5).unwrap();
        assert!(db.depart_summary_merged(b).unwrap().is_some());
        let c = db.get_or_create_character("Imported").unwrap();
        assert!(db.depart_summary_merged(c).unwrap().is_none());

        db.merge_characters(&[b], a).unwrap();
        let s = db.depart_summary_merged(a).unwrap().unwrap();
        assert_eq!((s.lifetime, s.current, s.in_logs), (5, 5, 2));
    }

    #[test]
    fn reset_clears_observations() {
        let db = Database::open_in_memory().unwrap();
        let id = db.get_or_create_character("A").unwrap();
        db.record_depart(id, "2020-01-01 00:00:00", 3).unwrap();
        db.reset_log_data().unwrap();
        assert!(db.depart_summary_merged(id).unwrap().is_none());
    }
}
