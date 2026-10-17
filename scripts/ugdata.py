"""Shared database access, tournament rules and JSON document builders.

The SQLite database stores raw facts only (participants, results, nominations,
award grants). Everything the website shows — standings, Challenger and
champion status, remaining tokens and statistics — is derived here, so the
rules live in exactly one place.
"""

from __future__ import annotations

import json
import re
import sqlite3
from dataclasses import dataclass, field
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parent.parent
DEFAULT_DB = ROOT / "database" / "ultimate-gamer.sqlite"
SCHEMA_FILE = ROOT / "schema" / "schema.sql"
DEFAULT_OUT = ROOT / "data"

SCHEMA_VERSION = 1
SLUG_RE = re.compile(r"^[a-z0-9]+(?:-[a-z0-9]+)*$")
RECENT_RESULTS = 6

Json = dict[str, Any]


# --------------------------------------------------------------------------- db


def connect(path: Path | str) -> sqlite3.Connection:
    conn = sqlite3.connect(path)
    conn.row_factory = sqlite3.Row
    conn.execute("PRAGMA foreign_keys = ON")
    return conn


def create_schema(conn: sqlite3.Connection) -> None:
    conn.executescript(SCHEMA_FILE.read_text(encoding="utf-8"))


def utc_now() -> str:
    return datetime.now(timezone.utc).replace(microsecond=0).isoformat().replace("+00:00", "Z")


# ------------------------------------------------------------------------ rules


@dataclass
class Result:
    player_id: int
    placement: int
    base_points: int
    double_down: bool
    nemesis_id: int | None
    defeated_nemesis: bool | None = None
    nemesis_bonus: int = 0
    points: int = 0


@dataclass
class Round:
    number: int
    category: str
    game_id: int | None
    played_at: str | None
    completed: bool
    results: list[Result] = field(default_factory=list)
    winner_id: int | None = None
    new_challengers: list[int] = field(default_factory=list)
    decided: bool = False


@dataclass
class PlayerState:
    player_id: int
    points: int = 0
    challenger_since: int | None = None
    games_played: int = 0
    games_won: int = 0
    podiums: int = 0
    category_wins: list[str] = field(default_factory=list)
    double_downs_used: int = 0
    nominations_used: int = 0
    rank: int = 0


@dataclass
class TournamentState:
    row: sqlite3.Row
    positions: list[tuple[int, str]]
    rounds: dict[int, Round]
    players: dict[int, PlayerState]
    champion_id: int | None = None
    deciding_round: int | None = None
    ignored_rounds: list[int] = field(default_factory=list)
    order: list[int] = field(default_factory=list)

    @property
    def id(self) -> int:
        return self.row["id"]

    @property
    def completed_rounds(self) -> list[Round]:
        return [r for _, r in sorted(self.rounds.items()) if r.completed and r.number not in self.ignored_rounds]

    def standings(self) -> list[PlayerState]:
        return [self.players[pid] for pid in self.order]

    def active_position(self) -> tuple[int, str] | None:
        """Category position currently being played or selected, if the tournament is live."""
        if self.row["status"] == "completed" or self.champion_id is not None:
            return None
        for position, category in self.positions:
            rnd = self.rounds.get(position)
            if rnd is None or not rnd.completed:
                return position, category
        return None


def compute_tournament(conn: sqlite3.Connection, row: sqlite3.Row, names: dict[int, str]) -> TournamentState:
    tid = row["id"]
    threshold = row["challenger_threshold"]
    positions = [
        (r["position"], r["category_slug"])
        for r in conn.execute(
            "SELECT position, category_slug FROM tournament_categories WHERE tournament_id = ? ORDER BY position",
            (tid,),
        )
    ]
    category_at = dict(positions)
    rounds: dict[int, Round] = {}
    for r in conn.execute("SELECT * FROM rounds WHERE tournament_id = ? ORDER BY number", (tid,)):
        rounds[r["number"]] = Round(
            number=r["number"],
            category=category_at[r["number"]],
            game_id=r["game_id"],
            played_at=r["played_at"],
            completed=bool(r["completed"]),
        )
    for r in conn.execute(
        "SELECT * FROM results WHERE tournament_id = ? ORDER BY round_number, placement, player_id", (tid,)
    ):
        rounds[r["round_number"]].results.append(
            Result(
                player_id=r["player_id"],
                placement=r["placement"],
                base_points=r["base_points"],
                double_down=bool(r["double_down"]),
                nemesis_id=r["nemesis_player_id"],
            )
        )

    players = {
        r["player_id"]: PlayerState(r["player_id"])
        for r in conn.execute("SELECT player_id FROM tournament_players WHERE tournament_id = ?", (tid,))
    }
    for r in conn.execute(
        "SELECT player_id, COUNT(*) AS n FROM nominations WHERE tournament_id = ? AND source = 'token' GROUP BY player_id",
        (tid,),
    ):
        if r["player_id"] in players:
            players[r["player_id"]].nominations_used = r["n"]

    state = TournamentState(row=row, positions=positions, rounds=rounds, players=players)

    for number in sorted(rounds):
        rnd = rounds[number]
        if not rnd.completed:
            continue
        if state.champion_id is not None:
            # The tournament ended when a Challenger won; later rounds are invalid (see validate.py).
            state.ignored_rounds.append(number)
            continue

        placement_of = {res.player_id: res.placement for res in rnd.results}
        for res in rnd.results:
            if res.nemesis_id is not None and res.nemesis_id in placement_of:
                res.defeated_nemesis = res.placement < placement_of[res.nemesis_id]
            res.nemesis_bonus = 1 if res.defeated_nemesis else 0
            res.points = (res.base_points + res.nemesis_bonus) * (2 if res.double_down else 1)

        winners = [res.player_id for res in rnd.results if res.placement == 1]
        rnd.winner_id = winners[0] if len(winners) == 1 else None

        # A player who was already a Challenger before this round and wins it is crowned.
        if rnd.winner_id is not None:
            winner = players.get(rnd.winner_id)
            if winner and winner.challenger_since is not None:
                state.champion_id = rnd.winner_id
                state.deciding_round = number
                rnd.decided = True

        for res in rnd.results:
            p = players.get(res.player_id)
            if p is None:
                continue
            p.points += res.points
            p.games_played += 1
            if res.placement <= 3:
                p.podiums += 1
            if res.double_down:
                p.double_downs_used += 1
            if res.player_id == rnd.winner_id:
                p.games_won += 1
                p.category_wins.append(rnd.category)

        for res in rnd.results:
            p = players.get(res.player_id)
            if p and p.challenger_since is None and p.points >= threshold:
                p.challenger_since = number
                rnd.new_challengers.append(p.player_id)

    # Competition ranking (1, 1, 3) by points; ties are shared, never broken.
    ordered = sorted(players.values(), key=lambda p: (-p.points, names.get(p.player_id, "")))
    previous_points, previous_rank = None, 0
    for index, p in enumerate(ordered, start=1):
        p.rank = previous_rank if p.points == previous_points else index
        previous_points, previous_rank = p.points, p.rank
    state.order = [p.player_id for p in ordered]
    return state


# ---------------------------------------------------------------------- exports


class Exporter:
    """Builds every JSON document from one consistent snapshot of the database."""

    def __init__(self, conn: sqlite3.Connection):
        self.conn = conn
        self.players = {r["id"]: r for r in conn.execute("SELECT * FROM players ORDER BY name COLLATE NOCASE")}
        self.names = {pid: r["name"] for pid, r in self.players.items()}
        self.games = {r["id"]: r for r in conn.execute("SELECT * FROM games ORDER BY name COLLATE NOCASE")}
        self.categories = list(conn.execute("SELECT * FROM categories ORDER BY sort_order, slug"))
        self.category_order = {c["slug"]: i for i, c in enumerate(self.categories)}
        self.awards = list(conn.execute("SELECT * FROM awards ORDER BY sort_order, slug"))
        self.award_order = {a["slug"]: i for i, a in enumerate(self.awards)}
        self.tournaments = list(
            conn.execute("SELECT * FROM tournaments ORDER BY start_date IS NULL, start_date, id")
        )
        self.tournament_rank = {t["id"]: i for i, t in enumerate(self.tournaments)}
        self.states = {t["id"]: compute_tournament(conn, t, self.names) for t in self.tournaments}
        self.grants = list(
            conn.execute(
                "SELECT g.*, a.sort_order FROM award_grants g JOIN awards a ON a.slug = g.award_slug ORDER BY g.id"
            )
        )
        self.nominations: dict[tuple[int, int], list[sqlite3.Row]] = {}
        for n in conn.execute("SELECT * FROM nominations ORDER BY id"):
            self.nominations.setdefault((n["tournament_id"], n["round_number"]), []).append(n)
        self.meta = {r["key"]: r["value"] for r in conn.execute("SELECT key, value FROM meta")}

    # ---- refs

    def player_ref(self, pid: int | None) -> Json | None:
        if pid is None:
            return None
        p = self.players[pid]
        return {"slug": p["slug"], "name": p["name"]}

    def game_ref(self, gid: int | None) -> Json | None:
        if gid is None:
            return None
        g = self.games[gid]
        return {"slug": g["slug"], "name": g["name"]}

    @staticmethod
    def tournament_ref(t: sqlite3.Row) -> Json:
        return {"slug": t["slug"], "name": t["name"]}

    def tournament_by_id(self, tid: int) -> sqlite3.Row:
        return self.tournaments[self.tournament_rank[tid]]

    # ---- selection / rounds

    def selection(self, state: TournamentState, number: int, category: str) -> Json:
        rnd = state.rounds.get(number)
        noms = self.nominations.get((state.id, number), [])
        selected = rnd.game_id if rnd else None
        return {
            "status": "drawn" if selected is not None else "pending",
            "category": category,
            "nominations": [
                {
                    "game": self.game_ref(n["game_id"]),
                    "nominatedBy": self.player_ref(n["player_id"]),
                    "source": n["source"],
                    "entries": n["entries"],
                }
                for n in noms
            ],
            "selected": self.game_ref(selected),
        }

    def game_result(self, state: TournamentState, rnd: Round) -> Json:
        results = sorted(rnd.results, key=lambda r: (r.placement, self.names[r.player_id]))
        doc: Json = {
            "number": rnd.number,
            "category": rnd.category,
            "game": self.game_ref(rnd.game_id),
            "tournament": self.tournament_ref(state.row),
            "playedAt": rnd.played_at,
            "winner": self.player_ref(rnd.winner_id),
            "results": [
                {
                    "player": self.player_ref(r.player_id),
                    "placement": r.placement,
                    "points": r.points,
                    "basePoints": r.base_points,
                    "nemesisBonus": r.nemesis_bonus,
                    "doubleDown": r.double_down,
                    "nemesis": self.player_ref(r.nemesis_id),
                    "defeatedNemesis": r.defeated_nemesis,
                }
                for r in results
            ],
            "newChallengers": [self.player_ref(pid) for pid in rnd.new_challengers],
            "decidedTournament": rnd.decided,
        }
        if self.nominations.get((state.id, rnd.number)):
            doc["selection"] = self.selection(state, rnd.number, rnd.category)
        return doc

    # ---- tournaments

    def is_first_tournament(self, pid: int, tid: int) -> bool:
        first = min(
            (self.tournament_rank[t] for t, s in self.states.items() if pid in s.players),
            default=None,
        )
        return first == self.tournament_rank[tid]

    def standings(self, state: TournamentState) -> list[Json]:
        t = state.row
        grants_by_player: dict[int, list[str]] = {}
        for g in sorted(self.grants, key=lambda g: (g["sort_order"], g["id"])):
            if g["tournament_id"] == state.id:
                grants_by_player.setdefault(g["player_id"], []).append(g["award_slug"])
        rows = []
        for p in state.standings():
            rows.append(
                {
                    "rank": p.rank,
                    "player": self.player_ref(p.player_id),
                    "points": p.points,
                    "isChallenger": p.challenger_since is not None,
                    "challengerSinceGame": p.challenger_since,
                    "isChampion": p.player_id == state.champion_id,
                    "tokens": {
                        "doubleDown": max(t["double_down_tokens"] - p.double_downs_used, 0),
                        "nominate": max(t["nominate_tokens"] - p.nominations_used, 0),
                    },
                    "gamesPlayed": p.games_played,
                    "gamesWon": p.games_won,
                    "categoryWins": p.category_wins,
                    "awards": grants_by_player.get(p.player_id, []),
                    "isFirstTournament": self.is_first_tournament(p.player_id, state.id),
                }
            )
        return rows

    def category_progress(self, state: TournamentState) -> list[Json]:
        active = state.active_position()
        finished = state.row["status"] == "completed" or state.champion_id is not None
        entries = []
        for position, category in state.positions:
            rnd = state.rounds.get(position)
            done = bool(rnd and rnd.completed and position not in state.ignored_rounds)
            if finished and not done:
                continue  # never reached
            status = "completed" if done else "active" if active and active[0] == position else "upcoming"
            entries.append(
                {
                    "order": position,
                    "category": category,
                    "status": status,
                    "gameNumber": position if rnd else None,
                    "game": self.game_ref(rnd.game_id) if rnd else None,
                    "winner": self.player_ref(rnd.winner_id) if done and rnd else None,
                }
            )
        return entries

    def tournament_awards(self, state: TournamentState) -> list[Json]:
        return [
            {"award": g["award_slug"], "player": self.player_ref(g["player_id"]), "detail": g["detail"]}
            for g in sorted(self.grants, key=lambda g: (g["sort_order"], g["id"]))
            if g["tournament_id"] == state.id
        ]

    def tournament_summary(self, state: TournamentState) -> Json:
        t = state.row
        completed = state.completed_rounds
        return {
            "slug": t["slug"],
            "name": t["name"],
            "edition": t["edition"],
            "startDate": t["start_date"],
            "endDate": t["end_date"],
            "location": t["location"],
            "status": t["status"],
            "isDemo": bool(t["is_demo"]),
            "challengerThreshold": t["challenger_threshold"],
            "playerCount": len(state.players),
            "gamesPlayed": len(completed),
            "categoriesCompleted": len(completed),
            "categoriesPlanned": len(state.positions),
            "champion": self.player_ref(state.champion_id),
            "leaders": [
                {"rank": p.rank, "player": self.player_ref(p.player_id), "points": p.points}
                for p in state.standings()[:3]
            ],
            "awards": self.tournament_awards(state),
        }

    def live_fields(self, state: TournamentState) -> Json:
        active = state.active_position()
        if active is None:
            return {"currentCategory": None, "currentGame": None, "nextSelection": None}
        position, category = active
        rnd = state.rounds.get(position)
        return {
            "currentCategory": category,
            "currentGame": self.game_ref(rnd.game_id) if rnd else None,
            "nextSelection": self.selection(state, position, category) if state.row["status"] == "ongoing" else None,
        }

    def tournament_detail(self, state: TournamentState) -> Json:
        records = [
            {"label": r["label"], "value": r["value"], "player": self.player_ref(r["player_id"])}
            for r in self.conn.execute(
                "SELECT * FROM tournament_records WHERE tournament_id = ? ORDER BY position", (state.id,)
            )
        ]
        return {
            **self.tournament_summary(state),
            **self.live_fields(state),
            "standings": self.standings(state),
            "categories": self.category_progress(state),
            "games": [self.game_result(state, r) for r in state.completed_rounds],
            "records": records,
        }

    def featured(self) -> TournamentState | None:
        """The tournament on the homepage: ongoing, else next upcoming, else most recent completed."""
        by_status = {s: [self.states[t["id"]] for t in self.tournaments if t["status"] == s] for s in ("ongoing", "upcoming", "completed")}
        if by_status["ongoing"]:
            return by_status["ongoing"][-1]
        if by_status["upcoming"]:
            return by_status["upcoming"][0]
        if by_status["completed"]:
            return by_status["completed"][-1]
        return None

    def current_ongoing(self) -> TournamentState | None:
        ongoing = [self.states[t["id"]] for t in self.tournaments if t["status"] == "ongoing"]
        return ongoing[-1] if ongoing else None

    # ---- players

    def player_summary(self, pid: int) -> Json:
        p = self.players[pid]
        mine = [(t, self.states[t["id"]]) for t in self.tournaments if pid in self.states[t["id"]].players]
        completed = [s for t, s in mine if t["status"] == "completed"]
        current = self.current_ongoing()
        doc: Json = {
            "slug": p["slug"],
            "name": p["name"],
            "initials": p["initials"],
            "avatarUrl": p["avatar_url"],
            "firstTournament": self.tournament_ref(mine[0][0]) if mine else None,
            "tournamentsPlayed": len(mine),
            "ultimateGamerWins": sum(1 for _, s in mine if s.champion_id == pid),
            "challengerAppearances": sum(1 for _, s in mine if s.players[pid].challenger_since is not None),
            "gameWins": sum(s.players[pid].games_won for _, s in mine),
            "podiumFinishes": sum(s.players[pid].podiums for _, s in mine),
            "awardCount": sum(1 for g in self.grants if g["player_id"] == pid),
            "bestPlacement": min((s.players[pid].rank for s in completed), default=None),
            "current": None,
        }
        if current and pid in current.players:
            doc["current"] = {
                "tournament": self.tournament_ref(current.row),
                "rank": current.players[pid].rank,
                "points": current.players[pid].points,
            }
        return doc

    def player_profile(self, pid: int) -> Json:
        mine = [(t, self.states[t["id"]]) for t in reversed(self.tournaments) if pid in self.states[t["id"]].players]

        category_wins: dict[str, int] = {}
        matches: list[Json] = []
        nemesis = {"defeated": 0, "defeatedBy": 0, "timesAssigned": 0}
        rivals: dict[int, dict[str, int]] = {}
        dd = {"used": 0, "wins": 0, "bonusPoints": 0}

        for t, s in mine:
            for rnd in reversed(s.completed_rounds):
                for res in rnd.results:
                    if res.player_id == pid:
                        if res.player_id == rnd.winner_id:
                            category_wins[rnd.category] = category_wins.get(rnd.category, 0) + 1
                        if res.double_down:
                            dd["used"] += 1
                            dd["wins"] += 1 if res.placement == 1 else 0
                            dd["bonusPoints"] += res.points // 2
                        if res.nemesis_id is not None:
                            nemesis["timesAssigned"] += 1
                            rival = rivals.setdefault(res.nemesis_id, {"nemesisWins": 0, "nemesisLosses": 0})
                            if res.defeated_nemesis:
                                nemesis["defeated"] += 1
                                rival["nemesisWins"] += 1
                        matches.append(
                            {
                                "tournament": self.tournament_ref(t),
                                "gameNumber": rnd.number,
                                "category": rnd.category,
                                "game": self.game_ref(rnd.game_id),
                                "placement": res.placement,
                                "points": res.points,
                                "doubleDown": res.double_down,
                                "nemesis": self.player_ref(res.nemesis_id),
                                "defeatedNemesis": res.defeated_nemesis,
                            }
                        )
                    elif res.nemesis_id == pid and res.defeated_nemesis:
                        nemesis["defeatedBy"] += 1
                        rival = rivals.setdefault(res.player_id, {"nemesisWins": 0, "nemesisLosses": 0})
                        rival["nemesisLosses"] += 1

        token_noms = [
            n for (tid, _), noms in self.nominations.items() for n in noms if n["player_id"] == pid and n["source"] == "token"
        ]
        selected = sum(
            1
            for n in token_noms
            if (rnd := self.states[n["tournament_id"]].rounds.get(n["round_number"])) and rnd.game_id == n["game_id"]
        )

        grants = sorted(
            (g for g in self.grants if g["player_id"] == pid),
            key=lambda g: (-(self.tournament_rank.get(g["tournament_id"], -1)), g["sort_order"], g["id"]),
        )

        return {
            **self.player_summary(pid),
            "tournaments": [
                {
                    "tournament": self.tournament_ref(t),
                    "startDate": t["start_date"],
                    "status": t["status"],
                    "placement": s.players[pid].rank,
                    "points": s.players[pid].points,
                    "gamesWon": s.players[pid].games_won,
                    "wasChallenger": s.players[pid].challenger_since is not None,
                    "wasChampion": s.champion_id == pid,
                }
                for t, s in mine
            ],
            "awards": [
                {
                    "award": g["award_slug"],
                    "tournament": self.tournament_ref(self.tournament_by_id(g["tournament_id"]))
                    if g["tournament_id"] is not None
                    else None,
                    "detail": g["detail"],
                }
                for g in grants
            ],
            "categoryWins": [
                {"category": c, "wins": n}
                for c, n in sorted(category_wins.items(), key=lambda kv: (-kv[1], self.category_order.get(kv[0], 99)))
            ],
            "nemesis": nemesis if nemesis["timesAssigned"] or nemesis["defeatedBy"] else None,
            "doubleDown": dd,
            "nominate": {"used": len(token_noms), "selected": selected},
            "matches": matches,
            "rivals": [
                {"player": self.player_ref(rid), **stats}
                for rid, stats in sorted(rivals.items(), key=lambda kv: self.names[kv[0]].lower())
            ],
        }

    # ---- library

    def games_doc(self) -> Json:
        game_cats: dict[int, list[str]] = {}
        for r in self.conn.execute("SELECT game_id, category_slug FROM game_categories"):
            game_cats.setdefault(r["game_id"], []).append(r["category_slug"])
        played: dict[int, list[tuple[int, int, sqlite3.Row]]] = {}
        for t in self.tournaments:
            for rnd in self.states[t["id"]].completed_rounds:
                if rnd.game_id is not None:
                    played.setdefault(rnd.game_id, []).append((self.tournament_rank[t["id"]], rnd.number, t))
        nominated: dict[int, int] = {}
        for noms in self.nominations.values():
            for n in noms:
                nominated[n["game_id"]] = nominated.get(n["game_id"], 0) + 1

        games = []
        for gid, g in self.games.items():
            plays = sorted(played.get(gid, []), key=lambda x: (x[0], x[1]))
            last = plays[-1] if plays else None
            games.append(
                {
                    "slug": g["slug"],
                    "name": g["name"],
                    "categories": sorted(game_cats.get(gid, []), key=lambda c: self.category_order.get(c, 99)),
                    "description": g["description"],
                    "minPlayers": g["min_players"],
                    "maxPlayers": g["max_players"],
                    "timesPlayed": len(plays),
                    "timesNominated": nominated.get(gid, 0),
                    "lastPlayed": {"tournament": self.tournament_ref(last[2]), "gameNumber": last[1]} if last else None,
                    "available": bool(g["available"]),
                }
            )
        return {"games": games}

    # ---- documents

    def documents(self) -> dict[str, Json]:
        docs: dict[str, Json] = {}
        ongoing = self.current_ongoing()
        docs["site.json"] = {
            "name": self.meta.get("site_name", "The Ultimate Gamer"),
            "tagline": self.meta.get("site_tagline"),
            "currentTournament": self.tournament_ref(ongoing.row) if ongoing else None,
            "demo": any(t["is_demo"] for t in self.tournaments) or any(p["is_demo"] for p in self.players.values()),
        }

        featured = self.featured()
        if featured:
            recent = [self.game_result(featured, r) for r in reversed(featured.completed_rounds)][:RECENT_RESULTS]
            docs["leaderboard.json"] = {
                "tournament": self.tournament_summary(featured),
                **self.live_fields(featured),
                "standings": self.standings(featured),
                "categories": self.category_progress(featured),
                "recentResults": recent,
            }
        else:
            docs["leaderboard.json"] = {
                "tournament": None,
                "currentCategory": None,
                "currentGame": None,
                "nextSelection": None,
                "standings": [],
                "categories": [],
                "recentResults": [],
            }

        docs["categories.json"] = {
            "categories": [
                {"slug": c["slug"], "name": c["name"], "icon": c["icon"], "description": c["description"]}
                for c in self.categories
            ]
        }

        docs["awards.json"] = {
            "awards": [
                {
                    "slug": a["slug"],
                    "name": a["name"],
                    "icon": a["icon"],
                    "description": a["description"],
                    "scope": a["scope"],
                    "recipients": [
                        {
                            "award": g["award_slug"],
                            "player": self.player_ref(g["player_id"]),
                            "tournament": self.tournament_ref(self.tournament_by_id(g["tournament_id"]))
                            if g["tournament_id"] is not None
                            else None,
                            "detail": g["detail"],
                        }
                        for g in sorted(
                            (g for g in self.grants if g["award_slug"] == a["slug"]),
                            key=lambda g: (-(self.tournament_rank.get(g["tournament_id"], -1)), g["id"]),
                        )
                    ],
                }
                for a in self.awards
            ]
        }

        docs["games/index.json"] = self.games_doc()

        docs["players/index.json"] = {"players": [self.player_summary(pid) for pid in self.players]}
        for pid, p in self.players.items():
            docs[f"players/{p['slug']}.json"] = {"player": self.player_profile(pid)}

        newest_first = list(reversed(self.tournaments))
        docs["tournaments/index.json"] = {
            "tournaments": [self.tournament_summary(self.states[t["id"]]) for t in newest_first]
        }
        for t in self.tournaments:
            docs[f"tournaments/{t['slug']}.json"] = {"tournament": self.tournament_detail(self.states[t["id"]])}
        return docs


# ----------------------------------------------------------------------- output


def envelope(data: Json, generated_at: str) -> Json:
    return {"schemaVersion": SCHEMA_VERSION, "generatedAt": generated_at, "data": data}


def dump(document: Json) -> str:
    return json.dumps(document, ensure_ascii=False, indent=2) + "\n"


def write_documents(docs: dict[str, Json], out_dir: Path, now: str | None = None) -> dict[str, list[str]]:
    """Write documents, keeping the previous generatedAt for unchanged files and removing stale ones."""
    now = now or utc_now()
    report: dict[str, list[str]] = {"written": [], "unchanged": [], "removed": []}
    out_dir.mkdir(parents=True, exist_ok=True)

    for rel, data in sorted(docs.items()):
        target = out_dir / rel
        target.parent.mkdir(parents=True, exist_ok=True)
        generated_at = now
        if target.exists():
            try:
                previous = json.loads(target.read_text(encoding="utf-8"))
                if previous.get("schemaVersion") == SCHEMA_VERSION and previous.get("data") == data:
                    report["unchanged"].append(rel)
                    continue
            except json.JSONDecodeError:
                pass
        target.write_text(dump(envelope(data, generated_at)), encoding="utf-8")
        report["written"].append(rel)

    for existing in sorted(out_dir.rglob("*.json")):
        rel = existing.relative_to(out_dir).as_posix()
        if rel in docs:
            continue
        try:
            ours = "schemaVersion" in json.loads(existing.read_text(encoding="utf-8"))
        except (json.JSONDecodeError, TypeError):
            ours = False
        # Only remove documents this exporter generated.
        if ours:
            existing.unlink()
            report["removed"].append(rel)
    return report


def export(conn: sqlite3.Connection, out_dir: Path, now: str | None = None) -> dict[str, list[str]]:
    return write_documents(Exporter(conn).documents(), out_dir, now)
