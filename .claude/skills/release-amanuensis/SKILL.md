---
name: release-amanuensis
description: Cut and publish a new Amanuensis release — bump the version, tag it, let CI build all platforms, and promote the result to a full GitHub release. Use this whenever the user wants to ship, cut, publish, or tag a release / new version of Amanuensis (e.g. "cut a release", "ship 0.6.0", "tag a new version", "release this"). Bakes in the four places the version lives, the prerelease-promotion step CI does NOT do for you, and the known transient build flake so you don't mistake it for a real failure.
---

# Releasing Amanuensis

Amanuensis ships as a Tauri desktop app (plus a CLI). A release is driven entirely by **pushing a `v*` git tag**: `.github/workflows/release.yml` then builds every platform via `tauri-apps/tauri-action` and creates the GitHub release. Two things bite people, so they're called out below: the version lives in **four** files, and CI publishes a **prerelease** that you must **promote by hand**.

## Preconditions

- `gh` is authenticated (`gh auth status`) and `origin` points at `github.com:thesquib/Amanuensis`.
- You're on `main`, the working tree is clean, and the work to ship is already merged.
- `cargo test -p amanuensis-core` passes. A release tag builds whatever is on the tagged commit — don't tag a red `main`.

## Step 1 — Choose the version

Current version is whatever the latest tag is (`git tag --sort=-creatordate | head -1`). Pick the next one by semver intent:
- **patch** (0.5.16 → 0.5.17) for fixes / small changes — matches the project's usual rapid cadence.
- **minor** (0.5.16 → 0.6.0) when the release contains a notable new user-facing feature.

When unsure which the user wants, ask — it's a permanent, public artifact.

## Step 2 — Bump the version in all four files

The version string appears in **four** places and they must match, or the built artifacts are inconsistent:

- `crates/amanuensis-cli/Cargo.toml`   → `version = "X"`
- `crates/amanuensis-core/Cargo.toml`  → `version = "X"`
- `crates/amanuensis-gui/Cargo.toml`   → `version = "X"`
- `crates/amanuensis-gui/tauri.conf.json` → `"version": "X"`

Then refresh the lockfile so the three `amanuensis-*` entries in `Cargo.lock` update:

```bash
cargo check -p amanuensis-cli -p amanuensis-core -p amanuensis-gui
```

Verify with `grep -rn '"X"' crates/*/Cargo.toml crates/amanuensis-gui/tauri.conf.json` before committing — a missed file is the most common mistake.

## Step 3 — Commit and push `main`

Match the project's commit convention exactly:

```bash
git add crates/*/Cargo.toml crates/amanuensis-gui/tauri.conf.json Cargo.lock
git commit -m "Bump version to X"
git push origin main
```

## Step 4 — Tag and push (this triggers the build)

```bash
git tag -a vX -m "Amanuensis vX — <one-line summary>"
git push origin vX
```

Pushing the `vX` tag is what starts `release.yml`. It builds a matrix: **macOS Apple Silicon (aarch64), macOS Intel (x86_64, 10.13+), Linux (x86_64), Windows (x86_64)**, then `tauri-action` creates the release and uploads the DMGs, `.deb`, `.AppImage`, `.rpm`, `.exe`, `.msi`, `.app.tar.gz`, and the portable `.zip`.

## Step 5 — Watch the build

```bash
gh run watch "$(gh run list --workflow=release.yml -L1 --json databaseId -q '.[0].databaseId')" --exit-status
```

**Known flake:** the **macOS Intel** leg has intermittently failed on `actions/checkout` with `Could not resolve host: github.com` — a runner DNS blip, not a code problem. If only that leg fails, re-run it (`gh run rerun <run-id> --failed`) rather than assuming the release is broken. The other legs and the release itself still publish.

## Step 6 — Promote the prerelease to a full release (do NOT skip)

`release.yml` sets `prerelease: true`, so CI always publishes a **prerelease**. Every shipped Amanuensis release is a *full* release, so you must promote it — CI will not do this for you, and it's easy to think you're done when you're not:

```bash
gh release edit vX --prerelease=false --latest
```

`--latest` is needed too: promoting out of prerelease does not automatically move the "Latest" badge.

## Step 7 — (Optional) Add a change note

The default release body is just a platform/file table. For a release with a real headline change, prepend a short note — **the big thing only**, user-facing, not an exhaustive changelog:

```bash
BODY=$(gh release view vX --json body -q .body)
printf '### What'"'"'s new\n\n<one or two sentences on the headline change>\n\n---\n\n%s\n' "$BODY" \
  | gh release edit vX --notes-file -
```

## Verify

```bash
gh release list -L 3   # vX should show as "Latest"
gh release view vX --json isPrerelease,assets -q '"prerelease=\(.isPrerelease) assets=\(.assets | length)"'
```

`prerelease=false` and the full set of platform assets present means the release is cut and live at `https://github.com/thesquib/Amanuensis/releases/tag/vX`.

## Why you can't "test" this skill by running it

Executing this procedure pushes tags and publishes public GitHub releases — there's no dry-run. Treat each step as real. If you need to rehearse, stop after Step 2 (local-only) and review the diff before pushing anything.
