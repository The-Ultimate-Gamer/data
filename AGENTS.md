# AGENTS.md — The Ultimate Gamer data

Guidance for developers and AI coding agents working in this repository. The README explains the workflow and has SQL examples. The website repository's `docs/data-contract.md` is the JSON reference.

## What this is

- `database/ultimate-gamer.sqlite` is the **authoritative** record of every tournament.
- The scripts turn it into public JSON in `data/`. GitHub Pages publishes that JSON at `https://the-ultimate-gamer.github.io/data/`.
- The website ([The-Ultimate-Gamer/the-ultimate-gamer.github.io](https://github.com/The-Ultimate-Gamer/the-ultimate-gamer.github.io), usually checked out at `../ultimate-gamer-website`) fetches that JSON in the browser at runtime.

## Rules (do not break)

1. **SQLite is the single source of truth. Never edit `data/` by hand.** Always regenerate it with `scripts/export_json.py`. CI fails if `data/` is out of date.
2. **The database stores facts; the scripts derive everything else.**
   - Facts: participants, placements, base points, Double Down use, Nemesis assignments, nominations and award grants.
   - Derived: points, Challenger and champion status, ranks, tokens left and statistics.
   - Don't add columns for derived values.
3. **All tournament rules live in `scripts/ugdata.py`** (`compute_tournament`). The website must never re-implement them.
4. **Never invent results or tie-breaks.**
   - Ties in points share a rank.
   - Award recipients are entered by the organisers in `award_grants`, because the data repo has no authority to decide ambiguous awards.
5. **Never put demo or fictional data in the real database.** `seed.py --demo` refuses the default path. Keep it that way. Demo output goes in the gitignored `demo/`.
6. **Slugs are permanent URLs.** They must be lowercase kebab-case. Don't rename a published slug unless the user asks.
7. **Use only the standard library.** Python ≥ 3.10, with no third-party packages, so the scripts run anywhere and in CI.

## Game rules as implemented

- **Points per game:** `(base_points + 1 if the Nemesis finished behind the player) × 2 if Double Down was used`.
- **Challenger:** the running total reaches `challenger_threshold`. `challengerSinceGame` is the round where that happened.
- **Champion (Ultimate Gamer):** the first round winner who was **already** a Challenger **before** that round. The tournament ends there, and later rounds are a validation error.
- **Every completed round** has exactly one placement-1 winner and at least two results.
- **Tokens:** each player starts with `double_down_tokens` and `nominate_tokens` (default 1 each). Nominate usage is counted from `nominations` rows where `source = 'token'`.
- **Round numbers** map to `tournament_categories.position`, which gives the round's category. A round with `completed = 0` is the next game, whose game may or may not have been drawn yet.

## Layout

```
schema/schema.sql            tables + constraints (raw facts)
scripts/ugdata.py            connect(), compute_tournament(), Exporter (all JSON builders), write_documents()
scripts/init_db.py           create the DB from the schema
scripts/seed.py              reference data (categories, awards) + --demo fictional dataset
scripts/export_json.py       DB -> data/ (keeps generatedAt for unchanged files, removes stale files)
scripts/validate.py          DB consistency checks + "data/ matches a fresh export"
scripts/migrate.py           add new tables/indexes from schema.sql to an existing DB
tests/                       unittest suites (helpers.py builds in-memory DBs)
admin/                       local SvelteKit admin app (node:sqlite); see admin/README.md
.github/workflows/publish.yml  tests + validate, then deploy data/ to Pages
```

## Admin app rules (`admin/`)

- **Local only:** bound to 127.0.0.1 and with no auth. Never add a hosted deployment or remote access.
- **All SQL goes in `src/lib/server/repo.ts`.** Every mutation runs through `mutate()` (`src/lib/server/forms.ts`), which re-exports and validates afterwards.
- **No tournament rules in the admin.** Read derived values (standings, Challengers, champion) from the exported JSON (`readExported`). The only exception is the labelled `= Points` preview in `ResultsEntry.svelte`.
- **Publishing is limited:** it only commits `database/` and `data/`, and only when validation reports no errors and the database is the real one.
- **Shared conventions with the website:** SvelteKit 3 conventions, `#lib/...` imports with file extensions, and design tokens copied from the website.
- **Checks:** run `npm run check` in `admin/` (0 errors), and test changes against a disposable copy of the demo database (`UG_DB=/tmp/x.sqlite UG_OUT=/tmp/x-data npm run dev`).

## Commands

```sh
python3 scripts/export_json.py                      # regenerate data/ from the real DB
python3 scripts/validate.py                         # must report 0 errors before committing
python3 -m unittest discover -s tests -t tests -v   # tests
python3 scripts/seed.py --demo --db demo/demo.sqlite && \
  python3 scripts/export_json.py --db demo/demo.sqlite --out demo/data   # fictional demo export
```

## Editing tournament data

The usual way is the **admin app**: run `cd admin && npm start` and open http://127.0.0.1:5174. See [admin/README.md](admin/README.md). It writes SQLite, re-exports and validates after every change, and can commit and push from its Publish page.

Without the admin app, edit the database with `sqlite3 database/ultimate-gamer.sqlite`, and always run `PRAGMA foreign_keys = ON;` first. The README has SQL examples. The skill at `.github/skills/record-results/SKILL.md` walks through the full workflow step by step.

After every data change:

1. `python3 scripts/export_json.py`
2. `python3 scripts/validate.py` (fix all errors and read the warnings)
3. Optionally preview the change in the website with `npm run dev:local` in `../ultimate-gamer-website`.
4. Commit `database/` and `data/` together. Pushing to `main` publishes the data. Only push when the user asks.

## Changing the schema or JSON contract

1. Edit `schema/schema.sql`. New tables reach existing databases via `python3 scripts/migrate.py`. Column changes need an explicit `ALTER TABLE` step added to `migrate.py`.
2. Update `seed.py` (demo data), the `Exporter` in `ugdata.py`, and the admin's `repo.ts` and pages if they touch the changed tables.
3. Add or adjust tests, and add validation rules in `validate.py` if the new facts can be inconsistent.
4. Re-export, then check that `validate.py` and the tests pass.
5. Update the website: `src/lib/data/types.ts`, `validate.ts` if the field is essential, and `docs/data-contract.md`.
6. **Breaking JSON changes** need `SCHEMA_VERSION` bumped in `ugdata.py`, but only **after** the website supports the new version.

## Before you finish a change

- The tests pass, and `validate.py` reports 0 errors against the real DB.
- `data/` is regenerated, and the diff only touches the documents you expected.
- The demo export still validates (`validate.py --db demo/demo.sqlite --out demo/data`).
