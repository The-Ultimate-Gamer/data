#!/usr/bin/env python3
"""Load reference data (categories and awards) and manage the fictional demo dataset.

    python3 scripts/seed.py                         # reference data into the real database
    python3 scripts/seed.py --add-demo              # add flagged demo data (e.g. to show the site off)
    python3 scripts/seed.py --remove-demo           # remove every flagged demo row again
    python3 scripts/seed.py --demo --db demo/demo.sqlite   # separate database with only demo data

Reference data is upserted, so running this again is safe. Demo rows are marked
is_demo = 1; the website shows a "fictional demo data" banner while any exist.
"""

from __future__ import annotations

import argparse
import sqlite3
import sys
from pathlib import Path

from init_db import init_db
from ugdata import DEFAULT_DB, connect

CATEGORIES = [
    ("strategy", "Strategy", "♟️", "Planning, economy and long-term decisions."),
    ("combat", "Combat", "⚔️", "Direct confrontation: aim, reflexes and nerve."),
    ("puzzle", "Puzzle", "🧩", "Logic and pattern recognition under pressure."),
    ("platform", "Platform", "🍄", "Jumps, timing and precise movement."),
    ("chaos", "Chaos", "🌀", "Unpredictable party games where anything can happen."),
    ("racing", "Racing", "🏁", "First across the line wins."),
    ("teamwork", "Teamwork", "🤝", "Temporary alliances: cooperate now, compete later."),
    ("wildcard", "Wildcard", "🃏", "Anything goes."),
    ("knowledge", "Knowledge", "🧠", "Trivia and quizzes."),
    ("tabletop", "Tabletop", "🎲", "Board and card games, digital or physical."),
    ("offline", "Offline", "🎯", "Away from the screen."),
    ("idle", "Idle", "⏳", "Patience and optimisation over a longer timeframe."),
    ("speedrun", "Speedrun", "⏱️", "Fastest completion of a set goal wins."),
    ("simulation", "Simulation", "🛠️", "Build, manage or simulate better than everyone else."),
]

AWARDS = [
    ("ultimate-gamer", "Ultimate Gamer", "👑", "tournament", "The tournament champion."),
    ("challenger", "Challenger", "⚔️", "tournament", "Reached the Challenger threshold for the first time."),
    ("dominator", "Dominator", "📈", "tournament", "Highest-scoring player when the tournament ends."),
    ("contender", "Contender", "🥊", "tournament", "Highest-scoring player who never became a Challenger."),
    ("podium", "Podium", "🥉", "tournament", "Most podium finishes across games."),
    ("believer", "Believer", "🙏", "tournament", "Won a game while using a Double Down token."),
    ("nominator", "Nominator", "🗳️", "tournament", "Had a game selected from one of their Nominate tokens."),
    ("rookie", "Rookie", "🌱", "tournament", "Highest-placed player in their first tournament."),
    ("underdog", "Underdog", "🐕", "tournament", "Largest improvement between consecutive tournaments."),
    ("fallout", "Fallout", "☢️", "tournament", "Largest decline between consecutive tournaments."),
    ("legend", "Legend", "🏛️", "career", "Won at least three Ultimate Gamer tournaments."),
    ("nemesis", "Nemesis", "😈", "tournament", "Defeated their assigned Nemesis the most times."),
    ("victim", "Victim", "🎯", "tournament", "Was defeated as someone else's Nemesis the most times."),
    ("ghost", "Ghost", "👻", "tournament", "Never finished last in a game during the tournament."),
    ("survivor", "Survivor", "🩹", "tournament", "Won a game while last in the overall standings."),
]


def seed_reference(conn: sqlite3.Connection) -> None:
    conn.executemany(
        """INSERT INTO categories (slug, name, icon, description, sort_order) VALUES (?, ?, ?, ?, ?)
           ON CONFLICT (slug) DO UPDATE SET name = excluded.name, icon = excluded.icon,
           description = excluded.description, sort_order = excluded.sort_order""",
        [(*c, i) for i, c in enumerate(CATEGORIES)],
    )
    conn.executemany(
        """INSERT INTO awards (slug, name, icon, scope, description, sort_order) VALUES (?, ?, ?, ?, ?, ?)
           ON CONFLICT (slug) DO UPDATE SET name = excluded.name, icon = excluded.icon, scope = excluded.scope,
           description = excluded.description, sort_order = excluded.sort_order""",
        [(*a, i) for i, a in enumerate(AWARDS)],
    )
    conn.execute("INSERT OR IGNORE INTO meta (key, value) VALUES ('site_name', 'The Ultimate Gamer')")


# ------------------------------------------------------------------------- demo
# Entirely fictional. Flagged is_demo = 1 so it can be removed again in one step.

DEMO_PLAYERS = ["Nova", "Blitz", "Echo", "Rook", "Vex", "Kite"]

DEMO_GAMES = [
    ("age-of-empires-2", "Age of Empires II", ["strategy"], 2, 8, "Classic real-time strategy."),
    ("counter-strike-2", "Counter-Strike 2", ["combat", "teamwork"], 2, 10, None),
    ("tetris-99", "Tetris 99", ["puzzle"], 1, 99, None),
    ("super-mario-bros-wonder", "Super Mario Bros. Wonder", ["platform"], 1, 4, None),
    ("fall-guys", "Fall Guys", ["chaos"], 1, 32, None),
    ("mario-kart-8-deluxe", "Mario Kart 8 Deluxe", ["racing", "chaos"], 1, 12, None),
    ("trackmania", "Trackmania", ["racing", "speedrun"], 1, 64, "Time-attack racing on wild tracks."),
    ("rocket-league", "Rocket League", ["wildcard", "teamwork"], 2, 8, None),
    ("golf-with-your-friends", "Golf With Your Friends", ["wildcard", "chaos"], 1, 12, None),
    ("trivia-murder-party", "Trivia Murder Party", ["knowledge", "chaos"], 1, 8, None),
    ("brawlhalla", "Brawlhalla", ["combat"], 2, 8, None),
    ("catan", "Catan", ["tabletop", "strategy"], 3, 6, None),
    ("overcooked-2", "Overcooked! 2", ["teamwork", "chaos"], 1, 4, None),
    ("darts", "Darts", ["offline"], 2, 8, "501, double out."),
    ("cookie-clicker", "Cookie Clicker", ["idle"], None, None, None),
    ("celeste", "Celeste", ["speedrun", "platform"], 1, 1, None),
    ("factorio", "Factorio", ["simulation", "strategy"], 1, 8, None),
    ("portal-2", "Portal 2 (co-op)", ["puzzle", "teamwork"], 2, 2, None),
]
DEMO_UNAVAILABLE = {"brawlhalla"}

# Each round: (category, game, finishing order, double-down users, {player: nemesis}, played_at)
DEMO_TOURNAMENTS = [
    {
        "slug": "demo-2025",
        "name": "Demo LAN 2025",
        "edition": "Demo edition",
        "start_date": "2025-11-07",
        "end_date": "2025-11-08",
        "location": "The basement",
        "status": "completed",
        "threshold": 20,
        "players": ["blitz", "echo", "nova", "rook", "vex"],
        "points": [6, 4, 3, 2, 1],
        "categories": ["wildcard", "knowledge", "racing", "combat", "tabletop", "teamwork", "simulation"],
        "rounds": [
            ("golf-with-your-friends", ["blitz", "rook", "nova", "echo", "vex"], [], {}, "2025-11-07T15:00:00Z"),
            ("trivia-murder-party", ["blitz", "echo", "nova", "vex", "rook"], [], {}, "2025-11-07T17:00:00Z"),
            ("trackmania", ["nova", "blitz", "echo", "rook", "vex"], [], {"nova": "rook", "vex": "blitz"}, "2025-11-07T19:30:00Z"),
            ("brawlhalla", ["rook", "blitz", "nova", "echo", "vex"], [], {"echo": "vex"}, "2025-11-07T22:00:00Z"),
            ("catan", ["echo", "blitz", "nova", "rook", "vex"], ["echo"], {}, "2025-11-08T12:00:00Z"),
            ("overcooked-2", ["echo", "blitz", "nova", "rook", "vex"], ["blitz"], {"nova": "vex", "rook": "nova"}, "2025-11-08T15:00:00Z"),
        ],
        "nominations": {5: [("darts", "rook", "winner"), ("overcooked-2", None, "vote"), ("catan", "rook", "token")]},
        "pending": None,
        "grants": [
            ("ultimate-gamer", "echo", "Won Overcooked! 2 as a Challenger"),
            ("dominator", "blitz", "32 points"),
            ("challenger", "blitz", None),
            ("challenger", "echo", None),
            ("challenger", "nova", None),
            ("contender", "rook", "17 points"),
            ("podium", "blitz", "6 podium finishes"),
            ("believer", "echo", None),
            ("nominator", "rook", None),
            ("nemesis", "nova", "2 Nemesis wins"),
            ("victim", "vex", None),
            ("ghost", "blitz", None),
        ],
        "records": [("Games played", "6", None), ("Biggest single-game haul", "12 pts", "echo")],
    },
    {
        "slug": "demo-2026",
        "name": "Demo LAN 2026",
        "edition": "Demo edition",
        "start_date": "2026-10-16",
        "end_date": None,
        "location": "Somewhere with good Wi-Fi",
        "status": "ongoing",
        "threshold": 20,
        "players": ["nova", "blitz", "echo", "rook", "vex", "kite"],
        "points": [6, 4, 3, 2, 1, 0],
        "categories": ["strategy", "combat", "puzzle", "platform", "chaos", "racing", "knowledge", "wildcard"],
        "rounds": [
            ("age-of-empires-2", ["nova", "blitz", "echo", "rook", "vex", "kite"], [], {"nova": "rook"}, "2026-10-16T14:00:00Z"),
            ("counter-strike-2", ["blitz", "nova", "rook", "echo", "kite", "vex"], ["blitz"], {"echo": "vex"}, "2026-10-16T16:30:00Z"),
            ("tetris-99", ["echo", "nova", "kite", "blitz", "rook", "vex"], [], {"echo": "nova"}, "2026-10-16T18:45:00Z"),
            ("super-mario-bros-wonder", ["nova", "rook", "vex", "echo", "blitz", "kite"], ["nova"], {}, "2026-10-16T20:30:00Z"),
            ("fall-guys", ["vex", "blitz", "nova", "echo", "kite", "rook"], [], {"vex": "nova"}, "2026-10-16T22:10:00Z"),
        ],
        "nominations": {
            6: [("mario-kart-8-deluxe", "vex", "winner"), ("trackmania", None, "vote"), ("rocket-league", "kite", "token")]
        },
        "pending": "mario-kart-8-deluxe",
        "grants": [
            ("challenger", "nova", None),
            ("challenger", "blitz", None),
            ("believer", "blitz", None),
            ("believer", "nova", None),
            ("survivor", "vex", "Won Fall Guys from last place"),
        ],
        "records": [("Biggest single-game haul", "12 pts", "nova")],
    },
]


def seed_demo(conn: sqlite3.Connection) -> None:
    player = {}
    for name in DEMO_PLAYERS:
        slug = name.lower()
        player[slug] = conn.execute(
            "INSERT INTO players (slug, name, is_demo) VALUES (?, ?, 1)", (slug, name)
        ).lastrowid

    game = {}
    for slug, name, cats, lo, hi, desc in DEMO_GAMES:
        existing = conn.execute("SELECT id FROM games WHERE slug = ?", (slug,)).fetchone()
        if existing:
            # Reuse a real game with the same slug; it is never removed with the demo data.
            game[slug] = existing[0]
            continue
        gid = conn.execute(
            """INSERT INTO games (slug, name, description, min_players, max_players, available, is_demo)
               VALUES (?, ?, ?, ?, ?, ?, 1)""",
            (slug, name, desc, lo, hi, 0 if slug in DEMO_UNAVAILABLE else 1),
        ).lastrowid
        game[slug] = gid
        conn.executemany(
            "INSERT INTO game_categories (game_id, category_slug) VALUES (?, ?)", [(gid, c) for c in cats]
        )

    for t in DEMO_TOURNAMENTS:
        tid = conn.execute(
            """INSERT INTO tournaments (slug, name, edition, start_date, end_date, location, status, challenger_threshold, is_demo)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?, 1)""",
            (t["slug"], t["name"], t["edition"], t["start_date"], t["end_date"], t["location"], t["status"], t["threshold"]),
        ).lastrowid
        conn.executemany(
            "INSERT INTO tournament_players (tournament_id, player_id) VALUES (?, ?)",
            [(tid, player[p]) for p in t["players"]],
        )
        conn.executemany(
            "INSERT INTO tournament_points (tournament_id, placement, points) VALUES (?, ?, ?)",
            [(tid, i, pts) for i, pts in enumerate(t["points"], start=1)],
        )
        conn.executemany(
            "INSERT INTO tournament_categories (tournament_id, position, category_slug) VALUES (?, ?, ?)",
            [(tid, i, c) for i, c in enumerate(t["categories"], start=1)],
        )
        for number, (game_slug, order, double_downs, nemeses, played_at) in enumerate(t["rounds"], start=1):
            conn.execute(
                "INSERT INTO rounds (tournament_id, number, game_id, played_at, completed) VALUES (?, ?, ?, ?, 1)",
                (tid, number, game[game_slug], played_at),
            )
            for placement, slug in enumerate(order, start=1):
                nemesis = nemeses.get(slug)
                conn.execute(
                    """INSERT INTO results (tournament_id, round_number, player_id, placement, base_points,
                       double_down, nemesis_player_id) VALUES (?, ?, ?, ?, ?, ?, ?)""",
                    (tid, number, player[slug], placement, t["points"][placement - 1],
                     1 if slug in double_downs else 0, player[nemesis] if nemesis else None),
                )
        if t["pending"]:
            conn.execute(
                "INSERT INTO rounds (tournament_id, number, game_id, completed) VALUES (?, ?, ?, 0)",
                (tid, len(t["rounds"]) + 1, game[t["pending"]]),
            )
        for number, noms in t["nominations"].items():
            conn.executemany(
                "INSERT INTO nominations (tournament_id, round_number, game_id, player_id, source) VALUES (?, ?, ?, ?, ?)",
                [(tid, number, game[g], player[p] if p else None, src) for g, p, src in noms],
            )
        conn.executemany(
            "INSERT INTO award_grants (award_slug, player_id, tournament_id, detail) VALUES (?, ?, ?, ?)",
            [(a, player[p], tid, d) for a, p, d in t["grants"]],
        )
        conn.executemany(
            "INSERT INTO tournament_records (tournament_id, position, label, value, player_id) VALUES (?, ?, ?, ?, ?)",
            [(tid, i, label, value, player[p] if p else None) for i, (label, value, p) in enumerate(t["records"], start=1)],
        )


def demo_present(conn: sqlite3.Connection) -> bool:
    return any(
        conn.execute(f"SELECT 1 FROM {table} WHERE is_demo = 1 LIMIT 1").fetchone()
        for table in ("players", "games", "tournaments")
    )


def add_demo(conn: sqlite3.Connection) -> None:
    """Add the flagged demo dataset next to any real data. Fails without changes on slug clashes."""
    if demo_present(conn):
        raise ValueError("Demo data is already loaded.")
    clashes = [
        f"player '{name.lower()}'"
        for name in DEMO_PLAYERS
        if conn.execute("SELECT 1 FROM players WHERE slug = ?", (name.lower(),)).fetchone()
    ] + [
        f"tournament '{t['slug']}'"
        for t in DEMO_TOURNAMENTS
        if conn.execute("SELECT 1 FROM tournaments WHERE slug = ?", (t["slug"],)).fetchone()
    ]
    if clashes:
        raise ValueError("Demo data would clash with existing " + ", ".join(clashes) + ".")
    seed_demo(conn)


def remove_demo(conn: sqlite3.Connection) -> dict[str, int]:
    """Delete every demo-flagged tournament, player and unused demo game."""
    mixed = conn.execute(
        """SELECT p.name, t.name FROM tournament_players tp
           JOIN players p ON p.id = tp.player_id JOIN tournaments t ON t.id = tp.tournament_id
           WHERE p.is_demo = 1 AND t.is_demo = 0 LIMIT 1"""
    ).fetchone()
    if mixed:
        raise ValueError(f"Demo player {mixed[0]} is registered in the real tournament {mixed[1]}; remove them there first.")
    counts = {
        "tournaments": conn.execute("SELECT COUNT(*) FROM tournaments WHERE is_demo = 1").fetchone()[0],
        "players": conn.execute("SELECT COUNT(*) FROM players WHERE is_demo = 1").fetchone()[0],
    }
    conn.execute("DELETE FROM award_grants WHERE player_id IN (SELECT id FROM players WHERE is_demo = 1)")
    conn.execute("DELETE FROM tournaments WHERE is_demo = 1")
    conn.execute("DELETE FROM players WHERE is_demo = 1")
    counts["games"] = conn.execute(
        """DELETE FROM games WHERE is_demo = 1
           AND id NOT IN (SELECT game_id FROM rounds WHERE game_id IS NOT NULL)
           AND id NOT IN (SELECT game_id FROM nominations)"""
    ).rowcount
    return counts


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--db", type=Path, default=DEFAULT_DB)
    group = parser.add_mutually_exclusive_group()
    group.add_argument("--demo", action="store_true", help="recreate --db (not the real one) with only demo data")
    group.add_argument("--add-demo", action="store_true", help="add flagged demo data to --db")
    group.add_argument("--remove-demo", action="store_true", help="remove all flagged demo data from --db")
    args = parser.parse_args(argv)

    if args.demo:
        if args.db.resolve() == DEFAULT_DB.resolve():
            raise SystemExit("--demo recreates the database; use --add-demo for the real one.")
        init_db(args.db, force=True)
    elif not args.db.exists():
        raise SystemExit(f"{args.db} does not exist; run scripts/init_db.py first")

    conn = connect(args.db)
    try:
        with conn:
            seed_reference(conn)
            if args.demo or args.add_demo:
                add_demo(conn)
                print(f"Added demo data to {args.db}")
            elif args.remove_demo:
                counts = remove_demo(conn)
                print(
                    f"Removed demo data from {args.db}: {counts['tournaments']} tournaments, "
                    f"{counts['players']} players, {counts['games']} games"
                )
            else:
                print(f"Seeded reference data into {args.db}")
    except ValueError as exc:
        raise SystemExit(f"error: {exc}") from exc
    finally:
        conn.close()
    return 0


if __name__ == "__main__":
    sys.exit(main())
