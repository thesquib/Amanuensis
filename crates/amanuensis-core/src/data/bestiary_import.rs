use std::collections::HashMap;

use quick_xml::events::Event;
use quick_xml::reader::Reader;

use crate::data::bestiary::BestiaryEntry;
use crate::error::{AmanuensisError, Result};

/// Parse a phpMyAdmin XML dump (clnet_bestiary.creatures) into BestiaryEntry rows.
pub fn parse_bestiary_xml(xml: &[u8]) -> Result<Vec<BestiaryEntry>> {
    let mut reader = Reader::from_reader(xml);
    reader.trim_text(true);

    let mut buf = Vec::new();
    let mut entries: Vec<BestiaryEntry> = Vec::new();
    let mut current: Option<HashMap<String, String>> = None;
    let mut current_column: Option<String> = None;
    let mut current_text = String::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => match e.name().as_ref() {
                b"table" => {
                    if attr_matches(&e, b"name", b"creatures") {
                        current = Some(HashMap::new());
                    }
                }
                b"column" if current.is_some() => {
                    if let Some(name) = attr_value(&e, b"name") {
                        current_column = Some(name);
                        current_text.clear();
                    }
                }
                _ => {}
            },
            Ok(Event::Text(t)) => {
                if current_column.is_some() {
                    let txt = t.unescape().map_err(xml_err)?;
                    current_text.push_str(&txt);
                }
            }
            Ok(Event::End(e)) => match e.name().as_ref() {
                b"column" => {
                    if let (Some(map), Some(col)) = (current.as_mut(), current_column.take()) {
                        map.insert(col, std::mem::take(&mut current_text));
                    }
                }
                b"table" => {
                    if let Some(map) = current.take() {
                        entries.push(entry_from_columns(map)?);
                    }
                }
                _ => {}
            },
            Ok(Event::Eof) => break,
            Err(e) => return Err(xml_err(e)),
            _ => {}
        }
        buf.clear();
    }

    Ok(entries)
}

fn xml_err<E: std::fmt::Display>(e: E) -> AmanuensisError {
    AmanuensisError::Parse(format!("bestiary XML: {}", e))
}

fn attr_matches(e: &quick_xml::events::BytesStart, key: &[u8], value: &[u8]) -> bool {
    e.attributes().flatten().any(|a| a.key.as_ref() == key && a.value.as_ref() == value)
}

fn attr_value(e: &quick_xml::events::BytesStart, key: &[u8]) -> Option<String> {
    e.attributes().flatten().find(|a| a.key.as_ref() == key).map(|a| {
        String::from_utf8_lossy(&a.value).into_owned()
    })
}

fn entry_from_columns(cols: HashMap<String, String>) -> Result<BestiaryEntry> {
    let name = cols.get("name").cloned().ok_or_else(|| {
        AmanuensisError::Parse("bestiary entry missing 'name' column".to_string())
    })?;

    Ok(BestiaryEntry {
        name,
        family: opt_str(&cols, "family"),
        location: opt_str(&cols, "location"),
        information: opt_str(&cols, "information"),
        exp_taxidermy: opt_int(&cols, "exp_taxidermy").unwrap_or(0),
        rarity: opt_str(&cols, "rarity"),
        worth: opt_int(&cols, "worth"),
        worth_range: opt_str(&cols, "worth_range"),
        frames_per_swing: opt_float(&cols, "framesperswing"),
        difficulty: opt_str(&cols, "difficulty"),
        attack: opt_int(&cols, "attack"),
        defense: opt_int(&cols, "defense"),
        damage: opt_int(&cols, "damage"),
        health: opt_int(&cols, "health"),
        attack_measured: opt_bool(&cols, "attack_ismeasured"),
        defense_measured: opt_bool(&cols, "defense_ismeasured"),
        damage_measured: opt_bool(&cols, "damage_ismeasured"),
        health_measured: opt_bool(&cols, "health_ismeasured"),
        luck_hits: opt_int(&cols, "luck_hits"),
        is_seasonal: opt_bool(&cols, "is_seasonal"),
        first_update: opt_str(&cols, "first_update"),
        last_update: opt_str(&cols, "last_update"),
        static_pic: opt_str(&cols, "static_pic"),
        static_width: opt_int(&cols, "static_width"),
        static_height: opt_int(&cols, "static_height"),
        action_pic: opt_str(&cols, "action_pic"),
        action_width: opt_int(&cols, "action_width"),
        action_height: opt_int(&cols, "action_height"),
    })
}

fn opt_str(cols: &HashMap<String, String>, key: &str) -> Option<String> {
    match cols.get(key) {
        Some(v) if v != "NULL" && !v.is_empty() => Some(v.clone()),
        _ => None,
    }
}

fn opt_int(cols: &HashMap<String, String>, key: &str) -> Option<i32> {
    cols.get(key).and_then(|v| if v == "NULL" { None } else { v.parse().ok() })
}

fn opt_float(cols: &HashMap<String, String>, key: &str) -> Option<f64> {
    cols.get(key).and_then(|v| if v == "NULL" { None } else { v.parse().ok() })
}

fn opt_bool(cols: &HashMap<String, String>, key: &str) -> bool {
    matches!(cols.get(key).map(String::as_str), Some("1"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINIMAL_FIXTURE: &[u8] = include_bytes!("../../tests/fixtures/bestiary_minimal.xml");

    #[test]
    fn parses_two_entries() {
        let entries = parse_bestiary_xml(MINIMAL_FIXTURE).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].name, "Rat");
        assert_eq!(entries[1].name, "Venomous Leech");
    }

    #[test]
    fn populates_rat_fields() {
        let entries = parse_bestiary_xml(MINIMAL_FIXTURE).unwrap();
        let rat = &entries[0];
        assert_eq!(rat.exp_taxidermy, 2);
        assert_eq!(rat.family.as_deref(), Some("Vermine"));
        assert_eq!(rat.rarity.as_deref(), Some("Common"));
        assert_eq!(rat.attack, Some(65));
        assert_eq!(rat.health, Some(2));
        assert!(rat.attack_measured);
        assert!(rat.health_measured);
        assert!(!rat.is_seasonal);
    }

    #[test]
    fn null_string_becomes_none() {
        let entries = parse_bestiary_xml(MINIMAL_FIXTURE).unwrap();
        let leech = &entries[1];
        assert_eq!(leech.first_update, None);          // first_update was "NULL"
        assert_eq!(leech.luck_hits, None);              // luck_hits was "NULL"
        assert_eq!(leech.action_pic, None);             // action_pic was "NULL"
        assert_eq!(leech.action_width, None);
    }

    #[test]
    fn ismeasured_null_becomes_false() {
        let entries = parse_bestiary_xml(MINIMAL_FIXTURE).unwrap();
        let leech = &entries[1];
        assert!(!leech.attack_measured);                // "NULL" → false
        assert!(!leech.defense_measured);
        assert!(!leech.damage_measured);
        assert!(!leech.health_measured);
    }

    #[test]
    fn html_entities_decoded() {
        let entries = parse_bestiary_xml(MINIMAL_FIXTURE).unwrap();
        let leech = &entries[1];
        // location is "Dal&#039;Nzoth Waters" in the fixture; should decode to "Dal'Nzoth Waters"
        assert_eq!(leech.location.as_deref(), Some("Dal'Nzoth Waters"));
    }
}

#[cfg(test)]
mod regen {
    use super::*;
    use crate::data::bestiary::BestiaryFile;
    use std::fs;
    use std::path::PathBuf;

    /// One-off generator: read the upstream XML dump, write bestiary.json.
    /// Run with: cargo test -p amanuensis-core regen::generate_bestiary_json -- --ignored --nocapture
    /// Env: BESTIARY_XML=/path/to/bestiary_YYYYMMDD_fullexport.xml
    #[test]
    #[ignore]
    fn generate_bestiary_json() {
        let xml_path = std::env::var("BESTIARY_XML")
            .expect("set BESTIARY_XML=/path/to/bestiary_YYYYMMDD_fullexport.xml");
        let xml = fs::read(&xml_path).expect("read XML");
        let mut entries = parse_bestiary_xml(&xml).expect("parse XML");
        entries.sort_by(|a, b| a.name.cmp(&b.name));

        let version = version_from_filename(&xml_path);
        let entry_count = entries.len();
        let file = BestiaryFile { version: version.clone(), entries };
        let out = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("data/bestiary.json");
        let json = serde_json::to_string_pretty(&file).unwrap();
        fs::write(&out, json).expect("write bestiary.json");
        eprintln!(
            "Wrote {} ({} entries, version {})",
            out.display(),
            entry_count,
            version
        );
    }

    fn version_from_filename(path: &str) -> String {
        // expects bestiary_YYYYMMDD_fullexport.xml
        std::path::Path::new(path)
            .file_name()
            .and_then(|s| s.to_str())
            .and_then(|s| s.strip_prefix("bestiary_"))
            .and_then(|s| s.split('_').next())
            .unwrap_or("00000000")
            .to_string()
    }
}
