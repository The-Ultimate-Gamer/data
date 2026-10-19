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

    def test_best_placement_counts_the_crown_as_first(self):
        docs = Exporter(self.conn).documents()
        echo = docs["players/echo.json"]["player"]
        blitz = docs["players/blitz.json"]["player"]
        # Blitz led on points in Demo LAN 2025, but Echo won the crown.
        self.assertEqual(echo["bestPlacement"], 1)
        self.assertEqual(blitz["bestPlacement"], 2)
        history = {t["tournament"]["slug"]: t for t in echo["tournaments"]}
        self.assertEqual(history["demo-2025"]["finish"], 1)
        self.assertEqual(history["demo-2025"]["placement"], 2)
        self.assertIsNone(history["demo-2026"]["finish"])
        self.assertIn("podiums", history["demo-2025"])

    def test_points_table_and_category_use(self):
        docs = Exporter(self.conn).documents()
        t = docs["tournaments/index.json"]["tournaments"][0]
        self.assertEqual([p["points"] for p in t["pointsTable"]], [6, 4, 3, 2, 1, 0])
        used = {c["slug"]: c["tournamentsUsed"] for c in docs["categories.json"]["categories"]}
        self.assertEqual(used["racing"], 2)
        self.assertEqual(used["idle"], 0)

    def test_announced_nemeses_for_the_next_game(self):
        docs = Exporter(self.conn).documents()
        pairs = docs["leaderboard.json"]["nextNemeses"]
        self.assertEqual(len(pairs), 6)
        targets = [p["nemesis"]["slug"] for p in pairs]
        self.assertEqual(targets.count("nova"), 2)
        self.assertIsNone(docs["tournaments/demo-2025.json"]["tournament"]["nextNemeses"])

    def test_avatars_hosted_with_the_data(self):
        with tempfile.TemporaryDirectory() as pics:
            avatars = Path(pics)
            (avatars / "nova.png").write_bytes(b"png")
            (avatars / "nobody.png").write_bytes(b"x")
            self.conn.execute("UPDATE players SET avatar_url = 'https://example.com/kite.jpg' WHERE slug = 'kite'")
            report = export(self.conn, self.out, avatars_dir=avatars)
            self.assertIn("avatars/nova.png", report["written"])
            self.assertEqual((self.out / "avatars" / "nova.png").read_bytes(), b"png")
            self.assertFalse((self.out / "avatars" / "nobody.png").exists())
            players = {p["slug"]: p for p in self.read("players/index.json")["data"]["players"]}
            self.assertEqual(players["nova"]["avatarUrl"], "avatars/nova.png")
            self.assertEqual(players["kite"]["avatarUrl"], "https://example.com/kite.jpg")
            standings = self.read("leaderboard.json")["data"]["standings"]
            nova = next(s for s in standings if s["player"]["slug"] == "nova")
            self.assertEqual(nova["player"]["avatarUrl"], "avatars/nova.png")

            (avatars / "nova.png").unlink()
            report = export(self.conn, self.out, avatars_dir=avatars)
            self.assertIn("avatars/nova.png", report["removed"])

    def test_share_pages_for_link_previews(self):
        self.conn.execute("UPDATE players SET name = 'Nova \"<b>\"' WHERE slug = 'nova'")
        self.conn.execute("INSERT INTO meta (key, value) VALUES ('site_url', 'https://ug.example')")
        with tempfile.TemporaryDirectory() as pics:
            (Path(pics) / "nova.png").write_bytes(b"png")
            report = export(self.conn, self.out, avatars_dir=Path(pics))
        self.assertIn("share/players/nova/index.html", report["written"])
        nova = (self.out / "share/players/nova/index.html").read_text(encoding="utf-8")
        self.assertIn('content="0; url=https://ug.example/players/nova"', nova)
        self.assertIn('<meta property="og:image" content="https://ug.example/data/avatars/nova.png">', nova)
        self.assertIn("Nova &quot;&lt;b&gt;&quot; · The Ultimate Gamer", nova)
        self.assertNotIn("<b>", nova)
        echo = (self.out / "share/players/echo/index.html").read_text(encoding="utf-8")
        self.assertIn("1× Ultimate Gamer", echo)
        self.assertIn("best finish 1st", echo)
        done = (self.out / "share/tournaments/demo-2025/index.html").read_text(encoding="utf-8")
        self.assertIn("Ultimate Gamer: Echo", done)
        live = (self.out / "share/tournaments/demo-2026/index.html").read_text(encoding="utf-8")
        self.assertIn("Live now", live)
        self.assertIn("5 of 8 games played", live)

    def test_share_pages_are_removed_with_their_player(self):
        self.conn.execute("INSERT INTO players (slug, name) VALUES ('temp', 'Temp')")
        export(self.conn, self.out)
        self.conn.execute("DELETE FROM players WHERE slug = 'temp'")
        report = export(self.conn, self.out)
        self.assertIn("share/players/temp/index.html", report["removed"])
        self.assertFalse((self.out / "share/players/temp").exists())

    def test_empty_database_exports_valid_empty_documents(self):
        conn = empty_db()
        docs = Exporter(conn).documents()
        self.assertIsNone(docs["leaderboard.json"]["tournament"])
        self.assertEqual(docs["players/index.json"], {"players": []})
        self.assertEqual(len(docs["categories.json"]["categories"]), 14)
        self.assertEqual(len(docs["awards.json"]["awards"]), 15)


if __name__ == "__main__":
    unittest.main()
