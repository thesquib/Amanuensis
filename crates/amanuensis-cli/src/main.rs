use std::path::{Path, PathBuf};

use clap::{Parser, Subcommand};
use comfy_table::{modifiers::UTF8_ROUND_CORNERS, presets::UTF8_FULL, Table, ContentArrangement};

use amanuensis_core::{Database, LogParser, import_scribius};

#[derive(Parser)]
#[command(name = "amanuensis", version, about = "Clan Lord log parser and stat tracker")]
struct Cli {
    /// Path to the SQLite database file
    #[arg(long, default_value = "amanuensis.db")]
    db: String,

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
}

fn main() {
    env_logger::init();
    let cli = Cli::parse();

    if let Err(e) = run(cli) {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}

fn run(cli: Cli) -> amanuensis_core::Result<()> {
    match cli.command {
        Commands::Scan { folder, force } => cmd_scan(&cli.db, &folder, force),
        Commands::Characters => cmd_characters(&cli.db),
        Commands::Summary { name } => cmd_summary(&cli.db, &name),
        Commands::Kills { name, sort, limit } => cmd_kills(&cli.db, &name, &sort, limit),
        Commands::Trainers { name } => cmd_trainers(&cli.db, &name),
        Commands::Pets { name } => cmd_pets(&cli.db, &name),
        Commands::Lastys { name } => cmd_lastys(&cli.db, &name),
        Commands::Merge { target, sources } => cmd_merge(&cli.db, &target, &sources),
        Commands::Unmerge { name } => cmd_unmerge(&cli.db, &name),
        Commands::Import { source, output, force } => cmd_import(&source, &output, force),
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

fn cmd_scan(db_path: &str, folder: &Path, force: bool) -> amanuensis_core::Result<()> {
    println!("Scanning logs in: {}", folder.display());

    let db = Database::open(db_path)?;
    let parser = LogParser::new(db)?;
    let result = parser.scan_folder(folder, force)?;

    // Determine professions and coin levels after scanning
    parser.finalize_characters()?;

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
    if char.coin_level > 0 {
        println!("Coin Level:     {}", char.coin_level);
    }
    println!("Logins:         {}", char.logins);
    println!("Deaths:         {}", char.deaths);
    println!("Departs:        {}", char.departs);
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
    println!("--- Trainers ---");
    println!("Total ranks:    {}", total_ranks);
    println!("Trainers visited: {}", trainers.len());
    println!();
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
    if char.bells_broken > 0 || char.chains_broken > 0 || char.shieldstones_used > 0 {
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

    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .apply_modifier(UTF8_ROUND_CORNERS)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(vec!["Trainer", "Ranks", "Last Rank"]);

    for t in &trainers {
        table.add_row(vec![
            t.trainer_name.clone(),
            t.ranks.to_string(),
            t.date_of_last_rank.clone().unwrap_or_default(),
        ]);
    }

    let total_ranks: i64 = trainers.iter().map(|t| t.ranks).sum();
    println!("Trainers for {} ({} total ranks):", name, total_ranks);
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
        .set_header(vec!["Creature", "Type", "Messages", "Status"]);

    for l in &lastys {
        table.add_row(vec![
            l.creature_name.clone(),
            l.lasty_type.clone(),
            l.message_count.to_string(),
            if l.finished { "Completed" } else { "Active" }.to_string(),
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
