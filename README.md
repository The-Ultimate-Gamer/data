# The Ultimate Gamer — Data

The authoritative results database for **The Ultimate Gamer** LAN tournament, plus the scripts that publish it as JSON for the website ([the-ultimate-gamer.github.io](https://the-ultimate-gamer.github.io/), source in [The-Ultimate-Gamer/the-ultimate-gamer.github.io](https://github.com/The-Ultimate-Gamer/the-ultimate-gamer.github.io)).

- `database/ultimate-gamer.sqlite` is the **single source of truth**.
- `data/` contains **generated** JSON. Never edit it by hand; regenerate it with `scripts/export_json.py`.
- GitHub Pages publishes `data/` at **https://the-ultimate-gamer.github.io/data/** (for example `…/data/leaderboard.json`).
- The website fetches these files in the browser at runtime, so publishing new data never requires a website rebuild.

## Layout

```
database/ultimate-gamer.sqlite   authoritative database (committed)
schema/schema.sql                table definitions
scripts/ugdata.py                shared rules + JSON builders (used by all scripts)
scripts/init_db.py               create an empty database from the schema
scripts/seed.py                  load reference data (categories, awards); --demo for fictional data
scripts/export_json.py           generate data/ from the database
scripts/validate.py              check the database and that data/ is up to date
data/                            generated JSON (committed, published)
tests/                           unit tests (standard library only)
```

Requirements: Python 3.10 or newer. No third-party packages are needed.

## Updating the data

```sh
# 1. edit the database (see "Recording a tournament")
# 2. regenerate and check
python3 scripts/export_json.py
python3 scripts/validate.py
# 3. publish
git add database data && git commit -m "Results: <what changed>" && git push
```

On every push to `main`, `.github/workflows/publish.yml` runs the tests and `validate.py`. Validation fails if the JSON is out of date with the database. If everything passes, the workflow deploys `data/` to GitHub Pages.

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
- Nominations
- Award recipients
- Records

`export_json.py` **derives** everything else:

| Derived value | Rule |
| --- | --- |
| Points per game | `(base points + 1 if the assigned Nemesis finished behind) × 2 if Double Down` |
| Challenger | Running total reaches the tournament's threshold. `challengerSinceGame` is the round in which that happened |
| Ultimate Gamer | The first player who **was already a Challenger before a round** and **wins** it. The tournament ends there |
| Ranks | Ranked by points, with ties sharing a rank (1, 1, 3). Ties are never broken automatically |
| Tokens left | Starting tokens minus Double Downs used and Nominate tokens spent |
| Statistics | Game wins, podiums, category wins, Nemesis record, rivalries, token usage, last played and similar |

**Award recipients** are not computed. Several awards need tie-breaking or judgement calls that the rules don't settle, so the organisers record them in `award_grants`.

### Validation rules (`scripts/validate.py`)

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

**Warnings:**
- A Nemesis who has no result in that round.
- A tournament marked `completed` without a champion.

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
- **Award recipients** go in `award_grants`.
- **Tournament records** go in `tournament_records`.
- **When a Challenger wins a round**, set the tournament's `status = 'completed'` and its `end_date`. Validation reminds you if you forget.

## Demo data and local development

`scripts/seed.py --demo` builds a **fictional** dataset in a separate database. The script refuses to write it into the real one.

```sh
python3 scripts/seed.py --demo --db demo/demo.sqlite
python3 scripts/export_json.py --db demo/demo.sqlite --out demo/data
```

`demo/` is gitignored. To run the website against the demo data, use `DEV_DATA_DIR=../ultimate-gamer-data/demo/data npm run dev:local` in the website repository. Leave out `DEV_DATA_DIR` to use this repository's real `data/`.

## Tests

```sh
python3 -m unittest discover -s tests -t tests -v
```

## JSON contract

Every file has the same envelope:

```json
{ "schemaVersion": 1, "generatedAt": "2026-10-17T08:00:00Z", "data": { } }
```

`generatedAt` only changes when that document's data changes. The files are `site.json`, `leaderboard.json`, `categories.json`, `awards.json`, `games/index.json`, `players/index.json`, `players/{slug}.json`, `tournaments/index.json` and `tournaments/{slug}.json`.

The full field reference lives in the website repository: `docs/data-contract.md` and `src/lib/data/types.ts`. Increase `SCHEMA_VERSION` in `scripts/ugdata.py` only for breaking changes, and update the website first.
