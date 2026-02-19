// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod state;

use state::AppState;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .manage(AppState::new())
        .invoke_handler(tauri::generate_handler![
            commands::open_database,
            commands::list_characters,
            commands::get_character,
            commands::get_kills,
            commands::get_trainers,
            commands::set_modified_ranks,
            commands::get_pets,
            commands::get_lastys,
            commands::get_scanned_log_count,
            commands::get_trainer_db_info,
            commands::scan_logs,
            commands::scan_files,
            commands::check_db_exists,
            commands::reset_database,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
