use rusqlite::params;

use crate::error::Result;
use crate::models::ProcessLog;
use super::Database;

impl Database {
    /// Insert a process log entry.
    pub fn add_process_log(&self, level: &str, message: &str) -> Result<()> {
        let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
        self.conn.execute(
            "INSERT INTO process_logs (created_at, level, message) VALUES (?1, ?2, ?3)",
            params![now, level, message],
        )?;
        Ok(())
    }

    /// Return all process log entries, newest first.
    pub fn get_process_logs(&self) -> Result<Vec<ProcessLog>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, created_at, level, message FROM process_logs ORDER BY id DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(ProcessLog {
                id: row.get(0)?,
                created_at: row.get(1)?,
                level: row.get(2)?,
                message: row.get(3)?,
            })
        })?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    /// Clear all process log entries (called at the start of each scan).
    pub fn clear_process_logs(&self) -> Result<()> {
        self.conn.execute("DELETE FROM process_logs", [])?;
        Ok(())
    }
}
