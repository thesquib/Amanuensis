//! Behavioural detection of player characters.
//!
//! Player-run trainers ("ledger holders") greet with the *exact* same wording as NPC
//! trainers, so their greetings are indistinguishable in isolation. What distinguishes
//! them is everything else they do: players clan, share experiences, think to you, hold
//! training ledgers, and make teacher offers. NPCs never do any of that.
//!
//! Deliberately separate from `line_classifier`: these signals are orthogonal to the
//! `LogEvent` taxonomy (several fire on lines the classifier already handles, such as
//! clanning, or deliberately discards, such as speech), and keeping them standalone makes
//! the ten patterns testable in isolation.

use once_cell::sync::Lazy;
use regex::Regex;

/// How a name was identified as a player. Ordered weakest to strongest by `rank()`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerSignal {
    Clanning,
    Sharing,
    Thinks,
    Ledger,
    Offer,
}

impl PlayerSignal {
    /// Stable string stored in `known_players.evidence`.
    pub fn as_str(&self) -> &'static str {
        match self {
            PlayerSignal::Clanning => "clanning",
            PlayerSignal::Sharing => "sharing",
            PlayerSignal::Thinks => "thinks",
            PlayerSignal::Ledger => "ledger",
            PlayerSignal::Offer => "offer",
        }
    }

    /// Precedence — a stronger signal overwrites a weaker one in `known_players`.
    pub fn rank(&self) -> u8 {
        match self {
            PlayerSignal::Clanning => 0,
            PlayerSignal::Sharing => 1,
            PlayerSignal::Thinks => 2,
            PlayerSignal::Offer => 3,
            PlayerSignal::Ledger => 4,
        }
    }
}

// "Fenwick is now Clanning." / "Fenwick is no longer Clanning."
static CLANNING: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^(.+?) is (?:now|no longer) Clanning\.$").expect("regex compile error"));

// "Fenwick is sharing experiences with you."
static SHARING_WITH_YOU: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^(.+?) is sharing experiences with you\.$").expect("regex compile error")
});

// "You begin sharing your experiences with Fenwick."
static BEGIN_SHARING: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^You begin sharing your experiences with (.+?)\.$").expect("regex compile error")
});

// "You are sharing experiences with Fenwick, Halloway and Brindle."
// Anchored on "You are sharing" so "You are no longer sharing ..." cannot match.
static SHARE_LIST: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^You are sharing experiences with (.+?)\.$").expect("regex compile error")
});

// Fenwick thinks to you, "..."
static THINKS_TO_YOU: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"^(.+?) thinks to you, ""#).expect("regex compile error"));

// "Fenwick writes Thistledown's name in her training ledger."
// "Bramwell Gorse writes your name in her training ledger."
static LEDGER_WRITES: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^(.+?) writes .+? in (?:his|her|their) training ledger\.$")
        .expect("regex compile error")
});

// "Rushlight shows his training ledger to you." / "... to Vetch."
static LEDGER_SHOWS: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^(.+?) shows (?:his|her|their) training ledger to ").expect("regex compile error")
});

// "¥ You begin training with Bramwell Gorse." — only the player-teacher flow emits this.
static BEGIN_TRAINING_WITH: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^[¥•]?\s*You begin training with (.+?)\.$").expect("regex compile error")
});

// ¥ Bramwell Gorse offers: “I can teach you to be more receptive to healing.”
// Real logs use curly quotes; straight quotes accepted for robustness.
static TEACHER_OFFER: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"^[¥•]\s*(.+?) offers: [\u{201C}"]I can teach you"#).expect("regex compile error")
});

// ¥ To accept her offer, say: I accept Bramwell Gorse as my teacher.
static TEACHER_ACCEPT: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^[¥•]\s*To accept (?:his|her|their) offer, say: I accept (.+?) as my teacher\.$")
        .expect("regex compile error")
});

/// Append every `(player_name, signal)` pair found in `message` to `out`.
///
/// `message` must already have its log timestamp stripped. `out` is a caller-owned scratch
/// buffer and is **not** cleared, so the caller can reuse one allocation across a whole file.
pub fn detect_player_signals(message: &str, out: &mut Vec<(String, PlayerSignal)>) {
    // Fast bail-out: every pattern below contains one of these substrings. Log files are
    // overwhelmingly lines that match none of them, so this keeps the regex work rare.
    if !(message.contains("Clanning")
        || message.contains("sharing")
        || message.contains("thinks to you")
        || message.contains("training ledger")
        || message.contains("offers:")
        || message.contains("as my teacher")
        || message.contains("begin training with"))
    {
        return;
    }

    let mut push = |name: &str, signal: PlayerSignal| {
        let trimmed = name.trim();
        if !trimmed.is_empty() {
            out.push((trimmed.to_string(), signal));
        }
    };

    if let Some(caps) = CLANNING.captures(message) {
        push(&caps[1], PlayerSignal::Clanning);
        return;
    }
    if let Some(caps) = SHARING_WITH_YOU.captures(message) {
        push(&caps[1], PlayerSignal::Sharing);
        return;
    }
    if let Some(caps) = BEGIN_SHARING.captures(message) {
        push(&caps[1], PlayerSignal::Sharing);
        return;
    }
    if let Some(caps) = SHARE_LIST.captures(message) {
        for part in split_name_list(&caps[1]) {
            push(part, PlayerSignal::Sharing);
        }
        return;
    }
    if let Some(caps) = THINKS_TO_YOU.captures(message) {
        push(&caps[1], PlayerSignal::Thinks);
        return;
    }
    if let Some(caps) = LEDGER_WRITES.captures(message) {
        push(&caps[1], PlayerSignal::Ledger);
        return;
    }
    if let Some(caps) = LEDGER_SHOWS.captures(message) {
        push(&caps[1], PlayerSignal::Ledger);
        return;
    }
    if let Some(caps) = BEGIN_TRAINING_WITH.captures(message) {
        push(&caps[1], PlayerSignal::Ledger);
        return;
    }
    if let Some(caps) = TEACHER_OFFER.captures(message) {
        push(&caps[1], PlayerSignal::Offer);
        return;
    }
    if let Some(caps) = TEACHER_ACCEPT.captures(message) {
        push(&caps[1], PlayerSignal::Offer);
    }
}

/// Split a Clan Lord name list — "A, B, C and D" — into individual names.
fn split_name_list(list: &str) -> Vec<&str> {
    list.split(", ")
        .flat_map(|chunk| chunk.split(" and "))
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn detect(msg: &str) -> Vec<(String, PlayerSignal)> {
        let mut out = Vec::new();
        detect_player_signals(msg, &mut out);
        out
    }

    #[test]
    fn clanning_on_and_off() {
        assert_eq!(detect("Fenwick is now Clanning."), vec![("Fenwick".into(), PlayerSignal::Clanning)]);
        assert_eq!(
            detect("Bramwell Gorse is no longer Clanning."),
            vec![("Bramwell Gorse".into(), PlayerSignal::Clanning)]
        );
    }

    #[test]
    fn sharing_forms() {
        assert_eq!(
            detect("Fenwick is sharing experiences with you."),
            vec![("Fenwick".into(), PlayerSignal::Sharing)]
        );
        assert_eq!(
            detect("You begin sharing your experiences with Fenwick."),
            vec![("Fenwick".into(), PlayerSignal::Sharing)]
        );
    }

    #[test]
    fn share_list_splits_every_name() {
        let got = detect("You are sharing experiences with Fenwick, Halloway, Quillon, Corven and Brindle.");
        let names: Vec<String> = got.iter().map(|(n, _)| n.clone()).collect();
        assert_eq!(names, vec!["Fenwick", "Halloway", "Quillon", "Corven", "Brindle"]);
        assert!(got.iter().all(|(_, s)| *s == PlayerSignal::Sharing));
    }

    #[test]
    fn no_longer_sharing_is_not_a_share_list() {
        // Must not be parsed as a name list — "no longer" breaks the prefix.
        assert!(detect("You are no longer sharing experiences with Abox.").is_empty());
    }

    #[test]
    fn thinks_to_you() {
        assert_eq!(
            detect(r#"Fenwick thinks to you, "helpful: https://example.com""#),
            vec![("Fenwick".into(), PlayerSignal::Thinks)]
        );
    }

    #[test]
    fn ledger_forms() {
        assert_eq!(
            detect("Fenwick writes Thistledown's name in her training ledger."),
            vec![("Fenwick".into(), PlayerSignal::Ledger)]
        );
        assert_eq!(
            detect("Bramwell Gorse writes your name in her training ledger."),
            vec![("Bramwell Gorse".into(), PlayerSignal::Ledger)]
        );
        assert_eq!(
            detect("Rushlight shows his training ledger to you."),
            vec![("Rushlight".into(), PlayerSignal::Ledger)]
        );
    }

    #[test]
    fn begin_training_with_is_a_ledger_signal() {
        // Only the player-teacher accept flow emits this; NPC training never does.
        assert_eq!(
            detect("¥ You begin training with Bramwell Gorse."),
            vec![("Bramwell Gorse".into(), PlayerSignal::Ledger)]
        );
    }

    #[test]
    fn teacher_offer_mac_prefix_and_curly_quotes() {
        assert_eq!(
            detect("¥ Bramwell Gorse offers: \u{201C}I can teach you to be more receptive to healing.\u{201D}"),
            vec![("Bramwell Gorse".into(), PlayerSignal::Offer)]
        );
    }

    #[test]
    fn teacher_offer_windows_prefix() {
        assert_eq!(
            detect("• Tallow offers: \u{201C}I can teach you to heal multiple people in your vicinity.\u{201D}"),
            vec![("Tallow".into(), PlayerSignal::Offer)]
        );
    }

    #[test]
    fn teacher_accept_line() {
        assert_eq!(
            detect("¥ To accept her offer, say: I accept Bramwell Gorse as my teacher."),
            vec![("Bramwell Gorse".into(), PlayerSignal::Offer)]
        );
        assert_eq!(
            detect("• To accept his offer, say: I accept Rushlight as my teacher."),
            vec![("Rushlight".into(), PlayerSignal::Offer)]
        );
    }

    #[test]
    fn npc_trainer_lines_produce_nothing() {
        for msg in [
            r#"Duvin Beastlore says, "Hail, Ruuk. You show great devotion to your studies.""#,
            r#"Duvin Beastlore says, "I can teach you to study the ways of various creatures.""#,
            "Duvin Beastlore bows.",
            "You slaughtered a Rat.",
            "¥You seem to fight more effectively now.",
        ] {
            assert!(detect(msg).is_empty(), "{msg} should yield no player signal");
        }
    }

    #[test]
    fn signal_precedence_ranks() {
        assert!(PlayerSignal::Ledger.rank() > PlayerSignal::Offer.rank());
        assert!(PlayerSignal::Offer.rank() > PlayerSignal::Thinks.rank());
        assert!(PlayerSignal::Thinks.rank() > PlayerSignal::Sharing.rank());
        assert!(PlayerSignal::Sharing.rank() > PlayerSignal::Clanning.rank());
    }

    #[test]
    fn out_buffer_is_appended_not_cleared() {
        let mut out = vec![("Sentinel".to_string(), PlayerSignal::Clanning)];
        detect_player_signals("Fenwick is now Clanning.", &mut out);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].0, "Sentinel");
    }
}
