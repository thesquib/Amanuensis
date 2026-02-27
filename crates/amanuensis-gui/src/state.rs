use std::sync::{Arc, Mutex};

use amanuensis_core::Database;

/// Application state shared across Tauri commands.
pub struct AppState {
    pub db: Arc<Mutex<Option<Database>>>,
    pub db_path: Mutex<Option<String>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            db: Arc::new(Mutex::new(None)),
            db_path: Mutex::new(None),
        }
    }

    /// Run a closure with a reference to the open database.
    /// Returns an error if the mutex is poisoned or no database is open.
    pub fn with_db<F, R>(&self, f: F) -> Result<R, String>
    where
        F: FnOnce(&Database) -> Result<R, String>,
    {
        let guard = self
            .db
            .lock()
            .map_err(|e| format!("Database lock poisoned: {e}"))?;
        let db = guard.as_ref().ok_or("No database open")?;
        f(db)
    }

    /// Take ownership of the database out of state (for async scan operations).
    /// Returns an error if the mutex is poisoned or no database is open.
    pub fn take_db(&self) -> Result<Database, String> {
        let mut guard = self
            .db
            .lock()
            .map_err(|e| format!("Database lock poisoned: {e}"))?;
        guard.take().ok_or("No database open".to_string())
    }
}
