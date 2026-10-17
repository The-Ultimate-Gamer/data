import unittest

from helpers import empty_db, mini_tournament, play
from ugdata import Exporter


def state_of(conn):
    return Exporter(conn).states[1]


class RulesTest(unittest.TestCase):
    def test_challenger_is_not_champion_until_winning_a_later_game(self):
        conn = empty_db()
        tid = mini_tournament(conn, threshold=10)
        play(conn, tid, 1, ["a", "b", "c"])  # a 6
        play(conn, tid, 2, ["a", "b", "c"])  # a 12 -> Challenger in round 2, not champion
        s = state_of(conn)
        a = s.players[1]
        self.assertEqual(a.challenger_since, 2)
        self.assertIsNone(s.champion_id)

        play(conn, tid, 3, ["a", "b", "c"])  # Challenger wins -> Ultimate Gamer
        s = state_of(conn)
        self.assertEqual(s.champion_id, 1)
        self.assertEqual(s.deciding_round, 3)

    def test_other_winner_keeps_tournament_going_and_champion_may_trail_on_points(self):
        conn = empty_db()
        tid = mini_tournament(conn, threshold=6)
        play(conn, tid, 1, ["a", "b", "c"])  # a 6 -> Challenger
        play(conn, tid, 2, ["b", "c", "a"], points=(12, 3, 0))  # b 15 -> Challenger, a doesn't win
        self.assertIsNone(state_of(conn).champion_id)
        play(conn, tid, 3, ["a", "c", "b"])  # a 12 wins as Challenger, b still has 15
        s = state_of(conn)
        self.assertEqual(s.champion_id, 1)
        self.assertGreater(s.players[2].points, s.players[1].points)

    def test_nemesis_bonus_is_doubled_by_double_down(self):
        conn = empty_db()
        tid = mini_tournament(conn)
        play(conn, tid, 1, ["a", "b", "c"], double_down=("a",), nemesis={"a": "b", "c": "a"})
        rnd = state_of(conn).rounds[1]
        a = next(r for r in rnd.results if r.player_id == 1)
        c = next(r for r in rnd.results if r.player_id == 3)
        self.assertTrue(a.defeated_nemesis)
        self.assertEqual(a.points, (6 + 1) * 2)
        self.assertFalse(c.defeated_nemesis)
        self.assertEqual(c.points, 0)

    def test_tied_points_share_rank(self):
        conn = empty_db()
        tid = mini_tournament(conn, threshold=100)
        play(conn, tid, 1, ["a", "b", "c"], points=(5, 5, 1))
        ranks = {p.player_id: p.rank for p in state_of(conn).players.values()}
        self.assertEqual(ranks, {1: 1, 2: 1, 3: 3})

    def test_tokens_remaining(self):
        conn = empty_db()
        tid = mini_tournament(conn, threshold=100)
        play(conn, tid, 1, ["a", "b", "c"], double_down=("b",))
        standings = {s["player"]["slug"]: s["tokens"] for s in Exporter(conn).standings(state_of(conn))}
        self.assertEqual(standings["b"], {"doubleDown": 0, "nominate": 1})
        self.assertEqual(standings["a"], {"doubleDown": 1, "nominate": 1})


if __name__ == "__main__":
    unittest.main()
