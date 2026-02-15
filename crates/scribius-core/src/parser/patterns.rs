use once_cell::sync::Lazy;
use regex::Regex;

// === Character detection ===
pub static WELCOME_LOGIN: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^Welcome to Clan Lord, (.+)!$").unwrap());
pub static WELCOME_BACK: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^Welcome back, (.+)!$").unwrap());

// === Kill patterns ===
// Solo: "You slaughtered a/an {creature}."
pub static SOLO_KILL: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^You (killed|slaughtered|vanquished|dispatched) an? (.+)\.$").unwrap());
// Assisted: "You helped kill/slaughter/vanquish/dispatch a/an {creature}."
pub static ASSISTED_KILL: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^You helped (kill|slaughter|vanquish|dispatch) an? (.+)\.$").unwrap());

// === Death/fall patterns ===
pub static FALLEN: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^(.+) has fallen to an? (.+)\.$").unwrap());
pub static RECOVERED: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^(.+) is no longer fallen\.$").unwrap());
pub static FIRST_DEPART: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^This is the first time your spirit has departed your body\.$").unwrap());
pub static DEPART_COUNT: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^Your spirit has departed your body (\d+) times?\.$").unwrap());

// === Coin patterns ===
pub static COINS_PICKED_UP: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^\* You pick up (\d+) coins?\.$").unwrap());
pub static COIN_BALANCE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^You have (\d+) coins?\.$").unwrap());
// Loot: "* {name} recovers the {item} fur/blood, worth Nc. Your share is Nc."
// Also: "* You recover the {item} fur/blood, worth Nc. Your share is Nc."
pub static LOOT_SHARE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^\* (?:.+) recovers? the (.+) (fur|blood|mandible), worth \d+c\. Your share is (\d+)c\.$").unwrap());

// === Equipment patterns ===
pub static BELL_BROKEN: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^\* Your bell crumbles to dust\.$").unwrap());
pub static BELL_USED: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^\* The bell rings soundlessly into the void, summoning").unwrap());
pub static CHAIN_BREAK: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^Your chain breaks as you try to use it\.$").unwrap());
pub static CHAIN_SHATTER: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^A link in your chain shatters\.$").unwrap());
pub static CHAIN_SNAP: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^Your chain snaps as you try to use it\.$").unwrap());
pub static CHAIN_DRAG: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^You start dragging (.+)\.$").unwrap());
pub static SHIELDSTONE_USED: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^\* You activate your shieldstone\.$").unwrap());
pub static SHIELDSTONE_BROKEN: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^Your Shieldstone goes inert\.$").unwrap());
pub static ETHEREAL_PORTAL: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^You open an ethereal portal\.$").unwrap());
pub static ETHEREAL_STONE_USED: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^Your ethereal portal stone disappears into the ether\.$").unwrap());

// === Speech/emote patterns to skip ===
pub static SPEECH: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"^.+ (says|exclaims|yells|ponders|thinks|asks), ""#).unwrap());
pub static EMOTE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^\(.+ .+\)$").unwrap());

// === Clanning ===
pub static CLANNING_ON: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^(.+) is now Clanning\.$").unwrap());
pub static CLANNING_OFF: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^(.+) is no longer Clanning\.$").unwrap());

// === Study messages (¥-prefixed, not trainer ranks) ===
pub static STUDY_PROGRESS: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^You are (?:currently studying|remembering your studies of) the (.+), and have (.+) left to learn\.$").unwrap());
pub static STUDY_CHARGE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^ You have been charged (\d+) coins? for advanced studies\.$").unwrap());

// === Disconnect ===
pub static DISCONNECT: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^\*\*\* We are no longer connected to the Clan Lord game server\. \*\*\*$").unwrap());

// === Experience ===
pub static EXPERIENCE_GAIN: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^\* You (grow more mindful|gain experience|gain morale)").unwrap());

// === Lasty patterns (¥-prefixed) ===
// "You learn to befriend the {creature}." → Befriend lasty + pet
pub static LASTY_BEFRIEND: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^You learn to befriend the (.+)\.$").unwrap());
// "You learn to assume the form of the {creature}." → Morph lasty
pub static LASTY_MORPH: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^You learn to assume the form of the (.+)\.$").unwrap());
// "You learn to fight the {creature} more effectively." → Movements lasty
pub static LASTY_MOVEMENTS: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^You learn to fight the (.+) more effectively\.$").unwrap());
// "You have completed your training with {trainer}." → Lasty completed
pub static LASTY_COMPLETED: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^You have completed your training with (.+)\.$").unwrap());

// === ¥-prefixed lines to skip (not trainer ranks) ===
pub static YEN_HEALING_SENSE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^You sense healing energy from .+\.$").unwrap());
pub static YEN_SUN_EVENT: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^The Sun (rises|sets)\.$").unwrap());
pub static YEN_STUDY_GAIN: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^ You gain experience from your").unwrap());
pub static YEN_STUDY_CONCURRENT: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^You can study up to \d+ creatures? concurrently\.$").unwrap());

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_welcome_login() {
        let caps = WELCOME_LOGIN.captures("Welcome to Clan Lord, Ruuk!").unwrap();
        assert_eq!(&caps[1], "Ruuk");
    }

    #[test]
    fn test_welcome_back() {
        let caps = WELCOME_BACK.captures("Welcome back, squib!").unwrap();
        assert_eq!(&caps[1], "squib");
    }

    #[test]
    fn test_solo_kill_slaughtered() {
        let caps = SOLO_KILL.captures("You slaughtered a Rat.").unwrap();
        assert_eq!(&caps[1], "slaughtered");
        assert_eq!(&caps[2], "Rat");
    }

    #[test]
    fn test_solo_kill_with_an() {
        let caps = SOLO_KILL.captures("You slaughtered an Orga Anger.").unwrap();
        assert_eq!(&caps[2], "Orga Anger");
    }

    #[test]
    fn test_assisted_kill() {
        let caps = ASSISTED_KILL.captures("You helped vanquish a Greater Death.").unwrap();
        assert_eq!(&caps[1], "vanquish");
        assert_eq!(&caps[2], "Greater Death");
    }

    #[test]
    fn test_fallen() {
        let caps = FALLEN.captures("Ruuk has fallen to a Large Vermine.").unwrap();
        assert_eq!(&caps[1], "Ruuk");
        assert_eq!(&caps[2], "Large Vermine");
    }

    #[test]
    fn test_fallen_acid() {
        let caps = FALLEN.captures("Ruuk has fallen to a spray of acid.").unwrap();
        assert_eq!(&caps[2], "spray of acid");
    }

    #[test]
    fn test_coins_picked_up() {
        let caps = COINS_PICKED_UP.captures("* You pick up 50 coins.").unwrap();
        assert_eq!(&caps[1], "50");
    }

    #[test]
    fn test_coin_balance() {
        let caps = COIN_BALANCE.captures("You have 101 coins.").unwrap();
        assert_eq!(&caps[1], "101");
    }

    #[test]
    fn test_loot_share_fur() {
        let caps = LOOT_SHARE.captures("* Ruuk recovers the Dark Vermine fur, worth 20c. Your share is 10c.").unwrap();
        assert_eq!(&caps[1], "Dark Vermine");
        assert_eq!(&caps[2], "fur");
        assert_eq!(&caps[3], "10");
    }

    #[test]
    fn test_loot_share_blood() {
        let caps = LOOT_SHARE.captures("* squib recovers the Orga blood, worth 30c. Your share is 15c.").unwrap();
        assert_eq!(&caps[2], "blood");
        assert_eq!(&caps[3], "15");
    }

    #[test]
    fn test_chain_drag() {
        let caps = CHAIN_DRAG.captures("You start dragging Olga.").unwrap();
        assert_eq!(&caps[1], "Olga");
    }

    #[test]
    fn test_speech_skip() {
        assert!(SPEECH.is_match(r#"Donk thinks, "south""#));
        assert!(SPEECH.is_match(r#"Ruuk says, "hello""#));
        assert!(SPEECH.is_match(r#"Ruuk yells, "help!""#));
    }

    #[test]
    fn test_emote_skip() {
        assert!(EMOTE.is_match("(Ruuk waves)"));
    }

    #[test]
    fn test_disconnect() {
        assert!(DISCONNECT.is_match("*** We are no longer connected to the Clan Lord game server. ***"));
    }

    #[test]
    fn test_depart_count() {
        let caps = DEPART_COUNT.captures("Your spirit has departed your body 42 times.").unwrap();
        assert_eq!(&caps[1], "42");
    }

    #[test]
    fn test_study_charge() {
        let caps = STUDY_CHARGE.captures(" You have been charged 100 coins for advanced studies.").unwrap();
        assert_eq!(&caps[1], "100");
    }

    #[test]
    fn test_lasty_befriend() {
        let caps = LASTY_BEFRIEND.captures("You learn to befriend the Maha Ruknee.").unwrap();
        assert_eq!(&caps[1], "Maha Ruknee");
    }

    #[test]
    fn test_lasty_morph() {
        let caps = LASTY_MORPH.captures("You learn to assume the form of the Orga Anger.").unwrap();
        assert_eq!(&caps[1], "Orga Anger");
    }

    #[test]
    fn test_lasty_movements() {
        let caps = LASTY_MOVEMENTS.captures("You learn to fight the Large Vermine more effectively.").unwrap();
        assert_eq!(&caps[1], "Large Vermine");
    }

    #[test]
    fn test_lasty_completed() {
        let caps = LASTY_COMPLETED.captures("You have completed your training with Sespus.").unwrap();
        assert_eq!(&caps[1], "Sespus");
    }
}
