use crate::data::TrainerDb;
use crate::parser::events::{KillVerb, LogEvent, LootType};
use crate::parser::patterns;

/// Classify a message body (after timestamp extraction) into a LogEvent.
pub fn classify_line(message: &str, trainer_db: &TrainerDb) -> LogEvent {
    // Skip empty lines
    if message.is_empty() {
        return LogEvent::Ignored;
    }

    // Skip speech and emotes early (very common)
    if patterns::SPEECH.is_match(message) || patterns::EMOTE.is_match(message) {
        return LogEvent::Ignored;
    }

    // Handle ¥-prefixed lines
    if message.starts_with('¥') {
        return classify_yen_message(message, trainer_db);
    }

    // Welcome messages
    if let Some(caps) = patterns::WELCOME_LOGIN.captures(message) {
        return LogEvent::Login {
            name: caps[1].to_string(),
        };
    }
    if let Some(caps) = patterns::WELCOME_BACK.captures(message) {
        return LogEvent::Reconnect {
            name: caps[1].to_string(),
        };
    }

    // Kill patterns
    if let Some(caps) = patterns::SOLO_KILL.captures(message) {
        let verb = match &caps[1] {
            "killed" => KillVerb::Killed,
            "slaughtered" => KillVerb::Slaughtered,
            "vanquished" => KillVerb::Vanquished,
            "dispatched" => KillVerb::Dispatched,
            _ => unreachable!(),
        };
        return LogEvent::SoloKill {
            creature: caps[2].to_string(),
            verb,
        };
    }
    if let Some(caps) = patterns::ASSISTED_KILL.captures(message) {
        let verb = match &caps[1] {
            "kill" => KillVerb::Killed,
            "slaughter" => KillVerb::Slaughtered,
            "vanquish" => KillVerb::Vanquished,
            "dispatch" => KillVerb::Dispatched,
            _ => unreachable!(),
        };
        return LogEvent::AssistedKill {
            creature: caps[2].to_string(),
            verb,
        };
    }

    // Death patterns
    if let Some(caps) = patterns::FALLEN.captures(message) {
        return LogEvent::Fallen {
            name: caps[1].to_string(),
            cause: caps[2].to_string(),
        };
    }
    if let Some(caps) = patterns::RECOVERED.captures(message) {
        return LogEvent::Recovered {
            name: caps[1].to_string(),
        };
    }
    if patterns::FIRST_DEPART.is_match(message) {
        return LogEvent::FirstDepart;
    }
    if let Some(caps) = patterns::DEPART_COUNT.captures(message) {
        let count: i64 = caps[1].parse().unwrap_or(0);
        return LogEvent::Depart { count };
    }

    // Coin patterns
    if let Some(caps) = patterns::COINS_PICKED_UP.captures(message) {
        let amount: i64 = caps[1].parse().unwrap_or(0);
        return LogEvent::CoinsPickedUp { amount };
    }
    if let Some(caps) = patterns::COIN_BALANCE.captures(message) {
        let amount: i64 = caps[1].parse().unwrap_or(0);
        return LogEvent::CoinBalance { amount };
    }
    if let Some(caps) = patterns::LOOT_SHARE.captures(message) {
        let loot_type = match &caps[2] {
            "fur" => LootType::Fur,
            "blood" => LootType::Blood,
            "mandible" => LootType::Mandible,
            _ => LootType::Other,
        };
        return LogEvent::LootShare {
            item: caps[1].to_string(),
            amount: caps[3].parse().unwrap_or(0),
            loot_type,
        };
    }

    // Equipment patterns
    if patterns::BELL_BROKEN.is_match(message) {
        return LogEvent::BellBroken;
    }
    if patterns::BELL_USED.is_match(message) {
        return LogEvent::BellUsed;
    }
    if patterns::CHAIN_BREAK.is_match(message) {
        return LogEvent::ChainBreak;
    }
    if patterns::CHAIN_SHATTER.is_match(message) {
        return LogEvent::ChainShatter;
    }
    if patterns::CHAIN_SNAP.is_match(message) {
        return LogEvent::ChainSnap;
    }
    if let Some(caps) = patterns::CHAIN_DRAG.captures(message) {
        return LogEvent::ChainUsed {
            target: caps[1].to_string(),
        };
    }
    if patterns::SHIELDSTONE_USED.is_match(message) {
        return LogEvent::ShieldstoneUsed;
    }
    if patterns::SHIELDSTONE_BROKEN.is_match(message) {
        return LogEvent::ShieldstoneBroken;
    }
    if patterns::ETHEREAL_PORTAL.is_match(message) {
        return LogEvent::EtherealPortalOpened;
    }
    if patterns::ETHEREAL_STONE_USED.is_match(message) {
        return LogEvent::EtherealPortalStoneUsed;
    }

    // Experience gain
    if patterns::EXPERIENCE_GAIN.is_match(message) {
        return LogEvent::ExperienceGain;
    }

    // Clanning
    if let Some(caps) = patterns::CLANNING_ON.captures(message) {
        return LogEvent::ClanningChange {
            name: caps[1].to_string(),
            is_clanning: true,
        };
    }
    if let Some(caps) = patterns::CLANNING_OFF.captures(message) {
        return LogEvent::ClanningChange {
            name: caps[1].to_string(),
            is_clanning: false,
        };
    }

    // Disconnect
    if patterns::DISCONNECT.is_match(message) {
        return LogEvent::Disconnect;
    }

    LogEvent::Ignored
}

/// Classify ¥-prefixed messages. These can be trainer ranks, study messages,
/// sun events, healing sense, etc.
fn classify_yen_message(message: &str, trainer_db: &TrainerDb) -> LogEvent {
    // Strip the ¥ prefix
    let body = &message['\u{00a5}'.len_utf8()..];

    // Check for study charge (note: has space after ¥)
    if let Some(caps) = patterns::STUDY_CHARGE.captures(body) {
        let amount: i64 = caps[1].parse().unwrap_or(0);
        return LogEvent::StudyCharge { amount };
    }

    // Check for study progress
    if let Some(caps) = patterns::STUDY_PROGRESS.captures(body) {
        return LogEvent::StudyProgress {
            creature: caps[1].to_string(),
            progress: caps[2].to_string(),
        };
    }

    // Lasty patterns (before trainer lookup, since these are also ¥-prefixed)
    if let Some(caps) = patterns::LASTY_BEFRIEND.captures(body) {
        return LogEvent::LastyProgress {
            creature: caps[1].to_string(),
            lasty_type: "Befriend".to_string(),
        };
    }
    if let Some(caps) = patterns::LASTY_MORPH.captures(body) {
        return LogEvent::LastyProgress {
            creature: caps[1].to_string(),
            lasty_type: "Morph".to_string(),
        };
    }
    if let Some(caps) = patterns::LASTY_MOVEMENTS.captures(body) {
        return LogEvent::LastyProgress {
            creature: caps[1].to_string(),
            lasty_type: "Movements".to_string(),
        };
    }
    if let Some(caps) = patterns::LASTY_COMPLETED.captures(body) {
        return LogEvent::LastyCompleted {
            trainer: caps[1].to_string(),
        };
    }

    // Skip known non-trainer ¥ messages
    if patterns::YEN_HEALING_SENSE.is_match(body)
        || patterns::YEN_SUN_EVENT.is_match(body)
        || patterns::YEN_STUDY_GAIN.is_match(body)
        || patterns::YEN_STUDY_CONCURRENT.is_match(body)
    {
        return LogEvent::Ignored;
    }

    // Try trainer lookup
    if let Some(trainer_name) = trainer_db.get_trainer(body) {
        return LogEvent::TrainerRank {
            trainer_name: trainer_name.to_string(),
            message: body.to_string(),
        };
    }

    // Unknown ¥ message — ignore
    LogEvent::Ignored
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_db() -> TrainerDb {
        TrainerDb::bundled().unwrap()
    }

    #[test]
    fn test_solo_kill() {
        let db = test_db();
        let event = classify_line("You slaughtered a Rat.", &db);
        assert!(matches!(
            event,
            LogEvent::SoloKill {
                ref creature,
                verb: KillVerb::Slaughtered
            } if creature == "Rat"
        ));
    }

    #[test]
    fn test_solo_kill_an() {
        let db = test_db();
        let event = classify_line("You slaughtered an Orga Anger.", &db);
        assert!(matches!(
            event,
            LogEvent::SoloKill {
                ref creature,
                verb: KillVerb::Slaughtered
            } if creature == "Orga Anger"
        ));
    }

    #[test]
    fn test_assisted_kill() {
        let db = test_db();
        let event = classify_line("You helped vanquish a Greater Death.", &db);
        assert!(matches!(
            event,
            LogEvent::AssistedKill {
                ref creature,
                verb: KillVerb::Vanquished
            } if creature == "Greater Death"
        ));
    }

    #[test]
    fn test_login() {
        let db = test_db();
        let event = classify_line("Welcome to Clan Lord, Ruuk!", &db);
        assert!(matches!(event, LogEvent::Login { ref name } if name == "Ruuk"));
    }

    #[test]
    fn test_reconnect() {
        let db = test_db();
        let event = classify_line("Welcome back, squib!", &db);
        assert!(matches!(event, LogEvent::Reconnect { ref name } if name == "squib"));
    }

    #[test]
    fn test_trainer_rank() {
        let db = test_db();
        let event = classify_line("¥Your combat ability improves.", &db);
        assert!(matches!(
            event,
            LogEvent::TrainerRank {
                ref trainer_name, ..
            } if trainer_name == "Bangus Anmash"
        ));
    }

    #[test]
    fn test_trainer_regia() {
        let db = test_db();
        let event = classify_line("¥You notice your balance recovering more quickly.", &db);
        assert!(matches!(
            event,
            LogEvent::TrainerRank {
                ref trainer_name, ..
            } if trainer_name == "Regia"
        ));
    }

    #[test]
    fn test_yen_healing_sense_ignored() {
        let db = test_db();
        let event = classify_line("¥You sense healing energy from Ruuk.", &db);
        assert!(matches!(event, LogEvent::Ignored));
    }

    #[test]
    fn test_yen_sun_event_ignored() {
        let db = test_db();
        let event = classify_line("¥The Sun rises.", &db);
        assert!(matches!(event, LogEvent::Ignored));
    }

    #[test]
    fn test_study_charge() {
        let db = test_db();
        let event = classify_line("¥ You have been charged 100 coins for advanced studies.", &db);
        assert!(matches!(event, LogEvent::StudyCharge { amount: 100 }));
    }

    #[test]
    fn test_study_progress() {
        let db = test_db();
        let event = classify_line(
            "¥You are currently studying the Rat, and have almost nothing left to learn.",
            &db,
        );
        assert!(matches!(
            event,
            LogEvent::StudyProgress {
                ref creature,
                ref progress
            } if creature == "Rat" && progress == "almost nothing"
        ));
    }

    #[test]
    fn test_speech_ignored() {
        let db = test_db();
        assert!(matches!(
            classify_line(r#"Donk thinks, "south""#, &db),
            LogEvent::Ignored
        ));
        assert!(matches!(
            classify_line(r#"Ruuk says, "hello""#, &db),
            LogEvent::Ignored
        ));
    }

    #[test]
    fn test_emote_ignored() {
        let db = test_db();
        assert!(matches!(
            classify_line("(Ruuk waves)", &db),
            LogEvent::Ignored
        ));
    }

    #[test]
    fn test_coin_balance() {
        let db = test_db();
        let event = classify_line("You have 101 coins.", &db);
        assert!(matches!(event, LogEvent::CoinBalance { amount: 101 }));
    }

    #[test]
    fn test_coins_picked_up() {
        let db = test_db();
        let event = classify_line("* You pick up 50 coins.", &db);
        assert!(matches!(event, LogEvent::CoinsPickedUp { amount: 50 }));
    }

    #[test]
    fn test_loot_share() {
        let db = test_db();
        let event = classify_line(
            "* Ruuk recovers the Dark Vermine fur, worth 20c. Your share is 10c.",
            &db,
        );
        assert!(matches!(
            event,
            LogEvent::LootShare {
                amount: 10,
                loot_type: LootType::Fur,
                ..
            }
        ));
    }

    #[test]
    fn test_fallen() {
        let db = test_db();
        let event = classify_line("Ruuk has fallen to a Large Vermine.", &db);
        assert!(matches!(
            event,
            LogEvent::Fallen {
                ref name,
                ref cause
            } if name == "Ruuk" && cause == "Large Vermine"
        ));
    }

    #[test]
    fn test_first_depart() {
        let db = test_db();
        let event = classify_line(
            "This is the first time your spirit has departed your body.",
            &db,
        );
        assert!(matches!(event, LogEvent::FirstDepart));
    }

    #[test]
    fn test_depart_count() {
        let db = test_db();
        let event = classify_line("Your spirit has departed your body 42 times.", &db);
        assert!(matches!(event, LogEvent::Depart { count: 42 }));
    }

    #[test]
    fn test_disconnect() {
        let db = test_db();
        let event = classify_line(
            "*** We are no longer connected to the Clan Lord game server. ***",
            &db,
        );
        assert!(matches!(event, LogEvent::Disconnect));
    }

    #[test]
    fn test_clanning() {
        let db = test_db();
        let event = classify_line("Borzon is now Clanning.", &db);
        assert!(matches!(
            event,
            LogEvent::ClanningChange {
                ref name,
                is_clanning: true
            } if name == "Borzon"
        ));
    }

    #[test]
    fn test_experience_gain() {
        let db = test_db();
        assert!(matches!(
            classify_line("* You grow more mindful.", &db),
            LogEvent::ExperienceGain
        ));
        assert!(matches!(
            classify_line("* You gain experience.", &db),
            LogEvent::ExperienceGain
        ));
    }

    #[test]
    fn test_yen_study_gain_ignored() {
        let db = test_db();
        let event = classify_line("¥ You gain experience from your adventures.", &db);
        assert!(matches!(event, LogEvent::Ignored));
    }

    #[test]
    fn test_bell_broken() {
        let db = test_db();
        assert!(matches!(
            classify_line("* Your bell crumbles to dust.", &db),
            LogEvent::BellBroken
        ));
    }

    #[test]
    fn test_chain_break() {
        let db = test_db();
        assert!(matches!(
            classify_line("Your chain breaks as you try to use it.", &db),
            LogEvent::ChainBreak
        ));
    }

    #[test]
    fn test_chain_used() {
        let db = test_db();
        let event = classify_line("You start dragging Olga.", &db);
        assert!(matches!(
            event,
            LogEvent::ChainUsed { ref target } if target == "Olga"
        ));
    }

    #[test]
    fn test_shieldstone() {
        let db = test_db();
        assert!(matches!(
            classify_line("* You activate your shieldstone.", &db),
            LogEvent::ShieldstoneUsed
        ));
        assert!(matches!(
            classify_line("Your Shieldstone goes inert.", &db),
            LogEvent::ShieldstoneBroken
        ));
    }

    #[test]
    fn test_lasty_befriend() {
        let db = test_db();
        let event = classify_line("¥You learn to befriend the Maha Ruknee.", &db);
        assert!(matches!(
            event,
            LogEvent::LastyProgress {
                ref creature,
                ref lasty_type
            } if creature == "Maha Ruknee" && lasty_type == "Befriend"
        ));
    }

    #[test]
    fn test_lasty_morph() {
        let db = test_db();
        let event = classify_line("¥You learn to assume the form of the Orga Anger.", &db);
        assert!(matches!(
            event,
            LogEvent::LastyProgress {
                ref creature,
                ref lasty_type
            } if creature == "Orga Anger" && lasty_type == "Morph"
        ));
    }

    #[test]
    fn test_lasty_movements() {
        let db = test_db();
        let event = classify_line("¥You learn to fight the Large Vermine more effectively.", &db);
        assert!(matches!(
            event,
            LogEvent::LastyProgress {
                ref creature,
                ref lasty_type
            } if creature == "Large Vermine" && lasty_type == "Movements"
        ));
    }

    #[test]
    fn test_lasty_completed() {
        let db = test_db();
        let event = classify_line("¥You have completed your training with Sespus.", &db);
        assert!(matches!(
            event,
            LogEvent::LastyCompleted { ref trainer } if trainer == "Sespus"
        ));
    }
}
