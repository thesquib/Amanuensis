//! Detects Clan Lord trainer sign-ups from the dealer-style exchange:
//! question, the character's "yes", then the trainer's welcome.

use once_cell::sync::Lazy;
use regex::Regex;

use super::patterns::{WELCOME_BACK, WELCOME_LOGIN};
use super::player_signals::BEGIN_TRAINING_WITH;

const WINDOW: u32 = 30;

static QUESTION: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"^(.+?) (?:asks|says), "Are you sure you would like to train with me\?""#)
        .expect("regex compile error")
});
static REPLY: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(?i)^(.+?) says, "(yes|no)[.!]?"$"#).expect("regex compile error")
});
static WELCOME_ABOARD: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"^(.+?) says, "Welcome aboard\. Go into the world"#).expect("regex compile error")
});

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SignupEvent {
    Npc(String),
    PlayerTeacher,
}

#[derive(Clone)]
struct Pending {
    trainer: String,
    char_name: String,
    confirmed: bool,
    lines_left: u32,
}

#[derive(Clone, Default)]
pub struct SignupTracker {
    pending: Option<Pending>,
}

impl SignupTracker {
    pub fn reset(&mut self) {
        self.pending = None;
    }

    pub fn feed(&mut self, message: &str, active_name: Option<&str>) -> Option<SignupEvent> {
        let message = message.trim();
        let active = active_name.map(str::trim).unwrap_or("");

        if WELCOME_LOGIN.is_match(message) || WELCOME_BACK.is_match(message) {
            self.pending = None;
            return None;
        }
        if BEGIN_TRAINING_WITH.is_match(message) {
            self.pending = None;
            return Some(SignupEvent::PlayerTeacher);
        }
        if let Some(c) = QUESTION.captures(message).filter(|_| !active.is_empty()) {
            self.pending = Some(Pending {
                trainer: c[1].trim().to_string(),
                char_name: active.to_string(),
                confirmed: false,
                lines_left: WINDOW,
            });
            return None;
        }

        let p = self.pending.as_mut()?;
        if let Some(c) = REPLY.captures(message) {
            let who = c[1].trim();
            if who.eq_ignore_ascii_case(&p.char_name) && who.eq_ignore_ascii_case(active) {
                if c[2].eq_ignore_ascii_case("yes") {
                    // only the first confirming yes advances the exchange for free
                    if !p.confirmed {
                        p.confirmed = true;
                        return None;
                    }
                } else {
                    self.pending = None;
                    return None;
                }
            }
        } else if let Some(c) = WELCOME_ABOARD.captures(message) {
            if p.confirmed
                && c[1].trim().eq_ignore_ascii_case(&p.trainer)
                && active.eq_ignore_ascii_case(&p.char_name)
            {
                let t = p.trainer.clone();
                self.pending = None;
                return Some(SignupEvent::Npc(t));
            }
        }
        // lines_left counts filler lines (anything that doesn't advance the exchange) after the question.
        if let Some(p) = self.pending.as_mut() {
            p.lines_left -= 1;
            if p.lines_left == 0 {
                self.pending = None;
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(lines: &[&str], active: &str) -> Vec<SignupEvent> {
        let mut t = SignupTracker::default();
        lines.iter().filter_map(|l| t.feed(l, Some(active))).collect()
    }
    const Q: &str = r#"Higgrus asks, "Are you sure you would like to train with me?""#;
    const W: &str = r#"Higgrus says, "Welcome aboard. Go into the world and learn what you may, while practicing our craft.""#;
    const Y: &str = r#"Squib says, "yes""#;
    fn npc(n: &str) -> Vec<SignupEvent> {
        vec![SignupEvent::Npc(n.into())]
    }

    #[test]
    fn full_exchange_asks() {
        assert_eq!(run(&[Q, Y, W], "Squib"), npc("Higgrus"));
    }
    #[test]
    fn full_exchange_says() {
        let q = r#"Higgrus says, "Are you sure you would like to train with me?""#;
        assert_eq!(run(&[q, Y, W], "Squib"), npc("Higgrus"));
    }
    #[test]
    fn yes_before_question_only_is_ignored() {
        assert_eq!(run(&[Y, Q, W], "Squib"), vec![]);
    }
    #[test]
    fn yes_before_and_after() {
        let q = r#"Respia says, "Are you sure you would like to train with me?""#;
        let w = r#"Respia says, "Welcome aboard. Go into the world and learn.""#;
        let y = r#"Tane says, "yes""#;
        assert_eq!(run(&[y, q, y, w], "Tane"), npc("Respia"));
    }
    #[test]
    fn bystander_yes_ignored() {
        assert_eq!(run(&[Q, r#"Garr says, "yes""#, W], "Ruuk"), vec![]);
    }
    #[test]
    fn no_cancels() {
        assert_eq!(run(&[Q, r#"Squib says, "no""#, Y, W], "Squib"), vec![]);
    }
    #[test]
    fn reset_between_yes_and_welcome_cancels() {
        let mut t = SignupTracker::default();
        t.feed(Q, Some("Squib"));
        t.feed(Y, Some("Squib"));
        t.reset();
        assert_eq!(t.feed(W, Some("Squib")), None);
    }
    #[test]
    fn window_expires_after_30_lines() {
        let mut v = vec![Q];
        v.extend(std::iter::repeat("filler").take(30));
        v.extend([Y, W]);
        assert_eq!(run(&v, "Squib"), vec![]);
        let mut v = vec![Q];
        v.extend(std::iter::repeat("filler").take(29));
        v.extend([Y, W]);
        assert_eq!(run(&v, "Squib").len(), 1);
    }
    #[test]
    fn player_teacher_clears_pending() {
        let got = run(&[Q, Y, "¥ You begin training with Jive Turkey.", W], "Squib");
        assert_eq!(got, vec![SignupEvent::PlayerTeacher]);
    }
    #[test]
    fn yes_matches_case_insensitively() {
        assert_eq!(run(&[Q, r#"Tane says, "yes""#, W], "tane"), npc("Higgrus"));
    }

    #[test]
    fn different_trainer_welcome_ignored() {
        let w = r#"Hardia says, "Welcome aboard. Go into the world and learn.""#;
        assert_eq!(run(&[Q, Y, w], "Squib"), vec![]);
    }
    #[test]
    fn second_question_replaces_pending() {
        let q2 = r#"Hardia asks, "Are you sure you would like to train with me?""#;
        let w2 = r#"Hardia says, "Welcome aboard. Go into the world and learn.""#;
        assert_eq!(run(&[Q, q2, Y, W, w2], "Squib"), npc("Hardia"));
    }
    #[test]
    fn no_active_name_gives_no_event() {
        let mut t = SignupTracker::default();
        let got: Vec<_> = [Q, Y, W].iter().filter_map(|l| t.feed(l, None)).collect();
        assert_eq!(got, vec![]);
    }
    #[test]
    fn active_change_without_reset_cancels() {
        let mut t = SignupTracker::default();
        t.feed(Q, Some("Squib"));
        t.feed(Y, Some("Squib"));
        assert_eq!(t.feed(W, Some("Tane")), None);
    }
    #[test]
    fn yes_with_trailing_words_does_not_confirm() {
        assert_eq!(run(&[Q, r#"Squib says, "yes, but later""#, W], "Squib"), vec![]);
    }
    #[test]
    fn quoted_welcome_in_other_speech_ignored() {
        let l = r#"Squib says, "he said Welcome aboard. Go into the world and learn""#;
        assert_eq!(run(&[Q, Y, l], "Squib"), vec![]);
    }
    #[test]
    fn welcome_back_clears_pending() {
        assert_eq!(run(&[Q, Y, "Welcome back, Squib!", W], "Squib"), vec![]);
    }
    #[test]
    fn capitalised_yes_confirms() {
        assert_eq!(run(&[Q, r#"Squib says, "Yes""#, W], "Squib"), npc("Higgrus"));
    }
    #[test]
    fn repeated_yes_spends_window() {
        let mut v = vec![Q, Y];
        v.extend(std::iter::repeat(Y).take(30));
        v.push(W);
        assert_eq!(run(&v, "Squib"), vec![]);
    }
}
