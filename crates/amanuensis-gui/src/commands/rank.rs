use tauri::State;

use crate::state::AppState;

/// Set modified ranks for a trainer (user-specified baseline).
#[tauri::command]
pub fn set_modified_ranks(
    char_id: i64,
    trainer_name: String,
    modified_ranks: i64,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state.with_db(|db| {
        db.set_modified_ranks(char_id, &trainer_name, modified_ranks)
            .map_err(|e| e.to_string())
    })
}

/// Set rank override mode for a trainer.
#[tauri::command]
pub fn set_rank_override(
    char_id: i64,
    trainer_name: String,
    rank_mode: String,
    modified_ranks: i64,
    override_date: Option<String>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state.with_db(|db| {
        db.set_rank_override(
            char_id,
            &trainer_name,
            &rank_mode,
            modified_ranks,
            override_date.as_deref(),
        )
        .map_err(|e| e.to_string())
    })
}

/// Clear all rank override data: resets modified_ranks to 0, rank_mode to 'modifier',
/// and override_date to NULL for every trainer across all characters.
#[tauri::command]
pub fn clear_rank_overrides(state: State<'_, AppState>) -> Result<(), String> {
    state.with_db(|db| db.clear_rank_overrides().map_err(|e| e.to_string()))
}
