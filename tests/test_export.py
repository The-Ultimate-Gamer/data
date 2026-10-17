import json
import tempfile
import unittest
from pathlib import Path

from helpers import demo_db, empty_db
from ugdata import SCHEMA_VERSION, Exporter, export

EXPECTED_FILES = {
    "site.json",
    "leaderboard.json",
    "categories.json",
    "awards.json",
    "games/index.json",
    "players/index.json",
    "tournaments/index.json",
}


class ExportTest(unittest.TestCase):
    def setUp(self):
        self.conn = demo_db()
        self.tmp = tempfile.TemporaryDirectory()
        self.out = Path(self.tmp.name)

    def tearDown(self):
        self.tmp.cleanup()
        self.conn.close()

    def read(self, rel):
        return json.loads((self.out / rel).read_text(encoding="utf-8"))

    def test_writes_all_documents_with_envelope(self):
        export(self.conn, self.out, now="2026-01-01T00:00:00Z")
        files = {p.relative_to(self.out).as_posix() for p in self.out.rglob("*.json")}
        self.assertTrue(EXPECTED_FILES <= files)
        self.assertIn("players/nova.json", files)
        self.assertIn("tournaments/demo-2025.json", files)
        for rel in files:
            doc = self.read(rel)
            self.assertEqual(doc["schemaVersion"], SCHEMA_VERSION)
            self.assertEqual(doc["generatedAt"], "2026-01-01T00:00:00Z")
            self.assertIn("data", doc)

    def test_unchanged_documents_keep_generated_at(self):
        export(self.conn, self.out, now="2026-01-01T00:00:00Z")
        self.conn.execute("UPDATE players SET name = 'Nova Prime' WHERE slug = 'nova'")
        report = export(self.conn, self.out, now="2026-02-02T00:00:00Z")
        self.assertIn("players/nova.json", report["written"])
        self.assertIn("categories.json", report["unchanged"])
        self.assertEqual(self.read("categories.json")["generatedAt"], "2026-01-01T00:00:00Z")
        self.assertEqual(self.read("players/nova.json")["generatedAt"], "2026-02-02T00:00:00Z")

    def test_removes_stale_documents_only(self):
        export(self.conn, self.out)
        (self.out / "players" / "ghost.json").write_text('{"schemaVersion": 1, "generatedAt": "x", "data": {}}')
        (self.out / "notes.json").write_text('{"keep": true}')
        report = export(self.conn, self.out)
        self.assertEqual(report["removed"], ["players/ghost.json"])
        self.assertTrue((self.out / "notes.json").exists())

    def test_leaderboard_reflects_live_tournament(self):
        lb = Exporter(self.conn).documents()["leaderboard.json"]
        self.assertEqual(lb["tournament"]["slug"], "demo-2026")
        self.assertEqual(lb["currentCategory"], "racing")
        self.assertEqual(lb["nextSelection"]["status"], "drawn")
        self.assertEqual([s["rank"] for s in lb["standings"]], [1, 2, 3, 4, 5, 6])
        self.assertTrue(all(not s["isChampion"] for s in lb["standings"]))
        self.assertEqual(lb["recentResults"][0]["number"], 5)
        active = [c for c in lb["categories"] if c["status"] == "active"]
        self.assertEqual([c["category"] for c in active], ["racing"])

    def test_completed_tournament_champion_and_unreached_categories(self):
        t = Exporter(self.conn).documents()["tournaments/demo-2025.json"]["tournament"]
        self.assertEqual(t["champion"]["slug"], "echo")
        top = t["standings"][0]
        self.assertEqual(top["player"]["slug"], "blitz")
        self.assertFalse(top["isChampion"])
        self.assertNotIn("simulation", [c["category"] for c in t["categories"]])
        self.assertEqual(t["categoriesPlanned"], 7)
        self.assertTrue(t["games"][-1]["decidedTournament"])

    def test_player_profile_statistics(self):
        p = Exporter(self.conn).documents()["players/echo.json"]["player"]
        self.assertEqual(p["ultimateGamerWins"], 1)
        self.assertEqual(p["tournamentsPlayed"], 2)
        self.assertEqual(p["doubleDown"], {"used": 1, "wins": 1, "bonusPoints": 6})
        self.assertEqual(p["matches"][0]["tournament"]["slug"], "demo-2026")

    def test_empty_database_exports_valid_empty_documents(self):
        conn = empty_db()
        docs = Exporter(conn).documents()
        self.assertIsNone(docs["leaderboard.json"]["tournament"])
        self.assertEqual(docs["players/index.json"], {"players": []})
        self.assertEqual(len(docs["categories.json"]["categories"]), 14)
        self.assertEqual(len(docs["awards.json"]["awards"]), 15)


if __name__ == "__main__":
    unittest.main()
