"""Shared test helpers. Tests use only the standard library: python3 -m unittest discover tests"""

from __future__ import annotations

import sqlite3
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent / "scripts"))

from seed import seed_demo, seed_reference  # noqa: E402
from ugdata import connect, create_schema  # noqa: E402


def empty_db() -> sqlite3.Connection:
    conn = connect(":memory:")
    create_schema(conn)
    seed_reference(conn)
    return conn


def demo_db() -> sqlite3.Connection:
    conn = empty_db()
    seed_demo(conn)
    conn.commit()
    return conn


def mini_tournament(conn: sqlite3.Connection, threshold: int = 10, status: str = "ongoing") -> int:
    """Three players (a, b, c) and four strategy positions; returns the tournament id."""
    for slug in ("a", "b", "c"):
        conn.execute("INSERT INTO players (slug, name) VALUES (?, ?)", (slug, slug.upper()))
    conn.execute("INSERT INTO games (slug, name) VALUES ('g', 'Game')")
    tid = conn.execute(
        "INSERT INTO tournaments (slug, name, status, challenger_threshold) VALUES ('t', 'T', ?, ?)",
        (status, threshold),
    ).lastrowid
    conn.execute("INSERT INTO tournament_players SELECT ?, id FROM players", (tid,))
    for pos in range(1, 5):
        conn.execute("INSERT INTO tournament_categories VALUES (?, ?, 'strategy')", (tid, pos))
    return tid


def play(conn, tid, number, order, points=(6, 3, 0), double_down=(), nemesis=None):
    ids = {r["slug"]: r["id"] for r in conn.execute("SELECT id, slug FROM players")}
    game = conn.execute("SELECT id FROM games WHERE slug = 'g'").fetchone()[0]
    conn.execute("INSERT INTO rounds VALUES (?, ?, ?, NULL, 1)", (tid, number, game))
    nemesis = nemesis or {}
    for placement, slug in enumerate(order, start=1):
        conn.execute(
            "INSERT INTO results VALUES (?, ?, ?, ?, ?, ?, ?)",
            (tid, number, ids[slug], placement, points[placement - 1], 1 if slug in double_down else 0,
             ids[nemesis[slug]] if slug in nemesis else None),
        )
