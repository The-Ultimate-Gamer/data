//! Reference data (categories and awards) and the fictional, flagged demo dataset.

use std::collections::HashMap;

use anyhow::{Result, bail};
use rusqlite::{Connection, OptionalExtension, params};

const CATEGORIES: &[(&str, &str, &str, &str)] = &[
    ("strategy", "Strategy", "♟️", "Planning, economy and long-term decisions."),
    ("combat", "Combat", "⚔️", "Direct confrontation: aim, reflexes and nerve."),
    ("puzzle", "Puzzle", "🧩", "Logic and pattern recognition under pressure."),
    ("platform", "Platform", "🍄", "Jumps, timing and precise movement."),
    ("chaos", "Chaos", "🌀", "Unpredictable party games where anything can happen."),
    ("racing", "Racing", "🏁", "First across the line wins."),
    ("teamwork", "Teamwork", "🤝", "Temporary alliances: cooperate now, compete later."),
    ("wildcard", "Wildcard", "🃏", "Anything goes."),
    ("knowledge", "Knowledge", "🧠", "Trivia and quizzes."),
    ("tabletop", "Tabletop", "🎲", "Board and card games, digital or physical."),
    ("offline", "Offline", "🎯", "Away from the screen."),
    ("idle", "Idle", "⏳", "Patience and optimisation over a longer timeframe."),
    ("speedrun", "Speedrun", "⏱️", "Fastest completion of a set goal wins."),
    ("simulation", "Simulation", "🛠️", "Build, manage or simulate better than everyone else."),
];

const AWARDS: &[(&str, &str, &str, &str, &str)] = &[
    ("ultimate-gamer", "Ultimate Gamer", "👑", "tournament", "The tournament champion."),
    ("challenger", "Challenger", "⚔️", "tournament", "Reached the Challenger threshold during the tournament."),
    ("dominator", "Dominator", "📈", "tournament", "Highest-scoring player when the tournament ends."),
    ("contender", "Contender", "🥊", "tournament", "Highest-scoring player who never became a Challenger."),
    ("podium", "Podium", "🥉", "tournament", "Most podium finishes across games."),
    ("believer", "Believer", "🙏", "tournament", "Won a game while using a Double Down token."),
    ("nominator", "Nominator", "🗳️", "tournament", "Had a game selected from one of their Nominate tokens."),
    ("rookie", "Rookie", "🌱", "tournament", "Highest-placed player in their first tournament."),
    ("underdog", "Underdog", "🐕", "tournament", "Largest improvement between consecutive tournaments."),
    ("fallout", "Fallout", "☢️", "tournament", "Largest decline between consecutive tournaments."),
    ("legend", "Legend", "🏛️", "career", "Won at least three Ultimate Gamer tournaments."),
    ("nemesis", "Nemesis", "😈", "tournament", "Defeated their assigned Nemesis the most times."),
    ("victim", "Victim", "🎯", "tournament", "Was defeated as someone else's Nemesis the most times."),
    ("ghost", "Ghost", "👻", "tournament", "Never finished last in a game during the tournament."),
    ("survivor", "Survivor", "🩹", "tournament", "Won a game while last in the overall standings."),
];

/// Upsert the categories and awards. Safe to run again.
pub fn seed_reference(conn: &Connection) -> Result<()> {
    for (i, (slug, name, icon, description)) in CATEGORIES.iter().enumerate() {
        conn.execute(
            "INSERT INTO categories (slug, name, icon, description, sort_order) VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT (slug) DO UPDATE SET name = excluded.name, icon = excluded.icon,
             description = excluded.description, sort_order = excluded.sort_order",
            params![slug, name, icon, description, i as i64],
        )?;
    }
    for (i, (slug, name, icon, scope, description)) in AWARDS.iter().enumerate() {
        conn.execute(
            "INSERT INTO awards (slug, name, icon, scope, description, sort_order) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT (slug) DO UPDATE SET name = excluded.name, icon = excluded.icon, scope = excluded.scope,
             description = excluded.description, sort_order = excluded.sort_order",
            params![slug, name, icon, scope, description, i as i64],
        )?;
    }
    conn.execute("INSERT OR IGNORE INTO meta (key, value) VALUES ('site_name', 'The Ultimate Gamer')", [])?;
    Ok(())
}

// ------------------------------------------------------------------------- demo
// Entirely fictional. Flagged is_demo = 1 so it can be removed again in one step.

const DEMO_PLAYERS: &[&str] = &["Nova", "Blitz", "Echo", "Rook", "Vex", "Kite"];

type DemoGame = (&'static str, &'static str, &'static [&'static str], Option<i64>, Option<i64>, Option<&'static str>);

const DEMO_GAMES: &[DemoGame] = &[
    ("age-of-empires-2", "Age of Empires II", &["strategy"], Some(2), Some(8), Some("Classic real-time strategy.")),
    ("counter-strike-2", "Counter-Strike 2", &["combat", "teamwork"], Some(2), Some(10), None),
    ("tetris-99", "Tetris 99", &["puzzle"], Some(1), Some(99), None),
    ("super-mario-bros-wonder", "Super Mario Bros. Wonder", &["platform"], Some(1), Some(4), None),
    ("fall-guys", "Fall Guys", &["chaos"], Some(1), Some(32), None),
    ("mario-kart-8-deluxe", "Mario Kart 8 Deluxe", &["racing", "chaos"], Some(1), Some(12), None),
    (
        "trackmania",
        "Trackmania",
        &["racing", "speedrun"],
        Some(1),
        Some(64),
        Some("Time-attack racing on wild tracks."),
    ),
    ("rocket-league", "Rocket League", &["wildcard", "teamwork"], Some(2), Some(8), None),
    ("golf-with-your-friends", "Golf With Your Friends", &["wildcard", "chaos"], Some(1), Some(12), None),
    ("trivia-murder-party", "Trivia Murder Party", &["knowledge", "chaos"], Some(1), Some(8), None),
    ("brawlhalla", "Brawlhalla", &["combat"], Some(2), Some(8), None),
    ("catan", "Catan", &["tabletop", "strategy"], Some(3), Some(6), None),
    ("overcooked-2", "Overcooked! 2", &["teamwork", "chaos"], Some(1), Some(4), None),
    ("darts", "Darts", &["offline"], Some(2), Some(8), Some("501, double out.")),
    ("cookie-clicker", "Cookie Clicker", &["idle"], None, None, None),
    ("celeste", "Celeste", &["speedrun", "platform"], Some(1), Some(1), None),
    ("factorio", "Factorio", &["simulation", "strategy"], Some(1), Some(8), None),
    ("portal-2", "Portal 2 (co-op)", &["puzzle", "teamwork"], Some(2), Some(2), None),
];
const DEMO_UNAVAILABLE: &[&str] = &["brawlhalla"];

/// (game, finishing order, Double Down users, (player, Nemesis) pairs, played at)
type DemoRound = (
    &'static str,
    &'static [&'static str],
    &'static [&'static str],
    &'static [(&'static str, &'static str)],
    &'static str,
);

/// (game, nominating player, source)
type DemoNomination = (&'static str, Option<&'static str>, &'static str);

struct DemoTournament {
    slug: &'static str,
    name: &'static str,
    edition: &'static str,
    start_date: &'static str,
    end_date: Option<&'static str>,
    location: &'static str,
    status: &'static str,
    threshold: i64,
    players: &'static [&'static str],
    points: &'static [i64],
    categories: &'static [&'static str],
    rounds: &'static [DemoRound],
    /// (round, [(game, player, source)])
    nominations: &'static [(i64, &'static [DemoNomination])],
    pending: Option<&'static str>,
    announced_nemeses: &'static [(&'static str, &'static str)],
    grants: &'static [(&'static str, &'static str, Option<&'static str>)],
    records: &'static [(&'static str, &'static str, Option<&'static str>)],
}

const DEMO_TOURNAMENTS: &[DemoTournament] = &[
    DemoTournament {
        slug: "demo-2025",
        name: "Demo LAN 2025",
        edition: "Demo edition",
        start_date: "2025-11-07",
        end_date: Some("2025-11-08"),
        location: "The basement",
        status: "completed",
        threshold: 20,
        players: &["blitz", "echo", "nova", "rook", "vex"],
        points: &[6, 4, 3, 2, 1],
        categories: &["wildcard", "knowledge", "racing", "combat", "tabletop", "teamwork", "simulation"],
        rounds: &[
            ("golf-with-your-friends", &["blitz", "rook", "nova", "echo", "vex"], &[], &[], "2025-11-07T15:00:00Z"),
            ("trivia-murder-party", &["blitz", "echo", "nova", "vex", "rook"], &[], &[], "2025-11-07T17:00:00Z"),
            (
                "trackmania",
                &["nova", "blitz", "echo", "rook", "vex"],
                &[],
                &[("nova", "rook"), ("vex", "blitz")],
                "2025-11-07T19:30:00Z",
            ),
            ("brawlhalla", &["rook", "blitz", "nova", "echo", "vex"], &[], &[("echo", "vex")], "2025-11-07T22:00:00Z"),
            ("catan", &["echo", "blitz", "nova", "rook", "vex"], &["echo"], &[], "2025-11-08T12:00:00Z"),
            (
                "overcooked-2",
                &["echo", "blitz", "nova", "rook", "vex"],
                &["blitz"],
                &[("nova", "vex"), ("rook", "nova")],
                "2025-11-08T15:00:00Z",
            ),
        ],
        nominations: &[(
            5,
            &[("darts", Some("rook"), "winner"), ("overcooked-2", None, "vote"), ("catan", Some("rook"), "token")],
        )],
        pending: None,
        announced_nemeses: &[],
        grants: &[
            ("ultimate-gamer", "echo", Some("Won Overcooked! 2 as a Challenger")),
            ("dominator", "blitz", Some("32 points")),
            ("challenger", "blitz", None),
            ("challenger", "echo", None),
            ("challenger", "nova", None),
            ("contender", "rook", Some("17 points")),
            ("podium", "blitz", Some("6 podium finishes")),
            ("believer", "echo", None),
            ("nominator", "rook", None),
            ("nemesis", "nova", Some("2 Nemesis wins")),
            ("victim", "vex", None),
            ("ghost", "blitz", None),
        ],
        records: &[("Games played", "6", None), ("Biggest single-game haul", "12 pts", Some("echo"))],
    },
    DemoTournament {
        slug: "demo-2026",
        name: "Demo LAN 2026",
        edition: "Demo edition",
        start_date: "2026-10-16",
        end_date: None,
        location: "Somewhere with good Wi-Fi",
        status: "ongoing",
        threshold: 20,
        players: &["nova", "blitz", "echo", "rook", "vex", "kite"],
        points: &[6, 4, 3, 2, 1, 0],
        categories: &["strategy", "combat", "puzzle", "platform", "chaos", "racing", "knowledge", "wildcard"],
        rounds: &[
            (
                "age-of-empires-2",
                &["nova", "blitz", "echo", "rook", "vex", "kite"],
                &[],
                &[("nova", "rook")],
                "2026-10-16T14:00:00Z",
            ),
            (
                "counter-strike-2",
                &["blitz", "nova", "rook", "echo", "kite", "vex"],
                &["blitz"],
                &[("echo", "vex")],
                "2026-10-16T16:30:00Z",
            ),
            (
                "tetris-99",
                &["echo", "nova", "kite", "blitz", "rook", "vex"],
                &[],
                &[("echo", "nova")],
                "2026-10-16T18:45:00Z",
            ),
            (
                "super-mario-bros-wonder",
                &["nova", "rook", "vex", "echo", "blitz", "kite"],
                &["nova"],
                &[],
                "2026-10-16T20:30:00Z",
            ),
            (
                "fall-guys",
                &["vex", "blitz", "nova", "echo", "kite", "rook"],
                &[],
                &[("vex", "nova")],
                "2026-10-16T22:10:00Z",
            ),
        ],
        nominations: &[(
            6,
            &[
                ("mario-kart-8-deluxe", Some("vex"), "winner"),
                ("trackmania", None, "vote"),
                ("rocket-league", Some("kite"), "token"),
            ],
        )],
        pending: Some("mario-kart-8-deluxe"),
        // Public Nemesis pairs for the next game: one-way, and Nova is shared by two players.
        announced_nemeses: &[
            ("nova", "vex"),
            ("blitz", "echo"),
            ("echo", "nova"),
            ("rook", "nova"),
            ("vex", "kite"),
            ("kite", "blitz"),
        ],
        grants: &[
            ("challenger", "nova", None),
            ("challenger", "blitz", None),
            ("believer", "blitz", None),
            ("believer", "nova", None),
            ("survivor", "vex", Some("Won Fall Guys from last place")),
        ],
        records: &[("Biggest single-game haul", "12 pts", Some("nova"))],
    },
];

fn seed_demo(conn: &Connection) -> Result<()> {
    let mut player = HashMap::new();
    for name in DEMO_PLAYERS {
        let slug = name.to_lowercase();
        conn.execute("INSERT INTO players (slug, name, is_demo) VALUES (?1, ?2, 1)", params![slug, name])?;
        player.insert(slug, conn.last_insert_rowid());
    }
    let p = |slug: &str| player[slug];

    let mut game = HashMap::new();
    for (slug, name, cats, lo, hi, desc) in DEMO_GAMES {
        let existing: Option<i64> =
            conn.query_row("SELECT id FROM games WHERE slug = ?1", [slug], |r| r.get(0)).optional()?;
        if let Some(id) = existing {
            // Reuse a real game with the same slug; it is never removed with the demo data.
            game.insert(*slug, id);
            continue;
        }
        conn.execute(
            "INSERT INTO games (slug, name, description, min_players, max_players, available, is_demo)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 1)",
            params![slug, name, desc, lo, hi, i64::from(!DEMO_UNAVAILABLE.contains(slug))],
        )?;
        let gid = conn.last_insert_rowid();
        game.insert(*slug, gid);
        for c in *cats {
            conn.execute("INSERT INTO game_categories (game_id, category_slug) VALUES (?1, ?2)", params![gid, c])?;
        }
    }

    for t in DEMO_TOURNAMENTS {
        conn.execute(
            "INSERT INTO tournaments (slug, name, edition, start_date, end_date, location, status, challenger_threshold, is_demo)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 1)",
            params![t.slug, t.name, t.edition, t.start_date, t.end_date, t.location, t.status, t.threshold],
        )?;
        let tid = conn.last_insert_rowid();
        for slug in t.players {
            conn.execute(
                "INSERT INTO tournament_players (tournament_id, player_id) VALUES (?1, ?2)",
                params![tid, p(slug)],
            )?;
        }
        for (i, points) in t.points.iter().enumerate() {
            conn.execute(
                "INSERT INTO tournament_points (tournament_id, placement, points) VALUES (?1, ?2, ?3)",
                params![tid, i as i64 + 1, points],
            )?;
        }
        for (i, c) in t.categories.iter().enumerate() {
            conn.execute(
                "INSERT INTO tournament_categories (tournament_id, position, category_slug) VALUES (?1, ?2, ?3)",
                params![tid, i as i64 + 1, c],
            )?;
        }
        for (i, (game_slug, order, double_downs, nemeses, played_at)) in t.rounds.iter().enumerate() {
            let number = i as i64 + 1;
            conn.execute(
                "INSERT INTO rounds (tournament_id, number, game_id, played_at, completed) VALUES (?1, ?2, ?3, ?4, 1)",
                params![tid, number, game[game_slug], played_at],
            )?;
            for (placement, slug) in order.iter().enumerate() {
                let nemesis = nemeses.iter().find(|(who, _)| who == slug).map(|(_, n)| p(n));
                conn.execute(
                    "INSERT INTO results (tournament_id, round_number, player_id, placement, base_points,
                     double_down, nemesis_player_id) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                    params![
                        tid,
                        number,
                        p(slug),
                        placement as i64 + 1,
                        t.points[placement],
                        i64::from(double_downs.contains(slug)),
                        nemesis
                    ],
                )?;
            }
        }
        if let Some(pending) = t.pending {
            let number = t.rounds.len() as i64 + 1;
            conn.execute(
                "INSERT INTO rounds (tournament_id, number, game_id, completed) VALUES (?1, ?2, ?3, 0)",
                params![tid, number, game[pending]],
            )?;
            for (who, nemesis) in t.announced_nemeses {
                conn.execute(
                    "INSERT INTO nemesis_assignments (tournament_id, round_number, player_id, nemesis_player_id)
                     VALUES (?1, ?2, ?3, ?4)",
                    params![tid, number, p(who), p(nemesis)],
                )?;
            }
        }
        for (number, noms) in t.nominations {
            for (g, who, source) in *noms {
                conn.execute(
                    "INSERT INTO nominations (tournament_id, round_number, game_id, player_id, source) VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![tid, number, game[g], who.map(p), source],
                )?;
            }
        }
        for (award, who, detail) in t.grants {
            conn.execute(
                "INSERT INTO award_grants (award_slug, player_id, tournament_id, detail) VALUES (?1, ?2, ?3, ?4)",
                params![award, p(who), tid, detail],
            )?;
        }
        for (i, (label, value, who)) in t.records.iter().enumerate() {
            conn.execute(
                "INSERT INTO tournament_records (tournament_id, position, label, value, player_id) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![tid, i as i64 + 1, label, value, who.map(p)],
            )?;
        }
    }
    Ok(())
}

/// Add the flagged demo dataset next to any real data. Fails without changes on slug clashes.
pub fn add_demo(conn: &Connection) -> Result<()> {
    if crate::model::demo_present(conn)? {
        bail!("Demo data is already loaded.");
    }
    let mut clashes = Vec::new();
    for name in DEMO_PLAYERS {
        let slug = name.to_lowercase();
        if conn.query_row("SELECT 1 FROM players WHERE slug = ?1", [&slug], |_| Ok(())).optional()?.is_some() {
            clashes.push(format!("player '{slug}'"));
        }
    }
    for t in DEMO_TOURNAMENTS {
        if conn.query_row("SELECT 1 FROM tournaments WHERE slug = ?1", [t.slug], |_| Ok(())).optional()?.is_some() {
            clashes.push(format!("tournament '{}'", t.slug));
        }
    }
    if !clashes.is_empty() {
        bail!("Demo data would clash with existing {}.", clashes.join(", "));
    }
    let tx = conn.unchecked_transaction()?;
    seed_demo(&tx)?;
    tx.commit()?;
    Ok(())
}

#[derive(Debug, Default, PartialEq)]
pub struct Removed {
    pub tournaments: i64,
    pub players: i64,
    pub games: i64,
}

/// Delete every demo-flagged tournament, player and unused demo game.
pub fn remove_demo(conn: &Connection) -> Result<Removed> {
    let mixed: Option<(String, String)> = conn
        .query_row(
            "SELECT p.name, t.name FROM tournament_players tp
             JOIN players p ON p.id = tp.player_id JOIN tournaments t ON t.id = tp.tournament_id
             WHERE p.is_demo = 1 AND t.is_demo = 0 LIMIT 1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    if let Some((player, tournament)) = mixed {
        bail!("Demo player {player} is registered in the real tournament {tournament}; remove them there first.");
    }
    let tx = conn.unchecked_transaction()?;
    let count = |sql: &str| tx.query_row(sql, [], |r| r.get::<_, i64>(0));
    let tournaments = count("SELECT COUNT(*) FROM tournaments WHERE is_demo = 1")?;
    let players = count("SELECT COUNT(*) FROM players WHERE is_demo = 1")?;
    tx.execute("DELETE FROM award_grants WHERE player_id IN (SELECT id FROM players WHERE is_demo = 1)", [])?;
    tx.execute("DELETE FROM tournaments WHERE is_demo = 1", [])?;
    tx.execute("DELETE FROM players WHERE is_demo = 1", [])?;
    let games = tx.execute(
        "DELETE FROM games WHERE is_demo = 1
         AND id NOT IN (SELECT game_id FROM rounds WHERE game_id IS NOT NULL)
         AND id NOT IN (SELECT game_id FROM nominations)",
        [],
    )? as i64;
    tx.commit()?;
    Ok(Removed { tournaments, players, games })
}

/// True when the database has any player, game or tournament that isn't demo data.
pub fn has_real_data(conn: &Connection) -> Result<bool> {
    Ok(conn.query_row(
        "SELECT EXISTS (SELECT 1 FROM players WHERE is_demo = 0)
             OR EXISTS (SELECT 1 FROM games WHERE is_demo = 0)
             OR EXISTS (SELECT 1 FROM tournaments WHERE is_demo = 0)",
        [],
        |r| r.get(0),
    )?)
}

/// A database with only demo data (for practising), at `path`. Never the real database.
pub fn create_demo_db(path: &std::path::Path) -> Result<()> {
    crate::db::init(path, true)?;
    let conn = crate::db::connect(path)?;
    seed_reference(&conn)?;
    add_demo(&conn)?;
    Ok(())
}
