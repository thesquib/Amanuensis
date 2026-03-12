use std::collections::HashMap;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use clap::{Parser, Subcommand};
use comfy_table::{modifiers::UTF8_ROUND_CORNERS, presets::UTF8_FULL, Table, ContentArrangement};

use amanuensis_core::{Database, LogParser, TrainerDb, import_scribius, compute_fighter_stats};
use amanuensis_core::models::RankMode;

#[derive(Parser)]
#[command(name = "amanuensis", version, about = "Clan Lord log parser and stat tracker")]
struct Cli {
    /// Path to the SQLite database file
    #[arg(long, default_value = "amanuensis.db")]
    db: String,

    /// Use the GUI's default database location instead of --db
    #[arg(long, conflicts_with = "db")]
    gui_db: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Scan log files from a folder and store in database
    Scan {
        /// Path to the log folder (containing character subdirectories)
        folder: PathBuf,
        /// Force re-scan of already-read files
        #[arg(long)]
        force: bool,
        /// Scan subdirectories recursively
        #[arg(long, short = 'r')]
        recursive: bool,
        /// Skip FTS5 full-text indexing of log lines
        #[arg(long)]
        no_index: bool,
    },
    /// Scan individual log files
    ScanFiles {
        /// Individual log files to scan
        #[arg(required = true)]
        files: Vec<PathBuf>,
        /// Force re-scan of already-read files
        #[arg(long)]
        force: bool,
        /// Skip FTS5 full-text indexing of log lines
        #[arg(long)]
        no_index: bool,
    },
    /// List all detected characters
    Characters,
    /// Show character summary
    Summary {
        /// Character name
        name: String,
    },
    /// Show kill statistics
    Kills {
        /// Character name
        name: String,
        /// Sort by: total, solo, assisted, value, name
        #[arg(long, default_value = "total")]
        sort: String,
        /// Limit number of results
        #[arg(long)]
        limit: Option<usize>,
    },
    /// Show trainer rank progression
    Trainers {
        /// Character name
        name: String,
    },
    /// Show pet information
    Pets {
        /// Character name
        name: String,
    },
    /// Show lasty (creature training) progress
    Lastys {
        /// Character name
        name: String,
    },
    /// Merge characters (rename consolidation)
    Merge {
        /// Name of the primary character (whose name is kept)
        target: String,
        /// Names of the source characters to merge into the primary
        #[arg(required = true)]
        sources: Vec<String>,
    },
    /// Unmerge a previously merged character
    Unmerge {
        /// Name of the character to unmerge
        name: String,
    },
    /// Import data from a Scribius (Core Data) database
    Import {
        /// Path to the Scribius Model.sqlite file
        source: PathBuf,
        /// Output Amanuensis database path
        #[arg(long, default_value = "amanuensis.db")]
        output: String,
        /// Overwrite existing data in the output database
        #[arg(long)]
        force: bool,
    },
    /// Set modified ranks for a trainer
    SetRanks {
        /// Character name
        name: String,
        /// Trainer name
        trainer: String,
        /// Modified rank count to set
        ranks: i64,
    },
    /// Search log text (requires FTS5 index; scan without --no-index first)
    Search {
        /// Search query (FTS5 syntax)
        query: String,
        /// Filter to a specific character
        #[arg(long)]
        character: Option<String>,
        /// Max results
        #[arg(long, default_value = "50")]
        limit: i64,
    },
    /// Delete all data and reset the database
    Reset {
        /// Skip confirmation prompt
        #[arg(long)]
        yes: bool,
    },
    /// Show the built-in trainer catalog
    TrainerCatalog {
        /// Filter by profession (fighter, healer, mystic, ranger, bloodmage, champion)
        #[arg(long)]
        profession: Option<String>,
    },
    /// Show coin and loot statistics
    Coins {
        /// Character name
        name: String,
    },
    /// Show computed fighter statistics (Gorvin's Calculator)
    FighterStats {
        /// Character name
        name: String,
    },
    /// Show process logs from the last scan (warnings, errors, override skips)
    Logs {
        /// Filter by level: error, warn, info
        #[arg(long)]
        level: Option<String>,
        /// Limit number of entries shown
        #[arg(long, default_value = "200")]
        limit: usize,
    },
    /// Show trainer rank checkpoints for a character
    Checkpoints {
        /// Character name
        name: String,
        /// Show all historical checkpoints (default: latest per trainer only)
        #[arg(long)]
        all: bool,
        /// Filter to a specific trainer
        #[arg(long)]
        trainer: Option<String>,
    },
    /// Set rank override mode for a trainer
    SetRankMode {
        /// Character name
        name: String,
        /// Trainer name
        trainer: String,
        /// Mode: modifier (additive), override (manual only), override_until_date (baseline + post-cutoff)
        mode: String,
        /// Manual rank count (baseline for override/override_until_date modes)
        #[arg(long, default_value_t = 0)]
        ranks: i64,
        /// Cutoff date for override_until_date mode (M/D/YY format, e.g. 1/15/25)
        #[arg(long)]
        date: Option<String>,
    },
    /// Set or clear the profession override for a character
    SetProfession {
        /// Character name
        name: String,
        /// Profession: fighter, healer, mystic, ranger, bloodmage, champion — or "auto" to clear
        profession: String,
    },
    /// Print the path to the GUI's default database file
    GuiDbPath,
    /// Scan log files and extract item usage command help blocks (no DB needed)
    #[command(name = "useitem-help")]
    UseItemHelp {
        /// Log folder to scan
        folder: String,
        /// Recurse into subdirectories
        #[arg(short = 'r', long)]
        recursive: bool,
    },
}

fn main() {
    env_logger::init();
    let cli = Cli::parse();

    if let Err(e) = run(cli) {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}

/// Return the path to the GUI's default database file.
/// Mirrors Tauri's app_data_dir() for identifier "com.dfsw.Amanuensis".
fn gui_db_path() -> Option<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        // ~/Library/Application Support/com.dfsw.Amanuensis/amanuensis.db
        let home = std::env::var("HOME").ok()?;
        Some(PathBuf::from(home)
            .join("Library")
            .join("Application Support")
            .join("com.dfsw.Amanuensis")
            .join("amanuensis.db"))
    }
    #[cfg(target_os = "windows")]
    {
        // %APPDATA%\com.dfsw.Amanuensis\amanuensis.db
        let appdata = std::env::var("APPDATA").ok()?;
        Some(PathBuf::from(appdata)
            .join("com.dfsw.Amanuensis")
            .join("amanuensis.db"))
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        // $XDG_DATA_HOME/com.dfsw.Amanuensis/amanuensis.db
        // or ~/.local/share/com.dfsw.Amanuensis/amanuensis.db
        let data_home = std::env::var("XDG_DATA_HOME").ok()
            .map(PathBuf::from)
            .or_else(|| {
                let home = std::env::var("HOME").ok()?;
                Some(PathBuf::from(home).join(".local").join("share"))
            })?;
        Some(data_home.join("com.dfsw.Amanuensis").join("amanuensis.db"))
    }
}

fn resolve_db_path(cli: &Cli) -> amanuensis_core::Result<String> {
    if cli.gui_db {
        gui_db_path()
            .map(|p| p.to_string_lossy().into_owned())
            .ok_or_else(|| amanuensis_core::AmanuensisError::Data(
                "Could not determine GUI database path on this platform".to_string()
            ))
    } else {
        Ok(cli.db.clone())
    }
}

fn run(cli: Cli) -> amanuensis_core::Result<()> {
    // Handle commands that don't need a DB before resolving the db path
    if matches!(cli.command, Commands::GuiDbPath) {
        match gui_db_path() {
            Some(p) => println!("{}", p.display()),
            None => {
                eprintln!("Could not determine GUI database path on this platform.");
                std::process::exit(1);
            }
        }
        return Ok(());
    }
    if let Commands::UseItemHelp { folder, recursive } = &cli.command {
        return cmd_useitem_help(folder, *recursive);
    }

    let db_path = resolve_db_path(&cli)?;
    if cli.gui_db {
        eprintln!("Using GUI database: {}", db_path);
    }

    match cli.command {
        Commands::Scan { folder, force, recursive, no_index } => {
            cmd_scan(&db_path, &folder, force, recursive, no_index)
        }
        Commands::ScanFiles { files, force, no_index } => {
            cmd_scan_files(&db_path, &files, force, no_index)
        }
        Commands::Characters => cmd_characters(&db_path),
        Commands::Summary { name } => cmd_summary(&db_path, &name),
        Commands::Kills { name, sort, limit } => cmd_kills(&db_path, &name, &sort, limit),
        Commands::Trainers { name } => cmd_trainers(&db_path, &name),
        Commands::Pets { name } => cmd_pets(&db_path, &name),
        Commands::Lastys { name } => cmd_lastys(&db_path, &name),
        Commands::Merge { target, sources } => cmd_merge(&db_path, &target, &sources),
        Commands::Unmerge { name } => cmd_unmerge(&db_path, &name),
        Commands::Import { source, output, force } => cmd_import(&source, &output, force),
        Commands::SetRanks { name, trainer, ranks } => {
            cmd_set_ranks(&db_path, &name, &trainer, ranks)
        }
        Commands::Search { query, character, limit } => {
            cmd_search(&db_path, &query, character.as_deref(), limit)
        }
        Commands::Reset { yes } => cmd_reset(&db_path, yes),
        Commands::TrainerCatalog { profession } => cmd_trainer_catalog(profession.as_deref()),
        Commands::Coins { name } => cmd_coins(&db_path, &name),
        Commands::FighterStats { name } => cmd_fighter_stats(&db_path, &name),
        Commands::Logs { level, limit } => cmd_logs(&db_path, level.as_deref(), limit),
        Commands::Checkpoints { name, all, trainer } => {
            cmd_checkpoints(&db_path, &name, all, trainer.as_deref())
        }
        Commands::SetRankMode { name, trainer, mode, ranks, date } => {
            cmd_set_rank_mode(&db_path, &name, &trainer, &mode, ranks, date.as_deref())
        }
        Commands::SetProfession { name, profession } => {
            cmd_set_profession(&db_path, &name, &profession)
        }
        Commands::GuiDbPath => unreachable!("handled above"),
        Commands::UseItemHelp { folder, recursive } => cmd_useitem_help(&folder, recursive),
    }
}

/// Look up a character by name, erroring if it's been merged into another.
fn resolve_character(db: &Database, name: &str) -> amanuensis_core::Result<amanuensis_core::models::Character> {
    let char = db
        .get_character(name)?
        .ok_or_else(|| amanuensis_core::AmanuensisError::Data(format!("Character '{}' not found", name)))?;
    let char_id = char.id.unwrap();
    if let Some(target_name) = db.get_merged_into_name(char_id)? {
        return Err(amanuensis_core::AmanuensisError::Data(format!(
            "Character '{}' is merged into '{}'. Use '{}' instead, or run 'amanuensis unmerge {}' first.",
            name, target_name, target_name, name
        )));
    }
    Ok(char)
}

/// Build a multiplier map from TrainerDb metadata.
fn build_multiplier_map() -> HashMap<String, f64> {
    let tdb = TrainerDb::bundled().expect("Failed to load bundled trainer data");
    let meta = tdb.all_trainer_metadata();
    meta.into_iter().map(|m| (m.name, m.multiplier)).collect()
}

fn print_scan_result(result: &amanuensis_core::parser::ScanResult) {
    println!();
    println!("Scan complete:");
    println!("  Characters found:  {}", result.characters);
    println!("  Files scanned:     {}", result.files_scanned);
    println!("  Files skipped:     {}", result.skipped);
    println!("  Lines parsed:      {}", result.lines_parsed);
    println!("  Events recorded:   {}", result.events_found);
    if result.errors > 0 {
        println!("  Errors:            {}", result.errors);
    }
}

fn cmd_scan(db_path: &str, folder: &Path, force: bool, recursive: bool, no_index: bool) -> amanuensis_core::Result<()> {
    println!("Scanning logs in: {}", folder.display());

    let db = Database::open(db_path)?;
    let parser = LogParser::new(db)?;
    let index_lines = !no_index;

    let progress = |current: usize, total: usize, filename: &str| {
        eprint!("\r[{}/{}] {}", current + 1, total, filename);
        let _ = io::stderr().flush();
    };

    let result = if recursive {
        parser.scan_recursive_with_progress(folder, force, index_lines, progress)?
    } else {
        parser.scan_folder_with_progress(folder, force, index_lines, progress)?
    };
    eprintln!();

    parser.finalize_characters()?;
    print_scan_result(&result);

    Ok(())
}

fn cmd_scan_files(db_path: &str, files: &[PathBuf], force: bool, no_index: bool) -> amanuensis_core::Result<()> {
    println!("Scanning {} file(s)...", files.len());

    let db = Database::open(db_path)?;
    let parser = LogParser::new(db)?;
    let index_lines = !no_index;

    let progress = |current: usize, total: usize, filename: &str| {
        eprint!("\r[{}/{}] {}", current + 1, total, filename);
        let _ = io::stderr().flush();
    };

    let result = parser.scan_files_with_progress(files, force, index_lines, progress)?;
    eprintln!();

    parser.finalize_characters()?;
    print_scan_result(&result);

    Ok(())
}

fn cmd_characters(db_path: &str) -> amanuensis_core::Result<()> {
    let db = Database::open(db_path)?;
    let chars = db.list_characters()?;

    if chars.is_empty() {
        println!("No characters found. Run 'amanuensis scan <folder>' first.");
        return Ok(());
    }

    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .apply_modifier(UTF8_ROUND_CORNERS)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(vec!["Name", "Profession", "Logins", "Deaths", "Departs"]);

    for c in &chars {
        table.add_row(vec![
            &c.name,
            c.profession.as_str(),
            &c.logins.to_string(),
            &c.deaths.to_string(),
            &c.departs.to_string(),
        ]);
    }

    println!("{table}");
    Ok(())
}

fn cmd_summary(db_path: &str, name: &str) -> amanuensis_core::Result<()> {
    let db = Database::open(db_path)?;
    let base_char = resolve_character(&db, name)?;

    let char_id = base_char.id.unwrap();
    let char = db.get_character_merged(char_id)?.unwrap_or(base_char);
    let kills = db.get_kills_merged(char_id)?;
    let trainers = db.get_trainers_merged(char_id)?;
    let lastys = db.get_lastys_merged(char_id)?;
    let pets = db.get_pets_merged(char_id)?;

    let total_solo: i64 = kills.iter().map(|k| k.total_solo()).sum();
    let total_assisted: i64 = kills.iter().map(|k| k.total_assisted()).sum();
    let total_killed_by: i64 = kills.iter().map(|k| k.killed_by_count).sum();
    let total_ranks: i64 = trainers.iter().map(|t| t.ranks).sum();

    // Effective ranks via multipliers (respects rank_mode and apply_learning)
    let effective_ranks: f64 = trainers.iter().map(|t| {
        t.effective_ranks() as f64 * t.effective_multiplier
    }).sum();
    let effective_ranks = (effective_ranks * 10.0).round() / 10.0;

    // Find highest value kill (nemesis)
    let nemesis = kills
        .iter()
        .filter(|k| k.total_all() > 0)
        .max_by_key(|k| k.total_all());

    let merge_sources = db.get_merge_sources(char_id)?;

    println!("=== {} ===", char.name);
    if !merge_sources.is_empty() {
        let names: Vec<&str> = merge_sources.iter().map(|s| s.name.as_str()).collect();
        println!("Merged from:    {}", names.join(", "));
    }
    println!("Profession:     {}", char.profession);
    if let Some(ref start) = char.start_date {
        println!("Start Date:     {}", start);
    }
    if char.coin_level > 0 {
        println!("Coin Level:     {}", char.coin_level);
    } else if char.coin_level_interim > 0 {
        println!("Coin Level:     0 (interim: {} value)", char.coin_level_interim);
    }
    println!("Logins:         {}", char.logins);
    println!("Deaths:         {}", char.deaths);
    println!("Departs:        {}", char.departs);
    if char.good_karma > 0 || char.bad_karma > 0 || char.gave_good_karma > 0 || char.gave_bad_karma > 0 {
        println!("Good Karma:     {} received, {} given", char.good_karma, char.gave_good_karma);
        println!("Bad Karma:      {} received, {} given", char.bad_karma, char.gave_bad_karma);
    }
    if char.esteem > 0 {
        println!("Esteem:         {}", char.esteem);
    }
    println!();
    println!("--- Kills ---");
    println!("Solo kills:     {}", total_solo);
    println!("Assisted kills: {}", total_assisted);
    println!("Killed by:      {}", total_killed_by);
    println!("Unique creatures: {}", kills.len());
    if let Some(n) = nemesis {
        println!(
            "Most killed:    {} ({}x)",
            n.creature_name,
            n.total_all()
        );
    }
    println!();
    println!("--- Ranks ---");
    println!("Total ranks:    {}", total_ranks);
    println!("Effective ranks: {}", effective_ranks);
    println!("Trainers visited: {}", trainers.len());
    if char.untraining_count > 0 {
        println!("Untrained:      {}x", char.untraining_count);
    }
    println!();

    // Survival stats
    let total_exits = char.deaths + char.departs;
    if total_exits > 0 {
        let depart_rate = char.departs as f64 / total_exits as f64 * 100.0;
        println!("--- Survival ---");
        println!("Depart Rate:    {:.1}%", depart_rate);
        let total_chains = char.chains_used + char.chains_broken;
        if total_chains > 0 {
            let chain_break_rate = char.chains_broken as f64 / total_chains as f64 * 100.0;
            println!("Chain Break Rate: {:.1}%", chain_break_rate);
        }
        if char.eps_broken > 0 {
            println!("EPS Broken:     {}", char.eps_broken);
        }
        println!();
    }

    println!("--- Coins ---");
    println!("Picked up:      {}", char.coins_picked_up);
    println!("Fur shares:     {}", char.fur_coins);
    println!("Blood shares:   {}", char.blood_coins);
    println!("Mandible shares: {}", char.mandible_coins);
    if !lastys.is_empty() || !pets.is_empty() {
        println!();
        println!("--- Lastys & Pets ---");
        if !lastys.is_empty() {
            let finished = lastys.iter().filter(|l| l.finished).count();
            let active = lastys.len() - finished;
            println!("Lastys:         {} total ({} active, {} completed)", lastys.len(), active, finished);
        }
        if !pets.is_empty() {
            println!("Pets:           {}", pets.len());
        }
    }
    if char.bells_broken > 0 || char.chains_broken > 0 || char.shieldstones_used > 0
        || char.purgatory_pendant > 0
    {
        println!();
        println!("--- Equipment ---");
        if char.bells_used > 0 || char.bells_broken > 0 {
            println!("Bells used/broken: {}/{}", char.bells_used, char.bells_broken);
        }
        if char.chains_broken > 0 {
            println!("Chains broken: {}", char.chains_broken);
        }
        if char.shieldstones_used > 0 || char.shieldstones_broken > 0 {
            println!(
                "Shieldstones used/broken: {}/{}",
                char.shieldstones_used, char.shieldstones_broken
            );
        }
        if char.ethereal_portals > 0 {
            println!("Ethereal portals: {}", char.ethereal_portals);
        }
        if char.purgatory_pendant > 0 {
            println!("Purgatory pendant: {}", char.purgatory_pendant);
        }
    }
    if char.ore_found > 0 || char.wood_taken > 0 {
        println!();
        println!("--- Gathering ---");
        if char.ore_found > 0 {
            println!("Ore Found:      {} total", char.ore_found);
            if char.iron_ore_found > 0 { println!("  Iron:         {}", char.iron_ore_found); }
            if char.copper_ore_found > 0 { println!("  Copper:       {}", char.copper_ore_found); }
            if char.tin_ore_found > 0 { println!("  Tin:          {}", char.tin_ore_found); }
            if char.gold_ore_found > 0 { println!("  Gold:         {}", char.gold_ore_found); }
        }
        if char.wood_taken > 0 {
            println!("Wood Taken:     {}", char.wood_taken);
        }
        if char.wood_useless > 0 {
            println!("Wood Useless:   {}", char.wood_useless);
        }
    }

    Ok(())
}

fn cmd_kills(
    db_path: &str,
    name: &str,
    sort: &str,
    limit: Option<usize>,
) -> amanuensis_core::Result<()> {
    let db = Database::open(db_path)?;
    let char = resolve_character(&db, name)?;

    let char_id = char.id.unwrap();
    let mut kills = db.get_kills_merged(char_id)?;

    // Sort
    match sort {
        "solo" => kills.sort_by_key(|k| std::cmp::Reverse(k.total_solo())),
        "assisted" => kills.sort_by_key(|k| std::cmp::Reverse(k.total_assisted())),
        "value" => kills.sort_by_key(|k| std::cmp::Reverse(k.creature_value)),
        "name" => kills.sort_by(|a, b| a.creature_name.cmp(&b.creature_name)),
        _ => kills.sort_by_key(|k| std::cmp::Reverse(k.total_all())),
    }

    if let Some(limit) = limit {
        kills.truncate(limit);
    }

    if kills.is_empty() {
        println!("No kills found for {}.", name);
        return Ok(());
    }

    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .apply_modifier(UTF8_ROUND_CORNERS)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(vec![
            "Creature", "Solo", "Assisted", "Total", "Killed By", "Value", "First", "Last",
        ]);

    for k in &kills {
        table.add_row(vec![
            k.creature_name.clone(),
            k.total_solo().to_string(),
            k.total_assisted().to_string(),
            k.total_all().to_string(),
            k.killed_by_count.to_string(),
            k.creature_value.to_string(),
            k.date_first.clone().unwrap_or_default(),
            k.date_last.clone().unwrap_or_default(),
        ]);
    }

    println!("Kills for {}:", name);
    println!("{table}");
    Ok(())
}

fn cmd_trainers(db_path: &str, name: &str) -> amanuensis_core::Result<()> {
    let db = Database::open(db_path)?;
    let char = resolve_character(&db, name)?;

    let char_id = char.id.unwrap();
    let trainers = db.get_trainers_merged(char_id)?;

    if trainers.is_empty() {
        println!("No trainer ranks found for {}.", name);
        return Ok(());
    }

    let has_overrides = trainers.iter().any(|t| t.rank_mode != RankMode::Modifier.as_str());

    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .apply_modifier(UTF8_ROUND_CORNERS)
        .set_content_arrangement(ContentArrangement::Dynamic);

    if has_overrides {
        table.set_header(vec!["Trainer", "Ranks", "Modified", "Apply", "Effective", "Mode", "Last Rank"]);
    } else {
        table.set_header(vec!["Trainer", "Ranks", "Modified", "Apply", "Effective", "Last Rank"]);
    }

    let mut total_effective: f64 = 0.0;

    for t in &trainers {
        let eff = t.effective_ranks();
        let effective = eff as f64 * t.effective_multiplier;
        total_effective += effective;

        let apply_str = if t.apply_learning_unknown_count > 0 {
            format!("{}+{}?", t.apply_learning_ranks, t.apply_learning_unknown_count)
        } else {
            t.apply_learning_ranks.to_string()
        };

        let effective_str = if (t.effective_multiplier - 1.0).abs() < f64::EPSILON {
            format!("{}", eff)
        } else {
            format!("{:.1}", effective)
        };

        if has_overrides {
            let mode_str = if t.rank_mode == RankMode::Modifier.as_str() {
                String::new()
            } else if let Some(ref d) = t.override_date {
                format!("{} ({})", t.rank_mode, d)
            } else {
                t.rank_mode.clone()
            };
            table.add_row(vec![
                t.trainer_name.clone(),
                t.ranks.to_string(),
                t.modified_ranks.to_string(),
                apply_str,
                effective_str,
                mode_str,
                t.date_of_last_rank.clone().unwrap_or_default(),
            ]);
        } else {
            table.add_row(vec![
                t.trainer_name.clone(),
                t.ranks.to_string(),
                t.modified_ranks.to_string(),
                apply_str,
                effective_str,
                t.date_of_last_rank.clone().unwrap_or_default(),
            ]);
        }
    }

    total_effective = (total_effective * 10.0).round() / 10.0;
    let total_ranks: i64 = trainers.iter().map(|t| t.ranks).sum();
    println!("Trainers for {} ({} total ranks, {} effective):", name, total_ranks, total_effective);
    println!("{table}");
    Ok(())
}

fn cmd_lastys(db_path: &str, name: &str) -> amanuensis_core::Result<()> {
    let db = Database::open(db_path)?;
    let char = resolve_character(&db, name)?;

    let char_id = char.id.unwrap();
    let lastys = db.get_lastys_merged(char_id)?;

    if lastys.is_empty() {
        println!("No lastys found for {}.", name);
        return Ok(());
    }

    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .apply_modifier(UTF8_ROUND_CORNERS)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(vec!["Creature", "Type", "Messages", "Status", "First Seen", "Last Seen"]);

    for l in &lastys {
        let status = if l.finished {
            if let Some(ref date) = l.completed_date {
                format!("Completed ({})", date)
            } else {
                "Completed".to_string()
            }
        } else {
            "Active".to_string()
        };

        table.add_row(vec![
            l.creature_name.clone(),
            l.lasty_type.clone(),
            l.message_count.to_string(),
            status,
            l.first_seen_date.clone().unwrap_or_default(),
            l.last_seen_date.clone().unwrap_or_default(),
        ]);
    }

    let finished = lastys.iter().filter(|l| l.finished).count();
    println!("Lastys for {} ({} total, {} completed):", name, lastys.len(), finished);
    println!("{table}");
    Ok(())
}

fn cmd_import(source: &Path, output: &str, force: bool) -> amanuensis_core::Result<()> {
    println!("Importing from: {}", source.display());
    println!("Output database: {}", output);

    let result = import_scribius(source, output, force)?;

    println!();
    println!("Import complete:");
    println!("  Characters imported: {}", result.characters_imported);
    println!("  Characters skipped:  {}", result.characters_skipped);
    println!("  Trainers imported:   {}", result.trainers_imported);
    println!("  Kills imported:      {}", result.kills_imported);
    println!("  Pets imported:       {}", result.pets_imported);
    println!("  Lastys imported:     {}", result.lastys_imported);

    if !result.warnings.is_empty() {
        println!();
        println!("Warnings:");
        for w in &result.warnings {
            println!("  - {}", w);
        }
    }

    Ok(())
}

fn cmd_merge(db_path: &str, target: &str, sources: &[String]) -> amanuensis_core::Result<()> {
    let db = Database::open(db_path)?;
    let target_char = db
        .get_character(target)?
        .ok_or_else(|| amanuensis_core::AmanuensisError::Data(format!("Target character '{}' not found", target)))?;
    let target_id = target_char.id.unwrap();

    let mut source_ids = Vec::new();
    for name in sources {
        let source_char = db
            .get_character(name)?
            .ok_or_else(|| amanuensis_core::AmanuensisError::Data(format!("Source character '{}' not found", name)))?;
        source_ids.push(source_char.id.unwrap());
    }

    db.merge_characters(&source_ids, target_id)?;

    println!("Merged {} into {}:", sources.join(", "), target);
    println!("  {} is now the primary character", target);
    println!("  {} source(s) hidden from character list", sources.len());
    println!();
    println!("To undo, run: amanuensis unmerge <name>");

    Ok(())
}

fn cmd_unmerge(db_path: &str, name: &str) -> amanuensis_core::Result<()> {
    let db = Database::open(db_path)?;

    // The character might be hidden (merged), so use the variant that doesn't filter.
    let char = db
        .get_character_including_merged(name)?
        .ok_or_else(|| amanuensis_core::AmanuensisError::Data(format!("Character '{}' not found", name)))?;

    db.unmerge_character(char.id.unwrap())?;

    println!("Unmerged '{}' — it is now a separate character again.", name);

    Ok(())
}

fn cmd_pets(db_path: &str, name: &str) -> amanuensis_core::Result<()> {
    let db = Database::open(db_path)?;
    let char = resolve_character(&db, name)?;

    let char_id = char.id.unwrap();
    let pets = db.get_pets_merged(char_id)?;

    if pets.is_empty() {
        println!("No pets found for {}.", name);
        return Ok(());
    }

    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .apply_modifier(UTF8_ROUND_CORNERS)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(vec!["Pet Name", "Creature"]);

    for p in &pets {
        table.add_row(vec![p.pet_name.clone(), p.creature_name.clone()]);
    }

    println!("Pets for {}:", name);
    println!("{table}");
    Ok(())
}

fn cmd_set_ranks(db_path: &str, name: &str, trainer: &str, ranks: i64) -> amanuensis_core::Result<()> {
    let db = Database::open(db_path)?;
    let char = resolve_character(&db, name)?;
    let char_id = char.id.unwrap();

    db.set_modified_ranks(char_id, trainer, ranks)?;
    println!("Set modified ranks for {} with {}: {}", name, trainer, ranks);

    Ok(())
}

fn cmd_search(db_path: &str, query: &str, character: Option<&str>, limit: i64) -> amanuensis_core::Result<()> {
    let db = Database::open(db_path)?;

    let char_id = if let Some(name) = character {
        let char = resolve_character(&db, name)?;
        Some(char.id.unwrap())
    } else {
        None
    };

    let results = db.search_log_lines(query, char_id, limit, true, 0, 0)?;

    if results.is_empty() {
        println!("No results found for '{}'.", query);
        let line_count = db.log_line_count()?;
        if line_count == 0 {
            println!("Hint: The FTS5 index is empty. Scan logs without --no-index to populate it.");
        }
        return Ok(());
    }

    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .apply_modifier(UTF8_ROUND_CORNERS)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(vec!["File", "Character", "Content"]);

    for r in &results {
        // Strip path to just filename for readability
        let filename = Path::new(&r.file_path)
            .file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_else(|| r.file_path.clone());

        // Strip <mark> tags from snippet for terminal display
        let content = r.snippet.replace("<mark>", "").replace("</mark>", "");

        table.add_row(vec![filename, r.character_name.clone(), content]);
    }

    println!("Search results for '{}' ({} matches):", query, results.len());
    println!("{table}");
    Ok(())
}

fn cmd_reset(db_path: &str, yes: bool) -> amanuensis_core::Result<()> {
    if !yes {
        eprint!("This will delete all data in '{}'. Continue? [y/N] ", db_path);
        let _ = io::stderr().flush();
        let mut input = String::new();
        io::stdin().read_line(&mut input).map_err(|e| {
            amanuensis_core::AmanuensisError::Data(format!("Failed to read input: {}", e))
        })?;
        if !input.trim().eq_ignore_ascii_case("y") {
            println!("Aborted.");
            return Ok(());
        }
    }

    let path = Path::new(db_path);
    if path.exists() {
        std::fs::remove_file(path).map_err(|e| {
            amanuensis_core::AmanuensisError::Data(format!("Failed to delete '{}': {}", db_path, e))
        })?;
    }

    // Re-create empty database (schema is created on open)
    let _db = Database::open(db_path)?;
    println!("Database '{}' has been reset.", db_path);

    Ok(())
}

fn cmd_trainer_catalog(profession_filter: Option<&str>) -> amanuensis_core::Result<()> {
    let tdb = TrainerDb::bundled()?;
    let mut trainers = tdb.all_trainer_metadata();

    if let Some(prof) = profession_filter {
        let prof_lower = prof.to_lowercase();
        trainers.retain(|t| {
            t.profession
                .as_ref()
                .map(|p: &String| p.to_lowercase() == prof_lower)
                .unwrap_or(false)
        });
    }

    if trainers.is_empty() {
        println!("No trainers found.");
        return Ok(());
    }

    let has_combos = trainers.iter().any(|t| t.is_combo);

    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .apply_modifier(UTF8_ROUND_CORNERS)
        .set_content_arrangement(ContentArrangement::Dynamic);

    if has_combos {
        table.set_header(vec!["Name", "Profession", "Multiplier", "Combo", "Components"]);
    } else {
        table.set_header(vec!["Name", "Profession", "Multiplier"]);
    }

    for t in &trainers {
        let prof = t.profession.as_deref().unwrap_or("-");
        let mult = if (t.multiplier - 1.0).abs() < f64::EPSILON {
            "1.0".to_string()
        } else {
            format!("{:.1}", t.multiplier)
        };

        if has_combos {
            let combo = if t.is_combo { "Yes" } else { "" };
            let components = t.combo_components.join(", ");
            table.add_row(vec![
                t.name.clone(),
                prof.to_string(),
                mult,
                combo.to_string(),
                components,
            ]);
        } else {
            table.add_row(vec![t.name.clone(), prof.to_string(), mult]);
        }
    }

    println!("Trainer Catalog ({} trainers):", trainers.len());
    println!("{table}");
    Ok(())
}

fn cmd_coins(db_path: &str, name: &str) -> amanuensis_core::Result<()> {
    let db = Database::open(db_path)?;
    let base_char = resolve_character(&db, name)?;
    let char_id = base_char.id.unwrap();
    let char = db.get_character_merged(char_id)?.unwrap_or(base_char);

    println!("=== Coins for {} ===", char.name);
    println!("Coin Level:      {}", char.coin_level);
    println!("Coins Picked Up: {}", char.coins_picked_up);
    println!("Fur Shares:      {}  (worth: {})", char.fur_coins, char.fur_worth);
    println!("Blood Shares:    {}  (worth: {})", char.blood_coins, char.blood_worth);
    println!("Mandible Shares: {}  (worth: {})", char.mandible_coins, char.mandible_worth);
    if char.casino_won > 0 || char.casino_lost > 0 {
        println!("Casino Won:      {}", char.casino_won);
        println!("Casino Lost:     {}", char.casino_lost);
    }
    if char.chest_coins > 0 {
        println!("Chest Coins:     {}", char.chest_coins);
    }
    if char.bounty_coins > 0 {
        println!("Bounty Coins:    {}", char.bounty_coins);
    }
    if char.darkstone > 0 {
        println!("Darkstone:       {}", char.darkstone);
    }

    Ok(())
}

fn cmd_logs(db_path: &str, level: Option<&str>, limit: usize) -> amanuensis_core::Result<()> {
    let db = Database::open(db_path)?;
    let all_logs = db.get_process_logs()?;

    let logs: Vec<_> = all_logs
        .iter()
        .filter(|l| level.map(|lv| l.level == lv).unwrap_or(true))
        .rev()
        .take(limit)
        .collect();

    if logs.is_empty() {
        if all_logs.is_empty() {
            println!("No process logs found. Run 'amanuensis scan' first.");
        } else {
            println!("No logs matching level filter '{}'.", level.unwrap_or(""));
        }
        return Ok(());
    }

    let errors = logs.iter().filter(|l| l.level == "error").count();
    let warns = logs.iter().filter(|l| l.level == "warn").count();
    let infos = logs.iter().filter(|l| l.level == "info").count();
    println!(
        "Process logs ({} shown: {} error, {} warn, {} info):",
        logs.len(), errors, warns, infos
    );
    println!();

    for log in &logs {
        let prefix = match log.level.as_str() {
            "error" => "[ERROR]",
            "warn"  => "[WARN] ",
            _       => "[INFO] ",
        };
        println!("{}  {}  {}", prefix, log.created_at, log.message);
    }

    Ok(())
}

fn cmd_checkpoints(
    db_path: &str,
    name: &str,
    all: bool,
    trainer_filter: Option<&str>,
) -> amanuensis_core::Result<()> {
    let db = Database::open(db_path)?;
    let char = resolve_character(&db, name)?;
    let char_id = char.id.unwrap();

    let mut checkpoints = if all {
        db.get_all_trainer_checkpoints(char_id)?
    } else {
        db.get_latest_trainer_checkpoints(char_id)?
    };

    if let Some(filter) = trainer_filter {
        let filter_lc = filter.to_lowercase();
        checkpoints.retain(|c| c.trainer_name.to_lowercase().contains(&filter_lc));
    }

    if checkpoints.is_empty() {
        println!("No trainer checkpoints found for {}.", name);
        println!("Hint: Checkpoints are recorded when a trainer greets you with a rank-status message.");
        return Ok(());
    }

    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .apply_modifier(UTF8_ROUND_CORNERS)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(vec!["Trainer", "Min Ranks", "Max Ranks", "Timestamp"]);

    for c in &checkpoints {
        let max_str = c.rank_max.map(|v| v.to_string()).unwrap_or_else(|| "maxed".to_string());
        table.add_row(vec![
            c.trainer_name.clone(),
            c.rank_min.to_string(),
            max_str,
            c.timestamp.clone(),
        ]);
    }

    let label = if all { "all checkpoints" } else { "latest checkpoints" };
    println!("Trainer checkpoints for {} ({}, {} entries):", name, label, checkpoints.len());
    println!("{table}");

    Ok(())
}

fn cmd_set_rank_mode(
    db_path: &str,
    name: &str,
    trainer: &str,
    mode: &str,
    ranks: i64,
    date: Option<&str>,
) -> amanuensis_core::Result<()> {
    // Validate mode early for a friendly error message
    RankMode::parse(mode).ok_or_else(|| {
        amanuensis_core::AmanuensisError::Data(
            format!("Invalid mode '{}'. Must be: modifier, override, override_until_date", mode)
        )
    })?;

    if mode == "override_until_date" && date.is_none() {
        return Err(amanuensis_core::AmanuensisError::Data(
            "override_until_date mode requires --date <M/D/YY>".to_string()
        ));
    }

    let db = Database::open(db_path)?;
    let char = resolve_character(&db, name)?;
    let char_id = char.id.unwrap();

    db.set_rank_override(char_id, trainer, mode, ranks, date)?;

    match mode {
        "modifier" => println!("Set {} / {} to modifier mode (+{} adjusted ranks)", name, trainer, ranks),
        "override" => println!("Set {} / {} to override mode ({} manual ranks)", name, trainer, ranks),
        "override_until_date" => println!(
            "Set {} / {} to override_until_date mode ({} baseline ranks, cutoff: {})",
            name, trainer, ranks, date.unwrap_or("")
        ),
        _ => unreachable!(),
    }
    println!("Run 'amanuensis scan --force' to rebuild log-derived rank counts.");

    Ok(())
}

fn cmd_set_profession(db_path: &str, name: &str, profession: &str) -> amanuensis_core::Result<()> {
    let db = Database::open(db_path)?;
    let char = resolve_character(&db, name)?;
    let char_id = char.id.unwrap();

    let override_value = if profession.eq_ignore_ascii_case("auto") {
        None
    } else {
        // Validate
        let valid = ["fighter", "healer", "mystic", "ranger", "bloodmage", "champion"];
        if !valid.contains(&profession.to_lowercase().as_str()) {
            return Err(amanuensis_core::AmanuensisError::Data(format!(
                "Invalid profession '{}'. Must be one of: {} — or 'auto' to clear",
                profession,
                valid.join(", ")
            )));
        }
        // Capitalize first letter
        let mut s = profession.to_lowercase();
        if let Some(c) = s.get_mut(0..1) {
            c.make_ascii_uppercase();
        }
        Some(s)
    };

    db.set_profession_override(char_id, override_value.as_deref())?;

    if let Some(ref prof) = override_value {
        println!("Set profession for {} to {} (manual override).", name, prof);
    } else {
        println!("Cleared profession override for {} — auto-detection will apply.", name);
    }
    println!("Run 'amanuensis scan --force' to recompute profession from logs.");

    Ok(())
}

fn cmd_fighter_stats(db_path: &str, name: &str) -> amanuensis_core::Result<()> {
    let db = Database::open(db_path)?;
    let base_char = resolve_character(&db, name)?;
    let char_id = base_char.id.unwrap();
    let trainers = db.get_trainers_merged(char_id)?;

    // Build ranks map: trainer_name -> ranks + modified_ranks
    let mut ranks: HashMap<String, i64> = HashMap::new();
    for t in &trainers {
        let total = t.ranks + t.modified_ranks;
        if total > 0 {
            ranks.insert(t.trainer_name.clone(), total);
        }
    }

    let multiplier_map = build_multiplier_map();
    let stats = compute_fighter_stats(&ranks, &multiplier_map);

    println!("=== Fighter Stats for {} ===", name);
    println!("(Human / Roguewood Club / No Items)");
    println!();
    println!("Trained Ranks:    {}", stats.trained_ranks);
    println!("Effective Ranks:  {}", stats.effective_ranks);
    println!("Slaughter Points: {}", stats.slaughter_points);
    println!();
    println!("--- Offense ---");
    println!("Accuracy:         {}", stats.accuracy);
    println!("Damage:           {} - {}", stats.damage_min, stats.damage_max);
    println!("Offense:          {}", stats.offense);
    println!("Balance/Swing:    {}", stats.balance_per_swing);
    println!();
    println!("--- Defense ---");
    println!("Defense:          {}", stats.defense);
    println!("Balance:          {}", stats.balance);
    println!("Balance Regen:    {} ({:.1}/frame)", stats.balance_regen, stats.balance_per_frame);
    println!("Health:           {}", stats.health);
    println!("Health Regen:     {} ({:.1}/frame)", stats.health_regen, stats.health_per_frame);
    println!("Spirit:           {}", stats.spirit);
    println!("Spirit Regen:     {} ({:.1}/frame)", stats.spirit_regen, stats.spirit_per_frame);
    println!();
    println!("--- Other ---");
    println!("Heal Receptivity: {}", stats.heal_receptivity);
    println!("Shieldstone Drain: {}", stats.shieldstone_drain);

    Ok(())
}

// ── useitem-help ─────────────────────────────────────────────────────────────

/// Replace Mac Roman smart quotes with ASCII equivalents.
fn normalize_quotes(s: &str) -> String {
    s.replace(['\u{201C}', '\u{201D}'], "\"")
     .replace(['\u{2018}', '\u{2019}'], "'")
}

fn find_log_files(dir: &Path, recursive: bool) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else { return files };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() && recursive {
            files.extend(find_log_files(&path, true));
        } else if path.extension().and_then(|e| e.to_str()) == Some("txt") {
            files.push(path);
        }
    }
    files
}

/// Split a CL log line into `(timestamp_slice, message_slice)`.
/// Returns `("", line)` if no timestamp prefix is found.
/// Handles both 12-hour (`8:38:19p`) and 24-hour (`15:25:42`) formats.
fn split_timestamp(line: &str) -> (&str, &str) {
    let bytes = line.as_bytes();
    let mut i = 0;
    // date part: digits '/' digits '/' digits
    while i < bytes.len() && bytes[i].is_ascii_digit() { i += 1; }
    if i == 0 || i >= bytes.len() || bytes[i] != b'/' { return ("", line); }
    i += 1;
    while i < bytes.len() && bytes[i].is_ascii_digit() { i += 1; }
    if i >= bytes.len() || bytes[i] != b'/' { return ("", line); }
    i += 1;
    while i < bytes.len() && bytes[i].is_ascii_digit() { i += 1; }
    if i >= bytes.len() || bytes[i] != b' ' { return ("", line); }
    i += 1;
    // time part: digits:digits:digits[ap]?
    let time_start = i;
    while i < bytes.len() && bytes[i].is_ascii_digit() { i += 1; }
    if i == time_start || i >= bytes.len() || bytes[i] != b':' { return ("", line); }
    i += 1;
    while i < bytes.len() && bytes[i].is_ascii_digit() { i += 1; }
    if i >= bytes.len() || bytes[i] != b':' { return ("", line); }
    i += 1;
    while i < bytes.len() && bytes[i].is_ascii_digit() { i += 1; }
    // optional a/p suffix
    if i < bytes.len() && (bytes[i] == b'a' || bytes[i] == b'p') { i += 1; }
    if i >= bytes.len() || bytes[i] != b' ' { return ("", line); }
    (&line[..i], &line[i + 1..])
}


/// Convert a CL timestamp string ("M/D/YY H:MM:SSa") to seconds since midnight.
/// Used to determine whether an equip event and a help block are close in time,
/// so we can avoid attributing a delayed help response to the wrong last_equipped item.
fn ts_to_seconds(ts: &str) -> Option<u32> {
    // Find the space separating date from time
    let space = ts.rfind(' ')?;
    let time_part = &ts[space + 1..];
    let (time_str, pm, has_ampm) = if let Some(t) = time_part.strip_suffix('p') {
        (t, true, true)
    } else if let Some(t) = time_part.strip_suffix('a') {
        (t, false, true)
    } else {
        (time_part, false, false) // 24-hour or no indicator
    };
    let mut parts = time_str.splitn(3, ':');
    let h: u32 = parts.next()?.parse().ok()?;
    let m: u32 = parts.next()?.parse().ok()?;
    let s: u32 = parts.next()?.parse().ok()?;
    let h24 = if has_ampm {
        if h == 12 && !pm { 0 } else if h != 12 && pm { h + 12 } else { h }
    } else {
        h
    };
    Some(h24 * 3600 + m * 60 + s)
}

fn extract_item_name(message: &str) -> Option<String> {
    // 1. /useitem "name" — quoted, highest confidence
    if let Some(rest) = message.strip_prefix("/useitem \"") {
        if let Some(end) = rest.find('"') {
            return Some(rest[..end].to_string());
        }
    }
    // 2. /useitem word — unquoted (e.g., /useitem sungem /add <name>...)
    if let Some(rest) = message.strip_prefix("/useitem ") {
        if !rest.starts_with('"') {
            let word: String = rest.chars().take_while(|c| !c.is_whitespace()).collect();
            if !word.is_empty() {
                return Some(word);
            }
        }
    }
    // 3. * The <name> allows
    if let Some(rest) = message.strip_prefix("* The ") {
        if let Some(pos) = rest.find(" allows ") {
            return Some(rest[..pos].to_string());
        }
    }
    // 4. The <name> allows
    if let Some(rest) = message.strip_prefix("The ") {
        if let Some(pos) = rest.find(" allows ") {
            return Some(rest[..pos].to_string());
        }
    }
    // 5. Your <name> allows you to (e.g., "Your sunstone allows you to think...")
    if let Some(rest) = message.strip_prefix("Your ") {
        if let Some(pos) = rest.find(" allows you to") {
            return Some(rest[..pos].to_string());
        }
    }
    // 6. This <name> helps/allows — skip generic single words
    if let Some(rest) = message.strip_prefix("This ") {
        let keyword_pos = rest.find(" helps").or_else(|| rest.find(" allows"));
        if let Some(pos) = keyword_pos {
            let candidate = &rest[..pos];
            if candidate.contains(' ') {
                return Some(candidate.to_string());
            }
        }
    }
    None
}

/// True if `s` looks like `Type <quote>/...` where quote may be ASCII " or Mac Roman curly quotes.
fn starts_with_type_slash(s: &str) -> bool {
    // "Type " followed by a quote char then "/"
    if let Some(rest) = s.strip_prefix("Type ") {
        // skip optional opening quote: ASCII " or ' or Mac Roman smart quotes U+201C/U+2018
        // Strip optional opening quote: ASCII ", ASCII ', or Mac Roman smart quotes U+201C/U+2018
        let inner = rest.trim_start_matches(['"', '\'', '\u{201C}', '\u{2018}']);
        return inner.starts_with('/');
    }
    false
}

/// True if `message` starts with an unquoted `/useitem <word>` command.
fn starts_with_useitem_unquoted(message: &str) -> bool {
    if let Some(rest) = message.strip_prefix("/useitem ") {
        !rest.starts_with('"') && rest.chars().next().is_some_and(|c| c.is_alphanumeric())
    } else {
        false
    }
}

fn is_help_trigger(message: &str) -> bool {
    if starts_with_type_slash(message) { return true; }
    if message.starts_with("/useitem \"") { return true; }
    if starts_with_useitem_unquoted(message) { return true; }
    if message.starts_with("* /") { return true; }
    if message.starts_with("* The ") && message.contains(" allows ") { return true; }
    if message.starts_with("The ") && message.contains(" allows ") { return true; }
    if message.starts_with("Your ") && message.contains(" allows you to") { return true; }
    if message.starts_with("This ") && (message.contains(" helps") || message.contains(" allows")) { return true; }
    false
}

fn is_help_continuation(message: &str) -> bool {
    if starts_with_type_slash(message) { return true; }
    if message.starts_with("/useitem \"") { return true; }
    if starts_with_useitem_unquoted(message) { return true; }
    if message.starts_with("* /") { return true; }
    if message.starts_with("* Hot tip:") { return true; }
    if message.starts_with("* You can currently hold") { return true; }
    if message.starts_with("* Your ") && message.contains(" can hold") { return true; }
    // /command : or /command < (catches /THINK <msg>, /examine :, etc.)
    if let Some(rest) = message.strip_prefix('/') {
        if rest.chars().next().is_some_and(|c| c.is_alphabetic())
            && (rest.contains(" :") || rest.contains(" <")) { return true; }
    }
    // Type A/B description lines
    if message.starts_with("This ") && (message.contains(" helps") || message.contains(" allows")) { return true; }
    if message.starts_with("* The ") && message.contains(" allows ") { return true; }
    if message.starts_with("The ") && message.contains(" allows ") { return true; }
    if message.starts_with("Your ") && message.contains(" allows you to") { return true; }
    false
}

/// Use `last_equipped` as an item name fallback only when the equip event happened
/// within 15 seconds of the current help block.  This prevents delayed server
/// responses (e.g. gossamer's /use ? arriving 30 s after the player switched to
/// Ethereal Boots) from being attributed to whatever happened to be equipped last.
fn equip_fallback(
    last_equipped: &Option<String>,
    last_equipped_ts: Option<u32>,
    block_ts: Option<&str>,
) -> Option<String> {
    const MAX_SECS: u32 = 15;
    let equip_secs = last_equipped_ts?;
    let block_secs = ts_to_seconds(block_ts?)?;
    // Handle midnight rollover
    let delta = if block_secs >= equip_secs {
        block_secs - equip_secs
    } else {
        block_secs + 86400 - equip_secs
    };
    if delta <= MAX_SECS { last_equipped.clone() } else { None }
}

fn cmd_useitem_help(folder: &str, recursive: bool) -> amanuensis_core::Result<()> {
    use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

    let dir = Path::new(folder);
    if !dir.is_dir() {
        return Err(amanuensis_core::AmanuensisError::Data(
            format!("'{}' is not a directory", folder)
        ));
    }

    let mut files = find_log_files(dir, recursive);
    files.sort();
    let total_files = files.len();

    // item_name -> file_path -> set of lines seen for that item in that file.
    // Using per-file sets lets us apply majority voting: a line belongs to an item
    // only if it appeared in the majority of files where that item was observed.
    // This drops contaminated lines (e.g. gossamer lines that sneak into belt of the wild
    // because of delayed server responses) without any special-casing.
    let mut item_file_lines: HashMap<String, HashMap<PathBuf, HashSet<String>>> = HashMap::new();

    for path in &files {
        let bytes = match std::fs::read(path) {
            Ok(b) => b,
            Err(_) => continue,
        };
        let content = amanuensis_core::encoding::decode_log_bytes(&bytes);
        let lines: Vec<&str> = content.lines().collect();

        // State machine
        let mut last_equipped: Option<String> = None;
        // Timestamp when last_equipped was set — used to reject delayed help responses
        // that arrive long after the equip event (gossamer contamination prevention).
        let mut last_equipped_ts: Option<u32> = None;
        let mut in_block = false;
        let mut block_item: Option<String> = None;
        let mut block_lines: Vec<String> = Vec::new();
        // Timestamp of the most recently added block line ("same-timestamp = same server burst")
        let mut block_last_ts: Option<String> = None;

        // Trim same-timestamp noise from block edges and record lines into item_file_lines.
        let finalize = |block_item: &Option<String>,
                            block_lines: &mut Vec<String>,
                            item_file_lines: &mut HashMap<String, HashMap<PathBuf, HashSet<String>>>| {
            if let Some(item) = block_item {
                // Trim non-help lines from both edges (blank lines, loot messages, etc.
                // that share a timestamp with the help burst but aren't part of it).
                while let Some(last) = block_lines.last() {
                    if is_help_trigger(last) || is_help_continuation(last) { break; }
                    block_lines.pop();
                }
                while !block_lines.is_empty() {
                    if is_help_trigger(&block_lines[0]) || is_help_continuation(&block_lines[0]) { break; }
                    block_lines.remove(0);
                }
                if !block_lines.is_empty() {
                    let file_set = item_file_lines
                        .entry(item.clone()).or_default()
                        .entry(path.clone()).or_default();
                    for line in block_lines.drain(..) {
                        file_set.insert(line);
                    }
                }
            }
            block_lines.clear();
        };

        let mut i = 0;
        while i < lines.len() {
            let raw = lines[i];
            let (ts, message) = split_timestamp(raw);
            let cur_ts = if ts.is_empty() { None } else { Some(ts) };

            // Equip pattern always takes priority over same-timestamp logic
            if let Some(item_name) = message.strip_prefix("You equip your ").and_then(|s| s.strip_suffix('.')) {
                finalize(&block_item, &mut block_lines, &mut item_file_lines);
                in_block = false;
                block_item = None;
                block_last_ts = None;
                last_equipped = Some(item_name.to_string());
                last_equipped_ts = cur_ts.and_then(ts_to_seconds);
                i += 1;
                continue;
            }

            if in_block {
                // CL delivers a complete help block in a single server burst — all lines share
                // the same timestamp.  Accept same-timestamp lines unconditionally so prose
                // continuation lines (e.g. "Do not include spaces…") don't break the block.
                let same_ts = cur_ts.is_some() && cur_ts == block_last_ts.as_deref();
                if same_ts {
                    // Same burst: always include regardless of content
                    if let Some(name) = extract_item_name(message) {
                        block_item = Some(name);
                    }
                    block_lines.push(normalize_quotes(message));
                    i += 1;
                } else if is_help_trigger(message) {
                    // New server response with a trigger: end the current block and re-process
                    // this line as the start of a new block (don't advance i).
                    if block_item.is_some() { last_equipped = None; last_equipped_ts = None; }
                    finalize(&block_item, &mut block_lines, &mut item_file_lines);
                    in_block = false;
                    block_item = None;
                    block_last_ts = None;
                } else if is_help_continuation(message) {
                    // A continuation-only line at a new timestamp (e.g. manually queued command)
                    if let Some(name) = extract_item_name(message) {
                        block_item = Some(name);
                    } else if block_item.is_none() {
                        block_item = equip_fallback(&last_equipped, last_equipped_ts, cur_ts);
                    }
                    block_lines.push(normalize_quotes(message));
                    block_last_ts = cur_ts.map(|s| s.to_string());
                    i += 1;
                } else {
                    // End of block; reset last_equipped so delayed responses from a prior item
                    // can't pollute the next block.
                    if block_item.is_some() { last_equipped = None; }
                    finalize(&block_item, &mut block_lines, &mut item_file_lines);
                    in_block = false;
                    block_item = None;
                    block_last_ts = None;
                    // Re-process this line without advancing i
                }
            } else if is_help_trigger(message) {
                in_block = true;
                block_item = extract_item_name(message)
                    .or_else(|| equip_fallback(&last_equipped, last_equipped_ts, cur_ts));
                block_lines.push(normalize_quotes(message));
                block_last_ts = cur_ts.map(|s| s.to_string());
                i += 1;
            } else {
                i += 1;
            }
        }
        // Finalize any open block at EOF
        finalize(&block_item, &mut block_lines, &mut item_file_lines);
    }

    // Majority voting: for each item, keep only lines that appeared in at least half
    // of the files where that item was observed.  This drops contaminated lines that
    // snuck in from delayed server responses in a small fraction of sessions.
    // Then deduplicate items whose canonical line sets are identical (misattributed
    // blocks) by keeping the candidate seen in the most files.
    let mut items: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut item_file_count: HashMap<String, usize> = HashMap::new();

    for (item, file_to_lines) in &item_file_lines {
        let file_count = file_to_lines.len();
        item_file_count.insert(item.clone(), file_count);

        let threshold = file_count.div_ceil(2); // strict majority
        let mut line_counts: HashMap<&str, usize> = HashMap::new();
        for lines in file_to_lines.values() {
            for line in lines {
                *line_counts.entry(line.as_str()).or_insert(0) += 1;
            }
        }
        let canonical: BTreeSet<String> = line_counts
            .into_iter()
            .filter(|(_, count)| *count >= threshold)
            .map(|(line, _)| line.to_string())
            .collect();
        if !canonical.is_empty() {
            items.insert(item.clone(), canonical);
        }
    }

    // Fuzzy dedup: items whose canonical line sets have ≥50% Jaccard similarity are
    // treated as the same help block misattributed to different names (e.g. gossamer's
    // "This weapon helps…" block appearing under "pair of Ethereal Boots" because boots
    // happened to be last_equipped when the delayed response arrived).
    // Process in descending file-count order so the most-seen name is always kept.
    {
        let mut item_names: Vec<String> = items.keys().cloned().collect();
        item_names.sort_by_key(|n| std::cmp::Reverse(item_file_count.get(n).copied().unwrap_or(0)));

        let mut to_remove: HashSet<String> = HashSet::new();
        for i in 0..item_names.len() {
            let a = &item_names[i];
            if to_remove.contains(a) { continue; }
            let lines_a = items[a].clone();
            for b in item_names.iter().skip(i + 1) {
                if to_remove.contains(b) { continue; }
                let lines_b = &items[b];
                let intersection = lines_a.intersection(lines_b).count();
                if intersection == 0 { continue; }
                let union = lines_a.union(lines_b).count();
                if intersection * 2 >= union {
                    // Jaccard >= 0.5 — b has fewer or equal files, drop it
                    to_remove.insert(b.clone());
                }
            }
        }
        for name in &to_remove { items.remove(name); }
    }

    println!("Found {} item(s) across {} files.", items.len(), total_files);
    for (item, line_set) in &items {
        println!();
        println!("=== {} ===", item);
        for line in line_set {
            println!("  {}", line);
        }
    }

    Ok(())
}
