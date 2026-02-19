use std::collections::HashMap;
use std::path::Path;

use rusqlite::{params, Connection, OpenFlags};
use serde::Serialize;

use crate::data::CreatureDb;
use crate::error::{AmanuensisError, Result};
use crate::models::Profession;

/// Core Data epoch: 2001-01-01 00:00:00 UTC, expressed as seconds since Unix epoch.
const COREDATA_EPOCH_OFFSET: f64 = 978_307_200.0;

/// Result summary from importing a Scribius database.
#[derive(Debug, Clone, Serialize)]
pub struct ImportResult {
    pub characters_imported: usize,
    pub characters_skipped: usize,
    pub trainers_imported: usize,
    pub kills_imported: usize,
    pub pets_imported: usize,
    pub lastys_imported: usize,
    pub warnings: Vec<String>,
}

/// Import data from a Scribius (Core Data) SQLite database into a new Amanuensis database.
///
/// The source database is opened read-only. The output path must point to either a
/// non-existent file or an empty Amanuensis database (unless `force` is true).
pub fn import_scribius(
    scribius_path: &Path,
    output_db_path: &str,
    force: bool,
) -> Result<ImportResult> {
    // Validate source exists
    if !scribius_path.exists() {
        return Err(AmanuensisError::Data(format!(
            "Scribius database not found: {}",
            scribius_path.display()
        )));
    }

    // Check if output already has data
    if !force && Path::new(output_db_path).exists() {
        let existing = crate::db::Database::open(output_db_path)?;
        let count: i64 = existing.conn().query_row(
            "SELECT COUNT(*) FROM characters",
            [],
            |row| row.get(0),
        )?;
        if count > 0 {
            return Err(AmanuensisError::Data(
                "Output database already contains data. Use --force to overwrite, \
                 or specify a different output path."
                    .to_string(),
            ));
        }
    }

    // Open source read-only
    let src = Connection::open_with_flags(scribius_path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;

    // Create fresh target
    let dst = crate::db::Database::open(output_db_path)?;

    let mut result = ImportResult {
        characters_imported: 0,
        characters_skipped: 0,
        trainers_imported: 0,
        kills_imported: 0,
        pets_imported: 0,
        lastys_imported: 0,
        warnings: Vec::new(),
    };

    // Build set of character Z_PKs that have related data
    let chars_with_data = find_characters_with_data(&src)?;

    // Load creature DB for value lookups
    let creature_db = CreatureDb::bundled()?;

    // Import characters, building PK mapping
    let pk_map = import_characters(&src, &dst, &chars_with_data, &mut result)?;

    // Import related data within a transaction
    {
        let conn = dst.conn();
        conn.execute_batch("BEGIN")?;

        import_trainers(&src, conn, &pk_map, &mut result)?;
        import_kills(&src, conn, &pk_map, &creature_db, &mut result)?;
        import_pets(&src, conn, &pk_map, &mut result)?;
        import_lastys(&src, conn, &pk_map, &mut result)?;

        // Recalculate coin_level for each imported character
        for &new_id in pk_map.values() {
            let coin_level: i64 = conn.query_row(
                "SELECT COALESCE(SUM(ranks + modified_ranks), 0) FROM trainers WHERE character_id = ?1",
                params![new_id],
                |row| row.get(0),
            )?;
            conn.execute(
                "UPDATE characters SET coin_level = ?1 WHERE id = ?2",
                params![coin_level, new_id],
            )?;
        }

        conn.execute_batch("COMMIT")?;
    }

    Ok(result)
}

/// Known macOS/app bundle directory names that are spurious character entries.
const SPURIOUS_DIRS: &[&str] = &[
    "contents", "frameworks", "resources", "macos", "_codesignature",
    "helpers", "plugins", "xpcservices", "sparkle.framework",
    "versions", "current", "updater.app", "autoupdate.app",
    "sparkle_relaunchhelper.app",
];

/// Check if a character name looks valid (not a filesystem path or directory name).
fn is_valid_character_name(name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    // Reject paths
    if name.contains('.') || name.contains('/') || name.contains('\\') {
        return false;
    }
    // Reject known macOS directory names
    if SPURIOUS_DIRS.contains(&name.to_lowercase().as_str()) {
        return false;
    }
    true
}

/// Find all character Z_PKs that have at least one related record.
fn find_characters_with_data(src: &Connection) -> Result<HashMap<i64, bool>> {
    let mut has_data: HashMap<i64, bool> = HashMap::new();

    let tables = [
        ("ZMODELTRAINERS", "ZRELATIONSHIP"),
        ("ZMODELKILLS", "ZRELATIONSHIP"),
        ("ZMODELPETS", "ZRELATIONSHIP"),
        ("ZMODELLASTYS", "ZRELATIONSHIP"),
    ];

    for (table, fk_col) in &tables {
        // Table might not exist in all Scribius versions
        let sql = format!("SELECT DISTINCT {} FROM {}", fk_col, table);
        match src.prepare(&sql) {
            Ok(mut stmt) => {
                let rows = stmt.query_map([], |row| row.get::<_, i64>(0))?;
                for pk in rows.flatten() {
                    has_data.insert(pk, true);
                }
            }
            Err(_) => continue,
        }
    }

    Ok(has_data)
}

/// Convert Core Data timestamp (seconds since 2001-01-01) to YYYY-MM-DD string.
fn coredata_timestamp_to_date(ts: f64) -> Option<String> {
    if ts == 0.0 || ts.is_nan() {
        return None;
    }
    let unix = ts + COREDATA_EPOCH_OFFSET;
    chrono::DateTime::from_timestamp(unix as i64, 0)
        .map(|dt| dt.format("%Y-%m-%d").to_string())
}

/// Map Scribius profession string to Amanuensis profession.
fn map_profession(s: &str) -> Profession {
    match s {
        "Exile" | "exile" | "" => Profession::Unknown,
        other => Profession::parse(other),
    }
}

/// Import characters from Scribius, returning a map from Scribius Z_PK to Amanuensis id.
fn import_characters(
    src: &Connection,
    dst: &crate::db::Database,
    chars_with_data: &HashMap<i64, bool>,
    result: &mut ImportResult,
) -> Result<HashMap<i64, i64>> {
    let mut pk_map: HashMap<i64, i64> = HashMap::new();

    let mut stmt = src.prepare(
        "SELECT Z_PK, ZCHARACTERNAME, ZPROFESSION, ZLOGINS, ZDEPARTS, ZFALLS,
                ZESTEEM, ZARMOR,
                ZCASINOCOINSWON, ZCASINOCOINSLOST,
                ZCHESTVALUE, ZMYBOUNTY,
                ZMYFURS, ZMYMANDIBLES, ZMYBLOOD,
                ZBELLSUSED, ZBELLSBROKEN,
                ZCHAINSUSED, ZCHAINSBROKEN,
                ZSHIELDSTONESUSED, ZSHIELDSTONESBROKEN,
                ZDARKSTONE, ZPURG,
                ZSTARTDATE,
                ZGK,
                ZMYRECOVEREDFURS, ZMYRECOVEREDMANDIBLES, ZMYRECOVEREDBLOOD,
                ZEPS, ZEPSBREAKS,
                ZCASINOCOINSFIXED
         FROM ZMODELCHARACTERS",
    )?;

    let rows = stmt.query_map([], |row| {
        Ok(ScribiusCharacter {
            z_pk: row.get(0)?,
            name: row.get::<_, Option<String>>(1)?.unwrap_or_default(),
            profession: row.get::<_, Option<String>>(2)?.unwrap_or_default(),
            logins: row.get::<_, Option<i64>>(3)?.unwrap_or(0),
            departs: row.get::<_, Option<i64>>(4)?.unwrap_or(0),
            deaths: row.get::<_, Option<i64>>(5)?.unwrap_or(0),
            esteem: row.get::<_, Option<i64>>(6)?.unwrap_or(0),
            armor: row.get::<_, Option<i64>>(7)?.unwrap_or(0),
            casino_won: row.get::<_, Option<i64>>(8)?.unwrap_or(0),
            casino_lost: row.get::<_, Option<i64>>(9)?.unwrap_or(0),
            chest_coins: row.get::<_, Option<i64>>(10)?.unwrap_or(0),
            bounty_coins: row.get::<_, Option<i64>>(11)?.unwrap_or(0),
            fur_coins: row.get::<_, Option<i64>>(12)?.unwrap_or(0),
            mandible_coins: row.get::<_, Option<i64>>(13)?.unwrap_or(0),
            blood_coins: row.get::<_, Option<i64>>(14)?.unwrap_or(0),
            bells_used: row.get::<_, Option<i64>>(15)?.unwrap_or(0),
            bells_broken: row.get::<_, Option<i64>>(16)?.unwrap_or(0),
            chains_used: row.get::<_, Option<i64>>(17)?.unwrap_or(0),
            chains_broken: row.get::<_, Option<i64>>(18)?.unwrap_or(0),
            shieldstones_used: row.get::<_, Option<i64>>(19)?.unwrap_or(0),
            shieldstones_broken: row.get::<_, Option<i64>>(20)?.unwrap_or(0),
            darkstone: row.get::<_, Option<i64>>(21)?.unwrap_or(0),
            purgatory_pendant: row.get::<_, Option<i64>>(22)?.unwrap_or(0),
            start_date_ts: row.get::<_, Option<f64>>(23)?.unwrap_or(0.0),
            good_karma: row.get::<_, Option<i64>>(24)?.unwrap_or(0),
            fur_worth: row.get::<_, Option<i64>>(25)?.unwrap_or(0),
            mandible_worth: row.get::<_, Option<i64>>(26)?.unwrap_or(0),
            blood_worth: row.get::<_, Option<i64>>(27)?.unwrap_or(0),
            ethereal_portals: row.get::<_, Option<i64>>(28)?.unwrap_or(0),
            eps_broken: row.get::<_, Option<i64>>(29)?.unwrap_or(0),
            casino_fixed: row.get::<_, Option<i64>>(30)?.unwrap_or(0),
        })
    })?;

    let conn = dst.conn();

    for row in rows {
        let ch = row?;

        // Decide whether to import this character
        let has_related = chars_with_data.contains_key(&ch.z_pk);
        let has_profession = map_profession(&ch.profession) != Profession::Unknown;
        let has_valid_name = is_valid_character_name(&ch.name) && ch.logins > 0;

        if !has_related && !has_profession && !has_valid_name {
            result.characters_skipped += 1;
            continue;
        }

        if !is_valid_character_name(&ch.name) {
            result.characters_skipped += 1;
            result.warnings.push(format!(
                "Skipped character with invalid name: {:?} (Z_PK={})",
                ch.name, ch.z_pk
            ));
            continue;
        }

        let profession = map_profession(&ch.profession);
        let start_date = coredata_timestamp_to_date(ch.start_date_ts);

        conn.execute(
            "INSERT OR IGNORE INTO characters (
                name, profession, logins, departs, deaths, esteem, armor,
                casino_won, casino_lost, chest_coins, bounty_coins,
                fur_coins, mandible_coins, blood_coins,
                bells_used, bells_broken, chains_used, chains_broken,
                shieldstones_used, shieldstones_broken,
                darkstone, purgatory_pendant, start_date, good_karma,
                fur_worth, mandible_worth, blood_worth,
                ethereal_portals, eps_broken
            ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6, ?7,
                ?8, ?9, ?10, ?11,
                ?12, ?13, ?14,
                ?15, ?16, ?17, ?18,
                ?19, ?20,
                ?21, ?22, ?23, ?24,
                ?25, ?26, ?27,
                ?28, ?29
            )",
            params![
                ch.name, profession.as_str(), ch.logins, ch.departs, ch.deaths,
                ch.esteem, ch.armor.to_string(),
                ch.casino_won, ch.casino_lost, ch.chest_coins, ch.bounty_coins,
                ch.fur_coins, ch.mandible_coins, ch.blood_coins,
                ch.bells_used, ch.bells_broken, ch.chains_used, ch.chains_broken,
                ch.shieldstones_used, ch.shieldstones_broken,
                ch.darkstone, ch.purgatory_pendant, start_date, ch.good_karma,
                ch.fur_worth, ch.mandible_worth, ch.blood_worth,
                ch.ethereal_portals, ch.eps_broken,
            ],
        )?;

        let new_id: i64 = conn.query_row(
            "SELECT id FROM characters WHERE name = ?1",
            params![ch.name],
            |row| row.get(0),
        )?;
        pk_map.insert(ch.z_pk, new_id);

        if ch.casino_fixed != 0 {
            result.warnings.push(format!(
                "Character '{}' has ZCASINOCOINSFIXED={} (no mapping in Amanuensis, value dropped)",
                ch.name, ch.casino_fixed
            ));
        }

        result.characters_imported += 1;
    }

    Ok(pk_map)
}

fn import_trainers(
    src: &Connection,
    dst: &Connection,
    pk_map: &HashMap<i64, i64>,
    result: &mut ImportResult,
) -> Result<()> {
    let mut stmt = match src.prepare(
        "SELECT ZRELATIONSHIP, ZTRAINERNAME, ZRANKS, ZMODIFIEDRANKS, ZLASTTRAINED
         FROM ZMODELTRAINERS",
    ) {
        Ok(s) => s,
        Err(_) => return Ok(()), // Table doesn't exist
    };

    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, Option<String>>(1)?.unwrap_or_default(),
            row.get::<_, Option<i64>>(2)?.unwrap_or(0),
            row.get::<_, Option<i64>>(3)?.unwrap_or(0),
            row.get::<_, Option<f64>>(4)?.unwrap_or(0.0),
        ))
    })?;

    for row in rows {
        let (char_zpk, trainer_name, ranks, modified_ranks, last_trained_ts) = row?;

        let Some(&new_char_id) = pk_map.get(&char_zpk) else {
            continue;
        };

        if trainer_name.is_empty() {
            continue;
        }

        let date_of_last_rank = coredata_timestamp_to_date(last_trained_ts);

        dst.execute(
            "INSERT OR IGNORE INTO trainers (character_id, trainer_name, ranks, modified_ranks, date_of_last_rank)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![new_char_id, trainer_name, ranks, modified_ranks, date_of_last_rank],
        )?;

        result.trainers_imported += 1;
    }

    Ok(())
}

fn import_kills(
    src: &Connection,
    dst: &Connection,
    pk_map: &HashMap<i64, i64>,
    creature_db: &CreatureDb,
    result: &mut ImportResult,
) -> Result<()> {
    let mut stmt = match src.prepare(
        "SELECT ZRELATIONSHIP, ZNAME, ZKILL, ZSLAUGHTER, ZDISP, ZVANQ, ZKILLEDBY,
                ZCOINLEVEL,
                ZDATEFIRSTKILL, ZDATEFIRSTSLAUGHTER, ZDATEFIRSTDISP,
                ZDATELASTENCOUNTER
         FROM ZMODELKILLS",
    ) {
        Ok(s) => s,
        Err(_) => return Ok(()),
    };

    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, Option<String>>(1)?.unwrap_or_default(),
            row.get::<_, Option<i64>>(2)?.unwrap_or(0),
            row.get::<_, Option<i64>>(3)?.unwrap_or(0),
            row.get::<_, Option<i64>>(4)?.unwrap_or(0),
            row.get::<_, Option<i64>>(5)?.unwrap_or(0),
            row.get::<_, Option<i64>>(6)?.unwrap_or(0),
            row.get::<_, Option<i64>>(7)?.unwrap_or(0),
            row.get::<_, Option<f64>>(8)?.unwrap_or(0.0),
            row.get::<_, Option<f64>>(9)?.unwrap_or(0.0),
            row.get::<_, Option<f64>>(10)?.unwrap_or(0.0),
            row.get::<_, Option<f64>>(11)?.unwrap_or(0.0),
        ))
    })?;

    for row in rows {
        let (char_zpk, creature_name, killed, slaughtered, dispatched, vanquished, killed_by,
             coin_level, first_kill_ts, first_slaught_ts, first_disp_ts, last_enc_ts) = row?;

        let Some(&new_char_id) = pk_map.get(&char_zpk) else {
            continue;
        };

        if creature_name.is_empty() {
            continue;
        }

        // Use Amanuensis creature DB value if available, fall back to Scribius coin_level
        let creature_value = creature_db
            .get_value(&creature_name)
            .unwrap_or(coin_level as i32);

        // date_first = earliest non-zero of the three first-date timestamps
        let first_dates: Vec<f64> = [first_kill_ts, first_slaught_ts, first_disp_ts]
            .into_iter()
            .filter(|&ts| ts != 0.0)
            .collect();
        let date_first = first_dates
            .iter()
            .copied()
            .reduce(f64::min)
            .and_then(coredata_timestamp_to_date);

        let date_last = coredata_timestamp_to_date(last_enc_ts);

        dst.execute(
            "INSERT OR IGNORE INTO kills (
                character_id, creature_name,
                killed_count, slaughtered_count, dispatched_count, vanquished_count,
                killed_by_count, creature_value, date_first, date_last
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                new_char_id, creature_name,
                killed, slaughtered, dispatched, vanquished,
                killed_by, creature_value, date_first, date_last,
            ],
        )?;

        result.kills_imported += 1;
    }

    Ok(())
}

fn import_pets(
    src: &Connection,
    dst: &Connection,
    pk_map: &HashMap<i64, i64>,
    result: &mut ImportResult,
) -> Result<()> {
    let mut stmt = match src.prepare(
        "SELECT ZRELATIONSHIP, ZPETNAME, ZMAXCREATURENAME
         FROM ZMODELPETS",
    ) {
        Ok(s) => s,
        Err(_) => return Ok(()),
    };

    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, Option<String>>(1)?.unwrap_or_default(),
            row.get::<_, Option<String>>(2)?.unwrap_or_default(),
        ))
    })?;

    for row in rows {
        let (char_zpk, pet_name, creature_name) = row?;

        let Some(&new_char_id) = pk_map.get(&char_zpk) else {
            continue;
        };

        if pet_name.is_empty() {
            continue;
        }

        let creature = if creature_name.is_empty() {
            &pet_name
        } else {
            &creature_name
        };

        dst.execute(
            "INSERT OR IGNORE INTO pets (character_id, pet_name, creature_name)
             VALUES (?1, ?2, ?3)",
            params![new_char_id, pet_name, creature],
        )?;

        result.pets_imported += 1;
    }

    Ok(())
}

fn import_lastys(
    src: &Connection,
    dst: &Connection,
    pk_map: &HashMap<i64, i64>,
    result: &mut ImportResult,
) -> Result<()> {
    let mut stmt = match src.prepare(
        "SELECT ZRELATIONSHIP, ZCREATURENAME, ZLASTYTYPE, ZFINISHED, ZMESSAGECOUNT
         FROM ZMODELLASTYS",
    ) {
        Ok(s) => s,
        Err(_) => return Ok(()),
    };

    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, Option<String>>(1)?.unwrap_or_default(),
            row.get::<_, Option<String>>(2)?.unwrap_or_default(),
            row.get::<_, Option<i64>>(3)?.unwrap_or(0),
            row.get::<_, Option<i64>>(4)?.unwrap_or(0),
        ))
    })?;

    for row in rows {
        let (char_zpk, creature_name, lasty_type, finished, message_count) = row?;

        let Some(&new_char_id) = pk_map.get(&char_zpk) else {
            continue;
        };

        if creature_name.is_empty() {
            continue;
        }

        dst.execute(
            "INSERT OR IGNORE INTO lastys (character_id, creature_name, lasty_type, finished, message_count)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![new_char_id, creature_name, lasty_type, finished, message_count],
        )?;

        result.lastys_imported += 1;
    }

    Ok(())
}

/// Intermediate struct for reading Scribius character rows.
struct ScribiusCharacter {
    z_pk: i64,
    name: String,
    profession: String,
    logins: i64,
    departs: i64,
    deaths: i64,
    esteem: i64,
    armor: i64,
    casino_won: i64,
    casino_lost: i64,
    chest_coins: i64,
    bounty_coins: i64,
    fur_coins: i64,
    mandible_coins: i64,
    blood_coins: i64,
    bells_used: i64,
    bells_broken: i64,
    chains_used: i64,
    chains_broken: i64,
    shieldstones_used: i64,
    shieldstones_broken: i64,
    darkstone: i64,
    purgatory_pendant: i64,
    start_date_ts: f64,
    good_karma: i64,
    fur_worth: i64,
    mandible_worth: i64,
    blood_worth: i64,
    ethereal_portals: i64,
    eps_broken: i64,
    casino_fixed: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_coredata_timestamp_to_date() {
        // 2024-01-15 = 726969600 seconds after 2001-01-01
        assert_eq!(
            coredata_timestamp_to_date(726969600.0),
            Some("2024-01-15".to_string())
        );
        assert_eq!(coredata_timestamp_to_date(0.0), None);
        assert_eq!(coredata_timestamp_to_date(f64::NAN), None);
    }

    #[test]
    fn test_is_valid_character_name() {
        assert!(is_valid_character_name("Ruuk"));
        assert!(is_valid_character_name("olga"));
        assert!(!is_valid_character_name(""));
        assert!(!is_valid_character_name("Contents"));
        assert!(!is_valid_character_name("Frameworks"));
        assert!(!is_valid_character_name("com.dfsw.Scribius"));
        assert!(!is_valid_character_name("/Users/thesquib/Applications"));
        assert!(!is_valid_character_name("Sparkle.framework"));
    }

    #[test]
    fn test_map_profession() {
        assert_eq!(map_profession("Fighter"), Profession::Fighter);
        assert_eq!(map_profession("Exile"), Profession::Unknown);
        assert_eq!(map_profession(""), Profession::Unknown);
        assert_eq!(map_profession("Healer"), Profession::Healer);
    }
}
