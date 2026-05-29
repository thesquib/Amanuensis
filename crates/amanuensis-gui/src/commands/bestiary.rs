use serde::Serialize;

use amanuensis_core::data::{BestiaryEntry, CreatureDb};

#[derive(Debug, Serialize)]
pub struct BestiaryPayload {
    pub version: String,
    pub entries: Vec<BestiaryEntry>,
}

#[tauri::command]
pub fn get_bestiary() -> Result<BestiaryPayload, String> {
    let db = CreatureDb::bundled().map_err(|e| e.to_string())?;
    let mut entries: Vec<BestiaryEntry> = db.entries().cloned().collect();
    entries.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(BestiaryPayload {
        version: db.bestiary_version().to_string(),
        entries,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_bestiary_returns_bundled_payload() {
        let payload = get_bestiary().expect("bundled bestiary should load");
        assert!(
            payload.entries.len() > 950,
            "expected > 950 entries, got {}",
            payload.entries.len()
        );
        assert_eq!(payload.version.len(), 8, "version should be YYYYMMDD");
        // sanity: a known creature should be present
        assert!(payload.entries.iter().any(|e| e.name == "Rat"));
    }
}
