//! Reference data (categories and awards) and the flagged demo dataset: two demo tournaments
//! played out at random (seeded, so always the same) by the real roster, or by fictional players.

use std::collections::HashMap;

use anyhow::{Result, bail};
use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use rand::{Rng, SeedableRng};
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
// Fictional results, flagged is_demo = 1 so they can be removed again in one step.

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

/// A demo tournament; its games are played out by a seeded random generator.
struct DemoTournament {
    /// Tags; the name ("Autumn LAN 2025") and address are made from them and the date.
    tags: &'static [&'static str],
    start: &'static str,
    end_date: Option<&'static str>,
    location: &'static str,
    threshold: i64,
    /// Kept short, so the games loop through the categories (lap 2) before anyone wins.
    categories: &'static [&'static str],
    /// None: play until a Challenger wins in the second lap or later (completed). Some(n): play
    /// n games (more than one lap), then leave the next one drawn but unplayed (live).
    games_played: Option<usize>,
    /// Leave the newest player out, so the next tournament has a first-timer.
    without_newest: bool,
}

const DEMO_TOURNAMENTS: &[DemoTournament] = &[
    DemoTournament {
        tags: &["lan", "physical", "overnight"],
        start: "2025-11-07",
        end_date: Some("2025-11-08"),
        location: "The basement",
        threshold: 20,
        categories: &["wildcard", "knowledge", "racing", "combat", "tabletop"],
        games_played: None,
        without_newest: true,
    },
    DemoTournament {
        tags: &["lan", "digital", "weekend"],
        start: "2026-10-16",
        end_date: None,
        location: "Somewhere with good Wi-Fi",
        threshold: 30,
        categories: &["strategy", "combat", "chaos", "racing"],
        games_played: Some(6),
        without_newest: false,
    },
];

const DEMO_POINTS: &[i64] = &[6, 4, 3, 2, 1];

/// The players the demo uses: the real roster when it has at least four players (so the demo
/// shows the people who will actually play), else fictional ones flagged as demo data.
fn demo_roster(conn: &Connection) -> Result<(Vec<i64>, bool)> {
    let mut stmt = conn.prepare("SELECT id FROM players WHERE is_demo = 0 ORDER BY id")?;
    let real: Vec<i64> = stmt.query_map([], |r| r.get(0))?.collect::<Result<_, _>>()?;
    Ok((real.clone(), real.len() >= 4))
}

fn demo_games(conn: &Connection) -> Result<HashMap<&'static str, i64>> {
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
    Ok(game)
}

/// Games of a category, falling back to the whole demo library.
fn games_for(category: &str, game: &HashMap<&'static str, i64>) -> Vec<i64> {
    let mut list: Vec<i64> = DEMO_GAMES
        .iter()
        .filter(|(slug, _, cats, ..)| cats.contains(&category) && !DEMO_UNAVAILABLE.contains(slug))
        .map(|(slug, ..)| game[slug])
        .collect();
    if list.is_empty() {
        list = DEMO_GAMES.iter().filter(|(slug, ..)| !DEMO_UNAVAILABLE.contains(slug)).map(|(s, ..)| game[s]).collect();
    }
    list
}

/// Nominations for a round: the previous winner's pick, a vote, and now and then a Nominate token.
#[allow(clippy::too_many_arguments)]
fn demo_nominations(
    tx: &Connection,
    rng: &mut StdRng,
    tid: i64,
    number: i64,
    category: &str,
    game: &HashMap<&'static str, i64>,
    players: &[i64],
    previous_winner: Option<i64>,
    tokens_used: &mut HashMap<i64, i64>,
) -> Result<()> {
    let mut options = games_for(category, game);
    options.shuffle(rng);
    let mut picks: Vec<(i64, &str, Option<i64>)> = Vec::new();
    if let Some(w) = previous_winner {
        picks.push((options[0], "winner", Some(w)));
    }
    picks.push((options[picks.len() % options.len()], "vote", None));
    if rng.random_bool(0.35) {
        let p = players[rng.random_range(0..players.len())];
        if tokens_used.get(&p).copied().unwrap_or(0) == 0 {
            *tokens_used.entry(p).or_default() += 1;
            picks.push((options[picks.len() % options.len()], "token", Some(p)));
        }
    }
    picks.dedup_by_key(|(g, _, _)| *g);
    for (g, source, who) in picks {
        tx.execute(
            "INSERT INTO nominations (tournament_id, round_number, game_id, player_id, source) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![tid, number, g, who, source],
        )?;
    }
    Ok(())
}

/// Play one demo tournament with this seed. Returns false (nothing kept) if it didn't turn out
/// as wanted: the completed one must crown a champion, the live one must still be open.
fn play_demo(
    conn: &Connection,
    t: &DemoTournament,
    players: &[i64],
    game: &HashMap<&'static str, i64>,
    seed: u64,
) -> Result<bool> {
    let mut rng = StdRng::seed_from_u64(seed);
    let tx = crate::db::begin(conn)?;
    let status = if t.games_played.is_some() { "ongoing" } else { "completed" };
    tx.execute(
        "INSERT INTO tournaments (slug, name, start_date, end_date, location, status, challenger_threshold, is_demo)
         VALUES ('demo', 'Demo', ?1, ?2, ?3, ?4, ?5, 1)",
        params![t.start, t.end_date, t.location, status, t.threshold],
    )?;
    let tid = tx.last_insert_rowid();
    for tag in t.tags {
        tx.execute("INSERT INTO tournament_tags (tournament_id, tag) VALUES (?1, ?2)", params![tid, tag])?;
    }
    crate::repo::sync_tournament_identity(&tx, tid)?;
    for p in players {
        tx.execute("INSERT INTO tournament_players (tournament_id, player_id) VALUES (?1, ?2)", params![tid, p])?;
    }
    for placement in 1..=DEMO_POINTS.len() {
        let points = DEMO_POINTS[placement - 1];
        tx.execute(
            "INSERT INTO tournament_points (tournament_id, placement, points) VALUES (?1, ?2, ?3)",
            params![tid, placement as i64, points],
        )?;
    }
    for (i, c) in t.categories.iter().enumerate() {
        tx.execute(
            "INSERT INTO tournament_categories (tournament_id, position, category_slug) VALUES (?1, ?2, ?3)",
            params![tid, i as i64 + 1, c],
        )?;
    }
    let row = crate::model::tournament(&tx, tid)?.expect("just inserted");
    let names = HashMap::new();
    let mut dd_used: HashMap<i64, i64> = HashMap::new();
    let mut tokens_used: HashMap<i64, i64> = HashMap::new();
    let mut previous_winner = None;
    let start = time::Date::parse(t.start, time::macros::format_description!("[year]-[month]-[day]"))?;

    // Games loop through the categories until someone is crowned (at most three passes here).
    for i in 0..t.categories.len() * 3 {
        let category = t.categories[i % t.categories.len()];
        let number = i as i64 + 1;
        let live_next = t.games_played == Some(i);
        tx.execute("INSERT INTO rounds (tournament_id, number, completed) VALUES (?1, ?2, 0)", params![tid, number])?;
        demo_nominations(&tx, &mut rng, tid, number, category, game, players, previous_winner, &mut tokens_used)?;
        crate::repo::draw_game_with(&tx, tid, number, &mut rng)?;
        crate::repo::draw_nemeses_with(&tx, tid, number, &mut rng)?;
        if live_next {
            break;
        }

        let mut order = players.to_vec();
        order.shuffle(&mut rng);
        let nemesis: HashMap<i64, i64> = crate::model::nemesis_assignments(&tx, tid, number)?.into_iter().collect();
        for (place, p) in order.iter().enumerate() {
            let double_down = dd_used.get(p).copied().unwrap_or(0) == 0 && rng.random_bool(0.18);
            if double_down {
                dd_used.insert(*p, 1);
            }
            let points = DEMO_POINTS.get(place).copied().unwrap_or(0);
            tx.execute(
                "INSERT INTO results (tournament_id, round_number, player_id, placement, base_points, double_down,
                 nemesis_player_id) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![tid, number, p, place as i64 + 1, points, double_down, nemesis.get(p)],
            )?;
        }
        let played_at = format!("{}T{:02}:00:00Z", start + time::Duration::days(i as i64 / 4), 14 + (i % 4) * 2);
        tx.execute(
            "UPDATE rounds SET completed = 1, played_at = ?1 WHERE tournament_id = ?2 AND number = ?3",
            params![played_at, tid, number],
        )?;
        previous_winner = Some(order[0]);
        let state = crate::rules::compute_tournament(&tx, &row, &names)?;
        if state.champion_id.is_some() {
            // The live demo must still be open, and the completed one decided after a full lap.
            if t.games_played.is_some() || i < t.categories.len() {
                return Ok(false);
            }
            tx.commit()?;
            return Ok(true);
        }
    }
    if t.games_played.is_none() {
        return Ok(false); // no champion: try another seed
    }
    tx.commit()?;
    Ok(true)
}

fn seed_demo(conn: &Connection) -> Result<()> {
    let (real, use_real) = demo_roster(conn)?;
    let roster = if use_real {
        real
    } else {
        let mut ids = Vec::new();
        for name in DEMO_PLAYERS {
            conn.execute(
                "INSERT INTO players (slug, name, is_demo) VALUES (?1, ?2, 1)",
                params![name.to_lowercase(), name],
            )?;
            ids.push(conn.last_insert_rowid());
        }
        ids
    };
    let game = demo_games(conn)?;
    for t in DEMO_TOURNAMENTS {
        let players = if t.without_newest { &roster[..roster.len() - 1] } else { &roster[..] };
        let played = (1..500u64)
            .find_map(|seed| play_demo(conn, t, players, &game, seed).map(|ok| ok.then_some(())).transpose());
        match played {
            Some(result) => result?,
            None => bail!("couldn't play out the demo tournament starting {}", t.start),
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
    if !demo_roster(conn)?.1 {
        for name in DEMO_PLAYERS {
            let slug = name.to_lowercase();
            if conn.query_row("SELECT 1 FROM players WHERE slug = ?1", [&slug], |_| Ok(())).optional()?.is_some() {
                clashes.push(format!("player '{slug}'"));
            }
        }
    }
    if !clashes.is_empty() {
        bail!("Demo data would clash with existing {}.", clashes.join(", "));
    }
    let tx = crate::db::begin(conn)?;
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
    let tx = crate::db::begin(conn)?;
    let count = |sql: &str| tx.query_row(sql, [], |r| r.get::<_, i64>(0));
    let tournaments = count("SELECT COUNT(*) FROM tournaments WHERE is_demo = 1")?;
    let players = count("SELECT COUNT(*) FROM players WHERE is_demo = 1")?;
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
