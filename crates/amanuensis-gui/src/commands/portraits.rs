use std::path::{Path, PathBuf};

use tauri::Manager;

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
