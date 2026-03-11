use chrono::NaiveDateTime;

/// Represents a single parsed event from a log line.
#[derive(Debug, Clone, PartialEq)]
pub enum KillVerb {
    Killed,
    Slaughtered,
    Vanquished,
    Dispatched,
}

impl std::fmt::Display for KillVerb {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            KillVerb::Killed => write!(f, "killed"),
            KillVerb::Slaughtered => write!(f, "slaughtered"),
            KillVerb::Vanquished => write!(f, "vanquished"),
            KillVerb::Dispatched => write!(f, "dispatched"),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum LogEvent {
    /// Character logged in: Welcome to Clan Lord, {name}!
    Login { name: String },
    /// Character reconnected: Welcome back, {name}!
    Reconnect { name: String },
    /// Solo kill: You {verb} a/an {creature}.
    SoloKill { creature: String, verb: KillVerb },
    /// Assisted kill: You helped {verb} a/an {creature}.
    AssistedKill { creature: String, verb: KillVerb },
    /// Character fell: {name} has fallen to a/an {creature/cause}.
    Fallen { name: String, cause: String },
    /// Character recovered: {name} is no longer fallen.
    Recovered { name: String },
    /// Spirit depart (first time)
    FirstDepart,
    /// Spirit depart with count
    Depart { count: i64 },
    /// Trainer rank gained
    TrainerRank { trainer_name: String, message: String },
    /// Coins picked up: * You pick up {N} coins.
    CoinsPickedUp { amount: i64 },
    /// Loot share: recovers the {item}, worth {W}c. Your share is {N}c.
    LootShare { item: String, worth: i64, amount: i64, loot_type: LootType },
    /// Coin balance: You have {N} coins.
    CoinBalance { amount: i64 },
    /// Bell broken
    BellBroken,
    /// Bell used (summoning)
    BellUsed,
    /// Chain break
    ChainBreak,
    /// Chain shatter (link)
    ChainShatter,
    /// Chain snap
    ChainSnap,
    /// Chain used (dragging someone)
    ChainUsed { target: String },
    /// Shieldstone activated
    ShieldstoneUsed,
    /// Shieldstone inert
    ShieldstoneBroken,
    /// Ethereal portal opened
    EtherealPortalOpened,
    /// Ethereal portal stone disappeared
    EtherealPortalStoneUsed,
    /// Study progress: studying {creature}, {progress} left
    StudyProgress { creature: String, progress: String },
    /// Experience/mindful gain
    ExperienceGain,
    /// Clanning status change
    ClanningChange { name: String, is_clanning: bool },
    /// Disconnect from server
    Disconnect,
    /// Study charge: coins for advanced studies
    StudyCharge { amount: i64 },
    /// Lasty progress: learning to befriend/morph/fight a creature
    LastyProgress { creature: String, lasty_type: String },
    /// Lasty finished: completed learning to befriend/morph/fight a creature
    LastyFinished { creature: String, lasty_type: String },
    /// Lasty begin study: started studying a creature
    LastyBeginStudy { creature: String, lasty_type: String },
    /// Lasty completed: finished training with a trainer
    LastyCompleted { trainer: String },
    /// Study abandon: player abandoned creature study
    StudyAbandon { creature: String },
    /// Apply-learning bonus rank for a trainer
    /// is_full: true = "much more" (10 confirmed ranks), false = "more" (1-9 unknown)
    ApplyLearningRank { character_name: String, trainer_name: String, is_full: bool },
    /// Karma received: "You just received good/bad karma from {name}."
    KarmaReceived { good: bool },
    /// Karma given: "You gave good/bad karma to {name}."
    KarmaGiven { good: bool },
    /// Esteem gain: "* You gain esteem." or "* You gain experience and esteem."
    EsteemGain,
    /// Profession announcement from NPC (circle test or "become a" message)
    ProfessionAnnouncement { name: String, profession: String },
    /// Ore found: "You found a lump of {type} ore!"
    /// The string is the ore type in lowercase, e.g. "iron", "copper", "tin", "gold".
    OreFound(String),
    /// Wood taken: "You take the wood."
    WoodTaken,
    /// Wood useless: "You find that the wood is useless."
    WoodUseless,
    /// Fishing miss: fish slipped free or empty hook
    FishingMiss,
    /// Fish caught: "You reel in a/an {item}." — item is normalized (e.g. "Fish", "Mimic", "Sea Bass")
    FishCaught { item: String },
    /// Ranger reflect: "You have studied the following creatures:" header
    ReflectStudiedHeader,
    /// Character was untrained by Untrainus
    Untrained,
    /// Trainer rank checkpoint: trainer greeting with rank status message.
    /// character_name is who the greeting was addressed to (may differ from log owner).
    /// rank_max=None means the trainer is maxed.
    TrainerCheckpoint { trainer_name: String, character_name: String, rank_min: i64, rank_max: Option<i64> },
    /// Trainer simple greeting (bow sequence step 1): Trainer says, "Hail, Name."
    /// character_name is who the greeting was addressed to.
    TrainerGreetingSimple { trainer_name: String, character_name: String },
    /// Trainer bow (bow sequence step 2): "Trainer bows." or "Trainer bows deeply."
    TrainerBow { trainer_name: String },
    /// Trainer checkpoint message without greeting prefix (bow sequence step 3):
    /// Trainer says, "{known_checkpoint_message}" — character name comes from the preceding greeting.
    TrainerCheckpointUnhailed { trainer_name: String, rank_min: i64, rank_max: Option<i64> },
    /// TRAINER_GREETING matched (has rank text after "Hail, Name.") but rank message
    /// not found in checkpoint DB — likely a missing entry in rankmessages.
    TrainerGreetingWithUnknownCheckpoint {
        trainer_name: String,
        character_name: String,
        raw_message: String,
    },
    /// Line was not classified (speech, emote, or unrecognized)
    Ignored,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LootType {
    Fur,
    Blood,
    Mandible,
    Other,
}

/// A log line with its parsed timestamp and event.
#[derive(Debug, Clone)]
pub struct ParsedLine {
    pub timestamp: Option<NaiveDateTime>,
    pub event: LogEvent,
    pub raw: String,
}
