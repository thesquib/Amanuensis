# Player Trainers & Bestiary Links Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Hide player-run trainers from the Trainer Checkpoint Progression graph behind a toggle, and add a right-click bestiary link to creature names in Ranger Stats → Top Targets.

**Architecture:** Players are identified *behaviourally* — ten log line forms that only player characters produce (clanning, experience sharing, thinking to you, training ledgers, teacher offers) — and recorded in a new global `known_players` table. Checkpoint queries filter against that table with a `LEFT JOIN` at **query time**, so improved detection retroactively cleans historic data with no rescan. A bundled NPC-trainer guard list, derived from `trainers.json` plus observed display-name variants, protects real trainers from ever being flagged. Part 2 is frontend-only: the bestiary exposes per-family pages, so a shared `ContextMenu` component opens `beast/<Family>.php` via the already-permitted `shell.open`.

**Tech Stack:** Rust (rusqlite, once_cell, regex, serde_json), Tauri v2, React 19 + TypeScript, Zustand, Recharts, TailwindCSS v4.

## Global Constraints

- Rust edition 2021. All existing tests must continue to pass: `cargo test -p amanuensis-core` (366 unit tests) and `cargo test -p amanuensis-cli` (7 clap smoke tests).
- **Never delete or modify rows in `trainer_checkpoints`** as part of the player-filtering feature. The flag lives entirely in the side table `known_players`. (The one exception is the explicit reset-duplication fix in Task 3, which is a separate pre-existing bug.)
- The NPC guard list is **additive-only**: presence protects a name from being classified as a player; absence flags nobody. Behaviour alone flags.
- Default behaviour: player trainers are **hidden**. Both the GUI toggle and the CLI flag default to off.
- Tauri v2 argument naming: Rust `include_players` ↔ JavaScript `includePlayers`. Follow the existing `charId` convention.
- Name comparisons for the guard list and `known_players` lookups are **case-insensitive**.
- Do **not** run `tsc -b` or `npm run build` inside `crates/amanuensis-gui/ui` while `cargo tauri dev` is running — it writes `tsconfig.tsbuildinfo` into the watched tree and cycles the app. Stop the dev server first.
- Evidence precedence, strongest first: `ledger` (4), `offer` (3), `thinks` (2), `sharing` (1), `clanning` (0).

---

### Task 1: NPC trainer guard list

**Files:**
- Create: `crates/amanuensis-core/src/data/npc_trainers.rs`
- Modify: `crates/amanuensis-core/src/data/mod.rs`

**Interfaces:**
- Consumes: nothing.
- Produces: `pub fn is_known_npc_trainer(name: &str) -> bool` — case-insensitive, trims whitespace. Re-exported from `crate::data`.

Background: `crates/amanuensis-core/data/trainers.json` is keyed by rank message; each value has a `"trainer"` field holding a *short* name (e.g. `"Sylpha"`). Real logs use in-game *display* names (e.g. `"Metta Sylpha"`), so the guard list must add the observed variants. This module parses the bundled JSON itself rather than depending on `TrainerDb`, so it can be called from the DB layer too.

- [ ] **Step 1: Write the failing test**

Create `crates/amanuensis-core/src/data/npc_trainers.rs` containing only the test module for now:

```rust
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
        // These appear in real logs but are absent from trainers.json,
        // which stores only short names.
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
    fn players_are_not_guarded() {
        assert!(!is_known_npc_trainer("Fenwick"));
        assert!(!is_known_npc_trainer("Bramwell Gorse"));
        assert!(!is_known_npc_trainer("Tallow"));
        assert!(!is_known_npc_trainer(""));
    }
}
```

Add to `crates/amanuensis-core/src/data/mod.rs`, keeping the existing lines in alphabetical order:

```rust
pub mod npc_trainers;
```

and in the `pub use` block:

```rust
pub use npc_trainers::is_known_npc_trainer;
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p amanuensis-core npc_trainers`
Expected: FAIL — compile error, `cannot find function 'is_known_npc_trainer' in this scope`.

- [ ] **Step 3: Write minimal implementation**

Prepend to `crates/amanuensis-core/src/data/npc_trainers.rs`, above the test module:

```rust
use std::collections::HashSet;

use once_cell::sync::Lazy;

/// In-game display names of NPC trainers that `trainers.json` does not contain, because
/// that file is keyed on rank-*message* short names rather than display names. Every entry
/// here was observed producing real checkpoints in live logs.
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

/// Lowercased set of every guarded NPC trainer name: the short names in `trainers.json`
/// plus `EXTRA_NPC_DISPLAY_NAMES`.
static NPC_TRAINER_NAMES: Lazy<HashSet<String>> = Lazy::new(|| {
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
                    set.insert(name.to_lowercase());
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
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p amanuensis-core npc_trainers`
Expected: PASS — 4 tests.

- [ ] **Step 5: Commit**

```bash
git add crates/amanuensis-core/src/data/npc_trainers.rs crates/amanuensis-core/src/data/mod.rs
git commit -m "feat(data): additive-only NPC trainer guard list"
```

---

### Task 2: Player signal detection

**Files:**
- Create: `crates/amanuensis-core/src/parser/player_signals.rs`
- Modify: `crates/amanuensis-core/src/parser/mod.rs` (add `pub mod player_signals;` near the other `mod` declarations at the top of the file)

**Interfaces:**
- Consumes: nothing.
- Produces:
  - `pub enum PlayerSignal { Clanning, Sharing, Thinks, Ledger, Offer }` — derives `Debug, Clone, Copy, PartialEq, Eq`.
  - `pub fn as_str(&self) -> &'static str` on `PlayerSignal`, returning `"clanning" | "sharing" | "thinks" | "ledger" | "offer"`.
  - `pub fn rank(&self) -> u8` on `PlayerSignal`, returning `0 | 1 | 2 | 4 | 3` respectively (see Global Constraints).
  - `pub fn detect_player_signals(message: &str, out: &mut Vec<(String, PlayerSignal)>)` — appends every `(player_name, signal)` pair found in one already-timestamp-stripped log line. Does **not** clear `out`; the caller owns it as a reusable scratch buffer.

Ten line forms, all observed in real logs. `message` is the line with the leading `MM/DD/YY H:MM:SSa ` timestamp already stripped by the caller.

- [ ] **Step 1: Write the failing test**

Create `crates/amanuensis-core/src/parser/player_signals.rs` containing only the test module for now:

```rust
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
```

Add `pub mod player_signals;` alongside the other `mod` declarations at the top of `crates/amanuensis-core/src/parser/mod.rs`.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p amanuensis-core player_signals`
Expected: FAIL — compile error, `cannot find type 'PlayerSignal' in this scope`.

- [ ] **Step 3: Write minimal implementation**

Prepend to `crates/amanuensis-core/src/parser/player_signals.rs`, above the test module:

```rust
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
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p amanuensis-core player_signals`
Expected: PASS — 12 tests.

- [ ] **Step 5: Commit**

```bash
git add crates/amanuensis-core/src/parser/player_signals.rs crates/amanuensis-core/src/parser/mod.rs
git commit -m "feat(parser): behavioural player-character signal detection"
```

---

### Task 3: `known_players` table, queries, and reset handling

**Files:**
- Create: `crates/amanuensis-core/src/db/queries/player.rs`
- Modify: `crates/amanuensis-core/src/db/queries/mod.rs`
- Modify: `crates/amanuensis-core/src/db/schema.rs` (both the `create_tables` batch around line 140-167 and the `migrate_tables` `execute_batch` around line 293-321)
- Modify: `crates/amanuensis-core/src/db/queries/log_file.rs:80-105` (`reset_log_data`)

**Interfaces:**
- Consumes: `PlayerSignal` from Task 2 (`crate::parser::player_signals::PlayerSignal`).
- Produces, all on `impl Database`:
  - `pub fn upsert_known_player(&self, name: &str, signal: PlayerSignal, first_seen: &str) -> Result<()>`
  - `pub fn is_known_player(&self, name: &str) -> Result<bool>`
  - `pub fn list_known_players(&self) -> Result<Vec<(String, String)>>` — `(name, evidence)` pairs sorted by name.
  - `pub fn set_db_meta(&self, key: &str, value: &str) -> Result<()>`
  - `pub fn get_db_meta(&self, key: &str) -> Result<Option<String>>`

`known_players.name` stores the name **lowercased** so the primary key doubles as the case-insensitive index. `list_known_players` therefore returns lowercased names; that is fine because its only consumers compare case-insensitively.

Also included here: a pre-existing bug fix. `reset_log_data` deletes `log_files` (forcing every file to re-scan) but does **not** delete `trainer_checkpoints`, so every Rescan Logs duplicates all checkpoint rows. The duplicates are invisible on the graph (identical dots overlap) but bloat the table and would make Task 11's real-data assertions unstable.

- [ ] **Step 1: Write the failing test**

Create `crates/amanuensis-core/src/db/queries/player.rs` with only the test module:

```rust
#[cfg(test)]
mod tests {
    use super::super::Database;
    use crate::parser::player_signals::PlayerSignal;

    #[test]
    fn upsert_and_lookup_is_case_insensitive() {
        let db = Database::open_in_memory().unwrap();
        db.upsert_known_player("Fenwick", PlayerSignal::Clanning, "2017-11-22 23:07:16").unwrap();

        assert!(db.is_known_player("Fenwick").unwrap());
        assert!(db.is_known_player("fenwick").unwrap());
        assert!(db.is_known_player("  FENWICK ").unwrap());
        assert!(!db.is_known_player("Histia").unwrap());
    }

    #[test]
    fn stronger_evidence_overwrites_weaker() {
        let db = Database::open_in_memory().unwrap();
        db.upsert_known_player("Fenwick", PlayerSignal::Clanning, "2017-01-01 00:00:00").unwrap();
        db.upsert_known_player("Fenwick", PlayerSignal::Ledger, "2017-06-01 00:00:00").unwrap();

        let players = db.list_known_players().unwrap();
        assert_eq!(players, vec![("fenwick".to_string(), "ledger".to_string())]);
    }

    #[test]
    fn weaker_evidence_does_not_overwrite_stronger() {
        let db = Database::open_in_memory().unwrap();
        db.upsert_known_player("Fenwick", PlayerSignal::Ledger, "2017-01-01 00:00:00").unwrap();
        db.upsert_known_player("Fenwick", PlayerSignal::Clanning, "2017-06-01 00:00:00").unwrap();

        let players = db.list_known_players().unwrap();
        assert_eq!(players, vec![("fenwick".to_string(), "ledger".to_string())]);
    }

    #[test]
    fn first_seen_is_not_overwritten() {
        let db = Database::open_in_memory().unwrap();
        db.upsert_known_player("Fenwick", PlayerSignal::Clanning, "2017-01-01 00:00:00").unwrap();
        db.upsert_known_player("Fenwick", PlayerSignal::Ledger, "2019-06-01 00:00:00").unwrap();

        let first_seen: String = db
            .conn_for_test()
            .query_row("SELECT first_seen FROM known_players WHERE name='fenwick'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(first_seen, "2017-01-01 00:00:00");
    }

    #[test]
    fn db_meta_roundtrips() {
        let db = Database::open_in_memory().unwrap();
        assert_eq!(db.get_db_meta("missing").unwrap(), None);
        db.set_db_meta("k", "v").unwrap();
        assert_eq!(db.get_db_meta("k").unwrap(), Some("v".to_string()));
        db.set_db_meta("k", "v2").unwrap();
        assert_eq!(db.get_db_meta("k").unwrap(), Some("v2".to_string()));
    }

    #[test]
    fn reset_log_data_clears_players_and_checkpoints() {
        let db = Database::open_in_memory().unwrap();
        let char_id = db.get_or_create_character("Ruuk").unwrap();
        db.insert_trainer_checkpoint(char_id, "Histia", 0, Some(9), "2024-01-01 12:00:00").unwrap();
        db.upsert_known_player("Fenwick", PlayerSignal::Clanning, "2024-01-01 12:00:00").unwrap();
        db.set_db_meta("known_players_backfilled", "1").unwrap();

        db.reset_log_data().unwrap();

        assert!(db.list_known_players().unwrap().is_empty());
        assert_eq!(db.get_db_meta("known_players_backfilled").unwrap(), None);

        // Asserted via raw SQL rather than get_all_trainer_checkpoints, which does not
        // gain its `include_players` parameter until Task 6.
        let remaining: i64 = db
            .conn_for_test()
            .query_row("SELECT COUNT(*) FROM trainer_checkpoints", [], |r| r.get(0))
            .unwrap();
        assert_eq!(remaining, 0);
    }
}
```

Add to `crates/amanuensis-core/src/db/queries/mod.rs`, following the existing `mod` ordering:

```rust
mod player;
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p amanuensis-core queries::player`
Expected: FAIL — compile error, `no method named 'upsert_known_player' found`.

- [ ] **Step 3: Write minimal implementation**

First add the schema. In `crates/amanuensis-core/src/db/schema.rs`, append these two tables inside the **`create_tables`** `execute_batch` string, immediately after the `idx_trainer_checkpoints_lookup` index and before the closing `"` (around line 167):

```sql
        CREATE TABLE IF NOT EXISTS known_players (
            name       TEXT PRIMARY KEY,
            evidence   TEXT NOT NULL,
            first_seen TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS db_meta (
            key   TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );
```

Then add the *same two* `CREATE TABLE IF NOT EXISTS` statements to the **`migrate_tables`** `execute_batch` (around line 293-321), after the `idx_trainer_checkpoints_lookup` line, so existing databases gain them on open.

Next, if `Database` has no test accessor for its connection, add one to `crates/amanuensis-core/src/db/mod.rs` inside `impl Database`:

```rust
    /// Raw connection access for tests that need to assert on storage details.
    #[cfg(test)]
    pub(crate) fn conn_for_test(&self) -> &rusqlite::Connection {
        &self.conn
    }
```

Now prepend the implementation to `crates/amanuensis-core/src/db/queries/player.rs`:

```rust
use rusqlite::{params, OptionalExtension};

use crate::error::Result;
use crate::parser::player_signals::PlayerSignal;
use super::Database;

/// SQL fragment mapping an evidence string to its precedence rank.
/// Must stay in sync with `PlayerSignal::rank`.
const EVIDENCE_RANK: &str =
    "CASE ? WHEN 'ledger' THEN 4 WHEN 'offer' THEN 3 WHEN 'thinks' THEN 2 WHEN 'sharing' THEN 1 ELSE 0 END";

impl Database {
    /// Record `name` as a known player character.
    ///
    /// Names are stored lowercased so the primary key doubles as a case-insensitive index.
    /// `evidence` is only overwritten by a strictly stronger signal; `first_seen` is never
    /// overwritten, so it keeps the earliest observation.
    pub fn upsert_known_player(
        &self,
        name: &str,
        signal: PlayerSignal,
        first_seen: &str,
    ) -> Result<()> {
        let key = name.trim().to_lowercase();
        if key.is_empty() {
            return Ok(());
        }
        let sql = format!(
            "INSERT INTO known_players (name, evidence, first_seen)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(name) DO UPDATE SET evidence = ?2
             WHERE ({new_rank}) > ({old_rank})",
            new_rank = EVIDENCE_RANK.replacen('?', "?2", 1),
            old_rank = EVIDENCE_RANK.replacen('?', "known_players.evidence", 1),
        );
        self.conn
            .execute(&sql, params![key, signal.as_str(), first_seen])?;
        Ok(())
    }

    /// True if `name` has been observed doing something only a player can do.
    pub fn is_known_player(&self, name: &str) -> Result<bool> {
        let key = name.trim().to_lowercase();
        if key.is_empty() {
            return Ok(false);
        }
        let found: Option<i64> = self
            .conn
            .query_row(
                "SELECT 1 FROM known_players WHERE name = ?1",
                params![key],
                |row| row.get(0),
            )
            .optional()?;
        Ok(found.is_some())
    }

    /// All known players as `(lowercased_name, evidence)`, sorted by name.
    pub fn list_known_players(&self) -> Result<Vec<(String, String)>> {
        let mut stmt = self
            .conn
            .prepare("SELECT name, evidence FROM known_players ORDER BY name")?;
        let rows = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    /// Store a small key/value fact about this database (migration/backfill markers).
    pub fn set_db_meta(&self, key: &str, value: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO db_meta (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = ?2",
            params![key, value],
        )?;
        Ok(())
    }

    /// Read a `db_meta` value, or `None` if unset.
    pub fn get_db_meta(&self, key: &str) -> Result<Option<String>> {
        let value: Option<String> = self
            .conn
            .query_row(
                "SELECT value FROM db_meta WHERE key = ?1",
                params![key],
                |row| row.get(0),
            )
            .optional()?;
        Ok(value)
    }
}
```

Finally extend `reset_log_data` in `crates/amanuensis-core/src/db/queries/log_file.rs`. Add these three statements to the `execute_batch` string, immediately after `DELETE FROM log_lines;`:

```sql
             DELETE FROM known_players;
             DELETE FROM db_meta WHERE key = 'known_players_backfilled';
             DELETE FROM trainer_checkpoints;
```

and update that function's doc comment to record why:

```rust
    /// Clear all log-derived data while preserving rank overrides and trainer notes.
    ///
    /// `trainer_checkpoints` is included because `log_files` is cleared here, so every file
    /// re-scans and would otherwise insert a duplicate copy of every checkpoint.
    /// `known_players` is log-derived too, and is repopulated by the following scan.
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p amanuensis-core queries::player`
Expected: PASS — 6 tests.

Then confirm nothing else regressed: `cargo test -p amanuensis-core`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/amanuensis-core/src/db/queries/player.rs crates/amanuensis-core/src/db/queries/mod.rs crates/amanuensis-core/src/db/schema.rs crates/amanuensis-core/src/db/queries/log_file.rs crates/amanuensis-core/src/db/mod.rs
git commit -m "feat(db): known_players table, db_meta, and reset-duplication fix"
```

---

### Task 4: Backfill `known_players` from the stored log index

**Files:**
- Modify: `crates/amanuensis-core/src/db/queries/player.rs`
- Modify: `crates/amanuensis-core/src/db/mod.rs` (call the backfill after migration in `Database::open`)

**Interfaces:**
- Consumes: `detect_player_signals`, `PlayerSignal` (Task 2); `upsert_known_player`, `get_db_meta`, `set_db_meta` (Task 3).
- Produces: `pub fn backfill_known_players(&self) -> Result<usize>` on `impl Database` — returns the number of players inserted. No-op returning `0` when the `known_players_backfilled` marker in `db_meta` is set.

Existing databases already store every log line in the `log_lines` FTS table, so historic players can be recovered without a Rescan. Lines in `log_lines.content` still carry their `MM/DD/YY H:MM:SSa ` timestamp prefix, which must be stripped before matching.

- [ ] **Step 1: Write the failing test**

Add to the `tests` module in `crates/amanuensis-core/src/db/queries/player.rs`:

```rust
    fn insert_log_line(db: &Database, char_id: i64, content: &str) {
        db.conn_for_test()
            .execute(
                "INSERT INTO log_lines (content, character_id, timestamp, file_path)
                 VALUES (?1, ?2, ?3, ?4)",
                rusqlite::params![content, char_id, "2017-11-22 23:07:16", "CL Log 11.22.17"],
            )
            .unwrap();
    }

    #[test]
    fn backfill_finds_players_in_stored_log_lines() {
        let db = Database::open_in_memory().unwrap();
        let char_id = db.get_or_create_character("Ruuk").unwrap();

        insert_log_line(&db, char_id, "11/22/17 11:07:16p Fenwick is now Clanning.");
        insert_log_line(&db, char_id, "11/1/18 1:17:31p Bramwell Gorse writes your name in her training ledger.");
        insert_log_line(&db, char_id, r#"11/22/17 11:07:16p Fenwick says, "Hail, Ruuk. You are one of my better pupils.""#);
        insert_log_line(&db, char_id, r#"11/22/17 11:11:59p Duvin Beastlore says, "You show great devotion to your studies.""#);

        let inserted = db.backfill_known_players().unwrap();
        assert_eq!(inserted, 2);

        assert!(db.is_known_player("Fenwick").unwrap());
        assert!(db.is_known_player("Bramwell Gorse").unwrap());
        assert!(!db.is_known_player("Duvin Beastlore").unwrap());
    }

    #[test]
    fn backfill_never_flags_a_guarded_npc_trainer() {
        let db = Database::open_in_memory().unwrap();
        let char_id = db.get_or_create_character("Ruuk").unwrap();
        // Contrived: even if an NPC name appeared in a player-only line, the guard wins.
        insert_log_line(&db, char_id, "11/22/17 11:07:16p Higgrus is now Clanning.");

        assert_eq!(db.backfill_known_players().unwrap(), 0);
        assert!(!db.is_known_player("Higgrus").unwrap());
    }

    #[test]
    fn backfill_is_idempotent() {
        let db = Database::open_in_memory().unwrap();
        let char_id = db.get_or_create_character("Ruuk").unwrap();
        insert_log_line(&db, char_id, "11/22/17 11:07:16p Fenwick is now Clanning.");

        assert_eq!(db.backfill_known_players().unwrap(), 1);
        assert_eq!(db.backfill_known_players().unwrap(), 0, "marker should short-circuit");
        assert_eq!(db.list_known_players().unwrap().len(), 1);
    }

    #[test]
    fn backfill_keeps_strongest_evidence_across_lines() {
        let db = Database::open_in_memory().unwrap();
        let char_id = db.get_or_create_character("Ruuk").unwrap();
        insert_log_line(&db, char_id, "11/22/17 11:07:16p Fenwick is now Clanning.");
        insert_log_line(&db, char_id, "12/8/17 9:11:48a Fenwick writes Thistledown's name in her training ledger.");

        db.backfill_known_players().unwrap();
        assert_eq!(db.list_known_players().unwrap(), vec![("fenwick".to_string(), "ledger".to_string())]);
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p amanuensis-core queries::player::tests::backfill`
Expected: FAIL — compile error, `no method named 'backfill_known_players' found`.

- [ ] **Step 3: Write minimal implementation**

Add to the `impl Database` block in `crates/amanuensis-core/src/db/queries/player.rs`:

```rust
    /// One-time recovery of `known_players` from the already-indexed `log_lines` table.
    ///
    /// Existing databases store every log line, so historic players can be identified
    /// without asking the user to Rescan. Guarded by a `db_meta` marker so it runs once.
    /// If log indexing was disabled the sweep finds nothing and `known_players` instead
    /// fills during the next scan.
    pub fn backfill_known_players(&self) -> Result<usize> {
        if self.get_db_meta("known_players_backfilled")?.is_some() {
            return Ok(0);
        }

        let mut stmt = self.conn.prepare("SELECT content, timestamp FROM log_lines")?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;

        // (lowercased name) -> (strongest signal, first_seen for that observation)
        let mut found: std::collections::HashMap<String, (PlayerSignal, String)> =
            std::collections::HashMap::new();
        let mut scratch: Vec<(String, PlayerSignal)> = Vec::new();

        for row in rows {
            let (content, timestamp) = match row {
                Ok(v) => v,
                Err(_) => continue,
            };
            // log_lines.content keeps the raw "MM/DD/YY H:MM:SSa " prefix.
            let message = crate::parser::strip_log_timestamp(&content);

            scratch.clear();
            crate::parser::player_signals::detect_player_signals(message, &mut scratch);
            for (name, signal) in scratch.drain(..) {
                if crate::data::is_known_npc_trainer(&name) {
                    continue;
                }
                let key = name.to_lowercase();
                match found.get_mut(&key) {
                    Some((existing, _)) if existing.rank() >= signal.rank() => {}
                    Some(slot) => slot.0 = signal,
                    None => {
                        found.insert(key, (signal, timestamp.clone()));
                    }
                }
            }
        }
        drop(stmt);

        for (name, (signal, first_seen)) in &found {
            self.upsert_known_player(name, *signal, first_seen)?;
        }

        self.set_db_meta("known_players_backfilled", "1")?;
        Ok(found.len())
    }
```

`strip_log_timestamp` must exist as a public helper. `crates/amanuensis-core/src/parser/mod.rs` already has a private `parse_timestamp(line) -> Option<(DateTime, &str)>`; add this thin public wrapper next to it:

```rust
/// The message portion of a log line, with any leading timestamp removed.
/// Used when re-reading lines out of the stored `log_lines` index.
pub fn strip_log_timestamp(line: &str) -> &str {
    match parse_timestamp(line) {
        Some((_dt, message)) => message,
        None => line,
    }
}
```

Finally, run the backfill on open. In `crates/amanuensis-core/src/db/mod.rs`, inside `Database::open`, immediately after the existing `migrate_tables(...)` call, add:

```rust
        // Recover players from the stored log index for databases created before
        // behavioural player detection existed. Guarded by a db_meta marker.
        let _ = db.backfill_known_players();
```

Adjust the receiver name to match the local variable already in scope in that function. Errors are intentionally swallowed: a failed backfill must never prevent the database from opening, and the next scan repopulates the table anyway.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p amanuensis-core queries::player`
Expected: PASS — 10 tests.

Then: `cargo test -p amanuensis-core`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/amanuensis-core/src/db/queries/player.rs crates/amanuensis-core/src/db/mod.rs crates/amanuensis-core/src/parser/mod.rs
git commit -m "feat(db): backfill known_players from the stored log index"
```

---

### Task 5: Detect players during scanning

**Files:**
- Modify: `crates/amanuensis-core/src/parser/mod.rs` (the `LogParser` struct around line 39-53, `LogParser::new` around line 56-67, and the per-line loop that begins `for line in content.lines()` around line 476)

**Interfaces:**
- Consumes: `detect_player_signals`, `PlayerSignal` (Task 2); `upsert_known_player` (Task 3).
- Produces: `known_players_seen: RefCell<HashSet<String>>` field on `LogParser`, used as a per-scan write-dedup cache. No new public API.

The scan loop already computes `message` (timestamp-stripped) and `date_str` for every line. Player detection hooks in there, independent of `classify_line`, because a single line can be both a classified event and a player signal.

- [ ] **Step 1: Write the failing test**

Add to the `tests` module at the bottom of `crates/amanuensis-core/src/parser/mod.rs`:

```rust
    #[test]
    fn scan_detects_player_trainers_and_spares_npcs() {
        let dir = tempfile::tempdir().unwrap();
        let char_dir = dir.path().join("Ruuk");
        std::fs::create_dir_all(&char_dir).unwrap();
        std::fs::write(
            char_dir.join("CL Log 11.22.17"),
            concat!(
                "11/22/17 10:00:00p Welcome to Clan Lord, Ruuk!\n",
                "11/22/17 10:49:07p Fenwick is now Clanning.\n",
                "11/22/17 11:07:16p Fenwick says, \"Hail, Ruuk. You are one of my better pupils.\"\n",
                "11/22/17 11:11:55p Duvin Beastlore says, \"Hail, Ruuk. You show great devotion to your studies.\"\n",
            ),
        )
        .unwrap();

        let db = Database::open_in_memory().unwrap();
        let parser = LogParser::new(db).unwrap();
        parser.scan_folder(dir.path(), false).unwrap();
        parser.finalize_characters().unwrap();

        assert!(parser.db().is_known_player("Fenwick").unwrap());
        assert!(!parser.db().is_known_player("Duvin Beastlore").unwrap());
    }

    #[test]
    fn scan_records_share_list_members_as_players() {
        let dir = tempfile::tempdir().unwrap();
        let char_dir = dir.path().join("Ruuk");
        std::fs::create_dir_all(&char_dir).unwrap();
        std::fs::write(
            char_dir.join("CL Log 11.22.17"),
            concat!(
                "11/22/17 10:00:00p Welcome to Clan Lord, Ruuk!\n",
                "11/22/17 10:49:32p You are sharing experiences with Fenwick, Halloway and Brindle.\n",
            ),
        )
        .unwrap();

        let db = Database::open_in_memory().unwrap();
        let parser = LogParser::new(db).unwrap();
        parser.scan_folder(dir.path(), false).unwrap();

        for name in ["Fenwick", "Halloway", "Brindle"] {
            assert!(parser.db().is_known_player(name).unwrap(), "{name} should be a known player");
        }
    }
```

If `LogParser` has no `db()` accessor, add one next to `LogParser::new`:

```rust
    /// Borrow the underlying database (used by tests and by callers that already own the parser).
    pub fn db(&self) -> &Database {
        &self.db
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p amanuensis-core scan_detects_player_trainers`
Expected: FAIL — the assertion `parser.db().is_known_player("Fenwick")` returns `false`, because nothing writes to `known_players` during a scan yet.

- [ ] **Step 3: Write minimal implementation**

Add the field to the `LogParser` struct in `crates/amanuensis-core/src/parser/mod.rs`:

```rust
    /// Lowercased names already written to `known_players` during this scan.
    /// Purely a write-dedup cache — a busy log can contain the same name thousands of times.
    known_players_seen: RefCell<HashSet<String>>,
```

and initialise it in `LogParser::new`:

```rust
            known_players_seen: RefCell::new(HashSet::new()),
```

Add this helper method to `impl LogParser`:

```rust
    /// Record every player identified by `message`, skipping guarded NPC trainer names and
    /// names already written during this scan.
    fn record_player_signals(&self, message: &str, date_str: &str) -> Result<()> {
        let mut signals: Vec<(String, player_signals::PlayerSignal)> = Vec::new();
        player_signals::detect_player_signals(message, &mut signals);
        if signals.is_empty() {
            return Ok(());
        }

        for (name, signal) in signals {
            if crate::data::is_known_npc_trainer(&name) {
                continue;
            }
            // Dedup on (name, signal) so a later, stronger signal still reaches the DB.
            let key = format!("{}\u{0}{}", name.to_lowercase(), signal.as_str());
            if !self.known_players_seen.borrow_mut().insert(key) {
                continue;
            }
            self.db.upsert_known_player(&name, signal, date_str)?;
        }
        Ok(())
    }
```

Then call it from the per-line loop. In the `for line in content.lines()` body, immediately after `date_str` is computed (just before the `if let Some(caps) = patterns::WELCOME_LOGIN.captures(message)` block), insert:

```rust
            // Behavioural player detection — orthogonal to `classify_line`, since a line can
            // be both a classified event and a player signal.
            self.record_player_signals(message, &date_str)?;
```

No `use` statement is needed: `player_signals` is declared with `pub mod player_signals;` in this same file, so it is already in scope as `player_signals::…`. Adding `use self::player_signals;` would be an E0255 name conflict.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p amanuensis-core scan_detects_player_trainers scan_records_share_list`
Expected: PASS — 2 tests.

Then: `cargo test -p amanuensis-core`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/amanuensis-core/src/parser/mod.rs
git commit -m "feat(parser): record known players during scanning"
```

---

### Task 6: Filter checkpoints at query time

**Files:**
- Modify: `crates/amanuensis-core/src/models/checkpoint.rs`
- Modify: `crates/amanuensis-core/src/db/queries/checkpoint.rs`

**Interfaces:**
- Consumes: the `known_players` table (Task 3).
- Produces:
  - `TrainerCheckpoint` gains `pub is_player: bool` (last field, after `timestamp`).
  - `get_latest_trainer_checkpoints(&self, char_id: i64, include_players: bool)`
  - `get_all_trainer_checkpoints(&self, char_id: i64, include_players: bool)`
  - `get_trainer_checkpoint_history(&self, char_id: i64, trainer_name: &str, include_players: bool)`

All three gain the same `LEFT JOIN known_players`. Because `known_players.name` is lowercased, the join condition lowercases `trainer_name`.

- [ ] **Step 1: Write the failing test**

Add to the `tests` module in `crates/amanuensis-core/src/db/queries/checkpoint.rs`:

```rust
    use crate::parser::player_signals::PlayerSignal;

    #[test]
    fn player_checkpoints_are_hidden_by_default() {
        let db = Database::open_in_memory().unwrap();
        let char_id = db.get_or_create_character("Ruuk").unwrap();

        db.insert_trainer_checkpoint(char_id, "Histia", 100, Some(149), "2024-01-01 12:00:00").unwrap();
        db.insert_trainer_checkpoint(char_id, "Fenwick", 50, Some(99), "2024-01-02 12:00:00").unwrap();
        db.upsert_known_player("Fenwick", PlayerSignal::Clanning, "2024-01-02 12:00:00").unwrap();

        let hidden = db.get_all_trainer_checkpoints(char_id, false).unwrap();
        assert_eq!(hidden.len(), 1);
        assert_eq!(hidden[0].trainer_name, "Histia");
        assert!(!hidden[0].is_player);

        let shown = db.get_all_trainer_checkpoints(char_id, true).unwrap();
        assert_eq!(shown.len(), 2);
        let fenwick = shown.iter().find(|c| c.trainer_name == "Fenwick").unwrap();
        assert!(fenwick.is_player, "Fenwick should be marked as a player when included");
    }

    #[test]
    fn filtering_is_case_insensitive() {
        let db = Database::open_in_memory().unwrap();
        let char_id = db.get_or_create_character("Ruuk").unwrap();

        // Checkpoint stores the display casing; known_players stores lowercase.
        db.insert_trainer_checkpoint(char_id, "Bramwell Gorse", 0, Some(9), "2024-01-01 12:00:00").unwrap();
        db.upsert_known_player("bramwell gorse", PlayerSignal::Offer, "2024-01-01 12:00:00").unwrap();

        assert!(db.get_all_trainer_checkpoints(char_id, false).unwrap().is_empty());
        assert_eq!(db.get_all_trainer_checkpoints(char_id, true).unwrap().len(), 1);
    }

    #[test]
    fn latest_and_history_also_filter() {
        let db = Database::open_in_memory().unwrap();
        let char_id = db.get_or_create_character("Ruuk").unwrap();

        db.insert_trainer_checkpoint(char_id, "Fenwick", 50, Some(99), "2024-01-01 12:00:00").unwrap();
        db.insert_trainer_checkpoint(char_id, "Histia", 10, Some(19), "2024-01-01 12:00:00").unwrap();
        db.upsert_known_player("Fenwick", PlayerSignal::Ledger, "2024-01-01 12:00:00").unwrap();

        let latest = db.get_latest_trainer_checkpoints(char_id, false).unwrap();
        assert_eq!(latest.len(), 1);
        assert_eq!(latest[0].trainer_name, "Histia");
        assert_eq!(db.get_latest_trainer_checkpoints(char_id, true).unwrap().len(), 2);

        assert!(db.get_trainer_checkpoint_history(char_id, "Fenwick", false).unwrap().is_empty());
        assert_eq!(db.get_trainer_checkpoint_history(char_id, "Fenwick", true).unwrap().len(), 1);
    }

    #[test]
    fn retroactively_learning_a_player_hides_old_checkpoints() {
        let db = Database::open_in_memory().unwrap();
        let char_id = db.get_or_create_character("Ruuk").unwrap();
        db.insert_trainer_checkpoint(char_id, "Fenwick", 50, Some(99), "2017-11-22 23:07:16").unwrap();

        // Before detection: Fenwick looks like a trainer.
        assert_eq!(db.get_all_trainer_checkpoints(char_id, false).unwrap().len(), 1);

        // Learning it later must clean history without touching the rows.
        db.upsert_known_player("Fenwick", PlayerSignal::Clanning, "2017-11-22 23:07:16").unwrap();
        assert!(db.get_all_trainer_checkpoints(char_id, false).unwrap().is_empty());

        let stored: i64 = db
            .conn_for_test()
            .query_row("SELECT COUNT(*) FROM trainer_checkpoints", [], |r| r.get(0))
            .unwrap();
        assert_eq!(stored, 1, "checkpoint rows must never be deleted");
    }
```

Update the four **existing** tests in that module to pass the new argument: `get_latest_trainer_checkpoints(char_id, true)` and `get_all_trainer_checkpoints(char_id, true)`.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p amanuensis-core queries::checkpoint`
Expected: FAIL — compile error, `this method takes 2 arguments but 1 argument was supplied`.

- [ ] **Step 3: Write minimal implementation**

Add the field to `crates/amanuensis-core/src/models/checkpoint.rs`:

```rust
    pub timestamp: String,
    /// True when the greeting came from a player-run ("ledger") trainer rather than an NPC.
    /// Always false in results fetched with `include_players = false`, since those are excluded.
    pub is_player: bool,
```

In `crates/amanuensis-core/src/db/queries/checkpoint.rs`, add this shared fragment above `impl Database`:

```rust
/// Joined onto every checkpoint query so player-run trainers can be identified and filtered.
/// `known_players.name` is stored lowercased, hence the LOWER() on the join.
const PLAYER_JOIN: &str = "LEFT JOIN known_players kp ON kp.name = LOWER(tc.trainer_name)";

/// Appended to the WHERE clause unless players are being included.
const PLAYER_FILTER: &str = " AND kp.name IS NULL";
```

Rewrite the three query methods. `get_latest_trainer_checkpoints`:

```rust
    /// Get the most recent checkpoint for each trainer for a character.
    /// "Most recent" is by log timestamp (the date/time from the log file), not insertion order.
    /// Player-run trainers are excluded unless `include_players` is set.
    pub fn get_latest_trainer_checkpoints(
        &self,
        char_id: i64,
        include_players: bool,
    ) -> Result<Vec<TrainerCheckpoint>> {
        let sql = format!(
            "SELECT tc.id, tc.character_id, tc.trainer_name, tc.rank_min, tc.rank_max,
                    tc.timestamp, kp.name IS NOT NULL AS is_player
             FROM trainer_checkpoints tc
             {PLAYER_JOIN}
             WHERE tc.character_id = ?1
               AND tc.rowid = (
                 SELECT t2.rowid FROM trainer_checkpoints t2
                 WHERE t2.character_id = tc.character_id
                   AND t2.trainer_name = tc.trainer_name
                 ORDER BY t2.timestamp DESC, t2.id DESC
                 LIMIT 1
               ){filter}
             ORDER BY tc.trainer_name",
            filter = if include_players { "" } else { PLAYER_FILTER },
        );

        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(params![char_id], map_checkpoint_row)?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }
```

`get_all_trainer_checkpoints`:

```rust
    /// Get all checkpoint events for a character, sorted by timestamp ascending.
    /// Used for the checkpoint progression timeline graph.
    /// Player-run trainers are excluded unless `include_players` is set.
    pub fn get_all_trainer_checkpoints(
        &self,
        char_id: i64,
        include_players: bool,
    ) -> Result<Vec<TrainerCheckpoint>> {
        let sql = format!(
            "SELECT tc.id, tc.character_id, tc.trainer_name, tc.rank_min, tc.rank_max,
                    tc.timestamp, kp.name IS NOT NULL AS is_player
             FROM trainer_checkpoints tc
             {PLAYER_JOIN}
             WHERE tc.character_id = ?1{filter}
             ORDER BY tc.timestamp ASC, tc.id ASC",
            filter = if include_players { "" } else { PLAYER_FILTER },
        );

        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(params![char_id], map_checkpoint_row)?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }
```

`get_trainer_checkpoint_history`:

```rust
    /// Get full checkpoint history for a specific trainer and character.
    /// Player-run trainers are excluded unless `include_players` is set.
    pub fn get_trainer_checkpoint_history(
        &self,
        char_id: i64,
        trainer_name: &str,
        include_players: bool,
    ) -> Result<Vec<TrainerCheckpoint>> {
        let sql = format!(
            "SELECT tc.id, tc.character_id, tc.trainer_name, tc.rank_min, tc.rank_max,
                    tc.timestamp, kp.name IS NOT NULL AS is_player
             FROM trainer_checkpoints tc
             {PLAYER_JOIN}
             WHERE tc.character_id = ?1 AND tc.trainer_name = ?2{filter}
             ORDER BY tc.id ASC",
            filter = if include_players { "" } else { PLAYER_FILTER },
        );

        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(params![char_id, trainer_name], map_checkpoint_row)?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }
```

Add the shared row mapper at the bottom of the file, outside `impl Database`:

```rust
/// Shared row -> TrainerCheckpoint mapping for the three checkpoint queries,
/// which all select the same seven columns in the same order.
fn map_checkpoint_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<TrainerCheckpoint> {
    Ok(TrainerCheckpoint {
        id: Some(row.get(0)?),
        character_id: row.get(1)?,
        trainer_name: row.get(2)?,
        rank_min: row.get(3)?,
        rank_max: row.get(4)?,
        timestamp: row.get(5)?,
        is_player: row.get(6)?,
    })
}
```

Now fix every remaining call site so the crate compiles. Update these to pass `false` (the GUI/CLI defaults are wired properly in Tasks 8 and 10):

- `crates/amanuensis-gui/src/commands/rank.rs:71` and `:83`
- `crates/amanuensis-cli/src/main.rs:1486` and `:1488`

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p amanuensis-core queries::checkpoint`
Expected: PASS — 8 tests.

Then the whole workspace: `cargo test`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/amanuensis-core/src/models/checkpoint.rs crates/amanuensis-core/src/db/queries/checkpoint.rs crates/amanuensis-gui/src/commands/rank.rs crates/amanuensis-cli/src/main.rs
git commit -m "feat(db): filter player-trainer checkpoints at query time"
```

---

### Task 7: Diagnostic for undetected players

**Files:**
- Modify: `crates/amanuensis-core/src/parser/mod.rs` (`LogParser` struct, `LogParser::new`, the `LogEvent::TrainerCheckpoint` and `LogEvent::TrainerCheckpointUnhailed` arms around lines 678-717, and `finalize_characters` at line 1699)

**Interfaces:**
- Consumes: `is_known_player` (Task 3), `is_known_npc_trainer` (Task 1).
- Produces: `checkpoint_speakers: RefCell<HashSet<String>>` field on `LogParser`. No new public API.

The residual risk the whole design cannot eliminate: a player trainer who never clans, shares, thinks to you, or makes an offer anywhere in the logs emits no signal and is silently treated as an NPC. This makes such names visible instead.

Emitting the diagnostic is deferred to `finalize_characters` — the universal end-of-scan hook, called by `scan_sources`, the CLI, and the GUI — for two reasons: the name may only be identified as a player *later* in the scan, and a name appearing in 742 files must not produce 742 log entries.

- [ ] **Step 1: Write the failing test**

Add to the `tests` module at the bottom of `crates/amanuensis-core/src/parser/mod.rs`:

```rust
    #[test]
    fn unrecognised_checkpoint_speaker_is_logged_once() {
        let dir = tempfile::tempdir().unwrap();
        let char_dir = dir.path().join("Ruuk");
        std::fs::create_dir_all(&char_dir).unwrap();
        // "Brindle" gives no player signal at all — exactly the blind spot this warns about.
        // Three checkpoints must still produce only one log entry.
        std::fs::write(
            char_dir.join("CL Log 11.22.17"),
            concat!(
                "11/22/17 10:00:00p Welcome to Clan Lord, Ruuk!\n",
                "11/22/17 10:01:00p Brindle says, \"Hail, Ruuk. You keep me on my toes.\"\n",
                "11/22/17 10:02:00p Brindle says, \"Hail, Ruuk. You keep me on my toes.\"\n",
                "11/22/17 10:03:00p Histia says, \"Hail, Ruuk. You keep me on my toes.\"\n",
                "11/22/17 10:04:00p Fenwick is now Clanning.\n",
                "11/22/17 10:05:00p Fenwick says, \"Hail, Ruuk. You keep me on my toes.\"\n",
            ),
        )
        .unwrap();

        let db = Database::open_in_memory().unwrap();
        let parser = LogParser::new(db).unwrap();
        parser.scan_folder(dir.path(), false).unwrap();
        parser.finalize_characters().unwrap();

        let logs = parser.db().get_process_logs().unwrap();
        let mork: Vec<_> = logs.iter().filter(|l| l.message.contains("\"Brindle\"")).collect();
        assert_eq!(mork.len(), 1, "one entry per distinct name per scan");
        assert!(mork[0].message.contains("unrecognised"));

        // Guarded NPC trainers and already-detected players must stay silent.
        assert!(!logs.iter().any(|l| l.message.contains("\"Histia\"")));
        assert!(!logs.iter().any(|l| l.message.contains("\"Fenwick\"")));
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p amanuensis-core unrecognised_checkpoint_speaker`
Expected: FAIL — `assertion failed: left == right, left: 0, right: 1`. No diagnostic is emitted yet.

- [ ] **Step 3: Write minimal implementation**

Add the field to the `LogParser` struct:

```rust
    /// Names that produced a checkpoint during this scan. Flushed in `finalize_characters`
    /// into one `process_logs` entry per name that is neither a known player nor a
    /// guarded NPC trainer, so undetectable player trainers become visible.
    checkpoint_speakers: RefCell<HashSet<String>>,
```

and initialise it in `LogParser::new`:

```rust
            checkpoint_speakers: RefCell::new(HashSet::new()),
```

Record the speaker in both checkpoint-insert arms. In the `LogEvent::TrainerCheckpoint` arm, immediately after `self.db.insert_trainer_checkpoint(...)?;`:

```rust
                        self.checkpoint_speakers.borrow_mut().insert(trainer_name.clone());
```

Do the same in the `LogEvent::TrainerCheckpointUnhailed` arm after its `insert_trainer_checkpoint` call.

Add the flush method to `impl LogParser`:

```rust
    /// Warn about checkpoint speakers that are neither known players nor guarded NPC
    /// trainers. Runs once per scan, after all detection, so a name identified late in the
    /// scan is not falsely reported. Clears the buffer so repeated scans stay quiet.
    fn flush_unrecognised_checkpoint_speakers(&self) -> Result<()> {
        let speakers: Vec<String> = self.checkpoint_speakers.borrow_mut().drain().collect();
        let mut unrecognised: Vec<String> = Vec::new();

        for name in speakers {
            if crate::data::is_known_npc_trainer(&name) {
                continue;
            }
            if self.db.is_known_player(&name)? {
                continue;
            }
            unrecognised.push(name);
        }

        unrecognised.sort();
        for name in unrecognised {
            let _ = self.db.add_process_log(
                "info",
                &format!(
                    "Checkpoint from unrecognised trainer \"{name}\" — if this is a player \
                     trainer, it will appear in the checkpoint graph"
                ),
            );
        }
        Ok(())
    }
```

Call it from `finalize_characters`. Add this as the first statement of that method's body:

```rust
        self.flush_unrecognised_checkpoint_speakers()?;
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p amanuensis-core unrecognised_checkpoint_speaker`
Expected: PASS.

Then: `cargo test -p amanuensis-core`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/amanuensis-core/src/parser/mod.rs
git commit -m "feat(parser): warn about unrecognised checkpoint speakers"
```

---

### Task 8: Expose `includePlayers` through Tauri

**Files:**
- Modify: `crates/amanuensis-gui/src/commands/rank.rs:63-86`
- Modify: `crates/amanuensis-gui/ui/src/types.ts:193-200`
- Modify: `crates/amanuensis-gui/ui/src/lib/commands.ts:228-235`

**Interfaces:**
- Consumes: `get_all_trainer_checkpoints` / `get_latest_trainer_checkpoints` with `include_players` (Task 6).
- Produces:
  - Tauri commands `get_all_trainer_checkpoints(charId, includePlayers)` and `get_trainer_checkpoints(charId, includePlayers)`.
  - TS `getAllTrainerCheckpoints(charId: number, includePlayers: boolean): Promise<TrainerCheckpoint[]>`
  - TS `getTrainerCheckpoints(charId: number, includePlayers: boolean): Promise<TrainerCheckpoint[]>`
  - TS `TrainerCheckpoint` gains `is_player: boolean`.

There is no Rust test harness for Tauri command wrappers; this task is verified by compilation plus the Task 9 manual check.

- [ ] **Step 1: Update the Tauri commands**

In `crates/amanuensis-gui/src/commands/rank.rs`, replace both command bodies:

```rust
/// Get all trainer rank checkpoints for a character, sorted by timestamp.
/// Used for the checkpoint progression timeline graph.
/// `include_players` reveals checkpoints from player-run ("ledger") trainers.
#[tauri::command]
pub fn get_all_trainer_checkpoints(
    char_id: i64,
    include_players: bool,
    state: State<'_, AppState>,
) -> Result<Vec<amanuensis_core::models::TrainerCheckpoint>, String> {
    state.with_db(|db| {
        db.get_all_trainer_checkpoints(char_id, include_players)
            .map_err(|e| e.to_string())
    })
}

/// Get the most recent trainer rank checkpoint for each trainer for a character.
/// `include_players` reveals checkpoints from player-run ("ledger") trainers.
#[tauri::command]
pub fn get_trainer_checkpoints(
    char_id: i64,
    include_players: bool,
    state: State<'_, AppState>,
) -> Result<Vec<amanuensis_core::models::TrainerCheckpoint>, String> {
    state.with_db(|db| {
        db.get_latest_trainer_checkpoints(char_id, include_players)
            .map_err(|e| e.to_string())
    })
}
```

- [ ] **Step 2: Update the TypeScript types and wrappers**

In `crates/amanuensis-gui/ui/src/types.ts`, extend the interface:

```typescript
/** Mirrors Rust `TrainerCheckpoint` struct */
export interface TrainerCheckpoint {
  id: number | null;
  character_id: number;
  trainer_name: string;
  rank_min: number;
  rank_max: number | null;
  timestamp: string;
  /** True when the greeting came from a player-run ("ledger") trainer, not an NPC. */
  is_player: boolean;
}
```

In `crates/amanuensis-gui/ui/src/lib/commands.ts`, replace both wrappers:

```typescript
export async function getTrainerCheckpoints(
  charId: number,
  includePlayers: boolean,
): Promise<TrainerCheckpoint[]> {
  return invoke("get_trainer_checkpoints", { charId, includePlayers });
}

export async function getAllTrainerCheckpoints(
  charId: number,
  includePlayers: boolean,
): Promise<TrainerCheckpoint[]> {
  return invoke("get_all_trainer_checkpoints", { charId, includePlayers });
}
```

- [ ] **Step 3: Verify the Rust side compiles**

Run: `cargo build -p amanuensis-gui`
Expected: SUCCESS.

The two TypeScript call sites (`CVGraphView.tsx`, `RankModifiersView.tsx`) will now fail typechecking because they pass one argument. Task 9 fixes both; do not run `tsc -b` until then.

- [ ] **Step 4: Commit**

```bash
git add crates/amanuensis-gui/src/commands/rank.rs crates/amanuensis-gui/ui/src/types.ts crates/amanuensis-gui/ui/src/lib/commands.ts
git commit -m "feat(gui): thread includePlayers through the checkpoint commands"
```

---

### Task 9: GUI toggle and player marking

**Files:**
- Modify: `crates/amanuensis-gui/ui/src/lib/constants.ts` (`STORAGE_KEYS`)
- Modify: `crates/amanuensis-gui/ui/src/lib/store.ts` (state interface and store body, near the `indexLogLines` pref around line 248)
- Modify: `crates/amanuensis-gui/ui/src/components/views/CVGraphView.tsx`
- Modify: `crates/amanuensis-gui/ui/src/components/views/RankModifiersView.tsx:161-175`

**Interfaces:**
- Consumes: `getAllTrainerCheckpoints(charId, includePlayers)`, `getTrainerCheckpoints(charId, includePlayers)`, `TrainerCheckpoint.is_player` (Task 8).
- Produces: `showPlayerTrainers: boolean` and `setShowPlayerTrainers: (show: boolean) => void` on the Zustand store, persisted to `localStorage`.

Critical detail: `CheckpointTooltip` and `CheckpointDot` index `payload[`${name}_evt`]` using the Recharts series `name`. Do **not** append "(player)" to the `name` prop — that would break the `_evt` lookup. Mark players via the `Legend` `formatter` and a `playerNames` set passed to the tooltip instead.

- [ ] **Step 1: Add the persisted preference**

In `crates/amanuensis-gui/ui/src/lib/constants.ts`, add to `STORAGE_KEYS`:

```typescript
  SHOW_PLAYER_TRAINERS: "amanuensis_show_player_trainers",
```

In `crates/amanuensis-gui/ui/src/lib/store.ts`, add to the state interface next to `indexLogLines`:

```typescript
  showPlayerTrainers: boolean;
  setShowPlayerTrainers: (show: boolean) => void;
```

and to the store body, immediately after the `setIndexLogLines` block:

```typescript
  // Player-run ("ledger") trainers greet exactly like NPC trainers, so they are hidden
  // from checkpoint surfaces by default. Opt in to inspect them.
  showPlayerTrainers: localStorage.getItem(STORAGE_KEYS.SHOW_PLAYER_TRAINERS) === "true",
  setShowPlayerTrainers: (show) => {
    localStorage.setItem(STORAGE_KEYS.SHOW_PLAYER_TRAINERS, String(show));
    set({ showPlayerTrainers: show });
  },
```

- [ ] **Step 2: Wire the checkpoint chart**

In `crates/amanuensis-gui/ui/src/components/views/CVGraphView.tsx`, add `showPlayerTrainers` and `setShowPlayerTrainers` to the `useStore()` destructuring at line 523:

```typescript
  const { kills, trainers, lastys, characters, selectedCharacterId, showPlayerTrainers, setShowPlayerTrainers } = useStore();
```

Replace the checkpoint fetch effect (lines 532-539) so it refetches when the preference changes:

```typescript
  useEffect(() => {
    if (selectedCharacterId == null) {
      setAllCheckpoints([]);
      return;
    }
    getAllTrainerCheckpoints(selectedCharacterId, showPlayerTrainers)
      .then(setAllCheckpoints)
      .catch(() => {});
  }, [selectedCharacterId, showPlayerTrainers]);
```

Add a derived set of player names, next to the other `useMemo` hooks:

```typescript
  // Names revealed as player-run trainers, used to badge them in the legend and tooltip.
  const playerNames = useMemo(
    () => new Set(allCheckpoints.filter((cp) => cp.is_player).map((cp) => cp.trainer_name)),
    [allCheckpoints],
  );
```

Update `CheckpointTooltip` (line 474) to accept and render the badge. Replace its signature and the entry row:

```typescript
function CheckpointTooltip({ active, payload, label, playerNames }: { active?: boolean; payload?: Array<{ value: number | null; name: string; color: string; payload: TrainerPoint }>; label?: string; playerNames?: Set<string> }) {
```

and inside the `entries.map` callback, replace the returned row with:

```typescript
        return (
          <div key={e.name} style={{ color: e.color }}>
            {e.name}
            {playerNames?.has(e.name) && (
              <span className="ml-1 rounded-full bg-[var(--color-border)] px-1.5 py-0.5 text-[10px] uppercase tracking-wide text-[var(--color-text-muted)]">
                player
              </span>
            )}
            : <span className="font-semibold">≥{e.value}</span>
            {isEvent && <span className="ml-1 text-xs opacity-70">● observed</span>}
          </div>
        );
```

In the chart JSX (lines 680-704), pass the set to the tooltip and add a legend formatter:

```tsx
              <Tooltip content={<CheckpointTooltip playerNames={playerNames} />} />
              <Legend
                wrapperStyle={{ color: "var(--color-text-muted)", fontSize: 11 }}
                formatter={(value: string) =>
                  playerNames.has(value) ? `${value} (player)` : value
                }
              />
```

Finally add the checkbox. Replace the closing caption paragraph (line 701-703) with:

```tsx
          <div className="mt-1 flex flex-wrap items-center justify-between gap-2">
            <p className="text-xs text-[var(--color-text-muted)]">
              Observed rank minimums from trainer greeting messages. Each dot is an actual checkpoint recorded from logs.
            </p>
            <label className="flex shrink-0 items-center gap-1.5 text-xs text-[var(--color-text-muted)]">
              <input
                type="checkbox"
                checked={showPlayerTrainers}
                onChange={(e) => setShowPlayerTrainers(e.target.checked)}
              />
              Show player trainers
            </label>
          </div>
```

- [ ] **Step 3: Wire the Rank Modifiers badges**

In `crates/amanuensis-gui/ui/src/components/views/RankModifiersView.tsx`, add `showPlayerTrainers` to the `useStore()` destructuring at line 145:

```typescript
  const { trainers, setTrainers, setCharacters, selectedCharacterId, rankModifiersViewState, setRankModifiersViewState, showPlayerTrainers } = useStore();
```

and update the checkpoint effect (lines 161-175) to pass it and depend on it:

```typescript
    getTrainerCheckpoints(selectedCharacterId, showPlayerTrainers)
      .then((checkpoints) => {
        const map = new Map<string, TrainerCheckpoint>();
        for (const cp of checkpoints) {
          map.set(cp.trainer_name, cp);
        }
        setCheckpointMap(map);
      })
      .catch(() => {});
  }, [selectedCharacterId, trainers, showPlayerTrainers]);
```

- [ ] **Step 4: Typecheck**

Stop `cargo tauri dev` if it is running, then:

Run: `cd crates/amanuensis-gui/ui && npm run build`
Expected: SUCCESS, no TypeScript errors.

- [ ] **Step 5: Verify in the app**

Run: `cd crates/amanuensis-gui && cargo tauri dev`

Open **CV Graph** for a character with checkpoints and confirm:
1. The Trainer Checkpoint Progression legend contains no player names by default.
2. Ticking "Show player trainers" adds them, each suffixed `(player)` in the legend.
3. Hovering a player series shows the `player` pill in the tooltip.
4. The setting survives an app restart.

- [ ] **Step 6: Commit**

```bash
git add crates/amanuensis-gui/ui/src/lib/constants.ts crates/amanuensis-gui/ui/src/lib/store.ts crates/amanuensis-gui/ui/src/components/views/CVGraphView.tsx crates/amanuensis-gui/ui/src/components/views/RankModifiersView.tsx
git commit -m "feat(gui): Show player trainers toggle on checkpoint surfaces"
```

---

### Task 10: CLI `--include-players`

**Files:**
- Modify: `crates/amanuensis-cli/src/main.rs` (the `Checkpoints` variant at lines 217-228, its dispatch at lines 454-456, and `cmd_checkpoints` at lines 1475-1521)

**Interfaces:**
- Consumes: `get_all_trainer_checkpoints` / `get_latest_trainer_checkpoints` with `include_players` (Task 6).
- Produces: `amanuensis checkpoints <name> [--all] [--trainer T] [--include-players]`.

- [ ] **Step 1: Write the failing test**

Add to the clap smoke tests in `crates/amanuensis-cli/src/main.rs`:

```rust
    #[test]
    fn checkpoints_accepts_include_players() {
        let cli = Cli::try_parse_from([
            "amanuensis", "checkpoints", "Ruuk", "--include-players",
        ])
        .unwrap();
        match cli.command {
            Commands::Checkpoints { name, include_players, .. } => {
                assert_eq!(name, "Ruuk");
                assert!(include_players);
            }
            _ => panic!("expected Checkpoints"),
        }
    }

    #[test]
    fn checkpoints_hides_players_by_default() {
        let cli = Cli::try_parse_from(["amanuensis", "checkpoints", "Ruuk"]).unwrap();
        match cli.command {
            Commands::Checkpoints { include_players, .. } => assert!(!include_players),
            _ => panic!("expected Checkpoints"),
        }
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p amanuensis-cli checkpoints_accepts_include_players`
Expected: FAIL — compile error, `struct variant Commands::Checkpoints has no field named include_players`.

- [ ] **Step 3: Write minimal implementation**

Add the flag to the `Checkpoints` variant:

```rust
    /// Show trainer rank checkpoints for a character
    Checkpoints {
        /// Character name
        name: String,
        /// Show all historical checkpoints (default: latest per trainer only)
        #[arg(long)]
        all: bool,
        /// Filter to a specific trainer
        #[arg(long)]
        trainer: Option<String>,
        /// Include checkpoints from player-run ("ledger") trainers, which are hidden by default
        #[arg(long)]
        include_players: bool,
    },
```

Update the dispatch:

```rust
        Commands::Checkpoints { name, all, trainer, include_players } => {
            cmd_checkpoints(&db_path, &name, all, trainer.as_deref(), include_players)
        }
```

Update the function signature and the two query calls:

```rust
fn cmd_checkpoints(
    db_path: &str,
    name: &str,
    all: bool,
    trainer_filter: Option<&str>,
    include_players: bool,
) -> amanuensis_core::Result<()> {
    let db = Database::open(db_path)?;
    let char = resolve_character(&db, name)?;
    let char_id = char.id.unwrap();

    let mut checkpoints = if all {
        db.get_all_trainer_checkpoints(char_id, include_players)?
    } else {
        db.get_latest_trainer_checkpoints(char_id, include_players)?
    };
```

Mark players in the output. In the `for c in &checkpoints` loop around line 1509, suffix the trainer name when `c.is_player` is true — for example, where the loop currently prints `c.trainer_name`, print:

```rust
        let display_name = if c.is_player {
            format!("{} (player)", c.trainer_name)
        } else {
            c.trainer_name.clone()
        };
```

and use `display_name` in that row's output instead of `c.trainer_name`.

Extend the empty-result hint so the filter is discoverable:

```rust
    if checkpoints.is_empty() {
        println!("No trainer checkpoints found for {}.", name);
        println!("Hint: Checkpoints are recorded when a trainer greets you with a rank-status message.");
        if !include_players {
            println!("Hint: player-run trainers are hidden; pass --include-players to show them.");
        }
        return Ok(());
    }
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p amanuensis-cli`
Expected: PASS — 9 tests.

- [ ] **Step 5: Commit**

```bash
git add crates/amanuensis-cli/src/main.rs
git commit -m "feat(cli): --include-players on the checkpoints command"
```

---

### Task 11: Real-data verification and documentation

**Files:**
- Modify: `crates/amanuensis-core/tests/real_data_comparison.rs`
- Modify: `CLAUDE.md`

**Interfaces:**
- Consumes: everything from Tasks 1-10.
- Produces: an `#[ignore]`d test `player_detection_matches_reference_corpus`, plus a new CLAUDE.md subsection.

The reference corpus is `~/Applications/Clan Lords/Text Logs/` (1,160 log files, 10 characters, 261k lines). Scanning it must flag exactly `Fenwick`, `Bramwell Gorse`, and `Tallow` as players, and must not flag any of the legitimate trainer names present.

This test uses the file's existing `scan_logs_to_temp()` helper, which scans the real logs into a fresh temp database. That exercises the live scan path from Task 5 and touches nothing the user owns — do **not** open or mutate `~/Library/Application Support/com.dfsw.Amanuensis/amanuensis.db`.

- [ ] **Step 1: Write the failing test**

Append to `crates/amanuensis-core/tests/real_data_comparison.rs`, following the `#[ignore]` / `skip_if_missing` conventions already used throughout that file:

```rust
// ---------------------------------------------------------------------------
// Player-trainer detection
// ---------------------------------------------------------------------------

/// The reference corpus contains exactly three player-run ("ledger") trainers. Behavioural
/// detection must find all three and must never flag a legitimate NPC trainer — especially
/// the display-name variants that `trainers.json` does not contain.
#[test]
#[ignore]
fn player_detection_matches_reference_corpus() {
    if skip_if_missing(&text_logs_path()) {
        return;
    }
    let (db, _tmp) = scan_logs_to_temp();

    // list_known_players returns lowercased names (the table's primary key).
    let players: std::collections::HashSet<String> = db
        .list_known_players()
        .unwrap()
        .into_iter()
        .map(|(name, _)| name)
        .collect();

    assert!(!players.is_empty(), "corpus should contain player characters");

    for expected in ["fenwick", "bramwell gorse", "tallow"] {
        assert!(
            players.contains(expected),
            "{expected} should be detected as a player (found {} players)",
            players.len()
        );
    }

    // Zero false positives. Every name here produced real checkpoints in the corpus.
    for trainer in [
        "higgrus", "chronos", "hardia", "splash o'sul", "diggin",
        "anan faure", "andeux faure", "anquart faure", "ansept faure", "antrix faure",
        "tra'kning", "par troon", "metta sylpha", "respin verminbane",
        "histia", "evus", "swengus", "atkus", "darkus", "regia", "balthus", "detha",
        "duvin beastlore", "sprite", "master bodrus", "master mentus", "master spirtus",
    ] {
        assert!(!players.contains(trainer), "{trainer} must not be flagged as a player");
    }
}

/// The three known player trainers must be absent from the checkpoint graph by default,
/// and present once players are included.
#[test]
#[ignore]
fn player_checkpoints_are_excluded_from_the_graph_by_default() {
    if skip_if_missing(&text_logs_path()) {
        return;
    }
    let (db, _tmp) = scan_logs_to_temp();

    for character in db.list_characters().unwrap() {
        let char_id = character.id.unwrap();
        for cp in db.get_all_trainer_checkpoints(char_id, false).unwrap() {
            let lower = cp.trainer_name.to_lowercase();
            assert!(
                !matches!(lower.as_str(), "fenwick" | "bramwell gorse" | "tallow"),
                "{} leaked into the default checkpoint view for {}",
                cp.trainer_name,
                character.name
            );
            assert!(!cp.is_player);
        }
    }
}
```

- [ ] **Step 2: Run the tests**

Run: `cargo test -p amanuensis-core --test real_data_comparison player_detection -- --ignored --nocapture`
Expected: PASS.

Run: `cargo test -p amanuensis-core --test real_data_comparison player_checkpoints -- --ignored --nocapture`
Expected: PASS.

If a name in the first assertion block is reported missing, the local corpus differs from the reference. Confirm against the live database before weakening the assertion — this query reproduces the expected three:

```bash
sqlite3 ~/Library/Application\ Support/com.dfsw.Amanuensis/amanuensis.db \
  "SELECT name, evidence FROM known_players ORDER BY name;"
```

- [ ] **Step 3: Confirm the full suite**

Run: `cargo test`
Expected: PASS.

Run: `cargo test -p amanuensis-core --test real_data_comparison -- --ignored`
Expected: PASS — 25 tests (23 existing plus the two new ones).

- [ ] **Step 4: Document the behaviour**

Add a numbered item to the **Key Functional Areas** list in `CLAUDE.md`, after item 11:

```markdown
12. **Player-trainer filtering**: players who own a training ledger can teach others, and they greet with the *exact* same wording as NPC trainers (`Fenwick says, "Hail, Ruuk. You are one of my better pupils."`), so their greetings used to become first-class series in the Trainer Checkpoint Progression graph. Players are now identified **behaviourally** by ten line forms only player characters produce — clanning on/off, the three experience-sharing forms, `thinks to you`, the two ledger-write forms, `shows their training ledger`, `You begin training with X`, and the `¥`/`•` teacher offer/accept pair (`parser/player_signals.rs`) — and recorded in the global `known_players` table (name lowercased as the PK, plus `evidence` and `first_seen`). A bundled **additive-only** guard list (`data/npc_trainers.rs`, derived from `trainers.json`'s short names plus observed display-name variants such as `Higgrus`, `Chronos`, `Splash O'Sul`, `Tra'Kning`, `Par Troon`, `Metta Sylpha`, the Faures) prevents a real trainer from ever being flagged; being absent from that list flags nobody. **Filtering happens at query time**, not scan time: `get_all_trainer_checkpoints` / `get_latest_trainer_checkpoints` / `get_trainer_checkpoint_history` take `include_players` and `LEFT JOIN known_players`, so newly-learned players retroactively disappear from the graph with **no rescan**, and `trainer_checkpoints` rows are never deleted or altered. Surfaced as a **"Show player trainers"** checkbox on the CV Graph checkpoint card (default off, persisted in localStorage, also governing the Rank Modifiers checkpoint badges; revealed series are suffixed `(player)`), and as `amanuensis checkpoints <char> --include-players`. **Existing databases are backfilled from the stored `log_lines` index on first open** (guarded by a `known_players_backfilled` marker in the new `db_meta` table), so no rescan is needed — unless log indexing was disabled, in which case `known_players` fills on the next Update/Rescan. Checkpoints from a speaker that is neither a known player nor guarded emit one `info` `process_logs` entry per name per scan, so an undetectable player trainer becomes visible rather than silently polluting the graph. Note `reset_log_data` now also clears `trainer_checkpoints` (it clears `log_files`, so every file re-scanned and duplicated every checkpoint row) and `known_players`. Translating a player trainer to the NPC they stand in for was considered and rejected — the `¥ X offers: "I can teach you …"` line does identify the trainer, but players who only ever hailed you (e.g. `Fenwick`) can never be translated; see `docs/superpowers/specs/2026-07-29-player-trainers-and-bestiary-links-design.md`.
```

- [ ] **Step 5: Commit**

```bash
git add crates/amanuensis-core/tests/real_data_comparison.rs CLAUDE.md
git commit -m "test: verify player detection against the reference corpus; document behaviour"
```

---

### Task 12: Bestiary family-page URL helper

**Files:**
- Modify: `crates/amanuensis-gui/ui/src/lib/bestiary.ts`

**Interfaces:**
- Consumes: nothing.
- Produces: `export function familyPageUrl(family: string | null | undefined): string | null`.

Verified against the live site: family pages live at `https://bestiary.clanlord.net/beast/<FamilyWithSpacesRemoved>.php` and respond `200` for 63 of our 66 families — including `AstralElemental.php`, which is missing from the site's own index. The families `Extinct` (31 entries) and `EXTINCT` (1 entry) have **no page**; both spellings return `404`. There are no per-creature anchors, and `search.php` is AJAX-only with no linkable results URL, so the family page is the only available target.

- [ ] **Step 1: Add the helper**

Append to `crates/amanuensis-gui/ui/src/lib/bestiary.ts`:

```typescript
/**
 * URL of the upstream bestiary page listing a family's creatures, or null when no such
 * page exists.
 *
 * The bestiary has no per-creature pages and no per-creature anchors, and its search is
 * AJAX-only with no linkable results URL — so the family page is the only available
 * target. Extinct creatures have no family page at all (both "Extinct" and "EXTINCT"
 * 404), hence the null.
 */
export function familyPageUrl(family: string | null | undefined): string | null {
  if (!family) return null;
  const trimmed = family.trim();
  if (!trimmed || trimmed.toUpperCase() === "EXTINCT") return null;
  const slug = trimmed.replace(/\s+/g, "");
  if (!slug) return null;
  return `https://bestiary.clanlord.net/beast/${slug}.php`;
}
```

- [ ] **Step 2: Verify the mapping by hand**

Run:

```bash
for f in Feline AstralElemental Darshak Uncategorized; do
  printf "%s: " "$f"
  curl -s -o /dev/null -w "%{http_code}\n" "https://bestiary.clanlord.net/beast/$f.php"
done
```

Expected: `200` for all four. This confirms the space-stripping slug (`Astral Elemental` → `AstralElemental`) resolves.

- [ ] **Step 3: Typecheck**

Stop `cargo tauri dev` if running, then:

Run: `cd crates/amanuensis-gui/ui && npm run build`
Expected: SUCCESS.

- [ ] **Step 4: Commit**

```bash
git add crates/amanuensis-gui/ui/src/lib/bestiary.ts
git commit -m "feat(gui): bestiary family-page URL helper"
```

---

### Task 13: Shared `ContextMenu` component

**Files:**
- Create: `crates/amanuensis-gui/ui/src/components/shared/ContextMenu.tsx`

**Interfaces:**
- Consumes: nothing.
- Produces:

```typescript
export interface ContextMenuItem {
  label: string;
  onSelect: () => void;
  disabled?: boolean;
}

export interface ContextMenuProps {
  x: number;
  y: number;
  items: ContextMenuItem[];
  onClose: () => void;
}

export function ContextMenu(props: ContextMenuProps): React.ReactElement;
```

Written generically so KillsView can adopt it later. Closes on outside click, `Escape`, scroll, and window resize. Clamps to the viewport so a right-click near the bottom-right edge does not open off-screen.

- [ ] **Step 1: Create the component**

Create `crates/amanuensis-gui/ui/src/components/shared/ContextMenu.tsx`:

```tsx
import { useEffect, useLayoutEffect, useRef, useState } from "react";

export interface ContextMenuItem {
  label: string;
  onSelect: () => void;
  disabled?: boolean;
}

export interface ContextMenuProps {
  /** Viewport coordinates of the originating right-click. */
  x: number;
  y: number;
  items: ContextMenuItem[];
  onClose: () => void;
}

/**
 * Cursor-positioned context menu. Generic on purpose — any view can supply items.
 * Dismisses on outside click, Escape, scroll, and resize.
 */
export function ContextMenu({ x, y, items, onClose }: ContextMenuProps) {
  const ref = useRef<HTMLDivElement>(null);
  const [pos, setPos] = useState({ x, y });

  // Clamp into the viewport once the real size is known.
  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;
    const { width, height } = el.getBoundingClientRect();
    const margin = 8;
    setPos({
      x: Math.min(x, window.innerWidth - width - margin),
      y: Math.min(y, window.innerHeight - height - margin),
    });
  }, [x, y]);

  useEffect(() => {
    const onPointerDown = (e: MouseEvent) => {
      if (!ref.current?.contains(e.target as Node)) onClose();
    };
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    // `true` captures scrolls inside nested scroll containers too.
    document.addEventListener("mousedown", onPointerDown);
    document.addEventListener("keydown", onKeyDown);
    window.addEventListener("scroll", onClose, true);
    window.addEventListener("resize", onClose);
    return () => {
      document.removeEventListener("mousedown", onPointerDown);
      document.removeEventListener("keydown", onKeyDown);
      window.removeEventListener("scroll", onClose, true);
      window.removeEventListener("resize", onClose);
    };
  }, [onClose]);

  return (
    <div
      ref={ref}
      role="menu"
      style={{ left: pos.x, top: pos.y }}
      className="fixed z-50 min-w-44 overflow-hidden rounded-md border border-[var(--color-border)] bg-[var(--color-card)] py-1 shadow-lg"
    >
      {items.map((item, i) => (
        <button
          key={i}
          role="menuitem"
          disabled={item.disabled}
          onClick={() => {
            item.onSelect();
            onClose();
          }}
          className="block w-full px-3 py-1.5 text-left text-sm text-[var(--color-text)] enabled:hover:bg-[var(--color-accent)] enabled:hover:text-white disabled:cursor-default disabled:text-[var(--color-text-muted)] disabled:opacity-60"
        >
          {item.label}
        </button>
      ))}
    </div>
  );
}
```

- [ ] **Step 2: Typecheck**

Stop `cargo tauri dev` if running, then:

Run: `cd crates/amanuensis-gui/ui && npm run build`
Expected: SUCCESS.

- [ ] **Step 3: Commit**

```bash
git add crates/amanuensis-gui/ui/src/components/shared/ContextMenu.tsx
git commit -m "feat(gui): shared cursor-positioned ContextMenu component"
```

---

### Task 14: Right-click bestiary link in Top Targets

**Files:**
- Modify: `crates/amanuensis-gui/ui/src/components/views/ranger/TopTargetsPanel.tsx`

**Interfaces:**
- Consumes: `familyPageUrl` (Task 12), `ContextMenu` / `ContextMenuItem` (Task 13), `open` from `@tauri-apps/plugin-shell`.
- Produces: no new exports.

`shell.open` is already a dependency (`@tauri-apps/plugin-shell` in `package.json`), already permitted for `https://` by `shell:default` in `crates/amanuensis-gui/capabilities/default.json`, and already used in `UpdateBanner.tsx`. No plugin or capability changes are needed.

`buildColumns` is a module-level function, so the creature cell needs the right-click handler passed in.

- [ ] **Step 1: Wire the menu**

In `crates/amanuensis-gui/ui/src/components/views/ranger/TopTargetsPanel.tsx`, extend the imports at the top:

```typescript
import { useState, useMemo, useCallback } from "react";
import { createColumnHelper, type ColumnDef } from "@tanstack/react-table";
import { open } from "@tauri-apps/plugin-shell";
import { DataTable } from "../../shared/DataTable";
import { CreatureImage } from "../../shared/CreatureImage";
import { ContextMenu, type ContextMenuItem } from "../../shared/ContextMenu";
import { familyPageUrl } from "../../../lib/bestiary";
import type { MorphCandidate, FamilyProgress } from "../../../lib/rangerStats";
```

Change `buildColumns` to accept the handler and attach it to the creature cell. Replace the signature and the `creature_name` accessor:

```tsx
function buildColumns(
  category: TargetCategory,
  onCreatureContextMenu: (e: React.MouseEvent, candidate: MorphCandidate) => void,
): ColumnDef<MorphCandidate, any>[] {
  const cols: ColumnDef<MorphCandidate, any>[] = [
    candidateHelper.accessor("creature_name", {
      header: "Creature",
      cell: (info) => (
        <div
          className="flex cursor-context-menu items-center gap-2"
          onContextMenu={(e) => onCreatureContextMenu(e, info.row.original)}
          title="Right-click for bestiary links"
        >
          <CreatureImage creatureName={info.getValue()} className="h-6 w-6" />
          <span>{info.getValue()}</span>
        </div>
      ),
    }),
```

Inside the `TopTargetsPanel` component, add the menu state and handler after the existing `useState` calls:

```typescript
  const [menu, setMenu] = useState<{ x: number; y: number; candidate: MorphCandidate } | null>(null);

  const handleCreatureContextMenu = useCallback((e: React.MouseEvent, candidate: MorphCandidate) => {
    e.preventDefault();
    setMenu({ x: e.clientX, y: e.clientY, candidate });
  }, []);

  const menuItems = useMemo<ContextMenuItem[]>(() => {
    if (!menu) return [];
    const url = familyPageUrl(menu.candidate.family);
    return [
      url
        ? {
            label: `Open in Bestiary — ${menu.candidate.family}`,
            onSelect: () => {
              open(url).catch(() => {});
            },
          }
        : {
            // Extinct creatures have no family page on the bestiary.
            label: "No bestiary page for this family",
            onSelect: () => {},
            disabled: true,
          },
      {
        label: "Copy creature name",
        onSelect: () => {
          navigator.clipboard.writeText(menu.candidate.creature_name).catch(() => {});
        },
      },
    ];
  }, [menu]);
```

Update the memoised columns to pass the handler:

```typescript
  const columns = useMemo(
    () => buildColumns(category, handleCreatureContextMenu),
    [category, handleCreatureContextMenu],
  );
```

Add the discoverability hint. Extend the existing description paragraph:

```tsx
        <p className="mb-3 text-xs text-[var(--color-text-muted)]">
          {activeCategory.description}
          {effectiveMax > 0 && <span> — showing creatures with value ≤ {effectiveMax.toLocaleString()}</span>}
          <span> — right-click a creature for bestiary links</span>
        </p>
```

Finally render the menu. Immediately before the closing `</div>` of the component's returned root element:

```tsx
      {menu && (
        <ContextMenu x={menu.x} y={menu.y} items={menuItems} onClose={() => setMenu(null)} />
      )}
```

- [ ] **Step 2: Typecheck**

Stop `cargo tauri dev` if running, then:

Run: `cd crates/amanuensis-gui/ui && npm run build`
Expected: SUCCESS.

- [ ] **Step 3: Verify in the app**

Run: `cd crates/amanuensis-gui && cargo tauri dev`

Open **Ranger Stats → Top Targets** for a Ranger character and confirm:
1. Right-clicking a creature name opens the menu at the cursor.
2. **Open in Bestiary — {Family}** opens the correct family page in the system browser.
3. A creature in the `Astral Elemental` family resolves (confirms space-stripping). Use the **Max value** box to widen the list if none is visible.
4. **Copy creature name** puts the name on the clipboard.
5. The menu closes on outside click, `Escape`, and scrolling the table.
6. Right-clicking near the bottom-right of the window keeps the menu on-screen.

For the disabled state, temporarily change `familyPageUrl` to `return null;` at the top, confirm the greyed **No bestiary page for this family** item appears, then revert. (Extinct creatures are not morph candidates, so the state is otherwise unreachable in this view.)

- [ ] **Step 4: Commit**

```bash
git add crates/amanuensis-gui/ui/src/components/views/ranger/TopTargetsPanel.tsx
git commit -m "feat(gui): right-click bestiary link on Top Targets creatures"
```

---

## Out of Scope

Recorded in the spec, deliberately not built here:

- Translating a player trainer to the NPC trainer they stand in for. The `¥ X offers: "I can teach you …"` line does identify the trainer, and the mapping could be learned from NPC speech without curation, but players who only ever hailed you (e.g. `Fenwick`) could never be translated, for 21 rows out of ~7,000.
- Wiring `ContextMenu` into `KillsView`.
- Reusing `KillDetailModal` from Top Targets — it takes a `Kill`, not a creature name, and would need refactoring.
- Adding a frontend test runner.
