# Trainer data fixes: shared rank messages, Rodnus, and fixes that don't stick

Date: 2026-11-23
Status: revision 8, approved by Codex review 6 (wording fix applied); ready to implement

## Problem

Player reports (2026-11):

1. Higgrus and Hardia (healer trainers) never appear in Amanuensis.
2. Rodnus is listed as a Healer trainer; he is a Fighter trainer.
3. "The fixes to trainers never seem to stick version to version."

## Findings (evidence)

### F1. One rank message can only map to one trainer

`TrainerDb` (`crates/amanuensis-core/src/data/trainers.rs`) is `HashMap<message, trainer>`
built from `trainers.json`, an object keyed by message. A message shared by two trainers
cannot be represented.

- **Higgrus shares Histia's message.** Upstream RankCounter (`rankmessages.txt` +
  `trainers.txt`, line-paired) gives `Histia` (line 4) and `Higgrus` (line 125) the same
  `You seem to be able to take more damage now.` Our import kept Histia only. The clump
  Healer Trainers page lists Higgrus as a basic healer trainer ("physical well-being")
  but leaves his message blank, so this rests on RankCounter alone.
- **Hardia shares Master Bodrus's message.** Clump Trainer Rank Messages (rev 2702) has
  one row `Master Bodrus (or Hardia)`: `Your body feels stronger.` The Healer Trainers
  page confirms: Hardia, third circle, "extended Bodrus for healers", message
  `Your body feels stronger. (same as Master Bodrus)`. (Distinct from Hardio, Mystic,
  `Your body feels a little stronger.`, already handled.)
- `git log -S Higgrus` / `-S Hardia`: neither has ever been in `trainers.json`. Both are
  in the hand-kept `EXTRA_NPC_DISPLAY_NAMES` guard list (`data/npc_trainers.rs:22`), and
  `Hardia` is a fighter-stat formula key (`fighter_stats.rs:39`, `lib/fighterStats.ts:31`)
  that today only a manual rank modifier can feed.
- Effect: Higgrus ranks count as Histia (fighter stats weight Histia 111 health/rank),
  Hardia ranks as Master Bodrus.

### F2. The sign-up identifies the trainer

Rank messages arrive in the field and credit whoever the character is signed with. The
sign-up is a fixed exchange:

```
Higgrus asks, "Are you sure you would like to train with me?"     (older clients)
Sprite says, "Are you sure you would like to train with me?"      (2026 client)
Tane says, "yes"
Sprite says, "Welcome aboard. Go into the world and learn what you may, while practicing our craft."
```

Developer corpus (`/Volumes/Aux/.../Text Logs`, every character, files in date order):

- 51 `Welcome aboard` lines, one wording. The question appears as `asks` in older logs and
  `says` in 2026 logs. It sits 2 to 4 lines before the welcome; a
  `Just tell me yes or no, please.` prompt can come in between.
- The reply is always exactly `yes` (59 occurrences near a welcome). Players sometimes
  type `yes` before the question as well: Tane, 2026-08-08, `yes` / question / `yes` /
  welcome.
- 50 of 51 welcomes are preceded by the log's own character saying `yes`. The exception
  is a bystander: Ruuk's log, 2018-05-20, `Garr says, "yes"` then `Ching Shih says,
  "Welcome aboard..."`. NPC speech is broadcast, so without a check Ruuk would be wrongly
  switched to Ching Shih.
- No log file contains a `Welcome aboard` without also containing a `Welcome to Clan Lord`
  / `Welcome back` line.
- Last confirmed sign-up before each shared/related rank message:

  | Character | Last sign-up | Message | n |
  |---|---|---|---|
  | tane | Master Bodrus | body feels stronger | 58 |
  | Ruuk | Histia | take more damage | 11 |
  | Ruuk | Detha | take more damage | 2 |
  | Ruuk | (player teacher) | receptive to healing (Rodnus) | 18 |
  | Agratis Feffer | none (newbie island) | body feels stronger | 17 |

Gaps: player teachers (`¥ You begin training with Jive Turkey.`) name the player, not
the NPC trainer; some sign-ups are missing from the logs (Detha rows); new characters
earn Bodrus ranks without signing up; the corpus has a Higgrus sign-up (Squib,
2019-10-07) but no Higgrus or Hardia rank line after one, so there is no real-log
fixture yet.

### F3. Rodnus flip-flopped, and a test pins the wrong value

Rodnus `profession` across `trainers.json` history: `0240dee` (community PR, noivad)
Fighter → `1cc8384` Healer → `66771bb` Fighter → `514c023` none → `ad69ab6`
("Recategorize trainers") Healer, which also added
`assert_eq!(db.get_profession("Rodnus"), Some("Healer"))` (`data/trainers.rs:288`).
Clump Fighter Trainers lists Rodnus as the Fourth Circle fighter trainer.

Root cause: one `profession` field has two readings. The Ranks view groups by it
(`TrainersView.tsx`), where "heal receptivity" suggests Healer. `determine_profession`
(`parser/mod.rs:1229`) sums ranks per tag to infer class, where the tag must be the
teaching guild. Heen Slostid hit the same conflation (`ad69ab6`, fixed `d6b2861`).

### F4. Upgraded databases keep the old answers

- Ranks aggregate into `trainers` rows by name at scan time; a data fix never moves them.
- `finalize_characters` keeps a stored profession, specialization or base class
  (`parser/mod.rs:1947-1955`), so class results don't self-heal on Update Logs.

Users upgrade, run Update Logs, see the same numbers, and conclude the fix didn't ship.

## Design

### D1. Data: alternates on a message

```json
"¥You seem to be able to take more damage now.": {
  "trainer": "Histia", "profession": "Fighter",
  "alternates": [ { "trainer": "Higgrus", "profession": "Healer" } ]
},
"¥Your body feels stronger.": {
  "trainer": "Master Bodrus", "effective_rank_multiplier": 0.9004,
  "alternates": [ { "trainer": "Hardia", "profession": "Healer" } ]
}
```

The top-level `trainer` stays the default. Every other entry is unchanged.

`TrainerDb`:
- registers each alternate's profession/multiplier like a primary trainer;
- includes alternates in `all_trainer_metadata` (feeds the Ranks view, zero-trainers
  toggle, Rank Modifiers catalog, CLI multiplier map and catalog: `TrainersView.tsx:210`,
  `RankModifiersView.tsx:231`, `cli main.rs:535,1492`; all key by unique name);
- new `alternates_of(default_trainer) -> &[String]`, keyed by the **default trainer
  name** the classifier already resolved. This inherits `get_trainer`'s whitespace and
  trailing-period normalization (`data/trainers.rs:96`);
- `get_trainer` unchanged.

Player-trainer guard (`data/npc_trainers.rs:74`) parses `trainers.json` itself and reads
only top-level `trainer`. Extend it to read `alternates[].trainer` (same exclusion list),
then remove Higgrus and Hardia from `EXTRA_NPC_DISPLAY_NAMES` so the existing
"short names are guarded" test actually exercises alternate derivation.

### D2. Shared ranks: record observations at scan, derive counts at finalize

Principle: **the scanner never counts a shared-message rank.** It records two kinds of
fact, sign-ups and shared-rank lines, and `finalize_characters` rebuilds the shared
trainers' counts from the full set of facts in one atomic, idempotent step.

Why (revision 2 moved ranks incrementally, and review 2 showed what that costs):
- No per-observation counting, so there is nothing to make atomic with an insert.
- No transfers that can be half-applied: a crash before the rebuild commits changes
  nothing, and a rerun recomputes the same answer.
- Result is independent of source order, of operation boundaries, and of restarts between
  scan and finalize.
- No "touched characters" set: all observations are rebuilt every time (tens to hundreds
  per character).
- The same pattern as `depart_observations`, where totals are derived from stored facts
  (`db/queries/depart.rs:70`).

#### D2a. Detection: `SignupTracker`

A per-file tracker fed every non-replay line in `scan_bytes` (after the movie filter,
`parser/mod.rs:632`), alongside `BlackjackTracker`. It works on the raw message, so it
sees speech the classifier filters out. It is **not** a `LogEvent`, so `has_game_session`
(`parser/mod.rs:2050`, shared with `would_scan` at `:2247`) is unchanged and a bystander's
welcome never turns a no-session file into a session.

State: `pending: Option<{ trainer, char_name, confirmed, lines_left }>`.

| Line | Action |
|---|---|
| `T (asks\|says), "Are you sure you would like to train with me?"` | `pending = {T, active name, false, 30}` |
| `N says, "yes"` (optional trailing `.`/`!`), N == active name (case-insensitive) == `pending.char_name` | `confirmed = true` (a `yes` before the question confirms nothing) |
| `N says, "no"`, N == `pending.char_name` | clear |
| `T says, "Welcome aboard. Go into the world…"`, T == `pending.trainer`, confirmed, active == `pending.char_name` | emit `Signup(T)`; clear |
| `[¥•]? You begin training with X.` (existing regex, `player_signals.rs:86`) | emit `Signup(None)` for the active character; clear |
| any `Welcome to Clan Lord` / `Welcome back`, or active character changes | clear |
| every other line | `lines_left -= 1`; clear at 0 |

The 30-line window is far above the largest gap observed (4 lines plus a
`Just tell me yes or no` prompt). It stops a stale ask from being confirmed by an
unrelated `yes` much later. Binding `char_name` closes the cross-character case. All NPC
sign-ups are recorded, not only Higgrus/Hardia, because any later sign-up ends the
earlier one.

#### D2b. Tail scans: `prefix_state`

Replace `active_char_at_offset` (`parser/mod.rs:1894`) with
`prefix_state(bytes, offset, fallback: Option<(i64, String)>) -> PrefixState`. The function
already decodes the whole prefix, so this adds no extra pass.

`PrefixState` = `{ active, movie: MovieFilter, signup: SignupTracker, line_count, last_ts }`:
- `active` is the last welcome's character, else `fallback`, exactly as callers compute
  today with `.or_else(...)`;
- `signup` keeps only the pending state (emits are discarded: those sign-ups were stored
  on the earlier pass);
- it keeps today's `load_override_config` call (`:1912`);
- `last_ts` is the last non-replay line's parsed timestamp. When present it seeds
  `scan_bytes`' `current_date` (`:599`), the time inherited by untimestamped lines.
  Without it a tail scan would stamp an untimestamped rank line with the filename time,
  where a full scan stamps it with the preceding timestamp. A noon sign-up could then
  apply on a full scan and not on a tail scan. `current_date` is otherwise only the
  per-line date: the filename start-date fallback reads `filename_date` and
  `had_real_timestamp` (`:1147`), which stay as they are.

The four tail callers pass their existing fallback: folder scan `:381`, loose file `:475`
(passes `None`, as today), progress folder scan `:1537`, explicit files `:1730`.
`scan_bytes` takes the movie, signup and line-offset seeds as new parameters.

Deliberately unchanged: `scan_bytes`' login, `found_login`, `first_date_str` and
start-date fallback logic (`:584`, `:1147`). Carrying them across offsets is a separate
change with its own risks, and attribution doesn't depend on them. Fixed in passing: a tail that starts inside movie
playback (CLAUDE.md "Movie playback" lists this as a known gap). It must be fixed here
anyway, otherwise replayed sign-ups would be recorded.

#### D2c. Storage

```sql
CREATE TABLE trainer_signups (
  file_path TEXT NOT NULL, line_no INTEGER NOT NULL,
  character_id INTEGER NOT NULL, at TEXT NOT NULL,
  trainer_name TEXT,                  -- NULL = player teacher
  PRIMARY KEY (file_path, line_no)
);
CREATE TABLE shared_rank_observations (
  file_path TEXT NOT NULL, line_no INTEGER NOT NULL,
  character_id INTEGER NOT NULL, at TEXT NOT NULL,
  default_trainer TEXT NOT NULL,      -- 'Histia' / 'Master Bodrus'
  PRIMARY KEY (file_path, line_no)
);
CREATE INDEX trainer_signups_char_at ON trainer_signups(character_id, at);
ALTER TABLE trainers ADD COLUMN shared_ranks INTEGER NOT NULL DEFAULT 0;
```

- `line_no` is absolute within the file (prefix `line_count` + index), so it is stable
  across tail scans. Inserts are `INSERT OR IGNORE`.
- There is no per-file transaction today. Transactions wrap a folder (`parser/mod.rs:254`)
  or a file batch (`:1621`), and a file that errors part-way is logged and skipped
  (`:408`), with its partial writes committed with the folder and the file left out of
  `log_files`, so the next scan reads it again. That is existing behaviour for kills
  etc., and this design doesn't change it. For the new facts, replay is harmless: the
  `(file_path, line_no)` key ignores already-stored lines, and counts are derived from the
  stored set, so a replayed file can't double-count shared ranks.
- `trainers.shared_ranks` is the part of `trainers.ranks` that came from observations.
  Rows from Scribius import (`import.rs:366`) and every non-shared trainer keep 0 and are
  never touched.
- `reset_log_data` deletes both tables and sets `shared_ranks = 0` in the same `UPDATE`
  that zeroes `ranks` (`log_file.rs:113`). The reset batch becomes one SAVEPOINT, since
  today it is an unwrapped multi-statement batch (`:85`). `delete_all_data` drops
  everything.
- `set_rank_override` zeroes `ranks` when switching to an override mode
  (`db/queries/trainer.rs:284`). It zeroes `shared_ranks` in the same statement.
  Restores, imports and `move_rank_override` (`rank_override.rs:169`) all go through it.
- Schema goes through the existing paths: `CREATE TABLE IF NOT EXISTS` in
  `create_tables`, and the column via `migrate_tables`' tolerant `ALTER` list (as for
  `merged_into`, `schema.rs:278`). Legacy DBs start with empty observation tables and
  `shared_ranks = 0`, so derivation is a no-op on them until they scan.
- `at` uses the scanner's `YYYY-MM-DD HH:MM:SS` from both timestamp and filename
  fallbacks (`parser/timestamp.rs:7`), so string order is time order.

`trainers.shared_ranks` is internal bookkeeping, read only by the derivation. It is
**not** added to the public `Trainer` model, so the readers that build `Trainer` by hand
(`get_trainers_from`, merged reader `merge.rs:276`) stay unchanged.

At scan time the `TrainerRank` arm (`parser/mod.rs:847`) checks first: if
`alternates_of(trainer)` is non-empty, insert the observation. **Only if the insert added
a row** (`changes() == 1`), push its key onto the parser's per-operation `new_shared_obs`
list (cleared in `begin_scan_op` when depth is 0, like `scan_token`), and count it in
`file_result.events_found` (as `:855` does for ranks today). Then stop: no rank write
and no skip report happen there. A re-read line that inserts nothing is not an event.
An inserted observation counts as an event even if the finalize step later finds it
override-skipped. That step reports skips separately (D2e step 5), so a shared-rank-only
Update never says "nothing found" while ranks change. A forced re-scan of a stored line adds nothing to
the list, and the primary key makes the list duplicate-free.

#### D2d. Resolution rule

For an observation O (character C, default D), the sign-ups that apply are those of
character C where `S.at < O.at`, or `S.at == O.at` with S in the same file and
`S.line_no < O.line_no`.

- Take the applying sign-ups with the greatest `at`. Within one file, take the greatest
  `line_no`.
- If two or more remain (equal `at`, different files) and they name different trainers,
  the result is **ambiguous, so use D**. This is conservative and independent of row
  order.
- If the chosen sign-up's trainer is D or in `alternates_of(D)`, use it. Otherwise
  (another trainer, a player teacher, or no sign-up), use D.

#### D2e. Derivation in `finalize_characters`

New step `derive_shared_ranks()`, run in `finalize_characters` before
`determine_profession` (so class inference sees corrected ranks).

1. **Override config.** For each character with observations, call
   `load_override_config`. Finalize also runs from a fresh parser on DB open
   (`gui commands/database.rs:16`), so nothing can be assumed loaded.
2. **Count.** In memory, resolve every observation, then keep it only if
   `should_count_rank(C, resolved, at)`. The resolved trainer's own override applies, and
   `date_after`'s calendar-day cutoff (`:2076`) is preserved. Count per (C, trainer), with
   the max `at`.
3. **Apply, in one SAVEPOINT `derive_shared`.** For every (C, trainer) whose stored
   `shared_ranks` differs from the new count, **or whose `ranks < shared_ranks`**, upsert:
   `ranks = MAX(ranks - shared_ranks, 0) + new`, `shared_ranks = new`,
   `date_of_last_rank = max(date_of_last_rank, new_max_at)`.
   - A new row takes `effective_multiplier` from `TrainerDb`.
   - Rows whose count falls to 0 stay, with `shared_ranks = 0`.

   The `MAX(…, 0)` makes the invariant self-healing. Nothing in the codebase lowers
   `ranks` except by setting it to 0 (`reset_log_data`, `set_rank_override`; grep shows no
   other writer). So `ranks < shared_ranks` can only mean the non-shared part was wiped
   too, and 0 is then the right remainder. If a future writer zeroes `ranks` and forgets
   `shared_ranks`, the result is still correct rather than negative.
4. **Last scan, in the same SAVEPOINT.** Filter `new_shared_obs` to the observations
   counted in step 2.
   - If that set is empty, don't touch the shadow tables or the token. A shared-rank-only
     operation whose new ranks were all override-skipped is not a "scan that wrote
     ranks", and the previous Last scan stays intact.
   - Otherwise, inside `derive_shared`: remember `prev = scan_token.get()`, call
     `note_scan_write()`, then add each counted new observation to `scan_trainers` via
     `upsert_trainer_rank_scan`.
     - If `scan_token` is `None` (the operation wrote nothing else), `note_scan_write`'s
       nested `mark_scan_write` SAVEPOINT (`scan_scope.rs:51`) clears the shadows and
       records the new token, all inside `derive_shared`. A rollback of `derive_shared`
       undoes the clear too, so the previous Last scan survives, and we
       `scan_token.set(prev)` to match.
     - If it's `Some`, the operation already owns the Last scan and the call is a no-op.
     - `scan_token` survives the end of an operation (reset only by the next
       `begin_scan_op`, `:94`), so a finalize run from a CLI command after its scan joins
       that scan.
   - Drain `new_shared_obs` only after `RELEASE derive_shared` succeeds.
5. **Skip report.** After release, for the observations in `new_shared_obs` that step 2
   didn't count, add one `info` `process_logs` entry per (character, trainer):
   `Skipped N rank(s) for T (C) — override active`, naming the resolved trainer.

Resolution runs per character: load its sign-ups once, sorted by `(at, file_path,
line_no)`, and binary-search for each observation. There is no per-observation query.

Properties: idempotent (a second finalize changes nothing), atomic (lifetime counts and
Last scan commit together or not at all), and order-independent.

**Recovery scope.** Observations that committed but missed their finalize (the process
stopped between scan commit and finalize) are counted by the next finalize: the next
scan, Update Logs, CLI `scan` / `scan-files` / `update` / `rescan`, or GUI DB open
(`gui commands/database.rs:16`). CLI read-only commands (`summary` `main.rs:692`,
`trainers` `:1061`) open the DB without a parser and don't finalize. Until then they show
the totals from before the interrupted scan, missing its shared ranks rather than
misattributing them, because the scanner never counts shared ranks itself. Strictly:
the previously derived shared totals and attribution remain visible. An older Higgrus
sign-up added by the interrupted scan only moves ranks at the next finalize. We accept
this rather than make read commands write.

Behaviour change, documented: shared trainers follow the **current** override config at
every finalize. Other trainers apply overrides at scan time. For `override_until_date`
this is what `effective_ranks` assumes anyway (`models/trainer.rs:78-86`).

Accepted limits, documented:
- `date_of_last_rank` is not rolled back on a trainer that loses ranks (it is last-write
  today anyway, `db/queries/trainer.rs:72`).
- Sign-ups resolve per source character id, so merged characters don't share sign-ups.
  Merge sources are typically Scribius imports, which have no sign-ups.
- Changes to the attribution of observations from earlier operations don't show in the
  Last-scan view.
- A file whose only content is a sign-up and no other session evidence is dropped as
  "no game session" before the tracker runs (`:365`). The corpus has zero such files
  (every file with a `Welcome aboard` has a login welcome), so we accept this rather than
  make `has_game_session` speaker-aware.

`ApplyLearningRank` (`Aitnos … Higgrus's lessons`) already names its trainer; no change.

#### Considered and deferred: class-based fallback

Histia is a first-circle fighter trainer and Higgrus a basic healer trainer, so an
unresolved `take more damage` line could plausibly go to Higgrus for healers. Deferred
because:
- no source confirms that healers cannot train Histia;
- profession provenance isn't stored, and Higgrus ranks would push detection towards
  Healer (circular);
- it can't decide Bodrus vs Hardia.

### D3. Rodnus is Fighter; pin confirmed facts

- `trainers.json`: Rodnus `profession: "Fighter"` (restores `0240dee`; clump Fighter
  Trainers).
- Fix the assert at `data/trainers.rs:288`.
- New test `community_confirmed_trainer_facts`, each fact with its source in a comment:
  - Rodnus = Fighter;
  - Histia = Fighter, with Higgrus (Healer) as its alternate;
  - Master Bodrus untagged, with Hardia (Healer) as its alternate;
  - Master Spirtus, Master Mentus and Heen Slostid untagged.

  The file also carries a comment saying "bump `TRAINER_REVISION` (D4) when changing
  trainer data or attribution".
- Flagged, not changed (no reliable source): `Spiritus` = Healer (`trainers.json:542`) vs
  untagged `Master Spirtus`; `Ethereal Training` = Mystic (`:215`).
- No separate display-group field (nobody asked; grouping by teaching guild is
  defensible).

### D4. Tell users when a rebuild would change their data

- Core constant `TRAINER_REVISION: u32 = 1`, bumped by hand with any trainer data or
  attribution change that makes existing scanned ranks wrong.
- `db_meta.trainer_revision` means "the scanned data here was built at revision R":
  - `reset_log_data` and `delete_all_data` set it to current (all later scanned data is
    built by current code, so "reset, then a normal scan" counts);
  - on open, if `log_files` is empty, set it to current;
  - missing with scanned files reads as 0.
- `db_meta.trainer_revision_ack` is set to current on dismiss. `delete_all_data` clears
  it.
- One core function `trainer_data_status() -> { stale, acknowledged, modifiers_to_review }`
  serves both GUI and CLI:
  - `stale`: `log_files` non-empty and revision < current;
  - `acknowledged`: ack ≥ current;
  - `modifiers_to_review`: see D6 (all four shared-message trainers, by rank mode).
- GUI: a dismissible banner when `stale && !acknowledged`. It says rank detection
  improved; that Rescan Logs rebuilds ranks from your log folders; that data not in
  those folders, such as Scribius-imported ranks or logs on a drive that isn't
  connected, is reset by a rescan; and it lists `modifiers_to_review`. Always showing the
  caution replaces the inferred "imported" heuristic from revision 2: a missing
  `log_files` row can't prove import provenance. Final wording to be approved by the
  user.
- CLI: `trainers` and `summary` print the same text (caution included) to stderr when
  `stale`.
- Not done: automatic rescan; automatic modifier removal; changing
  `finalize_characters`' keep-stored-profession rule.

### D6. Never clobber a user's own fixes

Players have lived with this bug, so some will have worked around it by hand. What they
could have done, and how each survives:

| Workaround | Where it lives | Effect of this change |
|---|---|---|
| Higgrus/Hardia ranks typed into Rank Modifiers (or imported JSON) | `trainers.modified_ranks` / `rank_mode` / `override_date`, backup file | Never written. Listed for review (below), because in additive modes logged ranks now stack on top |
| Negative modifier on Histia or Master Bodrus to cancel misattributed ranks | same | Never written. Listed for review, because after a rescan those ranks have already moved, so the negative modifier subtracts twice |
| Override mode on any of the four | same | Never written. Override ignores logged ranks, so nothing double-counts. Listed so the user knows logged ranks now exist |
| Overrides saved under `Higgrus`/`Hardia` and shown under "Renamed or unknown", or moved elsewhere with Move to… | backup file, `trainers` rows | Never written. Once the names are in the catalog they show as normal trainers. A moved override is preserved wherever it now lives, but appears in the review list only if its current destination is one of the four trainers; we don't guess what it was originally for |
| A backed-up Higgrus override that isn't in this DB (Restore / Discard banner) | backup file | Still restorable. Derivation creating a plain Higgrus row doesn't make it "present": `missing_with_ids` checks for a non-zero modifier or non-default mode, not row existence (`rank_override.rs:78,214`). Restore goes through `set_rank_override`, which zeroes `shared_ranks` with `ranks` (D2c) |
| Merging an imported or duplicate character into another | `characters.merged_into` | Never written |
| Profession override (e.g. Healer set by hand because Rodnus made a Fighter look like a Healer) | `characters.profession_override` | Never written; it already beats detection (`parser/mod.rs:1938`) |
| Trainer notes such as "these are really Higgrus" | `trainers.notes` | Never written |

Rule, scoped to what this change adds: the new derivation writes only
`trainers.ranks`, `shared_ranks`, `date_of_last_rank` and `effective_multiplier`, and its
conflict-update clause names only those columns. The user-entered columns are never
written by anything this change adds: `modified_ranks`, `rank_mode`, `override_date`,
`notes`, `characters.profession_override` and `characters.merged_into`. Existing
behaviour stays as it is:
- log-derived `apply_learning_*` counters are still incremented by scans and cleared by
  reset;
- ordinary rank upserts still refresh `effective_multiplier`;
- GUI open still rewrites the backup file via `backup_all` (`rank_override.rs:183`). It
  adds new entries and replaces the value of an existing character/trainer entry with
  the DB's current override.

Because this change never writes override columns, it never causes a backup entry to be
added, replaced or removed. That is the promise, not byte-identical files. Nothing is converted automatically: we can't tell a deliberate negative
Histia modifier from a workaround.

`trainer_data_status().modifiers_to_review` therefore covers **all four** trainers
(Histia, Higgrus, Master Bodrus, Hardia) on any character, for any of:
- `modified_ranks != 0` in `modifier` or `override_until_date` mode (these add logged
  ranks);
- `rank_mode = 'override'` (logged ranks are now available but hidden).

Each entry carries `character`, `trainer`, `rank_mode` and `modified_ranks`, so the
banner can say why it is listed. This replaces D4's Higgrus/Hardia-only list.

### D5. Fighter stats

`Hardia` is already a formula key in both implementations; crediting ranks under that
exact name is enough. Higgrus is not in the formulas; leave it out.

## Tests

- `TrainerDb`: alternates parsed; `alternates_of`; alternates in `all_trainer_metadata`;
  `get_trainer` still default; a message without its trailing period still resolves and
  still has alternates.
- Guard: Higgrus/Hardia guarded via alternate derivation (after removal from the extra
  list).
- `SignupTracker`:
  - full exchange with `asks` → `Signup(T)`, and with `says`;
  - `yes` before the question only → nothing; `yes` both before and after → `Signup(T)`;
  - bystander `yes` → nothing; `no` → nothing;
  - character switch between yes and welcome → nothing;
  - question, then 31 lines, then yes+welcome → nothing;
  - player teacher → `Signup(None)` and clears a pending Higgrus confirmation, so a later
    Higgrus welcome is ignored;
  - replayed movie lines ignored.
- Scan + finalize (in-memory DB, synthetic logs):
  1. Higgrus sign-up then `¥You seem to be able to take more damage now.` → Higgrus 1,
     Histia 0, in `trainers` and `scan_trainers`.
  2. No sign-up → Histia. 3. Bystander sign-up → Histia. 4. Higgrus then player teacher
     → Histia.
  5. Sign-up in file A, rank in later file B → Higgrus.
  6. Hardia → Bodrus message → Hardia; Detha sign-up → Bodrus.
  7. Two sign-ups in one timestamp-less file (Higgrus, then Detha), then the rank → Histia.
  8. Equal-`at` sign-ups in two files naming different trainers → default.
  9. Tail scans:
     - complete sign-up before the offset;
     - ask/yes before the offset and welcome after, in a file with no login welcome
       (fallback character only);
     - a tail starting inside movie playback, whose replayed sign-up and rank are ignored.
  10. Order independence: ranks in source 1 and an older sign-up in source 2 → Higgrus.
      Sources reversed → same result.
  11. Restart: scan without calling finalize, then a fresh parser's finalize → correct.
      Finalize twice → unchanged.
  12. Overrides: Histia override-skip, Higgrus not → Higgrus counted, and the reverse; the
      skip report names the resolved trainer.
  13. `scan --force` twice → still 1.
  14. Scribius-imported Histia row (`shared_ranks` 0) + scanned observations → imported
      ranks preserved, observations added on top.
  15. `has_game_session` unchanged by a bystander welcome in an otherwise empty file.
  16. `reset_log_data` clears both tables, zeroes `shared_ranks`, sets `trainer_revision`.
  17. Failure injection inside `derive_shared`, with a populated previous Last scan and
      a shared-rank-only operation: `trainers`, `scan_trainers` and `scan_kills` are
      unchanged, `last_scan_token` is unchanged, the in-memory token is restored, and
      `new_shared_obs` is kept, so the next finalize succeeds.
  18. Rank modes: a row with `ranks=10, shared_ranks=10`, switched to
      `override_until_date` with 2 post-cutoff observations → 2 (not -8). Switch back to
      modifier → all observations counted. `move_rank_override` onto Higgrus with derived
      ranks → consistent. Clear overrides → consistent.
  19. Shared-only operation where every new observation is override-skipped → previous
      Last scan intact.
  20. Tail scan whose appended untimestamped rank line follows a timestamped noon sign-up
      in the prefix → same attribution as the full scan.
  21. Import order: Scribius import, then scan (covered by 14), and scan, then Scribius
      import into the same DB (existing `INSERT OR IGNORE` leaves the scanned row alone;
      assert nothing goes negative and derivation is unchanged).
  22. Partial-file failure: a file errors after some observations were inserted; the
      retry re-reads it, and counts equal a clean scan.
- D3 facts test. D4: fresh DB, stale, acknowledged, import-only DB not stale, delete clears
  ack, modifier listing by rank mode.
- Real-data comparison: tane's 58 Bodrus and Ruuk's 11 Histia unchanged. Ruuk's
  2018-05-20 Ching Shih bystander is not recorded as a sign-up.

## Rollout

- Existing users need one Rescan Logs; the D4 banner says so. Release notes to be
  written and approved by the user.
- Update CLAUDE.md item 4 (alternates, observations + derivation, revision banner, the
  override-timing difference), the "Movie playback" note (tail gap fixed), and test
  counts.
- Ask Kim / noivad for a real Higgrus and a real Hardia log snippet (sign-up through the
  first rank line) to use as fixtures, and to confirm Higgrus's message.

## References

- Clump: Trainer_Rank_Messages (rev 2702), Fighter_Trainers, Healer_Trainers
  (`https://clump.clanlord.net/library/index.php?title=…`)
- RankCounter27 `rankmessages.txt` / `trainers.txt` (line 125 Higgrus):
  https://raw.githubusercontent.com/maxtraxv3/Macros/refs/heads/main/clanlord%20apps/RankCounter/RankCounter27/
- Code (core unless noted): `data/trainers.rs:96,288`; `data/npc_trainers.rs:22,74`;
  `parser/line_classifier.rs:482`;
  `parser/mod.rs:94,105,254,365,381,408,475,584,599,620,632,847,1147,1229,1537,1621,1730,1887,1894,1912,1947,2050,2076,2247`;
  `parser/player_signals.rs:86`; `parser/timestamp.rs:7`; `db/import.rs:366`;
  `db/queries/log_file.rs:85,113,122`; `db/queries/trainer.rs:72,284`;
  `db/queries/rank_override.rs:169`; `db/queries/scan_scope.rs:43,51`; `db/schema.rs:278`;
  `db/queries/depart.rs:70`; `models/trainer.rs:78`; gui `commands/database.rs:16`,
  `commands/mod.rs:112`; cli `main.rs:574,656,692,1061`
- Commits: `0240dee`, `1cc8384`, `66771bb`, `514c023`, `ad69ab6`, `d6b2861`
