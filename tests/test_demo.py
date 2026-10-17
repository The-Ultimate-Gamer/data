import tempfile
import unittest
from pathlib import Path

from helpers import empty_db
from migrate import migrate
from seed import add_demo, demo_present, remove_demo
from ugdata import Exporter, connect
from validate import validate_database


class DemoDataTest(unittest.TestCase):
    def test_add_and_remove_leaves_real_data_untouched(self):
        conn = empty_db()
        conn.execute("INSERT INTO players (slug, name) VALUES ('alex', 'Alex')")
        conn.execute("INSERT INTO games (slug, name) VALUES ('catan', 'Catan (real)')")

        add_demo(conn)
        self.assertTrue(demo_present(conn))
        self.assertEqual(validate_database(conn).errors, [])
        site = Exporter(conn).documents()["site.json"]
        self.assertTrue(site["demo"])
        # The real game with a clashing slug is reused, not duplicated or flagged.
        self.assertEqual(conn.execute("SELECT name, is_demo FROM games WHERE slug = 'catan'").fetchone()[:], ("Catan (real)", 0))

        counts = remove_demo(conn)
        self.assertEqual(counts["tournaments"], 2)
        self.assertEqual(counts["players"], 6)
        self.assertFalse(demo_present(conn))
        self.assertEqual([r[0] for r in conn.execute("SELECT slug FROM players")], ["alex"])
        self.assertIsNotNone(conn.execute("SELECT 1 FROM games WHERE slug = 'catan'").fetchone())
        self.assertEqual(conn.execute("SELECT COUNT(*) FROM rounds").fetchone()[0], 0)
        self.assertEqual(conn.execute("SELECT COUNT(*) FROM award_grants").fetchone()[0], 0)
        self.assertFalse(Exporter(conn).documents()["site.json"]["demo"])

    def test_add_refuses_clashes_and_duplicates(self):
        conn = empty_db()
        conn.execute("INSERT INTO players (slug, name) VALUES ('nova', 'Nova (real)')")
        with self.assertRaisesRegex(ValueError, "player 'nova'"):
            add_demo(conn)
        conn.execute("DELETE FROM players")
        add_demo(conn)
        with self.assertRaisesRegex(ValueError, "already loaded"):
            add_demo(conn)

    def test_remove_refuses_demo_player_in_real_tournament(self):
        conn = empty_db()
        add_demo(conn)
        tid = conn.execute(
            "INSERT INTO tournaments (slug, name, challenger_threshold) VALUES ('real', 'Real', 10)"
        ).lastrowid
        conn.execute("INSERT INTO tournament_players SELECT ?, id FROM players WHERE slug = 'nova'", (tid,))
        with self.assertRaisesRegex(ValueError, "Nova"):
            remove_demo(conn)


class MigrateTest(unittest.TestCase):
    def test_adds_is_demo_columns_to_old_databases(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "old.sqlite"
            conn = connect(path)
            conn.executescript(
                """CREATE TABLE players (id INTEGER PRIMARY KEY, slug TEXT NOT NULL UNIQUE, name TEXT NOT NULL,
                   initials TEXT, avatar_url TEXT);
                   INSERT INTO players (slug, name) VALUES ('alex', 'Alex');"""
            )
            conn.close()
            migrate(path)
            conn = connect(path)
            self.assertEqual(conn.execute("SELECT is_demo FROM players").fetchone()[0], 0)
            self.assertIsNotNone(conn.execute("SELECT 1 FROM sqlite_master WHERE name = 'tournament_points'").fetchone())
            conn.close()


if __name__ == "__main__":
    unittest.main()
