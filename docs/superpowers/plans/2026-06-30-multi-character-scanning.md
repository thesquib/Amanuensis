# Multi-Character Scanning + Loose Files — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the log scanner attribute events to the character active at each point in a file (multiple characters per file), count logins per `Welcome to Clan Lord` message, scan loose `CL Log` files in a log root, and skip-and-log content whose character can't be determined — instead of dropping all but the first character's data.

**Architecture:** `scan_bytes` gains a mutable **active character** that switches whenever a `Welcome to Clan Lord, X!` / `Welcome back, X!` line appears; the existing 40+ event handlers are left untouched by rebinding per-iteration `char_id`/`char_name` locals from that active character. Logins are counted per `Welcome to Clan Lord` occurrence (with a no-Welcome fallback for full scans). The folder walk additionally enumerates loose files in a log root; undetermined spans are skipped and logged (never an `"Unknown"` character). `pending_files`/`would_scan` are extended to stay in lockstep.

**Tech Stack:** Rust (`amanuensis-core`), `rusqlite`, `regex` (existing `patterns::WELCOME_LOGIN` / `WELCOME_BACK`). Tests: `cargo test -p amanuensis-core`.

## Global Constraints

- Spec: `docs/superpowers/specs/2026-06-30-multi-character-scanning-design.md`.
- **Active character** switches on `Welcome to Clan Lord, X!` (`patterns::WELCOME_LOGIN`) and `Welcome back, X!` (`patterns::WELCOME_BACK`). Names normalized with `titlecase_name`.
- **Logins:** one per `Welcome to Clan Lord, X!` occurrence, credited to X. `Welcome back` adds **no** login. A full scan (offset 0) with **no** `Welcome to Clan Lord` at all credits **1** login to the initial (folder-fallback) character, if one exists.
- **Undetermined → skip + log, never "Unknown".** No `"Unknown"` character row is ever created. A fully-undetermined file is recorded via `add_process_log("warn", …)` as `skipped: could not determine character (<path>)`, counted in `ScanResult.skipped`, and NOT marked scanned.
- **Subfolder files keep the folder-name fallback** as their initial active character (so they are never fully undetermined and preserve today's behavior).
- **Loose files** (directly in a log root) are enumerated and scanned, attributed by content only (initial active character = `None`).
- **`pending_files`/`would_scan` must stay in lockstep with the scanner**, including skip-on-undetermined for loose files.
- **Tail-scan (offset > 0):** the caller derives the active character at the offset from the prefix bytes already loaded by `plan_file_scan`, and passes it as the initial active character.
- Rust edition 2021; match existing error style (`?` / `crate::Result`). Run `cargo test -p amanuensis-core`.
- This diverges from Scribius's "1 login per file"; real-data login expectations are updated in Task 6.

---

## Task 1: `scan_bytes` — mutable active character for event attribution

Switch event **attribution** to a mutable active character, WITHOUT yet changing login counting (logins stay per-file via the existing `count_login`, credited to the initial character). This isolates the risky attribution refactor; existing login-count assertions stay green.

**Files:**
- Modify: `crates/amanuensis-core/src/parser/mod.rs` — `scan_bytes` (signature ~367-375, loop body ~406-871) and its three callers (`scan_folder_inner` ~248, `scan_folder_with_progress_inner` ~1183, `scan_files_with_progress_inner` ~1298). Add a test in the `#[cfg(test)] mod tests` block.

**Interfaces:**
- Consumes (existing): `patterns::WELCOME_LOGIN`, `patterns::WELCOME_BACK`, `titlecase_name`, `self.db.get_or_create_character(&str) -> Result<i64>`, `self.load_override_config(i64) -> Result<()>`, `self.db.increment_character_field(i64, &str, i64)`.
- Produces: new `scan_bytes` signature
  ```rust
  fn scan_bytes(
      &self,
      bytes: &[u8],
      initial_char: Option<(i64, String)>,
      file_path: &str,
      index_lines: bool,
      count_login: bool,
  ) -> Result<FileResult>
  ```
  and `FileResult` gains `pub attributed: bool` (true if an active character was ever set — used by later tasks). Callers pass `Some((char_id, char_name))` for subfolder files (unchanged behavior).

- [ ] **Step 1: Write the failing test**

Add to `#[cfg(test)] mod tests`:

```rust
#[test]
fn scan_attributes_events_to_active_character_within_one_file() {
    // One file containing two characters' sessions, each introduced by its own welcome.
    // Each character's kills must land on that character, not all on the first.
    let (tmp, char_dir) = create_test_log_dir();
    let body = "\
1/1/24 1:00:00p Welcome to Clan Lord, Alpha!
1/1/24 1:01:00p You slaughtered a Rat.
1/1/24 2:00:00p Welcome to Clan Lord, Beta!
1/1/24 2:01:00p You vanquished a Large Vermine.
";
    fs::write(char_dir.join("CL Log 2024-01-01 13.00.00.txt"), body).unwrap();

    let db = Database::open_in_memory().unwrap();
    let parser = LogParser::new(db).unwrap();
    parser.scan_folder(tmp.path(), false).unwrap();

    let alpha = parser.db().get_character("Alpha").unwrap().unwrap();
    let beta = parser.db().get_character("Beta").unwrap().unwrap();
    let alpha_kills = parser.db().get_kills(alpha.id.unwrap()).unwrap();
    let beta_kills = parser.db().get_kills(beta.id.unwrap()).unwrap();

    assert_eq!(
        alpha_kills.iter().map(|k| k.slaughtered_count).sum::<i64>(),
        1,
        "Alpha's Rat slaughter must be on Alpha"
    );
    assert!(
        beta_kills.iter().any(|k| k.creature_name == "Large Vermine"),
        "Beta's Large Vermine must be on Beta, not dropped"
    );
    assert_eq!(
        alpha_kills.iter().filter(|k| k.creature_name == "Large Vermine").count(),
        0,
        "Beta's kill must NOT be attributed to Alpha"
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p amanuensis-core scan_attributes_events_to_active_character`
Expected: FAIL — Beta's kill is dropped (only the first character is processed today), so `get_character("Beta")` is `None` / Beta's kills are empty.

- [ ] **Step 3: Change the `scan_bytes` signature**

Replace the signature (lines ~367-375):

```rust
    fn scan_bytes(
        &self,
        bytes: &[u8],
        initial_char: Option<(i64, String)>,
        file_path: &str,
        index_lines: bool,
        count_login: bool,
    ) -> Result<FileResult> {
```

- [ ] **Step 4: Add active-character state above the loop**

Immediately before `for line in content.lines() {` (line ~406), add:

```rust
        // The character active at the current point in the file. Switches on each welcome
        // line. Starts as the caller-provided fallback (folder name) or None for loose files.
        let mut active: Option<(i64, String)> = initial_char.clone();
```

And extend `FileResult` (definition ~1748-1753) with the new field:

```rust
#[derive(Debug, Default)]
struct FileResult {
    pub lines_parsed: usize,
    pub events_found: usize,
    pub override_skips: HashMap<String, u32>,
    pub attributed: bool,
}
```

- [ ] **Step 5: Detect welcomes and rebind the per-iteration character**

The current loop computes `let (ts, message) = …;` then a `date_str`, then indexes, then `classify_line`. Insert welcome handling **after `message` is available and after the `date_str` block, but before the log-line indexing and `classify_line`**, then rebind `char_id`/`char_name` from `active` (skipping the line entirely when undetermined). Concretely, after the `let date_str = …;` block (ends ~line 423) insert:

```rust
            // Welcome lines switch the active character (and `Welcome to Clan Lord` will
            // also be counted as a login in Task 2). Fall through afterward so the existing
            // WelcomeLogin event still records start_date under the now-active character.
            if let Some(caps) = patterns::WELCOME_LOGIN.captures(message) {
                let name = titlecase_name(&caps[1]);
                let id = self.db.get_or_create_character(&name)?;
                self.load_override_config(id)?;
                active = Some((id, name));
            } else if let Some(caps) = patterns::WELCOME_BACK.captures(message) {
                let name = titlecase_name(&caps[1]);
                let id = self.db.get_or_create_character(&name)?;
                self.load_override_config(id)?;
                active = Some((id, name));
            }

            // Everything below this point attributes to the active character. If none is
            // known yet (a loose file before its first welcome), skip the line entirely.
            let (char_id, char_name): (i64, &str) = match &active {
                Some((id, name)) => {
                    file_result.attributed = true;
                    (*id, name.as_str())
                }
                None => continue,
            };
```

This shadows the old `char_id`/`char_name` parameters with per-iteration locals, so the entire existing event-handling body (every `char_id` / `char_name` use from line ~425 onward) now targets the active character — **no other edits to the event handlers are needed**.

- [ ] **Step 6: Update the three call sites to the new signature**

Each caller currently passes `char_id, &char_name` (or `char_name`). Change to pass `Some((char_id, char_name.clone()))` as `initial_char`, keeping the same `index_lines` and `count_login` arguments.

`scan_folder_inner` (~line 248):
```rust
            match self.scan_bytes(&bytes[offset..], Some((char_id, char_name.clone())), &path_str, true, count_login) {
```

`scan_folder_with_progress_inner` (~line 1183):
```rust
            match self.scan_bytes(&bytes[offset..], Some((char_id, char_name.clone())), &path_str, index_lines, count_login) {
```

`scan_files_with_progress_inner` (~line 1298):
```rust
        match self.scan_bytes(&bytes[offset..], Some((char_id, char_name.clone())), &path_str, index_lines, count_login) {
```

(Here `char_name` is the `&String`/`&str` already in scope at each call; `.clone()` produces the owned `String` for the tuple.)

- [ ] **Step 7: Run the new test + full suite**

Run: `cargo test -p amanuensis-core`
Expected: PASS — the new attribution test passes and all existing tests stay green (login counts unchanged because `count_login` still drives the single per-file login on the initial character).

- [ ] **Step 8: Commit**

```bash
git add crates/amanuensis-core/src/parser/mod.rs
git commit -m "feat(core): scan_bytes attributes events to a mutable active character (per-welcome switch)"
```

---

## Task 2: Per-Welcome login counting

Replace per-file login counting with one login per `Welcome to Clan Lord, X!` occurrence (credited to X), plus a no-Welcome fallback (1 login to the initial character on a full scan).

**Files:**
- Modify: `crates/amanuensis-core/src/parser/mod.rs` — `scan_bytes` (welcome block from Task 1, the login block ~869-871, signature) and the three callers (replace `count_login` with `is_full_scan = offset == 0`). Update existing login-assertion tests.

**Interfaces:**
- Produces: final `scan_bytes` signature
  ```rust
  fn scan_bytes(
      &self,
      bytes: &[u8],
      initial_char: Option<(i64, String)>,
      file_path: &str,
      index_lines: bool,
      is_full_scan: bool,
  ) -> Result<FileResult>
  ```
  (`count_login` renamed/repurposed to `is_full_scan`, meaning offset == 0.)

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn logins_counted_per_welcome_to_clan_lord() {
    // Two full logins as the same character in one file => 2 logins (per-welcome, not per-file).
    // A "Welcome back" reconnect adds no login.
    let (tmp, char_dir) = create_test_log_dir();
    let body = "\
1/1/24 1:00:00p Welcome to Clan Lord, Ruuk!
1/1/24 1:30:00p Welcome back, Ruuk!
1/1/24 2:00:00p Welcome to Clan Lord, Ruuk!
";
    fs::write(char_dir.join("CL Log 2024-01-01 13.00.00.txt"), body).unwrap();

    let db = Database::open_in_memory().unwrap();
    let parser = LogParser::new(db).unwrap();
    parser.scan_folder(tmp.path(), false).unwrap();

    let ruuk = parser.db().get_character("Ruuk").unwrap().unwrap();
    assert_eq!(ruuk.logins, 2, "two 'Welcome to Clan Lord' => 2 logins; 'Welcome back' adds none");
}

#[test]
fn no_welcome_file_counts_one_login_for_folder_character() {
    // A subfolder file with no welcome at all still counts 1 login for the folder character.
    let (tmp, char_dir) = create_test_log_dir();
    fs::write(
        char_dir.join("CL Log 2024-01-01 13.00.00.txt"),
        "1/1/24 1:01:00p You slaughtered a Rat.\n",
    )
    .unwrap();

    let db = Database::open_in_memory().unwrap();
    let parser = LogParser::new(db).unwrap();
    parser.scan_folder(tmp.path(), false).unwrap();

    let ch = parser.db().get_character("Testchar").unwrap().unwrap();
    assert_eq!(ch.logins, 1, "no-welcome file => 1 fallback login for the folder character");
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p amanuensis-core logins_counted_per_welcome`
Expected: FAIL — today logins == 1 for the two-welcome file (per-file counting).

- [ ] **Step 3: Count a login on each `WELCOME_LOGIN`**

In the welcome block added in Task 1, add the login increment to the `WELCOME_LOGIN` arm and track that one was seen. Replace that block with:

```rust
            if let Some(caps) = patterns::WELCOME_LOGIN.captures(message) {
                let name = titlecase_name(&caps[1]);
                let id = self.db.get_or_create_character(&name)?;
                self.load_override_config(id)?;
                self.db.increment_character_field(id, "logins", 1)?;
                saw_welcome_login = true;
                active = Some((id, name));
            } else if let Some(caps) = patterns::WELCOME_BACK.captures(message) {
                let name = titlecase_name(&caps[1]);
                let id = self.db.get_or_create_character(&name)?;
                self.load_override_config(id)?;
                active = Some((id, name));
            }
```

And declare the tracker next to `active` (Task 1, Step 4):

```rust
        let mut active: Option<(i64, String)> = initial_char.clone();
        let mut saw_welcome_login = false;
```

- [ ] **Step 4: Replace the per-file login block with the no-Welcome fallback**

Remove the existing block (lines ~869-871):

```rust
        if count_login {
            self.db.increment_character_field(char_id, "logins", 1)?;
        }
```

Replace it with (note: outside the loop, `char_id` from the loop is no longer in scope — use the captured initial character):

```rust
        // No `Welcome to Clan Lord` anywhere in a full scan: credit one fallback login to the
        // initial (folder-fallback) character, preserving the mid-session-start behavior.
        // Tail scans (is_full_scan == false) never apply this — prefix welcomes aren't re-seen.
        if is_full_scan && !saw_welcome_login {
            if let Some((id, _)) = &initial_char {
                self.db.increment_character_field(*id, "logins", 1)?;
            }
        }
```

- [ ] **Step 5: Rename the parameter to `is_full_scan` and update callers**

Change the signature's last parameter `count_login: bool` → `is_full_scan: bool`. At each call site the value passed is the same boolean `count_login` already destructured from `ScanPlan::Scan { … count_login }` (it is `true` for offset 0, `false` for appends), so the meaning matches `is_full_scan`. Rename the local binding for clarity at each call site from `count_login` to `is_full_scan`:

At the three `ScanPlan::Scan { bytes, offset, full_hash, count_login }` destructures, rename the field binding:
```rust
                    ScanPlan::Scan { bytes, offset, full_hash, count_login: is_full_scan } => {
                        (bytes, offset, full_hash, is_full_scan)
                    }
```
and pass `is_full_scan` into `scan_bytes` in place of `count_login`.

- [ ] **Step 6: Update existing login-assertion tests that assumed per-file counting**

Search the test module for login assertions that now change. The append test `incremental_scan_picks_up_appended_kills` asserts `char.logins == 1` after a second scan of a grown single-welcome file — this stays **1** (one `Welcome to Clan Lord`, tail scan adds none), so it is unaffected. Run the suite (next step) and fix any test whose file contains multiple `Welcome to Clan Lord` lines or relies on a no-welcome file counting 1 — update the expected count to the per-welcome value. (Do not weaken assertions; set them to the correct new number.)

- [ ] **Step 7: Run the new tests + full suite**

Run: `cargo test -p amanuensis-core`
Expected: PASS — new login tests pass; any updated existing tests pass; no regressions.

- [ ] **Step 8: Commit**

```bash
git add crates/amanuensis-core/src/parser/mod.rs
git commit -m "feat(core): count logins per 'Welcome to Clan Lord' message (+ no-welcome fallback)"
```

---

## Task 3: Loose-file enumeration + skip-and-log undetermined (remove "Unknown")

Scan `CL Log` files directly in a log root (loose files), attributed by content (`initial_char = None`); skip-and-log fully-undetermined files; remove the `"Unknown"` fallback.

**Files:**
- Modify: `crates/amanuensis-core/src/parser/mod.rs` — `scan_folder_inner` (~157-293), `scan_folder_with_progress_inner` (~1103-1205), `scan_files_with_progress_inner` (~1244-1319, remove `"Unknown"`). Add tests.

**Interfaces:**
- Consumes: `find_log_files(&Path) -> Result<Vec<PathBuf>>`, `self.db.add_process_log(level: &str, msg: &str) -> Result<()>`, `FileResult.attributed` (Task 1).
- Produces: a shared helper
  ```rust
  // Scan one loose file (no folder character). Returns true if it was scanned (attributed),
  // false if skipped as undetermined (already logged + counted in `result.skipped`).
  fn scan_loose_file(&self, log_path: &Path, force: bool, index_lines: bool, result: &mut ScanResult) -> Result<bool>
  ```

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn loose_file_in_log_root_is_scanned_and_attributed_by_content() {
    // A CL Log file directly in the log root (not in a character subfolder) must be scanned
    // and attributed to the character named in its welcome.
    let (tmp, char_dir) = create_test_log_dir(); // tmp/TestChar (so tmp is a log root)
    fs::write(char_dir.join("CL Log 2024-01-02 10.00.00.txt"),
        "1/2/24 1:00:00p Welcome to Clan Lord, TestChar!\n").unwrap();
    fs::write(tmp.path().join("CL Log 2024-01-01 09.00.00.txt"),
        "1/1/24 1:00:00p Welcome to Clan Lord, Wanderer!\n1/1/24 1:01:00p You slaughtered a Rat.\n").unwrap();

    let db = Database::open_in_memory().unwrap();
    let parser = LogParser::new(db).unwrap();
    parser.scan_folder(tmp.path(), false).unwrap();

    let w = parser.db().get_character("Wanderer").unwrap().expect("loose file's character scanned");
    assert_eq!(w.logins, 1);
    assert_eq!(parser.db().get_kills(w.id.unwrap()).unwrap().iter().map(|k| k.slaughtered_count).sum::<i64>(), 1);
}

#[test]
fn loose_file_without_welcome_is_skipped_and_logged_not_unknown() {
    let (tmp, _char_dir) = create_test_log_dir();
    fs::write(tmp.path().join("CL Log 2020-02-07 23.28.54.txt"),
        "2/7/20 1:01:00p You slaughtered a Rat.\n").unwrap();

    let db = Database::open_in_memory().unwrap();
    let parser = LogParser::new(db).unwrap();
    let result = parser.scan_folder(tmp.path(), false).unwrap();

    assert!(parser.db().get_character("Unknown").unwrap().is_none(), "no 'Unknown' character is created");
    assert!(result.skipped >= 1, "the undetermined loose file is counted as skipped");
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p amanuensis-core loose_file`
Expected: FAIL — loose files aren't enumerated, so `Wanderer` doesn't exist and the undetermined file isn't counted/skipped.

- [ ] **Step 3: Add the `scan_loose_file` helper**

Add near `scan_folder_inner`:

```rust
    /// Scan a single loose log file (one sitting directly in a log root, with no character
    /// folder). Attributed purely by content (active character starts as None). Returns
    /// Ok(true) if scanned, Ok(false) if skipped as undetermined (logged + counted).
    fn scan_loose_file(
        &self,
        log_path: &Path,
        force: bool,
        index_lines: bool,
        result: &mut ScanResult,
    ) -> Result<bool> {
        let path_str = log_path.to_string_lossy().to_string();
        let (bytes, offset, full_hash, is_full_scan) = match self.plan_file_scan(log_path, &path_str, force)? {
            ScanPlan::Skip | ScanPlan::SkipDuplicate | ScanPlan::SkipChanged => {
                result.skipped += 1;
                return Ok(false);
            }
            ScanPlan::ReadError(e) => {
                log::warn!("Error reading {}: {}", path_str, e);
                result.errors += 1;
                return Ok(false);
            }
            ScanPlan::Scan { bytes, offset, full_hash, count_login } => (bytes, offset, full_hash, count_login),
        };

        let file_result = self.scan_bytes(&bytes[offset..], None, &path_str, index_lines, is_full_scan)?;
        if !file_result.attributed {
            // No determinable character anywhere in the file — skip and log; do NOT mark
            // scanned, and never create an "Unknown" character.
            let _ = self.db.add_process_log("warn", &format!("skipped: could not determine character ({path_str})"));
            result.skipped += 1;
            return Ok(false);
        }

        result.files_scanned += 1;
        result.lines_parsed += file_result.lines_parsed;
        result.events_found += file_result.events_found;
        let now = Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
        self.db.mark_log_scanned(0, &path_str, &full_hash, bytes.len() as i64, &now)?;
        Ok(true)
    }
```

Note: `mark_log_scanned` records the file as scanned so it is not reprocessed; `char_id` 0 is a non-character placeholder used only for the `log_files` bookkeeping row (the actual events were attributed to real characters inside `scan_bytes`). Confirm `mark_log_scanned`'s `character_id` column tolerates 0 (it is an unenforced FK in `log_files`); if a real character row is required, instead pass the id of the first character the file attributed to by having `scan_bytes` also return the first active `char_id`. (Implementer: verify against `db/schema.rs` `log_files`; pick whichever the schema allows and note it in the report.)

- [ ] **Step 4: Enumerate loose files in both folder-walk inners**

In `scan_folder_inner`, after the `for entry in entries { … }` character-subfolder loop closes (just before `Ok(())`), add:

```rust
        // Also scan loose CL Log files sitting directly in this log root.
        for log_path in find_log_files(folder)? {
            self.scan_loose_file(&log_path, force, true, result)?;
        }
```

In `scan_folder_with_progress_inner`, after the character loop closes (before `Ok(())`), add the same, using `index_lines`:

```rust
        for log_path in find_log_files(folder)? {
            current_file += 1;
            let filename = log_path.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default();
            progress(current_file, total_files, &filename);
            self.scan_loose_file(&log_path, force, index_lines, result)?;
        }
```

And include loose files in `total_files` for that inner: after the subfolder pre-collection loop computes `total_files`, add `total_files += find_log_files(folder)?.len();`.

- [ ] **Step 5: Remove the `"Unknown"` fallback in `scan_files_with_progress_inner`**

Replace the char-name fallback block (~1284-1290) so an undetermined explicit-pick file is skipped+logged instead of attributed to `"Unknown"`:

```rust
        // Determine character from content; fall back to the parent directory name (an
        // explicit pick is usually inside a character folder). If neither yields a name,
        // skip and log — never invent an "Unknown" character.
        let char_name = extract_character_name(&bytes).or_else(|| {
            log_path.parent().and_then(|p| p.file_name()).map(|n| titlecase_name(&n.to_string_lossy()))
        });
        let char_name = match char_name {
            Some(n) => n,
            None => {
                let _ = self.db.add_process_log("warn", &format!("skipped: could not determine character ({path_str})"));
                result.skipped += 1;
                continue;
            }
        };
```

(The rest of `scan_files_with_progress_inner` — `get_or_create_character`, `seen_characters`, the `scan_bytes` call with `Some((char_id, char_name.clone()))` and `is_full_scan` — is unchanged from Task 1/2.)

- [ ] **Step 6: Run the new tests + full suite**

Run: `cargo test -p amanuensis-core`
Expected: PASS — loose attributed file scanned; undetermined loose file skipped+logged with no `"Unknown"`; no regressions.

- [ ] **Step 7: Commit**

```bash
git add crates/amanuensis-core/src/parser/mod.rs
git commit -m "feat(core): scan loose files in log roots; skip+log undetermined (no 'Unknown')"
```

---

## Task 4: Pending-count lockstep — loose files + determinability

Extend the pending enumeration to include loose files and exclude undetermined ones, so the Update Logs badge matches the new scanner.

**Files:**
- Modify: `crates/amanuensis-core/src/parser/mod.rs` — `char_log_files` (~1737), `would_scan` (~1669). Add tests.

**Interfaces:**
- Consumes: `find_log_files`, `extract_character_name`, `hash_bytes`, `db.is_hash_scanned`, `db.get_log_scan_state`.
- Produces: `char_log_files` returns subfolder files **and** loose files of a log root; `would_scan` gains a `loose: bool` parameter and returns `false` for a loose new-path file with no determinable character. New signature:
  ```rust
  fn would_scan(db: &crate::db::Database, log_path: &Path, path_str: &str, loose: bool) -> Result<bool>
  ```
  `pending_files` tracks which enumerated files are loose and passes the flag.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn pending_counts_attributable_loose_file_but_not_undetermined() {
    use super::pending_files;
    let (tmp, char_dir) = create_test_log_dir();
    fs::write(char_dir.join("CL Log 2024-01-02 10.00.00.txt"),
        "1/2/24 1:00:00p Welcome to Clan Lord, TestChar!\n").unwrap();
    let good = tmp.path().join("CL Log 2024-01-03 11.00.00.txt"); // loose, has welcome
    fs::write(&good, "1/3/24 1:00:00p Welcome to Clan Lord, Wanderer!\n").unwrap();
    let bad = tmp.path().join("CL Log 2020-02-07 23.28.54.txt"); // loose, no welcome
    fs::write(&bad, "2/7/20 1:01:00p You slaughtered a Rat.\n").unwrap();

    let db = Database::open_in_memory().unwrap();
    let pend = pending_files(&db, &vec![(tmp.path().to_path_buf(), true)]).unwrap();

    assert!(pend.iter().any(|p| *p == good), "attributable loose file is pending");
    assert!(!pend.iter().any(|p| *p == bad), "undetermined loose file is NOT pending");
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p amanuensis-core pending_counts_attributable_loose`
Expected: FAIL — `char_log_files` doesn't include loose files, so the good loose file isn't found (and/or the undetermined one would be counted once loose files are added without the determinability gate).

- [ ] **Step 3: Add loose files to `char_log_files` and thread the `loose` flag**

Change `char_log_files` to return `(PathBuf, bool)` pairs (`bool` = loose), or keep it returning subfolder paths and have `source_log_files` append loose files separately. Use the latter (less churn): add loose files in `source_log_files`:

```rust
fn source_log_files(root: &Path, recursive: bool) -> Vec<(PathBuf, bool)> {
    let log_roots: Vec<PathBuf> = if recursive {
        let discovered = discover_log_folders(root);
        if discovered.is_empty() { vec![root.to_path_buf()] } else { discovered }
    } else {
        vec![root.to_path_buf()]
    };
    let mut out = Vec::new();
    for log_root in &log_roots {
        for f in char_log_files(log_root) {
            out.push((f, false)); // subfolder file
        }
        for f in find_log_files(log_root).unwrap_or_default() {
            out.push((f, true)); // loose file directly in the log root
        }
    }
    out
}
```

Update `pending_files` to consume the flag:

```rust
pub fn pending_files(db: &crate::db::Database, sources: &[(PathBuf, bool)]) -> Result<Vec<PathBuf>> {
    let mut pending = Vec::new();
    for (root, recursive) in sources {
        for (file, loose) in source_log_files(root, *recursive) {
            let path_str = file.to_string_lossy();
            if would_scan(db, &file, &path_str, loose)? {
                pending.push(file);
            }
        }
    }
    Ok(pending)
}
```

- [ ] **Step 4: Add the determinability gate to `would_scan`**

Add the `loose` parameter and, for a new-path loose file that passes the dedup check, require a determinable character. In the `None =>` arm:

```rust
fn would_scan(db: &crate::db::Database, log_path: &Path, path_str: &str, loose: bool) -> Result<bool> {
    let prior = db.get_log_scan_state(path_str)?;
    if let Some((prev_len, _)) = &prior {
        if *prev_len > 0 {
            if let Ok(meta) = std::fs::metadata(log_path) {
                if meta.len() == *prev_len as u64 { return Ok(false); }
            }
        } else {
            return Ok(false);
        }
    }
    let bytes = match std::fs::read(log_path) { Ok(b) => b, Err(_) => return Ok(false) };
    match prior {
        None => {
            let full_hash = hash_bytes(&bytes);
            if db.is_hash_scanned(&full_hash)? { return Ok(false); }
            // Loose files are scanned only if a character can be determined from content
            // (subfolder files always have the folder fallback, so they're always scannable).
            if loose && extract_character_name(&bytes).is_none() { return Ok(false); }
            Ok(true)
        }
        Some((prev_len, prev_hash)) => {
            let prev_len = prev_len as usize;
            let cur_len = bytes.len();
            if cur_len == prev_len { Ok(false) }
            else if cur_len > prev_len && hash_bytes(&bytes[..prev_len]) == prev_hash { Ok(true) }
            else { Ok(false) }
        }
    }
}
```

Update the existing `pending_files_*` tests (Task 1 of the Update Logs work) that call `pending_files` indirectly — they don't call `would_scan` directly, so only `source_log_files`'s return type changed; confirm those tests still compile/pass (they exercise subfolder files → `loose=false`, behavior unchanged).

- [ ] **Step 5: Run the new test + full suite**

Run: `cargo test -p amanuensis-core`
Expected: PASS — attributable loose file pending, undetermined loose file not pending; existing pending tests unchanged.

- [ ] **Step 6: Commit**

```bash
git add crates/amanuensis-core/src/parser/mod.rs
git commit -m "feat(core): pending count includes attributable loose files, excludes undetermined ones"
```

---

## Task 5: Tail-scan active-character re-derivation from the prefix

When a grown file is tail-scanned, seed the active character from the last welcome in the already-loaded prefix, so appended events attribute correctly.

**Files:**
- Modify: `crates/amanuensis-core/src/parser/mod.rs` — the three callers' `scan_bytes` invocations, plus a small helper. Add a test.

**Interfaces:**
- Consumes: `decode_log_bytes`, `parse_timestamp`, `patterns::WELCOME_LOGIN/WELCOME_BACK`, `titlecase_name`, `self.db.get_or_create_character`.
- Produces:
  ```rust
  // The character active at `offset` (last welcome in bytes[..offset]); None if none.
  fn active_char_at_offset(&self, bytes: &[u8], offset: usize) -> Result<Option<(i64, String)>>
  ```

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn tail_scan_attributes_appended_events_to_prefix_character() {
    // A growing daily file: first scan establishes the character; the appended tail (no new
    // welcome) must still attribute to that character, not be dropped as undetermined.
    let (tmp, char_dir) = create_test_log_dir();
    let log_path = char_dir.join("CL Log 2024-01-01 13.00.00.txt");
    let initial = "1/1/24 1:00:00p Welcome to Clan Lord, Ruuk!\n1/1/24 1:01:00p You slaughtered a Rat.\n";
    fs::write(&log_path, initial).unwrap();

    let db = Database::open_in_memory().unwrap();
    let parser = LogParser::new(db).unwrap();
    // Loose file (directly in the log root) so the folder fallback can't mask the bug.
    let loose = tmp.path().join("CL Log 2024-01-01 13.00.00.txt");
    fs::write(&loose, initial).unwrap();
    parser.scan_folder(tmp.path(), false).unwrap();

    // Append a kill with NO new welcome.
    fs::write(&loose, format!("{initial}1/1/24 2:01:00p You vanquished a Large Vermine.\n")).unwrap();
    parser.scan_folder(tmp.path(), false).unwrap();

    let ruuk = parser.db().get_character("Ruuk").unwrap().unwrap();
    assert!(
        parser.db().get_kills(ruuk.id.unwrap()).unwrap().iter().any(|k| k.creature_name == "Large Vermine"),
        "appended Large Vermine must attribute to Ruuk via prefix re-derivation"
    );
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p amanuensis-core tail_scan_attributes_appended`
Expected: FAIL — the loose tail scan starts with `initial_char = None` and no welcome in the tail, so the appended kill is skipped as undetermined.

- [ ] **Step 3: Add the `active_char_at_offset` helper**

```rust
    /// The character active at byte `offset` of `bytes` — the last `Welcome …` before it.
    /// Used to seed a tail (append) scan so events after the offset attribute correctly.
    fn active_char_at_offset(&self, bytes: &[u8], offset: usize) -> Result<Option<(i64, String)>> {
        if offset == 0 { return Ok(None); }
        let prefix = decode_log_bytes(&bytes[..offset.min(bytes.len())]);
        let mut name: Option<String> = None;
        for line in prefix.lines() {
            let message = match parse_timestamp(line) { Some((_dt, msg)) => msg, None => line };
            if let Some(caps) = patterns::WELCOME_LOGIN.captures(message) {
                name = Some(titlecase_name(&caps[1]));
            } else if let Some(caps) = patterns::WELCOME_BACK.captures(message) {
                name = Some(titlecase_name(&caps[1]));
            }
        }
        match name {
            Some(n) => {
                let id = self.db.get_or_create_character(&n)?;
                Ok(Some((id, n)))
            }
            None => Ok(None),
        }
    }
```

- [ ] **Step 4: Seed the initial character from the prefix on tail scans**

At each of the three call sites, when `offset > 0` use the prefix-derived character as the initial; otherwise keep the existing initial (folder character, or `None`/parent for loose/explicit). For the folder-walk inners (subfolder files), the initial is the folder `(char_id, char_name)` for full scans, and the prefix-derived character for tail scans (falling back to the folder character if the prefix had no welcome):

```rust
            let initial = if offset > 0 {
                self.active_char_at_offset(&bytes, offset)?
                    .or_else(|| Some((char_id, char_name.clone())))
            } else {
                Some((char_id, char_name.clone()))
            };
            match self.scan_bytes(&bytes[offset..], initial, &path_str, index_lines, is_full_scan) {
```

In `scan_loose_file` (Task 3), replace the `None` initial with the prefix derivation:

```rust
        let initial = self.active_char_at_offset(&bytes, offset)?;
        let file_result = self.scan_bytes(&bytes[offset..], initial, &path_str, index_lines, is_full_scan)?;
```

- [ ] **Step 5: Run the new test + full suite**

Run: `cargo test -p amanuensis-core`
Expected: PASS — appended events attribute to the prefix character; no regressions.

- [ ] **Step 6: Commit**

```bash
git add crates/amanuensis-core/src/parser/mod.rs
git commit -m "feat(core): tail-scan re-derives active character from the prefix"
```

---

## Task 6: Update real-data login expectations + docs

Reconcile the per-Welcome login change with the real-data comparison tests and document the new model.

**Files:**
- Modify: `crates/amanuensis-core/tests/real_data_comparison.rs` (login assertions). Modify: `CLAUDE.md` (Log Format Details + Key Functional Areas).

**Interfaces:** none (test-data + docs).

- [ ] **Step 1: Identify the affected real-data login assertions**

Run (these are `#[ignore]` and need local logs — run only if available):
`cargo test -p amanuensis-core --test real_data_comparison -- --ignored 2>&1 | grep -i login`
Expected: login-count assertions that previously matched Scribius now differ (logins are per-Welcome, typically ≥ the per-file count). Note each failing expectation.

- [ ] **Step 2: Update the login expectations**

For each affected assertion, change the expected value from the Scribius per-file count to the new per-Welcome count (the actual scanned value), and add a comment: `// per-Welcome logins (diverges from Scribius's per-file count; see multi-character spec)`. Do NOT delete the assertions or replace them with `>=` — pin the exact new number so regressions are still caught. If local logs are unavailable, mark this step as requiring the maintainer's data and leave a clear `// FIXME(maintainer): set per-Welcome login count from a local --ignored run` next to each, and record this limitation in the task report.

- [ ] **Step 3: Document the new model in CLAUDE.md**

Under "### Login counting" (Log Format Details), replace the "exactly 1 login" guidance with:

```markdown
- Logins are counted **per `Welcome to Clan Lord, X!` message**, credited to X (this diverges
  from Scribius's one-login-per-file). `Welcome back` (reconnect) is not a login. A scanned
  file with no welcome at all still counts 1 login for its folder-fallback character. Existing
  databases need a full Rescan to recompute.
```

Add to "## Key Functional Areas" (extend the Character management item, #2):

```markdown
   The scanner attributes events to a **mutable active character** that switches on each
   `Welcome to Clan Lord, X!` / `Welcome back, X!` line, so a single file containing multiple
   characters' sessions credits each correctly (no longer only the first). Loose `CL Log`
   files sitting directly in a log root (not in a character subfolder) are also scanned and
   attributed by content; a file (or a pre-first-welcome span) with no determinable character
   is **skipped and logged** (`skipped: could not determine character`) rather than attributed
   to an `"Unknown"` character. `pending_files`/`would_scan` mirror this (attributable loose
   files count toward the Update Logs badge; undetermined ones do not).
```

- [ ] **Step 4: Run the standard suite + commit**

Run: `cargo test -p amanuensis-core`
Expected: PASS (standard, non-ignored suite).

```bash
git add crates/amanuensis-core/tests/real_data_comparison.rs CLAUDE.md
git commit -m "docs+test: per-Welcome login expectations and multi-character scanning notes"
```

---

## Self-Review Notes

- **Spec coverage:** §1 mutable active character → Tasks 1 (+5 tail-scan); §2 login counting → Task 2; §3 skip-and-log undetermined / no "Unknown" → Task 3; §4 loose-file enumeration + pending lockstep → Tasks 3 & 4; §5 offset-resume → Task 5; §6 testing → tests in every task; §7 rollout/docs → Task 6.
- **Placeholder scan:** the only deferred decision is the `mark_log_scanned` placeholder `char_id` for undetermined-but-bookkept loose files (Task 3 Step 3) — flagged with an explicit verification instruction against `db/schema.rs`, not a silent TODO. Task 6 Step 2 has a maintainer-data FIXME for the `--ignored` real-data numbers, with explicit handling.
- **Type/name consistency:** `scan_bytes(bytes, initial_char: Option<(i64,String)>, file_path, index_lines, is_full_scan)`, `FileResult.attributed`, `scan_loose_file`, `would_scan(db, path, path_str, loose)`, `source_log_files -> Vec<(PathBuf, bool)>`, `active_char_at_offset` — used consistently across tasks.
- **Risk note for the executor:** Task 1 is the highest-risk change (it rewires attribution for every event handler via the per-iteration `char_id`/`char_name` rebinding). Verify after Task 1 that all existing single-character tests stay green before proceeding — that is the safety check that the rebinding preserved behavior.
