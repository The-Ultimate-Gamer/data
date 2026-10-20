---
name: record-results
description: 'Record or edit tournament data in the SQLite database and republish the JSON. Use when adding players, games, a tournament, category order, a played round/results, Double Down or Nemesis info, nominations/game draws, or finishing a tournament.'
---

# Record tournament data

Every change follows the same loop: **edit SQLite → export → validate → (preview) → commit**. Never edit `data/*.json` by hand.

If the user just wants to enter data themselves, point them to the admin app (`ug-admin`, see `admin/README.md`). It does this whole loop through forms and publishes with one button. The steps below are for when you edit the database directly.

## 0. Open the database

```sh
sqlite3 database/ultimate-gamer.sqlite
```

```sql
PRAGMA foreign_keys = ON;
```

Look up ids by slug instead of guessing them, for example `(SELECT id FROM players WHERE slug = 'alex')`. Slugs are lowercase kebab-case and become permanent URLs.

## 1. Typical operations

| Goal | Tables |
| --- | --- |
| New player | Prefer the admin app (Players → Discord user ID → + Add player). By hand: `players (slug, name, discord_id, nickname?)`, then `ug-admin discord` to copy their Discord username (which becomes the name and slug unless `nickname` is set) and picture. An uploaded `avatars/<slug>.jpg` or an `avatar_url` overrides the Discord picture |
| New game in the library | `games (slug, name, description?, min_players?, max_players?, available, url?)` + `game_categories (game_id, category_slug)`. `url` is an https link to get or play it |
| New tournament | Prefer the admin app, which makes the name and slug. By hand: `tournaments (slug, name, custom_name?, start_date, end_date?, location?, challenger_threshold, double_down_tokens=1, nominate_tokens=1)` + `tournament_tags (tournament_id, tag)` + `tournament_points (tournament_id, placement, points)` for the places that earn points |
| Register participants | `tournament_players (tournament_id, player_id)` |
| Category order | `tournament_categories (tournament_id, position, category_slug)`, set before the first game; games loop through it until someone wins |
| Next game drawn | `rounds (tournament_id, number, game_id, completed=0)`; `number` is the next game number, and its category is position `((number - 1) mod count) + 1` |
| Nominations for that draw | `nominations (tournament_id, round_number, game_id, player_id?, source 'winner'/'vote'/'token', entries)` (the drawn `game_id` must be among them) |
| Nemeses announced | `nemesis_assignments (tournament_id, round_number, player_id, nemesis_player_id)` for the next round. Public on the website; shared and one-way Nemeses are allowed |
| Game finished | one `results` row per player: `placement`, `base_points`, `double_down` (0/1), `nemesis_player_id` (the Nemesis assigned before the game). Then `UPDATE rounds SET completed = 1, played_at = '<ISO UTC>'` |
| Tournament decided | Nothing to do: a Challenger's win completes it. Optionally set `end_date` |

**Do not store derived values.** Points, Challenger and champion status, ranks, tokens left, awards and statistics are all computed by the admin tool (`admin/src/rules.rs` and `admin/src/export.rs`).

**Ask the user whenever a fact is unknown.** This includes placements, base points, who used Double Down and Nemesis assignments. Do not guess them.

## 2. Export and validate

```sh
cargo run --manifest-path admin/Cargo.toml --no-default-features -- export
cargo run --manifest-path admin/Cargo.toml --no-default-features -- validate
```

(With an installed binary: `ug-admin export` and `ug-admin validate`, run from the repository root.)

Fix every `error:` line. Read every `warning:` and confirm with the user whether it is expected.

## 3. Preview (optional)

Run `npm run dev:local` in `../ultimate-gamer-website` and open http://localhost:5173/.

## 4. Commit and publish

```sh
git add database data avatars
git commit -m "Results: <tournament> round <n>"
git push   # only when the user asks; this publishes to the live site
```

GitHub Actions validates the data and deploys it. Visitors see the update within a few minutes, because of the CDN cache.
