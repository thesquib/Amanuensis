# Kill-Frequency Max — Design Spec

**Date:** 2026-06-08
**Status:** Approved — ready for implementation planning

## Motivation

Org du Lac (author of the upstream bestiary / monster-frequency data we bundle)
asked for a way to answer, per creature:

> "What is the highest number of a single monster type someone has killed within a
> fixed period, ever?"

Collated across many players' logs, this is a **spawn-density / abundance proxy** —
the best material he has for quantitative frequency data ("orgas can spawn at
~2500/hour"). For an individual ranger it answers "what's the fastest I could
realistically finish this study/morph?" (e.g. ice-cave Mauling morphs).

His chosen method, verbatim from the design thread:

> "Strip the month/date/year from every kill, count each day, take the max of
> that — rather than a sliding window of hours."

## Scope

### Phase 1 (this spec — build now)
- Per creature, compute **two** max-ever numbers, each with the date/time they
  occurred:
  - **Best calendar day** — 24h **fixed calendar-day bins** (Org's method;
    midnight→midnight). Keeps days aligned/comparable across players for
    collation.
  - **Best 2 hours** — **sliding-window true max**: the densest any 2-hour span
    ever was, found by a two-pointer sweep over sorted kill timestamps. Catches
    bursts that fixed bins would split.
- Foundation: a per-kill **event table** with full timestamps, so both metrics
  (and any finer ones later) come from the same data with no re-scan.
- Surfaces: GUI columns in KillsView and a CLI export tool, both covering 24h
  and 2h.

### Phase 2 (noted, NOT built now)
- **Sliding-window max on the bestiary detail card** (same computation surfaced
  in a second place). Out of scope here.
- Additional bin sizes (4h/8h) if Org asks; trivial extension of the same data.

### Explicitly out of scope
- Filtering GM/invasion spawn outliers (Bastion parrots, etc.). We report **raw**
  numbers; Org handles outlier judgement downstream.

## Period model — decided

- **24h — fixed calendar-day bins** (local timestamp date, midnight→midnight).
  Org's stated method ("strip date, count per day, take max"); keeps days aligned
  across players for collation.
  - Accepted limitation: a burst spanning midnight is split across two days and
    undercounts. Org chose calendar days deliberately for the 24h metric.
- **2h — sliding window (true max).** Not bin-aligned: the densest 2-hour span
  found anywhere in the timeline. No undercount from bin boundaries. This is the
  more honest "fastest realistic burst" number and what a ranger planning a hunt
  actually cares about.

## Data model

New table, alongside (not replacing) the existing aggregated `kills` table:

```sql
CREATE TABLE IF NOT EXISTS kill_events (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    character_id  INTEGER NOT NULL,
    creature_name TEXT    NOT NULL,          -- alias-resolved, same as kills.creature_name
    verb          TEXT    NOT NULL,          -- killed | slaughtered | vanquished | dispatched (KillVerb Display)
    assisted      INTEGER NOT NULL DEFAULT 0,-- 0 = solo, 1 = helped
    timestamp     TEXT    NOT NULL,          -- "YYYY-MM-DD HH:MM:SS" (same format as kills.date_last)
    FOREIGN KEY (character_id) REFERENCES characters(id)
);
CREATE INDEX IF NOT EXISTS idx_kill_events_char_creature_ts
    ON kill_events(character_id, creature_name, timestamp);
```

We record **everything** — all four verbs, solo and assisted — and filter at query
time, so no fidelity is lost. No `log_id` column: idempotency is handled by the
existing scan machinery (see below), matching the `kills` table which also stores
no log identity.

## Scan integration & idempotency

- Populate during the existing scan pass: wherever we currently increment a
  `kills.*_count`, also append a `kill_events` row with the parsed timestamp,
  verb, assisted flag, and `log_id`.
- **Idempotency:** reuse the existing scan machinery — normal scans **skip**
  already-scanned logs (`is_log_scanned`), so events insert only for new logs and
  never double-count. A full re-scan goes through `reset_log_data()`, which we
  extend to also `DELETE FROM kill_events` (alongside its existing
  `DELETE FROM kills` etc.). No per-log delete and no `log_id` needed.
- **Backfill:** existing databases run a one-time `amanuensis scan --force
  <folder>` to populate `kill_events`. This is the same instruction already in
  CLAUDE.md after a bestiary update.

## Query layer

Single primitive — best calendar day per creature, with its date and count:

```sql
SELECT creature_name, day AS best_day_date, cnt AS best_day_count
FROM (
    SELECT creature_name,
           date(timestamp) AS day,
           COUNT(*)        AS cnt
    FROM kill_events
    WHERE character_id = ?
      -- AND assisted = 0   (when solo-only requested; see Verb/assisted handling)
    GROUP BY creature_name, day
)
GROUP BY creature_name
HAVING cnt = MAX(cnt);   -- pick the peak day; ties resolved deterministically (earliest date)
```

Cheap with the timestamp index. That covers the **24h** metric.

The **2h sliding-window** metric is computed in Rust, not SQL: fetch each
creature's kill timestamps in sorted order (the index already provides this),
then run a two-pointer sweep — advance the right edge while
`t[right] - t[left] <= 2h`, track the max `(right - left + 1)` and the window's
start time. O(n) per creature. The peak window's start timestamp is reported as
`best_2h_datetime`.

## Verb / assisted handling — decided default

- The headline max counts **all four kill verbs combined** ("total kill rate
  regardless" — Org).
- **Assisted kills are included by default** in the headline, because the primary
  goal is spawn density (an assisted kill still means the creature spawned and
  died near you).
- A **solo-only** view is available as a toggle (GUI) / `--solo` flag (CLI) for
  the "personal study rate" reading.

## Surfaces

### GUI — columns in KillsView
- Per creature, two sortable columns in KillsView:
  - **Best day: N** (with date `YYYY-MM-DD`, e.g. tooltip or secondary text).
  - **Best 2h: N** (with date/time of the peak sliding 2h window's start).
- Placement decided: KillsView columns (not the bestiary card, for now).
- Solo-only toggle reuses existing filter affordances.
- Reuses existing family/rarity/seasonal chip filters already in KillsView.
- This is GUI-first per project convention (Amanuensis is GUI-first).

### CLI — collation export tool
```
amanuensis frequency <db> [--bin 24h|2h|both] [--creature NAME] [--solo] [--by-verb] [--format csv|json]
```
- Default `--bin both` — emit 24h and 2h max columns. `--format csv` default for
  spreadsheet collation.
- Output columns:
  `creature_name, best_day_count, best_day_date, best_2h_count, best_2h_datetime`.
  With `--by-verb`, additionally break out per-verb columns
  (`killed, slaughtered, vanquished, dispatched`, and assisted equivalents) for
  each peak bin. Default output is total-only; `--by-verb` adds the breakdown.
- This is the artifact Org collates across many players.

**CLI parity requirement:** the CLI must expose the *same* metrics the GUI shows
— both the 24h calendar-day max and the 2h sliding-window max, with their
dates/times, per creature. Neither metric is GUI-only. The GUI and CLI read from
the same query/compute layer (see Query layer) so they cannot drift; that shared
layer is the single source of truth.

## Testing

- Unit tests in `amanuensis-core`:
  - `kill_events` rows written with correct verb/assisted/timestamp/log_id.
  - Re-scan of a log replaces (not duplicates) its events.
  - Daily-max query returns correct count + date, including a burst that spans a
    midnight boundary (asserting the documented split behaviour).
  - 2h sliding-window max returns correct count + window-start datetime,
    including a burst the two-pointer sweep must capture in full (no bin split).
  - GUI and CLI produce identical numbers for the same DB (shared compute layer
    — parity test).
  - Solo vs include-assisted produce different maxes on a constructed fixture.
- Extend real-data comparison tests where a known character has an obvious peak
  day (sanity, run with `--ignored`).

## Resolved decisions

- **Periods:** 24h fixed calendar-day bins **and** 2h sliding-window true max,
  both in Phase 1.
- **CLI parity:** CLI exposes both metrics, identical to the GUI, via a shared
  compute layer.
- **Per-verb breakdown:** total-only by default, `--by-verb` flag for the full
  per-verb breakdown (both supported).
- **GUI placement:** KillsView columns (Best day, Best 2h). Bestiary card deferred.
- **Verbs/assisted:** all four verbs combined; assisted included by default with
  a solo-only toggle.
