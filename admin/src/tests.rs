//! End-to-end tests of the rules, export, validation and demo data (in-memory databases).

use std::fs;
use std::path::Path;

use rusqlite::{Connection, params};
use serde_json::{Value, json};

use crate::db::{self, SCHEMA_VERSION};
use crate::export::{Exporter, export};
use crate::rules::TournamentState;
use crate::validate::{validate_database, validate_output};
use crate::{model, seed};

fn empty_db() -> Connection {
    db::memory().unwrap()
}

fn demo_db() -> Connection {
    let conn = empty_db();
    seed::add_demo(&conn).unwrap();
    conn
}

/// Three players (a, b, c) and four strategy positions; returns the tournament id.
fn mini_tournament(conn: &Connection, threshold: i64) -> i64 {
    for slug in ["a", "b", "c"] {
        conn.execute("INSERT INTO players (slug, name) VALUES (?1, ?2)", params![slug, slug.to_uppercase()]).unwrap();
    }
    conn.execute("INSERT INTO games (slug, name) VALUES ('g', 'Game')", []).unwrap();
    conn.execute(
        "INSERT INTO tournaments (slug, name, status, challenger_threshold) VALUES ('t', 'T', 'ongoing', ?1)",
        [threshold],
    )
    .unwrap();
    let tid = conn.last_insert_rowid();
    conn.execute("INSERT INTO tournament_players SELECT ?1, id FROM players", [tid]).unwrap();
    for pos in 1..=4 {
        conn.execute("INSERT INTO tournament_categories VALUES (?1, ?2, 'strategy')", params![tid, pos]).unwrap();
    }
    tid
}

struct Play<'a> {
    points: &'a [i64],
    double_down: &'a [&'a str],
    nemesis: &'a [(&'a str, &'a str)],
}

const PLAIN: Play = Play { points: &[6, 3, 0], double_down: &[], nemesis: &[] };

fn play(conn: &Connection, tid: i64, number: i64, order: &[&str], how: &Play) -> rusqlite::Result<()> {
    let id =
        |slug: &str| -> i64 { conn.query_row("SELECT id FROM players WHERE slug = ?1", [slug], |r| r.get(0)).unwrap() };
    let game: i64 = conn.query_row("SELECT id FROM games WHERE slug = 'g'", [], |r| r.get(0))?;
    conn.execute("INSERT INTO rounds VALUES (?1, ?2, ?3, NULL, 1)", params![tid, number, game])?;
    for (i, slug) in order.iter().enumerate() {
        let nemesis = how.nemesis.iter().find(|(p, _)| p == slug).map(|(_, n)| id(n));
        conn.execute(
            "INSERT INTO results VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![tid, number, id(slug), i as i64 + 1, how.points[i], how.double_down.contains(slug), nemesis],
        )?;
    }
    Ok(())
}

fn exporter(conn: &Connection) -> Exporter {
    Exporter::new(conn, None).unwrap()
}

fn state_of(conn: &Connection) -> TournamentState {
    exporter(conn).states.remove(&1).unwrap()
}

fn docs(conn: &Connection) -> std::collections::BTreeMap<String, Value> {
    exporter(conn).documents()
}

// --------------------------------------------------------------------- rules

#[test]
fn challenger_is_not_champion_until_winning_a_later_game() {
    let conn = empty_db();
    let tid = mini_tournament(&conn, 10);
    play(&conn, tid, 1, &["a", "b", "c"], &PLAIN).unwrap(); // a 6
    play(&conn, tid, 2, &["a", "b", "c"], &PLAIN).unwrap(); // a 12 -> Challenger in round 2, not champion
    let s = state_of(&conn);
    assert_eq!(s.players[&1].challenger_since, Some(2));
    assert_eq!(s.champion_id, None);

    play(&conn, tid, 3, &["a", "b", "c"], &PLAIN).unwrap(); // Challenger wins -> Ultimate Gamer
    let s = state_of(&conn);
    assert_eq!(s.champion_id, Some(1));
    assert_eq!(s.deciding_round, Some(3));
}

#[test]
fn other_winner_keeps_tournament_going_and_champion_may_trail_on_points() {
    let conn = empty_db();
    let tid = mini_tournament(&conn, 6);
    play(&conn, tid, 1, &["a", "b", "c"], &PLAIN).unwrap(); // a 6 -> Challenger
    play(&conn, tid, 2, &["b", "c", "a"], &Play { points: &[12, 3, 0], ..PLAIN }).unwrap(); // b 15, a doesn't win
    assert_eq!(state_of(&conn).champion_id, None);
    play(&conn, tid, 3, &["a", "c", "b"], &PLAIN).unwrap(); // a 12 wins as Challenger, b still has 15
    let s = state_of(&conn);
    assert_eq!(s.champion_id, Some(1));
    assert!(s.players[&2].points > s.players[&1].points);
}

#[test]
fn nemesis_bonus_is_doubled_by_double_down() {
    let conn = empty_db();
    let tid = mini_tournament(&conn, 10);
    play(&conn, tid, 1, &["a", "b", "c"], &Play { double_down: &["a"], nemesis: &[("a", "b"), ("c", "a")], ..PLAIN })
        .unwrap();
    let s = state_of(&conn);
    let rnd = &s.rounds[&1];
    let a = rnd.results.iter().find(|r| r.player_id == 1).unwrap();
    let c = rnd.results.iter().find(|r| r.player_id == 3).unwrap();
    assert_eq!(a.defeated_nemesis, Some(true));
    assert_eq!(a.points, (6 + 1) * 2);
    assert_eq!(c.defeated_nemesis, Some(false));
    assert_eq!(c.points, 0);
}

#[test]
fn tied_points_share_rank() {
    let conn = empty_db();
    let tid = mini_tournament(&conn, 100);
    play(&conn, tid, 1, &["a", "b", "c"], &Play { points: &[5, 5, 1], ..PLAIN }).unwrap();
    let s = state_of(&conn);
    let ranks: Vec<(i64, i64)> = (1..=3).map(|p| (p, s.players[&p].rank)).collect();
    assert_eq!(ranks, [(1, 1), (2, 1), (3, 3)]);
}

#[test]
fn tokens_remaining() {
    let conn = empty_db();
    let tid = mini_tournament(&conn, 100);
    play(&conn, tid, 1, &["a", "b", "c"], &Play { double_down: &["b"], ..PLAIN }).unwrap();
    let d = docs(&conn);
    let standings = d["tournaments/t.json"]["tournament"]["standings"].as_array().unwrap().clone();
    let tokens = |slug: &str| standings.iter().find(|s| s["player"]["slug"] == slug).unwrap()["tokens"].clone();
    assert_eq!(tokens("b"), json!({"doubleDown": 0, "nominate": 1}));
    assert_eq!(tokens("a"), json!({"doubleDown": 1, "nominate": 1}));
}

// -------------------------------------------------------------------- export

fn read(out: &Path, rel: &str) -> Value {
    serde_json::from_str(&fs::read_to_string(out.join(rel)).unwrap()).unwrap()
}

fn json_files(out: &Path) -> Vec<String> {
    crate::export::walk_files(out)
        .into_iter()
        .filter(|p| p.extension().is_some_and(|e| e == "json"))
        .map(|p| p.strip_prefix(out).unwrap().to_string_lossy().replace('\\', "/"))
        .collect()
}

#[test]
fn writes_all_documents_with_envelope() {
    let conn = demo_db();
    let tmp = tempfile::tempdir().unwrap();
    export(&conn, tmp.path(), None, Some("2026-01-01T00:00:00Z")).unwrap();
    let files = json_files(tmp.path());
    for expected in [
        "site.json",
        "leaderboard.json",
        "categories.json",
        "awards.json",
        "games/index.json",
        "players/index.json",
        "tournaments/index.json",
        "players/nova.json",
        "tournaments/demo-2025.json",
    ] {
        assert!(files.iter().any(|f| f == expected), "{expected} missing");
    }
    for rel in &files {
        let doc = read(tmp.path(), rel);
        assert_eq!(doc["schemaVersion"], SCHEMA_VERSION);
        assert_eq!(doc["generatedAt"], "2026-01-01T00:00:00Z");
        assert!(doc.get("data").is_some());
    }
}

#[test]
fn unchanged_documents_keep_generated_at() {
    let conn = demo_db();
    let tmp = tempfile::tempdir().unwrap();
    export(&conn, tmp.path(), None, Some("2026-01-01T00:00:00Z")).unwrap();
    conn.execute("UPDATE players SET name = 'Nova Prime' WHERE slug = 'nova'", []).unwrap();
    let report = export(&conn, tmp.path(), None, Some("2026-02-02T00:00:00Z")).unwrap();
    assert!(report.written.contains(&"players/nova.json".to_string()));
    assert!(report.unchanged.contains(&"categories.json".to_string()));
    assert_eq!(read(tmp.path(), "categories.json")["generatedAt"], "2026-01-01T00:00:00Z");
    assert_eq!(read(tmp.path(), "players/nova.json")["generatedAt"], "2026-02-02T00:00:00Z");
}

#[test]
fn removes_stale_documents_only() {
    let conn = demo_db();
    let tmp = tempfile::tempdir().unwrap();
    export(&conn, tmp.path(), None, None).unwrap();
    fs::write(tmp.path().join("players/ghost.json"), r#"{"schemaVersion": 1, "generatedAt": "x", "data": {}}"#)
        .unwrap();
    fs::write(tmp.path().join("notes.json"), r#"{"keep": true}"#).unwrap();
    let report = export(&conn, tmp.path(), None, None).unwrap();
    assert_eq!(report.removed, ["players/ghost.json"]);
    assert!(tmp.path().join("notes.json").exists());
}

#[test]
fn leaderboard_reflects_live_tournament() {
    let conn = demo_db();
    let lb = &docs(&conn)["leaderboard.json"];
    assert_eq!(lb["tournament"]["slug"], "demo-2026");
    assert_eq!(lb["currentCategory"], "racing");
    assert_eq!(lb["nextSelection"]["status"], "drawn");
    let ranks: Vec<i64> = lb["standings"].as_array().unwrap().iter().map(|s| s["rank"].as_i64().unwrap()).collect();
    assert_eq!(ranks, [1, 2, 3, 4, 5, 6]);
    assert!(lb["standings"].as_array().unwrap().iter().all(|s| s["isChampion"] == false));
    assert_eq!(lb["recentResults"][0]["number"], 5);
    let active: Vec<&Value> = lb["categories"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|c| c["status"] == "active")
        .map(|c| &c["category"])
        .collect();
    assert_eq!(active, [&json!("racing")]);
}

#[test]
fn completed_tournament_champion_and_unreached_categories() {
    let conn = demo_db();
    let t = docs(&conn)["tournaments/demo-2025.json"]["tournament"].clone();
    assert_eq!(t["champion"]["slug"], "echo");
    assert_eq!(t["standings"][0]["player"]["slug"], "blitz");
    assert_eq!(t["standings"][0]["isChampion"], false);
    assert!(!t["categories"].as_array().unwrap().iter().any(|c| c["category"] == "simulation"));
    assert_eq!(t["categoriesPlanned"], 7);
    assert_eq!(t["games"].as_array().unwrap().last().unwrap()["decidedTournament"], true);
}

#[test]
fn player_profile_statistics() {
    let conn = demo_db();
    let p = docs(&conn)["players/echo.json"]["player"].clone();
    assert_eq!(p["ultimateGamerWins"], 1);
    assert_eq!(p["tournamentsPlayed"], 2);
    assert_eq!(p["doubleDown"], json!({"used": 1, "wins": 1, "bonusPoints": 6}));
    assert_eq!(p["matches"][0]["tournament"]["slug"], "demo-2026");
}

#[test]
fn best_placement_counts_the_crown_as_first() {
    let conn = demo_db();
    let d = docs(&conn);
    let echo = &d["players/echo.json"]["player"];
    // Blitz led on points in Demo LAN 2025, but Echo won the crown.
    assert_eq!(echo["bestPlacement"], 1);
    assert_eq!(d["players/blitz.json"]["player"]["bestPlacement"], 2);
    let history = |slug: &str| {
        echo["tournaments"].as_array().unwrap().iter().find(|t| t["tournament"]["slug"] == slug).unwrap().clone()
    };
    assert_eq!(history("demo-2025")["finish"], 1);
    assert_eq!(history("demo-2025")["placement"], 2);
    assert_eq!(history("demo-2026")["finish"], Value::Null);
    assert!(history("demo-2025").get("podiums").is_some());
}

#[test]
fn points_table_and_category_use() {
    let conn = demo_db();
    let d = docs(&conn);
    let t = &d["tournaments/index.json"]["tournaments"][0];
    let points: Vec<i64> = t["pointsTable"].as_array().unwrap().iter().map(|p| p["points"].as_i64().unwrap()).collect();
    assert_eq!(points, [6, 4, 3, 2, 1, 0]);
    let used = |slug: &str| {
        d["categories.json"]["categories"].as_array().unwrap().iter().find(|c| c["slug"] == slug).unwrap()["tournamentsUsed"].clone()
    };
    assert_eq!(used("racing"), 2);
    assert_eq!(used("idle"), 0);
}

#[test]
fn announced_nemeses_for_the_next_game() {
    let conn = demo_db();
    let d = docs(&conn);
    let pairs = d["leaderboard.json"]["nextNemeses"].as_array().unwrap();
    assert_eq!(pairs.len(), 6);
    assert_eq!(pairs.iter().filter(|p| p["nemesis"]["slug"] == "nova").count(), 2);
    assert_eq!(d["tournaments/demo-2025.json"]["tournament"]["nextNemeses"], Value::Null);
}

#[test]
fn avatars_hosted_with_the_data() {
    let conn = demo_db();
    let (tmp, pics) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let (out, avatars) = (tmp.path(), pics.path());
    fs::write(avatars.join("nova.png"), b"png").unwrap();
    fs::write(avatars.join("nobody.png"), b"x").unwrap();
    conn.execute("UPDATE players SET avatar_url = 'https://example.com/kite.jpg' WHERE slug = 'kite'", []).unwrap();
    let report = export(&conn, out, Some(avatars), None).unwrap();
    assert!(report.written.contains(&"avatars/nova.png".to_string()));
    assert_eq!(fs::read(out.join("avatars/nova.png")).unwrap(), b"png");
    assert!(!out.join("avatars/nobody.png").exists());
    let players = read(out, "players/index.json")["data"]["players"].clone();
    let avatar =
        |slug: &str| players.as_array().unwrap().iter().find(|p| p["slug"] == slug).unwrap()["avatarUrl"].clone();
    assert_eq!(avatar("nova"), "avatars/nova.png");
    assert_eq!(avatar("kite"), "https://example.com/kite.jpg");
    let standings = read(out, "leaderboard.json")["data"]["standings"].clone();
    let nova = standings.as_array().unwrap().iter().find(|s| s["player"]["slug"] == "nova").unwrap().clone();
    assert_eq!(nova["player"]["avatarUrl"], "avatars/nova.png");

    fs::remove_file(avatars.join("nova.png")).unwrap();
    let report = export(&conn, out, Some(avatars), None).unwrap();
    assert!(report.removed.contains(&"avatars/nova.png".to_string()));
}

#[test]
fn share_pages_for_link_previews() {
    let conn = demo_db();
    conn.execute("UPDATE players SET name = 'Nova \"<b>\"' WHERE slug = 'nova'", []).unwrap();
    conn.execute("INSERT INTO meta (key, value) VALUES ('site_url', 'https://ug.example')", []).unwrap();
    let (tmp, pics) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    fs::write(pics.path().join("nova.png"), b"png").unwrap();
    let out = tmp.path();
    let report = export(&conn, out, Some(pics.path()), None).unwrap();
    assert!(report.written.contains(&"share/players/nova/index.html".to_string()));
    let page = |rel: &str| fs::read_to_string(out.join(rel)).unwrap();
    let nova = page("share/players/nova/index.html");
    assert!(nova.contains(r#"content="0; url=https://ug.example/players/nova""#));
    assert!(nova.contains(r#"<meta property="og:image" content="https://ug.example/data/avatars/nova.png">"#));
    assert!(nova.contains("Nova &quot;&lt;b&gt;&quot; · The Ultimate Gamer"));
    assert!(!nova.contains("<b>"));
    let echo = page("share/players/echo/index.html");
    assert!(echo.contains("1× Ultimate Gamer") && echo.contains("best finish 1st"));
    assert!(page("share/tournaments/demo-2025/index.html").contains("Ultimate Gamer: Echo"));
    let live = page("share/tournaments/demo-2026/index.html");
    assert!(live.contains("Live now") && live.contains("5 of 8 games played"));
}

#[test]
fn share_pages_are_removed_with_their_player() {
    let conn = demo_db();
    let tmp = tempfile::tempdir().unwrap();
    conn.execute("INSERT INTO players (slug, name) VALUES ('temp', 'Temp')", []).unwrap();
    export(&conn, tmp.path(), None, None).unwrap();
    conn.execute("DELETE FROM players WHERE slug = 'temp'", []).unwrap();
    let report = export(&conn, tmp.path(), None, None).unwrap();
    assert!(report.removed.contains(&"share/players/temp/index.html".to_string()));
    assert!(!tmp.path().join("share/players/temp").exists());
}

#[test]
fn empty_database_exports_valid_empty_documents() {
    let d = docs(&empty_db());
    assert_eq!(d["leaderboard.json"]["tournament"], Value::Null);
    assert_eq!(d["players/index.json"], json!({"players": []}));
    assert_eq!(d["categories.json"]["categories"].as_array().unwrap().len(), 14);
    assert_eq!(d["awards.json"]["awards"].as_array().unwrap().len(), 15);
}

// ------------------------------------------------------------------ validate

fn errors_of(conn: &Connection) -> Vec<String> {
    validate_database(conn, None).unwrap().errors
}

fn has(list: &[String], needle: &str) -> bool {
    list.iter().any(|e| e.contains(needle))
}

#[test]
fn demo_data_is_valid() {
    assert_eq!(errors_of(&demo_db()), Vec::<String>::new());
}

#[test]
fn two_winners_in_a_round() {
    let conn = empty_db();
    let tid = mini_tournament(&conn, 10);
    play(&conn, tid, 1, &["a", "b", "c"], &PLAIN).unwrap();
    conn.execute("UPDATE results SET placement = 1 WHERE player_id = 2", []).unwrap();
    assert!(has(&errors_of(&conn), "exactly one winner"));
}

#[test]
fn double_down_overuse() {
    let conn = empty_db();
    let tid = mini_tournament(&conn, 100);
    let dd = Play { double_down: &["a"], ..PLAIN };
    play(&conn, tid, 1, &["a", "b", "c"], &dd).unwrap();
    play(&conn, tid, 2, &["a", "b", "c"], &dd).unwrap();
    assert!(has(&errors_of(&conn), "Double Down"));
}

#[test]
fn rounds_after_decision_and_status() {
    let conn = empty_db();
    let tid = mini_tournament(&conn, 6);
    play(&conn, tid, 1, &["a", "b", "c"], &PLAIN).unwrap();
    play(&conn, tid, 2, &["a", "b", "c"], &PLAIN).unwrap(); // decided here
    play(&conn, tid, 3, &["b", "a", "c"], &PLAIN).unwrap();
    let errors = errors_of(&conn);
    assert!(has(&errors, "after the tournament was decided"));
    assert!(has(&errors, "set status to 'completed'"));
}

#[test]
fn self_nemesis_rejected_by_schema() {
    let conn = empty_db();
    let tid = mini_tournament(&conn, 10);
    assert!(play(&conn, tid, 1, &["a", "b", "c"], &Play { nemesis: &[("a", "a")], ..PLAIN }).is_err());
}

#[test]
fn selected_game_must_be_nominated() {
    let conn = empty_db();
    let tid = mini_tournament(&conn, 10);
    play(&conn, tid, 1, &["a", "b", "c"], &PLAIN).unwrap();
    conn.execute("INSERT INTO games (slug, name) VALUES ('other', 'Other')", []).unwrap();
    let other = conn.last_insert_rowid();
    conn.execute(
        "INSERT INTO nominations (tournament_id, round_number, game_id, source) VALUES (?1, 1, ?2, 'vote')",
        params![tid, other],
    )
    .unwrap();
    assert!(has(&errors_of(&conn), "not among the recorded nominations"));
}

#[test]
fn tournament_award_needs_participant() {
    let conn = empty_db();
    let tid = mini_tournament(&conn, 10);
    conn.execute("INSERT INTO players (slug, name) VALUES ('z', 'Z')", []).unwrap();
    let outsider = conn.last_insert_rowid();
    conn.execute(
        "INSERT INTO award_grants (award_slug, player_id, tournament_id) VALUES ('ghost', ?1, ?2)",
        params![outsider, tid],
    )
    .unwrap();
    assert!(has(&errors_of(&conn), "did not take part"));
}

#[test]
fn base_points_above_the_points_table_are_an_error() {
    let conn = demo_db();
    conn.execute(
        "UPDATE results SET base_points = 123231 WHERE tournament_id = 2 AND round_number = 5 AND placement = 2",
        [],
    )
    .unwrap();
    assert!(has(&errors_of(&conn), "check for a typo"));
}

#[test]
fn base_points_off_the_points_table_are_a_warning() {
    let conn = demo_db();
    conn.execute(
        "UPDATE results SET base_points = 4 WHERE tournament_id = 2 AND round_number = 5 AND placement = 3",
        [],
    )
    .unwrap();
    let report = validate_database(&conn, None).unwrap();
    assert!(report.errors.is_empty());
    assert!(has(&report.warnings, "the points table gives 3"));
}

#[test]
fn expiring_discord_avatar_is_a_warning() {
    let conn = demo_db();
    conn.execute(
        "UPDATE players SET avatar_url = 'https://cdn.discordapp.com/attachments/1/2/a.jpg?ex=6ad4da10&is=1' WHERE slug = 'kite'",
        [],
    )
    .unwrap();
    let warnings = validate_database(&conn, None).unwrap().warnings;
    assert!(warnings.iter().any(|w| w.contains("expires") && w.contains("avatars/kite.jpg")));
}

#[test]
fn nemesis_pair_needs_participants() {
    let conn = demo_db();
    conn.execute("INSERT INTO players (slug, name) VALUES ('z', 'Z')", []).unwrap();
    let outsider = conn.last_insert_rowid();
    conn.execute(
        "UPDATE nemesis_assignments SET nemesis_player_id = ?1 WHERE round_number = 6 AND player_id = 1",
        [outsider],
    )
    .unwrap();
    assert!(has(&errors_of(&conn), "Nemesis pair"));
}

#[test]
fn detects_out_of_date_json() {
    let conn = demo_db();
    let tmp = tempfile::tempdir().unwrap();
    export(&conn, tmp.path(), None, None).unwrap();
    assert!(validate_output(&conn, tmp.path(), None).unwrap().errors.is_empty());
    conn.execute("UPDATE players SET name = 'Changed' WHERE slug = 'kite'", []).unwrap();
    assert!(has(&validate_output(&conn, tmp.path(), None).unwrap().errors, "out of date"));
}

#[test]
fn detects_out_of_date_share_page() {
    let conn = demo_db();
    let tmp = tempfile::tempdir().unwrap();
    export(&conn, tmp.path(), None, None).unwrap();
    fs::write(tmp.path().join("share/players/kite/index.html"), "stale").unwrap();
    let errors = validate_output(&conn, tmp.path(), None).unwrap().errors;
    assert!(has(&errors, "share/players/kite/index.html: out of date"));
}

#[test]
fn detects_missing_avatar_copy() {
    let conn = demo_db();
    let (tmp, pics) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    export(&conn, tmp.path(), Some(pics.path()), None).unwrap();
    fs::write(pics.path().join("kite.jpg"), b"jpg").unwrap();
    let errors = validate_output(&conn, tmp.path(), Some(pics.path())).unwrap().errors;
    assert!(has(&errors, "avatars/kite.jpg: missing"));
}

// ---------------------------------------------------------------- demo data

#[test]
fn add_and_remove_demo_leaves_real_data_untouched() {
    let conn = empty_db();
    conn.execute("INSERT INTO players (slug, name) VALUES ('alex', 'Alex')", []).unwrap();
    conn.execute("INSERT INTO games (slug, name) VALUES ('catan', 'Catan (real)')", []).unwrap();

    seed::add_demo(&conn).unwrap();
    assert!(model::demo_present(&conn).unwrap());
    assert!(errors_of(&conn).is_empty());
    assert_eq!(docs(&conn)["site.json"]["demo"], true);
    // The real game with a clashing slug is reused, not duplicated or flagged.
    let catan: (String, bool) = conn
        .query_row("SELECT name, is_demo FROM games WHERE slug = 'catan'", [], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap();
    assert_eq!(catan, ("Catan (real)".to_string(), false));

    let counts = seed::remove_demo(&conn).unwrap();
    assert_eq!((counts.tournaments, counts.players), (2, 6));
    assert!(!model::demo_present(&conn).unwrap());
    let slugs: Vec<String> = model::players(&conn).unwrap().into_iter().map(|p| p.slug).collect();
    assert_eq!(slugs, ["alex"]);
    let count = |sql: &str| conn.query_row(sql, [], |r| r.get::<_, i64>(0)).unwrap();
    assert_eq!(count("SELECT COUNT(*) FROM games WHERE slug = 'catan'"), 1);
    assert_eq!(count("SELECT COUNT(*) FROM rounds"), 0);
    assert_eq!(count("SELECT COUNT(*) FROM award_grants"), 0);
    assert_eq!(docs(&conn)["site.json"]["demo"], false);
}

#[test]
fn add_demo_refuses_clashes_and_duplicates() {
    let conn = empty_db();
    conn.execute("INSERT INTO players (slug, name) VALUES ('nova', 'Nova (real)')", []).unwrap();
    assert!(seed::add_demo(&conn).unwrap_err().to_string().contains("player 'nova'"));
    conn.execute("DELETE FROM players", []).unwrap();
    seed::add_demo(&conn).unwrap();
    assert!(seed::add_demo(&conn).unwrap_err().to_string().contains("already loaded"));
}

#[test]
fn remove_demo_refuses_demo_player_in_real_tournament() {
    let conn = empty_db();
    seed::add_demo(&conn).unwrap();
    conn.execute("INSERT INTO tournaments (slug, name, challenger_threshold) VALUES ('real', 'Real', 10)", []).unwrap();
    let tid = conn.last_insert_rowid();
    conn.execute("INSERT INTO tournament_players SELECT ?1, id FROM players WHERE slug = 'nova'", [tid]).unwrap();
    assert!(seed::remove_demo(&conn).unwrap_err().to_string().contains("Nova"));
}

#[test]
fn migrate_adds_is_demo_columns_to_old_databases() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("old.sqlite");
    {
        let conn = db::connect(&path).unwrap();
        conn.execute_batch(
            "CREATE TABLE players (id INTEGER PRIMARY KEY, slug TEXT NOT NULL UNIQUE, name TEXT NOT NULL,
             initials TEXT, avatar_url TEXT);
             INSERT INTO players (slug, name) VALUES ('alex', 'Alex');",
        )
        .unwrap();
    }
    let conn = db::connect(&path).unwrap();
    db::migrate(&conn).unwrap();
    assert_eq!(conn.query_row("SELECT is_demo FROM players", [], |r| r.get::<_, i64>(0)).unwrap(), 0);
    assert!(
        conn.query_row("SELECT COUNT(*) FROM sqlite_master WHERE name = 'tournament_points'", [], |r| r
            .get::<_, i64>(0))
            .unwrap()
            == 1
    );
}

#[test]
fn discord_profile_link() {
    use crate::discord::DiscordUser;
    use crate::repo::{PlayerInput, save_discord_names, save_player};

    let conn = empty_db();
    let input = PlayerInput { name: "Ann".into(), discord_id: "not-an-id".into(), ..Default::default() };
    assert!(save_player(&conn, None, &input).is_err());
    // The database refuses a malformed id too.
    assert!(conn.execute("INSERT INTO players (slug, name, discord_id) VALUES ('x', 'X', '12ab')", []).is_err());

    let mut input = PlayerInput { name: "Ann".into(), discord_id: " 80351110224678912 ".into(), ..Default::default() };
    let id = save_player(&conn, None, &input).unwrap();
    let user = DiscordUser {
        id: "80351110224678912".into(),
        username: "ann_plays".into(),
        global_name: Some("Ann".into()),
        avatar: None,
    };
    save_discord_names(&conn, id, &user).unwrap();

    let docs = docs(&conn);
    let profile = &docs["players/ann.json"]["player"];
    assert_eq!(profile["discord"], json!({"id": "80351110224678912", "username": "ann_plays", "displayName": "Ann"}));
    assert_eq!(docs["players/index.json"]["players"][0]["discord"]["username"], "ann_plays");

    // A different account drops the names copied from the old one.
    input.discord_id = "175928847299117063".into();
    save_player(&conn, Some(id), &input).unwrap();
    assert!(save_discord_names(&conn, id, &user).is_err());
    let profile = &docs_of(&conn, "players/ann.json")["player"];
    assert_eq!(profile["discord"], json!({"id": "175928847299117063", "username": null, "displayName": null}));

    input.discord_id.clear();
    save_player(&conn, Some(id), &input).unwrap();
    assert_eq!(docs_of(&conn, "players/ann.json")["player"]["discord"], Value::Null);
}

fn docs_of(conn: &Connection, rel: &str) -> Value {
    docs(conn).remove(rel).unwrap()
}
