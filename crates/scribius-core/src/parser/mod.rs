pub mod events;
pub mod line_classifier;
pub mod patterns;
pub mod timestamp;

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

use chrono::Utc;

use crate::data::{CreatureDb, TrainerDb};
use crate::db::Database;
use crate::encoding::decode_log_bytes;
use crate::error::Result;
use crate::parser::events::{KillVerb, LogEvent, LootType};
use crate::parser::line_classifier::classify_line;
use crate::parser::timestamp::parse_timestamp;

/// Main log parser orchestrator.
/// Walks character subdirectories, scans log files, and stores events in the database.
pub struct LogParser {
    creature_db: CreatureDb,
    trainer_db: TrainerDb,
    db: Database,
}

impl LogParser {
    pub fn new(db: Database) -> Result<Self> {
        let creature_db = CreatureDb::bundled()?;
        let trainer_db = TrainerDb::bundled()?;
        Ok(Self {
            creature_db,
            trainer_db,
            db,
        })
    }

    pub fn db(&self) -> &Database {
        &self.db
    }

    /// Scan a log folder. Expects character-named subdirectories containing CL Log files.
    pub fn scan_folder(&self, folder: &Path, force: bool) -> Result<ScanResult> {
        let mut result = ScanResult::default();

        if !folder.is_dir() {
            return Err(crate::error::ScribiusError::Data(format!(
                "Not a directory: {}",
                folder.display()
            )));
        }

        // Find character subdirectories
        let mut entries: Vec<_> = std::fs::read_dir(folder)?
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().map(|ft| ft.is_dir()).unwrap_or(false))
            .collect();
        entries.sort_by_key(|e| e.file_name());

        for entry in entries {
            let dir_name = entry.file_name().to_string_lossy().to_string();
            // Skip hidden dirs and known non-character dirs
            if dir_name.starts_with('.') || dir_name == "CL_Movies" {
                continue;
            }

            let char_name = &dir_name;
            log::info!("Processing character: {}", char_name);
            let char_id = self.db.get_or_create_character(char_name)?;

            // Find log files in this character's directory
            let char_dir = entry.path();
            let mut log_files = find_log_files(&char_dir)?;
            // Sort chronologically by filename (CL Log YYYY:MM:DD HH.MM.SS.txt)
            log_files.sort();

            for log_path in &log_files {
                let path_str = log_path.to_string_lossy().to_string();

                // Skip by path (fast check for exact same file)
                if !force && self.db.is_log_scanned(&path_str)? {
                    result.skipped += 1;
                    continue;
                }

                // Read file bytes for hashing and parsing
                let bytes = match std::fs::read(log_path) {
                    Ok(b) => b,
                    Err(e) => {
                        log::warn!("Error reading {}: {}", path_str, e);
                        result.errors += 1;
                        continue;
                    }
                };

                // Content hash dedup: skip if identical file was already scanned at another path
                let content_hash = hash_bytes(&bytes);
                if !force && self.db.is_hash_scanned(&content_hash)? {
                    log::debug!("Skipping duplicate content: {}", path_str);
                    result.skipped += 1;
                    continue;
                }

                match self.scan_bytes(&bytes, char_id) {
                    Ok(file_result) => {
                        result.files_scanned += 1;
                        result.lines_parsed += file_result.lines_parsed;
                        result.events_found += file_result.events_found;

                        let now = Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
                        self.db
                            .mark_log_scanned(char_id, &path_str, &content_hash, &now)?;
                    }
                    Err(e) => {
                        log::warn!("Error scanning {}: {}", path_str, e);
                        result.errors += 1;
                    }
                }
            }
            result.characters += 1;
        }

        Ok(result)
    }

    /// Scan log file bytes and process events into the database.
    fn scan_bytes(&self, bytes: &[u8], char_id: i64) -> Result<FileResult> {
        let content = decode_log_bytes(bytes);

        let mut file_result = FileResult::default();

        for line in content.lines() {
            file_result.lines_parsed += 1;

            let (ts, message) = match parse_timestamp(line) {
                Some((dt, msg)) => (Some(dt), msg),
                None => (None, line),
            };

            let event = classify_line(message, &self.trainer_db);

            let date_str = ts
                .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
                .unwrap_or_default();

            match event {
                LogEvent::Ignored
                | LogEvent::CoinBalance { .. }
                | LogEvent::ExperienceGain
                | LogEvent::ClanningChange { .. }
                | LogEvent::Disconnect
                | LogEvent::StudyProgress { .. }
                | LogEvent::Recovered { .. } => {}

                LogEvent::Login { .. } => {
                    self.db.increment_character_field(char_id, "logins", 1)?;
                    file_result.events_found += 1;
                }
                LogEvent::Reconnect { .. } => {
                    self.db.increment_character_field(char_id, "logins", 1)?;
                    file_result.events_found += 1;
                }

                LogEvent::SoloKill { creature, verb } => {
                    let field = kill_verb_to_field(&verb, false);
                    let value = self.creature_db.get_value(&creature).unwrap_or(0);
                    self.db
                        .upsert_kill(char_id, &creature, field, value, &date_str)?;
                    file_result.events_found += 1;
                }
                LogEvent::AssistedKill { creature, verb } => {
                    let field = kill_verb_to_field(&verb, true);
                    let value = self.creature_db.get_value(&creature).unwrap_or(0);
                    self.db
                        .upsert_kill(char_id, &creature, field, value, &date_str)?;
                    file_result.events_found += 1;
                }

                LogEvent::Fallen { cause, .. } => {
                    self.db
                        .upsert_kill(char_id, &cause, "killed_by_count", 0, &date_str)?;
                    self.db.increment_character_field(char_id, "deaths", 1)?;
                    file_result.events_found += 1;
                }
                LogEvent::FirstDepart => {
                    self.db.increment_character_field(char_id, "departs", 1)?;
                    file_result.events_found += 1;
                }
                LogEvent::Depart { count } => {
                    // Set departs to the absolute count (it's cumulative)
                    self.db.conn().execute(
                        "UPDATE characters SET departs = ?1 WHERE id = ?2",
                        rusqlite::params![count, char_id],
                    )?;
                    file_result.events_found += 1;
                }

                LogEvent::TrainerRank { trainer_name, .. } => {
                    self.db
                        .upsert_trainer_rank(char_id, &trainer_name, &date_str)?;
                    file_result.events_found += 1;
                }

                LogEvent::CoinsPickedUp { amount } => {
                    self.db
                        .increment_character_field(char_id, "coins_picked_up", amount)?;
                    file_result.events_found += 1;
                }
                LogEvent::LootShare {
                    amount, loot_type, ..
                } => {
                    let field = match loot_type {
                        LootType::Fur => "fur_coins",
                        LootType::Blood => "blood_coins",
                        LootType::Mandible => "mandible_coins",
                        LootType::Other => "bounty_coins",
                    };
                    self.db
                        .increment_character_field(char_id, field, amount)?;
                    file_result.events_found += 1;
                }
                LogEvent::StudyCharge { amount } => {
                    // Track as negative coins (spent on studies)
                    self.db
                        .increment_character_field(char_id, "chest_coins", amount)?;
                    file_result.events_found += 1;
                }

                LogEvent::BellBroken => {
                    self.db
                        .increment_character_field(char_id, "bells_broken", 1)?;
                    file_result.events_found += 1;
                }
                LogEvent::BellUsed => {
                    self.db
                        .increment_character_field(char_id, "bells_used", 1)?;
                    file_result.events_found += 1;
                }
                LogEvent::ChainBreak | LogEvent::ChainShatter | LogEvent::ChainSnap => {
                    self.db
                        .increment_character_field(char_id, "chains_broken", 1)?;
                    file_result.events_found += 1;
                }
                LogEvent::ChainUsed { .. } => {
                    self.db
                        .increment_character_field(char_id, "chains_used", 1)?;
                    file_result.events_found += 1;
                }
                LogEvent::ShieldstoneUsed => {
                    self.db
                        .increment_character_field(char_id, "shieldstones_used", 1)?;
                    file_result.events_found += 1;
                }
                LogEvent::ShieldstoneBroken => {
                    self.db
                        .increment_character_field(char_id, "shieldstones_broken", 1)?;
                    file_result.events_found += 1;
                }
                LogEvent::EtherealPortalOpened | LogEvent::EtherealPortalStoneUsed => {
                    self.db
                        .increment_character_field(char_id, "ethereal_portals", 1)?;
                    file_result.events_found += 1;
                }
            }
        }

        Ok(file_result)
    }
}

/// Compute a hex-encoded hash of file bytes for content-based dedup.
fn hash_bytes(bytes: &[u8]) -> String {
    let mut hasher = DefaultHasher::new();
    bytes.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

fn kill_verb_to_field(verb: &KillVerb, assisted: bool) -> &'static str {
    match (verb, assisted) {
        (KillVerb::Killed, false) => "killed_count",
        (KillVerb::Slaughtered, false) => "slaughtered_count",
        (KillVerb::Vanquished, false) => "vanquished_count",
        (KillVerb::Dispatched, false) => "dispatched_count",
        (KillVerb::Killed, true) => "assisted_kill_count",
        (KillVerb::Slaughtered, true) => "assisted_slaughter_count",
        (KillVerb::Vanquished, true) => "assisted_vanquish_count",
        (KillVerb::Dispatched, true) => "assisted_dispatch_count",
    }
}

/// Find CL Log files in a directory.
fn find_log_files(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with("CL Log ") && name.ends_with(".txt") {
            files.push(entry.path());
        }
    }
    Ok(files)
}

#[derive(Debug, Default)]
pub struct ScanResult {
    pub characters: usize,
    pub files_scanned: usize,
    pub skipped: usize,
    pub lines_parsed: usize,
    pub events_found: usize,
    pub errors: usize,
}

#[derive(Debug, Default)]
struct FileResult {
    pub lines_parsed: usize,
    pub events_found: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn create_test_log_dir() -> (tempfile::TempDir, PathBuf) {
        let tmp = tempfile::tempdir().unwrap();
        let char_dir = tmp.path().join("TestChar");
        fs::create_dir(&char_dir).unwrap();
        (tmp, char_dir)
    }

    #[test]
    fn test_scan_folder_with_kills() {
        let (tmp, char_dir) = create_test_log_dir();

        let log_content = "\
1/1/24 1:00:00p Welcome to Clan Lord, TestChar!
1/1/24 1:01:00p You slaughtered a Rat.
1/1/24 1:02:00p You slaughtered a Rat.
1/1/24 1:03:00p You helped vanquish a Large Vermine.
1/1/24 1:04:00p You have 50 coins.
1/1/24 1:05:00p * You pick up 25 coins.
";
        fs::write(
            char_dir.join("CL Log 2024:01:01 13.00.00.txt"),
            log_content,
        )
        .unwrap();

        let db = Database::open_in_memory().unwrap();
        let parser = LogParser::new(db).unwrap();
        let result = parser.scan_folder(tmp.path(), false).unwrap();

        assert_eq!(result.characters, 1);
        assert_eq!(result.files_scanned, 1);

        let char = parser.db().get_character("TestChar").unwrap().unwrap();
        assert_eq!(char.logins, 1);
        assert_eq!(char.coins_picked_up, 25);

        let char_id = char.id.unwrap();
        let kills = parser.db().get_kills(char_id).unwrap();
        assert_eq!(kills.len(), 2); // Rat + Large Vermine

        let rat = kills.iter().find(|k| k.creature_name == "Rat").unwrap();
        assert_eq!(rat.slaughtered_count, 2);
        assert_eq!(rat.creature_value, 2); // Rat = 2 from creatures.csv

        let vermine = kills
            .iter()
            .find(|k| k.creature_name == "Large Vermine")
            .unwrap();
        assert_eq!(vermine.assisted_vanquish_count, 1);
    }

    #[test]
    fn test_scan_skips_already_read() {
        let (tmp, char_dir) = create_test_log_dir();

        fs::write(
            char_dir.join("CL Log 2024:01:01 13.00.00.txt"),
            "1/1/24 1:00:00p You slaughtered a Rat.\n",
        )
        .unwrap();

        let db = Database::open_in_memory().unwrap();
        let parser = LogParser::new(db).unwrap();

        let r1 = parser.scan_folder(tmp.path(), false).unwrap();
        assert_eq!(r1.files_scanned, 1);
        assert_eq!(r1.skipped, 0);

        let r2 = parser.scan_folder(tmp.path(), false).unwrap();
        assert_eq!(r2.files_scanned, 0);
        assert_eq!(r2.skipped, 1);
    }

    #[test]
    fn test_force_rescan() {
        let (tmp, char_dir) = create_test_log_dir();

        fs::write(
            char_dir.join("CL Log 2024:01:01 13.00.00.txt"),
            "1/1/24 1:00:00p You slaughtered a Rat.\n",
        )
        .unwrap();

        let db = Database::open_in_memory().unwrap();
        let parser = LogParser::new(db).unwrap();

        parser.scan_folder(tmp.path(), false).unwrap();
        let r2 = parser.scan_folder(tmp.path(), true).unwrap();
        assert_eq!(r2.files_scanned, 1);
        assert_eq!(r2.skipped, 0);
    }

    #[test]
    fn test_scan_trainer_ranks() {
        let (tmp, char_dir) = create_test_log_dir();

        let log_content = "\
1/1/24 1:00:00p \u{00a5}Your combat ability improves.
1/1/24 1:01:00p \u{00a5}Your combat ability improves.
1/1/24 1:02:00p \u{00a5}You notice your balance recovering more quickly.
";
        fs::write(
            char_dir.join("CL Log 2024:01:01 13.00.00.txt"),
            log_content,
        )
        .unwrap();

        let db = Database::open_in_memory().unwrap();
        let parser = LogParser::new(db).unwrap();
        parser.scan_folder(tmp.path(), false).unwrap();

        let char_id = parser.db().get_or_create_character("TestChar").unwrap();
        let trainers = parser.db().get_trainers(char_id).unwrap();
        assert_eq!(trainers.len(), 2);

        let bangus = trainers
            .iter()
            .find(|t| t.trainer_name == "Bangus Anmash")
            .unwrap();
        assert_eq!(bangus.ranks, 2);

        let regia = trainers
            .iter()
            .find(|t| t.trainer_name == "Regia")
            .unwrap();
        assert_eq!(regia.ranks, 1);
    }

    #[test]
    fn test_scan_with_speech_filtered() {
        let (tmp, char_dir) = create_test_log_dir();

        let log_content = r#"1/1/24 1:00:00p Donk thinks, "south"
1/1/24 1:01:00p Ruuk says, "hello everyone"
1/1/24 1:02:00p (Ruuk waves)
1/1/24 1:03:00p You slaughtered a Rat.
"#;
        fs::write(
            char_dir.join("CL Log 2024:01:01 13.00.00.txt"),
            log_content,
        )
        .unwrap();

        let db = Database::open_in_memory().unwrap();
        let parser = LogParser::new(db).unwrap();
        let result = parser.scan_folder(tmp.path(), false).unwrap();
        assert_eq!(result.events_found, 1); // Only the kill
    }

    #[test]
    fn test_mac_roman_encoded_file() {
        let (tmp, char_dir) = create_test_log_dir();

        // Build a Mac Roman encoded line: "1/1/24 1:00:00p ¥Your combat ability improves.\n"
        let mut bytes = b"1/1/24 1:00:00p ".to_vec();
        bytes.push(0xA5); // Mac Roman ¥
        bytes.extend_from_slice(b"Your combat ability improves.\n");

        fs::write(
            char_dir.join("CL Log 2024:01:01 13.00.00.txt"),
            &bytes,
        )
        .unwrap();

        let db = Database::open_in_memory().unwrap();
        let parser = LogParser::new(db).unwrap();
        let result = parser.scan_folder(tmp.path(), false).unwrap();
        assert_eq!(result.events_found, 1); // Trainer rank detected

        let char_id = parser.db().get_or_create_character("TestChar").unwrap();
        let trainers = parser.db().get_trainers(char_id).unwrap();
        assert_eq!(trainers.len(), 1);
        assert_eq!(trainers[0].trainer_name, "Bangus Anmash");
    }

    #[test]
    fn test_fallen_death_tracking() {
        let (tmp, char_dir) = create_test_log_dir();

        let log_content = "\
1/1/24 1:00:00p TestChar has fallen to a Large Vermine.
1/1/24 1:01:00p Your spirit has departed your body 5 times.
";
        fs::write(
            char_dir.join("CL Log 2024:01:01 13.00.00.txt"),
            log_content,
        )
        .unwrap();

        let db = Database::open_in_memory().unwrap();
        let parser = LogParser::new(db).unwrap();
        parser.scan_folder(tmp.path(), false).unwrap();

        let char = parser.db().get_character("TestChar").unwrap().unwrap();
        assert_eq!(char.deaths, 1);
        assert_eq!(char.departs, 5);
    }

    #[test]
    fn test_loot_share_tracking() {
        let (tmp, char_dir) = create_test_log_dir();

        let log_content = "\
1/1/24 1:00:00p * Ruuk recovers the Dark Vermine fur, worth 20c. Your share is 10c.
1/1/24 1:01:00p * squib recovers the Orga blood, worth 30c. Your share is 15c.
";
        fs::write(
            char_dir.join("CL Log 2024:01:01 13.00.00.txt"),
            log_content,
        )
        .unwrap();

        let db = Database::open_in_memory().unwrap();
        let parser = LogParser::new(db).unwrap();
        parser.scan_folder(tmp.path(), false).unwrap();

        let char = parser.db().get_character("TestChar").unwrap().unwrap();
        assert_eq!(char.fur_coins, 10);
        assert_eq!(char.blood_coins, 15);
    }
}
