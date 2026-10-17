# Ultimate Gamer Admin

A **local-only** admin app for the tournament database. Use it to add players, games and tournaments before a LAN, record every game during it, and publish the results to the website.

- Runs on your laptop at **http://127.0.0.1:5174**. It only accepts connections from your own machine and has no login.
- Reads and writes `../database/ultimate-gamer.sqlite` directly, using Node's built-in `node:sqlite`.
- After every change it runs `scripts/export_json.py` and `scripts/validate.py`. The standings, Challengers and champion you see are therefore exactly what the website will show; the tournament rules live only in the Python scripts.
- **Publish** commits `database/` and `data/` and pushes them to GitHub, which deploys the JSON to the live site.

## Requirements

- Node.js ≥ 22.17. Run `nvm use` to get Node 24, which has `node:sqlite` built in.
- Python 3.10+ available as `python3`, for the export and validation scripts.
- `git` with push access to the data repository. Only needed for publishing.

## Run it

```sh
cd admin
nvm use
npm install        # first time only
npm start          # opens http://127.0.0.1:5174 on the real database
```

To practise in a **separate practice database** (publishing is disabled there):

```sh
cd ..   # data repo root
python3 scripts/seed.py --demo --db demo/demo.sqlite
cd admin && npm run demo
```

To show friends what the website will look like, use the **Demo data** page in the normal app instead. It adds fictional data to the real database, flagged so that **Remove demo data** takes it out again exactly. Publish after adding and after removing.

| Env variable | Default | Purpose |
| --- | --- | --- |
| `UG_DB` | `database/ultimate-gamer.sqlite` | Database to edit. Publishing is disabled for any other database |
| `UG_OUT` | `data` | Where the JSON is exported |
| `UG_PYTHON` | `python3` | Python executable |
| `UG_REPO` | `..` | Data repository root |

## Workflow

The **Dashboard** has a *Getting started* checklist that always highlights the next step. In short:

1. **Before the LAN:**
   - **Players** → add everyone.
   - **Games** → build the library and tag each game with its categories.
   - **Tournaments** → create the tournament with its threshold, tokens and points per placement.
   - On the tournament page, follow **Set up this tournament**: tick the players (each box saves instantly), add categories in play order, then **▶ Start tournament**.
2. **Every game:** press **▶ Enter game N** on the dashboard.
   1. Record nominations (winner's pick, votes, Nominate tokens), then **🎲 Draw at random**, or pick the game in **Game being played** if you drew in person.
   2. **Tap players in finishing order.** Placements and base points fill in from the points table, and you can edit both for ties.
   3. Tick **Double Down** and choose each player's **Nemesis**. The `= Points` column previews the total.
   4. Press **✓ Finish game**. The app shows the winner, any new Challengers, or the new Ultimate Gamer.
3. **Publish** whenever the website should update. A gold bar on every page reminds you when there are unpublished changes. The Publish page shows what will change, sends it with one button, and tracks the website update (that last part needs the GitHub CLI `gh`).
4. **The end:** when a Challenger wins, mark the tournament completed. Then grant the awards and add any fun records on the tournament page.

Web addresses (slugs) of players, games and tournaments can be edited. The website updates its own links, but links that people have already shared will stop working.

Mistakes are easy to fix:
- **Reopen to edit** works on the most recent finished round.
- Categories can be reordered or removed until their round has started.

## Development

```sh
npm run dev      # dev server (localhost only)
npm run check    # svelte-check
```

Code map:
- `src/lib/server/`: config, `db.ts` (node:sqlite helpers), `repo.ts` (every query), `scripts.ts` (runs the Python export and validation), `git.ts` (publish).
- `src/routes/`: pages and form actions. The pages use `use:enhance`, so they also work without client-side JavaScript.
- `src/lib/styles/tokens.css`: copied from the website so the admin shares its look. Keep the two in sync by hand.
- The admin stores **facts only**. Never compute points, Challengers or champions here. The `= Points` column in round entry is a labelled preview; the authoritative values come from the exported JSON.
