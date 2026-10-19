# ug-admin

One program for everything an organiser does with the tournament data:

- **Desktop app:** add players, games and tournaments, record every game during the LAN, and grant awards.
- **Export and checks:** turn the database into the website's JSON, and check the data.
- **Publish:** send the changes to GitHub with one button. The website updates itself a minute or two later.

It is a single file with nothing else to install. You don't need Python, Node.js or git.

## Install

Download the file for your computer from the repository's [Releases](https://github.com/The-Ultimate-Gamer/data/releases) (look for `admin-v…`):

| Computer | File |
| --- | --- |
| Windows | `ug-admin-windows-x86_64.exe` |
| macOS (Apple silicon) | `ug-admin-macos-arm64` |
| Linux | `ug-admin-linux-x86_64` |

Then start it. Each system needs one extra step the first time:

- **macOS:** the file isn't signed. Run `chmod +x ug-admin-macos-arm64`, then right-click it → **Open** → **Open**.
- **Linux:** run `chmod +x ug-admin-linux-x86_64`.
- **Windows:** if SmartScreen warns you, choose **More info** → **Run anyway**.

## First start

The app asks how you want to start:

1. **Connect to GitHub** (normal use). If the app shows **Sign in with GitHub**, press it: GitHub opens in your browser, you enter the code the app shows and approve, and the app downloads the data. (That button appears once the organisation has registered the app with GitHub; see [Sign in with GitHub](#sign-in-with-github-oauth-app) below.) Otherwise, create a fine-grained token at GitHub → Settings → Developer settings → Personal access tokens, with these settings:
   - **Resource owner:** The-Ultimate-Gamer.
   - **Repository access:** only the `data` repository.
   - **Permissions:** Contents: *Read and write*, Actions: *Read-only*.

   Paste the token into the app. It then downloads the data. If you already use the GitHub CLI, choose **Use my `gh` login** instead.
2. **Practise with demo data** opens a separate database full of fictional data. Nothing you do there is ever published.
3. **Start an empty database** is only for the very first setup.

The sign-in is stored only on your computer, in the app's settings file, which only your user can read. Settings → Storage shows where it is.

## Using it

The **Dashboard** always shows the next step.

1. **Before the LAN:**
   - **Players:** type a name and press Enter. Add a picture by dropping an image file on the window.
   - **Games:** build the library and tag each game with its categories.
   - **Tournaments → + New tournament:** set the threshold, tokens and points per placement.
   - On the tournament's **Setup** tab, tick the players and add the categories in play order. Then press **▶ Start tournament**.
2. **Every game:** press **▶ Enter game N** on the dashboard.
   1. Record nominations, then press **🎲 Draw at random**, or pick the game directly.
   2. Announce the Nemeses: draw them, or choose them per player.
   3. **Tap players in finishing order.** Places and base points fill in from the points table. You can edit both for ties.
   4. Tick Double Down where it was used. The **= Points** column and the "If you finish now…" line show the result *before* you save, including new Challengers or a new Ultimate Gamer.
   5. Press **✔ Finish game**.
3. **Publish** whenever the website should update. The top bar tells you when something isn't on the website yet. The Publish page lists what will change and shows the data checks. After you publish, it follows the website update until it's live.
4. **The end:** when a Challenger wins, press **🏁 Mark completed**. Then grant the awards on the **Awards & records** tab, which suggests the clear-cut ones.

Fixing mistakes:

- **↺ Undo** (Ctrl+Z / ⌘Z) reverses the last changes to the database. Picture files aren't undone.
- **Reopen to edit** works on the most recent finished game.
- Categories can be reordered or removed until their game has started.
- Web addresses (slugs) can be changed. The website follows, but links people already shared stop working, and the app warns you about this.

### Several organisers

Each computer has its own copy of the data. If someone else published after you last downloaded, Publish stops and offers a choice:

- **Download theirs (discard mine)**
- **Publish mine anyway (replace theirs)**

The database can't be merged automatically. Agree on who enters results during the LAN, and use **⬇ Download latest** on the Publish page before you start.

## Command line

The same program also works without the window. Run these from a checkout of the data repository:

```sh
ug-admin export                    # regenerate data/ from database/
ug-admin validate                  # check the database, and that data/ is up to date (exit 1 on errors)
ug-admin init                      # new, empty database with reference data
ug-admin migrate                   # bring an older database up to date with schema/schema.sql
ug-admin seed --add-demo           # add flagged demo data (--remove-demo takes it out again)
ug-admin seed --demo --repo demo   # a separate demo-only database in demo/
ug-admin --repo .                  # the desktop app on this folder (publish with git yourself)
```

The commands below work on the app's own synced data, or on `--workspace <folder>`:

```sh
ug-admin download                  # latest data from GitHub
ug-admin status                    # what publishing would change
ug-admin publish -m "Results day 1"
```

They use the app's GitHub sign-in, or the `UG_GITHUB_TOKEN` environment variable.

## Development

Requirements: Rust 1.88 or newer.

```sh
cargo run                          # the desktop app (on its own synced data)
cargo run -- --repo ..             # the desktop app on this repository
cargo test                         # all tests
cargo clippy --all-targets && cargo fmt --check
cargo run --no-default-features -- validate --repo ..   # without the desktop window (as CI does)
```

Builds made with `--no-default-features` have no desktop window, which keeps CI fast.

### Sign in with GitHub (OAuth App)

**Sign in with GitHub** uses GitHub's [device flow](https://docs.github.com/en/apps/oauth-apps/building-oauth-apps/authorizing-oauth-apps#device-flow), so nobody has to create or paste a token. It uses the organisation's OAuth App **Ultimate Gamer Admin** (client id `Ov23liMIivjp8MnAcxwd`, in `OAUTH_CLIENT_ID` in `src/github.rs`), which has device flow on and non-expiring tokens. Pasting a token or **Use my `gh` login** still work as alternatives.

To set up a replacement app (an organisation owner):

1. GitHub → **The-Ultimate-Gamer** → Settings → Developer settings → **OAuth Apps** → **New OAuth App**.
   - **Application name:** Ultimate Gamer Admin.
   - **Homepage URL:** the website, `https://the-ultimate-gamer.github.io`.
   - **Authorization callback URL:** the same URL. The device flow doesn't use it, but GitHub requires one.
   - Tick **Enable Device Flow**.
2. Register it and copy the **Client ID**. Don't create a client secret: the device flow doesn't need one, and a desktop app can't keep it secret anyway. The client id isn't secret.
3. Put it in `OAUTH_CLIENT_ID` in `src/github.rs` (`Some("Ov23li…")`) and release a new version.
4. If the organisation restricts third-party application access (Settings → Third-party access → OAuth application policy), approve **Ultimate Gamer Admin** there, or its tokens can't see the `data` repository.

The app asks for the `repo` scope, because OAuth Apps can't be limited to one repository. That token can reach every repository the person can. A fine-grained token stays the narrower choice for anyone who prefers it.

To try a client id without rebuilding, set `UG_GITHUB_CLIENT_ID=<client id>`. `UG_GITHUB_WEB` (like `UG_GITHUB_API` for the REST API) points the sign-in at another host, which the tests use for a local stand-in.

Run with `UG_AUTOSHOT=<folder>` to visit every screen, save a screenshot of each to that folder, and quit. This checks quickly that every screen still renders.

### Code map

| File | What it holds |
| --- | --- |
| `src/rules.rs` | **The tournament rules**: points, Challengers, champion, ranks, final placement. The only implementation |
| `src/export.rs` | Builds every JSON document and share page; writes `data/` (keeps `generatedAt` for unchanged files) |
| `src/validate.rs` | Database consistency checks, and "data/ matches a fresh export" |
| `src/model.rs` | Rows as stored in SQLite, and the queries that read them |
| `src/repo.rs` | Every change the app makes to the database, with friendly errors |
| `src/db.rs`, `src/seed.rs` | Open, create and migrate the database (`schema/schema.sql` is compiled in); reference and demo data |
| `src/github.rs`, `src/sync.rs` | GitHub REST client; download, pending changes, publish as one commit (blobs → tree → commit → branch) |
| `src/workspace.rs`, `src/config.rs` | Folder layout (`database/`, `data/`, `avatars/`); per-user settings |
| `src/cli.rs` | The commands above |
| `src/gui/` | The desktop app (egui). Screens return `Action`s; `mod.rs` applies them (undo snapshot → change → export → validate → reload) |
| `src/tests.rs` | End-to-end tests of rules, export, validation and demo data |

Rules for changes:

- The app stores **facts only**. Anything derived comes from `rules.rs` through the exporter.
- The "If you finish now…" preview runs the real rules on an in-memory copy of the database, so nothing is computed twice.
- The JSON output is a public contract. Its shape and key order are covered by tests, and CI checks that the committed `data/` matches a fresh export.
