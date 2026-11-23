# Trainer Data Fixes Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Credit Higgrus and Hardia ranks (which share Histia's and Master Bodrus's rank messages) from the character's trainer sign-ups. Make Rodnus a Fighter trainer. Tell upgraded users when a rescan would change their trainer data, without ever overwriting their own fixes.

**Architecture:** `trainers.json` entries gain `alternates`. The scanner never counts a shared-message rank: it stores sign-ups (detected by a per-file `SignupTracker`) and shared-rank observations, keyed by `(file_path, line_no)`. `finalize_characters` rebuilds `trainers.shared_ranks` for every observation in one SAVEPOINT, Last-scan shadow included. A hand-bumped `TRAINER_REVISION` drives a dismissible GUI banner and a CLI stderr notice.

**Tech Stack:** Rust (rusqlite 0.31, serde_json, regex), Tauri 2 commands, React/TypeScript UI.

**Spec:** `docs/superpowers/specs/2026-11-23-trainer-data-fixes-design.md` (revision 8). Read sections D1 to D6 before starting any task. Citations like `parser/mod.rs:847` refer to `crates/amanuensis-core/src/…` at commit `7ac257d`.

## Global Constraints

- **User-entered data is never written by anything this change adds** (D6): `trainers.modified_ranks`, `rank_mode`, `override_date`, `notes`, `characters.profession_override`, `characters.merged_into`. Backup-file **entries** are preserved; the file itself is still rewritten by the existing `backup_all` on GUI open. Existing behaviour stays as it is: `apply_learning_*` counters are log-derived, and ordinary upserts refresh `effective_multiplier`. Nothing is converted automatically.
- **The new derivation writes only** `trainers.ranks`, `shared_ranks`, `date_of_last_rank` and `effective_multiplier`. Its `ON CONFLICT` clause names only those columns.
- **`shared_ranks` is internal**: it is not added to the `Trainer` model or any reader.
- **Sign-up wording:**
  - question: `"Are you sure you would like to train with me?"`, spoken with `asks` or `says`;
  - reply: exactly `yes`, optionally followed by `.` or `!`;
  - welcome: `"Welcome aboard. Go into the world and learn what you may, while practicing our craft."`
- **Pending sign-up window:** 30 lines.
- **Alternates:** Histia → Higgrus (Healer). Master Bodrus → Hardia (Healer). Rodnus `profession` = `"Fighter"`.
- **`TRAINER_REVISION: u32 = 1`.** `db_meta` keys are `trainer_revision` and `trainer_revision_ack`.
- **User-facing banner and notice text** must be approved by the user before release: no em-dashes, no internal jargon. Draft text is in Task 8.
- **CLI parity:** everything the GUI can show or do in this change, the CLI can too (it may do more). Here that means the trainer catalog including Higgrus/Hardia (already via `all_trainer_metadata`), the trainer-data status with its review list, and dismissing it.
- **Tests:** `cargo test -p amanuensis-core` and `cargo test -p amanuensis-cli`. Use `Database::open_in_memory()` and `create_test_log_dir()` (`parser/mod.rs:2381`) for synthetic logs.
- **Don't** run `tsc`/`npm run build` in `crates/amanuensis-gui/ui` while `cargo tauri dev` is running: it cycles the app.

## Review Focus

These five input classes or failure modes most likely to bite a user are not covered by the spec's own test list. Each has a test added to the task named.

1. **A user's hand workaround** (negative Histia modifier, Higgrus override, notes, profession override) survives scan, finalize and rescan byte-for-byte, and appears in the review list. Task 7, `user_fixes_never_written`; Task 8, `review_list_covers_all_four_trainers`.
2. **Case differences:** a folder `tane` with the game saying `Tane says, "yes"` still confirms the sign-up. Task 4, `yes_matches_case_insensitively`.
3. **Prefix and punctuation variants:** a rank line with `•` instead of `¥`, or without its trailing period, still becomes an observation. Task 5, `bullet_and_periodless_rank_lines_observed`.
4. **One file, two characters:** A signs up with Higgrus, then B logs in and earns the shared message. B gets Histia, not Higgrus. Task 7, `signup_does_not_cross_characters`.
5. **Legacy database:** opening a pre-change DB (no observations) and finalizing leaves every `trainers` row untouched. Task 7, `legacy_db_finalize_is_noop`.

---

### Task 1: TrainerDb alternates and the NPC guard

**Files:**
- Modify: `crates/amanuensis-core/src/data/trainers.rs` (`TrainerDb`, `from_json_bytes`, `all_trainer_metadata`, tests)
- Modify: `crates/amanuensis-core/src/data/npc_trainers.rs:68-90` (derivation), `:22-23` (`EXTRA_NPC_DISPLAY_NAMES`)

**Interfaces:**
- Produces:
  - `TrainerDb::alternates_of(&self, default_trainer: &str) -> &[String]`, returning an empty slice when there are none;
  - alternates registered in `get_profession`, `get_multiplier` and `all_trainer_metadata`;
  - `get_trainer` unchanged.

- [ ] **Step 1: Write failing tests in `data/trainers.rs` tests**

```rust
#[test]
fn alternates_parsed_and_registered() {
    let json = r#"{"¥You seem to be able to take more damage now.": {"trainer": "Histia", "profession": "Fighter",
        "alternates": [{"trainer": "Higgrus", "profession": "Healer", "effective_rank_multiplier": 1.5}]}}"#;
    let db = TrainerDb::from_json_bytes(json.as_bytes()).unwrap();
    assert_eq!(db.get_trainer("You seem to be able to take more damage now"), Some("Histia")); // no period
    assert_eq!(db.alternates_of("Histia"), &["Higgrus".to_string()]);
    assert!(db.alternates_of("Higgrus").is_empty());
    assert_eq!(db.get_profession("Higgrus"), Some("Healer"));
    assert_eq!(db.get_multiplier("Higgrus"), 1.5);
    let names: Vec<_> = db.all_trainer_metadata().into_iter().map(|m| m.name).collect();
    assert!(names.contains(&"Higgrus".to_string()) && names.contains(&"Histia".to_string()));
}
```

In `npc_trainers.rs` tests, add `alternate_names_are_guarded`. It asserts `is_known_npc_trainer("Higgrus")` and `is_known_npc_trainer("Hardia")`. This test passes only after Task 2 adds the data and Step 3 removes both names from `EXTRA_NPC_DISPLAY_NAMES`; mark it `#[ignore]` with a comment until Task 2.

- [ ] **Step 2: Run** `cargo test -p amanuensis-core alternates_parsed_and_registered`. Expected: FAIL (`alternates_of` not found).

- [ ] **Step 3: Implement**
  - Add `alternates: HashMap<String, Vec<String>>` to `TrainerDb`. In `from_json_bytes`, register each `alternates[]` entry's `trainer`, `profession` and `effective_rank_multiplier` exactly like the primary trainer. Key the map by the primary trainer name.
  - `all_trainer_metadata` iterates the union of primary and alternate names, still de-duplicated and sorted.
  - In `npc_trainers.rs` derivation, also read `alternates[].trainer`, applying the same `excluded` filter. Remove `"Hardia"` and `"Higgrus"` from `EXTRA_NPC_DISPLAY_NAMES`, and update the test list at `:115` that names them.

- [ ] **Step 4: Run** `cargo test -p amanuensis-core trainers` and `cargo test -p amanuensis-core npc_trainers`. Expected: PASS, apart from the one ignored test.

- [ ] **Step 5: Commit** `feat(trainers): support alternate trainers on a shared rank message`

### Task 2: Trainer data: alternates, Rodnus, pinned facts, revision constant

**Files:**
- Modify: `crates/amanuensis-core/data/trainers.json` (`¥You seem to be able to take more damage now.`, `¥Your body feels stronger.`, `¥You feel more receptive to healing.`)
- Modify: `crates/amanuensis-core/src/data/trainers.rs` (assert at `:288`; new test; `pub const TRAINER_REVISION: u32 = 1;` exported from `data/mod.rs` and `lib.rs`)
- Modify: `crates/amanuensis-core/src/data/npc_trainers.rs` (un-ignore the Task 1 test)

**Interfaces:**
- Produces: `amanuensis_core::TRAINER_REVISION: u32`.

- [ ] **Step 1: Write failing test `community_confirmed_trainer_facts`.** Give it one source comment per group (clump Fighter_Trainers / Healer_Trainers / Trainer_Rank_Messages rev 2702; RankCounter27 line 125), plus a comment: "Changing trainer data or attribution? Bump TRAINER_REVISION."

```rust
let db = TrainerDb::bundled().unwrap();
assert_eq!(db.get_profession("Rodnus"), Some("Fighter"));
assert_eq!(db.get_trainer("You seem to be able to take more damage now."), Some("Histia"));
assert_eq!(db.get_profession("Histia"), Some("Fighter"));
assert_eq!(db.alternates_of("Histia"), &["Higgrus".to_string()]);
assert_eq!(db.get_profession("Higgrus"), Some("Healer"));
assert_eq!(db.get_trainer("Your body feels stronger."), Some("Master Bodrus"));
assert_eq!(db.alternates_of("Master Bodrus"), &["Hardia".to_string()]);
assert_eq!(db.get_profession("Hardia"), Some("Healer"));
for universal in ["Master Bodrus", "Master Spirtus", "Master Mentus", "Heen Slostid"] {
    assert_eq!(db.get_profession(universal), None, "{universal}");
}
```

Change the `:288` assert to `Some("Fighter")`.

- [ ] **Step 2: Run** `cargo test -p amanuensis-core community_confirmed_trainer_facts`. Expected: FAIL.
- [ ] **Step 3: Edit `trainers.json`** per spec D1 and D3. Rodnus becomes `"Fighter"`. Add the constant. Un-ignore `alternate_names_are_guarded`.
- [ ] **Step 4: Run** `cargo test -p amanuensis-core`. Expected: PASS. If `test_profession_mappings` or real-data expectations break on Rodnus, update them to Fighter: that is the fix.
- [ ] **Step 5: Commit** `fix(data): Rodnus is a Fighter trainer; Higgrus and Hardia share Histia/Bodrus messages`

### Task 3: Schema and storage queries

**Files:**
- Modify: `crates/amanuensis-core/src/db/schema.rs` (`create_tables`, `migrate_tables`)
- Create: `crates/amanuensis-core/src/db/queries/shared_rank.rs` (register in `db/queries/mod.rs`)
- Modify: `crates/amanuensis-core/src/db/queries/log_file.rs:85-118` (`reset_log_data`), `:122` (`delete_all_data`)
- Modify: `crates/amanuensis-core/src/db/queries/trainer.rs:283-289` (`set_rank_override` zeroing)

**Interfaces:**
- Produces, on `impl Database`:
  - `insert_trainer_signup(&self, file_path: &str, line_no: i64, char_id: i64, at: &str, trainer: Option<&str>) -> Result<bool>`, returning true when a row was inserted;
  - `insert_shared_rank_observation(&self, file_path: &str, line_no: i64, char_id: i64, at: &str, default_trainer: &str) -> Result<bool>`;
  - `signups_for(&self, char_id: i64) -> Result<Vec<Signup>>`, sorted by `(at, file_path, line_no)`;
  - `all_shared_rank_observations(&self) -> Result<Vec<SharedRankObservation>>`.
- Produces `Database::shared_ranks_of(&self, char_id: i64, trainer: &str) -> Result<i64>` (internal bookkeeping read, used by tests and derivation), and the structs `pub struct Signup { pub file_path: String, pub line_no: i64, pub at: String, pub trainer: Option<String> }` and `pub struct SharedRankObservation { pub file_path: String, pub line_no: i64, pub character_id: i64, pub at: String, pub default_trainer: String }`.

- [ ] **Step 1: Write failing tests in `shared_rank.rs`**
  - `signup_insert_is_idempotent`: same key twice gives `true` then `false`.
  - `reset_clears_observations_and_shared_ranks`: insert one observation; set a trainer row to `ranks=5, shared_ranks=3`; call `reset_log_data`. Expect both tables empty and `ranks=0, shared_ranks=0`.
  - `override_switch_zeroes_shared_ranks`: from `ranks=10, shared_ranks=10`, `set_rank_override(.., "override_until_date", ..)` gives `ranks=0, shared_ranks=0`, with `modified_ranks` and `notes` as set by the user.
  - `migrate_adds_shared_ranks_to_legacy_db`: create the old `trainers` table without the column, run `migrate_tables`, and expect `shared_ranks` defaulting to 0.
- [ ] **Step 2: Run** `cargo test -p amanuensis-core shared_rank`. Expected: FAIL.
- [ ] **Step 3: Implement** with the DDL in spec D2c.
  - Use `CREATE TABLE IF NOT EXISTS` / `CREATE INDEX IF NOT EXISTS` in `create_tables`.
  - Add `ALTER TABLE trainers ADD COLUMN shared_ranks INTEGER NOT NULL DEFAULT 0` to the tolerant ALTER list in `migrate_tables`, like `schema.rs:278`.
  - Wrap `reset_log_data`'s batch in `SAVEPOINT reset_log_data … RELEASE`, rolling back on error. Add the two `DELETE`s and `shared_ranks=0` in the existing `UPDATE trainers` statement.
  - `delete_all_data` also deletes both tables and the `db_meta` key `trainer_revision_ack`.
  - The `set_rank_override` zeroing statement also sets `shared_ranks = 0`.
- [ ] **Step 4: Run** `cargo test -p amanuensis-core`. Expected: PASS.
- [ ] **Step 5: Commit** `feat(db): storage for trainer sign-ups and shared rank observations`

### Task 4: SignupTracker

**Files:**
- Create: `crates/amanuensis-core/src/parser/signup.rs` (register in `parser/mod.rs`)

**Interfaces:**
- Produces:
  - `#[derive(Clone, Default)] pub struct SignupTracker`;
  - `pub enum SignupEvent { Npc(String), PlayerTeacher }`;
  - `impl SignupTracker { pub fn feed(&mut self, message: &str, active_name: Option<&str>) -> Option<SignupEvent>; pub fn reset(&mut self) }`.
- Behaviour:
  - `message` is a line after timestamp stripping and movie filtering;
  - `reset()` is called by the scanner when the active character changes;
  - the tracker itself clears on `Welcome to Clan Lord` / `Welcome back` (`patterns::WELCOME_LOGIN` / `WELCOME_BACK`);
  - the player-teacher line reuses the regex at `player_signals.rs:86` (make it `pub(crate)`).

- [ ] **Step 1: Write the failing tests.** Helper `fn run(lines: &[&str], active: &str) -> Vec<SignupEvent>`.
  - `full_exchange_asks` and `full_exchange_says`: the question, `Squib says, "yes"`, then the welcome. Expect `[Npc("Higgrus")]`.
  - `yes_before_question_only_is_ignored`: `yes`, question, welcome. Expect `[]`.
  - `yes_before_and_after`: Tane 2026-08-08 shape. Expect `[Npc("Respia")]`.
  - `bystander_yes_ignored`: `Garr says, "yes"` with active `Ruuk`. Expect `[]`.
  - `no_cancels`.
  - `reset_between_yes_and_welcome_cancels`: call `reset()` mid-exchange.
  - `window_expires_after_30_lines`: the question, then 30 filler lines, then yes and welcome. Expect `[]`. With 29 filler lines: one event.
  - `player_teacher_clears_pending`: Higgrus question + yes, `¥ You begin training with Jive Turkey.`, then a Higgrus welcome. Expect `[PlayerTeacher]`.
  - `yes_matches_case_insensitively` (Review Focus 2): active `tane`, line `Tane says, "yes"`. Expect the event.
- [ ] **Step 2: Run** `cargo test -p amanuensis-core signup::`. Expected: FAIL.
- [ ] **Step 3: Implement** the state machine in spec D2a, with `pending: Option<Pending { trainer, char_name, confirmed, lines_left }>`.
- [ ] **Step 4: Run** the tests. Expected: PASS.
- [ ] **Step 5: Commit** `feat(parser): detect trainer sign-ups`

### Task 5: Record sign-ups and observations during scans

**Files:**
- Modify: `crates/amanuensis-core/src/parser/mod.rs`:
  - `LogParser` gains a `new_shared_obs: RefCell<Vec<(String, i64)>>` field;
  - `begin_scan_op` at `:94` clears it when depth is 0;
  - `scan_bytes` at `:574`;
  - the `TrainerRank` arm at `:847`.

**Interfaces:**
- Consumes:
  - Task 1 `alternates_of`;
  - Task 3 `insert_trainer_signup` and `insert_shared_rank_observation`;
  - Task 4 `SignupTracker`.
- Produces:
  - `scan_bytes` gains a `seed: ScanSeed` parameter, defined here as `pub(crate) struct ScanSeed { movie: MovieFilter, signup: SignupTracker, line_offset: i64, last_ts: Option<String> }` with `Default`;
  - existing callers pass `ScanSeed::default()` until Task 6;
  - `MovieFilter` derives `Clone, Default`.

- [ ] **Step 1: Write failing scan tests** in `parser/mod.rs` tests, scanning with `scan_folder` and **not** calling finalize:
  - `signup_and_observation_recorded`:
    - log: welcome Squib, Higgrus question/yes/welcome, then `¥You seem to be able to take more damage now.`;
    - expect `signups_for(squib) == [Npc Higgrus]` at the welcome's line number;
    - expect one observation with `default_trainer == "Histia"`;
    - expect the `trainers` Histia row absent or at `ranks == 0`. The scanner must not count it.
  - `non_shared_rank_still_counted_at_scan`: `¥You seem to attack more precisely now.` gives Atkus `ranks == 1` (unchanged path).
  - `bullet_and_periodless_rank_lines_observed` (Review Focus 3): `•You seem to be able to take more damage now` gives one observation.
  - `force_rescan_does_not_requeue`: scan, then `scan_folder(.., force=true)`. Observation count stays 1, and `new_shared_obs` after the second op is empty.
  - `bystander_welcome_keeps_no_session_file_skipped`: a folder file with only Garr's exchange still skips as no game session.
  - `shared_rank_counts_as_event`: a tail scan whose only new line is a shared rank gives `events_found == 1`. Re-reading the same line under `force` gives 0.
- [ ] **Step 2: Run.** Expected: FAIL.
- [ ] **Step 3: Implement.**
  - In `scan_bytes`, feed `SignupTracker` after the movie filter, with the active character's name; call `reset()` wherever `active` changes.
  - `line_no = seed.line_offset + index`.
  - Store `SignupEvent` via `insert_trainer_signup`, with `trainer = None` for `PlayerTeacher`.
  - At the top of the `TrainerRank` arm: if `!alternates_of(&trainer_name).is_empty()`, call `insert_shared_rank_observation`. When it returns true, push the key onto `new_shared_obs` and do `file_result.events_found += 1`. Then skip the rank write and the skip report.
  - Seed `current_date` from `seed.last_ts` when it is `Some`.
- [ ] **Step 4: Run** `cargo test -p amanuensis-core`. Expected: PASS. No existing unit test counts Histia/Bodrus through a scan (existing ones upsert rows directly; real-data tests are already `#[ignore]`), so nothing needs ignoring. Observation-only tests use `scan_folder`, which doesn't finalize; `update_sources` does.
- [ ] **Step 5: Commit** `feat(parser): record trainer sign-ups and shared rank observations`

### Task 6: Tail scans resume tracker state (`prefix_state`)

**Files:**
- Modify: `crates/amanuensis-core/src/parser/mod.rs`:
  - replace `active_char_at_offset` at `:1894`;
  - its four callers: `:381`, `:475`, `:1537`, `:1730`.

**Interfaces:**
- Consumes: Task 5 `ScanSeed`.
- Produces: `fn prefix_state(&self, bytes: &[u8], offset: usize, fallback: Option<(i64, String)>) -> Result<(Option<(i64, String)>, ScanSeed)>`. It returns the resumed active character (last welcome, else `fallback`) and the seed.
  - The seed's `signup` keeps pending state only; its emitted events are discarded.
  - `line_offset` is the prefix's line count.
  - `last_ts` is the last non-replay timestamp, formatted `%Y-%m-%d %H:%M:%S`.
  - It keeps the `load_override_config` call at `:1912`.
  - With `offset == 0` it returns `(fallback, ScanSeed::default())`.

- [ ] **Step 1: Write failing tests.** Each writes a file, scans it, appends to it, then runs `update_sources` or a second `scan_folder`, so the second pass is a tail scan.
  - `tail_completes_signup_split_across_offset`: the first part holds a kill line (so `has_game_session` accepts it, `parser/mod.rs:365`), then the question and the yes. Assert the first pass recorded the file's `byte_len`. The appended part holds the welcome and the rank line. The file has no `Welcome to Clan Lord` line, so it relies on the folder fallback. Expect a `Higgrus` sign-up stored.
  - `tail_starting_in_movie_ignores_replay`: prefix ends after `* Starting movie '…clMov'...`. The appended replayed exchange and rank line, then `*** End of movie file. ***`, record nothing.
  - `tail_untimestamped_line_inherits_prefix_time`: prefix has a timestamped noon sign-up; the appended rank line has no timestamp. The observation's `at` equals the noon timestamp, not the filename time.
  - `tail_line_numbers_are_absolute`: the observation's `line_no` equals its line index in the whole file.
- [ ] **Step 2: Run.** Expected: FAIL.
- [ ] **Step 3: Implement.** Pass each caller's existing fallback (`None` for loose files at `:475`).
- [ ] **Step 4: Run** `cargo test -p amanuensis-core`. Expected: PASS, including the existing tail-scan and movie tests.
- [ ] **Step 5: Commit** `fix(parser): tail scans resume movie, sign-up and timestamp state`. In the same commit, update CLAUDE.md "Movie playback": the tail-scan gap is fixed.

### Task 7: Resolution and derivation in `finalize_characters`

**Files:**
- Create: `crates/amanuensis-core/src/parser/shared_ranks.rs` (resolution and derivation, `impl LogParser`)
- Modify: `crates/amanuensis-core/src/parser/mod.rs:1928` (call it first in `finalize_characters`)
- Modify: `crates/amanuensis-core/src/db/queries/shared_rank.rs` (apply helpers)

**Interfaces:**
- Consumes: Tasks 1, 3, 5 and 6.
- Produces:
  - `fn resolve(signups: &[Signup], obs: &SharedRankObservation, alternates: &[String]) -> String` (pure, spec D2d);
  - `fn derive_shared_ranks(&self) -> Result<()>` on `LogParser` (spec D2e steps 1-5);
  - `Database::apply_shared_counts(&self, counts: &[(i64, String, i64, String)], multiplier: impl Fn(&str) -> f64) -> Result<()>` writes `trainers` only. The tuple is `(char_id, trainer, count, max_at)`. `counts` must also include every (char, trainer) that currently has `shared_ranks > 0` but no observations left, with count 0. The update is `ranks = MAX(ranks - shared_ranks, 0) + new`, applied only where the count changed or `ranks < shared_ranks`.
  - Last-scan shadow uses one path only: the existing `upsert_trainer_rank_scan`, called once per counted new observation.
  - Failure hook: a `#[cfg(test)] fail_after: Cell<Option<&'static str>>` field on `LogParser`. Gate its initializer in `LogParser::new`, and both checks in `derive_shared_ranks`, with `#[cfg(test)]` too, so non-test builds never reference it. At the two named points, `"after_lifetime"` and `"after_shadow"`, the check returns `Err`.

Resolution algorithm, fixed by the spec:
1. The sign-ups that apply are those with `at < obs.at`, plus those with `at == obs.at`, the same file, and `line_no < obs.line_no`.
2. Take the greatest applying `at`. Within one file, take the greatest `line_no`.
3. If sign-ups from more than one file remain and they name different trainers, return the default.
4. Return the sign-up's trainer if it is the default or in `alternates`; otherwise return the default. `None` (player teacher) also gives the default.

- [ ] **Step 1: Write failing tests**
  - Pure `resolve` tests:
    - no sign-ups gives the default;
    - a Higgrus sign-up gives Higgrus;
    - Higgrus then a player teacher gives the default;
    - Higgrus then Detha in one timestamp-less file gives the default;
    - equal-`at` sign-ups in two files naming different trainers give the default, and naming the same trainer give that trainer;
    - a sign-up at the same `at` but a later line gives the default.
  - Scan + finalize tests, following spec "Tests" items 1-6, 10-14, 16, 17, 19, 21, 22:
    - `higgrus_credited_after_finalize`: Higgrus 1 and Histia 0, in both `trainers` and `scan_trainers`;
    - `order_independent_across_sources`: two temp roots, scanned in both orders, give the same result;
    - `finalize_twice_is_idempotent`;
    - `fresh_parser_finalize_recovers`: scan with parser A without finalizing, then finalize with parser B on the same DB file. Use a temp-file DB, not in-memory;
    - `override_follows_resolved_trainer`: a Histia override-skip doesn't block Higgrus, and the skip report names the resolved trainer;
    - `override_transitions` (spec test 18): starting from 10 observations:
      - switch to `override_until_date` with 2 after the cutoff: `ranks == 2`, never negative;
      - switch back to modifier: 10;
      - `move_rank_override` from Atkus onto Higgrus, which already has derived ranks: Higgrus's `ranks` unchanged, and the override is applied;
      - `clear_rank_overrides`: 10;
    - `backup_only_higgrus_override_restorable` (D6): a backup has Higgrus `+30`, but the DB has none. Finalize creates a derived Higgrus row; `missing_rank_overrides` still lists it; restore gives `modified_ranks == 30`; finalize again leaves `ranks` equal to the derived count;
    - `skipped_only_operation_keeps_last_scan`;
    - `derive_failure_rolls_back_everything`: populate a previous Last scan, then run a shared-rank-only operation. For each hook point (`after_lifetime`, `after_shadow`), finalize returns `Err`, and then:
      - `trainers`, `scan_trainers` and `scan_kills` are unchanged;
      - the `db_meta` keys `last_scan_token` and `scan_token_counter` are unchanged;
      - the in-memory `scan_token` equals its prior value;
      - `new_shared_obs` is unchanged;
      - clearing the hook and finalizing again succeeds, with the expected counts;
    - `imported_row_preserved`: Scribius-style row `ranks=40, shared_ranks=0` plus 2 observations gives 42. Also run the reverse order: import into a DB that already holds a scanned row leaves it unchanged;
    - `partial_file_failure_retry_counts_once`;
    - `signup_does_not_cross_characters` (Review Focus 4);
    - `legacy_db_finalize_is_noop` (Review Focus 5): snapshot all `trainers` rows of a DB with no observations, finalize, and expect them identical.
  - `user_fixes_never_written` (Review Focus 1):
    - set up: Histia `modified_ranks=-20` (modifier), Higgrus `override` with `modified_ranks=150`, a Hardia note, `profession_override='Healer'`, a merged source character, and an existing `rank-overrides-backup.json`;
    - run scan, finalize, `reset_log_data`, rescan and finalize;
    - expect every user-entered column listed in Global Constraints equal to its starting value, and `read_overrides_file` on the backup returning the same entries.
- [ ] **Step 2: Run.** Expected: FAIL.
- [ ] **Step 3: Implement**
  - `derive_shared_ranks` runs at the top of `finalize_characters`, before profession work.
  - Per character, load sign-ups once and binary-search them.
  - SAVEPOINT `derive_shared`; Last-scan handling per spec D2e step 4, with `prev` token restoration and draining `new_shared_obs` only after release.
  - The skip report goes through `add_process_log("info", …)`, using the existing message format: `Skipped {count} rank(s) for {trainer} ({char_name}) — override active`.
- [ ] **Step 4: Run** `cargo test -p amanuensis-core` and `cargo test -p amanuensis-core --test real_data_comparison -- --ignored`, with `AMANUENSIS_TEST_LOGS="/Volumes/Aux/Clan Lord related/Clan Lords/Text Logs"`. Expected:
  - all unit tests PASS;
  - for real data, tane's Master Bodrus = 58 and Ruuk's Histia = 11 (assert both in a new ignored test `shared_message_real_data`), and no sign-up row for Ruuk at 2018-05-20 07:59:13;
  - the pre-existing real-data drift failures (20, per CLAUDE.md "Testing") are unchanged in number.
- [ ] **Step 5: Commit** `feat(parser): credit Higgrus and Hardia from sign-ups, derived at finalize`

### Task 8: Trainer revision status, GUI banner, CLI notice, docs

**Files:**
- Create: `crates/amanuensis-core/src/db/queries/trainer_revision.rs`
- Modify: `crates/amanuensis-core/src/db/queries/mod.rs:125` (`Database::open`: set the revision when `log_files` is empty), `log_file.rs` (reset/delete set the revision)
- Modify: `crates/amanuensis-gui/src/commands/rank.rs`, `crates/amanuensis-gui/src/main.rs:14` (register commands)
- Create: `crates/amanuensis-gui/ui/src/components/shared/TrainerDataBanner.tsx`
- Modify: `crates/amanuensis-gui/ui/src/lib/commands.ts`, `crates/amanuensis-gui/ui/src/components/layout/AppShell.tsx` (render next to `UpdateBanner`)
- Modify: `crates/amanuensis-cli/src/main.rs:692,1061` (`cmd_summary`, `cmd_trainers`), plus the new `Commands::TrainerStatus { ack: bool }` and `cmd_trainer_status`
- Modify: `CLAUDE.md` (item 4; test counts)

**Interfaces:**
- Produces:
  - `pub struct ReviewItem { pub character: String, pub trainer: String, pub rank_mode: String, pub modified_ranks: i64 }`;
  - `pub struct TrainerDataStatus { pub stale: bool, pub acknowledged: bool, pub modifiers_to_review: Vec<ReviewItem> }`, both deriving `Serialize`;
  - `Database::trainer_data_status(&self) -> Result<TrainerDataStatus>`;
  - `Database::acknowledge_trainer_data(&self) -> Result<()>`;
  - Tauri `get_trainer_data_status` / `acknowledge_trainer_data`;
  - TS `getTrainerDataStatus()` / `acknowledgeTrainerData()`.

- [ ] **Step 1: Write failing tests** in `trainer_revision.rs`:
  - `fresh_db_not_stale`;
  - `scanned_db_without_revision_is_stale`;
  - `reset_sets_current_revision`;
  - `acknowledge_hides_but_stays_stale`;
  - `delete_all_clears_ack`;
  - `import_only_db_not_stale`: trainer rows and no `log_files`;
  - `review_list_covers_all_four_trainers` (Review Focus 1), which expects all of:
    - Histia `-20` (modifier) listed;
    - Master Bodrus `+5` (override_until_date) listed;
    - an override moved onto Atkus not listed (D6: listed only if the destination is one of the four);
    - Higgrus override listed;
    - Hardia modifier `0` with mode modifier not listed;
    - Atkus `+10` not listed.
  - In `amanuensis-cli`, a smoke test that `cmd_trainers` on a stale temp DB writes the notice to stderr. Use the existing test style, or extract `fn stale_notice(db: &Database) -> Option<String>` and test that. `stale_notice` returns `None` once acknowledged, matching the GUI.
  - Clap-parsing smoke tests, in the existing style at `main.rs:2393`: `trainer-status` parses, and `trainer-status --ack` sets `ack`.
- [ ] **Step 2: Run.** Expected: FAIL.
- [ ] **Step 3: Implement** per spec D4/D6.
  - Banner: shows when `stale && !acknowledged`. The Dismiss button calls `acknowledgeTrainerData`.
  - It lists `modifiers_to_review` with one reason line per rank mode, and links to Rank Modifiers.
  - Draft copy, for the user to approve (no em-dashes):
    - Title: "Trainer ranks can now be read more accurately"
    - Body: "This version recognises Higgrus and Hardia and fixes Rodnus. Run Rescan Logs to rebuild your ranks from your log folders. A rescan resets anything that is not in those folders, such as ranks imported from Scribius or logs on a drive that is not connected."
    - Modifier mode: "{trainer} ({character}): you entered {n}. After a rescan, logged ranks are added to this, so check it is still what you want."
    - Override mode: "{trainer} ({character}): set to {n} by hand. Logged ranks are now available if you switch back."
  - The CLI prints the title and body lines to stderr from `cmd_summary` and `cmd_trainers` when `stale && !acknowledged`, the same condition as the GUI banner.
  - New CLI command `amanuensis trainer-status [--ack]`, the GUI banner's equivalent.
    - It takes the same `--db` / `--gui-db` handling as other commands.
    - It prints the status (stale, acknowledged), the same title and body text, and every `modifiers_to_review` entry with the same per-mode reason lines.
    - With `--ack` it calls `acknowledge_trainer_data` and prints a confirmation.
    - When not stale it prints `Trainer data is up to date.`
  - CLAUDE.md item 4 gets a short paragraph covering:
    - alternates;
    - sign-ups and the derivation at finalize;
    - shared trainers following the current override config;
    - the revision banner;
    - the D6 rule that user-owned columns are never written;
    - "bump TRAINER_REVISION with trainer data changes".
- [ ] **Step 4: Run** `cargo test -p amanuensis-core`, `cargo test -p amanuensis-cli`, and `npx tsc --noEmit` in `crates/amanuensis-gui/ui` (with no `cargo tauri dev` running). Expected: PASS. Then run the app (`cargo tauri dev`) against a copy of the real DB, made first with `sqlite3 "$HOME/Library/Application Support/com.dfsw.Amanuensis/amanuensis.db" ".backup /tmp/…"`. Confirm the banner shows, Dismiss hides it, and it stays hidden after relaunch.
- [ ] **Step 5: Commit** `feat: tell users when trainer data changed and which modifiers to review`. CLAUDE.md's CLI-parity notes mention `trainer-status`.

---

## Self-review notes

- **Spec coverage:** D1 → Tasks 1-2. D2a → Task 4. D2b → Task 6. D2c → Task 3. D2c scan-time and D2e → Tasks 5 and 7. D2d → Task 7. D3 → Task 2. D4 and D6 → Tasks 7-8. D5 needs no code (the formula key exists). Rollout docs → Tasks 6 and 8. Spec tests 1-22 are mapped to Tasks 4-7.
- **Open before release (not blocking implementation):** the user approves the banner copy and release notes; players are asked for a real Higgrus and a real Hardia log snippet, to add as fixtures in `shared_message_real_data`.
