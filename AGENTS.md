# AGENTS.md — The Ultimate Gamer data

Guidance for developers and AI coding agents working in this repository. The README explains the workflow and has SQL examples. The website repository's `docs/data-contract.md` is the JSON reference.

## What this is

- `database/ultimate-gamer.sqlite` is the **authoritative** record of every tournament.
- The admin tool (`admin/`, a single Rust binary called `ug-admin`) turns it into public JSON in `data/`. GitHub Pages publishes that JSON at `https://the-ultimate-gamer.github.io/data/`.
- The website ([The-Ultimate-Gamer/the-ultimate-gamer.github.io](https://github.com/The-Ultimate-Gamer/the-ultimate-gamer.github.io), usually checked out at `../ultimate-gamer-website`) fetches that JSON in the browser at runtime.

## Rules (do not break)

1. **SQLite is the single source of truth. Never edit `data/` by hand.** Always regenerate it with `ug-admin export`. CI fails if `data/` is out of date.
2. **The database stores facts; the admin tool derives everything else.**
   - Facts: participants, placements, base points, Double Down use, Nemesis assignments, nominations and players' Discord accounts.
   - Derived: points, Challenger and champion status, ranks, tokens left, awards and statistics.
   - Don't add columns for derived values.
3. **All tournament rules live in `admin/src/rules.rs`** (`compute_tournament`). Nothing else may re-implement them: not the website, and not the admin's screens. The admin's "If you finish now…" preview runs these rules on an in-memory copy of the database.
4. **Never invent results or tie-breaks.**
   - Ties in points share a rank.
   - Awards are computed by `compute_awards` in `rules.rs` from the results. Everyone tied for an award gets it; nothing is decided by hand.
5. **Fictional data must be flagged.**
   - Demo data goes into the real database only through `ug-admin seed --add-demo` (or the admin's Settings → Demo data). Every demo row has `is_demo = 1`, and `--remove-demo` deletes exactly those rows.
   - While demo data exists, `site.json` exports `demo: true` and the website shows a banner.
   - Never insert unflagged fictional data. `ug-admin seed --demo --repo demo` builds a separate demo-only database in the gitignored `demo/`. The admin's practice mode uses its own demo-only database outside this repository.
6. **Slugs are URLs.** They must be lowercase kebab-case. Renaming is allowed (the admin warns about it), because the website regenerates its links, but previously shared links break. Don't rename one unless the user asks.
7. **One binary, nothing else to install.** Organisers download `ug-admin` and nothing more: no Python, Node.js or git. Keep it that way. Crates are fine, but avoid ones that need system libraries at build or run time.

## Game rules as implemented

- **Points per game:** `(base_points + 1 if the Nemesis finished behind the player) × 2 if Double Down was used`.
- **Challenger:** the running total reaches `challenger_threshold`. `challengerSinceGame` is the round where that happened.
- **Champion (Ultimate Gamer):** the first round winner who was **already** a Challenger **before** that round. The tournament ends there, and later rounds are a validation error.
- **Every completed round** has exactly one placement-1 winner and at least two results.
- **Tokens:** each player starts with `double_down_tokens` and `nominate_tokens` (default 1 each). Nominate usage is counted from `nominations` rows where `source = 'token'`.
- **Round numbers** map to `tournament_categories.position`, which gives the round's category. A round with `completed = 0` is the next game, whose game may or may not have been drawn yet.
- **Nemeses are public.** Pairs for the next game go in `nemesis_assignments` and are exported as `nextNemeses`. Anyone but yourself can be drawn, so Nemeses can be shared and need not be mutual. `results.nemesis_player_id` still records the Nemesis a player actually had.
- **Base points sanity check:** `ug-admin validate` fails when base points exceed the tournament's points table, and warns when they differ from the value for that placement.
- **Final placement (`finish`):** in a finished tournament the champion is 1st and everyone else follows by points. `bestPlacement` uses it, so the crown counts as 1st.
- **Finishing the deciding game completes the tournament** (status and end date), and the first drawn or finished game starts it. Reopening that game puts it back to live.
- **Awards** (`compute_awards`): Ultimate Gamer, Challenger, Believer (won with Double Down), Nominator (a Nominate token's game was drawn) and Survivor (won while last, from game 2) appear live. Dominator, Contender, Podium, Nemesis, Victim, Ghost, Rookie (needs a returning player), Underdog and Fallout (vs. the player's previous tournament) once it's over. Legend is a career award (3+ titles). New awards need a rule there; the admin only edits names, icons and descriptions.
- **Players need a Discord user ID** (except fictional demo players). Their names and picture (`avatars/<slug>.discord.png`) are copied from Discord. Picture priority: uploaded `avatars/<slug>.<ext>` > `avatar_url` > Discord picture > initials.
- **Demo data** is played out by a seeded random generator (`seed.rs`) with the real roster when it has at least 4 players, else with fictional flagged players.

## Layout

```
schema/schema.sql            tables + constraints (raw facts); compiled into ug-admin
avatars/                     player pictures as <player-slug>.jpg/.png/.webp; copied to data/avatars/ on export
data/share/                  generated link-preview pages (players/<slug>/, tournaments/<slug>/); never edit by hand
admin/                       ug-admin (Rust): desktop app + CLI; see admin/README.md for the code map
admin/src/rules.rs           compute_tournament(): THE tournament rules
admin/src/export.rs          Exporter (all JSON builders), write_documents(), share pages, avatars
admin/src/validate.rs        DB consistency checks + "data/ matches a fresh export"
admin/src/db.rs, seed.rs     open/init/migrate; reference data (categories, awards) and flagged demo data
admin/src/github.rs, sync.rs download from / publish to GitHub through its REST API (no git needed)
admin/src/gui/               desktop app (egui); screens return Actions applied in gui/mod.rs
admin/src/tests.rs           end-to-end tests (in-memory databases)
.github/workflows/publish.yml      tests + validate, then deploy data/ to Pages; lints and tests the admin
.github/workflows/admin-release.yml  tag admin-v* -> binaries for Linux, Windows, macOS on a GitHub release
```

## Admin app rules (`admin/`)

- **A desktop app, not a server.** It never listens on the network. It calls the GitHub API to download and publish, and, only when an organiser presses Fetch (or runs `ug-admin discord`), Discord's API to copy a linked player's names and picture. The Discord bot token lives in the per-user settings file, never in the repository.
- **Writes go through `src/repo.rs`.** Reads go through `src/model.rs`. Screens never touch the database directly: they return `Action`s, and `App::apply_db` (`src/gui/mod.rs`) runs them. It takes an undo snapshot, applies the change, re-exports, re-validates and reloads.
- **No tournament rules outside `rules.rs`.** Screens read derived values (standings, Challengers, champion) from the `TournamentState` it computes.
- **Publishing is limited:** it only sends `database/`, `data/` and `avatars/`, as one commit on top of the branch, never a force push. It refuses when validation reports errors, and it is off in practice mode and in `--repo` folder mode. If someone else published data in the meantime, it stops and asks.
- **Design tokens** are copied from the website (`src/gui/theme.rs`). Keep them in sync by hand.
- **Checks:** `cargo test`, `cargo clippy --all-targets -- -D warnings` and `cargo fmt --check` in `admin/`. Try UI changes against a disposable copy (`cargo run -- --repo /tmp/copy`), and use `UG_AUTOSHOT=<dir>` to screenshot every screen.

## Commands

Run these from the repository root (`ug-admin` stands for `cargo run --manifest-path admin/Cargo.toml --no-default-features --`):

```sh
ug-admin export                     # regenerate data/ from the real DB
ug-admin validate                   # must report 0 errors before committing
cargo test --manifest-path admin/Cargo.toml   # tests
ug-admin seed --demo --repo demo && ug-admin export --repo demo   # fictional demo export in demo/
```

## Editing tournament data

The usual way is the **admin app** (`ug-admin`, see [admin/README.md](admin/README.md)). It writes SQLite, re-exports and validates after every change, and publishes from its Publish page. To use it on this checkout instead of its own synced copy, run `cargo run --manifest-path admin/Cargo.toml -- --repo .`. Then commit with git yourself.

Without the admin app, edit the database with `sqlite3 database/ultimate-gamer.sqlite`, and always run `PRAGMA foreign_keys = ON;` first. The README has SQL examples. The skill at `.github/skills/record-results/SKILL.md` walks through the full workflow step by step.

After every data change:

1. `ug-admin export`
2. `ug-admin validate` (fix all errors and read the warnings)
3. Optionally preview the change in the website with `npm run dev:local` in `../ultimate-gamer-website`.
4. Commit `database/`, `avatars/` and `data/` together. Pushing to `main` publishes the data. Only push when the user asks.

## Changing the schema or JSON contract

1. Edit `schema/schema.sql`. New tables reach existing databases automatically: the admin migrates on open, or run `ug-admin migrate`. Column changes need an explicit entry in `ADDED_COLUMNS` (`admin/src/db.rs`).
2. Update `seed.rs` (demo data), the `Exporter` in `export.rs`, `model.rs`/`repo.rs`, and the admin's screens if they touch the changed tables.
3. Add or adjust tests, and add validation rules in `validate.rs` if the new facts can be inconsistent.
4. Re-export, then check that `ug-admin validate` and the tests pass.
5. Update the website: `src/lib/data/types.ts`, `validate.ts` if the field is essential, and `docs/data-contract.md`.
6. **Breaking JSON changes** need `SCHEMA_VERSION` bumped in `admin/src/db.rs`, but only **after** the website supports the new version.

## Before you finish a change

- The tests pass, and `ug-admin validate` reports 0 errors against the real DB.
- `data/` is regenerated, and the diff only touches the documents you expected.
- The demo export still validates (`ug-admin validate --repo demo`).
