# questlog

[![ci](https://github.com/ProportanilityConstant/questlog/actions/workflows/ci.yml/badge.svg)](https://github.com/ProportanilityConstant/questlog/actions/workflows/ci.yml) ![license](https://img.shields.io/badge/license-MIT-blue) ![rust](https://img.shields.io/badge/rust-std--only-orange)

Audit a git repo the way a reviewer skims it, then see the result as a pixel-RPG character sheet with a fix list ranked by score gain.

![example card](assets/example-card.png)

Hiring managers decide in seconds whether a repo is worth opening. questlog checks the signals they actually look at (README quality, tests, CI, license, commit history, leaked secrets, releases) and tells you what to fix first.

## Install

- **Windows:** download `questlog-x64.msi` from the latest GitHub Release. It installs to Program Files, adds questlog to PATH (open a new terminal), and adds a Start menu entry. Needs git on PATH and Windows Terminal or any console with ANSI support (Windows 10+).
- **Any OS:** `cargo install --path .`

## Usage

```
cargo install --path .
questlog                      # audit the current repo
questlog ../other --svg card.svg   # also write a shareable SVG card for your README
questlog . --min-score 70     # exit 1 below 70, usable as a CI gate
```

## What it checks

28 checks in 6 categories (DOCS, TESTS, HYGIENE, HISTORY, SECURITY, POLISH), each weighted so a category totals 100. It reads only files tracked by git (what a visitor sees), runs offline, and has zero dependencies.

## How scoring works

Each check is worth points inside its category (for example, a README is 30 of DOCS, CI-run tests are 40 of TESTS). Category scores are summed to 0-100, the overall score is their average, and the rank comes from fixed thresholds: Wanderer, Apprentice, Journeyman, Adept, Master, Legend (90+). The quest list shows every failed check, largest gain first, so the top line is always the single best use of your next hour. The pixel avatar changes armor, cape, and helmet colors per rank, and the SVG card is meant to be pasted into a profile README.

## Limits

- Heuristics, not truth. A high score means the repo looks maintained; it does not judge code quality.
- Secret detection covers private-key blocks, AWS key IDs and GitHub tokens only. Use a dedicated scanner for real audits.
- Commit-history checks need a real history; they cannot be gamed by backdating without it being obvious.
