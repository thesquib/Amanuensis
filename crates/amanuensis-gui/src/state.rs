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
}
