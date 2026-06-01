#!/usr/bin/env python3
"""Sync bestiary creature sprites from bestiary.clanlord.net.

For every entry in crates/amanuensis-core/data/bestiary.json that has a
`static_pic`, download
    https://bestiary.clanlord.net/images/creatures_static/<family>/<static_pic>
into crates/amanuensis-gui/ui/public/bestiary/<static_pic>, writing the file
only when it is missing or its bytes differ (so the git diff is limited to
actually-added / actually-changed sprites).

Usage:
    python3 scripts/sync-bestiary-sprites.py [--dry-run]

Re-run after `amanuensis update-bestiary` to pick up new or changed icons.
"""
from __future__ import annotations

import argparse
import json
import sys
import urllib.parse
import urllib.request
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
BESTIARY_JSON = REPO / "crates/amanuensis-core/data/bestiary.json"
SPRITE_DIR = REPO / "crates/amanuensis-gui/ui/public/bestiary"
BASE_URL = "https://bestiary.clanlord.net/images/creatures_static"


def sprite_url(family: str, static_pic: str) -> str:
    fam = urllib.parse.quote(family)
    pic = urllib.parse.quote(static_pic)
    return f"{BASE_URL}/{fam}/{pic}"


def fetch(url: str) -> bytes | None:
    req = urllib.request.Request(url, headers={"User-Agent": "amanuensis-sprite-sync"})
    try:
        with urllib.request.urlopen(req, timeout=30) as resp:
            if resp.status != 200:
                return None
            return resp.read()
    except Exception:
        return None


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--dry-run", action="store_true", help="report changes without writing")
    args = ap.parse_args()

    data = json.loads(BESTIARY_JSON.read_text())
    entries = data["entries"]
    SPRITE_DIR.mkdir(parents=True, exist_ok=True)

    # Deduplicate by (family, static_pic) -> the local filename is static_pic.
    jobs: dict[str, tuple[str, str]] = {}
    for e in entries:
        sp = e.get("static_pic")
        fam = e.get("family")
        if sp and fam:
            jobs[sp] = (fam, sp)

    # Extinct creatures' sprites live under their original family folder, not an
    # "Extinct" one. Fall back to searching every known family folder.
    all_families = sorted({fam for fam, _ in jobs.values()})

    added: list[str] = []
    updated: list[str] = []
    failed: list[str] = []

    def fetch_sprite(family: str, static_pic: str) -> bytes | None:
        body = fetch(sprite_url(family, static_pic))
        if body is not None:
            return body
        # Folder for this family did not have it; try the others.
        for other in all_families:
            if other == family:
                continue
            body = fetch(sprite_url(other, static_pic))
            if body is not None:
                return body
        return None

    def process(item: tuple[str, str]) -> None:
        family, static_pic = item
        dest = SPRITE_DIR / static_pic
        body = fetch_sprite(family, static_pic)
        if body is None:
            # Only flag as failed if we don't already have the file locally.
            if not dest.exists():
                failed.append(f"{static_pic} (family={family})")
            return
        if dest.exists():
            if dest.read_bytes() == body:
                return
            if not args.dry_run:
                dest.write_bytes(body)
            updated.append(static_pic)
        else:
            if not args.dry_run:
                dest.write_bytes(body)
            added.append(static_pic)

    with ThreadPoolExecutor(max_workers=8) as pool:
        pool.map(process, jobs.values())

    print(f"Synced {len(jobs)} referenced sprites.")
    print(f"  added:   {len(added)}")
    for n in sorted(added):
        print(f"    + {n}")
    print(f"  updated: {len(updated)}")
    for n in sorted(updated):
        print(f"    ~ {n}")
    print(f"  failed:  {len(failed)}")
    for n in sorted(failed):
        print(f"    ! {n}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
