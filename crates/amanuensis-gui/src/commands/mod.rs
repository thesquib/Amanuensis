mod database;
mod scanning;
mod characters;
mod data;
mod rank;
mod portraits;
mod updates;

// Re-export all commands so main.rs keeps using `commands::X` unchanged.
pub use database::*;
pub use scanning::*;
pub use characters::*;
pub use data::*;
pub use rank::*;
pub use portraits::*;
pub use updates::*;

// ---------------------------------------------------------------------------
// Shared scan infrastructure (used by scanning.rs)
// ---------------------------------------------------------------------------

use std::path::{Path, PathBuf};

use serde::Serialize;
use tauri::{Emitter, State};

use amanuensis_core::parser::ScanResult;
use amanuensis_core::{LogParser};

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
    pub multiplier: f64,
    pub is_combo: bool,
    pub combo_components: Vec<String>,
}

pub(super) enum ScanOp {
    Folder { path: String, force: bool, recursive: bool },
    Files { files: Vec<String>, force: bool },
}

pub(super) async fn run_scan(
    state: &State<'_, AppState>,
    app: tauri::AppHandle,
    op: ScanOp,
    index_lines: bool,
    reset_first: bool,
) -> Result<ScanResult, String> {
    let db = state.take_db()?;
    let state_db = state.db.clone();

    let result = tauri::async_runtime::spawn_blocking(move || {
        if reset_first {
            db.reset_log_data().map_err(|e| e.to_string())?;
        }

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

        let result = match op {
            ScanOp::Folder { path, force, recursive } => {
                if recursive {
                    parser
                        .scan_recursive_with_progress(Path::new(&path), force, index_lines, progress_cb)
                        .map_err(|e| e.to_string())?
                } else {
                    parser
                        .scan_folder_with_progress(Path::new(&path), force, index_lines, progress_cb)
                        .map_err(|e| e.to_string())?
                }
            }
            ScanOp::Files { files, force } => {
                let paths: Vec<PathBuf> = files.iter().map(PathBuf::from).collect();
                parser
                    .scan_files_with_progress(&paths, force, index_lines, progress_cb)
                    .map_err(|e| e.to_string())?
            }
        };

        parser.finalize_characters().map_err(|e| e.to_string())?;
        *state_db.lock().map_err(|e| format!("Lock poisoned: {e}"))? = Some(parser.into_db());
        Ok(result)
    })
    .await
    .map_err(|e| e.to_string())?;

    result
}
