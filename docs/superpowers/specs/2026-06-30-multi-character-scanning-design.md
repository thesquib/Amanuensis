# Content-driven multi-character scanning + loose files

**Date:** 2026-06-30
**Status:** Design — pending spec review
**Scope:** core scanner (`crates/amanuensis-core/src/parser/`). Separate from the Update Logs
branch; lands on its own branch and requires a one-time Rescan for existing users.

## Problem

The parser assumes **one character per file**: `extract_character_name` returns the *first*
`Welcome to Clan Lord, X!` and the whole file is scanned under that single `char_id`. Every
per-line event filter compares against that one name, so any **other** character's events in
the same file are silently dropped. Two real situations break this:

1. **Multi-character files.** Some clients write several characters' sessions into one file,
   each introduced by its own `Welcome to Clan Lord, …!`. Today only the first character's
   data survives; the rest is lost.
2. **Loose files.** `CL Log` files sitting **directly in a log root** (not inside a
   character subfolder) are never enumerated — `scan_folder_inner` /
   `scan_folder_with_progress_inner` iterate subdirectories only. Such files are never
   scanned at all (and, as of the Update Logs work, can leave the pending badge stuck).

## Goals

- Attribute each event to the character active at that point in the file (multi-character
  support), via a mutable "active character" that switches on each Welcome message.
- Enumerate and scan loose `CL Log` files in a log root, attributing them by content.
- Never invent an `"Unknown"` character: when the character of a span genuinely can't be
  determined, **skip that span and log it**.
- Keep the pending-count enumeration (`char_log_files`) in lockstep with the scanner.

## Non-goals

- Character-identification **heuristics** (NPC `Welcome, <playername>` greeting votes, etc.)
  — deferred to a follow-up spec. This design leaves a clean seam for them.
- Fully flat / folderless layouts as a first-class mode (loose files are handled, but the
  character-subfolder model remains the primary structure).
- Changing event *parsing* (kill/death/coin/trainer regexes) — only **who** an event is
  attributed to changes.

## Design

### 1. Mutable "active character" (Approach A)

`scan_bytes` stops taking a fixed `char_id`/`char_name` and instead maintains an **active
character** as it walks lines:

- On `Welcome to Clan Lord, X!` (`WELCOME_LOGIN`): set active character to `X`
  (`get_or_create_character` on first sight within the run), and **record a login** for `X`
  (see §2).
- On `Welcome back, X!` (`WELCOME_BACK`): set active character to `X` (reconnect). **No
  login.**
- All existing per-line event filters (deaths, trainers, professions, apply-learning, kills,
  coins) already compare the parsed subject name against `char_name`; they now compare
  against the **active** character. No filter logic changes — only the active-character
  variable becomes mutable.
- **Initial** active character (before any Welcome in the scanned span):
  - character-subfolder file → the **folder name** (today's fallback);
  - loose file → **undetermined** (see §3 — its events are skipped/logged until a Welcome
    appears).

The active character is a `(char_id, name)` pair held in a small scan-state struct rather
than threaded as immutable function parameters.

### 2. Login counting (per-Welcome)

- A login is recorded for **each `Welcome to Clan Lord, X!` occurrence**, credited to `X`.
  (Three full logins as Ruuk in one file → 3 logins for Ruuk; a multi-character file credits
  each character its own logins.)
- `Welcome back` (reconnect) records **no** login.
- A freshly-scanned file (`offset == 0`) with a **determinable** character but **no** Welcome
  at all still records **1 login** for that character (today's mid-session-start behavior;
  applies to subfolder files via the folder-name fallback).
- An undetermined span (§3) records **no** login.

**Consequence — Scribius divergence (accepted).** This abandons Scribius's "exactly 1 login
per file." Single-character files containing multiple full logins will now count more logins
than before. The real-data comparison tests that assert Scribius login parity, and the
import-vs-scan login-agreement tests, **must be updated/relaxed** as part of this work.
Existing databases need a full **Rescan** to recompute logins and re-attribute events.

### 3. Undetermined characters → skip and log (no "Unknown")

When the active character is undetermined for a span (a loose file, or the portion of any
file before its first Welcome with no folder fallback):

- Events in that span are **not attributed** to anyone (skipped).
- The file is recorded once in the **process log** as
  `skipped: could not determine character (<path>)`, and counted in `ScanResult` as a skip.
- **No `"Unknown"` character row is ever created.** The existing `"Unknown"` fallbacks in
  `scan_files_with_progress_inner` and the folder walk are removed in favor of this.

Character-subfolder files are never fully undetermined (the folder name fills the initial
span), so this only affects loose files and genuinely nameless content. The skip-log is also
the **measurement** that tells us how often this happens — input for the deferred heuristics
spec.

**Heuristics seam:** active-character resolution goes through a single function
`resolve_active_character(context)` that today consults only Welcome messages + folder
fallback. A future heuristics layer plugs in here (consulted only when Welcome/folder yield
nothing) without touching the scan loop.

### 4. Loose-file enumeration (+ pending lockstep)

- The folder walk (`scan_folder_inner` / `scan_folder_with_progress_inner`) additionally
  scans `CL Log` files found **directly in the log root** (via `find_log_files(folder)`),
  in addition to the per-subdirectory scan. These are attributed by content (§1/§3).
- `char_log_files` (the pending-count enumeration added in the Update Logs work) is extended
  the same way — it already iterates subdirectories; it now also includes
  `find_log_files(log_root)`.
- **Lockstep with skip-on-undetermined:** `would_scan` (the pending decision) must mirror the
  *new* scanner behavior, not just the old one. A loose file the scanner would **skip as
  undetermined** (no determinable character) must return `false` from `would_scan` so it is
  not counted. Concretely, when `would_scan` reads a candidate loose file (it already reads
  new candidates for the dedup check), it must also confirm the file yields a determinable
  character; if not, it is not pending. This both keeps the badge honest and **resolves the
  stuck loose-file caveat**: an attributable loose file (e.g. one with a Welcome) becomes
  scannable and clears once processed; an undetermined one is simply never counted.

### 5. Offset-resume / tail-scan with multiple characters

For a grown file, `plan_file_scan` already reads the **full bytes** to verify the prefix
hash, then scans `bytes[offset..]`. To attribute the appended span correctly:

- Before scanning the tail, **re-derive the active character at `offset`** by finding the
  last Welcome message in the prefix `bytes[..offset]` (already in memory). That seeds the
  tail scan's active character.
- Welcome messages **within the appended span** switch the active character and count logins
  as in §1/§2 — so a new session appended to a growing daily log gets its login.
- The `offset == 0` "no-Welcome → 1 login" fallback (§2) applies only to full scans, never to
  tail scans (prefix welcomes are not re-seen, so not re-counted).

No schema change.

### 6. Testing

New `amanuensis-core` unit tests:
- Multi-character file: `Welcome … A`, A-events, `Welcome … B`, B-events → A and B each get
  their own events and **1 login each**.
- Per-Welcome login: a file with two `Welcome to Clan Lord, Ruuk!` → **2** logins for Ruuk.
- `Welcome back` adds **no** login but switches attribution.
- Subfolder file with no Welcome → **1** login credited to the folder-name character.
- Loose file in a log root with a Welcome → scanned and attributed by content.
- Loose file with no Welcome → **skipped + logged**, no character row, no login.
- Pre-first-Welcome span in an otherwise-attributable file → that span skipped, rest
  attributed.
- `char_log_files` includes loose files (pending/scanner lockstep).
- Tail-scan of a grown multi-character file: active character re-derived from the prefix;
  appended events attributed correctly; appended Welcome counts its login.

Update the real-data comparison tests' login expectations per §2 (logins no longer match
Scribius; document the new expected counts or relax those specific assertions).

### 7. Rollout

- One spec, two cohesive parts (mutable attribution; loose-file enumeration). Implemented on
  its own branch, after the Update Logs branch lands.
- Requires a one-time **Rescan** for existing users (`reset_log_data` + re-scan) — logins and
  per-character attribution shift. Surface this in the GUI Rescan affordance / release notes.
- Document the new model in `CLAUDE.md` (Log Format Details + Key Functional Areas), replacing
  the "one character per file / 1 login per file" assumptions.
