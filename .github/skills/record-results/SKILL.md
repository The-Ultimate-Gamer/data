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
| New player | `players (slug, name, discord_id)`; the Discord user ID is required. Then run `ug-admin discord --player <slug>` to copy their Discord name and picture. An uploaded `avatars/<slug>.jpg` or an `avatar_url` overrides the Discord picture |
| New game in the library | `games (slug, name, description?, min_players?, max_players?, available)` + `game_categories (game_id, category_slug)` |
| New tournament | `tournaments (slug, name, edition?, start_date, end_date?, location?, status, challenger_threshold, double_down_tokens=1, nominate_tokens=1)` + `tournament_points (tournament_id, placement, points)` for prefill |
| Register participants | `tournament_players (tournament_id, player_id)` |
| Announce category order | `tournament_categories (tournament_id, position, category_slug)` (add rows as categories are revealed) |
| Next game drawn | `rounds (tournament_id, number, game_id, completed=0)` (`number` = category position) |
| Nominations for that draw | `nominations (tournament_id, round_number, game_id, player_id?, source 'winner'/'vote'/'token', entries)` (the drawn `game_id` must be among them) |
| Nemeses announced | `nemesis_assignments (tournament_id, round_number, player_id, nemesis_player_id)` for the next round. Public on the website; shared and one-way Nemeses are allowed |
| Game finished | one `results` row per player: `placement`, `base_points`, `double_down` (0/1), `nemesis_player_id` (the Nemesis assigned before the game). Then `UPDATE rounds SET completed = 1, played_at = '<ISO UTC>'` |
| Tournament decided | `ug-admin validate` reports when a Challenger has won. Set `status = 'completed'` and `end_date` |

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
