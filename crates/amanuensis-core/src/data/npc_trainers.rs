use std::collections::HashSet;

use once_cell::sync::Lazy;

/// In-game display names of NPC trainers. Fourteen of these are absent from
/// `trainers.json`, because that file is keyed on rank-*message* short names rather than
/// display names; `Duvin Beastlore` is listed redundantly for documentation value. Every
/// entry here was observed producing real checkpoints in live logs.
///
/// This guard is ADDITIVE-ONLY: being listed protects a name from ever being classified as
/// a player. Being absent flags nobody — behavioural detection alone flags. An incomplete
/// list therefore cannot cost a user data.
static EXTRA_NPC_DISPLAY_NAMES: &[&str] = &[
    "Anan Faure",
    "AnDeux Faure",
    "AnQuart Faure",
    "AnSept Faure",
    "AnTrix Faure",
    "Chronos",
    "Diggin",
    "Duvin Beastlore",
    "Hardia",
    "Higgrus",
    "Metta Sylpha",
    "Par Troon",
    "Respin Verminbane",
    "Splash O'Sul",
    "Tra'Kning",
];

/// `trainers.json` entries that are craftable objects or skill labels rather than people.
/// They are excluded from the guard because they can never greet you with `Hail, X`, so
/// guarding them protects nothing — it only makes a *player* who happens to share the
/// name permanently undetectable.
///
/// This matters because the NPC namespace is closed and small while the player namespace
/// is open and grows forever, so every guarded name is a permanent hole that becomes more
/// likely to be hit over time. Several of these labels are ordinary words a player could
/// plausibly take as a name (`Gossamer`, `Phantasm`, `Bloodblade`), and since none of them
/// can ever speak, excluding them costs nothing.
static NON_PERSON_TRAINER_LABELS: &[&str] = &[
    "Bloodblade",
    "Bloodblade Decay",
    "Catsbane Necklace",
    "Champion Blade",
    "Champion Blade Decay",
    "Dark Blue Paint",
    "Dark Green Paint",
    "Energy Potion",
    "Gossamer",
    "Gossamer Decay",
    "Light Blue Paint",
    "Light Green Paint",
    "Pink Paint",
    "Purple Paint",
    "Thieves' Cant",
    "Tykan Potion",
    "Yellow Paint",
];

/// Lowercased set of every guarded NPC trainer name: the short names in `trainers.json`
/// (minus `NON_PERSON_TRAINER_LABELS`) plus `EXTRA_NPC_DISPLAY_NAMES`.
static NPC_TRAINER_NAMES: Lazy<HashSet<String>> = Lazy::new(|| {
    let excluded: HashSet<String> = NON_PERSON_TRAINER_LABELS
        .iter()
        .map(|s| s.to_lowercase())
        .collect();

    let mut set: HashSet<String> = EXTRA_NPC_DISPLAY_NAMES
        .iter()
        .map(|s| s.to_lowercase())
        .collect();

    // trainers.json maps "¥message" -> { "trainer": "Name", ... }
    if let Ok(raw) =
        serde_json::from_slice::<serde_json::Value>(include_bytes!("../../data/trainers.json"))
    {
        if let Some(map) = raw.as_object() {
            for value in map.values() {
                if let Some(name) = value.get("trainer").and_then(|v| v.as_str()) {
                    let key = name.to_lowercase();
                    if !excluded.contains(&key) {
                        set.insert(key);
                    }
                }
            }
        }
    }

    set
});

/// True if `name` is a known NPC trainer and must never be classified as a player.
pub fn is_known_npc_trainer(name: &str) -> bool {
    let key = name.trim().to_lowercase();
    !key.is_empty() && NPC_TRAINER_NAMES.contains(&key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_names_from_trainers_json_are_guarded() {
        assert!(is_known_npc_trainer("Histia"));
        assert!(is_known_npc_trainer("Bangus Anmash"));
        assert!(is_known_npc_trainer("Master Bodrus"));
    }

    #[test]
    fn observed_display_name_variants_are_guarded() {
        // Display names observed in real logs. All but "Duvin Beastlore" are absent from
        // trainers.json, which stores only rank-message short names.
        for name in [
            "Higgrus", "Chronos", "Hardia", "Splash O'Sul", "Diggin",
            "Anan Faure", "AnDeux Faure", "AnQuart Faure", "AnSept Faure", "AnTrix Faure",
            "Tra'Kning", "Par Troon", "Metta Sylpha", "Respin Verminbane", "Duvin Beastlore",
        ] {
            assert!(is_known_npc_trainer(name), "{name} should be guarded");
        }
    }

    #[test]
    fn matching_is_case_insensitive_and_trimmed() {
        assert!(is_known_npc_trainer("  histia  "));
        assert!(is_known_npc_trainer("SPLASH O'SUL"));
    }

    #[test]
    fn object_and_skill_labels_are_not_guarded() {
        // These are trainers.json entries, but they are craftable objects and skill
        // labels, not people — they can never greet you, so guarding them would only
        // block a player who shares the name.
        for label in [
            "Bloodblade", "Bloodblade Decay", "Gossamer", "Gossamer Decay",
            "Champion Blade", "Champion Blade Decay", "Catsbane Necklace",
            "Thieves' Cant", "Tykan Potion", "Energy Potion",
            "Yellow Paint", "Pink Paint", "Purple Paint",
            "Dark Blue Paint", "Dark Green Paint", "Light Blue Paint", "Light Green Paint",
        ] {
            assert!(
                !is_known_npc_trainer(label),
                "{label} is an object/skill label and must not be guarded"
            );
        }
    }

    #[test]
    fn actual_npc_trainers_remain_guarded_alongside_the_exclusions() {
        // Guard removal must not leak into real people.
        for name in ["Histia", "Evus", "Swengus", "Atkus", "Sprite", "Master Bodrus"] {
            assert!(is_known_npc_trainer(name), "{name} should still be guarded");
        }
    }

    #[test]
    fn players_are_not_guarded() {
        assert!(!is_known_npc_trainer("Fenwick"));
        assert!(!is_known_npc_trainer("Bramwell Gorse"));
        assert!(!is_known_npc_trainer("Tallow"));
        assert!(!is_known_npc_trainer(""));
    }
}
