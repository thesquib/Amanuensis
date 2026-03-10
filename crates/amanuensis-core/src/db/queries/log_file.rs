use rusqlite::params;

use crate::error::Result;
use super::{Database, LogSearchResult};

impl Database {
    /// Check if a log file has already been scanned (by path or content hash).
    pub fn is_log_scanned(&self, file_path: &str) -> Result<bool> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM log_files WHERE file_path = ?1",
            params![file_path],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    /// Check if a content hash has already been scanned (catches duplicate files at different paths).
    pub fn is_hash_scanned(&self, content_hash: &str) -> Result<bool> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM log_files WHERE content_hash = ?1",
            params![content_hash],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    /// Mark a log file as scanned with its content hash.
    pub fn mark_log_scanned(
        &self,
        char_id: i64,
        file_path: &str,
        content_hash: &str,
        date_read: &str,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT OR IGNORE INTO log_files (character_id, file_path, content_hash, date_read)
             VALUES (?1, ?2, ?3, ?4)",
            params![char_id, file_path, content_hash, date_read],
        )?;
        Ok(())
    }

    /// Get count of scanned log files.
    pub fn scanned_log_count(&self) -> Result<i64> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM log_files",
            [],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    /// Clear all log-derived data while preserving user rank overrides.
    /// Deletes kills, lastys, pets, log_files, log_lines and resets all stat
    /// columns on characters/trainers to zero. Does NOT touch modified_ranks,
    /// rank_mode, or override_date.
    pub fn reset_log_data(&self) -> Result<()> {
        self.conn.execute_batch(
            "DELETE FROM kills;
             DELETE FROM lastys;
             DELETE FROM pets;
             DELETE FROM log_files;
             DELETE FROM log_lines;
             UPDATE characters SET
               logins=0, departs=0, deaths=0, esteem=0, coins_picked_up=0,
               casino_won=0, casino_lost=0, chest_coins=0, bounty_coins=0,
               fur_coins=0, mandible_coins=0, blood_coins=0,
               bells_used=0, bells_broken=0, chains_used=0, chains_broken=0,
               shieldstones_used=0, shieldstones_broken=0, ethereal_portals=0,
               darkstone=0, purgatory_pendant=0, coin_level=0,
               ore_found=0, tin_ore_found=0, copper_ore_found=0, gold_ore_found=0, iron_ore_found=0,
               wood_taken=0, wood_useless=0,
               good_karma=0, bad_karma=0, start_date=NULL,
               fur_worth=0, mandible_worth=0, blood_worth=0,
               eps_broken=0, untraining_count=0, profession='Unknown';
             UPDATE trainers SET
               ranks=0, apply_learning_ranks=0, apply_learning_unknown_count=0,
               date_of_last_rank=NULL;",
        )?;
        Ok(())
    }

    /// Delete all data: characters, trainers, kills, pets, lastys, log files, process logs.
    /// This is a full wipe — no data is preserved. Use reset_log_data to keep rank overrides.
    pub fn delete_all_data(&self) -> Result<()> {
        self.conn.execute_batch(
            "DELETE FROM kills;
             DELETE FROM lastys;
             DELETE FROM pets;
             DELETE FROM log_files;
             DELETE FROM log_lines;
             DELETE FROM process_logs;
             DELETE FROM trainer_checkpoints;
             DELETE FROM trainers;
             DELETE FROM characters;",
        )?;
        Ok(())
    }

    /// Clear all user-controlled rank override data, resetting trainers back to
    /// modifier mode with zero modified ranks.  Recomputes coin_level for all
    /// characters afterwards.
    pub fn clear_rank_overrides(&self) -> Result<()> {
        self.conn.execute_batch(
            "UPDATE trainers SET modified_ranks=0, rank_mode='modifier', override_date=NULL;",
        )?;
        // Recompute coin_level for every character
        let char_ids: Vec<i64> = {
            let mut stmt = self.conn.prepare("SELECT id FROM characters")?;
            let ids = stmt
                .query_map([], |row| row.get(0))?
                .filter_map(|r| r.ok())
                .collect();
            ids
        };
        for char_id in char_ids {
            let coin_level = self.compute_coin_level_from_kills(char_id)?;
            self.update_coin_level(char_id, coin_level)?;
            let interim = if coin_level == 0 { self.compute_interim_coin_level_from_kills(char_id)? } else { 0 };
            self.update_coin_level_interim(char_id, interim)?;
        }
        Ok(())
    }

    /// Batch-insert log lines into the FTS5 table.
    /// Each tuple is (character_id, content, timestamp, file_path).
    pub fn insert_log_lines(&self, lines: &[(i64, &str, &str, &str)]) -> Result<()> {
        let mut stmt = self.conn.prepare_cached(
            "INSERT INTO log_lines (content, character_id, timestamp, file_path)
             VALUES (?1, ?2, ?3, ?4)",
        )?;
        for &(char_id, content, timestamp, file_path) in lines {
            stmt.execute(params![content, char_id, timestamp, file_path])?;
        }
        Ok(())
    }

    /// Search log lines using FTS5 full-text search.
    /// Returns results with highlighted snippets.
    pub fn search_log_lines(
        &self,
        query: &str,
        char_id: Option<i64>,
        limit: i64,
    ) -> Result<Vec<LogSearchResult>> {
        // Escape double quotes in the query and wrap for literal matching
        let escaped = query.replace('"', "\"\"");
        let fts_query = format!("\"{}\"", escaped);

        let row_mapper = |row: &rusqlite::Row| -> rusqlite::Result<LogSearchResult> {
            // character_id may be stored as integer or text depending on how it was inserted
            let character_id: i64 = row.get::<_, i64>(1).or_else(|_| {
                row.get::<_, String>(1).map(|s| s.parse().unwrap_or(0))
            })?;
            Ok(LogSearchResult {
                content: row.get(0)?,
                character_id,
                timestamp: row.get(2)?,
                file_path: row.get(3)?,
                snippet: row.get(4)?,
                character_name: row.get(5)?,
            })
        };

        if let Some(cid) = char_id {
            let mut stmt = self.conn.prepare(
                "SELECT l.content, l.character_id, l.timestamp, l.file_path,
                        snippet(log_lines, 0, '<mark>', '</mark>', '...', 64) AS snippet,
                        COALESCE(c.name, 'Unknown') AS character_name
                 FROM log_lines l
                 LEFT JOIN characters c ON CAST(l.character_id AS INTEGER) = c.id
                 WHERE log_lines MATCH ?1 AND CAST(l.character_id AS INTEGER) = ?2
                 ORDER BY l.file_path DESC, l.rowid DESC
                 LIMIT ?3",
            )?;
            let mut results = Vec::new();
            for row in stmt.query_map(params![fts_query, cid, limit], row_mapper)? {
                match row {
                    Ok(r) => results.push(r),
                    Err(e) => log::warn!("FTS5 row error: {}", e),
                }
            }
            Ok(results)
        } else {
            let mut stmt = self.conn.prepare(
                "SELECT l.content, l.character_id, l.timestamp, l.file_path,
                        snippet(log_lines, 0, '<mark>', '</mark>', '...', 64) AS snippet,
                        COALESCE(c.name, 'Unknown') AS character_name
                 FROM log_lines l
                 LEFT JOIN characters c ON CAST(l.character_id AS INTEGER) = c.id
                 WHERE log_lines MATCH ?1
                 ORDER BY l.file_path DESC, l.rowid DESC
                 LIMIT ?2",
            )?;
            let mut results = Vec::new();
            for row in stmt.query_map(params![fts_query, limit], row_mapper)? {
                match row {
                    Ok(r) => results.push(r),
                    Err(e) => log::warn!("FTS5 row error: {}", e),
                }
            }
            Ok(results)
        }
    }

    /// Get the total number of indexed log lines.
    pub fn log_line_count(&self) -> Result<i64> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM log_lines",
            [],
            |row| row.get(0),
        )?;
        Ok(count)
    }
}
