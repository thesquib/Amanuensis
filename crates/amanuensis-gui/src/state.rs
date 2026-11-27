use std::path::Path;
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
    fn take_db(&self) -> Result<Database, String> {
        let mut guard = self
            .db
            .lock()
            .map_err(|e| format!("Database lock poisoned: {e}"))?;
        guard.take().ok_or("No database open".to_string())
    }

    /// Run `work` on the database in a blocking task (scans), and always leave a database in
    /// state afterwards: the one `work` hands back, or the file reopened if it was lost (a
    /// panic, or an error before `work` could hand it back). Without this a failed scan left
    /// every later command failing with "No database open" until the app restarted.
    pub async fn run_blocking<T, F>(&self, work: F) -> Result<T, String>
    where
        T: Send + 'static,
        F: FnOnce(Database) -> (Option<Database>, Result<T, String>) + Send + 'static,
    {
        // Path first: a failure after take_db would drop the database.
        let path = self.db_path.lock().map_err(|e| format!("Lock poisoned: {e}"))?.clone();
        let db = self.take_db()?;
        let (back, result) = match tauri::async_runtime::spawn_blocking(move || work(db)).await {
            Ok(pair) => pair,
            Err(e) => (None, Err(format!("The scan stopped unexpectedly: {e}"))),
        };
        match (result, self.restore_db(back, path.as_deref())) {
            (r, Ok(())) => r,
            (Ok(_), Err(re)) => Err(re),
            (Err(e), Err(re)) => Err(format!("{e}\n{re}")),
        }
    }

    fn restore_db(&self, back: Option<Database>, path: Option<&str>) -> Result<(), String> {
        let mut guard = self.db.lock().map_err(|e| format!("Database lock poisoned: {e}"))?;
        if guard.is_some() {
            return Ok(()); // another database was opened meanwhile: keep it
        }
        let db = match back {
            Some(db) => db,
            // Only reopen a file that exists: Database::open would create an empty one.
            None => match path.filter(|p| Path::new(p).exists()) {
                Some(p) => Database::open(p)
                    .map_err(|e| format!("Couldn't reopen the database ({e}). Please restart Amanuensis."))?,
                None => return Err("The database was closed and couldn't be reopened. Please restart Amanuensis.".into()),
            },
        };
        *guard = Some(db);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state_at(path: &str) -> AppState {
        let s = AppState::new();
        *s.db.lock().unwrap() = Some(Database::open(path).unwrap());
        *s.db_path.lock().unwrap() = Some(path.to_string());
        s
    }

    #[test]
    fn database_comes_back_after_error_and_panic() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.db");
        let s = state_at(path.to_str().unwrap());
        let rt = tauri::async_runtime::block_on(async {
            // Handed back with an error.
            let e = s.run_blocking(|db| (Some(db), Err::<(), _>("boom".to_string()))).await;
            assert_eq!(e.unwrap_err(), "boom");
            assert!(s.with_db(|_| Ok(())).is_ok());
            // Lost before it could be handed back.
            let e = s.run_blocking(|db| { drop(db); (None, Err::<(), _>("lost".to_string())) }).await;
            assert_eq!(e.unwrap_err(), "lost");
            assert!(s.with_db(|_| Ok(())).is_ok());
            // Panic.
            let e = s.run_blocking(|_db| -> (Option<Database>, Result<(), String>) { panic!("oops") }).await;
            assert!(e.unwrap_err().contains("stopped unexpectedly"));
            s.with_db(|_| Ok(()))
        });
        assert!(rt.is_ok());
    }

    #[test]
    fn missing_file_is_not_recreated() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.db");
        let s = state_at(path.to_str().unwrap());
        let p2 = path.clone();
        let e = tauri::async_runtime::block_on(s.run_blocking(move |db| {
            drop(db);
            std::fs::remove_file(&p2).unwrap();
            (None, Err::<(), _>("lost".to_string()))
        }));
        assert!(e.unwrap_err().contains("restart"));
        assert!(!path.exists());
    }
}
