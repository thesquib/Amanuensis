use rusqlite::params;
use serde::Serialize;

use crate::error::Result;
use crate::parser::blackjack::BlackjackHand;
use super::Database;

/// Blackjack totals for a character (and its merge sources), the figures Scribius shows.
#[derive(Debug, Clone, Default, Serialize, PartialEq)]
pub struct BlackjackSummary {
    pub hands: i64,
    pub wins: i64,
    pub losses: i64,
    pub pushes: i64,
    /// Natural blackjacks dealt to the player.
    pub naturals: i64,
    /// Sum of the hands that came out ahead (insurance included).
    pub coins_won: i64,
    /// Sum of the hands that came out behind, as a positive number.
    pub coins_lost: i64,
    /// Total of the bets that were seen, and how many; their ratio is the average bet.
    pub bet_total: i64,
    pub bet_count: i64,
}

impl Database {
    pub fn record_blackjack_hand(&self, char_id: i64, played_at: &str, hand: &BlackjackHand) -> Result<()> {
        self.conn.execute(
            "INSERT INTO blackjack_hands (character_id, played_at, bet, outcome, net, doubled, natural)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                char_id,
                played_at,
                hand.bet,
                hand.outcome.as_str(),
                hand.net,
                hand.doubled,
                hand.natural
            ],
        )?;
        Ok(())
    }

    /// Blackjack summary across a character and all its merge sources, or None when no
    /// hand was ever recorded.
    pub fn blackjack_summary_merged(&self, char_id: i64) -> Result<Option<BlackjackSummary>> {
        let ids = self.char_ids_for_merged(char_id)?;
        let placeholders = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let sql = format!(
            "SELECT COUNT(*),
                    COALESCE(SUM(outcome = 'win'), 0),
                    COALESCE(SUM(outcome = 'loss'), 0),
                    COALESCE(SUM(outcome = 'push'), 0),
                    COALESCE(SUM(natural), 0),
                    COALESCE(SUM(CASE WHEN net > 0 THEN net ELSE 0 END), 0),
                    COALESCE(SUM(CASE WHEN net < 0 THEN -net ELSE 0 END), 0),
                    COALESCE(SUM(bet), 0),
                    COALESCE(SUM(bet > 0), 0)
             FROM blackjack_hands WHERE character_id IN ({placeholders})"
        );
        let s = self.conn.query_row(&sql, rusqlite::params_from_iter(ids.iter()), |r| {
            Ok(BlackjackSummary {
                hands: r.get(0)?,
                wins: r.get(1)?,
                losses: r.get(2)?,
                pushes: r.get(3)?,
                naturals: r.get(4)?,
                coins_won: r.get(5)?,
                coins_lost: r.get(6)?,
                bet_total: r.get(7)?,
                bet_count: r.get(8)?,
            })
        })?;
        Ok((s.hands > 0).then_some(s))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::blackjack::HandOutcome;

    fn hand(bet: i64, outcome: HandOutcome, net: i64) -> BlackjackHand {
        BlackjackHand { bet, outcome, net, doubled: false, natural: false }
    }

    #[test]
    fn summary_totals_hands() {
        let db = Database::open_in_memory().unwrap();
        let id = db.get_or_create_character("Ruuk").unwrap();
        assert_eq!(db.blackjack_summary_merged(id).unwrap(), None);
        db.record_blackjack_hand(id, "2026-10-08 15:34:15", &hand(50, HandOutcome::Loss, -100)).unwrap();
        db.record_blackjack_hand(id, "2026-10-08 15:36:05", &hand(50, HandOutcome::Win, 50)).unwrap();
        db.record_blackjack_hand(id, "2026-10-08 15:36:25", &hand(300, HandOutcome::Push, 0)).unwrap();
        db.record_blackjack_hand(id, "2026-10-08 15:40:00", &hand(0, HandOutcome::Win, 0)).unwrap();
        let s = db.blackjack_summary_merged(id).unwrap().unwrap();
        assert_eq!(
            s,
            BlackjackSummary {
                hands: 4,
                wins: 2,
                losses: 1,
                pushes: 1,
                naturals: 0,
                coins_won: 50,
                coins_lost: 100,
                bet_total: 400,
                bet_count: 3,
            }
        );
    }
}
