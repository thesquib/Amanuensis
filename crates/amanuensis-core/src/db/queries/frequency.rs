use std::collections::BTreeMap;

use chrono::NaiveDateTime;
use rusqlite::params_from_iter;
use serde::Serialize;

use crate::error::Result;
use super::Database;

/// Window length for the sliding 2-hour max, in seconds.
const TWO_HOURS_SECS: i64 = 2 * 60 * 60;

/// Per-creature max-frequency stats.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct CreatureFrequency {
    pub creature_name: String,
    pub best_day_count: i64,
    pub best_day_date: Option<String>,
    pub best_day_verbs: BTreeMap<String, i64>,
    pub best_2h_count: i64,
    pub best_2h_start: Option<String>,
    pub best_2h_verbs: BTreeMap<String, i64>,
}

struct Event {
    verb: String,
    ts_raw: String,
    ts: NaiveDateTime,
}

/// Parse a stored timestamp. Real CL lines are full datetimes; if a line lacked a
/// time component the stored value is date-only, which we treat as midnight so the
/// 24h metric still works (the 2h metric collapses such events to one instant).
fn parse_ts(s: &str) -> NaiveDateTime {
    NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S")
        .or_else(|_| {
            chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d")
                .map(|d| d.and_hms_opt(0, 0, 0).unwrap())
        })
        .unwrap_or_else(|_| {
            NaiveDateTime::parse_from_str("1970-01-01 00:00:00", "%Y-%m-%d %H:%M:%S").unwrap()
        })
}

impl Database {
    /// Compute per-creature max-frequency stats across one or more character IDs
    /// (multiple IDs support merged characters). `include_assisted=false` counts
    /// solo kills only.
    pub fn kill_frequency_for_char_ids(
        &self,
        char_ids: &[i64],
        include_assisted: bool,
    ) -> Result<Vec<CreatureFrequency>> {
        if char_ids.is_empty() {
            return Ok(Vec::new());
        }
        let placeholders = char_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let assisted_clause = if include_assisted { "" } else { " AND assisted = 0" };
        let sql = format!(
            "SELECT creature_name, verb, timestamp FROM kill_events
             WHERE character_id IN ({placeholders}){assisted_clause}
             ORDER BY creature_name, timestamp",
        );

        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(params_from_iter(char_ids.iter()), |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?;

        let mut by_creature: BTreeMap<String, Vec<Event>> = BTreeMap::new();
        for r in rows {
            let (creature, verb, ts_raw) = r?;
            let ts = parse_ts(&ts_raw);
            by_creature
                .entry(creature)
                .or_default()
                .push(Event { verb, ts_raw, ts });
        }

        let mut out = Vec::with_capacity(by_creature.len());
        for (creature_name, events) in by_creature {
            out.push(compute_one(creature_name, &events));
        }
        out.sort_by(|a, b| {
            b.best_day_count
                .cmp(&a.best_day_count)
                .then(a.creature_name.cmp(&b.creature_name))
        });
        Ok(out)
    }

    /// Merged-character convenience wrapper. Includes assisted kills.
    pub fn kill_frequency_merged(&self, char_id: i64) -> Result<Vec<CreatureFrequency>> {
        let ids = self.char_ids_for_merged(char_id)?;
        self.kill_frequency_for_char_ids(&ids, true)
    }

    /// Frequency for a (possibly merged) character with explicit assisted control.
    pub fn kill_frequency_merged_with(&self, char_id: i64, include_assisted: bool) -> Result<Vec<CreatureFrequency>> {
        let ids = self.char_ids_for_merged(char_id)?;
        self.kill_frequency_for_char_ids(&ids, include_assisted)
    }
}

/// Compute both metrics for one creature's time-sorted events.
fn compute_one(creature_name: String, events: &[Event]) -> CreatureFrequency {
    // --- 24h: fixed calendar-day bins ---
    let mut day_counts: BTreeMap<String, BTreeMap<String, i64>> = BTreeMap::new();
    for e in events {
        let day = e.ts_raw.get(0..10).unwrap_or(&e.ts_raw).to_string();
        *day_counts
            .entry(day)
            .or_default()
            .entry(e.verb.clone())
            .or_default() += 1;
    }
    let mut best_day_count = 0i64;
    let mut best_day_date: Option<String> = None;
    let mut best_day_verbs: BTreeMap<String, i64> = BTreeMap::new();
    for (day, verbs) in &day_counts {
        let total: i64 = verbs.values().sum();
        if total > best_day_count {
            best_day_count = total;
            best_day_date = Some(day.clone());
            best_day_verbs = verbs.clone();
        }
    }

    // --- 2h: sliding-window true max (two-pointer over sorted events) ---
    let mut best_2h_count = 0i64;
    let mut best_2h_start: Option<String> = None;
    let mut best_2h_verbs: BTreeMap<String, i64> = BTreeMap::new();
    let mut left = 0usize;
    for right in 0..events.len() {
        while (events[right].ts - events[left].ts).num_seconds() > TWO_HOURS_SECS {
            left += 1;
        }
        let count = (right - left + 1) as i64;
        if count > best_2h_count {
            best_2h_count = count;
            best_2h_start = Some(events[left].ts_raw.clone());
            let mut verbs: BTreeMap<String, i64> = BTreeMap::new();
            for e in &events[left..=right] {
                *verbs.entry(e.verb.clone()).or_default() += 1;
            }
            best_2h_verbs = verbs;
        }
    }

    CreatureFrequency {
        creature_name,
        best_day_count,
        best_day_date,
        best_day_verbs,
        best_2h_count,
        best_2h_start,
        best_2h_verbs,
    }
}

#[cfg(test)]
mod tests {
    use crate::db::queries::Database;

    fn insert(db: &Database, char_id: i64, creature: &str, verb: &str, ts: &str) {
        db.insert_kill_event(char_id, creature, verb, false, ts).unwrap();
    }

    #[test]
    fn best_day_picks_peak_calendar_day() {
        let db = Database::open_in_memory().unwrap();
        let c = db.get_or_create_character("Tester").unwrap();
        insert(&db, c, "Rat", "killed", "2024-01-01 09:00:00");
        insert(&db, c, "Rat", "killed", "2024-01-01 22:00:00");
        insert(&db, c, "Rat", "killed", "2024-01-02 08:00:00");
        insert(&db, c, "Rat", "slaughtered", "2024-01-02 09:00:00");
        insert(&db, c, "Rat", "killed", "2024-01-02 10:00:00");

        let freq = db.kill_frequency_for_char_ids(&[c], true).unwrap();
        let rat = freq.iter().find(|f| f.creature_name == "Rat").unwrap();
        assert_eq!(rat.best_day_count, 3);
        assert_eq!(rat.best_day_date.as_deref(), Some("2024-01-02"));
    }

    #[test]
    fn best_2h_sliding_window_catches_cross_bin_burst() {
        let db = Database::open_in_memory().unwrap();
        let c = db.get_or_create_character("Tester").unwrap();
        insert(&db, c, "Orga", "killed", "2024-01-01 01:00:00");
        insert(&db, c, "Orga", "killed", "2024-01-01 01:30:00");
        insert(&db, c, "Orga", "killed", "2024-01-01 01:59:00");
        insert(&db, c, "Orga", "killed", "2024-01-01 02:30:00");

        let freq = db.kill_frequency_for_char_ids(&[c], true).unwrap();
        let orga = freq.iter().find(|f| f.creature_name == "Orga").unwrap();
        assert_eq!(orga.best_2h_count, 4);
        assert_eq!(orga.best_2h_start.as_deref(), Some("2024-01-01 01:00:00"));
    }

    #[test]
    fn solo_only_excludes_assisted() {
        let db = Database::open_in_memory().unwrap();
        let c = db.get_or_create_character("Tester").unwrap();
        db.insert_kill_event(c, "Rat", "killed", false, "2024-01-01 09:00:00").unwrap();
        db.insert_kill_event(c, "Rat", "killed", true, "2024-01-01 09:30:00").unwrap();

        let with_assist = db.kill_frequency_for_char_ids(&[c], true).unwrap();
        assert_eq!(with_assist[0].best_day_count, 2);

        let solo = db.kill_frequency_for_char_ids(&[c], false).unwrap();
        assert_eq!(solo[0].best_day_count, 1);
    }

    #[test]
    fn per_verb_breakdown_of_best_day() {
        let db = Database::open_in_memory().unwrap();
        let c = db.get_or_create_character("Tester").unwrap();
        insert(&db, c, "Rat", "killed", "2024-01-02 08:00:00");
        insert(&db, c, "Rat", "killed", "2024-01-02 09:00:00");
        insert(&db, c, "Rat", "slaughtered", "2024-01-02 10:00:00");

        let freq = db.kill_frequency_for_char_ids(&[c], true).unwrap();
        let rat = freq.iter().find(|f| f.creature_name == "Rat").unwrap();
        assert_eq!(rat.best_day_count, 3);
        assert_eq!(rat.best_day_verbs.get("killed").copied(), Some(2));
        assert_eq!(rat.best_day_verbs.get("slaughtered").copied(), Some(1));
    }
}
