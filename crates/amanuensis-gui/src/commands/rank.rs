use std::path::{Path, PathBuf};

use tauri::{Manager, State};

use amanuensis_core::{read_overrides_file, write_overrides_file, ApplyOverridesResult, Database, RankOverride};

use crate::state::AppState;

/// The GUI's rank override backup. It lives in the app data folder rather than next to
/// a database so it outlives any one database (new file from an import, Delete All Data).
pub(crate) fn overrides_backup_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    Ok(app.path().app_data_dir().map_err(|e| e.to_string())?.join("rank-overrides-backup.json"))
}

/// Mirror an edit into the backup. A failed backup must not report the save itself
/// as failed (the database write already happened), so it is logged instead.
fn sync_backup(app: &tauri::AppHandle, db: &Database, char_ids: &[i64], trainers: &[&str]) {
    let r = overrides_backup_path(app)
        .and_then(|p| db.sync_rank_override_backup(&p, char_ids, trainers).map_err(|e| e.to_string()));
    if let Err(e) = r {
        eprintln!("rank override backup failed: {e}");
    }
}

/// Set modified ranks for a trainer (user-specified baseline).
#[tauri::command]
pub fn set_modified_ranks(
    char_id: i64,
    trainer_name: String,
    modified_ranks: i64,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state.with_db(|db| {
        db.set_modified_ranks(char_id, &trainer_name, modified_ranks)
            .map_err(|e| e.to_string())?;
        sync_backup(&app, db, &[char_id], &[&trainer_name]);
        Ok(())
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
    app: tauri::AppHandle,
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
        .map_err(|e| e.to_string())?;
        sync_backup(&app, db, &[char_id], &[&trainer_name]);
        Ok(())
    })
}

/// Clear rank overrides for one character (and its merge sources). The backup is left
/// alone on purpose, so the cleared values can be restored until the user discards them.
#[tauri::command]
pub fn clear_rank_overrides(char_id: i64, state: State<'_, AppState>) -> Result<(), String> {
    state.with_db(|db| db.clear_rank_overrides(Some(char_id)).map_err(|e| e.to_string()))
}

/// Move a trainer's override to another trainer name (e.g. one renamed in trainers.json).
#[tauri::command]
pub fn move_rank_override(
    char_id: i64,
    from: String,
    to: String,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state.with_db(|db| {
        db.move_rank_override(char_id, &from, &to).map_err(|e| e.to_string())?;
        let ids = db.char_ids_for_merged(char_id).map_err(|e| e.to_string())?;
        sync_backup(&app, db, &ids, &[&from, &to]);
        Ok(())
    })
}

/// Write every rank override in the database to a file. Returns how many were written.
#[tauri::command]
pub fn export_rank_overrides(path: String, state: State<'_, AppState>) -> Result<usize, String> {
    state.with_db(|db| {
        let all = db.rank_overrides(None).map_err(|e| e.to_string())?;
        write_overrides_file(Path::new(&path), &all).map_err(|e| e.to_string())?;
        Ok(all.len())
    })
}

/// Apply an exported overrides file, matching characters by name.
#[tauri::command]
pub fn import_rank_overrides(
    path: String,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<ApplyOverridesResult, String> {
    state.with_db(|db| {
        let entries = read_overrides_file(Path::new(&path)).map_err(|e| e.to_string())?;
        let result = db.apply_rank_overrides(None, &entries).map_err(|e| e.to_string())?;
        backup_all(&app, db);
        Ok(result)
    })
}

/// Backed-up overrides for this character that the database no longer has.
#[tauri::command]
pub fn get_missing_rank_overrides(
    char_id: i64,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<Vec<RankOverride>, String> {
    let backup = overrides_backup_path(&app)?;
    state.with_db(|db| db.missing_rank_overrides(&backup, char_id).map_err(|e| e.to_string()))
}

/// Put the missing backed-up overrides back on this character.
#[tauri::command]
pub fn restore_missing_rank_overrides(
    char_id: i64,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<ApplyOverridesResult, String> {
    let backup = overrides_backup_path(&app)?;
    state.with_db(|db| db.restore_missing_rank_overrides(&backup, char_id).map_err(|e| e.to_string()))
}

/// Forget this character's missing backed-up overrides.
#[tauri::command]
pub fn discard_missing_rank_overrides(
    char_id: i64,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let backup = overrides_backup_path(&app)?;
    state.with_db(|db| db.discard_missing_rank_overrides(&backup, char_id).map_err(|e| e.to_string()))
}

/// Add everything in the open database to the backup (logged, never fatal).
pub(crate) fn backup_all(app: &tauri::AppHandle, db: &Database) {
    let r = overrides_backup_path(app)
        .and_then(|p| db.backup_all_rank_overrides(&p).map_err(|e| e.to_string()));
    if let Err(e) = r {
        eprintln!("rank override backup failed: {e}");
    }
}

/// Set (or clear) a manual profession override for a character.
/// Pass `null` to clear the override; the profession will be recalculated on the next scan.
/// Pass a profession string to immediately set both the override and the displayed profession.
#[tauri::command]
pub fn set_profession_override(
    char_id: i64,
    profession: Option<String>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state.with_db(|db| {
        db.set_profession_override(char_id, profession.as_deref())
            .map_err(|e| e.to_string())
    })
}

/// Get all trainer rank checkpoints for a character, sorted by timestamp.
/// Used for the checkpoint progression timeline graph.
/// `include_players` reveals checkpoints from player-run ("ledger") trainers.
#[tauri::command]
pub fn get_all_trainer_checkpoints(
    char_id: i64,
    include_players: bool,
    state: State<'_, AppState>,
) -> Result<Vec<amanuensis_core::models::TrainerCheckpoint>, String> {
    state.with_db(|db| {
        db.get_all_trainer_checkpoints(char_id, include_players)
            .map_err(|e| e.to_string())
    })
}

/// Get the most recent trainer rank checkpoint for each trainer for a character.
/// `include_players` reveals checkpoints from player-run ("ledger") trainers.
#[tauri::command]
pub fn get_trainer_checkpoints(
    char_id: i64,
    include_players: bool,
    state: State<'_, AppState>,
) -> Result<Vec<amanuensis_core::models::TrainerCheckpoint>, String> {
    state.with_db(|db| {
        db.get_latest_trainer_checkpoints(char_id, include_players)
            .map_err(|e| e.to_string())
    })
}
