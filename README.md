# The Ultimate Gamer — Data

The authoritative results database for **The Ultimate Gamer** LAN tournament, plus the scripts that publish it as JSON for the website ([the-ultimate-gamer.github.io](https://the-ultimate-gamer.github.io/), source in [The-Ultimate-Gamer/the-ultimate-gamer.github.io](https://github.com/The-Ultimate-Gamer/the-ultimate-gamer.github.io)).

- `database/ultimate-gamer.sqlite` is the **single source of truth**.
- `data/` contains **generated** JSON. Never edit it by hand; regenerate it with `ug-admin export`.
- GitHub Pages publishes `data/` at **https://the-ultimate-gamer.github.io/data/** (for example `…/data/leaderboard.json`).
- The website fetches these files in the browser at runtime, so publishing new data never requires a website rebuild.

> Working with an AI coding agent? See [AGENTS.md](AGENTS.md). The skill [.github/skills/record-results](.github/skills/record-results/SKILL.md) guides an agent through recording results.

## Quick start

**Organisers** don't need this repository at all: download the admin app from [Releases](https://github.com/The-Ultimate-Gamer/data/releases), sign in to GitHub once, and it downloads and publishes the data for you. See [admin/README.md](admin/README.md).

**Developers:**

```sh
git clone git@github.com:The-Ultimate-Gamer/data.git ultimate-gamer-data
cd ultimate-gamer-data
cargo test --manifest-path admin/Cargo.toml                                  # tests
cargo run --manifest-path admin/Cargo.toml --no-default-features -- validate # DB + JSON consistency
```

Clone the website next to this repository as `ultimate-gamer-website`. Then run `npm run dev:local` there to preview this repository's `data/` locally.

## Layout

```
database/ultimate-gamer.sqlite   authoritative database (committed)
schema/schema.sql                table definitions (compiled into the admin tool)
avatars/                         player pictures, published with the data
admin/                           ug-admin: desktop app, rules, export, validation, publishing (Rust)
data/                            generated JSON (committed, published)
```

Requirements: the `ug-admin` binary, or Rust 1.88+ to build it.

## Updating the data

**Easiest: the admin app.** It's a desktop app for adding players, games and tournaments, recording each game during the LAN, and publishing with one click. It works without a clone of this repository, git or Python. See [admin/README.md](admin/README.md) for the full workflow.

By hand, in a checkout of this repository:

```sh
# 1. edit the database (see "Recording a tournament")
# 2. regenerate and check
ug-admin export
ug-admin validate
# 3. publish
git add database data avatars && git commit -m "Results: <what changed>" && git push
```

On every push to `main`, `.github/workflows/publish.yml` runs the tests and `ug-admin validate`. Validation fails if the JSON is out of date with the database. If everything passes, the workflow deploys `data/` to GitHub Pages.

Visitors see the update after the Pages deployment, which takes a minute or two. The CDN may also keep serving cached copies for up to about 10 minutes.

**One-time setup:** go to **Settings → Pages → Source** and choose **GitHub Actions**.

## What is stored and what is derived

The database stores **facts only**:
- Players, games and categories
- Tournaments with their Challenger threshold and token counts
- Participants
- Category order
- Rounds (which game was played, and whether it is finished)
- Each player's placement, base points, Double Down use and assigned Nemesis
- Nemesis pairs announced for the next game (Nemeses are public)
- Nominations
- Award recipients
- Records

`ug-admin export` **derives** everything else (the rules live in `admin/src/rules.rs`):

| Derived value | Rule |
| --- | --- |
| Points per game | `(base points + 1 if the assigned Nemesis finished behind) × 2 if Double Down` |
| Challenger | Running total reaches the tournament's threshold. `challengerSinceGame` is the round in which that happened |
| Ultimate Gamer | The first player who **was already a Challenger before a round** and **wins** it. The tournament ends there |
| Ranks | Ranked by points, with ties sharing a rank (1, 1, 3). Ties are never broken automatically |
| Final placement | Once a tournament is over, the champion finishes 1st and everyone else follows by points. "Best finish" uses this |
| Tokens left | Starting tokens minus Double Downs used and Nominate tokens spent |
| Statistics | Game wins, podiums, category wins, Nemesis record, rivalries, token usage, last played and similar |
| Awards | See `compute_awards`: some appear as they happen, the rest when the tournament ends |

**Awards** are computed from the results by `compute_awards` in `admin/src/rules.rs`. Everyone tied for an award gets it, so nothing is entered by hand.

### Validation rules (`ug-admin validate`, `admin/src/validate.rs`)

**Errors:**
- An invalid slug.
- A completed round without exactly one winner, with fewer than two results, or without a game.
- Results from a player who isn't registered for the tournament.
- Gaps in round numbering, or more than one unfinished round.
- More tokens used than the tournament allows.
- A selected game that isn't among the recorded nominations.
- A round recorded after a Challenger has already won.
- A decided tournament that isn't marked `completed`.
- A tournament award without a tournament, or given to a non-participant.
- Base points higher than any placement in the tournament's points table (catches typos such as `123231`).
- A Nemesis pair that involves a player who isn't registered.

**Warnings:**
- A Nemesis who has no result in that round.
- A tournament marked `completed` without a champion.
- Base points that differ from the points table for that placement.
- A result whose Nemesis differs from the pair announced before the game.
- A signed Discord avatar link, which expires. Save the picture in `avatars/` instead.
- A file in `avatars/` that isn't a supported picture or doesn't match a player slug.

## Share links

The export also writes small HTML pages to `data/share/players/<slug>/` and `data/share/tournaments/<slug>/`, published at `https://the-ultimate-gamer.github.io/data/share/…`. They carry the title, a short summary and the player's picture for link previews in Discord, WhatsApp and similar apps, then redirect to the real page on the website. The website's Share buttons copy these links.

Website pages such as `/players/<slug>` are rendered in the browser and served through GitHub Pages' 404 fallback, so apps can't build a preview for them. The share pages exist to fix that.

The public addresses come from the `meta` table: `site_url` (default `https://the-ultimate-gamer.github.io/`) and `data_url` (default `<site_url>data/`).

## Player pictures

Save a player's picture as `avatars/<player-slug>.jpg` (or `.png`, `.webp`, `.gif`). The export copies it to `data/avatars/`, so it is published with the data and never expires, and it wins over the player's `avatar_url`. The JSON points to it with a path relative to the data URL, for example `"avatarUrl": "avatars/tnoy.jpg"`.

## Recording a tournament

Use any SQLite tool, for example the `sqlite3` CLI or DB Browser for SQLite. Run `PRAGMA foreign_keys = ON;` first. Slugs are lowercase kebab-case and become URLs, so don't change them once they're published.

```sql
-- players and tournament
INSERT INTO players (slug, name) VALUES ('alex', 'Alex'), ('chris', 'Chris');
INSERT INTO tournaments (slug, name, start_date, location, status, challenger_threshold)
VALUES ('lan-2026', 'LAN 2026', '2026-11-14', 'Somewhere', 'ongoing', 30);
INSERT INTO tournament_players SELECT t.id, p.id FROM tournaments t, players p
WHERE t.slug = 'lan-2026' AND p.slug IN ('alex', 'chris');

-- category order (add rows as categories are announced)
INSERT INTO tournament_categories VALUES ((SELECT id FROM tournaments WHERE slug = 'lan-2026'), 1, 'strategy');

-- game library
INSERT INTO games (slug, name) VALUES ('age-of-empires-2', 'Age of Empires II');
INSERT INTO game_categories VALUES ((SELECT id FROM games WHERE slug = 'age-of-empires-2'), 'strategy');

-- round 1: game drawn but not played yet (completed = 0)
INSERT INTO rounds (tournament_id, number, game_id, completed)
VALUES ((SELECT id FROM tournaments WHERE slug = 'lan-2026'), 1, (SELECT id FROM games WHERE slug = 'age-of-empires-2'), 0);

-- results, then mark the round completed
INSERT INTO results (tournament_id, round_number, player_id, placement, base_points, double_down, nemesis_player_id)
VALUES (1, 1, (SELECT id FROM players WHERE slug = 'alex'), 1, 6, 0, (SELECT id FROM players WHERE slug = 'chris')),
       (1, 1, (SELECT id FROM players WHERE slug = 'chris'), 2, 4, 0, (SELECT id FROM players WHERE slug = 'alex'));
UPDATE rounds SET completed = 1, played_at = '2026-11-14T15:00:00Z' WHERE tournament_id = 1 AND number = 1;
```

- **Nominations** go in `nominations`, using `source` = `winner`, `vote` or `token`. Each nomination is linked to the round whose game it competes for.
- **Awards** are computed; there's nothing to enter.
- **When a Challenger wins a round**, set the tournament's `status = 'completed'` and its `end_date` (the admin app does this for you). Validation reminds you if you forget.

## Demo data and local development

To **show the website off before the first real tournament**, use **Settings → Demo data on the website** in the admin app, then publish. Or run:

```sh
ug-admin seed --add-demo      # 2 demo tournaments (and games) played by your players, all flagged is_demo
ug-admin export && git add database data && git commit -m "Add demo data" && git push
```

- **What visitors see:** a "fictional demo data" banner, with demo tournaments marked "Demo".
- **Removing it:** `ug-admin seed --remove-demo` (or Settings → Remove demo data) removes exactly the flagged rows. Your real players, games and tournaments are never touched. Re-export and push afterwards.
- **Clashes:** adding the demo is refused if a real player or tournament already uses one of the demo slugs. Games with the same slug are shared, not duplicated.

For development, `ug-admin seed --demo` builds a separate **demo-only** database instead. The admin app's **practice mode** uses one too.

```sh
ug-admin seed --demo --repo demo   # demo/database/ultimate-gamer.sqlite
ug-admin export --repo demo        # demo/data
```

`demo/` is gitignored. To run the website against the demo data, use `DEV_DATA_DIR=../ultimate-gamer-data/demo/data npm run dev:local` in the website repository. Leave out `DEV_DATA_DIR` to use this repository's real `data/`.

## Tests

```sh
cargo test --manifest-path admin/Cargo.toml
```

## JSON contract

Every file has the same envelope:

```json
{ "schemaVersion": 1, "generatedAt": "2026-10-17T08:00:00Z", "data": { } }
```

`generatedAt` only changes when that document's data changes. The files are `site.json`, `leaderboard.json`, `categories.json`, `awards.json`, `games/index.json`, `players/index.json`, `players/{slug}.json`, `tournaments/index.json` and `tournaments/{slug}.json`.

The full field reference lives in the website repository: `docs/data-contract.md` and `src/lib/data/types.ts`. Increase `SCHEMA_VERSION` in `admin/src/db.rs` only for breaking changes, and update the website first.
