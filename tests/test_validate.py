import tempfile
import unittest
from pathlib import Path

from helpers import demo_db, empty_db, mini_tournament, play
from ugdata import export
from validate import validate_database, validate_output


def errors_of(conn):
    return validate_database(conn).errors


class ValidateDatabaseTest(unittest.TestCase):
    def test_demo_data_is_valid(self):
        self.assertEqual(errors_of(demo_db()), [])

    def test_two_winners_in_a_round(self):
        conn = empty_db()
        tid = mini_tournament(conn)
        play(conn, tid, 1, ["a", "b", "c"])
        conn.execute("UPDATE results SET placement = 1 WHERE player_id = 2")
        self.assertTrue(any("exactly one winner" in e for e in errors_of(conn)))

    def test_double_down_overuse(self):
        conn = empty_db()
        tid = mini_tournament(conn, threshold=100)
        play(conn, tid, 1, ["a", "b", "c"], double_down=("a",))
        play(conn, tid, 2, ["a", "b", "c"], double_down=("a",))
        self.assertTrue(any("Double Down" in e for e in errors_of(conn)))

    def test_rounds_after_decision_and_status(self):
        conn = empty_db()
        tid = mini_tournament(conn, threshold=6)
        play(conn, tid, 1, ["a", "b", "c"])
        play(conn, tid, 2, ["a", "b", "c"])  # decided here
        play(conn, tid, 3, ["b", "a", "c"])
        errors = errors_of(conn)
        self.assertTrue(any("after the tournament was decided" in e for e in errors))
        self.assertTrue(any("set status to 'completed'" in e for e in errors))

    def test_self_nemesis_rejected_by_schema(self):
        conn = empty_db()
        tid = mini_tournament(conn)
        with self.assertRaises(Exception):
            play(conn, tid, 1, ["a", "b", "c"], nemesis={"a": "a"})

    def test_selected_game_must_be_nominated(self):
        conn = empty_db()
        tid = mini_tournament(conn)
        play(conn, tid, 1, ["a", "b", "c"])
        other = conn.execute("INSERT INTO games (slug, name) VALUES ('other', 'Other')").lastrowid
        conn.execute("INSERT INTO nominations (tournament_id, round_number, game_id, source) VALUES (?, 1, ?, 'vote')", (tid, other))
        self.assertTrue(any("not among the recorded nominations" in e for e in errors_of(conn)))

    def test_tournament_award_needs_participant(self):
        conn = empty_db()
        tid = mini_tournament(conn)
        outsider = conn.execute("INSERT INTO players (slug, name) VALUES ('z', 'Z')").lastrowid
        conn.execute("INSERT INTO award_grants (award_slug, player_id, tournament_id) VALUES ('ghost', ?, ?)", (outsider, tid))
        self.assertTrue(any("did not take part" in e for e in errors_of(conn)))


class ValidateOutputTest(unittest.TestCase):
    def test_detects_out_of_date_json(self):
        conn = demo_db()
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp)
            export(conn, out)
            self.assertEqual(validate_output(conn, out).errors, [])
            conn.execute("UPDATE players SET name = 'Changed' WHERE slug = 'kite'")
            errors = validate_output(conn, out).errors
            self.assertTrue(any("out of date" in e for e in errors))


if __name__ == "__main__":
    unittest.main()
