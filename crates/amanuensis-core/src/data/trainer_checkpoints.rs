use std::collections::HashMap;
use once_cell::sync::Lazy;

/// A rank range checkpoint: (min, max). max=None means "Maxed" (no upper bound known).
pub type CheckpointRange = (i64, Option<i64>);

static CHECKPOINT_MESSAGES: Lazy<HashMap<&'static str, CheckpointRange>> = Lazy::new(|| {
    let mut m: HashMap<&'static str, CheckpointRange> = HashMap::new();

    // === Regular trainer messages ===
    m.insert("You have much to learn.", (0, Some(9)));
    m.insert("It is good to see you.", (10, Some(19)));
    m.insert("Your persistence is paying off.", (20, Some(29)));
    m.insert("You are progressing well.", (30, Some(39)));
    m.insert("You are a good pupil of mine.", (40, Some(49)));
    m.insert("You are one of my better pupils.", (50, Some(99)));
    m.insert("You keep me on my toes.", (100, Some(149)));
    m.insert("It is hard to find more to teach you.", (150, Some(199)));
    m.insert("Teaching you is a challenge.", (200, Some(249)));
    m.insert("There is not much more I can teach you.", (250, Some(299)));
    m.insert("Teaching you has taught me much.", (300, Some(349)));
    m.insert("You have attained tremendous skill.", (350, Some(399)));
    m.insert("We are nearly equals.", (400, Some(449)));
    m.insert("You may be proud of your accomplishment.", (450, Some(499)));
    m.insert("You are becoming a master of our art.", (500, Some(549)));
    m.insert("Your dedication is commendable.", (550, Some(599)));
    m.insert("You show great devotion to your studies.", (600, Some(649)));
    m.insert("You are a credit to our craft.", (650, Some(699)));
    m.insert("Few indeed are your peers.", (700, Some(749)));
    m.insert("Your devotion to the craft is exemplary.", (750, Some(799)));
    m.insert("It is always good to greet a respected colleague.", (800, Some(899)));
    m.insert("You are truly a grand master.", (900, Some(999)));
    m.insert("Let us search for more we might learn together.", (1000, Some(1249)));
    m.insert("Your persistence is an example to us all.", (1250, Some(1499)));
    m.insert("Your skill astounds me.", (1500, Some(1749)));
    m.insert("You have progressed further than most.", (1750, Some(1999)));
    m.insert("You are nearly peerless.", (2000, Some(2249)));
    m.insert("You are a model of dedication.", (2250, Some(2499)));
    m.insert("You have achieved mastery.", (2500, Some(2749)));
    m.insert("You are enlightened.", (2750, Some(2999)));
    m.insert("Your command of our craft is inspiring.", (3000, Some(3249)));
    m.insert("All commend your dedication to our craft.", (3250, Some(3499)));
    m.insert("I marvel at your skill.", (3500, Some(3749)));
    m.insert("You walk where few have tread.", (3750, Some(3999)));
    m.insert("Few stones are unturned in your path.", (4000, Some(4249)));
    m.insert("Your footsteps guide the dedicated.", (4250, Some(4499)));
    m.insert("You chart a way through the unknown.", (4500, Some(4749)));
    m.insert("Your path illuminates the wilderness.", (4750, Some(4999)));
    m.insert("Your skill casts a long shadow.", (5000, Some(5249)));
    m.insert("You are a luminary of our art.", (5250, Some(5499)));
    m.insert("Your skill is a beacon to all.", (5500, Some(5749)));
    // "There is nothing I can teach you." is intentionally NOT mapped.
    // Trainers have wildly different rank caps (Diggun=1, Bodrus=100, Histia=5750+),
    // so this message cannot be assigned a meaningful rank_min without per-trainer limit data.

    // === Special Weapon trainer messages (different text, same ranges as above where overlapping) ===
    m.insert("You feel you have much to learn.", (0, Some(9)));
    m.insert("You feel tolerably skilled.", (10, Some(19)));
    // "Your persistence is paying off." — same as regular (already inserted)
    // "You are progressing well." — same as regular (already inserted)
    m.insert("You are becoming proficient.", (40, Some(49)));
    m.insert("You have learned much.", (50, Some(99)));
    m.insert("You have become skilled.", (100, Some(149)));
    m.insert("You have become very skilled.", (150, Some(199)));
    m.insert("Learning more is a challenge.", (200, Some(249)));
    m.insert("You have attained great skill.", (250, Some(299)));
    m.insert("You are becoming an expert.", (300, Some(349)));
    // "You have attained tremendous skill." — same as regular
    m.insert("You are close to attaining mastery.", (400, Some(449)));
    // "You may be proud of your accomplishment." — same as regular
    m.insert("You are becoming a master of your art.", (500, Some(549))); // "your" vs "our"
    // "Your dedication is commendable." — same as regular
    // "You show great devotion to your studies." — same as regular
    m.insert("You are a credit to your craft.", (650, Some(699))); // "your" vs "our"
    // "Few indeed are your peers." — same as regular
    m.insert("Your devotion to your craft is exemplary.", (750, Some(799))); // "your" vs "the"
    m.insert("Your expertise is unquestionable.", (800, Some(899)));
    // "You are truly a grand master." — same as regular
    m.insert("Few if any are your equal.", (1000, Some(1249)));
    // All messages from 1250+ are identical to regular table

    m
});

/// Look up a trainer checkpoint message and return the rank range it implies.
/// Returns `None` if the message is not a known checkpoint message.
pub fn lookup_checkpoint_message(msg: &str) -> Option<CheckpointRange> {
    CHECKPOINT_MESSAGES.get(msg).copied()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_known_checkpoint_messages() {
        assert_eq!(lookup_checkpoint_message("You keep me on my toes."), Some((100, Some(149))));
        assert_eq!(lookup_checkpoint_message("You have attained tremendous skill."), Some((350, Some(399))));
        assert_eq!(lookup_checkpoint_message("There is nothing I can teach you."), None);
        assert_eq!(lookup_checkpoint_message("You feel tolerably skilled."), Some((10, Some(19))));
        assert_eq!(lookup_checkpoint_message("You are a credit to your craft."), Some((650, Some(699))));
    }

    #[test]
    fn test_unknown_message() {
        assert_eq!(lookup_checkpoint_message("Hello there."), None);
        assert_eq!(lookup_checkpoint_message(""), None);
    }
}
