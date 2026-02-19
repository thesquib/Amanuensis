//! Integration tests comparing Scribius import vs Amanuensis log scan.
//!
//! Both tools were pointed at the same log folder:
//!     ~/Applications/Clan Lords/Text Logs/
//!
//! These tests use real local data files and are marked `#[ignore]` so they
//! only run when explicitly requested:
//!
//!     cargo test -p amanuensis-core --test real_data_comparison -- --ignored
//!
//! Required local paths:
//!   - Scribius DB:  ~/Library/Application Support/Scribius/Model.sqlite
//!   - Text logs:    ~/Applications/Clan Lords/Text Logs/

use std::path::{Path, PathBuf};

use amanuensis_core::{import_scribius, Database, LogParser};

fn scribius_db_path() -> PathBuf {
    dirs_path("Library/Application Support/Scribius/Model.sqlite")
}

fn text_logs_path() -> PathBuf {
    dirs_path("Applications/Clan Lords/Text Logs")
}

fn dirs_path(relative: &str) -> PathBuf {
    let home = std::env::var("HOME").expect("HOME not set");
    PathBuf::from(home).join(relative)
}

fn skip_if_missing(path: &Path) -> bool {
    if !path.exists() {
        eprintln!("Skipping: {} not found", path.display());
        return true;
    }
    false
}

// ---------------------------------------------------------------------------
// Helper: import Scribius DB into a temp file, return the Database
// ---------------------------------------------------------------------------
fn import_scribius_to_temp() -> (Database, tempfile::NamedTempFile) {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let tmp_path = tmp.path().to_str().unwrap();

    let result = import_scribius(&scribius_db_path(), tmp_path, true).unwrap();
    assert!(
        result.characters_imported > 0,
        "Should import at least one character"
    );

    let db = Database::open(tmp_path).unwrap();
    (db, tmp)
}

// ---------------------------------------------------------------------------
// Helper: scan main Text Logs into a temp DB, return the Database
// ---------------------------------------------------------------------------
fn scan_logs_to_temp() -> (Database, tempfile::NamedTempFile) {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let tmp_path = tmp.path().to_str().unwrap();

    let db = Database::open(tmp_path).unwrap();
    let parser = LogParser::new(db).unwrap();
    let result = parser.scan_folder(&text_logs_path(), false).unwrap();
    parser.finalize_characters().unwrap();

    assert!(result.files_scanned > 0, "Should scan some files");
    assert!(result.characters > 0, "Should find some characters");

    let db = parser.into_db();
    (db, tmp)
}

// ===========================================================================
// IMPORT FIDELITY TESTS
// Verify that importing the Scribius DB faithfully reproduces its values.
// ===========================================================================

#[test]
#[ignore]
fn import_ruuk_character_stats() {
    if skip_if_missing(&scribius_db_path()) {
        return;
    }

    let (db, _tmp) = import_scribius_to_temp();
    let ruuk = db.get_character("Ruuk").unwrap().expect("Ruuk should exist");

    // Values from Scribius ZMODELCHARACTERS for Ruuk
    assert_eq!(ruuk.logins, 742);
    assert_eq!(ruuk.deaths, 86); // ZFALLS
    assert_eq!(ruuk.departs, 0);
    assert_eq!(ruuk.esteem, 4);
    assert_eq!(ruuk.good_karma, 200); // ZGK
    assert_eq!(ruuk.chest_coins, 1345); // ZCHESTVALUE
    assert_eq!(ruuk.bounty_coins, 182); // ZMYBOUNTY
    assert_eq!(ruuk.fur_coins, 7908); // ZMYFURS
    assert_eq!(ruuk.mandible_coins, 428); // ZMYMANDIBLES
    assert_eq!(ruuk.blood_coins, 73); // ZMYBLOOD
    assert_eq!(ruuk.fur_worth, 6368); // ZMYRECOVEREDFURS
    assert_eq!(ruuk.mandible_worth, 210); // ZMYRECOVEREDMANDIBLES
    assert_eq!(ruuk.blood_worth, 46); // ZMYRECOVEREDBLOOD
    assert_eq!(ruuk.chains_used, 38); // ZCHAINSUSED
    assert_eq!(ruuk.chains_broken, 0);
    assert_eq!(ruuk.casino_won, 0);
    assert_eq!(ruuk.casino_lost, 0);
    assert_eq!(ruuk.darkstone, 0);
    assert_eq!(ruuk.ethereal_portals, 0);
    assert_eq!(ruuk.eps_broken, 0);
    assert_eq!(ruuk.bells_used, 0);
    assert_eq!(ruuk.bells_broken, 0);
    assert_eq!(ruuk.shieldstones_used, 0);
    assert_eq!(ruuk.shieldstones_broken, 0);
    assert_eq!(ruuk.purgatory_pendant, 0);
    assert_eq!(ruuk.coins_picked_up, 600); // ZCASINOCOINSFIXED

    // Profession: Scribius stores "Ranger" for Ruuk
    assert_eq!(
        ruuk.profession,
        amanuensis_core::models::character::Profession::Ranger
    );

    // Start date: Core Data timestamp 532809057 → 2017-11-19
    assert_eq!(ruuk.start_date.as_deref(), Some("2017-11-19"));

    // Coin level = sum of all trainer ranks (382 total)
    assert_eq!(ruuk.coin_level, 382);
}

#[test]
#[ignore]
fn import_ruuk_trainers() {
    if skip_if_missing(&scribius_db_path()) {
        return;
    }

    let (db, _tmp) = import_scribius_to_temp();
    let ruuk = db.get_character("Ruuk").unwrap().expect("Ruuk should exist");
    let trainers = db.get_trainers(ruuk.id.unwrap()).unwrap();

    // Scribius has 15 trainers for Ruuk (5 with 0 ranks)
    assert_eq!(trainers.len(), 15);

    let expected = [
        ("Duvin Beastlore", 136),
        ("Regia", 97),
        ("Gossamer", 51),
        ("Splash O'Sul", 35),
        ("Detha", 19),
        ("Rodnus", 18),
        ("Histia", 11),
        ("Skea Brightfur", 9),
        ("Bangus Anmash", 4),
        ("Pathfinding", 2),
        ("Aktur", 0),
        ("Atkia", 0),
        ("Angilsa", 0),
        ("Darktur", 0),
        ("Farly Buff", 0),
    ];

    for (name, expected_ranks) in &expected {
        let t = trainers
            .iter()
            .find(|t| t.trainer_name == *name)
            .unwrap_or_else(|| panic!("Trainer '{}' should exist", name));
        assert_eq!(
            t.ranks, *expected_ranks,
            "Trainer '{}' should have {} ranks, got {}",
            name, expected_ranks, t.ranks
        );
        assert_eq!(t.modified_ranks, 0, "All modified_ranks should be 0");
    }

    let total: i64 = trainers.iter().map(|t| t.ranks).sum();
    assert_eq!(total, 382);
}

#[test]
#[ignore]
fn import_ruuk_no_kills_pets_lastys() {
    if skip_if_missing(&scribius_db_path()) {
        return;
    }

    let (db, _tmp) = import_scribius_to_temp();
    let ruuk = db.get_character("Ruuk").unwrap().expect("Ruuk should exist");
    let char_id = ruuk.id.unwrap();

    // Scribius had empty kills/pets/lastys tables
    assert!(db.get_kills(char_id).unwrap().is_empty());
    assert!(db.get_pets(char_id).unwrap().is_empty());
    assert!(db.get_lastys(char_id).unwrap().is_empty());
}

#[test]
#[ignore]
fn import_all_characters() {
    if skip_if_missing(&scribius_db_path()) {
        return;
    }

    let (db, _tmp) = import_scribius_to_temp();
    let chars = db.list_characters().unwrap();

    // Scribius DB has exactly 12 characters
    assert_eq!(chars.len(), 12);

    let names: Vec<&str> = chars.iter().map(|c| c.name.as_str()).collect();
    for expected in [
        "Agratis Brax",
        "Agratis Feffer",
        "Cl_Movies",
        "Fungal Stein",
        "Kraken",
        "Mort",
        "Olga",
        "Ruuk",
        "Squib",
        "Tane",
        "Thesquib",
        "Tu Whawha",
    ] {
        assert!(names.contains(&expected), "Missing character: {}", expected);
    }
}

#[test]
#[ignore]
fn import_total_trainers() {
    if skip_if_missing(&scribius_db_path()) {
        return;
    }

    let (db, _tmp) = import_scribius_to_temp();

    let count: i64 = db
        .conn()
        .query_row("SELECT COUNT(*) FROM trainers", [], |row| row.get(0))
        .unwrap();
    assert_eq!(count, 38);
}

// ===========================================================================
// SCAN FIDELITY TESTS
// Verify our scanner produces consistent values from the main Text Logs folder.
// ===========================================================================

#[test]
#[ignore]
fn scan_ruuk_character_stats() {
    if skip_if_missing(&text_logs_path()) {
        return;
    }

    let (db, _tmp) = scan_logs_to_temp();
    let ruuk = db.get_character("Ruuk").unwrap().expect("Ruuk should exist");

    // Values from scanning ~/Applications/Clan Lords/Text Logs/
    // Logins: every file counts as at least 1 login (742 files)
    assert_eq!(ruuk.logins, 742);
    assert_eq!(ruuk.departs, 57);
    assert_eq!(ruuk.esteem, 5);
    assert_eq!(ruuk.good_karma, 200);
    assert_eq!(ruuk.bad_karma, 0);
    assert_eq!(ruuk.coins_picked_up, 1352);
    assert_eq!(ruuk.fur_coins, 9036);
    assert_eq!(ruuk.blood_coins, 133);
    assert_eq!(ruuk.fur_worth, 17883);
    assert_eq!(ruuk.blood_worth, 230);
    assert_eq!(ruuk.chains_used, 38);
    assert_eq!(ruuk.chains_broken, 0);
    assert_eq!(ruuk.bells_used, 0);
    assert_eq!(ruuk.bells_broken, 0);
    assert_eq!(ruuk.shieldstones_used, 0);
    assert_eq!(ruuk.shieldstones_broken, 0);

    // Profession: detected as Ranger via majority-vote from trainer professions
    assert_eq!(
        ruuk.profession,
        amanuensis_core::models::character::Profession::Ranger
    );

    // Start date from earliest timestamp in any log file
    assert_eq!(
        ruuk.start_date.as_deref(),
        Some("2017-11-20 07:31:48")
    );
}

#[test]
#[ignore]
fn scan_ruuk_trainers() {
    if skip_if_missing(&text_logs_path()) {
        return;
    }

    let (db, _tmp) = scan_logs_to_temp();
    let ruuk = db.get_character("Ruuk").unwrap().expect("Ruuk should exist");
    let trainers = db.get_trainers(ruuk.id.unwrap()).unwrap();

    // 11 trainers from the main logs
    assert_eq!(trainers.len(), 11);

    let expected = [
        ("Duvin Beastlore", 136),
        ("Regia", 97),
        ("Gossamer", 51),
        ("Spleisha'Sul", 35),
        ("Thieves' Cant", 22),
        ("Detha", 19),
        ("Rodnus", 18),
        ("Histia", 11),
        ("Skea Brightfur", 9),
        ("Bangus Anmash", 4),
        ("Pathfinding", 2),
    ];

    for (name, expected_ranks) in &expected {
        let t = trainers
            .iter()
            .find(|t| t.trainer_name == *name)
            .unwrap_or_else(|| panic!("Trainer '{}' should exist", name));
        assert_eq!(
            t.ranks, *expected_ranks,
            "Trainer '{}' should have {} ranks, got {}",
            name, expected_ranks, t.ranks
        );
    }

    let total: i64 = trainers.iter().map(|t| t.ranks).sum();
    assert_eq!(total, 404);
    assert_eq!(ruuk.coin_level, 404);
}

#[test]
#[ignore]
fn scan_ruuk_kills() {
    if skip_if_missing(&text_logs_path()) {
        return;
    }

    let (db, _tmp) = scan_logs_to_temp();
    let ruuk = db.get_character("Ruuk").unwrap().expect("Ruuk should exist");
    let kills = db.get_kills(ruuk.id.unwrap()).unwrap();

    assert_eq!(kills.len(), 372);

    // Top creature by total kills is Vermine (all slaughtered)
    let vermine = kills
        .iter()
        .find(|k| k.creature_name == "Vermine")
        .unwrap();
    assert_eq!(vermine.slaughtered_count, 1857);
    assert_eq!(vermine.total_solo(), 1857);
}

#[test]
#[ignore]
fn scan_ruuk_lastys() {
    if skip_if_missing(&text_logs_path()) {
        return;
    }

    let (db, _tmp) = scan_logs_to_temp();
    let ruuk = db.get_character("Ruuk").unwrap().expect("Ruuk should exist");
    let lastys = db.get_lastys(ruuk.id.unwrap()).unwrap();

    // 29 active lastys, all Movements type
    assert_eq!(lastys.len(), 29);
    assert!(lastys.iter().all(|l| l.lasty_type == "Movements"));
    assert!(lastys.iter().all(|l| !l.finished));
}

#[test]
#[ignore]
fn scan_ruuk_no_pets() {
    if skip_if_missing(&text_logs_path()) {
        return;
    }

    let (db, _tmp) = scan_logs_to_temp();
    let ruuk = db.get_character("Ruuk").unwrap().expect("Ruuk should exist");
    assert!(db.get_pets(ruuk.id.unwrap()).unwrap().is_empty());
}

#[test]
#[ignore]
fn scan_finds_expected_characters() {
    if skip_if_missing(&text_logs_path()) {
        return;
    }

    let (db, _tmp) = scan_logs_to_temp();
    let chars = db.list_characters().unwrap();

    // The main log folder has character subfolders; exact count may vary
    // slightly depending on which folders contain valid CL log files.
    assert!(chars.len() >= 10 && chars.len() <= 12);

    let names: Vec<&str> = chars.iter().map(|c| c.name.as_str()).collect();
    assert!(names.contains(&"Ruuk"), "Ruuk should be found");
}

// ===========================================================================
// CROSS-SOURCE COMPARISON TESTS
// Both tools scanned the same ~/Applications/Clan Lords/Text Logs/ folder.
// Differences here reveal parsing differences between Scribius and Amanuensis.
// ===========================================================================

#[test]
#[ignore]
fn compare_trainers_same_source() {
    if skip_if_missing(&scribius_db_path()) || skip_if_missing(&text_logs_path()) {
        return;
    }

    let (import_db, _tmp1) = import_scribius_to_temp();
    let (scan_db, _tmp2) = scan_logs_to_temp();

    let import_ruuk = import_db.get_character("Ruuk").unwrap().unwrap();
    let scan_ruuk = scan_db.get_character("Ruuk").unwrap().unwrap();

    let import_trainers = import_db.get_trainers(import_ruuk.id.unwrap()).unwrap();
    let scan_trainers = scan_db.get_trainers(scan_ruuk.id.unwrap()).unwrap();

    // Scribius: 15 trainers (5 with 0 ranks), Scan: 11 trainers
    assert_eq!(import_trainers.len(), 15);
    assert_eq!(scan_trainers.len(), 11);

    // 9 trainers match exactly by name and rank count
    let exact_matches = [
        ("Duvin Beastlore", 136),
        ("Regia", 97),
        ("Gossamer", 51),
        ("Detha", 19),
        ("Rodnus", 18),
        ("Histia", 11),
        ("Skea Brightfur", 9),
        ("Bangus Anmash", 4),
        ("Pathfinding", 2),
    ];

    for (name, expected_ranks) in &exact_matches {
        let import_t = import_trainers
            .iter()
            .find(|t| t.trainer_name == *name)
            .unwrap_or_else(|| panic!("'{}' should exist in import", name));
        let scan_t = scan_trainers
            .iter()
            .find(|t| t.trainer_name == *name)
            .unwrap_or_else(|| panic!("'{}' should exist in scan", name));

        assert_eq!(import_t.ranks, *expected_ranks);
        assert_eq!(scan_t.ranks, *expected_ranks);
    }

    // Scribius calls this trainer "Splash O'Sul"; our trainers.json calls it
    // "Spleisha'Sul". Both found 35 ranks from the same logs.
    let scribius_splash = import_trainers
        .iter()
        .find(|t| t.trainer_name == "Splash O'Sul")
        .expect("Splash O'Sul should exist in import");
    let scan_splash = scan_trainers
        .iter()
        .find(|t| t.trainer_name == "Spleisha'Sul")
        .expect("Spleisha'Sul should exist in scan");
    assert_eq!(scribius_splash.ranks, 35);
    assert_eq!(scan_splash.ranks, 35);

    // Thieves' Cant: found by our scan (22 ranks) but not by Scribius.
    // Scribius didn't have this trainer in its trainers.plist.
    assert!(
        !import_trainers
            .iter()
            .any(|t| t.trainer_name == "Thieves' Cant")
    );
    let scan_tc = scan_trainers
        .iter()
        .find(|t| t.trainer_name == "Thieves' Cant")
        .expect("Thieves' Cant should exist in scan");
    assert_eq!(scan_tc.ranks, 22);
}

#[test]
#[ignore]
fn compare_fields_that_match() {
    if skip_if_missing(&scribius_db_path()) || skip_if_missing(&text_logs_path()) {
        return;
    }

    let (import_db, _tmp1) = import_scribius_to_temp();
    let (scan_db, _tmp2) = scan_logs_to_temp();

    let import_ruuk = import_db.get_character("Ruuk").unwrap().unwrap();
    let scan_ruuk = scan_db.get_character("Ruuk").unwrap().unwrap();

    // Fields where both tools agree exactly (same logs, same result)
    assert_eq!(import_ruuk.good_karma, scan_ruuk.good_karma, "good_karma");
    assert_eq!(import_ruuk.bad_karma, scan_ruuk.bad_karma, "bad_karma");
    assert_eq!(
        import_ruuk.chains_used, scan_ruuk.chains_used,
        "chains_used"
    );
    assert_eq!(
        import_ruuk.chains_broken, scan_ruuk.chains_broken,
        "chains_broken"
    );
    assert_eq!(
        import_ruuk.bells_used, scan_ruuk.bells_used,
        "bells_used"
    );
    assert_eq!(
        import_ruuk.bells_broken, scan_ruuk.bells_broken,
        "bells_broken"
    );
    assert_eq!(
        import_ruuk.shieldstones_used, scan_ruuk.shieldstones_used,
        "shieldstones_used"
    );
    assert_eq!(
        import_ruuk.shieldstones_broken, scan_ruuk.shieldstones_broken,
        "shieldstones_broken"
    );
    assert_eq!(
        import_ruuk.darkstone, scan_ruuk.darkstone,
        "darkstone"
    );
    assert_eq!(
        import_ruuk.purgatory_pendant, scan_ruuk.purgatory_pendant,
        "purgatory_pendant"
    );
    assert_eq!(
        import_ruuk.ethereal_portals, scan_ruuk.ethereal_portals,
        "ethereal_portals"
    );
    assert_eq!(
        import_ruuk.eps_broken, scan_ruuk.eps_broken,
        "eps_broken"
    );
    assert_eq!(
        import_ruuk.casino_won, scan_ruuk.casino_won,
        "casino_won"
    );
    assert_eq!(
        import_ruuk.casino_lost, scan_ruuk.casino_lost,
        "casino_lost"
    );
}

/// Document known differences between Scribius and Amanuensis parsing.
/// These are intentionally tested as-is to prevent regressions while
/// the differences are investigated.
#[test]
#[ignore]
fn compare_known_differences() {
    if skip_if_missing(&scribius_db_path()) || skip_if_missing(&text_logs_path()) {
        return;
    }

    let (import_db, _tmp1) = import_scribius_to_temp();
    let (scan_db, _tmp2) = scan_logs_to_temp();

    let import_ruuk = import_db.get_character("Ruuk").unwrap().unwrap();
    let scan_ruuk = scan_db.get_character("Ruuk").unwrap().unwrap();

    // LOGINS: Both now count 742 (every file = at least 1 login).
    assert_eq!(import_ruuk.logins, 742);
    assert_eq!(scan_ruuk.logins, 742);

    // DEATHS: Both now count only the character's own deaths (86).
    assert_eq!(import_ruuk.deaths, 86);
    assert_eq!(scan_ruuk.deaths, 86);

    // DEPARTS: Scribius shows 0; our scan finds 57.
    // Scribius may not have tracked departs, or tracked them differently.
    assert_eq!(import_ruuk.departs, 0);
    assert_eq!(scan_ruuk.departs, 57);

    // ESTEEM: Off by 1. Minor parsing difference.
    assert_eq!(import_ruuk.esteem, 4);
    assert_eq!(scan_ruuk.esteem, 5);

    // MANDIBLE COINS/WORTH: Now parsed correctly (regex matches "mandibles" plural).
    // Scan finds more than Scribius due to self-recovery lines (solo recoveries
    // where no "Your share is" suffix appears — full worth goes to player).
    assert_eq!(import_ruuk.mandible_coins, 428);
    assert!(scan_ruuk.mandible_coins > 0, "mandible_coins should be non-zero");
    assert_eq!(import_ruuk.mandible_worth, 210);
    assert!(scan_ruuk.mandible_worth > 0, "mandible_worth should be non-zero");

    // BOUNTY COINS: Scribius tracked 182; Amanuensis doesn't parse bounty patterns.
    assert_eq!(import_ruuk.bounty_coins, 182);
    assert_eq!(scan_ruuk.bounty_coins, 0);

    // CHEST COINS: Scribius ZCHESTVALUE (1345) likely means something different
    // from our chest_coins field (11595) which tracks study charges.
    assert_eq!(import_ruuk.chest_coins, 1345);
    assert_eq!(scan_ruuk.chest_coins, 11595);

    // FUR COINS: Close but not exact. Scribius: 7908, Scan: 9036.
    // Our scan includes self-recovery lines (solo loot, full value to player).
    assert_eq!(import_ruuk.fur_coins, 7908);
    assert_eq!(scan_ruuk.fur_coins, 9036);

    // BLOOD COINS: Similar — scan includes self-recovery lines.
    assert_eq!(import_ruuk.blood_coins, 73);
    assert_eq!(scan_ruuk.blood_coins, 133);

    // FUR/BLOOD WORTH: Our values are higher because we count self-recovery
    // worth (solo loot where full value goes to player).
    assert_eq!(import_ruuk.fur_worth, 6368);
    assert_eq!(scan_ruuk.fur_worth, 17883);
    assert_eq!(import_ruuk.blood_worth, 46);
    assert_eq!(scan_ruuk.blood_worth, 230);
}

#[test]
#[ignore]
fn compare_profession_detection() {
    if skip_if_missing(&scribius_db_path()) || skip_if_missing(&text_logs_path()) {
        return;
    }

    let (import_db, _tmp1) = import_scribius_to_temp();
    let (scan_db, _tmp2) = scan_logs_to_temp();

    let import_ruuk = import_db.get_character("Ruuk").unwrap().unwrap();
    let scan_ruuk = scan_db.get_character("Ruuk").unwrap().unwrap();

    // Both Scribius and Amanuensis detect Ranger for Ruuk
    assert_eq!(
        import_ruuk.profession,
        amanuensis_core::models::character::Profession::Ranger
    );
    assert_eq!(
        scan_ruuk.profession,
        amanuensis_core::models::character::Profession::Ranger
    );
}

#[test]
#[ignore]
fn compare_scan_has_kills_import_does_not() {
    if skip_if_missing(&scribius_db_path()) || skip_if_missing(&text_logs_path()) {
        return;
    }

    let (import_db, _tmp1) = import_scribius_to_temp();
    let (scan_db, _tmp2) = scan_logs_to_temp();

    let import_ruuk = import_db.get_character("Ruuk").unwrap().unwrap();
    let scan_ruuk = scan_db.get_character("Ruuk").unwrap().unwrap();

    // Scribius had no kill records (empty ZMODELKILLS table)
    assert!(import_db
        .get_kills(import_ruuk.id.unwrap())
        .unwrap()
        .is_empty());

    // Our scan finds 372 unique creatures from parsing the logs
    let scan_kills = scan_db.get_kills(scan_ruuk.id.unwrap()).unwrap();
    assert_eq!(scan_kills.len(), 372);
}

// ===========================================================================
// MULTI-CHARACTER SCAN TESTS
// Verify scanner produces correct values for all characters with meaningful data.
// ===========================================================================

#[test]
#[ignore]
fn scan_olga_character_stats() {
    if skip_if_missing(&text_logs_path()) {
        return;
    }

    let (db, _tmp) = scan_logs_to_temp();
    let olga = db.get_character("Olga").unwrap().expect("Olga should exist");

    assert_eq!(olga.logins, 120);
    assert_eq!(olga.deaths, 30);
    assert_eq!(olga.departs, 15);
    assert_eq!(olga.esteem, 0);
    assert_eq!(olga.good_karma, 20);
    assert_eq!(olga.bad_karma, 0);
    assert_eq!(olga.fur_coins, 157);
    assert_eq!(olga.blood_coins, 47);
    assert_eq!(olga.mandible_coins, 12);
    assert_eq!(olga.fur_worth, 288);
    assert_eq!(olga.blood_worth, 108);
    assert_eq!(olga.mandible_worth, 25);
    assert_eq!(olga.chains_used, 0);
    assert_eq!(olga.coins_picked_up, 0);
    assert_eq!(olga.chest_coins, 1282);

    assert_eq!(
        olga.profession,
        amanuensis_core::models::character::Profession::Mystic
    );

    let kills = db.get_kills(olga.id.unwrap()).unwrap();
    assert_eq!(kills.len(), 30);
}

#[test]
#[ignore]
fn scan_squib_character_stats() {
    if skip_if_missing(&text_logs_path()) {
        return;
    }

    let (db, _tmp) = scan_logs_to_temp();
    let squib = db.get_character("Squib").unwrap().expect("Squib should exist");

    assert_eq!(squib.logins, 134);
    assert_eq!(squib.deaths, 9);
    assert_eq!(squib.departs, 0);
    assert_eq!(squib.esteem, 0);
    assert_eq!(squib.good_karma, 3);
    assert_eq!(squib.bad_karma, 0);
    assert_eq!(squib.fur_coins, 42);
    assert_eq!(squib.blood_coins, 26);
    assert_eq!(squib.mandible_coins, 0);
    assert_eq!(squib.fur_worth, 319);
    assert_eq!(squib.blood_worth, 149);
    assert_eq!(squib.mandible_worth, 0);
    assert_eq!(squib.chains_used, 0);
    assert_eq!(squib.coins_picked_up, 1453);
    assert_eq!(squib.chest_coins, 92);

    assert_eq!(
        squib.profession,
        amanuensis_core::models::character::Profession::Healer
    );

    let kills = db.get_kills(squib.id.unwrap()).unwrap();
    assert_eq!(kills.len(), 9);
}

#[test]
#[ignore]
fn scan_tu_whawha_character_stats() {
    if skip_if_missing(&text_logs_path()) {
        return;
    }

    let (db, _tmp) = scan_logs_to_temp();
    let tu = db.get_character("Tu Whawha").unwrap().expect("Tu Whawha should exist");

    assert_eq!(tu.logins, 71);
    assert_eq!(tu.deaths, 1);
    assert_eq!(tu.departs, 0);
    assert_eq!(tu.esteem, 0);
    assert_eq!(tu.good_karma, 1);
    assert_eq!(tu.bad_karma, 0);
    assert_eq!(tu.fur_coins, 42);
    assert_eq!(tu.blood_coins, 0);
    assert_eq!(tu.mandible_coins, 0);
    assert_eq!(tu.fur_worth, 42);
    assert_eq!(tu.blood_worth, 0);
    assert_eq!(tu.mandible_worth, 0);
    assert_eq!(tu.chains_used, 0);
    assert_eq!(tu.coins_picked_up, 0);
    assert_eq!(tu.chest_coins, 64);

    assert_eq!(
        tu.profession,
        amanuensis_core::models::character::Profession::Fighter
    );

    let kills = db.get_kills(tu.id.unwrap()).unwrap();
    assert_eq!(kills.len(), 21);
}

#[test]
#[ignore]
fn scan_tane_character_stats() {
    if skip_if_missing(&text_logs_path()) {
        return;
    }

    let (db, _tmp) = scan_logs_to_temp();
    let tane = db.get_character("Tane").unwrap().expect("Tane should exist");

    assert_eq!(tane.logins, 65);
    assert_eq!(tane.deaths, 0);
    assert_eq!(tane.departs, 1);
    assert_eq!(tane.good_karma, 0);
    assert_eq!(tane.fur_coins, 0);
    assert_eq!(tane.blood_coins, 0);
    assert_eq!(tane.mandible_coins, 0);

    assert_eq!(
        tane.profession,
        amanuensis_core::models::character::Profession::Unknown
    );

    let kills = db.get_kills(tane.id.unwrap()).unwrap();
    assert_eq!(kills.len(), 3);
}

/// Cross-source comparison for Olga: Scribius import vs log scan.
#[test]
#[ignore]
fn compare_olga_import_vs_scan() {
    if skip_if_missing(&scribius_db_path()) || skip_if_missing(&text_logs_path()) {
        return;
    }

    let (import_db, _tmp1) = import_scribius_to_temp();
    let (scan_db, _tmp2) = scan_logs_to_temp();

    let import_olga = import_db.get_character("Olga").unwrap().unwrap();
    let scan_olga = scan_db.get_character("Olga").unwrap().unwrap();

    // Fields that match exactly
    assert_eq!(import_olga.logins, scan_olga.logins, "logins");
    assert_eq!(import_olga.deaths, scan_olga.deaths, "deaths");
    assert_eq!(import_olga.good_karma, scan_olga.good_karma, "good_karma");
    assert_eq!(import_olga.bad_karma, scan_olga.bad_karma, "bad_karma");
    assert_eq!(import_olga.chains_used, scan_olga.chains_used, "chains_used");
}

/// Cross-source comparison for Squib: Scribius import vs log scan.
#[test]
#[ignore]
fn compare_squib_import_vs_scan() {
    if skip_if_missing(&scribius_db_path()) || skip_if_missing(&text_logs_path()) {
        return;
    }

    let (import_db, _tmp1) = import_scribius_to_temp();
    let (scan_db, _tmp2) = scan_logs_to_temp();

    let import_squib = import_db.get_character("Squib").unwrap().unwrap();
    let scan_squib = scan_db.get_character("Squib").unwrap().unwrap();

    // Fields that match exactly
    assert_eq!(import_squib.logins, scan_squib.logins, "logins");
    assert_eq!(import_squib.deaths, scan_squib.deaths, "deaths");
    assert_eq!(import_squib.good_karma, scan_squib.good_karma, "good_karma");
    assert_eq!(import_squib.bad_karma, scan_squib.bad_karma, "bad_karma");
    assert_eq!(import_squib.chains_used, scan_squib.chains_used, "chains_used");
}

/// Cross-source comparison for Tu Whawha: Scribius import vs log scan.
#[test]
#[ignore]
fn compare_tu_whawha_import_vs_scan() {
    if skip_if_missing(&scribius_db_path()) || skip_if_missing(&text_logs_path()) {
        return;
    }

    let (import_db, _tmp1) = import_scribius_to_temp();
    let (scan_db, _tmp2) = scan_logs_to_temp();

    let import_tu = import_db.get_character("Tu Whawha").unwrap().unwrap();
    let scan_tu = scan_db.get_character("Tu Whawha").unwrap().unwrap();

    // Fields that match exactly
    assert_eq!(import_tu.logins, scan_tu.logins, "logins");
    assert_eq!(import_tu.deaths, scan_tu.deaths, "deaths");
    assert_eq!(import_tu.good_karma, scan_tu.good_karma, "good_karma");
    assert_eq!(import_tu.bad_karma, scan_tu.bad_karma, "bad_karma");
    assert_eq!(import_tu.chains_used, scan_tu.chains_used, "chains_used");
}
