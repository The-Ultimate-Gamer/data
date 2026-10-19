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

    def test_base_points_above_the_points_table_are_an_error(self):
        conn = demo_db()
        conn.execute("UPDATE results SET base_points = 123231 WHERE tournament_id = 2 AND round_number = 5 AND placement = 2")
        self.assertTrue(any("check for a typo" in e for e in errors_of(conn)))

    def test_base_points_off_the_points_table_are_a_warning(self):
        conn = demo_db()
        conn.execute("UPDATE results SET base_points = 4 WHERE tournament_id = 2 AND round_number = 5 AND placement = 3")
        report = validate_database(conn)
        self.assertEqual(report.errors, [])
        self.assertTrue(any("the points table gives 3" in w for w in report.warnings))

    def test_expiring_discord_avatar_is_a_warning(self):
        conn = demo_db()
        conn.execute(
            "UPDATE players SET avatar_url = 'https://cdn.discordapp.com/attachments/1/2/a.jpg?ex=6ad4da10&is=1' WHERE slug = 'kite'"
        )
        warnings = validate_database(conn, avatars_dir=None).warnings
        self.assertTrue(any("expires" in w and "avatars/kite.jpg" in w for w in warnings))

    def test_nemesis_pair_needs_participants(self):
        conn = demo_db()
        outsider = conn.execute("INSERT INTO players (slug, name) VALUES ('z', 'Z')").lastrowid
        conn.execute("UPDATE nemesis_assignments SET nemesis_player_id = ? WHERE round_number = 6 AND player_id = 1", (outsider,))
        self.assertTrue(any("Nemesis pair" in e for e in errors_of(conn)))


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

    def test_detects_missing_avatar_copy(self):
        conn = demo_db()
        with tempfile.TemporaryDirectory() as tmp, tempfile.TemporaryDirectory() as pics:
            out, avatars = Path(tmp), Path(pics)
            export(conn, out, avatars_dir=avatars)
            (avatars / "kite.jpg").write_bytes(b"jpg")
            errors = validate_output(conn, out, avatars_dir=avatars).errors
            self.assertTrue(any("avatars/kite.jpg: missing" in e for e in errors))


if __name__ == "__main__":
    unittest.main()
