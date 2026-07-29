# Player Trainers & Bestiary Links — Design

Date: 2026-07-29

Two independent features, specified together because they were requested together.
They share no code and can be implemented and shipped in either order.

- **Part 1** — stop player-run trainers (Fenwick, Marrow, Brindle, …) polluting the Trainer
  Checkpoint Progression graph.
- **Part 2** — right-click a creature in Ranger Stats → Top Targets to open the
  Clan Lord Bestiary.

---

## Part 1 — Player trainers

### Problem

Players who own a training ledger can teach other players, and when they do they use the
*exact same* greeting wording as an NPC trainer:

```
Fenwick says, "Hail, Ruuk. You are one of my better pupils."
```

`parser/line_classifier.rs` matches this with `TRAINER_GREETING`, which accepts any speaker
name. The only filter applied in `parser/mod.rs` is that the *hailed* name equals the active
character. So player teachers become first-class series in the Trainer Checkpoint Progression
graph, sitting alongside Balthus and Histia as though they were real trainers.

### Why the obvious fix is wrong

The tempting fix — keep only speakers found in `trainers.json` — destroys real data.
`trainers.json` is keyed on rank-*message* short names, not in-game display names. In the
reference corpus (`~/Library/Application Support/com.dfsw.Amanuensis/amanuensis.db`), 56
distinct speaker names have produced checkpoints and **16 of them are legitimate NPC trainers
absent from `trainers.json`**:

- Missing outright: `Higgrus`, `Chronos`, `Hardia`, `Splash O'Sul`, `Diggin`,
  `Anan Faure`, `AnDeux Faure`, `AnQuart Faure`, `AnSept Faure`, `AnTrix Faure`
- Spelling drift: `Tra'Kning` vs `TraKning`, `Par Troon` vs `ParTroon`,
  `Metta Sylpha` vs `Sylpha`, `Respin Verminbane` vs `Respin Verminebane`

An allowlist-only filter would silently delete roughly a third of the graph.

### Approach: behavioural detection with an allowlist guard

Detect **players**, not trainers. Player characters do things NPCs never do. Nine line
forms, all observed in the reference corpus, identify a name as a player:

| Signal | Example |
|---|---|
| `clanning` | `Fenwick is now Clanning.` / `Fenwick is no longer Clanning.` |
| `sharing` | `Fenwick is sharing experiences with you.` |
| `sharing` | `You begin sharing your experiences with Fenwick.` |
| `sharing` | `You are sharing experiences with Kalian, Jazz and Fenwick.` |
| `thinks` | `Fenwick thinks to you, "helpful: …"` |
| `ledger` | `Fenwick writes Thistledown's name in her training ledger.` |
| `ledger` | `Bramwell Gorse writes your name in her training ledger.` |
| `ledger` | `Rushlight shows his training ledger to you.` |
| `offer` | `¥ Bramwell Gorse offers: "I can teach you to be more receptive to healing."` |
| `offer` | `¥ To accept her offer, say: I accept Bramwell Gorse as my teacher.` |

Both the Mac `¥` and Windows `•` prefixes must be handled, and the offer lines use curly
quotes (`“…”`) in real logs, not straight quotes.

**Validation.** Sweeping the 261,433 indexed lines of the reference corpus with exactly these
patterns yields:

- **Flagged (3):** `Fenwick` (clanning ×66, share-list ×49, thinks ×16, ledger ×1),
  `Bramwell Gorse` (clanning ×16, offer, accept, ledger), `Tallow` (clanning ×3, offer, accept,
  thinks).
- **Not flagged (52):** every legitimate NPC trainer, including all 16 of the awkward names
  listed above.

Zero false positives, zero false negatives on the reference data.

**Guard.** A bundled NPC display-name list. A name on it is never written to
`known_players`. The list is **additive-only**: presence protects, absence flags nobody
(behaviour alone flags). An incomplete guard list therefore cannot cost the user data. It
exists to survive a hypothetical name collision, not to carry the filter.

Seed it from `trainers.json`'s 115 short names plus the display-name variants the corpus
revealed (`Tra'Kning`, `Par Troon`, `Metta Sylpha`, `Respin Verminbane`, the four Faures,
`Higgrus`, `Chronos`, `Hardia`, `Splash O'Sul`, `Diggin`).

### Rejected: translating player → real trainer

A ledger covers exactly one trainer, and the offer text identifies it — `"I can teach you to
be more receptive to healing."` is Rodnus, confirmed in-log by the player replying "ah no
rodnus". The mapping could even be learned without curation, since NPC trainers advertise the
same sentence in plain speech (`Duvin Beastlore says, "I can teach you to study the ways of
various creatures."`).

Not doing it. Coverage would be uneven — `Fenwick` produced checkpoints but never made an offer
in the corpus, so Fenwick could never be translated — and the volume is 21 rows out of ~7,000.
The user's decision is to hide these rows, with an option to reveal them.

### When detection runs

Incrementally, never as a repeated full sweep:

1. **During every scan/update** — lines already being parsed are additionally tested against
   the nine player patterns. New names are inserted into `known_players`; already-known names
   are a no-op. Nine regex tests per line, on lines already in memory.
2. **Once, at migration** — the `log_lines` backfill sweep described below.
3. **Never at display time** — filtering is a `LEFT JOIN`.

`known_players` only ever accumulates. Nothing is recomputed per view or per character.

### Diagnostic for undetected players

The residual risk is a player trainer who never clans, shares, thinks-to-you, or makes an
offer anywhere in the logs. No behavioural signal fires, so they are silently treated as an
NPC trainer. With a modest log collection this is plausible.

Mitigation: when a checkpoint is recorded from a speaker that is **neither** in
`known_players` **nor** on the NPC guard list, emit a `process_logs` entry at `info`:

```
Checkpoint from unrecognised trainer "Brindle" — if this is a player, it will appear in the graph
```

This makes suspicious names visible in the Process Logs panel instead of silently polluting
the graph, and doubles as the feedback loop for growing the guard list. It must be
rate-limited to one entry per distinct name per scan, since a single name can produce
hundreds of checkpoints.

Note this diagnostic fires for legitimate-but-unlisted NPC trainers too (`Higgrus`,
`Chronos`, the Faures, …) until the guard list is extended. That is the intended behaviour:
the entry says "unrecognised", not "player", and every such name is worth a look.

### Data model

New global table. A player is a player regardless of which character met them, so this is
not keyed by `character_id`.

```sql
CREATE TABLE known_players (
  name       TEXT PRIMARY KEY,
  evidence   TEXT NOT NULL,   -- strongest signal seen: ledger|offer|thinks|sharing|clanning
  first_seen TEXT NOT NULL
);
```

`evidence` exists so the UI can explain *why* a name was classified as a player. Precedence
when several signals are seen, strongest first: `ledger`, `offer`, `thinks`, `sharing`,
`clanning`.

Rows in `trainer_checkpoints` are **never deleted or modified** by this feature.

### Filtering: query time, not scan time

The three checkpoint queries in `db/queries/checkpoint.rs` —
`get_all_trainer_checkpoints`, `get_latest_trainer_checkpoints`,
`get_trainer_checkpoint_history` — gain an `include_players: bool` parameter and a
`LEFT JOIN known_players kp ON kp.name = trainer_name`, filtering on `kp.name IS NULL`
unless `include_players` is set.

This is the key property: the instant Amanuensis learns a name is a player, **every historic
checkpoint from that name disappears from the graph, with no rescan**. Scan-time flagging
would have required a rescan every time detection improved.

`TrainerCheckpoint` gains `is_player: bool` so that revealed rows can be marked in the UI.

### Backfill

The migration creates `known_players` empty, then performs a one-time sweep of the
`log_lines` FTS table using the same nine patterns. This is the exact procedure validated
above; it recovered all three players from the reference corpus. At 261k rows the sweep is
sub-second in Rust.

If log indexing was disabled (`--no-index`), `log_lines` is empty and the sweep finds
nothing. The table then populates on the next Update/Rescan, and full historical coverage
requires one full Rescan Logs. This must be documented in `CLAUDE.md` alongside the other
rescan-required notes.

The sweep runs once, guarded so it does not repeat on subsequent opens.

### UI

- New `showPlayerTrainers` preference in the Zustand store, default **off**, persisted to
  `localStorage` via `STORAGE_KEYS`, following the existing `showZero` / `showEffective`
  pattern in `trainersViewState`.
- A **"Show player trainers"** checkbox on the Trainer Checkpoint Progression card in
  `CVGraphView.tsx`.
- The same preference governs the checkpoint badges in `RankModifiersView.tsx`, so the two
  surfaces never disagree.
- When revealed, player-sourced series carry a "player" pill in the legend and tooltip. They
  must never be mistakable for NPC trainer data.
- CLI parity: a `--include-players` flag on `amanuensis checkpoints`
  (`Commands::Checkpoints` / `cmd_checkpoints` in `amanuensis-cli/src/main.rs`), defaulting
  to off to match the GUI.

### Components

| Unit | Purpose | Depends on |
|---|---|---|
| `parser/player_signals.rs` | Recognise the nine player-only line forms; return `(name, signal)` | patterns only |
| `data/npc_trainers.rs` | The additive-only guard list + `is_known_npc_trainer(name)` | nothing |
| `db/queries/player.rs` | `upsert_known_player`, `is_known_player`, backfill sweep | schema |
| `db/queries/checkpoint.rs` | `include_players` parameter on the three queries | `known_players` |

`player_signals.rs` is deliberately separate from `line_classifier.rs`: its signals are
orthogonal to the event taxonomy, several of them fire on lines the classifier already
handles (clanning) or deliberately ignores (speech), and keeping it standalone makes the
nine patterns testable in isolation.

### Testing

- One unit test per signal wording, covering `¥` and `•` prefixes and curly quotes.
- Guard test: the 16 awkward real trainer names above are never flagged.
- Query tests: `include_players` true and false.
- Real-data comparison test (`--ignored`): the local corpus flags exactly
  `{Fenwick, Bramwell Gorse, Tallow}`.
- Backfill test: a DB with populated `log_lines` and empty `known_players` fills correctly
  and is idempotent on a second run.
- Diagnostic test: an unrecognised speaker emits exactly one `process_logs` entry per scan
  regardless of how many checkpoints it produces, and a guard-listed speaker emits none.

---

## Part 2 — Bestiary links from Top Targets

### Problem

Ranger Stats → Top Targets lists creature names with no route to the upstream bestiary.

### The constraint

`bestiary.clanlord.net` has **no per-creature page**. Verified:

- Per-family pages exist at `https://bestiary.clanlord.net/beast/<FamilyNoSpaces>.php` and
  are live for 63 of our 66 families — including `AstralElemental.php`, which is absent from
  the site's own index page.
- Family pages contain **no per-creature anchors**, so a creature cannot be deep-linked
  within its family page.
- `search.php` is AJAX-only: it assembles its query string in JavaScript and injects results
  client-side, so there is no linkable results URL. Requesting the underlying
  `beast/code/search_page_creation.php` fragment directly returns HTTP 500 or an empty body.
  Not usable.
- The 32 entries with family `Extinct` (and 1 with `EXTINCT`) have no page — both spellings
  404.

Linking to the family page is therefore the only available behaviour. The user lands on the
page containing the creature and finds it from there.

### Implementation

- New `components/shared/ContextMenu.tsx` — a generic cursor-positioned menu that closes on
  outside-click, Escape, and scroll, and is keyboard-navigable. Written generically so
  KillsView can adopt it later.
- `onContextMenu` on the creature cell in `ranger/TopTargetsPanel.tsx`. `MorphCandidate`
  already carries `family`, so the URL is derived entirely client-side — **no new Tauri
  command and no backend change**.
- `familyPageUrl(family)` in `lib/bestiary.ts`: strips whitespace from the family name and
  returns `https://bestiary.clanlord.net/beast/<slug>.php`; returns `null` for `Extinct`,
  `EXTINCT`, and empty/missing family.
- Menu items:
  - **Open in Bestiary — {Family}**, disabled with an explanatory label when
    `familyPageUrl` returns `null`.
  - **Copy creature name**.
- Opening uses `shell.open` from `@tauri-apps/plugin-shell`, already a dependency, already
  permitted for `https://` by `shell:default` in `capabilities/default.json`, and already
  used in `UpdateBanner.tsx`. No plugin or capability changes.
- Discoverability: `cursor-context-menu` on the creature name plus a one-line hint beneath
  the table, since a right-click-only affordance is otherwise invisible.

### Testing

The UI has no test runner. Verification is `tsc -b` plus a manual pass in the running app
against three cases: a normal family, `Astral Elemental` (exercises space-stripping), and an
extinct creature (exercises the disabled state).

Adding a frontend test runner is out of scope for this work.

### Out of scope

- Wiring the same context menu into KillsView.
- Reusing `KillDetailModal` from Top Targets — it takes a `Kill`, not a creature name, and
  would need refactoring.

Both are straightforward follow-ups.
