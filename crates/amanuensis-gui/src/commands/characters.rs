use tauri::State;

use amanuensis_core::models::Character;

use crate::state::AppState;

/// List all characters in the database.
#[tauri::command]
pub fn list_characters(state: State<'_, AppState>) -> Result<Vec<Character>, String> {
    state.with_db(|db| db.list_characters().map_err(|e| e.to_string()))
}

/// Get a single character by name.
#[tauri::command]
pub fn get_character(name: String, state: State<'_, AppState>) -> Result<Option<Character>, String> {
    state.with_db(|db| db.get_character(&name).map_err(|e| e.to_string()))
}

/// Get a single character by ID with merged aggregated stats.
#[tauri::command]
pub fn get_character_merged(char_id: i64, state: State<'_, AppState>) -> Result<Option<Character>, String> {
    state.with_db(|db| db.get_character_merged(char_id).map_err(|e| e.to_string()))
}

/// Merge source characters into a target character.
#[tauri::command]
pub fn merge_characters(
    source_ids: Vec<i64>,
    target_id: i64,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state.with_db(|db| db.merge_characters(&source_ids, target_id).map_err(|e| e.to_string()))
}

/// Unmerge a character (restore it from a merged state).
#[tauri::command]
pub fn unmerge_character(source_id: i64, state: State<'_, AppState>) -> Result<(), String> {
    state.with_db(|db| db.unmerge_character(source_id).map_err(|e| e.to_string()))
}

/// Get all characters that have been merged into the given target.
#[tauri::command]
pub fn get_merge_sources(target_id: i64, state: State<'_, AppState>) -> Result<Vec<Character>, String> {
    state.with_db(|db| db.get_merge_sources(target_id).map_err(|e| e.to_string()))
}
