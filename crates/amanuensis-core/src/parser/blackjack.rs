//! Casino blackjack, followed line by line through a dealer's table talk.
//!
//! Confirmed against real sessions (dealer Djill, 2026-10-08; dealer Jack Vintian, 2018-2019):
//! - the dealer asks `Ruuk, your bet please?` and the bet is the number the player then says;
//!   `...sitting this hand out.` means no hand;
//! - `deals Ruuk the J of gems and the 3 of swords.` starts the hand;
//! - `takes Ruuk's coins and deals one more card.` is a double (the bet is taken again);
//! - `picks up your bet.` loses the bet, `hands you your winnings.` wins the bet (even money:
//!   the `You have N coins.` balance moves by exactly the bet), `returns your bet in the push.`
//!   is a push. A bust is followed by `picks up your bet.`
//! - a natural: `deals Ruuk the K of gems and the A of gems. Blackjack!`, paid 3:2 (527 → 677
//!   coins on a 100 bet);
//! - at a shared table other players' hands read `hands Sunny her winnings.`, so only lines
//!   saying "your" settle our hands;
//! - `Please tell me the number of coins you want to bet, Ruuk.` as a second bet prompt;
//! - `takes Sunny's coins and splits the pair.`, and `Sunny, do you want insurance for 27c?`
//!   (insurance costs half the bet, rounded up).
//!
//! Inferred, never seen settled in a log: each split hand carries the bet and the outcome
//! lines settle the hands in order, with a double applying to the hand being played;
//! insurance is taken when the player says yes, paid 2:1 on `pays your insurance bet` (a
//! Scribius 2.0.5 fragment) and otherwise lost; `hands you back your bet in the push` (also
//! a Scribius fragment) is a second push wording.

use once_cell::sync::Lazy;
use regex::Regex;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandOutcome {
    Win,
    Loss,
    Push,
}

impl HandOutcome {
    pub fn as_str(self) -> &'static str {
        match self {
            HandOutcome::Win => "win",
            HandOutcome::Loss => "loss",
            HandOutcome::Push => "push",
        }
    }
}

/// One settled hand.
#[derive(Debug, Clone, PartialEq)]
pub struct BlackjackHand {
    /// The bet placed before the deal, or 0 when the bet line wasn't seen.
    pub bet: i64,
    pub outcome: HandOutcome,
    /// Coins won (positive) or lost (negative), including doubling and any insurance.
    pub net: i64,
    pub doubled: bool,
    pub natural: bool,
}

static BET_PROMPT: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"^.+? asks, "(.+?), your bet please\?"$|you want to bet, ([^?.,"]+)"#).unwrap()
});
static SAYS_NUMBER: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"^(.+?) says, "(\d[\d,]*)(?: coins?)?\.?"$"#).unwrap());
static SAYS_YES: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"^(.+?) says, "(?i:yes|y|ok|sure)[.!]?"$"#).unwrap());
static DEAL: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^.+? deals (.+?) the .+? and the .+?\.( Blackjack!)?$").unwrap());
static DOUBLE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^.+? takes (.+?)'s coins and deals one more card").unwrap());
static SPLIT: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^.+? takes (.+?)'s coins and splits the pair").unwrap());
static PLAYER_BUST: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^.+? deals (.+?) the .+?, giving .+\. Bust!$").unwrap());
static INSURANCE_OFFER: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(.+?), do you want insurance for (\d[\d,]*)").unwrap());

#[derive(Debug)]
struct OpenHand {
    bet: i64,
    stake: i64,
    doubled: bool,
    natural: bool,
    outcome: Option<(HandOutcome, i64)>,
}

/// Follows one character's blackjack rounds. Feed it every line's message in order; it
/// returns the round's hands once they have all been settled.
#[derive(Debug, Default)]
pub struct BlackjackTracker {
    awaiting_bet: bool,
    /// Whether the dealer's latest bet prompt was to us. At a shared table the dealer asks
    /// each player in turn, and "sitting this hand out" answers the latest prompt.
    last_prompt_ours: bool,
    bet: i64,
    hands: Vec<OpenHand>,
    /// The hand being played, for a double after a split.
    playing: usize,
    insurance_offer: Option<i64>,
    insurance: i64,
    insurance_paid: bool,
}

fn is(name: &str, char_name: &str) -> bool {
    name.trim().eq_ignore_ascii_case(char_name)
}

fn number(s: &str) -> i64 {
    s.replace(',', "").parse().unwrap_or(0)
}

impl BlackjackTracker {
    pub fn feed(&mut self, message: &str, char_name: &str) -> Option<Vec<BlackjackHand>> {
        if let Some(c) = BET_PROMPT.captures(message) {
            let who = c.get(1).or_else(|| c.get(2)).map_or("", |m| m.as_str());
            let ours = is(who, char_name);
            if ours && !self.awaiting_bet {
                self.bet = 0;
            }
            // Once another player is asked, anything we say is chat, not a bet.
            self.awaiting_bet = ours;
            self.last_prompt_ours = ours;
            return None;
        }
        if message.ends_with("sitting this hand out.\"") {
            if self.last_prompt_ours {
                self.reset();
            }
            return None;
        }
        if let Some(c) = SAYS_NUMBER.captures(message) {
            if self.awaiting_bet && is(&c[1], char_name) {
                self.bet = number(&c[2]);
            }
            return None;
        }
        if let Some(c) = INSURANCE_OFFER.captures(message) {
            if c[1].rsplit('"').next().is_some_and(|n| is(n, char_name)) {
                self.insurance_offer = Some(number(&c[2]));
            }
            return None;
        }
        if let Some(c) = SAYS_YES.captures(message) {
            if is(&c[1], char_name) {
                if let Some(amount) = self.insurance_offer.take() {
                    self.insurance = amount;
                }
            }
            return None;
        }
        if let Some(c) = DEAL.captures(message) {
            if is(&c[1], char_name) {
                // A new round; anything left open from a round we lost track of is dropped.
                let bet = self.bet;
                self.hands = vec![OpenHand {
                    bet,
                    stake: bet,
                    doubled: false,
                    natural: c.get(2).is_some(),
                    outcome: None,
                }];
                self.playing = 0;
                self.awaiting_bet = false;
                self.insurance = 0;
                self.insurance_offer = None;
                self.insurance_paid = false;
            }
            return None;
        }
        if let Some(c) = DOUBLE.captures(message) {
            if is(&c[1], char_name) {
                if let Some(h) = self.hands.get_mut(self.playing) {
                    h.stake *= 2;
                    h.doubled = true;
                }
            }
            return None;
        }
        if let Some(c) = SPLIT.captures(message) {
            if is(&c[1], char_name) && !self.hands.is_empty() {
                let bet = self.hands[0].bet;
                self.hands.push(OpenHand { bet, stake: bet, doubled: false, natural: false, outcome: None });
            }
            return None;
        }
        if message.contains(" says, \"You stand with ") {
            self.next_hand();
            return None;
        }
        if let Some(c) = PLAYER_BUST.captures(message) {
            if is(&c[1], char_name) {
                self.next_hand();
            }
            return None;
        }
        if message.contains(" pays your insurance bet") {
            self.insurance_paid = true;
            return None;
        }

        let outcome = if message.ends_with(" picks up your bet.") {
            HandOutcome::Loss
        } else if message.ends_with(" hands you your winnings.") {
            HandOutcome::Win
        } else if message.ends_with(" returns your bet in the push.")
            || message.ends_with(" hands you back your bet in the push.")
        {
            HandOutcome::Push
        } else {
            return None;
        };
        self.settle(outcome)
    }

    fn next_hand(&mut self) {
        if self.playing + 1 < self.hands.len() {
            self.playing += 1;
        }
    }

    fn settle(&mut self, outcome: HandOutcome) -> Option<Vec<BlackjackHand>> {
        if self.hands.is_empty() {
            // The deal wasn't seen (e.g. the log starts mid-hand); the bet may still be known.
            let bet = self.bet;
            self.hands.push(OpenHand { bet, stake: bet, doubled: false, natural: false, outcome: None });
        }
        let hand = self.hands.iter_mut().find(|h| h.outcome.is_none())?;
        let net = match outcome {
            HandOutcome::Win if hand.natural => hand.stake * 3 / 2,
            HandOutcome::Win => hand.stake,
            HandOutcome::Loss => -hand.stake,
            HandOutcome::Push => 0,
        };
        hand.outcome = Some((outcome, net));
        if self.hands.iter().any(|h| h.outcome.is_none()) {
            return None;
        }

        let insurance_net = match (self.insurance, self.insurance_paid) {
            (0, _) => 0,
            (stake, true) => stake * 2,
            (stake, false) => -stake,
        };
        let settled = self
            .hands
            .drain(..)
            .enumerate()
            .map(|(i, h)| {
                let (outcome, net) = h.outcome.expect("all hands settled");
                BlackjackHand {
                    bet: h.bet,
                    outcome,
                    net: net + if i == 0 { insurance_net } else { 0 },
                    doubled: h.doubled,
                    natural: h.natural,
                }
            })
            .collect();
        self.reset();
        Some(settled)
    }

    fn reset(&mut self) {
        *self = Self::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn play(lines: &str) -> Vec<BlackjackHand> {
        let mut t = BlackjackTracker::default();
        lines.lines().filter_map(|l| t.feed(l, "Ruuk")).flatten().collect()
    }

    #[test]
    fn real_session_with_djill() {
        let hands = play(include_str!("../../tests/fixtures/blackjack_djill.txt"));
        assert_eq!(hands.len(), 12, "the sat-out hand is not a hand");
        let count = |o| hands.iter().filter(|h| h.outcome == o).count();
        assert_eq!(count(HandOutcome::Win), 3);
        assert_eq!(count(HandOutcome::Loss), 8);
        assert_eq!(count(HandOutcome::Push), 1);
        // The first hand was doubled on 50 and lost.
        assert_eq!(hands[0], BlackjackHand { bet: 50, outcome: HandOutcome::Loss, net: -100, doubled: true, natural: false });
        assert_eq!(hands.iter().map(|h| h.bet).sum::<i64>(), 1000);
        // Wins 50 + 100 + 100; losses 100 + 6 x 50 + 100.
        assert_eq!(hands.iter().map(|h| h.net).sum::<i64>(), 250 - 500);
    }

    #[test]
    fn natural_pays_three_to_two() {
        let hands = play(
            "Djill asks, \"Ruuk, your bet please?\"\n\
             Ruuk says, \"100\"\n\
             Djill deals Ruuk the A of gems and the K of swords. Blackjack!\n\
             Djill hands you your winnings.\n",
        );
        assert_eq!(hands, vec![BlackjackHand { bet: 100, outcome: HandOutcome::Win, net: 150, doubled: false, natural: true }]);
    }

    #[test]
    fn shared_table_wording() {
        // Jack Vintian's table, 2018: another player's win is not ours, and the second
        // bet prompt form is understood.
        let hands = play(
            "Jack Vintian says, \"Please tell me the number of coins you want to bet, Ruuk.\"\n\
             Ruuk says, \"100\"\n\
             Jack Vintian deals Sunny the A of leaves and the 4 of gems.\n\
             Jack Vintian deals Ruuk the K of gems and the A of gems. Blackjack!\n\
             Jack Vintian hands Sunny her winnings.\n\
             Jack Vintian hands you your winnings.\n",
        );
        assert_eq!(hands, vec![BlackjackHand { bet: 100, outcome: HandOutcome::Win, net: 150, doubled: false, natural: true }]);
    }

    #[test]
    fn only_our_prompt_takes_a_bet() {
        // Jack Vintian's table, 2018-02-04: Ruuk bets 67, then jokes "1000000" while Mork is
        // being asked; and the sit-out after Mork's prompt is Mork's, not Ruuk's.
        let hands = play(
            "Jack Vintian asks, \"Ruuk, your bet please?\"\n\
             Ruuk says, \"67\"\n\
             Jack Vintian asks, \"Mork, your bet please?\"\n\
             Ruuk says, \"1000000\"\n\
             Jack Vintian says, \"Never mind then; I guess you're sitting this hand out.\"\n\
             Jack Vintian deals Ruuk the K of swords and the 3 of cups.\n\
             Jack Vintian picks up your bet.\n",
        );
        assert_eq!(hands.len(), 1);
        assert_eq!((hands[0].bet, hands[0].net), (67, -67));
    }

    #[test]
    fn split_settles_each_hand() {
        let hands = play(
            "Djill asks, \"Ruuk, your bet please?\"\n\
             Ruuk says, \"50\"\n\
             Djill deals Ruuk the 8 of gems and the 8 of swords.\n\
             Djill takes Ruuk's coins and splits the pair.\n\
             Djill deals Ruuk the 3 of leaves.\n\
             Djill takes Ruuk's coins and deals one more card.\n\
             Djill says, \"You stand with 19.\"\n\
             Djill says, \"You stand with 18.\"\n\
             Djill hands you your winnings.\n\
             Djill picks up your bet.\n",
        );
        assert_eq!(hands.len(), 2);
        assert_eq!((hands[0].net, hands[0].doubled), (100, true));
        assert_eq!((hands[1].net, hands[1].doubled), (-50, false));
    }

    #[test]
    fn insurance_is_paid_or_lost() {
        let base = "Djill asks, \"Ruuk, your bet please?\"\n\
                    Ruuk says, \"100\"\n\
                    Djill deals Ruuk the 9 of gems and the 9 of swords.\n\
                    Djill asks, \"Ruuk, do you want insurance for 50 coins?\"\n\
                    Ruuk says, \"yes\"\n";
        let paid = play(&format!("{base}Djill pays your insurance bet.\nDjill picks up your bet.\n"));
        assert_eq!(paid[0].net, -100 + 100);
        let lost = play(&format!("{base}Djill hands you your winnings.\n"));
        assert_eq!(lost[0].net, 100 - 50);
    }

    #[test]
    fn other_players_and_unknown_bets() {
        // Another player's deal is not ours; an outcome with no seen bet still counts as a hand.
        let hands = play(
            "Djill asks, \"Biro, your bet please?\"\n\
             Ruuk says, \"500\"\n\
             Djill deals Biro the 2 of gems and the 3 of swords.\n\
             Djill picks up your bet.\n",
        );
        assert_eq!(hands, vec![BlackjackHand { bet: 0, outcome: HandOutcome::Loss, net: 0, doubled: false, natural: false }]);
    }
}
