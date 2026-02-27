use tauri::State;

use amanuensis_core::parser::ScanResult;

use crate::state::AppState;

use super::{run_scan, ScanOp};

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

/// Rescan logs: clear all log-derived data (preserving rank overrides) then rescan.
/// Equivalent to "reset + scan" but keeps user-entered modifier/override settings.
#[tauri::command]
pub async fn rescan_logs(
    folder: String,
    recursive: bool,
    index_lines: bool,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<ScanResult, String> {
    run_scan(
        &state,
        app,
        ScanOp::Folder { path: folder, force: false, recursive },
        index_lines,
        true,
    )
    .await
}
