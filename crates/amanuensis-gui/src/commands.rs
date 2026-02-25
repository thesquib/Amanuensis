use std::path::{Path, PathBuf};

use serde::Serialize;
use tauri::{Emitter, Manager, State};

use amanuensis_core::models::{Character, Kill, Lasty, Pet, Trainer};
use amanuensis_core::parser::ScanResult;
use amanuensis_core::{Database, LogParser, LogSearchResult, TrainerDb};

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

/// Open (or create) a database at the given path.
/// Re-finalizes characters so profession detection uses the latest algorithm.
#[tauri::command]
pub fn open_database(path: String, state: State<'_, AppState>) -> Result<(), String> {
    let db = Database::open(&path).map_err(|e| e.to_string())?;
    // Re-run profession detection so existing DBs pick up algorithm fixes
    let parser = LogParser::new(db).map_err(|e| e.to_string())?;
    parser.finalize_characters().map_err(|e| e.to_string())?;
    *state.db.lock().unwrap() = Some(parser.into_db());
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

/// Get a single character by ID with merged aggregated stats.
#[tauri::command]
pub fn get_character_merged(char_id: i64, state: State<'_, AppState>) -> Result<Option<Character>, String> {
    let guard = state.db.lock().unwrap();
    let db = guard.as_ref().ok_or("No database open")?;
    db.get_character_merged(char_id).map_err(|e| e.to_string())
}

/// Get kills for a character (includes merged sources).
#[tauri::command]
pub fn get_kills(char_id: i64, state: State<'_, AppState>) -> Result<Vec<Kill>, String> {
    let guard = state.db.lock().unwrap();
    let db = guard.as_ref().ok_or("No database open")?;
    db.get_kills_merged(char_id).map_err(|e| e.to_string())
}

/// Get trainers for a character (includes merged sources).
#[tauri::command]
pub fn get_trainers(char_id: i64, state: State<'_, AppState>) -> Result<Vec<Trainer>, String> {
    let guard = state.db.lock().unwrap();
    let db = guard.as_ref().ok_or("No database open")?;
    db.get_trainers_merged(char_id).map_err(|e| e.to_string())
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
    let guard = state.db.lock().unwrap();
    let db = guard.as_ref().ok_or("No database open")?;
    db.set_rank_override(
        char_id,
        &trainer_name,
        &rank_mode,
        modified_ranks,
        override_date.as_deref(),
    )
    .map_err(|e| e.to_string())
}

/// Get pets for a character (includes merged sources).
#[tauri::command]
pub fn get_pets(char_id: i64, state: State<'_, AppState>) -> Result<Vec<Pet>, String> {
    let guard = state.db.lock().unwrap();
    let db = guard.as_ref().ok_or("No database open")?;
    db.get_pets_merged(char_id).map_err(|e| e.to_string())
}

/// Get lastys for a character (includes merged sources).
#[tauri::command]
pub fn get_lastys(char_id: i64, state: State<'_, AppState>) -> Result<Vec<Lasty>, String> {
    let guard = state.db.lock().unwrap();
    let db = guard.as_ref().ok_or("No database open")?;
    db.get_lastys_merged(char_id).map_err(|e| e.to_string())
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

/// Scan a log folder, emitting progress events.
/// When `recursive` is true, recursively discovers log root folders under `folder`.
/// When `index_lines` is true, raw log lines are stored in the FTS5 index for search.
/// Runs on a background thread so the UI stays responsive.
#[tauri::command]
pub async fn scan_logs(
    folder: String,
    force: bool,
    recursive: bool,
    index_lines: bool,
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
                .scan_recursive_with_progress(Path::new(&folder), force, index_lines, progress_cb)
                .map_err(|e| e.to_string())?
        } else {
            parser
                .scan_folder_with_progress(Path::new(&folder), force, index_lines, progress_cb)
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

/// Scan individual log files, emitting progress events.
/// When `index_lines` is true, raw log lines are stored in the FTS5 index for search.
/// Runs on a background thread so the UI stays responsive.
#[tauri::command]
pub async fn scan_files(
    files: Vec<String>,
    force: bool,
    index_lines: bool,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<ScanResult, String> {
    let db = state
        .db
        .lock()
        .unwrap()
        .take()
        .ok_or("No database open")?;

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

        let paths: Vec<std::path::PathBuf> = files.iter().map(std::path::PathBuf::from).collect();
        let result = parser
            .scan_files_with_progress(&paths, force, index_lines, progress_cb)
            .map_err(|e| e.to_string())?;

        parser.finalize_characters().map_err(|e| e.to_string())?;
        *state_db.lock().unwrap() = Some(parser.into_db());

        Ok(result)
    })
    .await
    .map_err(|e| e.to_string())?;

    result
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
    *state.db.lock().unwrap() = Some(db);
    *state.db_path.lock().unwrap() = Some(output_path);

    Ok(result)
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
    let guard = state.db.lock().unwrap();
    let db = guard.as_ref().ok_or("No database open")?;
    let limit = limit.unwrap_or(200);
    db.search_log_lines(&query, char_id, limit)
        .map_err(|e| e.to_string())
}

/// Get the total number of indexed log lines.
#[tauri::command]
pub fn get_log_line_count(state: State<'_, AppState>) -> Result<i64, String> {
    let guard = state.db.lock().unwrap();
    let db = guard.as_ref().ok_or("No database open")?;
    db.log_line_count().map_err(|e| e.to_string())
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
    // Take the DB out of state so we can move it to the background thread
    let db = state
        .db
        .lock()
        .unwrap()
        .take()
        .ok_or("No database open")?;

    let state_db = state.db.clone();

    let result = tauri::async_runtime::spawn_blocking(move || {
        // Clear log-derived data first
        db.reset_log_data().map_err(|e| e.to_string())?;

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
                .scan_recursive_with_progress(Path::new(&folder), false, index_lines, progress_cb)
                .map_err(|e| e.to_string())?
        } else {
            parser
                .scan_folder_with_progress(Path::new(&folder), false, index_lines, progress_cb)
                .map_err(|e| e.to_string())?
        };

        parser.finalize_characters().map_err(|e| e.to_string())?;
        *state_db.lock().unwrap() = Some(parser.into_db());

        Ok(result)
    })
    .await
    .map_err(|e| e.to_string())?;

    result
}

/// Clear all rank override data: resets modified_ranks to 0, rank_mode to 'modifier',
/// and override_date to NULL for every trainer across all characters.
#[tauri::command]
pub fn clear_rank_overrides(state: State<'_, AppState>) -> Result<(), String> {
    let guard = state.db.lock().unwrap();
    let db = guard.as_ref().ok_or("No database open")?;
    db.clear_rank_overrides().map_err(|e| e.to_string())
}

/// Reset the database: clear all log-derived data while preserving rank overrides.
/// To also clear overrides, use "Clear All Overrides" in the Rank Modifiers view first.
#[tauri::command]
pub fn reset_database(state: State<'_, AppState>) -> Result<(), String> {
    let guard = state.db.lock().unwrap();
    let db = guard.as_ref().ok_or("No database open")?;
    db.reset_log_data().map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// Character merge commands
// ---------------------------------------------------------------------------

/// Merge source characters into a target character.
#[tauri::command]
pub fn merge_characters(
    source_ids: Vec<i64>,
    target_id: i64,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let guard = state.db.lock().unwrap();
    let db = guard.as_ref().ok_or("No database open")?;
    db.merge_characters(&source_ids, target_id)
        .map_err(|e| e.to_string())
}

/// Unmerge a character (restore it from a merged state).
#[tauri::command]
pub fn unmerge_character(source_id: i64, state: State<'_, AppState>) -> Result<(), String> {
    let guard = state.db.lock().unwrap();
    let db = guard.as_ref().ok_or("No database open")?;
    db.unmerge_character(source_id).map_err(|e| e.to_string())
}

/// Get all characters that have been merged into the given target.
#[tauri::command]
pub fn get_merge_sources(
    target_id: i64,
    state: State<'_, AppState>,
) -> Result<Vec<Character>, String> {
    let guard = state.db.lock().unwrap();
    let db = guard.as_ref().ok_or("No database open")?;
    db.get_merge_sources(target_id).map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// Character portrait commands (Rank Tracker mirror)
// ---------------------------------------------------------------------------

/// Sanitize a character name for use as a filename.
fn sanitize_portrait_name(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' || c == ' ' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// Directory for cached character portraits.
fn portraits_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    Ok(dir.join("portraits"))
}

/// Fetch a character portrait from Rank Tracker, cache it locally.
/// Always fetches from the server (to pick up new avatars), but returns
/// quickly if the server is unreachable and a cached copy exists.
/// Returns base64-encoded PNG data on success, or None if not found.
#[tauri::command]
pub async fn fetch_character_portrait(
    name: String,
    app: tauri::AppHandle,
) -> Result<Option<String>, String> {
    let sanitized = sanitize_portrait_name(&name);
    let dir = portraits_dir(&app)?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

    let dest = dir.join(format!("{sanitized}.png"));

    let encoded_name = urlencoding::encode(&name);
    let url = format!("https://ranktracker.squib.co.nz/avatar/{encoded_name}");

    let dest_clone = dest.clone();
    let result = tauri::async_runtime::spawn(async move {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .map_err(|e| e.to_string())?;

        let resp = match client.get(&url).send().await {
            Ok(r) => r,
            Err(_) => {
                return read_cached_as_base64(&dest_clone);
            }
        };

        if !resp.status().is_success() {
            return read_cached_as_base64(&dest_clone);
        }

        let bytes = match resp.bytes().await {
            Ok(b) => b,
            Err(_) => {
                return read_cached_as_base64(&dest_clone);
            }
        };

        // Only write if we got actual image data
        if bytes.len() > 100 {
            std::fs::write(&dest_clone, &bytes).map_err(|e| e.to_string())?;
            use base64::Engine;
            let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
            return Ok(Some(format!("data:image/png;base64,{b64}")));
        }

        read_cached_as_base64(&dest_clone)
    })
    .await
    .map_err(|e| e.to_string())?;

    result
}

/// Get the cached portrait as a base64 data URL if it exists.
#[tauri::command]
pub fn get_character_portrait_path(
    name: String,
    app: tauri::AppHandle,
) -> Result<Option<String>, String> {
    let sanitized = sanitize_portrait_name(&name);
    let dir = portraits_dir(&app)?;
    let path = dir.join(format!("{sanitized}.png"));
    read_cached_as_base64(&path)
}

/// Read a cached portrait file and return it as a base64 data URL.
fn read_cached_as_base64(path: &Path) -> Result<Option<String>, String> {
    if path.exists() {
        let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
        use base64::Engine;
        let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
        Ok(Some(format!("data:image/png;base64,{b64}")))
    } else {
        Ok(None)
    }
}

// ---------------------------------------------------------------------------
// Update check
// ---------------------------------------------------------------------------

#[derive(Serialize)]
pub struct UpdateInfo {
    pub version: String,
    pub url: String,
}

/// Check GitHub releases for a newer version.
/// Returns Some(UpdateInfo) if a newer release exists, None otherwise.
/// Silently returns None on any error (network, parse, etc.).
#[tauri::command]
pub async fn check_for_update() -> Result<Option<UpdateInfo>, String> {
    let result = tauri::async_runtime::spawn(async {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()
            .ok()?;

        let resp = client
            .get("https://api.github.com/repos/thesquib/Amanuensis/releases/latest")
            .header("User-Agent", "Amanuensis")
            .send()
            .await
            .ok()?;

        if !resp.status().is_success() {
            return None;
        }

        let json: serde_json::Value = resp.json().await.ok()?;
        let tag = json["tag_name"].as_str()?;
        let url = json["html_url"].as_str()?;

        let remote = tag.strip_prefix('v').unwrap_or(tag);
        let current = env!("CARGO_PKG_VERSION");

        if version_newer(remote, current) {
            Some(UpdateInfo {
                version: remote.to_string(),
                url: url.to_string(),
            })
        } else {
            None
        }
    })
    .await
    .map_err(|e| e.to_string())?;

    Ok(result)
}

/// Returns true if `remote` is strictly newer than `current` using numeric comparison.
fn version_newer(remote: &str, current: &str) -> bool {
    let parse = |v: &str| -> Vec<u64> {
        v.split('.').filter_map(|s| s.parse().ok()).collect()
    };
    let r = parse(remote);
    let c = parse(current);
    for i in 0..r.len().max(c.len()) {
        let rv = r.get(i).copied().unwrap_or(0);
        let cv = c.get(i).copied().unwrap_or(0);
        if rv > cv {
            return true;
        }
        if rv < cv {
            return false;
        }
    }
    false
}
