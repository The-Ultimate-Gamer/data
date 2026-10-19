#!/usr/bin/env python3
"""Validate the database and check that the committed JSON matches a fresh export.

Exits with status 1 when errors are found. Warnings are printed but do not fail.
"""

from __future__ import annotations

import argparse
import json
import re
import sqlite3
import sys
from pathlib import Path

from ugdata import (
    AVATAR_EXTENSIONS,
    DEFAULT_AVATARS,
    DEFAULT_DB,
    DEFAULT_OUT,
    SCHEMA_VERSION,
    SHARE_DIR,
    SLUG_RE,
    Exporter,
    compute_tournament,
    connect,
)

# Discord CDN links carry an expiry (ex=...) and stop working after about a day.
EXPIRING_AVATAR = re.compile(r"^https://(cdn|media)\.discordapp\.(com|net)/.*[?&]ex=", re.IGNORECASE)


class Report:
    def __init__(self) -> None:
        self.errors: list[str] = []
        self.warnings: list[str] = []

    def error(self, msg: str) -> None:
        self.errors.append(msg)

    def warn(self, msg: str) -> None:
        self.warnings.append(msg)


def validate_database(conn: sqlite3.Connection, avatars_dir: Path | None = DEFAULT_AVATARS) -> Report:
    report = Report()

    integrity = conn.execute("PRAGMA integrity_check").fetchone()[0]
    if integrity != "ok":
        report.error(f"SQLite integrity check failed: {integrity}")
    for row in conn.execute("PRAGMA foreign_key_check"):
        report.error(f"Foreign key violation in {row[0]} (rowid {row[1]}) -> {row[2]}")

    for table in ("players", "games", "tournaments", "categories", "awards"):
        for (slug,) in conn.execute(f"SELECT slug FROM {table}"):
            if not SLUG_RE.match(slug):
                report.error(f"{table}: invalid slug {slug!r} (use lowercase kebab-case)")

    names = {r["id"]: r["name"] for r in conn.execute("SELECT id, name FROM players")}
    game_names = {r["id"]: r["name"] for r in conn.execute("SELECT id, name FROM games")}

    slugs = set()
    for p in conn.execute("SELECT slug, name, avatar_url FROM players"):
        slugs.add(p["slug"])
        if p["avatar_url"] and EXPIRING_AVATAR.match(p["avatar_url"]):
            report.warn(
                f"player {p['slug']}: the avatar URL is a signed Discord link that expires; "
                f"save the picture as avatars/{p['slug']}.jpg instead"
            )
    if avatars_dir is not None and avatars_dir.is_dir():
        for path in sorted(avatars_dir.iterdir()):
            if path.name.startswith("."):
                continue
            if path.suffix.lower() not in AVATAR_EXTENSIONS:
                report.warn(f"avatars/{path.name}: not a supported picture ({', '.join(AVATAR_EXTENSIONS)})")
            elif path.stem not in slugs:
                report.warn(f"avatars/{path.name}: no player with the slug {path.stem!r}")

    for t in conn.execute("SELECT * FROM tournaments ORDER BY id"):
        tid, label = t["id"], f"tournament {t['slug']}"
        participants = {r[0] for r in conn.execute("SELECT player_id FROM tournament_players WHERE tournament_id = ?", (tid,))}
        points_table = {
            r["placement"]: r["points"]
            for r in conn.execute("SELECT placement, points FROM tournament_points WHERE tournament_id = ?", (tid,))
        }
        max_points = max(points_table.values(), default=None)
        rounds = list(conn.execute("SELECT * FROM rounds WHERE tournament_id = ? ORDER BY number", (tid,)))
        completed = [r["number"] for r in rounds if r["completed"]]
        pending = [r["number"] for r in rounds if not r["completed"]]

        if t["status"] == "upcoming" and completed:
            report.error(f"{label}: status is 'upcoming' but has completed rounds")
        if completed != list(range(1, len(completed) + 1)):
            report.error(f"{label}: completed rounds must be numbered 1..n without gaps (got {completed})")
        if len(pending) > 1:
            report.error(f"{label}: more than one unfinished round ({pending})")
        if pending and completed and pending[0] < max(completed):
            report.error(f"{label}: unfinished round {pending[0]} comes before completed round {max(completed)}")

        for r in rounds:
            number = r["number"]
            where = f"{label} round {number}"
            results = list(
                conn.execute(
                    "SELECT * FROM results WHERE tournament_id = ? AND round_number = ?", (tid, number)
                )
            )
            in_round = {res["player_id"] for res in results}
            if r["completed"]:
                if r["game_id"] is None:
                    report.error(f"{where}: completed round has no game")
                if len(results) < 2:
                    report.error(f"{where}: completed round needs results for at least two players")
                winners = [res for res in results if res["placement"] == 1]
                if len(winners) != 1:
                    report.error(f"{where}: needs exactly one winner (placement 1), found {len(winners)}")
            elif results:
                report.error(f"{where}: has results but is not marked completed")

            for res in results:
                who = names.get(res["player_id"], res["player_id"])
                if res["player_id"] not in participants:
                    report.error(f"{where}: {who} is not registered for the tournament")
                nemesis = res["nemesis_player_id"]
                if nemesis is not None and nemesis not in in_round:
                    report.warn(f"{where}: {who}'s Nemesis {names.get(nemesis, nemesis)} has no result in this round")
                # Sanity check against typos: base points come from the tournament's points table.
                if max_points is not None and res["base_points"] > max_points:
                    report.error(
                        f"{where}: {who} has {res['base_points']} base points, more than any placement gives "
                        f"(max {max_points}); check for a typo"
                    )
                elif res["placement"] in points_table and res["base_points"] != points_table[res["placement"]]:
                    report.warn(
                        f"{where}: {who} placed {res['placement']} and got {res['base_points']} base points; "
                        f"the points table gives {points_table[res['placement']]}"
                    )

            announced = {
                a["player_id"]: a["nemesis_player_id"]
                for a in conn.execute(
                    "SELECT player_id, nemesis_player_id FROM nemesis_assignments WHERE tournament_id = ? AND round_number = ?",
                    (tid, number),
                )
            }
            for player_id, nemesis in announced.items():
                for pid in (player_id, nemesis):
                    if pid not in participants:
                        report.error(f"{where}: Nemesis pair with {names.get(pid, pid)}, who is not registered")
            if r["completed"]:
                for res in results:
                    pid = res["player_id"]
                    if pid in announced and res["nemesis_player_id"] != announced[pid]:
                        report.warn(
                            f"{where}: {names.get(pid, pid)}'s result has a different Nemesis than the one announced "
                            f"({names.get(announced[pid])})"
                        )

            noms = list(
                conn.execute(
                    "SELECT * FROM nominations WHERE tournament_id = ? AND round_number = ?", (tid, number)
                )
            )
            for n in noms:
                if n["player_id"] is not None and n["player_id"] not in participants:
                    report.error(f"{where}: nomination by non-participant {names.get(n['player_id'])}")
            if noms and r["game_id"] is not None and r["game_id"] not in {n["game_id"] for n in noms}:
                report.error(
                    f"{where}: selected game {game_names.get(r['game_id'])} is not among the recorded nominations"
                )

        for row in conn.execute(
            """SELECT player_id, SUM(double_down) AS used FROM results WHERE tournament_id = ?
               GROUP BY player_id HAVING used > ?""",
            (tid, t["double_down_tokens"]),
        ):
            report.error(f"{label}: {names.get(row['player_id'])} used {row['used']} Double Down tokens (max {t['double_down_tokens']})")
        for row in conn.execute(
            """SELECT player_id, COUNT(*) AS used FROM nominations WHERE tournament_id = ? AND source = 'token'
               GROUP BY player_id HAVING used > ?""",
            (tid, t["nominate_tokens"]),
        ):
            report.error(f"{label}: {names.get(row['player_id'])} used {row['used']} Nominate tokens (max {t['nominate_tokens']})")

        state = compute_tournament(conn, t, names)
        if state.ignored_rounds:
            report.error(
                f"{label}: rounds {state.ignored_rounds} were played after the tournament was decided in round {state.deciding_round}"
            )
        if state.champion_id is not None and t["status"] != "completed":
            report.error(
                f"{label}: {names[state.champion_id]} won round {state.deciding_round} as a Challenger; set status to 'completed'"
            )
        if t["status"] == "completed" and state.champion_id is None:
            report.warn(f"{label}: marked completed but no Challenger has won a game")
        if pending and state.champion_id is not None:
            report.error(f"{label}: has an unfinished round although the tournament is decided")

    for g in conn.execute(
        """SELECT g.id, g.award_slug, g.player_id, g.tournament_id, a.scope, t.slug AS t_slug
           FROM award_grants g JOIN awards a ON a.slug = g.award_slug LEFT JOIN tournaments t ON t.id = g.tournament_id"""
    ):
        who = names.get(g["player_id"])
        if g["scope"] == "tournament" and g["tournament_id"] is None:
            report.error(f"award grant {g['id']}: '{g['award_slug']}' is a tournament award but has no tournament")
        if g["tournament_id"] is not None and not conn.execute(
            "SELECT 1 FROM tournament_players WHERE tournament_id = ? AND player_id = ?",
            (g["tournament_id"], g["player_id"]),
        ).fetchone():
            report.error(f"award grant {g['id']}: {who} did not take part in {g['t_slug']}")

    return report


def validate_output(conn: sqlite3.Connection, out_dir: Path, avatars_dir: Path | None = DEFAULT_AVATARS) -> Report:
    """Check every JSON document's envelope and that it matches a fresh export of the database."""
    report = Report()
    exporter = Exporter(conn, avatars_dir)
    expected = exporter.documents()

    published = out_dir / "avatars"
    wanted = set(exporter.avatar_files.values())
    present_pictures = {p.name for p in published.iterdir() if p.is_file()} if published.is_dir() else set()
    for name in sorted(wanted - present_pictures):
        report.error(f"avatars/{name}: missing — run scripts/export_json.py")
    for name in sorted(present_pictures - wanted):
        report.error(f"avatars/{name}: not produced by the export — run scripts/export_json.py")
    for name in sorted(wanted & present_pictures):
        if avatars_dir is not None and (published / name).read_bytes() != (avatars_dir / name).read_bytes():
            report.error(f"avatars/{name}: out of date — run scripts/export_json.py")
    present = {p.relative_to(out_dir).as_posix(): p for p in out_dir.rglob("*.json")} if out_dir.exists() else {}

    shares = exporter.share_pages()
    share_root = out_dir / SHARE_DIR
    present_shares = (
        {p.relative_to(out_dir).as_posix(): p for p in share_root.rglob("*") if p.is_file()}
        if share_root.is_dir()
        else {}
    )
    for rel in sorted(set(shares) - set(present_shares)):
        report.error(f"{rel}: missing — run scripts/export_json.py")
    for rel in sorted(set(present_shares) - set(shares)):
        report.error(f"{rel}: not produced by the export — run scripts/export_json.py")
    for rel in sorted(set(shares) & set(present_shares)):
        if present_shares[rel].read_text(encoding="utf-8") != shares[rel]:
            report.error(f"{rel}: out of date — run scripts/export_json.py")

    for rel in sorted(set(expected) - set(present)):
        report.error(f"{rel}: missing — run scripts/export_json.py")
    for rel in sorted(set(present) - set(expected)):
        report.error(f"{rel}: not produced by the export — run scripts/export_json.py")

    for rel in sorted(set(present) & set(expected)):
        try:
            doc = json.loads(present[rel].read_text(encoding="utf-8"))
        except json.JSONDecodeError as exc:
            report.error(f"{rel}: invalid JSON ({exc})")
            continue
        if doc.get("schemaVersion") != SCHEMA_VERSION:
            report.error(f"{rel}: schemaVersion must be {SCHEMA_VERSION}")
        if not isinstance(doc.get("generatedAt"), str) or not doc["generatedAt"].endswith("Z"):
            report.error(f"{rel}: generatedAt must be an ISO 8601 UTC timestamp")
        if doc.get("data") != expected[rel]:
            report.error(f"{rel}: out of date with the database — run scripts/export_json.py")
    return report


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--db", type=Path, default=DEFAULT_DB)
    parser.add_argument("--out", type=Path, default=DEFAULT_OUT)
    parser.add_argument("--skip-json", action="store_true", help="only validate the database")
    args = parser.parse_args(argv)

    if not args.db.exists():
        raise SystemExit(f"{args.db} does not exist")
    conn = connect(args.db)
    reports = [validate_database(conn)]
    if not args.skip_json:
        reports.append(validate_output(conn, args.out))
    conn.close()

    errors = [e for r in reports for e in r.errors]
    warnings = [w for r in reports for w in r.warnings]
    for w in warnings:
        print(f"warning: {w}")
    for e in errors:
        print(f"error: {e}")
    print(f"{len(errors)} error(s), {len(warnings)} warning(s)")
    return 1 if errors else 0


if __name__ == "__main__":
    sys.exit(main())
