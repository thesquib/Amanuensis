use std::path::PathBuf;
use tauri::State;
use tauri::Emitter;
use amanuensis_core::parser::{LogParser, ScanResult};
use crate::state::AppState;
use super::{run_scan, ScanOp, ScanProgress, SourceSpec};

/// Scan a log folder, emitting progress events.
/// When `recursive` is true, recursively discovers log root folders under `folder`.
/// When `index_lines` is true, raw log lines are stored in the FTS5 index for search.
#[tauri::command]
pub async fn scan_logs(
    folder: String,
    force: bool,
    recursive: bool,
    index_lines: bool,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<ScanResult, String> {
    run_scan(
        &state,
        app,
        ScanOp::Folder { path: folder, force, recursive },
        index_lines,
        false,
    )
    .await
}

/// Scan individual log files, emitting progress events.
#[tauri::command]
pub async fn scan_files(
    files: Vec<String>,
    force: bool,
    index_lines: bool,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<ScanResult, String> {
    run_scan(
        &state,
        app,
        ScanOp::Files { files, force },
        index_lines,
        false,
    )
    .await
}

/// Rescan logs: clear all log-derived data (preserving rank overrides) then rescan
/// every remembered source folder (each with its own recursive flag).
#[tauri::command]
pub async fn rescan_logs(
    sources: Vec<SourceSpec>,
    index_lines: bool,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<ScanResult, String> {
    let db = state.take_db()?;
    let state_db = state.db.clone();
    let folders: Vec<(std::path::PathBuf, bool)> = sources
        .into_iter()
        .map(|s| (std::path::PathBuf::from(s.path), s.recursive))
        .collect();

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
        let result = parser
            .rescan_sources(&folders, index_lines, progress_cb)
            .map_err(|e| e.to_string())?;
        *state_db.lock().map_err(|e| format!("Lock poisoned: {e}"))? = Some(parser.into_db());
        Ok(result)
    })
    .await
    .map_err(|e| e.to_string())?;

    result
}

/// Count the log files an incremental Update would touch right now (new + grown), without
/// scanning. Metadata-only. Returns 0 when no database is open is treated as an error by
/// `with_db`; callers should only invoke this once a DB is open.
#[tauri::command]
pub async fn get_pending_log_count(
    sources: Vec<SourceSpec>,
    state: State<'_, AppState>,
) -> Result<usize, String> {
    let folders: Vec<(PathBuf, bool)> = sources
        .into_iter()
        .map(|s| (PathBuf::from(s.path), s.recursive))
        .collect();
    state.with_db(|db| {
        amanuensis_core::parser::pending_files(db, &folders)
            .map(|v| v.len())
            .map_err(|e| e.to_string())
    })
}

/// Incrementally process new/grown logs across all configured sources WITHOUT resetting.
#[tauri::command]
pub async fn update_logs(
    sources: Vec<SourceSpec>,
    index_lines: bool,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<ScanResult, String> {
    let db = state.take_db()?;
    let state_db = state.db.clone();
    let folders: Vec<(PathBuf, bool)> = sources
        .into_iter()
        .map(|s| (PathBuf::from(s.path), s.recursive))
        .collect();

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
        let result = parser
            .update_sources(&folders, index_lines, progress_cb)
            .map_err(|e| e.to_string())?;
        *state_db.lock().map_err(|e| format!("Lock poisoned: {e}"))? = Some(parser.into_db());
        Ok(result)
    })
    .await
    .map_err(|e| e.to_string())?;

    result
}
