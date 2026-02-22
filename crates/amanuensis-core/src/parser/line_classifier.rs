use crate::data::TrainerDb;
use crate::parser::events::{KillVerb, LogEvent, LootType};
use crate::parser::patterns;

/// Classify a message body (after timestamp extraction) into a LogEvent.
pub fn classify_line(message: &str, trainer_db: &TrainerDb) -> LogEvent {
    // Skip empty lines
    if message.is_empty() {
        return LogEvent::Ignored;
    }

    // Karma messages look like speech but aren't — check before speech filter
    if let Some(caps) = patterns::KARMA_RECEIVED.captures(message) {
        return LogEvent::KarmaReceived {
            good: &caps[1] == "good",
        };
    }

    // Apply-learning bonus rank (NPC speech containing the confirmation)
    // Check "much more" (full) before "more" (partial) since "much more" contains "more"
    if let Some(caps) = patterns::APPLY_LEARNING_CONFIRM.captures(message) {
        return LogEvent::ApplyLearningRank {
            character_name: caps[1].to_string(),
            trainer_name: caps[2].to_string(),
            is_full: true,
        };
    }
    if let Some(caps) = patterns::APPLY_LEARNING_PARTIAL.captures(message) {
        return LogEvent::ApplyLearningRank {
            character_name: caps[1].to_string(),
            trainer_name: caps[2].to_string(),
            is_full: false,
        };
    }

    // Profession announcements (NPC speech — check before speech filter)
    if let Some(caps) = patterns::PROFESSION_CIRCLE_TEST.captures(message) {
        return LogEvent::ProfessionAnnouncement {
            name: caps[1].to_string(),
            profession: normalize_profession(&caps[2]),
        };
    }
    if let Some(caps) = patterns::PROFESSION_BECOME.captures(message) {
        return LogEvent::ProfessionAnnouncement {
            name: caps[1].to_string(),
            profession: normalize_profession(&caps[2]),
        };
    }

    // Untrainus completion (NPC speech — check before speech filter)
    if patterns::UNTRAINED.is_match(message) {
        return LogEvent::Untrained;
    }

    // Skip speech and emotes early (very common)
    if patterns::SPEECH.is_match(message) || patterns::EMOTE.is_match(message) {
        return LogEvent::Ignored;
    }

    // Handle ¥-prefixed lines (Mac client) and •-prefixed lines (Windows client)
    if message.starts_with('¥') || message.starts_with('•') {
        return classify_system_message(message, trainer_db);
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
            creature: strip_article(&caps[2]),
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
            creature: strip_article(&caps[2]),
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
            "mandible" | "mandibles" => LootType::Mandible,
            _ => LootType::Other,
        };
        return LogEvent::LootShare {
            item: caps[1].to_string(),
            worth: caps[3].parse().unwrap_or(0),
            amount: caps[4].parse().unwrap_or(0),
            loot_type,
        };
    }
    if let Some(caps) = patterns::SELF_RECOVERY.captures(message) {
        let loot_type = match &caps[2] {
            "fur" => LootType::Fur,
            "blood" => LootType::Blood,
            "mandible" | "mandibles" => LootType::Mandible,
            _ => LootType::Other,
        };
        let worth: i64 = caps[3].parse().unwrap_or(0);
        return LogEvent::LootShare {
            item: caps[1].to_string(),
            worth,
            amount: worth, // solo recovery: full value to player
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

    // Esteem gain (check before experience since it also starts with "* You gain")
    if patterns::ESTEEM_GAIN.is_match(message) {
        return LogEvent::EsteemGain;
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

/// Strip grammatical articles ("a ", "an ") from creature names but preserve "the " (boss creatures).
fn strip_article(name: &str) -> String {
    if let Some(rest) = name.strip_prefix("an ") {
        rest.to_string()
    } else if let Some(rest) = name.strip_prefix("a ") {
        rest.to_string()
    } else {
        name.to_string()
    }
}

/// Normalize a profession name from log text to canonical form.
fn normalize_profession(raw: &str) -> String {
    match raw.to_lowercase().as_str() {
        "fighter" => "Fighter".to_string(),
        "healer" => "Healer".to_string(),
        "mystic" => "Mystic".to_string(),
        "ranger" => "Ranger".to_string(),
        "bloodmage" => "Bloodmage".to_string(),
        "champion" => "Champion".to_string(),
        other => {
            // Title-case unknown profession
            let mut chars = other.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => {
                    let upper: String = first.to_uppercase().collect();
                    upper + chars.as_str()
                }
            }
        }
    }
}

/// Map study type names to lasty display types.
/// ways → Befriend, movements → Movements, essence → Morph
fn study_type_to_lasty(study_type: &str) -> String {
    match study_type {
        "ways" => "Befriend".to_string(),
        "movements" => "Movements".to_string(),
        "essence" => "Morph".to_string(),
        other => other.to_string(),
    }
}

/// Classify system-prefixed messages (¥ on Mac, • on Windows).
/// These can be trainer ranks, study messages, sun events, healing sense, etc.
fn classify_system_message(message: &str, trainer_db: &TrainerDb) -> LogEvent {
    // Strip the prefix character (¥ or •) and any surrounding whitespace
    let body = if message.starts_with('¥') {
        &message['¥'.len_utf8()..]
    } else {
        &message['•'.len_utf8()..]
    }
    .trim();

    // Check for study charge
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

    // Study abandon: "You abandon your study of the {creature}."
    if let Some(caps) = patterns::STUDY_ABANDON.captures(body) {
        return LogEvent::StudyAbandon {
            creature: caps[1].to_string(),
        };
    }

    // Lasty begin study pattern
    if let Some(caps) = patterns::LASTY_BEGIN_STUDY.captures(body) {
        return LogEvent::LastyBeginStudy {
            creature: caps[2].to_string(),
            lasty_type: study_type_to_lasty(&caps[1]),
        };
    }
    if let Some(caps) = patterns::LASTY_LEARN_PROGRESS.captures(body) {
        return LogEvent::LastyProgress {
            creature: caps[2].to_string(),
            lasty_type: study_type_to_lasty(&caps[1]),
        };
    }

    // Lasty finished patterns (before trainer lookup, since these are also ¥-prefixed)
    if let Some(caps) = patterns::LASTY_BEFRIEND.captures(body) {
        return LogEvent::LastyFinished {
            creature: caps[1].to_string(),
            lasty_type: "Befriend".to_string(),
        };
    }
    if let Some(caps) = patterns::LASTY_MORPH.captures(body) {
        return LogEvent::LastyFinished {
            creature: caps[1].to_string(),
            lasty_type: "Morph".to_string(),
        };
    }
    if let Some(caps) = patterns::LASTY_MOVEMENTS.captures(body) {
        return LogEvent::LastyFinished {
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
        let event = classify_line("Welcome to Clan Lord, Fen!", &db);
        assert!(matches!(event, LogEvent::Login { ref name } if name == "Fen"));
    }

    #[test]
    fn test_reconnect() {
        let db = test_db();
        let event = classify_line("Welcome back, pip!", &db);
        assert!(matches!(event, LogEvent::Reconnect { ref name } if name == "pip"));
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
        let event = classify_line("¥You sense healing energy from Fen.", &db);
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
            classify_line(r#"Fen says, "hello""#, &db),
            LogEvent::Ignored
        ));
    }

    #[test]
    fn test_emote_ignored() {
        let db = test_db();
        assert!(matches!(
            classify_line("(Fen waves)", &db),
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
            "* Fen recovers the Dark Vermine fur, worth 20c. Your share is 10c.",
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
        let event = classify_line("Fen has fallen to a Large Vermine.", &db);
        assert!(matches!(
            event,
            LogEvent::Fallen {
                ref name,
                ref cause
            } if name == "Fen" && cause == "Large Vermine"
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
        let event = classify_line("You start dragging Ava.", &db);
        assert!(matches!(
            event,
            LogEvent::ChainUsed { ref target } if target == "Ava"
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
            LogEvent::LastyFinished {
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
            LogEvent::LastyFinished {
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
            LogEvent::LastyFinished {
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

    #[test]
    fn test_lasty_movements_with_space() {
        // Real log format has a space after ¥
        let db = test_db();
        let event =
            classify_line("¥ You learn to fight the Purple Arachnoid more effectively.", &db);
        assert!(matches!(
            event,
            LogEvent::LastyFinished {
                ref creature,
                ref lasty_type
            } if creature == "Purple Arachnoid" && lasty_type == "Movements"
        ));
    }

    #[test]
    fn test_lasty_befriend_with_space() {
        let db = test_db();
        let event = classify_line("¥ You learn to befriend the Vermine.", &db);
        assert!(matches!(
            event,
            LogEvent::LastyFinished {
                ref creature,
                ref lasty_type
            } if creature == "Vermine" && lasty_type == "Befriend"
        ));
    }

    #[test]
    fn test_lasty_begin_study_movements() {
        let db = test_db();
        let event =
            classify_line("¥You begin studying the movements of the Darshak Liche.", &db);
        assert!(matches!(
            event,
            LogEvent::LastyBeginStudy {
                ref creature,
                ref lasty_type
            } if creature == "Darshak Liche" && lasty_type == "Movements"
        ));
    }

    #[test]
    fn test_lasty_begin_study_ways() {
        let db = test_db();
        let event =
            classify_line("¥You begin studying the ways of the Purple Arachnoid.", &db);
        assert!(matches!(
            event,
            LogEvent::LastyBeginStudy {
                ref creature,
                ref lasty_type
            } if creature == "Purple Arachnoid" && lasty_type == "Befriend"
        ));
    }

    #[test]
    fn test_lasty_learn_progress() {
        let db = test_db();
        let event = classify_line(
            "¥ You have almost nothing left to learn about the movements of the Vermine.",
            &db,
        );
        assert!(matches!(
            event,
            LogEvent::LastyProgress {
                ref creature,
                ref lasty_type
            } if creature == "Vermine" && lasty_type == "Movements"
        ));
    }

    #[test]
    fn test_trainer_rank_bullet_prefix() {
        // Windows client uses • (U+2022) instead of ¥ (U+00A5)
        let db = test_db();
        let event = classify_line("•You notice yourself dealing more damage.", &db);
        assert!(matches!(
            event,
            LogEvent::TrainerRank {
                ref trainer_name, ..
            } if trainer_name == "Darkus"
        ));
    }

    #[test]
    fn test_trainer_rank_bullet_with_space() {
        let db = test_db();
        let event = classify_line("• Your combat ability improves.", &db);
        assert!(matches!(
            event,
            LogEvent::TrainerRank {
                ref trainer_name, ..
            } if trainer_name == "Bangus Anmash"
        ));
    }

    #[test]
    fn test_bullet_study_gain_ignored() {
        let db = test_db();
        let event = classify_line("• You gain experience from your recent studies.", &db);
        assert!(matches!(event, LogEvent::Ignored));
    }

    #[test]
    fn test_study_abandon() {
        let db = test_db();
        let event = classify_line("¥You abandon your study of the Orga Anger.", &db);
        assert!(matches!(
            event,
            LogEvent::StudyAbandon { ref creature } if creature == "Orga Anger"
        ));
    }

    #[test]
    fn test_study_abandon_bullet() {
        let db = test_db();
        let event = classify_line("•You abandon your study of the Maha Ruknee.", &db);
        assert!(matches!(
            event,
            LogEvent::StudyAbandon { ref creature } if creature == "Maha Ruknee"
        ));
    }

    #[test]
    fn test_apply_learning_confirm() {
        let db = test_db();
        let event = classify_line(
            r#"Aitnos says, "Congratulations, Ajahn. You should now understand much more of Evus's teachings.""#,
            &db,
        );
        assert!(matches!(
            event,
            LogEvent::ApplyLearningRank { ref character_name, ref trainer_name, is_full: true }
                if character_name == "Ajahn" && trainer_name == "Evus"
        ));
    }

    #[test]
    fn test_apply_learning_partial() {
        let db = test_db();
        let event = classify_line(
            "Aitnos says, \"Congratulations, Ajahn. You should now understand more of Evus\u{2019}s teachings.\"",
            &db,
        );
        assert!(matches!(
            event,
            LogEvent::ApplyLearningRank { ref character_name, ref trainer_name, is_full: false }
                if character_name == "Ajahn" && trainer_name == "Evus"
        ));
    }

    #[test]
    fn test_apply_learning_offer_is_ignored() {
        // The offer is just an NPC prompt — we only act on the confirmation
        let db = test_db();
        let event = classify_line(
            r#"Aitnos says, "Would you like to apply some of your learning to Evus's lessons?""#,
            &db,
        );
        // Should be ignored (filtered as speech since we don't act on the offer)
        assert!(matches!(event, LogEvent::Ignored));
    }

    #[test]
    fn test_solo_kill_the_ramandu() {
        let db = test_db();
        let event = classify_line("You killed the Ramandu.", &db);
        assert!(matches!(
            event,
            LogEvent::SoloKill {
                ref creature,
                verb: KillVerb::Killed
            } if creature == "the Ramandu"
        ));
    }

    #[test]
    fn test_assisted_kill_the_ramandu() {
        let db = test_db();
        let event = classify_line("You helped vanquish the Ramandu.", &db);
        assert!(matches!(
            event,
            LogEvent::AssistedKill {
                ref creature,
                verb: KillVerb::Vanquished
            } if creature == "the Ramandu"
        ));
    }

    #[test]
    fn test_solo_kill_strips_a_article() {
        let db = test_db();
        let event = classify_line("You slaughtered a Ramandu.", &db);
        assert!(matches!(
            event,
            LogEvent::SoloKill {
                ref creature,
                verb: KillVerb::Slaughtered
            } if creature == "Ramandu"
        ));
    }

    #[test]
    fn test_karma_good() {
        let db = test_db();
        let event = classify_line("You just received good karma from Fen.", &db);
        assert!(matches!(event, LogEvent::KarmaReceived { good: true }));
    }

    #[test]
    fn test_karma_bad() {
        let db = test_db();
        let event = classify_line("You just received bad karma from Troll.", &db);
        assert!(matches!(event, LogEvent::KarmaReceived { good: false }));
    }

    #[test]
    fn test_karma_anonymous() {
        let db = test_db();
        let event = classify_line("You just received anonymous good karma.", &db);
        assert!(matches!(event, LogEvent::KarmaReceived { good: true }));
    }

    #[test]
    fn test_profession_circle_test_fighter() {
        let db = test_db();
        let event = classify_line(
            r#"Honor thinks, "Congratulations go out to Camo, who has just passed the seventh circle fighter test.""#,
            &db,
        );
        assert!(matches!(
            event,
            LogEvent::ProfessionAnnouncement { ref name, ref profession }
            if name == "Camo" && profession == "Fighter"
        ));
    }

    #[test]
    fn test_profession_circle_test_healer() {
        let db = test_db();
        let event = classify_line(
            r#"Glory thinks, "Congratulations go out to Squib, who has just passed the sixth circle healer test.""#,
            &db,
        );
        assert!(matches!(
            event,
            LogEvent::ProfessionAnnouncement { ref name, ref profession }
            if name == "Squib" && profession == "Healer"
        ));
    }

    #[test]
    fn test_profession_become_bloodmage() {
        let db = test_db();
        let event = classify_line(
            r#"Haima Myrtillus thinks, "Congratulations to Kargan, who has just become a Bloodmage.""#,
            &db,
        );
        assert!(matches!(
            event,
            LogEvent::ProfessionAnnouncement { ref name, ref profession }
            if name == "Kargan" && profession == "Bloodmage"
        ));
    }

    #[test]
    fn test_untrained() {
        let db = test_db();
        let event = classify_line(
            r#"Untrainus says, "Squib, your mind is less cluttered now.""#,
            &db,
        );
        assert!(matches!(event, LogEvent::Untrained));
    }

    #[test]
    fn test_untrained_greeting_ignored() {
        let db = test_db();
        let event = classify_line(
            r#"Untrainus says, "Greetings, Lord Squib.""#,
            &db,
        );
        assert!(matches!(event, LogEvent::Ignored));
    }

    #[test]
    fn test_untrained_question_ignored() {
        let db = test_db();
        let event = classify_line(
            r#"Untrainus asks, "Squib, are you certain you wish to undertake this irrevocable step?""#,
            &db,
        );
        assert!(matches!(event, LogEvent::Ignored));
    }

    #[test]
    fn test_esteem_gain() {
        let db = test_db();
        assert!(matches!(
            classify_line("* You gain esteem.", &db),
            LogEvent::EsteemGain
        ));
        assert!(matches!(
            classify_line("* You gain experience and esteem.", &db),
            LogEvent::EsteemGain
        ));
    }

    #[test]
    fn test_loot_share_with_worth() {
        let db = test_db();
        let event = classify_line(
            "* Fen recovers the Dark Vermine fur, worth 20c. Your share is 10c.",
            &db,
        );
        assert!(matches!(
            event,
            LogEvent::LootShare {
                worth: 20,
                amount: 10,
                loot_type: LootType::Fur,
                ..
            }
        ));
    }

    #[test]
    fn test_loot_share_mandibles_plural() {
        let db = test_db();
        let event = classify_line(
            "* You recover the Noble Myrm mandibles, worth 2c. Your share is 1c.",
            &db,
        );
        assert!(matches!(
            event,
            LogEvent::LootShare {
                worth: 2,
                amount: 1,
                loot_type: LootType::Mandible,
                ..
            }
        ));
    }

    #[test]
    fn test_self_recovery_fur() {
        let db = test_db();
        let event = classify_line(
            "* You recover the Dark Vermine fur, worth 20c.",
            &db,
        );
        assert!(matches!(
            event,
            LogEvent::LootShare {
                worth: 20,
                amount: 20,
                loot_type: LootType::Fur,
                ..
            }
        ));
    }

    #[test]
    fn test_self_recovery_mandibles() {
        let db = test_db();
        let event = classify_line(
            "* You recover the Noble Myrm mandibles, worth 2c.",
            &db,
        );
        assert!(matches!(
            event,
            LogEvent::LootShare {
                worth: 2,
                amount: 2,
                loot_type: LootType::Mandible,
                ..
            }
        ));
    }
}
