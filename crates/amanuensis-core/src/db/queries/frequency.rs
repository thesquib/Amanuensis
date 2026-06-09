use std::collections::BTreeMap;

use chrono::NaiveDateTime;
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

struct Bucket {
    hour_raw: String,        // "YYYY-MM-DD HH"
    ts: NaiveDateTime,       // hour start
    total: i64,
    verbs: BTreeMap<String, i64>,
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
        let sql = format!(
            "SELECT creature_name, hour,
                    killed_count, slaughtered_count, vanquished_count, dispatched_count,
                    assisted_kill_count, assisted_slaughter_count, assisted_vanquish_count, assisted_dispatch_count
             FROM kill_hourly
             WHERE character_id IN ({placeholders})
             ORDER BY creature_name, hour",
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(rusqlite::params_from_iter(char_ids.iter()), |row| {
            Ok((
                row.get::<_, String>(0)?,   // creature_name
                row.get::<_, String>(1)?,   // hour
                [
                    row.get::<_, i64>(2)?, row.get::<_, i64>(3)?, row.get::<_, i64>(4)?, row.get::<_, i64>(5)?,
                    row.get::<_, i64>(6)?, row.get::<_, i64>(7)?, row.get::<_, i64>(8)?, row.get::<_, i64>(9)?,
                ],
            ))
        })?;

        // Merge buckets per creature. Multiple char_ids (merged characters) may share a
        // (creature, hour) bucket — accumulate them so the hour total is correct.
        let mut by_creature: BTreeMap<String, BTreeMap<String, Bucket>> = BTreeMap::new();
        for r in rows {
            let (creature, hour_raw, c) = r?;
            // c order: killed, slaughtered, vanquished, dispatched, a_kill, a_slaughter, a_vanquish, a_dispatch
            let verbs_in = [
                ("killed", c[0] + if include_assisted { c[4] } else { 0 }),
                ("slaughtered", c[1] + if include_assisted { c[5] } else { 0 }),
                ("vanquished", c[2] + if include_assisted { c[6] } else { 0 }),
                ("dispatched", c[3] + if include_assisted { c[7] } else { 0 }),
            ];
            let creature_map = by_creature.entry(creature).or_default();
            let bucket = creature_map.entry(hour_raw.clone()).or_insert_with(|| Bucket {
                ts: parse_ts(&format!("{hour_raw}:00:00")),
                hour_raw,
                total: 0,
                verbs: BTreeMap::new(),
            });
            for (name, n) in verbs_in {
                if n > 0 {
                    *bucket.verbs.entry(name.to_string()).or_default() += n;
                    bucket.total += n;
                }
            }
        }

        let mut out = Vec::with_capacity(by_creature.len());
        for (creature_name, bucket_map) in by_creature {
            // bucket_map is a BTreeMap keyed by hour string → already ascending chronological
            // (zero-padded fixed-width "YYYY-MM-DD HH" sorts lexically == chronologically).
            let buckets: Vec<Bucket> = bucket_map.into_values().collect();
            // Skip creatures whose buckets are all empty (can happen in solo-only mode when a
            // creature was only ever assisted): no positive totals → no meaningful stats.
            if buckets.iter().all(|b| b.total == 0) {
                continue;
            }
            out.push(compute_one(creature_name, &buckets));
        }
        out.sort_by(|a, b| {
            b.best_day_count
                .cmp(&a.best_day_count)
                .then(a.creature_name.cmp(&b.creature_name))
        });
        Ok(out)
    }

    /// Frequency for a (possibly merged) character with explicit assisted control.
    pub fn kill_frequency_merged_with(&self, char_id: i64, include_assisted: bool) -> Result<Vec<CreatureFrequency>> {
        let ids = self.char_ids_for_merged(char_id)?;
        self.kill_frequency_for_char_ids(&ids, include_assisted)
    }
}

fn compute_one(creature_name: String, buckets: &[Bucket]) -> CreatureFrequency {
    // --- best calendar day (exact): sum hour buckets within each day ---
    let mut day_total: BTreeMap<String, i64> = BTreeMap::new();
    let mut day_verbs: BTreeMap<String, BTreeMap<String, i64>> = BTreeMap::new();
    for b in buckets {
        let day = b.hour_raw.get(0..10).unwrap_or(&b.hour_raw).to_string();
        *day_total.entry(day.clone()).or_default() += b.total;
        let dv = day_verbs.entry(day).or_default();
        for (v, n) in &b.verbs {
            *dv.entry(v.clone()).or_default() += n;
        }
    }
    let mut best_day_count = 0i64;
    let mut best_day_date: Option<String> = None;
    let mut best_day_verbs: BTreeMap<String, i64> = BTreeMap::new();
    for (day, total) in &day_total {
        if *total > best_day_count {
            best_day_count = *total;
            best_day_date = Some(day.clone());
            best_day_verbs = day_verbs.get(day).cloned().unwrap_or_default();
        }
    }

    // --- best 2h sliding window: at most 2 adjacent hour buckets within a 2h span ---
    let mut best_2h_count = 0i64;
    let mut best_2h_start: Option<String> = None;
    let mut best_2h_verbs: BTreeMap<String, i64> = BTreeMap::new();
    let mut left = 0usize;
    for right in 0..buckets.len() {
        while (buckets[right].ts - buckets[left].ts).num_seconds() >= TWO_HOURS_SECS {
            left += 1;
        }
        let total: i64 = buckets[left..=right].iter().map(|b| b.total).sum();
        if total > best_2h_count {
            best_2h_count = total;
            best_2h_start = Some(format!("{}:00", buckets[left].hour_raw));
            let mut verbs: BTreeMap<String, i64> = BTreeMap::new();
            for b in &buckets[left..=right] {
                for (v, n) in &b.verbs {
                    *verbs.entry(v.clone()).or_default() += n;
                }
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
        // ts is "YYYY-MM-DD HH:MM:SS"; bucket on the hour. verb is the bare verb ("killed", ...).
        let field = format!("{verb}_count");
        let hour = &ts[..13];
        db.upsert_kill_hourly(char_id, creature, &field, hour).unwrap();
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
        assert_eq!(orga.best_2h_start.as_deref(), Some("2024-01-01 01:00"));
    }

    #[test]
    fn solo_only_excludes_assisted() {
        let db = Database::open_in_memory().unwrap();
        let c = db.get_or_create_character("Tester").unwrap();
        db.upsert_kill_hourly(c, "Rat", "killed_count", "2024-01-01 09").unwrap();
        db.upsert_kill_hourly(c, "Rat", "assisted_kill_count", "2024-01-01 09").unwrap();

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
