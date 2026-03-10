use std::path::Path;

use tauri::{Manager, State};

use amanuensis_core::{Database, LogParser};

use crate::state::AppState;

/// Open (or create) a database at the given path.
/// Re-finalizes characters so profession detection uses the latest algorithm.
#[tauri::command]
pub fn open_database(path: String, state: State<'_, AppState>) -> Result<(), String> {
    let db = Database::open(&path).map_err(|e| e.to_string())?;
    // Re-run profession detection so existing DBs pick up algorithm fixes
    let parser = LogParser::new(db).map_err(|e| e.to_string())?;
    parser.finalize_characters().map_err(|e| e.to_string())?;
    *state.db.lock().map_err(|e| format!("Lock poisoned: {e}"))? = Some(parser.into_db());
    *state.db_path.lock().map_err(|e| format!("Lock poisoned: {e}"))? = Some(path);
    Ok(())
}

/// Get the default database path in the app's data directory.
#[tauri::command]
pub fn get_default_db_path(app: tauri::AppHandle) -> Result<String, String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.join("amanuensis.db").to_string_lossy().into_owned())
}

/// Check if a database file exists at a path (for auto-detection).
#[tauri::command]
pub fn check_db_exists(path: String) -> bool {
    Path::new(&path).exists()
}

/// Reset the database: clear all log-derived data while preserving rank overrides.
#[tauri::command]
pub fn reset_database(state: State<'_, AppState>) -> Result<(), String> {
    state.with_db(|db| db.reset_log_data().map_err(|e| e.to_string()))
}

/// Delete all data: completely wipes characters, trainers, kills, pets, lastys, and all logs.
/// No data is preserved. The database file remains open and ready for a fresh scan.
#[tauri::command]
pub fn delete_all_data(state: State<'_, AppState>) -> Result<(), String> {
    state.with_db(|db| db.delete_all_data().map_err(|e| e.to_string()))
}

/// Reveal the database file in the OS file manager (Finder/Explorer/Nautilus).
#[tauri::command]
pub fn reveal_database(path: String) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .args(["-R", &path])
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg(format!("/select,{path}"))
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "linux")]
    {
        let parent = std::path::Path::new(&path)
            .parent()
            .unwrap_or(std::path::Path::new("/"))
            .to_string_lossy()
            .into_owned();
        std::process::Command::new("xdg-open")
            .arg(&parent)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Import data from a Scribius (Core Data) database into a new Amanuensis database.
/// After import, the new database is opened in the app state.
#[tauri::command]
pub fn import_scribius_db(
    scribius_path: String,
    output_path: String,
    force: bool,
    state: State<'_, AppState>,
) -> Result<amanuensis_core::ImportResult, String> {
    let result = amanuensis_core::import_scribius(
        Path::new(&scribius_path),
        &output_path,
        force,
    )
    .map_err(|e| e.to_string())?;

    // Open the newly created database in app state
    let db = Database::open(&output_path).map_err(|e| e.to_string())?;
    *state.db.lock().map_err(|e| format!("Lock poisoned: {e}"))? = Some(db);
    *state.db_path.lock().map_err(|e| format!("Lock poisoned: {e}"))? = Some(output_path);

    Ok(result)
}
