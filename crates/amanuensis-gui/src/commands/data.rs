use tauri::State;

use amanuensis_core::models::{Kill, Lasty, Pet, Trainer};
use amanuensis_core::{LogSearchResult, TrainerDb};

use crate::state::AppState;

use super::TrainerInfo;

/// Get kills for a character (includes merged sources).
#[tauri::command]
pub fn get_kills(char_id: i64, state: State<'_, AppState>) -> Result<Vec<Kill>, String> {
    state.with_db(|db| db.get_kills_merged(char_id).map_err(|e| e.to_string()))
}

/// Get trainers for a character (includes merged sources).
#[tauri::command]
pub fn get_trainers(char_id: i64, state: State<'_, AppState>) -> Result<Vec<Trainer>, String> {
    state.with_db(|db| db.get_trainers_merged(char_id).map_err(|e| e.to_string()))
}

/// Get pets for a character (includes merged sources).
#[tauri::command]
pub fn get_pets(char_id: i64, state: State<'_, AppState>) -> Result<Vec<Pet>, String> {
    state.with_db(|db| db.get_pets_merged(char_id).map_err(|e| e.to_string()))
}

/// Get lastys for a character (includes merged sources).
#[tauri::command]
pub fn get_lastys(char_id: i64, state: State<'_, AppState>) -> Result<Vec<Lasty>, String> {
    state.with_db(|db| db.get_lastys_merged(char_id).map_err(|e| e.to_string()))
}

/// Get total scanned log file count.
#[tauri::command]
pub fn get_scanned_log_count(state: State<'_, AppState>) -> Result<i64, String> {
    state.with_db(|db| db.scanned_log_count().map_err(|e| e.to_string()))
}

/// Get the total number of indexed log lines.
#[tauri::command]
pub fn get_log_line_count(state: State<'_, AppState>) -> Result<i64, String> {
    state.with_db(|db| db.log_line_count().map_err(|e| e.to_string()))
}

/// Get the full trainer catalog (for "Show Zero Trainers" toggle).
#[tauri::command]
pub fn get_trainer_db_info() -> Result<Vec<TrainerInfo>, String> {
    let trainer_db = TrainerDb::bundled().map_err(|e| e.to_string())?;
    Ok(trainer_db
        .all_trainer_metadata()
        .into_iter()
        .map(|m| TrainerInfo {
            name: m.name,
            profession: m.profession,
            multiplier: m.multiplier,
            is_combo: m.is_combo,
            combo_components: m.combo_components,
        })
        .collect())
}

/// Search indexed log lines using FTS5 full-text search.
#[tauri::command]
pub fn search_logs(
    query: String,
    char_id: Option<i64>,
    limit: Option<i64>,
    state: State<'_, AppState>,
) -> Result<Vec<LogSearchResult>, String> {
    if query.trim().is_empty() {
        return Ok(Vec::new());
    }
    let limit = limit.unwrap_or(200);
    state.with_db(|db| db.search_log_lines(&query, char_id, limit).map_err(|e| e.to_string()))
}
