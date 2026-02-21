use std::collections::HashMap;

/// Human race base stats (from Gorvin's Fighter Calculator).
const RACE_ACCURACY: i64 = 300;
const RACE_MIN_DAMAGE: i64 = 100;
const RACE_MAX_DAMAGE: i64 = 200;
const RACE_BALANCE: i64 = 5000;
const RACE_BAL_REGEN: i64 = 400;
const RACE_HEALTH: i64 = 3000;
const RACE_DEFENSE: i64 = 300;
const RACE_HEALTH_REGEN: i64 = 100;
const RACE_SPIRIT: i64 = 800;
const RACE_SPIRIT_REGEN: i64 = 600;

/// Human race slaughter points base.
const RACE_SP: i64 = RACE_ACCURACY
    + RACE_MIN_DAMAGE
    + RACE_MAX_DAMAGE
    + RACE_BALANCE / 3
    + RACE_BAL_REGEN
    + RACE_HEALTH / 3
    + RACE_DEFENSE
    + RACE_HEALTH_REGEN
    + RACE_SPIRIT
    + RACE_SPIRIT_REGEN;

/// Slaughter point costs per trainer rank.
fn sp_cost(trainer: &str) -> Option<i64> {
    match trainer {
        "Atkus" => Some(21),
        "Darkus" => Some(19),
        "Balthus" => Some(18),
        "Regia" => Some(18),
        "Evus" => Some(24),
        "Swengus" => Some(18),
        "Histia" => Some(29),
        "Detha" => Some(22),
        "Bodrus" => Some(24),
        "Hardia" => Some(30),
        "Troilus" => Some(20),
        "Spiritus" => Some(20),
        "Aktur" => Some(22),
        "Atkia" => Some(21),
        "Darktur" => Some(20),
        "Angilsa" => Some(10),
        "Knox" => Some(12),
        "Heen" => Some(20),
        "Bangus" => Some(23),
        "Farly" => Some(22),
        "Stedfustus" => Some(25),
        "Forvyola" => Some(23),
        "Anemia" => Some(24),
        "Rodnus" => Some(20),
        "Erthron" => Some(29),
        _ => None,
    }
}

/// Map DB trainer names to formula names.
fn formula_name(db_name: &str) -> &str {
    match db_name {
        "Bangus Anmash" => "Bangus",
        "Farly Buff" => "Farly",
        _ => db_name,
    }
}

/// Computed fighter statistics.
#[derive(Debug, Clone)]
pub struct FighterStats {
    pub trained_ranks: i64,
    pub effective_ranks: f64,
    pub slaughter_points: i64,
    pub accuracy: i64,
    pub damage_min: i64,
    pub damage_max: i64,
    pub offense: i64,
    pub balance: i64,
    pub balance_regen: i64,
    pub balance_per_frame: f64,
    pub health: i64,
    pub health_regen: i64,
    pub health_per_frame: f64,
    pub defense: i64,
    pub spirit: i64,
    pub spirit_regen: i64,
    pub spirit_per_frame: f64,
    pub heal_receptivity: i64,
    pub balance_per_swing: i64,
    pub shieldstone_drain: i64,
}

/// Compute fighter stats from trainer ranks and multipliers.
///
/// `ranks`: trainer name -> total ranks (ranks + modified_ranks).
/// `multipliers`: trainer name -> effective rank multiplier.
///
/// Trainer names should use DB names; aliases (e.g. "Bangus Anmash") are
/// mapped internally to formula names (e.g. "Bangus").
pub fn compute_fighter_stats(
    ranks: &HashMap<String, i64>,
    multipliers: &HashMap<String, f64>,
) -> FighterStats {
    // Build a formula-name -> ranks map
    let mut r: HashMap<&str, i64> = HashMap::new();
    for (name, &total) in ranks {
        let fname = formula_name(name);
        *r.entry(fname).or_insert(0) += total;
    }

    let get = |name: &str| -> i64 { r.get(name).copied().unwrap_or(0) };

    let atkus = get("Atkus");
    let darkus = get("Darkus");
    let balthus = get("Balthus");
    let regia = get("Regia");
    let evus = get("Evus");
    let swengus = get("Swengus");
    let histia = get("Histia");
    let detha = get("Detha");
    let bodrus = get("Bodrus");
    let hardia = get("Hardia");
    let troilus = get("Troilus");
    let spiritus = get("Spiritus");
    let aktur = get("Aktur");
    let atkia = get("Atkia");
    let darktur = get("Darktur");
    let angilsa = get("Angilsa");
    let knox = get("Knox");
    let heen = get("Heen");
    let bangus = get("Bangus");
    let farly = get("Farly");
    let stedfustus = get("Stedfustus");
    let forvyola = get("Forvyola");
    let anemia = get("Anemia");
    let rodnus = get("Rodnus");
    let erthron = get("Erthron");

    // Primary stat formulas
    let accuracy = atkus * 16 + evus * 4 + bodrus * 4 + aktur * 25 + atkia * 13
        - knox * 4 - angilsa * 4 + bangus * 2 + erthron * 3;

    let min_damage = darkus * 6 + evus + bodrus + knox * 11 - angilsa
        + erthron + atkia * 3 + darktur * 10 + bangus * 2;

    let max_damage = darkus * 6 + evus + bodrus + knox * 11 - angilsa
        + erthron + atkia * 3 + darktur * 10 + bangus * 3 + hardia;

    let balance = balthus * 51 + evus * 18 + bodrus * 9 + atkus * 15 + darkus * 18
        + swengus * 30 + knox * 18 - angilsa * 18 + bangus * 21 + erthron * 15;

    let bal_regen = regia * 15 + evus * 4 + bodrus * 3 + atkus + darkus
        + swengus * 7 - knox * 2 + angilsa * 26 + forvyola * 8 + bangus * 5
        + erthron * 3 + atkia * 3 + stedfustus * 6 + anemia * 8;

    let health = histia * 111 + evus * 24 + bodrus * 24 + detha * 3 + rodnus * 36
        + farly * 48 - knox * 24 - angilsa * 24 + forvyola * 54 + bangus * 6
        + erthron * 24 + spiritus * 21 + stedfustus * 54 + anemia * 69;

    let defense = detha * 19 + evus + bodrus + hardia + farly * 2
        - knox - angilsa + erthron * 7;

    let health_regen = troilus * 6 + farly * 4 + bangus - anemia;

    let spirit = spiritus * 9;
    let spirit_regen = 0_i64; // Base fighter has no spirit regen trainers

    let heal_receptivity = 2 * rodnus + spiritus;

    // Total stats (trainer contribution + race base)
    let total_accuracy = accuracy + RACE_ACCURACY;
    let total_min_dmg = min_damage + RACE_MIN_DAMAGE;
    let total_max_dmg = max_damage + RACE_MAX_DAMAGE;
    let total_balance = balance + RACE_BALANCE;
    let total_bal_regen = bal_regen + RACE_BAL_REGEN;
    let total_health = health + RACE_HEALTH;
    let total_defense = defense + RACE_DEFENSE;
    let total_health_regen = health_regen + RACE_HEALTH_REGEN;
    let total_spirit = spirit + RACE_SPIRIT;
    let total_spirit_regen = spirit_regen + RACE_SPIRIT_REGEN;

    // Derived stats
    let damage_min = total_min_dmg.max(0) + 100;
    let damage_max = (total_max_dmg * 3).max(0) + 100;

    let offense = total_accuracy + (3 * total_max_dmg + total_min_dmg) / 4;
    let balance_per_swing = (5 * offense.max(200)) / 3;

    let shieldstone_drain = if heen < 50 {
        // (1066 - 436*heen/49) rounded
        ((1066 * 49 - 436 * heen) as f64 / 49.0).round() as i64
    } else if heen > 0 {
        ((628 * 50) as f64 / heen as f64).round() as i64
    } else {
        1066 // heen=0 case
    };

    let health_per_frame = total_health_regen as f64 / 100.0;
    let balance_per_frame = total_bal_regen as f64 / 6.0;
    let spirit_per_frame = total_spirit_regen as f64 / 100.0;

    // Trained ranks
    let trained_ranks: i64 = ranks.values().sum();

    // Effective ranks
    let mut effective_ranks: f64 = 0.0;
    for (name, &total) in ranks {
        let mult = multipliers.get(name.as_str()).copied().unwrap_or(1.0);
        effective_ranks += total as f64 * mult;
    }
    effective_ranks = (effective_ranks * 10.0).round() / 10.0;

    // Slaughter points
    let mut slaughter_points = RACE_SP;
    for (name, &total) in ranks {
        let fname = formula_name(name);
        if let Some(cost) = sp_cost(fname) {
            slaughter_points += total * cost;
        }
    }

    FighterStats {
        trained_ranks,
        effective_ranks,
        slaughter_points,
        accuracy: total_accuracy,
        damage_min,
        damage_max,
        offense,
        balance: total_balance,
        balance_regen: total_bal_regen,
        balance_per_frame,
        health: total_health,
        health_regen: total_health_regen,
        health_per_frame,
        defense: total_defense,
        spirit: total_spirit,
        spirit_regen: total_spirit_regen,
        spirit_per_frame,
        heal_receptivity,
        balance_per_swing,
        shieldstone_drain,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zero_ranks() {
        let ranks = HashMap::new();
        let multipliers = HashMap::new();
        let stats = compute_fighter_stats(&ranks, &multipliers);

        assert_eq!(stats.trained_ranks, 0);
        assert_eq!(stats.accuracy, RACE_ACCURACY);
        assert_eq!(stats.health, RACE_HEALTH);
        assert_eq!(stats.defense, RACE_DEFENSE);
        assert_eq!(stats.balance, RACE_BALANCE);
        assert_eq!(stats.damage_min, RACE_MIN_DAMAGE.max(0) + 100);
        assert_eq!(stats.damage_max, (RACE_MAX_DAMAGE * 3).max(0) + 100);
        assert_eq!(stats.slaughter_points, RACE_SP);
        assert_eq!(stats.shieldstone_drain, 1066);
    }

    #[test]
    fn test_single_trainer_atkus() {
        let mut ranks = HashMap::new();
        ranks.insert("Atkus".to_string(), 10);
        let multipliers = HashMap::new();
        let stats = compute_fighter_stats(&ranks, &multipliers);

        assert_eq!(stats.trained_ranks, 10);
        // Atkus contributes: accuracy +16/rank, balance +15/rank, bal_regen +1/rank
        assert_eq!(stats.accuracy, RACE_ACCURACY + 160);
        assert_eq!(stats.balance, RACE_BALANCE + 150);
        assert_eq!(stats.balance_regen, RACE_BAL_REGEN + 10);
        assert_eq!(stats.slaughter_points, RACE_SP + 10 * 21);
    }

    #[test]
    fn test_bangus_alias() {
        let mut ranks = HashMap::new();
        ranks.insert("Bangus Anmash".to_string(), 5);
        let multipliers = HashMap::new();
        let stats = compute_fighter_stats(&ranks, &multipliers);

        // Bangus contributes: accuracy +2, minDmg +2, maxDmg +3, balance +21,
        // balRegen +5, health +6, healthRegen +1 per rank
        assert_eq!(stats.accuracy, RACE_ACCURACY + 10);
        assert_eq!(stats.balance, RACE_BALANCE + 105);
        assert_eq!(stats.health, RACE_HEALTH + 30);
        assert_eq!(stats.slaughter_points, RACE_SP + 5 * 23);
    }

    #[test]
    fn test_effective_ranks_with_multiplier() {
        let mut ranks = HashMap::new();
        ranks.insert("Histia".to_string(), 100);
        let mut multipliers = HashMap::new();
        multipliers.insert("Histia".to_string(), 0.5);
        let stats = compute_fighter_stats(&ranks, &multipliers);

        assert_eq!(stats.trained_ranks, 100);
        assert!((stats.effective_ranks - 50.0).abs() < 0.01);
    }

    #[test]
    fn test_heen_shieldstone_below_50() {
        let mut ranks = HashMap::new();
        ranks.insert("Heen".to_string(), 25);
        let multipliers = HashMap::new();
        let stats = compute_fighter_stats(&ranks, &multipliers);

        // heen=25: round(1066 - 436*25/49) = round(1066 - 222.45) = round(843.55) = 844
        let expected = ((1066 * 49 - 436 * 25) as f64 / 49.0).round() as i64;
        assert_eq!(stats.shieldstone_drain, expected);
    }

    #[test]
    fn test_heen_shieldstone_above_50() {
        let mut ranks = HashMap::new();
        ranks.insert("Heen".to_string(), 100);
        let multipliers = HashMap::new();
        let stats = compute_fighter_stats(&ranks, &multipliers);

        // heen=100: round(628*50/100) = round(314) = 314
        assert_eq!(stats.shieldstone_drain, 314);
    }

    #[test]
    fn test_knox_negative_contributions() {
        let mut ranks = HashMap::new();
        ranks.insert("Knox".to_string(), 10);
        let multipliers = HashMap::new();
        let stats = compute_fighter_stats(&ranks, &multipliers);

        // Knox: accuracy -4, minDmg +11, maxDmg +11, balance +18,
        // balRegen -2, health -24, defense -1 per rank
        assert_eq!(stats.accuracy, RACE_ACCURACY - 40);
        assert_eq!(stats.health, RACE_HEALTH - 240);
        assert_eq!(stats.balance, RACE_BALANCE + 180);
    }

    #[test]
    fn test_farly_buff_alias() {
        let mut ranks = HashMap::new();
        ranks.insert("Farly Buff".to_string(), 10);
        let multipliers = HashMap::new();
        let stats = compute_fighter_stats(&ranks, &multipliers);

        // Farly contributes: health +48, defense +2, healthRegen +4 per rank
        assert_eq!(stats.health, RACE_HEALTH + 480);
        assert_eq!(stats.defense, RACE_DEFENSE + 20);
        assert_eq!(stats.health_regen, RACE_HEALTH_REGEN + 40);
        assert_eq!(stats.slaughter_points, RACE_SP + 10 * 22);
    }

    #[test]
    fn test_heen_shieldstone_at_exactly_50() {
        let mut ranks = HashMap::new();
        ranks.insert("Heen".to_string(), 50);
        let multipliers = HashMap::new();
        let stats = compute_fighter_stats(&ranks, &multipliers);

        // heen=50 takes the >= 50 branch (condition is `heen < 50`):
        // round(628*50/50) = round(628) = 628
        assert_eq!(stats.shieldstone_drain, 628);
    }

    #[test]
    fn test_multi_trainer_derived_stats() {
        let mut ranks = HashMap::new();
        ranks.insert("Atkus".to_string(), 20);   // accuracy +16, balance +15, balRegen +1
        ranks.insert("Darkus".to_string(), 10);  // minDmg +6, maxDmg +6, balance +18, balRegen +1
        ranks.insert("Detha".to_string(), 15);   // defense +19, health +3
        let multipliers = HashMap::new();
        let stats = compute_fighter_stats(&ranks, &multipliers);

        // Verify primary stats
        let exp_accuracy = RACE_ACCURACY + 20 * 16;
        let exp_min_dmg = RACE_MIN_DAMAGE + 10 * 6;
        let exp_max_dmg = RACE_MAX_DAMAGE + 10 * 6;
        let exp_defense = RACE_DEFENSE + 15 * 19;
        let exp_balance = RACE_BALANCE + 20 * 15 + 10 * 18;
        let exp_health = RACE_HEALTH + 15 * 3;

        assert_eq!(stats.accuracy, exp_accuracy);
        assert_eq!(stats.balance, exp_balance);
        assert_eq!(stats.defense, exp_defense);
        assert_eq!(stats.health, exp_health);

        // Verify derived: damage_min = max(totalMinDmg, 0) + 100
        assert_eq!(stats.damage_min, exp_min_dmg.max(0) + 100);
        // damage_max = max(totalMaxDmg * 3, 0) + 100
        assert_eq!(stats.damage_max, (exp_max_dmg * 3).max(0) + 100);

        // offense = accuracy + (3*maxDmg + minDmg) / 4
        let exp_offense = exp_accuracy + (3 * exp_max_dmg + exp_min_dmg) / 4;
        assert_eq!(stats.offense, exp_offense);

        // balance_per_swing = (5 * max(offense, 200)) / 3
        let exp_bps = (5 * exp_offense.max(200)) / 3;
        assert_eq!(stats.balance_per_swing, exp_bps);

        // trained_ranks = sum of all ranks
        assert_eq!(stats.trained_ranks, 45);

        // SP = RACE_SP + 20*21 + 10*19 + 15*22
        assert_eq!(stats.slaughter_points, RACE_SP + 20 * 21 + 10 * 19 + 15 * 22);
    }

    #[test]
    fn test_per_frame_calculations() {
        let mut ranks = HashMap::new();
        ranks.insert("Troilus".to_string(), 10);  // healthRegen +6/rank
        ranks.insert("Regia".to_string(), 20);    // balRegen +15/rank
        let multipliers = HashMap::new();
        let stats = compute_fighter_stats(&ranks, &multipliers);

        let exp_health_regen = RACE_HEALTH_REGEN + 10 * 6;
        let exp_bal_regen = RACE_BAL_REGEN + 20 * 15;

        assert_eq!(stats.health_regen, exp_health_regen);
        assert_eq!(stats.balance_regen, exp_bal_regen);

        // healthPerFrame = floor(healthRegen) / 100
        assert!((stats.health_per_frame - exp_health_regen as f64 / 100.0).abs() < f64::EPSILON);
        // balancePerFrame = balRegen / 6
        assert!((stats.balance_per_frame - exp_bal_regen as f64 / 6.0).abs() < f64::EPSILON);
        // spiritPerFrame = floor(spiritRegen) / 100 — no spirit regen trainers for base fighter
        assert!((stats.spirit_per_frame - RACE_SPIRIT_REGEN as f64 / 100.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_unknown_trainer_no_sp_contribution() {
        let mut ranks = HashMap::new();
        ranks.insert("SomeRandomTrainer".to_string(), 50);
        let multipliers = HashMap::new();
        let stats = compute_fighter_stats(&ranks, &multipliers);

        // Unknown trainer contributes no stat formulas and no SP
        assert_eq!(stats.slaughter_points, RACE_SP);
        assert_eq!(stats.trained_ranks, 50);
        // Effective ranks default to multiplier 1.0
        assert!((stats.effective_ranks - 50.0).abs() < 0.01);
        // All combat stats unchanged from base
        assert_eq!(stats.accuracy, RACE_ACCURACY);
        assert_eq!(stats.health, RACE_HEALTH);
    }

    #[test]
    fn test_heal_receptivity() {
        let mut ranks = HashMap::new();
        ranks.insert("Rodnus".to_string(), 10);    // healReceptivity: 2*rodnus
        ranks.insert("Spiritus".to_string(), 5);   // healReceptivity: +spiritus
        let multipliers = HashMap::new();
        let stats = compute_fighter_stats(&ranks, &multipliers);

        assert_eq!(stats.heal_receptivity, 2 * 10 + 5);
    }
}
