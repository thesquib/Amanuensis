use std::path::Path;

use serde::Serialize;
use tauri::{Emitter, State};

use scribius_core::models::{Character, Kill, Lasty, Pet, Trainer};
use scribius_core::parser::ScanResult;
use scribius_core::{Database, LogParser, TrainerDb};

use crate::state::AppState;

#[derive(Clone, Serialize)]
pub struct ScanProgress {
    pub current_file: usize,
    pub total_files: usize,
    pub filename: String,
}

#[derive(Serialize)]
pub struct TrainerInfo {
    pub name: String,
    pub profession: Option<String>,
}

/// Open (or create) a database at the given path.
#[tauri::command]
pub fn open_database(path: String, state: State<'_, AppState>) -> Result<(), String> {
    let db = Database::open(&path).map_err(|e| e.to_string())?;
    *state.db.lock().unwrap() = Some(db);
    *state.db_path.lock().unwrap() = Some(path);
    Ok(())
}

/// List all characters in the database.
#[tauri::command]
pub fn list_characters(state: State<'_, AppState>) -> Result<Vec<Character>, String> {
    let guard = state.db.lock().unwrap();
    let db = guard.as_ref().ok_or("No database open")?;
    db.list_characters().map_err(|e| e.to_string())
}

/// Get a single character by name.
#[tauri::command]
pub fn get_character(name: String, state: State<'_, AppState>) -> Result<Option<Character>, String> {
    let guard = state.db.lock().unwrap();
    let db = guard.as_ref().ok_or("No database open")?;
    db.get_character(&name).map_err(|e| e.to_string())
}

/// Get kills for a character.
#[tauri::command]
pub fn get_kills(char_id: i64, state: State<'_, AppState>) -> Result<Vec<Kill>, String> {
    let guard = state.db.lock().unwrap();
    let db = guard.as_ref().ok_or("No database open")?;
    db.get_kills(char_id).map_err(|e| e.to_string())
}

/// Get trainers for a character.
#[tauri::command]
pub fn get_trainers(char_id: i64, state: State<'_, AppState>) -> Result<Vec<Trainer>, String> {
    let guard = state.db.lock().unwrap();
    let db = guard.as_ref().ok_or("No database open")?;
    db.get_trainers(char_id).map_err(|e| e.to_string())
}

/// Set modified ranks for a trainer (user-specified baseline).
#[tauri::command]
pub fn set_modified_ranks(
    char_id: i64,
    trainer_name: String,
    modified_ranks: i64,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let guard = state.db.lock().unwrap();
    let db = guard.as_ref().ok_or("No database open")?;
    db.set_modified_ranks(char_id, &trainer_name, modified_ranks)
        .map_err(|e| e.to_string())
}

/// Get pets for a character.
#[tauri::command]
pub fn get_pets(char_id: i64, state: State<'_, AppState>) -> Result<Vec<Pet>, String> {
    let guard = state.db.lock().unwrap();
    let db = guard.as_ref().ok_or("No database open")?;
    db.get_pets(char_id).map_err(|e| e.to_string())
}

/// Get lastys for a character.
#[tauri::command]
pub fn get_lastys(char_id: i64, state: State<'_, AppState>) -> Result<Vec<Lasty>, String> {
    let guard = state.db.lock().unwrap();
    let db = guard.as_ref().ok_or("No database open")?;
    db.get_lastys(char_id).map_err(|e| e.to_string())
}

/// Get total scanned log file count.
#[tauri::command]
pub fn get_scanned_log_count(state: State<'_, AppState>) -> Result<i64, String> {
    let guard = state.db.lock().unwrap();
    let db = guard.as_ref().ok_or("No database open")?;
    db.scanned_log_count().map_err(|e| e.to_string())
}

/// Get the full trainer catalog (for "Show Zero Trainers" toggle).
#[tauri::command]
pub fn get_trainer_db_info() -> Result<Vec<TrainerInfo>, String> {
    let trainer_db = TrainerDb::bundled().map_err(|e| e.to_string())?;
    Ok(trainer_db
        .all_trainers_with_professions()
        .into_iter()
        .map(|(name, profession)| TrainerInfo { name, profession })
        .collect())
}

/// Scan a log folder, emitting progress events.
/// When `recursive` is true, recursively discovers log root folders under `folder`.
/// Runs on a background thread so the UI stays responsive.
#[tauri::command]
pub async fn scan_logs(
    folder: String,
    force: bool,
    recursive: bool,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<ScanResult, String> {
    // Take the DB out of state so we can move it to the background thread
    let db = state
        .db
        .lock()
        .unwrap()
        .take()
        .ok_or("No database open")?;

    // Clone the Arc'd state so we can restore the DB after the scan
    let state_db = state.db.clone();

    let result = tauri::async_runtime::spawn_blocking(move || {
        let parser = LogParser::new(db).map_err(|e| e.to_string())?;

        let app_handle = app.clone();
        let progress_cb = |current: usize, total: usize, filename: &str| {
            let _ = app_handle.emit(
                "scan-progress",
                ScanProgress {
                    current_file: current,
                    total_files: total,
                    filename: filename.to_string(),
                },
            );
        };

        let result = if recursive {
            parser
                .scan_recursive_with_progress(Path::new(&folder), force, progress_cb)
                .map_err(|e| e.to_string())?
        } else {
            parser
                .scan_folder_with_progress(Path::new(&folder), force, progress_cb)
                .map_err(|e| e.to_string())?
        };

        // Finalize characters (profession detection, coin levels)
        parser.finalize_characters().map_err(|e| e.to_string())?;

        // Put the DB back into state
        *state_db.lock().unwrap() = Some(parser.into_db());

        Ok(result)
    })
    .await
    .map_err(|e| e.to_string())?;

    result
}

/// Check if a database file exists at a path (for auto-detection).
#[tauri::command]
pub fn check_db_exists(path: String) -> bool {
    Path::new(&path).exists()
}

/// Reset the database: delete the file and reopen a fresh one.
#[tauri::command]
pub fn reset_database(state: State<'_, AppState>) -> Result<(), String> {
    let path = state
        .db_path
        .lock()
        .unwrap()
        .clone()
        .ok_or("No database open")?;

    // Close the existing DB
    *state.db.lock().unwrap() = None;

    // Delete the file
    if Path::new(&path).exists() {
        std::fs::remove_file(&path).map_err(|e| e.to_string())?;
    }

    // Reopen fresh
    let db = Database::open(&path).map_err(|e| e.to_string())?;
    *state.db.lock().unwrap() = Some(db);

    Ok(())
}
