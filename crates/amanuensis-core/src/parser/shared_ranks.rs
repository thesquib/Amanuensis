//! Shared rank messages (spec D2d/D2e): one message, several possible trainers. The scanner
//! only records observations and sign-ups; `derive_shared_ranks` resolves each observation
//! against the character's latest sign-up and rebuilds the shared trainers' counts.
use std::collections::{BTreeMap, HashMap};

use super::LogParser;
use crate::db::queries::{SharedRankObservation, Signup};
use crate::error::Result;

/// The trainer an observation belongs to: the latest applying sign-up's trainer if it is the
/// default or one of its alternates, else the default. `signups` must be one character's,
/// sorted by `(at, file_path, line_no)`.
pub(crate) fn resolve(signups: &[Signup], obs: &SharedRankObservation, alternates: &[String]) -> String {
    let lo = signups.partition_point(|s| s.at < obs.at);
    let hi = signups.partition_point(|s| s.at <= obs.at);
    // Same `at`: only earlier lines of the same file apply, and they beat any older sign-up.
    let chosen: Vec<&Signup> = match signups[lo..hi]
        .iter()
        .rfind(|s| s.file_path == obs.file_path && s.line_no < obs.line_no)
    {
        Some(s) => vec![s],
        None if lo > 0 => {
            let at = &signups[lo - 1].at;
            let start = signups[..lo].partition_point(|s| s.at < *at);
            // Rows are sorted by (file, line) within one `at`: keep each file's last line.
            let group = &signups[start..lo];
            group
                .iter()
                .enumerate()
                .filter(|(i, s)| group.get(i + 1).is_none_or(|n| n.file_path != s.file_path))
                .map(|(_, s)| s)
                .collect()
        }
        None => Vec::new(),
    };
    let Some(first) = chosen.first() else { return obs.default_trainer.clone() };
    if chosen.iter().any(|s| s.trainer != first.trainer) {
        return obs.default_trainer.clone(); // ambiguous across files
    }
    match &first.trainer {
        Some(t) if *t == obs.default_trainer || alternates.contains(t) => t.clone(),
        _ => obs.default_trainer.clone(),
    }
}

impl LogParser {
    /// Rebuild every shared trainer's count from the stored observations (spec D2e).
    /// Idempotent; lifetime counts and the Last-scan shadow commit together or not at all.
    pub(crate) fn derive_shared_ranks(&self) -> Result<()> {
        let all = self.db.all_shared_rank_observations()?;
        let mut by_char: BTreeMap<i64, Vec<&SharedRankObservation>> = BTreeMap::new();
        for o in &all {
            by_char.entry(o.character_id).or_default().push(o);
        }

        // Steps 1-2: resolve, then apply the resolved trainer's current override config.
        // (char, trainer) -> (count, max at)
        let mut counts: BTreeMap<(i64, String), (i64, String)> = BTreeMap::new();
        // (file, line) -> (char, trainer, at, counted)
        let mut resolved: HashMap<(&str, i64), (i64, String, &str, bool)> = HashMap::new();
        for (&cid, list) in &by_char {
            self.load_override_config(cid)?;
            let signups = self.db.signups_for(cid)?;
            for o in list {
                let t = resolve(&signups, o, self.trainer_db.alternates_of(&o.default_trainer));
                let counted = self.should_count_rank(cid, &t, &o.at);
                if counted {
                    let e = counts.entry((cid, t.clone())).or_insert((0, String::new()));
                    e.0 += 1;
                    if o.at > e.1 {
                        e.1 = o.at.clone();
                    }
                }
                resolved.insert((o.file_path.as_str(), o.line_no), (cid, t, o.at.as_str(), counted));
            }
        }
        for key in self.db.trainers_with_shared_ranks()? {
            counts.entry(key).or_insert((0, String::new()));
        }
        let counts: Vec<(i64, String, i64, String)> =
            counts.into_iter().map(|((c, t), (n, at))| (c, t, n, at)).collect();

        let mut new_counted: Vec<(i64, &str, &str)> = Vec::new();
        let mut new_skipped: BTreeMap<(i64, &str), i64> = BTreeMap::new();
        let pending = self.new_shared_obs.borrow().clone();
        for (file, line) in &pending {
            if let Some((cid, t, at, counted)) = resolved.get(&(file.as_str(), *line)) {
                if *counted {
                    new_counted.push((*cid, t, at));
                } else {
                    *new_skipped.entry((*cid, t)).or_insert(0) += 1;
                }
            }
        }
        new_counted.sort_by_key(|&(_, _, at)| at);

        // Steps 3-4, in one SAVEPOINT.
        let conn = self.db.conn();
        conn.execute_batch("SAVEPOINT derive_shared")?;
        let prev = self.scan_token.get();
        let res = (|| -> Result<()> {
            self.db.apply_shared_counts(&counts, |t| self.trainer_db.get_multiplier(t))?;
            #[cfg(test)]
            self.fail_at("after_lifetime")?;
            if !new_counted.is_empty() {
                self.note_scan_write()?;
                for &(cid, t, at) in &new_counted {
                    self.db.upsert_trainer_rank_scan(cid, t, at, self.trainer_db.get_multiplier(t))?;
                }
            }
            #[cfg(test)]
            self.fail_at("after_shadow")?;
            conn.execute_batch("RELEASE derive_shared")?;
            Ok(())
        })();
        if let Err(e) = res {
            let _ = conn.execute_batch("ROLLBACK TO derive_shared; RELEASE derive_shared");
            self.scan_token.set(prev);
            return Err(e);
        }
        self.new_shared_obs.borrow_mut().clear();

        // Step 5: report this operation's override-skipped observations by resolved trainer.
        for ((cid, trainer), count) in new_skipped {
            // Non-fatal: the derive has already committed.
            let char_name = self.db.get_character_by_id(cid).ok().flatten().map(|c| c.name).unwrap_or_else(|| cid.to_string());
            let _ = self.db.add_process_log(
                "info",
                &format!("Skipped {count} rank(s) for {trainer} ({char_name}) — override active"),
            );
        }
        Ok(())
    }

    #[cfg(test)]
    fn fail_at(&self, point: &'static str) -> Result<()> {
        if self.fail_after.get() == Some(point) {
            return Err(crate::error::AmanuensisError::Data(format!("injected failure {point}")));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::queries::{read_overrides_file, write_overrides_file, RankOverride, ScanScope};
    use crate::db::Database;
    use std::fs;
    use std::path::Path;

    // ---- resolve ----

    fn su(file: &str, line: i64, at: &str, t: Option<&str>) -> Signup {
        Signup { file_path: file.into(), line_no: line, at: at.into(), trainer: t.map(Into::into) }
    }

    fn ob(file: &str, line: i64, at: &str) -> SharedRankObservation {
        SharedRankObservation {
            file_path: file.into(), line_no: line, character_id: 1, at: at.into(),
            default_trainer: "Histia".into(),
        }
    }

    const T1: &str = "2017-11-22 10:00:00";
    const T2: &str = "2017-11-23 10:00:00";

    fn r(signups: &[Signup], o: &SharedRankObservation) -> String {
        resolve(signups, o, &["Higgrus".to_string()])
    }

    #[test]
    fn resolve_no_signups_gives_default() {
        assert_eq!(r(&[], &ob("b", 0, T2)), "Histia");
    }

    #[test]
    fn resolve_higgrus_signup() {
        assert_eq!(r(&[su("a", 3, T1, Some("Higgrus"))], &ob("b", 0, T2)), "Higgrus");
    }

    #[test]
    fn resolve_player_teacher_after_higgrus_gives_default() {
        let s = [su("a", 3, T1, Some("Higgrus")), su("a", 9, "2017-11-22 11:00:00", None)];
        assert_eq!(r(&s, &ob("b", 0, T2)), "Histia");
    }

    #[test]
    fn resolve_later_line_in_timestampless_file_wins() {
        let s = [su("a", 1, T1, Some("Higgrus")), su("a", 5, T1, Some("Detha"))];
        assert_eq!(r(&s, &ob("a", 9, T1)), "Histia");
        assert_eq!(r(&s, &ob("b", 0, T2)), "Histia");
        // Between the two: only Higgrus applies.
        assert_eq!(r(&s, &ob("a", 3, T1)), "Higgrus");
    }

    #[test]
    fn resolve_equal_at_two_files() {
        let differ = [su("a", 1, T1, Some("Higgrus")), su("b", 1, T1, Some("Detha"))];
        assert_eq!(r(&differ, &ob("c", 0, T2)), "Histia");
        let same = [su("a", 1, T1, Some("Higgrus")), su("a", 7, T1, Some("Higgrus")), su("b", 1, T1, Some("Higgrus"))];
        assert_eq!(r(&same, &ob("c", 0, T2)), "Higgrus");
    }

    #[test]
    fn resolve_same_at_later_line_does_not_apply() {
        assert_eq!(r(&[su("a", 5, T1, Some("Higgrus"))], &ob("a", 3, T1)), "Histia");
        // ...nor does an equal-`at` sign-up in another file.
        assert_eq!(r(&[su("b", 1, T1, Some("Higgrus"))], &ob("a", 3, T1)), "Histia");
    }

    // ---- scan + finalize ----

    const WELCOME: &str = "11/22/17 10:00:00p Welcome to Clan Lord, Squib!\n";
    const SIGNUP: &str = "\
11/22/17 10:01:00p Higgrus asks, \"Are you sure you would like to train with me?\"
11/22/17 10:01:01p Squib says, \"yes\"
11/22/17 10:01:02p Higgrus says, \"Welcome aboard. Go into the world and learn what you may, while practicing our craft.\"
";
    const RANK: &str = "11/22/17 10:02:00p ¥You seem to be able to take more damage now.\n";
    const FILE: &str = "CL Log 2017-11-22 22.00.00.txt";

    fn shared_log() -> String {
        format!("{WELCOME}{SIGNUP}{RANK}")
    }

    fn write(root: &Path, file: &str, body: &str) -> String {
        let dir = root.join("Squib");
        fs::create_dir_all(&dir).unwrap();
        let p = dir.join(file);
        fs::write(&p, body).unwrap();
        p.to_string_lossy().into_owned()
    }

    fn parser() -> LogParser {
        LogParser::new(Database::open_in_memory().unwrap()).unwrap()
    }

    fn scan(p: &LogParser, root: &Path) {
        p.scan_folder(root, false).unwrap();
    }

    fn cid(p: &LogParser, name: &str) -> i64 {
        p.db().get_or_create_character(name).unwrap()
    }

    fn ranks(p: &LogParser, name: &str, trainer: &str, scope: ScanScope) -> i64 {
        p.db().get_trainers_scoped(cid(p, name), scope).unwrap().iter()
            .find(|t| t.trainer_name == trainer).map(|t| t.ranks).unwrap_or(0)
    }

    fn all(p: &LogParser, name: &str, trainer: &str) -> i64 {
        ranks(p, name, trainer, ScanScope::All)
    }

    fn dump(db: &Database, sql: &str) -> Vec<String> {
        let mut stmt = db.conn().prepare(sql).unwrap();
        let n = stmt.column_count();
        stmt.query_map([], |row| {
            Ok((0..n).map(|i| format!("{:?}", row.get_ref(i).unwrap())).collect::<Vec<_>>().join("|"))
        }).unwrap().map(|r| r.unwrap()).collect()
    }

    const TRAINERS: &str = "SELECT * FROM trainers ORDER BY character_id, trainer_name";
    const SCAN_TRAINERS: &str = "SELECT * FROM scan_trainers ORDER BY character_id, trainer_name";
    const SCAN_KILLS: &str = "SELECT * FROM scan_kills ORDER BY 1, 2";
    const META: &str = "SELECT key, value FROM db_meta WHERE key IN ('last_scan_token', 'scan_token_counter') ORDER BY key";

    fn logs(p: &LogParser) -> Vec<String> {
        p.db().get_process_logs().unwrap().into_iter().map(|l| l.message).collect()
    }

    #[test]
    fn higgrus_credited_after_finalize() {
        let t = tempfile::tempdir().unwrap();
        write(t.path(), FILE, &shared_log());
        let p = parser();
        scan(&p, t.path());
        p.finalize_characters().unwrap();
        assert_eq!(all(&p, "Squib", "Higgrus"), 1);
        assert_eq!(all(&p, "Squib", "Histia"), 0);
        assert_eq!(ranks(&p, "Squib", "Higgrus", ScanScope::LastScan), 1);
        assert_eq!(ranks(&p, "Squib", "Histia", ScanScope::LastScan), 0);
    }

    #[test]
    fn order_independent_across_sources() {
        let a = tempfile::tempdir().unwrap();
        write(a.path(), "CL Log 2017-11-23 10.00.00.txt",
            "11/23/17 10:00:00a Welcome to Clan Lord, Squib!\n11/23/17 10:02:00a ¥You seem to be able to take more damage now.\n");
        let b = tempfile::tempdir().unwrap();
        write(b.path(), FILE, &format!("{WELCOME}{SIGNUP}"));
        let run = |order: [&Path; 2]| {
            let p = parser();
            for root in order {
                scan(&p, root);
                p.finalize_characters().unwrap();
            }
            (all(&p, "Squib", "Histia"), all(&p, "Squib", "Higgrus"))
        };
        assert_eq!(run([a.path(), b.path()]), (0, 1));
        assert_eq!(run([b.path(), a.path()]), (0, 1));
    }

    #[test]
    fn finalize_twice_is_idempotent() {
        let t = tempfile::tempdir().unwrap();
        write(t.path(), FILE, &shared_log());
        let p = parser();
        scan(&p, t.path());
        p.finalize_characters().unwrap();
        let before = (dump(p.db(), TRAINERS), dump(p.db(), SCAN_TRAINERS));
        p.finalize_characters().unwrap();
        assert_eq!(before, (dump(p.db(), TRAINERS), dump(p.db(), SCAN_TRAINERS)));
    }

    #[test]
    fn fresh_parser_finalize_recovers() {
        let t = tempfile::tempdir().unwrap();
        write(t.path(), FILE, &shared_log());
        let db_path = t.path().join("a.db").to_string_lossy().into_owned();
        {
            let a = LogParser::new(Database::open(&db_path).unwrap()).unwrap();
            scan(&a, t.path());
            assert_eq!(all(&a, "Squib", "Higgrus"), 0);
        }
        let b = LogParser::new(Database::open(&db_path).unwrap()).unwrap();
        b.finalize_characters().unwrap();
        assert_eq!(all(&b, "Squib", "Higgrus"), 1);
        assert_eq!(all(&b, "Squib", "Histia"), 0);
    }

    #[test]
    fn override_follows_resolved_trainer() {
        // Histia override: the Higgrus rank still counts, and nothing is reported skipped.
        let t = tempfile::tempdir().unwrap();
        write(t.path(), FILE, &shared_log());
        let p = parser();
        p.db().set_rank_override(cid(&p, "Squib"), "Histia", "override", 0, None).unwrap();
        scan(&p, t.path());
        p.finalize_characters().unwrap();
        assert_eq!(all(&p, "Squib", "Higgrus"), 1);
        assert!(!logs(&p).iter().any(|m| m.starts_with("Skipped")));

        // Higgrus override: skipped, and the report names Higgrus.
        let p = parser();
        p.db().set_rank_override(cid(&p, "Squib"), "Higgrus", "override", 0, None).unwrap();
        scan(&p, t.path());
        p.finalize_characters().unwrap();
        assert_eq!((all(&p, "Squib", "Higgrus"), all(&p, "Squib", "Histia")), (0, 0));
        assert!(logs(&p).contains(&"Skipped 1 rank(s) for Higgrus (Squib) — override active".to_string()));
    }

    #[test]
    fn override_transitions() {
        let mut body = format!("{WELCOME}{SIGNUP}");
        for i in 0..8 {
            body += &format!("11/22/17 10:1{i}:00p ¥You seem to be able to take more damage now.\n");
        }
        for i in 0..2 {
            body += &format!("11/23/17 10:1{i}:00a ¥You seem to be able to take more damage now.\n");
        }
        let t = tempfile::tempdir().unwrap();
        write(t.path(), FILE, &body);
        let p = parser();
        scan(&p, t.path());
        p.finalize_characters().unwrap();
        let id = cid(&p, "Squib");
        assert_eq!(all(&p, "Squib", "Higgrus"), 10);

        p.db().set_rank_override(id, "Higgrus", "override_until_date", 0, Some("11/22/17")).unwrap();
        p.finalize_characters().unwrap();
        assert_eq!(all(&p, "Squib", "Higgrus"), 2);

        p.db().set_rank_override(id, "Higgrus", "modifier", 0, None).unwrap();
        p.finalize_characters().unwrap();
        assert_eq!(all(&p, "Squib", "Higgrus"), 10);

        p.db().set_rank_override(id, "Atkus", "modifier", 5, None).unwrap();
        p.db().move_rank_override(id, "Atkus", "Higgrus").unwrap();
        p.finalize_characters().unwrap();
        let h = p.db().get_trainers(id).unwrap().into_iter().find(|t| t.trainer_name == "Higgrus").unwrap();
        assert_eq!((h.ranks, h.modified_ranks), (10, 5));

        p.db().clear_rank_overrides(Some(id)).unwrap();
        p.finalize_characters().unwrap();
        assert_eq!(all(&p, "Squib", "Higgrus"), 10);
        assert_eq!(p.db().shared_ranks_of(id, "Higgrus").unwrap(), 10);
    }

    #[test]
    fn backup_only_higgrus_override_restorable() {
        let t = tempfile::tempdir().unwrap();
        write(t.path(), FILE, &shared_log());
        let backup = t.path().join("rank-overrides-backup.json");
        write_overrides_file(&backup, &[RankOverride {
            character: "Squib".into(), trainer: "Higgrus".into(), modified_ranks: 30,
            rank_mode: "modifier".into(), override_date: None,
        }]).unwrap();
        let p = parser();
        scan(&p, t.path());
        p.finalize_characters().unwrap();
        let id = cid(&p, "Squib");
        assert_eq!(all(&p, "Squib", "Higgrus"), 1);
        assert_eq!(p.db().missing_rank_overrides(&backup, id).unwrap().len(), 1);
        p.db().restore_missing_rank_overrides(&backup, id).unwrap();
        p.finalize_characters().unwrap();
        let h = p.db().get_trainers(id).unwrap().into_iter().find(|t| t.trainer_name == "Higgrus").unwrap();
        assert_eq!((h.ranks, h.modified_ranks), (1, 30));
    }

    #[test]
    fn skipped_only_operation_keeps_last_scan() {
        let t = tempfile::tempdir().unwrap();
        write(t.path(), FILE, &format!("{WELCOME}11/22/17 10:05:00p You slaughtered a Rat.\n"));
        let p = parser();
        scan(&p, t.path());
        p.finalize_characters().unwrap();
        let before = (dump(p.db(), SCAN_KILLS), dump(p.db(), SCAN_TRAINERS), dump(p.db(), META));
        assert_eq!(before.0.len(), 1);

        p.db().set_rank_override(cid(&p, "Squib"), "Histia", "override", 0, None).unwrap();
        write(t.path(), "CL Log 2017-11-23 10.00.00.txt",
            "11/23/17 10:00:00a Welcome to Clan Lord, Squib!\n11/23/17 10:02:00a ¥You seem to be able to take more damage now.\n");
        scan(&p, t.path());
        p.finalize_characters().unwrap();
        assert_eq!(before, (dump(p.db(), SCAN_KILLS), dump(p.db(), SCAN_TRAINERS), dump(p.db(), META)));
        assert!(logs(&p).contains(&"Skipped 1 rank(s) for Histia (Squib) — override active".to_string()));
    }

    #[test]
    fn derive_failure_rolls_back_everything() {
        let t = tempfile::tempdir().unwrap();
        write(t.path(), "CL Log 2017-11-21 22.00.00.txt",
            "11/21/17 10:00:00p Welcome to Clan Lord, Squib!\n11/21/17 10:05:00p You slaughtered a Rat.\n");
        let p = parser();
        scan(&p, t.path());
        p.finalize_characters().unwrap();
        write(t.path(), FILE, &shared_log());
        scan(&p, t.path());

        let snap = |p: &LogParser| {
            (dump(p.db(), TRAINERS), dump(p.db(), SCAN_TRAINERS), dump(p.db(), SCAN_KILLS),
             dump(p.db(), META), p.scan_token.get(), p.new_shared_obs.borrow().clone())
        };
        let before = snap(&p);
        assert_eq!(before.2.len(), 1);
        assert_eq!(before.5.len(), 1);
        for point in ["after_lifetime", "after_shadow"] {
            p.fail_after.set(Some(point));
            assert!(p.finalize_characters().is_err(), "{point}");
            assert_eq!(snap(&p), before, "{point}");
        }
        p.fail_after.set(None);
        p.finalize_characters().unwrap();
        assert_eq!(all(&p, "Squib", "Higgrus"), 1);
        assert_eq!(ranks(&p, "Squib", "Higgrus", ScanScope::LastScan), 1);
        assert!(dump(p.db(), SCAN_KILLS).is_empty());
        assert!(p.new_shared_obs.borrow().is_empty());
    }

    const IMPORT_SQL: &str = "INSERT OR IGNORE INTO trainers (character_id, trainer_name, ranks, modified_ranks, date_of_last_rank, effective_multiplier, notes)
         VALUES (?1, 'Histia', 40, 0, NULL, 1.0, NULL)";
    const TWO_RANKS: &str = "\
11/22/17 10:00:00p Welcome to Clan Lord, Squib!
11/22/17 10:02:00p ¥You seem to be able to take more damage now.
11/22/17 10:03:00p ¥You seem to be able to take more damage now.
";

    #[test]
    fn imported_row_preserved() {
        let t = tempfile::tempdir().unwrap();
        write(t.path(), FILE, TWO_RANKS);
        let p = parser();
        p.db().conn().execute(IMPORT_SQL, [cid(&p, "Squib")]).unwrap();
        scan(&p, t.path());
        p.finalize_characters().unwrap();
        assert_eq!(all(&p, "Squib", "Histia"), 42);

        // Reverse: importing over a scanned row is ignored and derivation is unchanged.
        let p = parser();
        scan(&p, t.path());
        p.finalize_characters().unwrap();
        let id = cid(&p, "Squib");
        p.db().conn().execute(IMPORT_SQL, [id]).unwrap();
        p.finalize_characters().unwrap();
        assert_eq!(all(&p, "Squib", "Histia"), 2);
        assert_eq!(p.db().shared_ranks_of(id, "Histia").unwrap(), 2);
    }

    #[test]
    fn partial_file_failure_retry_counts_once() {
        // The failed first pass committed its sign-up and observation but not the log_files
        // row, so the retry re-reads the whole file.
        let t = tempfile::tempdir().unwrap();
        let path = write(t.path(), FILE, &shared_log());
        let p = parser();
        let id = cid(&p, "Squib");
        p.db().insert_trainer_signup(&path, 3, id, "2017-11-22 22:01:02", Some("Higgrus")).unwrap();
        p.db().insert_shared_rank_observation(&path, 4, id, "2017-11-22 22:02:00", "Histia").unwrap();
        // Last scan is deliberately not asserted: the pre-inserted observation isn't new to this operation.
        scan(&p, t.path());
        p.finalize_characters().unwrap();
        assert_eq!(p.db().all_shared_rank_observations().unwrap().len(), 1);
        assert_eq!((all(&p, "Squib", "Higgrus"), all(&p, "Squib", "Histia")), (1, 0));
    }

    fn signup_by(trainer: &str, yes_by: &str) -> String {
        format!("11/22/17 10:01:00p {trainer} asks, \"Are you sure you would like to train with me?\"
11/22/17 10:01:01p {yes_by} says, \"yes\"
11/22/17 10:01:02p {trainer} says, \"Welcome aboard. Go into the world and learn what you may.\"
")
    }

    fn derive(body: &str) -> LogParser {
        let t = tempfile::tempdir().unwrap();
        write(t.path(), FILE, body);
        let p = parser();
        scan(&p, t.path());
        p.finalize_characters().unwrap();
        p
    }

    const BODY_LINE: &str = "11/22/17 10:02:00p ¥Your body feels stronger.\n";

    #[test]
    fn hardia_credited_and_other_trainer_gives_bodrus() {
        let p = derive(&format!("{WELCOME}{}{BODY_LINE}", signup_by("Hardia", "Squib")));
        assert_eq!((all(&p, "Squib", "Hardia"), all(&p, "Squib", "Master Bodrus")), (1, 0));
        let p = derive(&format!("{WELCOME}{}{BODY_LINE}", signup_by("Detha", "Squib")));
        assert_eq!((all(&p, "Squib", "Hardia"), all(&p, "Squib", "Master Bodrus")), (0, 1));
    }

    #[test]
    fn bystander_yes_gives_default() {
        let p = derive(&format!("{WELCOME}{}{RANK}", signup_by("Higgrus", "Garr")));
        assert_eq!((all(&p, "Squib", "Histia"), all(&p, "Squib", "Higgrus")), (1, 0));
    }

    #[test]
    fn player_teacher_supersedes_higgrus() {
        let p = derive(&format!(
            "{WELCOME}{SIGNUP}11/22/17 10:01:30p ¥ You begin training with Somebody.\n{RANK}"
        ));
        assert_eq!((all(&p, "Squib", "Histia"), all(&p, "Squib", "Higgrus")), (1, 0));
    }

    #[test]
    fn force_rescan_twice_counts_once() {
        let t = tempfile::tempdir().unwrap();
        write(t.path(), FILE, &shared_log());
        let p = parser();
        p.scan_folder(t.path(), true).unwrap();
        p.scan_folder(t.path(), true).unwrap();
        p.finalize_characters().unwrap();
        assert_eq!((all(&p, "Squib", "Higgrus"), all(&p, "Squib", "Histia")), (1, 0));
    }

    #[test]
    fn signup_does_not_cross_characters() {
        let t = tempfile::tempdir().unwrap();
        write(t.path(), FILE, &format!("{WELCOME}{SIGNUP}11/22/17 10:01:30p Welcome to Clan Lord, Ruuk!\n{RANK}"));
        let p = parser();
        scan(&p, t.path());
        p.finalize_characters().unwrap();
        assert_eq!((all(&p, "Ruuk", "Histia"), all(&p, "Ruuk", "Higgrus")), (1, 0));
        assert_eq!((all(&p, "Squib", "Histia"), all(&p, "Squib", "Higgrus")), (0, 0));
    }

    #[test]
    fn legacy_db_finalize_is_noop() {
        let p = parser();
        let id = cid(&p, "Squib");
        p.db().conn().execute(
            "INSERT INTO trainers (character_id, trainer_name, ranks, date_of_last_rank) VALUES
             (?1, 'Histia', 5, '2017-01-01 00:00:00'), (?1, 'Atkus', 3, NULL)",
            [id],
        ).unwrap();
        let before = dump(p.db(), TRAINERS);
        p.finalize_characters().unwrap();
        assert_eq!(dump(p.db(), TRAINERS), before);
    }

    #[test]
    fn user_fixes_never_written() {
        let t = tempfile::tempdir().unwrap();
        write(t.path(), FILE, &format!("{}11/22/17 10:03:00p ¥Your body feels stronger.\n", shared_log()));
        let backup = t.path().join("rank-overrides-backup.json");
        let p = parser();
        let id = cid(&p, "Squib");
        let source = cid(&p, "Squibby");
        p.db().merge_characters(&[source], id).unwrap();
        p.db().set_rank_override(id, "Histia", "modifier", -20, None).unwrap();
        p.db().set_rank_override(id, "Higgrus", "override", 150, None).unwrap();
        p.db().set_trainer_note(id, "Hardia", Some("these are really Hardia")).unwrap();
        p.db().set_profession_override(id, Some("Healer")).unwrap();
        p.db().backup_all_rank_overrides(&backup).unwrap();
        let backed_up = read_overrides_file(&backup).unwrap();
        assert_eq!(backed_up.len(), 2);

        const USER_TRAINERS: &str = "SELECT character_id, trainer_name, modified_ranks, rank_mode, override_date, notes
             FROM trainers WHERE modified_ranks != 0 OR rank_mode != 'modifier' OR override_date IS NOT NULL OR notes IS NOT NULL
             ORDER BY 1, 2";
        const USER_CHARS: &str = "SELECT id, profession_override, merged_into FROM characters
             WHERE profession_override IS NOT NULL OR merged_into IS NOT NULL ORDER BY id";
        let before = (dump(p.db(), USER_TRAINERS), dump(p.db(), USER_CHARS));
        assert_eq!((before.0.len(), before.1.len()), (3, 2));

        scan(&p, t.path());
        p.finalize_characters().unwrap();
        assert_eq!(all(&p, "Squib", "Master Bodrus"), 1);
        p.db().reset_log_data().unwrap();
        scan(&p, t.path());
        p.finalize_characters().unwrap();

        assert_eq!((dump(p.db(), USER_TRAINERS), dump(p.db(), USER_CHARS)), before);
        assert_eq!(read_overrides_file(&backup).unwrap(), backed_up);
    }
}
