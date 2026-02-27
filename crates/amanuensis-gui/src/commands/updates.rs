use serde::Serialize;

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
