use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Profession {
    Fighter,
    Healer,
    Mystic,
    Ranger,
    Bloodmage,
    Champion,
    Unknown,
}

impl Profession {
    pub fn as_str(&self) -> &'static str {
        match self {
            Profession::Fighter => "Fighter",
            Profession::Healer => "Healer",
            Profession::Mystic => "Mystic",
            Profession::Ranger => "Ranger",
            Profession::Bloodmage => "Bloodmage",
            Profession::Champion => "Champion",
            Profession::Unknown => "Unknown",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s {
            "Fighter" => Profession::Fighter,
            "Healer" => Profession::Healer,
            "Mystic" => Profession::Mystic,
            "Ranger" => Profession::Ranger,
            "Bloodmage" => Profession::Bloodmage,
            "Champion" => Profession::Champion,
            _ => Profession::Unknown,
        }
    }
}

impl std::fmt::Display for Profession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Character {
    pub id: Option<i64>,
    pub name: String,
    pub profession: Profession,
    pub logins: i64,
    pub departs: i64,
    pub deaths: i64,
    pub esteem: i64,
    pub armor: String,
    // Coin tracking
    pub coins_picked_up: i64,
    pub casino_won: i64,
    pub casino_lost: i64,
    pub chest_coins: i64,
    pub bounty_coins: i64,
    pub fur_coins: i64,
    pub mandible_coins: i64,
    pub blood_coins: i64,
    // Equipment tracking
    pub bells_used: i64,
    pub bells_broken: i64,
    pub chains_used: i64,
    pub chains_broken: i64,
    pub shieldstones_used: i64,
    pub shieldstones_broken: i64,
    pub ethereal_portals: i64,
    pub darkstone: i64,
    pub purgatory_pendant: i64,
    pub coin_level: i64,
    // Karma
    pub good_karma: i64,
    pub bad_karma: i64,
    // Start date (earliest login timestamp)
    pub start_date: Option<String>,
    // Loot worth (total recovered value, not just share)
    pub fur_worth: i64,
    pub mandible_worth: i64,
    pub blood_worth: i64,
    // Ethereal Portal Stone broken (separate from portals opened)
    pub eps_broken: i64,
}

impl Character {
    pub fn new(name: String) -> Self {
        Self {
            id: None,
            name,
            profession: Profession::Unknown,
            logins: 0,
            departs: 0,
            deaths: 0,
            esteem: 0,
            armor: String::new(),
            coins_picked_up: 0,
            casino_won: 0,
            casino_lost: 0,
            chest_coins: 0,
            bounty_coins: 0,
            fur_coins: 0,
            mandible_coins: 0,
            blood_coins: 0,
            bells_used: 0,
            bells_broken: 0,
            chains_used: 0,
            chains_broken: 0,
            shieldstones_used: 0,
            shieldstones_broken: 0,
            ethereal_portals: 0,
            darkstone: 0,
            purgatory_pendant: 0,
            coin_level: 0,
            good_karma: 0,
            bad_karma: 0,
            start_date: None,
            fur_worth: 0,
            mandible_worth: 0,
            blood_worth: 0,
            eps_broken: 0,
        }
    }
}
