use std::path::{Path, PathBuf};

use clap::{Parser, Subcommand};
use comfy_table::{modifiers::UTF8_ROUND_CORNERS, presets::UTF8_FULL, Table, ContentArrangement};

use scribius_core::{Database, LogParser};

#[derive(Parser)]
#[command(name = "scribius", version, about = "Clan Lord log parser and stat tracker")]
struct Cli {
    /// Path to the SQLite database file
    #[arg(long, default_value = "scribius.db")]
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
}

fn main() {
    env_logger::init();
    let cli = Cli::parse();

    if let Err(e) = run(cli) {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}

fn run(cli: Cli) -> scribius_core::Result<()> {
    match cli.command {
        Commands::Scan { folder, force } => cmd_scan(&cli.db, &folder, force),
        Commands::Characters => cmd_characters(&cli.db),
        Commands::Summary { name } => cmd_summary(&cli.db, &name),
        Commands::Kills { name, sort, limit } => cmd_kills(&cli.db, &name, &sort, limit),
        Commands::Trainers { name } => cmd_trainers(&cli.db, &name),
        Commands::Pets { name } => cmd_pets(&cli.db, &name),
    }
}

fn cmd_scan(db_path: &str, folder: &Path, force: bool) -> scribius_core::Result<()> {
    println!("Scanning logs in: {}", folder.display());

    let db = Database::open(db_path)?;
    let parser = LogParser::new(db)?;
    let result = parser.scan_folder(folder, force)?;

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

fn cmd_characters(db_path: &str) -> scribius_core::Result<()> {
    let db = Database::open(db_path)?;
    let chars = db.list_characters()?;

    if chars.is_empty() {
        println!("No characters found. Run 'scribius scan <folder>' first.");
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

fn cmd_summary(db_path: &str, name: &str) -> scribius_core::Result<()> {
    let db = Database::open(db_path)?;
    let char = db
        .get_character(name)?
        .ok_or_else(|| scribius_core::ScribiusError::Data(format!("Character '{}' not found", name)))?;

    let char_id = char.id.unwrap();
    let kills = db.get_kills(char_id)?;
    let trainers = db.get_trainers(char_id)?;

    let total_solo: i64 = kills.iter().map(|k| k.total_solo()).sum();
    let total_assisted: i64 = kills.iter().map(|k| k.total_assisted()).sum();
    let total_killed_by: i64 = kills.iter().map(|k| k.killed_by_count).sum();
    let total_ranks: i64 = trainers.iter().map(|t| t.ranks).sum();

    // Find highest value kill (nemesis)
    let nemesis = kills
        .iter()
        .filter(|k| k.total_all() > 0)
        .max_by_key(|k| k.total_all());

    println!("=== {} ===", char.name);
    println!("Profession:     {}", char.profession);
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
) -> scribius_core::Result<()> {
    let db = Database::open(db_path)?;
    let char = db
        .get_character(name)?
        .ok_or_else(|| scribius_core::ScribiusError::Data(format!("Character '{}' not found", name)))?;

    let char_id = char.id.unwrap();
    let mut kills = db.get_kills(char_id)?;

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

fn cmd_trainers(db_path: &str, name: &str) -> scribius_core::Result<()> {
    let db = Database::open(db_path)?;
    let char = db
        .get_character(name)?
        .ok_or_else(|| scribius_core::ScribiusError::Data(format!("Character '{}' not found", name)))?;

    let char_id = char.id.unwrap();
    let trainers = db.get_trainers(char_id)?;

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

fn cmd_pets(db_path: &str, name: &str) -> scribius_core::Result<()> {
    let db = Database::open(db_path)?;
    let char = db
        .get_character(name)?
        .ok_or_else(|| scribius_core::ScribiusError::Data(format!("Character '{}' not found", name)))?;

    let char_id = char.id.unwrap();
    let pets = db.get_pets(char_id)?;

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
