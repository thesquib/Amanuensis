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
use crate::models::Profession;
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

    /// Consume the parser and return the inner Database.
    pub fn into_db(self) -> Database {
        self.db
    }

    /// Scan a log folder. Expects character-named subdirectories containing CL Log files.
    pub fn scan_folder(&self, folder: &Path, force: bool) -> Result<ScanResult> {
        let mut result = ScanResult::default();

        if !folder.is_dir() {
            return Err(crate::error::AmanuensisError::Data(format!(
                "Not a directory: {}",
                folder.display()
            )));
        }

        self.db.set_scan_pragmas()?;
        self.db.begin_transaction()?;

        let scan_result = self.scan_folder_inner(folder, force, &mut result);

        match scan_result {
            Ok(()) => {
                self.db.commit_transaction()?;
                self.db.reset_pragmas()?;
            }
            Err(e) => {
                let _ = self.db.rollback_transaction();
                let _ = self.db.reset_pragmas();
                return Err(e);
            }
        }

        Ok(result)
    }

    fn scan_folder_inner(&self, folder: &Path, force: bool, result: &mut ScanResult) -> Result<()> {
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

            // Find log files BEFORE creating a character record
            let char_dir = entry.path();
            let mut log_files = find_log_files(&char_dir)?;
            if log_files.is_empty() {
                log::debug!("Skipping directory with no CL Log files: {}", dir_name);
                continue;
            }
            // Sort chronologically by filename (CL Log YYYY:MM:DD HH.MM.SS.txt)
            log_files.sort();

            // Try to extract character name from welcome message in earliest log files
            let char_name = log_files
                .iter()
                .find_map(|path| {
                    std::fs::read(path)
                        .ok()
                        .and_then(|bytes| extract_character_name(&bytes))
                })
                .unwrap_or_else(|| dir_name.clone());

            log::info!("Processing character: {}", char_name);
            let char_id = self.db.get_or_create_character(&char_name)?;

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

                match self.scan_bytes(&bytes, char_id, &char_name, &path_str, true) {
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

        Ok(())
    }

    /// Scan log file bytes and process events into the database.
    fn scan_bytes(
        &self,
        bytes: &[u8],
        char_id: i64,
        char_name: &str,
        file_path: &str,
        index_lines: bool,
    ) -> Result<FileResult> {
        let content = decode_log_bytes(bytes);

        let mut file_result = FileResult::default();
        let mut found_login = false;
        let mut first_date_str: Option<String> = None;
        let mut log_lines: Vec<(i64, String, String, String)> = Vec::new();

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

            if index_lines && !line.trim().is_empty() {
                log_lines.push((
                    char_id,
                    line.to_string(),
                    date_str.clone(),
                    file_path.to_string(),
                ));
            }

            // Track first timestamp in file for file-as-login fallback
            if first_date_str.is_none() && !date_str.is_empty() {
                first_date_str = Some(date_str.clone());
            }

            match event {
                LogEvent::Ignored
                | LogEvent::CoinBalance { .. }
                | LogEvent::ExperienceGain
                | LogEvent::ClanningChange { .. }
                | LogEvent::Disconnect
                | LogEvent::StudyProgress { .. }
                | LogEvent::StudyAbandon { .. }
                | LogEvent::Recovered { .. } => {}

                LogEvent::Login { .. } | LogEvent::Reconnect { .. } => {
                    found_login = true;
                    if !date_str.is_empty() {
                        self.db.update_start_date(char_id, &date_str)?;
                    }
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

                LogEvent::Fallen { name, cause } => {
                    if name.eq_ignore_ascii_case(char_name) {
                        self.db
                            .upsert_kill(char_id, &cause, "killed_by_count", 0, &date_str)?;
                        self.db.increment_character_field(char_id, "deaths", 1)?;
                        file_result.events_found += 1;
                    }
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
                    worth, amount, loot_type, ..
                } => {
                    let (share_field, worth_field) = match loot_type {
                        LootType::Fur => ("fur_coins", "fur_worth"),
                        LootType::Blood => ("blood_coins", "blood_worth"),
                        LootType::Mandible => ("mandible_coins", "mandible_worth"),
                        LootType::Other => ("bounty_coins", "bounty_coins"), // no separate worth for Other
                    };
                    self.db
                        .increment_character_field(char_id, share_field, amount)?;
                    if worth_field != share_field {
                        self.db
                            .increment_character_field(char_id, worth_field, worth)?;
                    }
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
                LogEvent::EtherealPortalOpened => {
                    self.db
                        .increment_character_field(char_id, "ethereal_portals", 1)?;
                    file_result.events_found += 1;
                }
                LogEvent::EtherealPortalStoneUsed => {
                    self.db
                        .increment_character_field(char_id, "ethereal_portals", 1)?;
                    self.db
                        .increment_character_field(char_id, "eps_broken", 1)?;
                    file_result.events_found += 1;
                }

                LogEvent::KarmaReceived { good } => {
                    let field = if good { "good_karma" } else { "bad_karma" };
                    self.db
                        .increment_character_field(char_id, field, 1)?;
                    file_result.events_found += 1;
                }
                LogEvent::EsteemGain => {
                    self.db
                        .increment_character_field(char_id, "esteem", 1)?;
                    file_result.events_found += 1;
                }
                LogEvent::ProfessionAnnouncement { name, profession } => {
                    if name.eq_ignore_ascii_case(char_name) {
                        self.db
                            .update_character_profession(char_id, &profession)?;
                    }
                    file_result.events_found += 1;
                }

                LogEvent::LastyProgress {
                    creature,
                    lasty_type,
                } => {
                    self.db.upsert_lasty(char_id, &creature, &lasty_type)?;
                    file_result.events_found += 1;
                }
                LogEvent::LastyCompleted { trainer } => {
                    self.db.complete_lasty(char_id, &trainer)?;
                    file_result.events_found += 1;
                }
                LogEvent::ApplyLearningRank { character_name, trainer_name, is_full } => {
                    // Only count apply-learning ranks for the current character
                    // (other characters' ranks can appear in our logs)
                    if character_name.eq_ignore_ascii_case(char_name) {
                        if is_full {
                            // "much more" = exactly 10 confirmed bonus ranks
                            self.db
                                .upsert_apply_learning(char_id, &trainer_name, &date_str, 10)?;
                        } else {
                            // "more" = 1-9 unknown bonus ranks, just count occurrences
                            self.db
                                .upsert_apply_learning_unknown(char_id, &trainer_name, &date_str)?;
                        }
                        file_result.events_found += 1;
                    }
                }
            }
        }

        // Every scanned file counts as exactly 1 login (matching Scribius behavior).
        self.db.increment_character_field(char_id, "logins", 1)?;
        // If no Login/Reconnect had a timestamp, use the file's first timestamp for start_date
        if !found_login {
            if let Some(ref first_ts) = first_date_str {
                self.db.update_start_date(char_id, first_ts)?;
            }
        }

        // Batch-insert log lines into FTS5 index
        if index_lines && !log_lines.is_empty() {
            for chunk in log_lines.chunks(1000) {
                let refs: Vec<(i64, &str, &str, &str)> = chunk
                    .iter()
                    .map(|(id, content, ts, fp)| (*id, content.as_str(), ts.as_str(), fp.as_str()))
                    .collect();
                self.db.insert_log_lines(&refs)?;
            }
        }

        Ok(file_result)
    }

    /// Determine profession for a character based on their trained trainers.
    /// Uses the original app's logic: check each trainer against the profession mapping,
    /// and use the first profession-bearing trainer found (last-writer-wins through iteration).
    pub fn determine_profession(&self, char_id: i64) -> Result<Profession> {
        let trainers = self.db.get_trainers(char_id)?;

        // Count ranks per profession
        let mut fighter_ranks: i64 = 0;
        let mut healer_ranks: i64 = 0;
        let mut mystic_ranks: i64 = 0;
        let mut ranger_ranks: i64 = 0;
        let mut bloodmage_ranks: i64 = 0;
        let mut champion_ranks: i64 = 0;

        for t in &trainers {
            if let Some(prof) = self.trainer_db.get_profession(&t.trainer_name) {
                let total = t.ranks + t.modified_ranks;
                if total > 0 {
                    match prof {
                        "Fighter" => fighter_ranks += total,
                        "Healer" => healer_ranks += total,
                        "Mystic" => mystic_ranks += total,
                        "Ranger" => ranger_ranks += total,
                        "Bloodmage" => bloodmage_ranks += total,
                        "Champion" => champion_ranks += total,
                        _ => {}
                    }
                }
            }
        }

        // Specialization-wins logic: if any Fighter specialization has ranks,
        // pick the specialization with the most ranks (specialists also train
        // base Fighter trainers, so Fighter would always outnumber them in a
        // simple majority vote).
        if ranger_ranks > 0 || bloodmage_ranks > 0 || champion_ranks > 0 {
            // Pick highest specialization; tie-break: Ranger > Bloodmage > Champion
            if ranger_ranks >= bloodmage_ranks && ranger_ranks >= champion_ranks {
                return Ok(Profession::Ranger);
            }
            if bloodmage_ranks >= champion_ranks {
                return Ok(Profession::Bloodmage);
            }
            return Ok(Profession::Champion);
        }

        // No specialization — use base class majority vote
        let max = *[fighter_ranks, healer_ranks, mystic_ranks]
            .iter().max().unwrap_or(&0);

        if max == 0 {
            return Ok(Profession::Unknown);
        }

        // Priority: Fighter > Healer > Mystic
        if fighter_ranks == max { return Ok(Profession::Fighter); }
        if healer_ranks == max { return Ok(Profession::Healer); }
        if mystic_ranks == max { return Ok(Profession::Mystic); }

        Ok(Profession::Unknown)
    }

    /// Compute coin level based on total trainer ranks.
    /// The original app computes this from rank data — it appears to be the sum
    /// of all effective ranks divided by a factor.
    pub fn compute_coin_level(&self, char_id: i64) -> Result<i64> {
        let trainers = self.db.get_trainers(char_id)?;
        let total_ranks: i64 = trainers.iter().map(|t| t.ranks + t.modified_ranks + t.apply_learning_ranks).sum();
        // Coin level is approximately total ranks (effective + modified)
        // The original app has a more complex formula, but this is a reasonable approximation
        Ok(total_ranks)
    }

    /// Scan a log folder with a progress callback.
    /// The callback receives (current_file_index, total_files, filename).
    /// When `index_lines` is true, raw log lines are stored in the FTS5 index for search.
    pub fn scan_folder_with_progress<F>(
        &self,
        folder: &Path,
        force: bool,
        index_lines: bool,
        progress: F,
    ) -> Result<ScanResult>
    where
        F: Fn(usize, usize, &str),
    {
        let mut result = ScanResult::default();

        if !folder.is_dir() {
            return Err(crate::error::AmanuensisError::Data(format!(
                "Not a directory: {}",
                folder.display()
            )));
        }

        self.db.set_scan_pragmas()?;
        self.db.begin_transaction()?;

        let scan_result = self.scan_folder_with_progress_inner(folder, force, index_lines, &progress, &mut result);

        match scan_result {
            Ok(()) => {
                self.db.commit_transaction()?;
                self.db.reset_pragmas()?;
            }
            Err(e) => {
                let _ = self.db.rollback_transaction();
                let _ = self.db.reset_pragmas();
                return Err(e);
            }
        }

        Ok(result)
    }

    fn scan_folder_with_progress_inner<F>(
        &self,
        folder: &Path,
        force: bool,
        index_lines: bool,
        progress: &F,
        result: &mut ScanResult,
    ) -> Result<()>
    where
        F: Fn(usize, usize, &str),
    {
        // Collect all (char_dir, char_name, log_files) first to know total count
        let mut all_work: Vec<(PathBuf, String, Vec<PathBuf>)> = Vec::new();
        let mut total_files: usize = 0;

        let mut entries: Vec<_> = std::fs::read_dir(folder)?
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().map(|ft| ft.is_dir()).unwrap_or(false))
            .collect();
        entries.sort_by_key(|e| e.file_name());

        for entry in entries {
            let dir_name = entry.file_name().to_string_lossy().to_string();
            if dir_name.starts_with('.') || dir_name == "CL_Movies" {
                continue;
            }

            let char_dir = entry.path();
            let mut log_files = find_log_files(&char_dir)?;
            if log_files.is_empty() {
                continue;
            }
            log_files.sort();

            let char_name = log_files
                .iter()
                .find_map(|path| {
                    std::fs::read(path)
                        .ok()
                        .and_then(|bytes| extract_character_name(&bytes))
                })
                .unwrap_or_else(|| dir_name.clone());

            total_files += log_files.len();
            all_work.push((char_dir, char_name, log_files));
        }

        let mut current_file: usize = 0;

        for (_char_dir, char_name, log_files) in &all_work {
            log::info!("Processing character: {}", char_name);
            let char_id = self.db.get_or_create_character(char_name)?;

            for log_path in log_files {
                current_file += 1;
                let filename = log_path
                    .file_name()
                    .map(|f| f.to_string_lossy().to_string())
                    .unwrap_or_default();
                progress(current_file, total_files, &filename);

                let path_str = log_path.to_string_lossy().to_string();

                if !force && self.db.is_log_scanned(&path_str)? {
                    result.skipped += 1;
                    continue;
                }

                let bytes = match std::fs::read(log_path) {
                    Ok(b) => b,
                    Err(e) => {
                        log::warn!("Error reading {}: {}", path_str, e);
                        result.errors += 1;
                        continue;
                    }
                };

                let content_hash = hash_bytes(&bytes);
                if !force && self.db.is_hash_scanned(&content_hash)? {
                    result.skipped += 1;
                    continue;
                }

                match self.scan_bytes(&bytes, char_id, char_name, &path_str, index_lines) {
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

        Ok(())
    }

    /// Scan individual log files with a progress callback.
    /// Character name is extracted from each file's welcome message, falling back to
    /// the parent directory name.
    /// When `index_lines` is true, raw log lines are stored in the FTS5 index for search.
    pub fn scan_files_with_progress<F>(
        &self,
        files: &[PathBuf],
        force: bool,
        index_lines: bool,
        progress: F,
    ) -> Result<ScanResult>
    where
        F: Fn(usize, usize, &str),
    {
        let mut result = ScanResult::default();

        self.db.set_scan_pragmas()?;
        self.db.begin_transaction()?;

        let scan_result = self.scan_files_with_progress_inner(files, force, index_lines, &progress, &mut result);

        match scan_result {
            Ok(()) => {
                self.db.commit_transaction()?;
                self.db.reset_pragmas()?;
            }
            Err(e) => {
                let _ = self.db.rollback_transaction();
                let _ = self.db.reset_pragmas();
                return Err(e);
            }
        }

        Ok(result)
    }

    fn scan_files_with_progress_inner<F>(
        &self,
        files: &[PathBuf],
        force: bool,
        index_lines: bool,
        progress: &F,
        result: &mut ScanResult,
    ) -> Result<()>
    where
        F: Fn(usize, usize, &str),
    {
        let total_files = files.len();
        let mut seen_characters = std::collections::HashSet::new();

        for (i, log_path) in files.iter().enumerate() {
            let filename = log_path
                .file_name()
                .map(|f| f.to_string_lossy().to_string())
                .unwrap_or_default();
            progress(i + 1, total_files, &filename);

            let path_str = log_path.to_string_lossy().to_string();

            if !force && self.db.is_log_scanned(&path_str)? {
                result.skipped += 1;
                continue;
            }

            let bytes = match std::fs::read(log_path) {
                Ok(b) => b,
                Err(e) => {
                    log::warn!("Error reading {}: {}", path_str, e);
                    result.errors += 1;
                    continue;
                }
            };

            let content_hash = hash_bytes(&bytes);
            if !force && self.db.is_hash_scanned(&content_hash)? {
                result.skipped += 1;
                continue;
            }

            // Determine character name from file content or parent directory
            let char_name = extract_character_name(&bytes).unwrap_or_else(|| {
                log_path
                    .parent()
                    .and_then(|p| p.file_name())
                    .map(|n| titlecase_name(&n.to_string_lossy()))
                    .unwrap_or_else(|| "Unknown".to_string())
            });

            let char_id = self.db.get_or_create_character(&char_name)?;
            if seen_characters.insert(char_name.clone()) {
                result.characters += 1;
            }

            match self.scan_bytes(&bytes, char_id, &char_name, &path_str, index_lines) {
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

        Ok(())
    }

    /// Recursively scan for log folders under `root`, then scan each discovered folder.
    /// The callback receives (current_file_index, total_files, filename).
    /// When `index_lines` is true, raw log lines are stored in the FTS5 index for search.
    pub fn scan_recursive_with_progress<F>(
        &self,
        root: &Path,
        force: bool,
        index_lines: bool,
        progress: F,
    ) -> Result<ScanResult>
    where
        F: Fn(usize, usize, &str),
    {
        let folders = discover_log_folders(root);
        if folders.is_empty() {
            // Fall back to treating root as a direct log root
            return self.scan_folder_with_progress(root, force, index_lines, progress);
        }

        let mut combined = ScanResult::default();

        self.db.set_scan_pragmas()?;
        self.db.begin_transaction()?;

        let scan_result = (|| -> Result<()> {
            for folder in &folders {
                log::info!("Discovered log root: {}", folder.display());
                self.scan_folder_with_progress_inner(folder, force, index_lines, &progress, &mut combined)?;
            }
            Ok(())
        })();

        match scan_result {
            Ok(()) => {
                self.db.commit_transaction()?;
                self.db.reset_pragmas()?;
            }
            Err(e) => {
                let _ = self.db.rollback_transaction();
                let _ = self.db.reset_pragmas();
                return Err(e);
            }
        }

        Ok(combined)
    }

    /// After scanning, determine professions and coin levels for all characters.
    /// If a character already has a profession set from a direct announcement (circle test
    /// or "become a" message), keep it. Otherwise, fall back to majority-vote from trainers.
    pub fn finalize_characters(&self) -> Result<()> {
        let chars = self.db.list_characters()?;
        for c in &chars {
            let char_id = c.id.unwrap();
            // Only use trainer-based detection if no direct profession was set
            if c.profession == Profession::Unknown {
                let profession = self.determine_profession(char_id)?;
                if profession != Profession::Unknown {
                    self.db.update_character_profession(char_id, profession.as_str())?;
                }
            }
            let coin_level = self.compute_coin_level(char_id)?;
            if coin_level > 0 {
                self.db.update_coin_level(char_id, coin_level)?;
            }
        }
        Ok(())
    }
}

/// Check if a word is a Roman numeral (I, II, III, IV, V, VI, VII, VIII, IX, X, etc.)
fn is_roman_numeral(word: &str) -> bool {
    if word.is_empty() {
        return false;
    }
    word.chars().all(|c| matches!(c, 'I' | 'V' | 'X' | 'L' | 'C' | 'D' | 'M'))
}

/// Normalize a character name to title case (first letter of each word capitalized).
/// Preserves Roman numerals (e.g., "II", "IV", "XIV").
fn titlecase_name(name: &str) -> String {
    name.split_whitespace()
        .map(|word| {
            // Preserve Roman numerals
            if is_roman_numeral(word) {
                return word.to_string();
            }
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => {
                    let upper: String = first.to_uppercase().collect();
                    upper + &chars.as_str().to_lowercase()
                }
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Scan log file bytes to find the character name from a welcome message.
fn extract_character_name(bytes: &[u8]) -> Option<String> {
    let content = decode_log_bytes(bytes);
    for line in content.lines() {
        let message = match parse_timestamp(line) {
            Some((_dt, msg)) => msg,
            None => line,
        };
        if let Some(caps) = patterns::WELCOME_LOGIN.captures(message) {
            return Some(titlecase_name(&caps[1]));
        }
        if let Some(caps) = patterns::WELCOME_BACK.captures(message) {
            return Some(titlecase_name(&caps[1]));
        }
    }
    None
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

/// Recursively discover log root folders under `root`.
/// A "log root" is a directory that contains subdirectories with CL Log files.
/// Skips hidden directories and `CL_Movies`.
pub fn discover_log_folders(root: &Path) -> Vec<PathBuf> {
    let mut results = Vec::new();
    discover_log_folders_inner(root, &mut results);
    results
}

fn discover_log_folders_inner(dir: &Path, results: &mut Vec<PathBuf>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(rd) => rd,
        Err(_) => return,
    };

    let mut subdirs: Vec<PathBuf> = Vec::new();
    for entry in entries.filter_map(|e| e.ok()) {
        if !entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false) {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') || name == "CL_Movies" {
            continue;
        }
        subdirs.push(entry.path());
    }

    // Check if this directory is a log root: any immediate subdirectory has CL Log files
    let is_log_root = subdirs
        .iter()
        .any(|sub| find_log_files(sub).map(|f| !f.is_empty()).unwrap_or(false));

    if is_log_root {
        results.push(dir.to_path_buf());
        // Don't recurse further — children are character folders
    } else {
        // Recurse into subdirectories
        for sub in &subdirs {
            discover_log_folders_inner(sub, results);
        }
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

#[derive(Debug, Default, serde::Serialize)]
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
            char_dir.join("CL Log 2024-01-01 13.00.00.txt"),
            log_content,
        )
        .unwrap();

        let db = Database::open_in_memory().unwrap();
        let parser = LogParser::new(db).unwrap();
        let result = parser.scan_folder(tmp.path(), false).unwrap();

        assert_eq!(result.characters, 1);
        assert_eq!(result.files_scanned, 1);

        let char = parser.db().get_character("Testchar").unwrap().unwrap();
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
            char_dir.join("CL Log 2024-01-01 13.00.00.txt"),
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
            char_dir.join("CL Log 2024-01-01 13.00.00.txt"),
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
            char_dir.join("CL Log 2024-01-01 13.00.00.txt"),
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
1/1/24 1:01:00p Fen says, "hello everyone"
1/1/24 1:02:00p (Fen waves)
1/1/24 1:03:00p You slaughtered a Rat.
"#;
        fs::write(
            char_dir.join("CL Log 2024-01-01 13.00.00.txt"),
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
            char_dir.join("CL Log 2024-01-01 13.00.00.txt"),
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
            char_dir.join("CL Log 2024-01-01 13.00.00.txt"),
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
    fn test_lasty_and_pet_tracking() {
        let (tmp, char_dir) = create_test_log_dir();

        // Build log with ¥-prefixed lasty messages
        let mut bytes = Vec::new();
        // Befriend
        bytes.extend_from_slice(b"1/1/24 1:00:00p ");
        bytes.push(0xA5);
        bytes.extend_from_slice(b"You learn to befriend the Maha Ruknee.\n");
        // Another befriend message (increments count)
        bytes.extend_from_slice(b"1/1/24 1:01:00p ");
        bytes.push(0xA5);
        bytes.extend_from_slice(b"You learn to befriend the Maha Ruknee.\n");
        // Morph
        bytes.extend_from_slice(b"1/1/24 1:02:00p ");
        bytes.push(0xA5);
        bytes.extend_from_slice(b"You learn to assume the form of the Orga Anger.\n");
        // Movements
        bytes.extend_from_slice(b"1/1/24 1:03:00p ");
        bytes.push(0xA5);
        bytes.extend_from_slice(b"You learn to fight the Large Vermine more effectively.\n");
        // Completed
        bytes.extend_from_slice(b"1/1/24 1:04:00p ");
        bytes.push(0xA5);
        bytes.extend_from_slice(b"You have completed your training with Sespus.\n");

        fs::write(
            char_dir.join("CL Log 2024-01-01 13.00.00.txt"),
            &bytes,
        )
        .unwrap();

        let db = Database::open_in_memory().unwrap();
        let parser = LogParser::new(db).unwrap();
        let result = parser.scan_folder(tmp.path(), false).unwrap();
        assert_eq!(result.events_found, 5);

        let char_id = parser.db().get_or_create_character("TestChar").unwrap();

        // Check lastys
        let lastys = parser.db().get_lastys(char_id).unwrap();
        assert_eq!(lastys.len(), 3);

        let maha = lastys.iter().find(|l| l.creature_name == "Maha Ruknee").unwrap();
        assert_eq!(maha.lasty_type, "Befriend");
        assert_eq!(maha.message_count, 2);

        let orga = lastys.iter().find(|l| l.creature_name == "Orga Anger").unwrap();
        assert_eq!(orga.lasty_type, "Morph");
        assert_eq!(orga.message_count, 1);

        let vermine = lastys.iter().find(|l| l.creature_name == "Large Vermine").unwrap();
        assert_eq!(vermine.lasty_type, "Movements");

        // Befriend does NOT create pets (only healers get pets via adoption)
        let pets = parser.db().get_pets(char_id).unwrap();
        assert_eq!(pets.len(), 0);

        // One lasty should be completed (the most recent unfinished one — Large Vermine)
        let finished: Vec<_> = lastys.iter().filter(|l| l.finished).collect();
        assert_eq!(finished.len(), 1);
    }

    /// Helper: build a log file with the given ¥-prefixed rank messages (as raw bytes).
    fn build_rank_log(messages: &[&[u8]], count_each: usize) -> Vec<u8> {
        let mut bytes = Vec::new();
        for msg in messages {
            for _ in 0..count_each {
                bytes.extend_from_slice(b"1/1/24 1:00:00p ");
                bytes.push(0xA5);
                bytes.extend_from_slice(msg);
                bytes.push(b'\n');
            }
        }
        bytes
    }

    #[test]
    fn test_profession_detection_fighter() {
        let (tmp, char_dir) = create_test_log_dir();

        let bytes = build_rank_log(&[b"You seem to fight more effectively now."], 5);
        fs::write(char_dir.join("CL Log 2024-01-01 13.00.00.txt"), &bytes).unwrap();

        let db = Database::open_in_memory().unwrap();
        let parser = LogParser::new(db).unwrap();
        parser.scan_folder(tmp.path(), false).unwrap();
        parser.finalize_characters().unwrap();

        let char = parser.db().get_character("TestChar").unwrap().unwrap();
        assert_eq!(char.profession, crate::models::Profession::Fighter);
        assert!(char.coin_level > 0);
    }

    #[test]
    fn test_profession_detection_healer() {
        let (tmp, char_dir) = create_test_log_dir();

        let bytes = build_rank_log(&[b"You seem to heal more effectively."], 5);
        fs::write(char_dir.join("CL Log 2024-01-01 13.00.00.txt"), &bytes).unwrap();

        let db = Database::open_in_memory().unwrap();
        let parser = LogParser::new(db).unwrap();
        parser.scan_folder(tmp.path(), false).unwrap();
        parser.finalize_characters().unwrap();

        let char = parser.db().get_character("TestChar").unwrap().unwrap();
        assert_eq!(char.profession, crate::models::Profession::Healer);
    }

    #[test]
    fn test_profession_detection_ranger_over_fighter() {
        let (tmp, char_dir) = create_test_log_dir();

        // Ranger has many Fighter ranks + some Ranger ranks → should detect Ranger
        let bytes = build_rank_log(
            &[
                b"You seem to fight more effectively now.",  // Fighter (Evus)
                b"Your combat ability improves.",            // Ranger (Bangus Anmash)
            ],
            10, // 10 of each
        );
        fs::write(char_dir.join("CL Log 2024-01-01 13.00.00.txt"), &bytes).unwrap();

        let db = Database::open_in_memory().unwrap();
        let parser = LogParser::new(db).unwrap();
        parser.scan_folder(tmp.path(), false).unwrap();
        parser.finalize_characters().unwrap();

        let char = parser.db().get_character("TestChar").unwrap().unwrap();
        assert_eq!(char.profession, crate::models::Profession::Ranger);
    }

    #[test]
    fn test_profession_detection_champion_over_fighter() {
        let (tmp, char_dir) = create_test_log_dir();

        // Champion has Fighter ranks + Champion ranks → should detect Champion
        let bytes = build_rank_log(
            &[
                b"You seem to fight more effectively now.",  // Fighter (Evus)
                b"Your Earthpower improves.",                // Champion (Toomeria)
            ],
            5,
        );
        fs::write(char_dir.join("CL Log 2024-01-01 13.00.00.txt"), &bytes).unwrap();

        let db = Database::open_in_memory().unwrap();
        let parser = LogParser::new(db).unwrap();
        parser.scan_folder(tmp.path(), false).unwrap();
        parser.finalize_characters().unwrap();

        let char = parser.db().get_character("TestChar").unwrap().unwrap();
        assert_eq!(char.profession, crate::models::Profession::Champion);
    }

    #[test]
    fn test_profession_detection_bloodmage_over_fighter() {
        let (tmp, char_dir) = create_test_log_dir();

        // Bloodmage has Fighter ranks + Bloodmage ranks → should detect Bloodmage
        let bytes = build_rank_log(
            &[
                b"You seem to fight more effectively now.",         // Fighter (Evus)
                b"You are better able to feign death.",             // Bloodmage (Posuhm)
            ],
            5,
        );
        fs::write(char_dir.join("CL Log 2024-01-01 13.00.00.txt"), &bytes).unwrap();

        let db = Database::open_in_memory().unwrap();
        let parser = LogParser::new(db).unwrap();
        parser.scan_folder(tmp.path(), false).unwrap();
        parser.finalize_characters().unwrap();

        let char = parser.db().get_character("TestChar").unwrap().unwrap();
        assert_eq!(char.profession, crate::models::Profession::Bloodmage);
    }

    #[test]
    fn test_profession_detection_unknown_no_trainers() {
        let (tmp, char_dir) = create_test_log_dir();

        // Log with kills only, no trainer messages
        let log_content = "1/1/24 1:00:00p You slaughtered a Rat.\n";
        fs::write(char_dir.join("CL Log 2024-01-01 13.00.00.txt"), log_content).unwrap();

        let db = Database::open_in_memory().unwrap();
        let parser = LogParser::new(db).unwrap();
        parser.scan_folder(tmp.path(), false).unwrap();
        parser.finalize_characters().unwrap();

        let char = parser.db().get_character("TestChar").unwrap().unwrap();
        assert_eq!(char.profession, crate::models::Profession::Unknown);
    }

    #[test]
    fn test_loot_share_tracking() {
        let (tmp, char_dir) = create_test_log_dir();

        let log_content = "\
1/1/24 1:00:00p * Fen recovers the Dark Vermine fur, worth 20c. Your share is 10c.
1/1/24 1:01:00p * pip recovers the Orga blood, worth 30c. Your share is 15c.
";
        fs::write(
            char_dir.join("CL Log 2024-01-01 13.00.00.txt"),
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

    #[test]
    fn test_scan_skips_dirs_without_cl_logs() {
        let tmp = tempfile::tempdir().unwrap();
        // Create subdirectories with no CL Log files
        fs::create_dir(tmp.path().join("RandomFolder")).unwrap();
        fs::create_dir(tmp.path().join("AnotherDir")).unwrap();
        fs::write(tmp.path().join("RandomFolder").join("notes.txt"), "not a log").unwrap();

        let db = Database::open_in_memory().unwrap();
        let parser = LogParser::new(db).unwrap();
        let result = parser.scan_folder(tmp.path(), false).unwrap();

        assert_eq!(result.characters, 0);
        assert_eq!(result.files_scanned, 0);
        assert!(parser.db().list_characters().unwrap().is_empty());
    }

    #[test]
    fn test_scan_uses_name_from_welcome_message() {
        let tmp = tempfile::tempdir().unwrap();
        // Folder name differs from the character name in the log
        let char_dir = tmp.path().join("SomeFolder");
        fs::create_dir(&char_dir).unwrap();

        let log_content = "\
1/1/24 1:00:00p Welcome to Clan Lord, ActualName!
1/1/24 1:01:00p You slaughtered a Rat.
";
        fs::write(
            char_dir.join("CL Log 2024-01-01 13.00.00.txt"),
            log_content,
        )
        .unwrap();

        let db = Database::open_in_memory().unwrap();
        let parser = LogParser::new(db).unwrap();
        let result = parser.scan_folder(tmp.path(), false).unwrap();

        assert_eq!(result.characters, 1);
        // Character should be named from the welcome message (title-cased), not the folder
        assert!(parser.db().get_character("Actualname").unwrap().is_some());
        assert!(parser.db().get_character("SomeFolder").unwrap().is_none());
    }

    #[test]
    fn test_scan_falls_back_to_folder_name() {
        let tmp = tempfile::tempdir().unwrap();
        let char_dir = tmp.path().join("FolderName");
        fs::create_dir(&char_dir).unwrap();

        // Log with events but no welcome message
        let log_content = "\
1/1/24 1:00:00p You slaughtered a Rat.
";
        fs::write(
            char_dir.join("CL Log 2024-01-01 13.00.00.txt"),
            log_content,
        )
        .unwrap();

        let db = Database::open_in_memory().unwrap();
        let parser = LogParser::new(db).unwrap();
        let result = parser.scan_folder(tmp.path(), false).unwrap();

        assert_eq!(result.characters, 1);
        // Falls back to folder name when no welcome message found
        assert!(parser.db().get_character("FolderName").unwrap().is_some());
    }

    #[test]
    fn test_discover_log_folders() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();

        // Build: root/Clan Lords/Text Logs/CharA/CL Log ...
        let text_logs = root.join("Clan Lords").join("Text Logs");
        let char_a = text_logs.join("CharA");
        fs::create_dir_all(&char_a).unwrap();
        fs::write(
            char_a.join("CL Log 2024-01-01 13.00.00.txt"),
            "1/1/24 1:00:00p You slaughtered a Rat.\n",
        )
        .unwrap();

        // Build: root/Other/Logs/CharB/CL Log ...
        let other_logs = root.join("Other").join("Logs");
        let char_b = other_logs.join("CharB");
        fs::create_dir_all(&char_b).unwrap();
        fs::write(
            char_b.join("CL Log 2024-01-02 14.00.00.txt"),
            "1/2/24 2:00:00p You slaughtered a Vermine.\n",
        )
        .unwrap();

        // Build: root/Empty/ (no log files — should not be found)
        fs::create_dir_all(root.join("Empty").join("SubDir")).unwrap();

        let mut found = super::discover_log_folders(root);
        found.sort();

        assert_eq!(found.len(), 2);
        assert!(found.contains(&text_logs));
        assert!(found.contains(&other_logs));
    }

    #[test]
    fn test_scan_recursive_with_progress() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();

        // Two separate log roots
        let logs1 = root.join("App1").join("Text Logs");
        let char1 = logs1.join("Fen");
        fs::create_dir_all(&char1).unwrap();
        fs::write(
            char1.join("CL Log 2024-01-01 13.00.00.txt"),
            "1/1/24 1:00:00p Welcome to Clan Lord, Fen!\n1/1/24 1:01:00p You slaughtered a Rat.\n",
        )
        .unwrap();

        let logs2 = root.join("App2").join("Logs");
        let char2 = logs2.join("Pip");
        fs::create_dir_all(&char2).unwrap();
        fs::write(
            char2.join("CL Log 2024-01-02 14.00.00.txt"),
            "1/2/24 2:00:00p Welcome to Clan Lord, Pip!\n1/2/24 2:01:00p You slaughtered a Vermine.\n",
        )
        .unwrap();

        let db = Database::open_in_memory().unwrap();
        let parser = LogParser::new(db).unwrap();
        let result = parser
            .scan_recursive_with_progress(root, false, false, |_, _, _| {})
            .unwrap();

        assert_eq!(result.characters, 2);
        assert_eq!(result.files_scanned, 2);
        assert!(parser.db().get_character("Fen").unwrap().is_some());
        assert!(parser.db().get_character("Pip").unwrap().is_some());
    }

    #[test]
    fn test_extract_character_name_login() {
        let bytes = b"1/1/24 1:00:00p Welcome to Clan Lord, Fen!\n";
        assert_eq!(extract_character_name(bytes), Some("Fen".to_string()));
    }

    #[test]
    fn test_extract_character_name_welcome_back() {
        let bytes = b"1/1/24 1:00:00p Welcome back, pip!\n";
        assert_eq!(extract_character_name(bytes), Some("Pip".to_string()));
    }

    #[test]
    fn test_extract_character_name_none() {
        let bytes = b"1/1/24 1:00:00p You slaughtered a Rat.\n";
        assert_eq!(extract_character_name(bytes), None);
    }

    #[test]
    fn test_titlecase_name_single_word() {
        assert_eq!(titlecase_name("fen"), "Fen");
        assert_eq!(titlecase_name("FEN"), "Fen");
        assert_eq!(titlecase_name("Fen"), "Fen");
    }

    #[test]
    fn test_titlecase_name_multi_word() {
        assert_eq!(titlecase_name("some player"), "Some Player");
        assert_eq!(titlecase_name("SOME PLAYER"), "Some Player");
    }

    #[test]
    fn test_titlecase_name_roman_numerals() {
        assert_eq!(titlecase_name("Magnic II"), "Magnic II");
        assert_eq!(titlecase_name("magnic ii"), "Magnic Ii"); // lowercase 'ii' is not Roman numeral
        assert_eq!(titlecase_name("MAGNIC II"), "Magnic II");
        assert_eq!(titlecase_name("Character III"), "Character III");
        assert_eq!(titlecase_name("Character IV"), "Character IV");
        assert_eq!(titlecase_name("Character XIV"), "Character XIV");
    }

    #[test]
    fn test_scan_normalizes_name_from_welcome() {
        let tmp = tempfile::tempdir().unwrap();
        let char_dir = tmp.path().join("somechar");
        fs::create_dir(&char_dir).unwrap();

        let log_content = "\
1/1/24 1:00:00p Welcome to Clan Lord, somechar!
1/1/24 1:01:00p You slaughtered a Rat.
";
        fs::write(
            char_dir.join("CL Log 2024-01-01 13.00.00.txt"),
            log_content,
        )
        .unwrap();

        let db = Database::open_in_memory().unwrap();
        let parser = LogParser::new(db).unwrap();
        parser.scan_folder(tmp.path(), false).unwrap();

        // Name extracted from welcome message should be title-cased
        assert!(parser.db().get_character("Somechar").unwrap().is_some());
    }

    #[test]
    fn test_scan_finds_welcome_in_later_log_file() {
        let tmp = tempfile::tempdir().unwrap();
        let char_dir = tmp.path().join("wrongname");
        fs::create_dir(&char_dir).unwrap();

        // First log file has no welcome message
        fs::write(
            char_dir.join("CL Log 2024-01-01 13.00.00.txt"),
            "1/1/24 1:00:00p You slaughtered a Rat.\n",
        )
        .unwrap();

        // Second log file has the welcome message
        let log_content = "\
1/2/24 1:00:00p Welcome to Clan Lord, RealName!
1/2/24 1:01:00p You slaughtered a Rat.
";
        fs::write(
            char_dir.join("CL Log 2024-01-02 13.00.00.txt"),
            log_content,
        )
        .unwrap();

        let db = Database::open_in_memory().unwrap();
        let parser = LogParser::new(db).unwrap();
        parser.scan_folder(tmp.path(), false).unwrap();

        // Should find the name from the second log file, not use folder name
        assert!(parser.db().get_character("Realname").unwrap().is_some());
        assert!(parser.db().get_character("wrongname").unwrap().is_none());
    }

    #[test]
    fn test_karma_tracking() {
        let (tmp, char_dir) = create_test_log_dir();

        let log_content = "\
1/1/24 1:00:00p You just received good karma from Donk.
1/1/24 1:01:00p You just received good karma from Pip.
1/1/24 1:02:00p You just received bad karma from Troll.
";
        fs::write(
            char_dir.join("CL Log 2024-01-01 13.00.00.txt"),
            log_content,
        )
        .unwrap();

        let db = Database::open_in_memory().unwrap();
        let parser = LogParser::new(db).unwrap();
        parser.scan_folder(tmp.path(), false).unwrap();

        let char = parser.db().get_character("TestChar").unwrap().unwrap();
        assert_eq!(char.good_karma, 2);
        assert_eq!(char.bad_karma, 1);
    }

    #[test]
    fn test_esteem_tracking() {
        let (tmp, char_dir) = create_test_log_dir();

        let log_content = "\
1/1/24 1:00:00p * You gain esteem.
1/1/24 1:01:00p * You gain experience and esteem.
1/1/24 1:02:00p * You gain experience.
";
        fs::write(
            char_dir.join("CL Log 2024-01-01 13.00.00.txt"),
            log_content,
        )
        .unwrap();

        let db = Database::open_in_memory().unwrap();
        let parser = LogParser::new(db).unwrap();
        parser.scan_folder(tmp.path(), false).unwrap();

        let char = parser.db().get_character("TestChar").unwrap().unwrap();
        assert_eq!(char.esteem, 2);
    }

    #[test]
    fn test_start_date_tracking() {
        let (tmp, char_dir) = create_test_log_dir();

        let log_content = "\
1/15/24 3:00:00p Welcome to Clan Lord, TestChar!
1/16/24 1:00:00p Welcome back, TestChar!
";
        fs::write(
            char_dir.join("CL Log 2024-01-15 15.00.00.txt"),
            log_content,
        )
        .unwrap();

        let db = Database::open_in_memory().unwrap();
        let parser = LogParser::new(db).unwrap();
        parser.scan_folder(tmp.path(), false).unwrap();

        let char = parser.db().get_character("Testchar").unwrap().unwrap();
        // Should be the earlier date
        assert_eq!(char.start_date, Some("2024-01-15 15:00:00".to_string()));
    }

    #[test]
    fn test_loot_worth_tracking() {
        let (tmp, char_dir) = create_test_log_dir();

        let log_content = "\
1/1/24 1:00:00p * Fen recovers the Dark Vermine fur, worth 20c. Your share is 10c.
1/1/24 1:01:00p * pip recovers the Orga blood, worth 30c. Your share is 15c.
1/1/24 1:02:00p * You recover the Spider mandible, worth 50c. Your share is 25c.
";
        fs::write(
            char_dir.join("CL Log 2024-01-01 13.00.00.txt"),
            log_content,
        )
        .unwrap();

        let db = Database::open_in_memory().unwrap();
        let parser = LogParser::new(db).unwrap();
        parser.scan_folder(tmp.path(), false).unwrap();

        let char = parser.db().get_character("TestChar").unwrap().unwrap();
        assert_eq!(char.fur_coins, 10);
        assert_eq!(char.fur_worth, 20);
        assert_eq!(char.blood_coins, 15);
        assert_eq!(char.blood_worth, 30);
        assert_eq!(char.mandible_coins, 25);
        assert_eq!(char.mandible_worth, 50);
    }

    #[test]
    fn test_highest_kill_query() {
        let db = Database::open_in_memory().unwrap();
        let id = db.get_or_create_character("Fen").unwrap();

        // Rat: value 2, killed 10 times -> score 20
        for _ in 0..10 {
            db.upsert_kill(id, "Rat", "killed_count", 2, "2024-01-01").unwrap();
        }
        // Vermine: value 5, killed 3 times -> score 15
        for _ in 0..3 {
            db.upsert_kill(id, "Vermine", "killed_count", 5, "2024-01-01").unwrap();
        }

        let result = db.get_highest_kill(id).unwrap();
        assert!(result.is_some());
        let (name, score) = result.unwrap();
        assert_eq!(name, "Rat");
        assert_eq!(score, 20);
    }

    #[test]
    fn test_nemesis_query() {
        let db = Database::open_in_memory().unwrap();
        let id = db.get_or_create_character("Fen").unwrap();

        for _ in 0..5 {
            db.upsert_kill(id, "Orga Fury", "killed_by_count", 0, "2024-01-01").unwrap();
        }
        for _ in 0..3 {
            db.upsert_kill(id, "Large Vermine", "killed_by_count", 0, "2024-01-01").unwrap();
        }

        let result = db.get_nemesis(id).unwrap();
        assert!(result.is_some());
        let (name, count) = result.unwrap();
        assert_eq!(name, "Orga Fury");
        assert_eq!(count, 5);
    }

    #[test]
    fn test_fallen_other_character_not_counted() {
        let (tmp, char_dir) = create_test_log_dir();

        let log_content = "\
1/1/24 1:00:00p Welcome to Clan Lord, TestChar!
1/1/24 1:01:00p OtherPlayer has fallen to a Large Vermine.
1/1/24 1:02:00p AnotherPlayer has fallen to an Orga Fury.
1/1/24 1:03:00p TestChar has fallen to a Rat.
";
        fs::write(
            char_dir.join("CL Log 2024-01-01 13.00.00.txt"),
            log_content,
        )
        .unwrap();

        let db = Database::open_in_memory().unwrap();
        let parser = LogParser::new(db).unwrap();
        parser.scan_folder(tmp.path(), false).unwrap();

        let char = parser.db().get_character("Testchar").unwrap().unwrap();
        // Only TestChar's own death should count
        assert_eq!(char.deaths, 1);

        // killed_by should only have Rat (from TestChar's death)
        let kills = parser.db().get_kills(char.id.unwrap()).unwrap();
        let rat_kb = kills.iter().find(|k| k.creature_name == "Rat").unwrap();
        assert_eq!(rat_kb.killed_by_count, 1);
        // Other creatures should not appear in killed_by
        assert!(kills.iter().find(|k| k.creature_name == "Large Vermine").is_none());
        assert!(kills.iter().find(|k| k.creature_name == "Orga Fury").is_none());
    }

    #[test]
    fn test_mandible_plural_loot_share() {
        let (tmp, char_dir) = create_test_log_dir();

        let log_content = "\
1/1/24 1:00:00p * You recover the Noble Myrm mandibles, worth 2c. Your share is 1c.
1/1/24 1:01:00p * Fen recovers the Spider mandibles, worth 4c. Your share is 2c.
";
        fs::write(
            char_dir.join("CL Log 2024-01-01 13.00.00.txt"),
            log_content,
        )
        .unwrap();

        let db = Database::open_in_memory().unwrap();
        let parser = LogParser::new(db).unwrap();
        parser.scan_folder(tmp.path(), false).unwrap();

        let char = parser.db().get_character("TestChar").unwrap().unwrap();
        assert_eq!(char.mandible_coins, 3); // 1 + 2
        assert_eq!(char.mandible_worth, 6); // 2 + 4
    }

    #[test]
    fn test_profession_from_circle_test() {
        let (tmp, char_dir) = create_test_log_dir();

        let log_content = "\
1/1/24 1:00:00p Welcome to Clan Lord, TestChar!
1/1/24 1:01:00p Honor thinks, \"Congratulations go out to TestChar, who has just passed the second circle fighter test.\"
";
        fs::write(
            char_dir.join("CL Log 2024-01-01 13.00.00.txt"),
            log_content,
        )
        .unwrap();

        let db = Database::open_in_memory().unwrap();
        let parser = LogParser::new(db).unwrap();
        parser.scan_folder(tmp.path(), false).unwrap();
        parser.finalize_characters().unwrap();

        let char = parser.db().get_character("Testchar").unwrap().unwrap();
        assert_eq!(char.profession, crate::models::Profession::Fighter);
    }

    #[test]
    fn test_profession_from_become_message() {
        let (tmp, char_dir) = create_test_log_dir();

        let log_content = "\
1/1/24 1:00:00p Welcome to Clan Lord, TestChar!
1/1/24 1:01:00p Haima Myrtillus thinks, \"Congratulations to TestChar, who has just become a Bloodmage.\"
";
        fs::write(
            char_dir.join("CL Log 2024-01-01 13.00.00.txt"),
            log_content,
        )
        .unwrap();

        let db = Database::open_in_memory().unwrap();
        let parser = LogParser::new(db).unwrap();
        parser.scan_folder(tmp.path(), false).unwrap();
        parser.finalize_characters().unwrap();

        let char = parser.db().get_character("Testchar").unwrap().unwrap();
        assert_eq!(char.profession, crate::models::Profession::Bloodmage);
    }

    #[test]
    fn test_profession_announcement_other_character_ignored() {
        let (tmp, char_dir) = create_test_log_dir();

        let log_content = "\
1/1/24 1:00:00p Welcome to Clan Lord, TestChar!
1/1/24 1:01:00p Honor thinks, \"Congratulations go out to SomeoneElse, who has just passed the second circle fighter test.\"
";
        fs::write(
            char_dir.join("CL Log 2024-01-01 13.00.00.txt"),
            log_content,
        )
        .unwrap();

        let db = Database::open_in_memory().unwrap();
        let parser = LogParser::new(db).unwrap();
        parser.scan_folder(tmp.path(), false).unwrap();
        parser.finalize_characters().unwrap();

        let char = parser.db().get_character("Testchar").unwrap().unwrap();
        // Should remain Unknown since the announcement was for a different character
        assert_eq!(char.profession, crate::models::Profession::Unknown);
    }

    #[test]
    fn test_self_recovery_fur() {
        let (tmp, char_dir) = create_test_log_dir();

        let log_content = "\
1/1/24 1:00:00p * You recover the Dark Vermine fur, worth 20c.
1/1/24 1:01:00p * You recover the Orga blood, worth 10c.
";
        fs::write(
            char_dir.join("CL Log 2024-01-01 13.00.00.txt"),
            log_content,
        )
        .unwrap();

        let db = Database::open_in_memory().unwrap();
        let parser = LogParser::new(db).unwrap();
        parser.scan_folder(tmp.path(), false).unwrap();

        let char = parser.db().get_character("TestChar").unwrap().unwrap();
        // Self-recovery: amount = worth (full value to player)
        assert_eq!(char.fur_coins, 20);
        assert_eq!(char.fur_worth, 20);
        assert_eq!(char.blood_coins, 10);
        assert_eq!(char.blood_worth, 10);
    }

    #[test]
    fn test_file_without_login_counts_as_login() {
        let (tmp, char_dir) = create_test_log_dir();

        // Log with kills but no welcome message
        let log_content = "\
1/1/24 1:00:00p You slaughtered a Rat.
1/1/24 1:01:00p You slaughtered a Vermine.
";
        fs::write(
            char_dir.join("CL Log 2024-01-01 13.00.00.txt"),
            log_content,
        )
        .unwrap();

        let db = Database::open_in_memory().unwrap();
        let parser = LogParser::new(db).unwrap();
        parser.scan_folder(tmp.path(), false).unwrap();

        let char = parser.db().get_character("TestChar").unwrap().unwrap();
        // File without welcome message still counts as 1 login
        assert_eq!(char.logins, 1);
        // Start date should come from first timestamp in file
        assert_eq!(char.start_date, Some("2024-01-01 13:00:00".to_string()));
    }
}
