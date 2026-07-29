#!/usr/bin/env python3
"""Validate behavioural player-trainer detection against a real Amanuensis database.

Players who own a training ledger can teach other players, and when they do they greet
with the *exact* same wording as an NPC trainer, so their greetings become checkpoint
series indistinguishable from real trainers. Detection therefore keys on behaviour that
only player characters exhibit (clanning, experience sharing, thinking to you, training
ledgers, teacher offers) rather than on any trainer name list.

This script is the read-only evidence harness used to design that feature. It sweeps the
`log_lines` FTS table of an existing database and reports which checkpoint speakers the
signals flag. It exists so the numbers quoted in
`docs/superpowers/specs/2026-07-29-player-trainers-and-bestiary-links-design.md` stay
reproducible, and so the Rust implementation in
`crates/amanuensis-core/src/parser/player_signals.rs` can be cross-checked against an
independent implementation of the same patterns.

Expected result on the reference corpus (1,160 files / 10 characters / 261k lines):
flags exactly Fenwick, Bramwell Gorse and Tallow; flags none of the 52 legitimate trainers,
including the display-name variants absent from trainers.json (Higgrus, Chronos, Hardia,
Splash O'Sul, Diggin, the four Faures, Tra'Kning, Par Troon, Metta Sylpha,
Respin Verminbane).

Read-only: opens the database with `mode=ro` and writes nothing.

Usage:
    python3 tools/check-player-detection.py [path-to-amanuensis.db]

Defaults to ~/Library/Application Support/com.dfsw.Amanuensis/amanuensis.db
"""

import os
import re
import sqlite3
import sys

DEFAULT_DB = os.path.expanduser(
    "~/Library/Application Support/com.dfsw.Amanuensis/amanuensis.db"
)

# The ten player-only line forms. Keep in sync with
# crates/amanuensis-core/src/parser/player_signals.rs.
# Values are (regex, evidence-class).
PATTERNS = [
    (re.compile(r"^(.+?) is (?:now|no longer) Clanning\.$"), "clanning"),
    (re.compile(r"^(.+?) is sharing experiences with you\.$"), "sharing"),
    (re.compile(r"^You begin sharing your experiences with (.+?)\.$"), "sharing"),
    (re.compile(r'^(.+?) thinks to you, "'), "thinks"),
    (re.compile(r"^(.+?) writes .+? in (?:his|her|their) training ledger\.$"), "ledger"),
    (re.compile(r"^(.+?) shows (?:his|her|their) training ledger to "), "ledger"),
    (re.compile(r"^[¥•]?\s*You begin training with (.+?)\.$"), "ledger"),
    (re.compile(r'^[¥•]\s*(.+?) offers: [“"]I can teach you'), "offer"),
    (
        re.compile(
            r"^[¥•]\s*To accept (?:his|her|their) offer, say: "
            r"I accept (.+?) as my teacher\.$"
        ),
        "offer",
    ),
]

# Handled separately because one line yields several names.
SHARE_LIST = re.compile(r"^You are sharing experiences with (.+?)\.$")

# log_lines.content keeps the raw "MM/DD/YY H:MM:SSa " prefix.
TIMESTAMP = re.compile(r"^\d+/\d+/\d+ \d+:\d+:\d+[ap]?\s*")

RANK = {"clanning": 0, "sharing": 1, "thinks": 2, "offer": 3, "ledger": 4}


def split_name_list(raw):
    """Split a Clan Lord name list -- 'A, B, C and D' -- into individual names."""
    parts = []
    for chunk in raw.split(", "):
        parts.extend(chunk.split(" and "))
    return [p.strip() for p in parts if p.strip()]


def detect(message):
    """Yield (name, evidence) for every player signal in one timestamp-stripped line."""
    for pattern, evidence in PATTERNS:
        match = pattern.match(message)
        if match:
            yield match.group(1).strip(), evidence
            return
    match = SHARE_LIST.match(message)
    if match:
        for name in split_name_list(match.group(1)):
            yield name, "sharing"


def main():
    db_path = sys.argv[1] if len(sys.argv) > 1 else DEFAULT_DB
    if not os.path.exists(db_path):
        sys.exit(f"database not found: {db_path}")

    conn = sqlite3.connect(f"file:{db_path}?mode=ro", uri=True)

    speakers = {
        row[0] for row in conn.execute("SELECT DISTINCT trainer_name FROM trainer_checkpoints")
    }
    if not speakers:
        print("No trainer checkpoints in this database -- nothing to validate.")
        return

    players = {}  # name -> {evidence: count}
    lines = 0
    for (content,) in conn.execute("SELECT content FROM log_lines"):
        lines += 1
        message = TIMESTAMP.sub("", content).strip()
        for name, evidence in detect(message):
            players.setdefault(name, {}).setdefault(evidence, 0)
            players[name][evidence] += 1

    print(f"Scanned {lines:,} indexed lines from {db_path}")
    print(f"Distinct names with a player signal: {len(players):,}")
    print(f"Distinct checkpoint speakers: {len(speakers)}\n")

    flagged = sorted(n for n in speakers if n in players)
    clean = sorted(n for n in speakers if n not in players)

    print(f"--- CHECKPOINT SPEAKERS FLAGGED AS PLAYERS ({len(flagged)}) ---")
    for name in flagged:
        rows = conn.execute(
            "SELECT COUNT(*) FROM trainer_checkpoints WHERE trainer_name = ?", (name,)
        ).fetchone()[0]
        signals = players[name]
        strongest = max(signals, key=lambda e: RANK[e])
        detail = ", ".join(f"{e}x{c}" for e, c in sorted(signals.items()))
        print(f"  {name:24s} rows={rows:5,d}  evidence={strongest:8s}  ({detail})")

    print(f"\n--- CHECKPOINT SPEAKERS NOT FLAGGED ({len(clean)}) ---")
    for name in clean:
        print(f"  {name}")

    if lines == 0:
        print(
            "\nNote: log_lines is empty (indexing disabled), so no players can be "
            "recovered this way."
        )


if __name__ == "__main__":
    main()
