//! Every change the admin makes to the database. Facts only: derived values come from the exporter.

use anyhow::Result;
use rusqlite::{Connection, OptionalExtension, params};

use crate::db::{is_slug, slugify, today, utc_now};
use crate::model::{self, Source, Status};

/// A problem the admin can fix (shown as a message, never a crash).
#[derive(Debug)]
pub struct UserError(pub String);

impl std::fmt::Display for UserError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for UserError {}

macro_rules! user_bail {
    ($($arg:tt)*) => { return Err(UserError(format!($($arg)*)).into()) };
}

fn exists(conn: &Connection, sql: &str, p: impl rusqlite::Params) -> Result<bool> {
    Ok(conn.query_row(sql, p, |_| Ok(())).optional()?.is_some())
}

fn count(conn: &Connection, sql: &str, p: impl rusqlite::Params) -> Result<i64> {
    Ok(conn.query_row(sql, p, |r| r.get(0))?)
}

fn clean(value: &str) -> Option<String> {
    let v = value.trim();
    (!v.is_empty()).then(|| v.to_string())
}

fn required(value: &str, what: &str) -> Result<String> {
    match clean(value) {
        Some(v) => Ok(v),
        None => user_bail!("{what} is required."),
    }
}

/// The slug to use: the given one, else one made from the name.
fn pick_slug(slug: &str, name: &str) -> Result<String> {
    let slug = clean(slug).unwrap_or_else(|| slugify(name));
    if !is_slug(&slug) {
        user_bail!("\"{slug}\" is not a valid web address (use lowercase letters, digits and dashes).");
    }
    Ok(slug)
}

#[derive(Clone, Copy)]
enum Table {
    Players,
    Games,
    Tournaments,
}

impl Table {
    fn name(self) -> &'static str {
        match self {
            Table::Players => "players",
            Table::Games => "games",
            Table::Tournaments => "tournaments",
        }
    }

    fn noun(self) -> &'static str {
        match self {
            Table::Players => "player",
            Table::Games => "game",
            Table::Tournaments => "tournament",
        }
    }
}

fn ensure_unique_slug(conn: &Connection, table: Table, slug: &str, id: Option<i64>) -> Result<()> {
    let taken =
        exists(conn, &format!("SELECT 1 FROM {} WHERE slug = ?1 AND id IS NOT ?2", table.name()), params![slug, id])?;
    if taken {
        user_bail!("Another {} already uses the web address \"{slug}\".", table.noun());
    }
    Ok(())
}

// ------------------------------------------------------------------ players

#[derive(Clone, Debug, Default, PartialEq)]
pub struct PlayerInput {
    pub name: String,
    pub slug: String,
    pub initials: String,
    pub avatar_url: String,
}

impl PlayerInput {
    pub fn from(p: &model::Player) -> Self {
        Self {
            name: p.name.clone(),
            slug: p.slug.clone(),
            initials: p.initials.clone().unwrap_or_default(),
            avatar_url: p.avatar_url.clone().unwrap_or_default(),
        }
    }
}

fn https(value: &str) -> Result<Option<String>> {
    let v = clean(value);
    if let Some(url) = &v
        && !url.to_lowercase().starts_with("https://")
    {
        user_bail!("The picture link must start with https://");
    }
    Ok(v)
}

/// Create (id = None) or update a player. Returns the player's id.
pub fn save_player(conn: &Connection, id: Option<i64>, p: &PlayerInput) -> Result<i64> {
    let name = required(&p.name, "Name")?;
    let slug = pick_slug(&p.slug, &name)?;
    ensure_unique_slug(conn, Table::Players, &slug, id)?;
    let avatar = https(&p.avatar_url)?;
    let initials = clean(&p.initials);
    match id {
        None => {
            conn.execute(
                "INSERT INTO players (slug, name, initials, avatar_url) VALUES (?1, ?2, ?3, ?4)",
                params![slug, name, initials, avatar],
            )?;
            Ok(conn.last_insert_rowid())
        }
        Some(id) => {
            conn.execute(
                "UPDATE players SET slug = ?1, name = ?2, initials = ?3, avatar_url = ?4 WHERE id = ?5",
                params![slug, name, initials, avatar, id],
            )?;
            Ok(id)
        }
    }
}

pub fn delete_player(conn: &Connection, id: i64) -> Result<()> {
    let used = count(
        conn,
        "SELECT (SELECT COUNT(*) FROM tournament_players WHERE player_id = ?1) +
                (SELECT COUNT(*) FROM award_grants WHERE player_id = ?1)",
        [id],
    )?;
    if used > 0 {
        user_bail!("This player has tournament history and can't be deleted.");
    }
    conn.execute("DELETE FROM players WHERE id = ?1", [id])?;
    Ok(())
}

// -------------------------------------------------------------------- games

#[derive(Clone, Debug, Default, PartialEq)]
pub struct GameInput {
    pub name: String,
    pub slug: String,
    pub description: String,
    pub min_players: String,
    pub max_players: String,
    pub available: bool,
    pub categories: Vec<String>,
}

impl GameInput {
    pub fn new() -> Self {
        Self { available: true, ..Default::default() }
    }

    pub fn from(g: &model::Game) -> Self {
        Self {
            name: g.name.clone(),
            slug: g.slug.clone(),
            description: g.description.clone().unwrap_or_default(),
            min_players: g.min_players.map(|n| n.to_string()).unwrap_or_default(),
            max_players: g.max_players.map(|n| n.to_string()).unwrap_or_default(),
            available: g.available,
            categories: g.categories.clone(),
        }
    }
}

fn opt_int(value: &str, what: &str, min: i64) -> Result<Option<i64>> {
    let Some(v) = clean(value) else { return Ok(None) };
    match v.parse::<i64>() {
        Ok(n) if n >= min => Ok(Some(n)),
        _ => user_bail!("{what} must be a whole number of at least {min}."),
    }
}

pub fn save_game(conn: &Connection, id: Option<i64>, g: &GameInput) -> Result<i64> {
    let name = required(&g.name, "Name")?;
    let slug = pick_slug(&g.slug, &name)?;
    ensure_unique_slug(conn, Table::Games, &slug, id)?;
    let (lo, hi) = (opt_int(&g.min_players, "Minimum players", 1)?, opt_int(&g.max_players, "Maximum players", 1)?);
    if let (Some(lo), Some(hi)) = (lo, hi)
        && lo > hi
    {
        user_bail!("Minimum players can't be more than maximum players.");
    }
    let tx = conn.unchecked_transaction()?;
    let gid = match id {
        None => {
            tx.execute(
                "INSERT INTO games (slug, name, description, min_players, max_players, available) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![slug, name, clean(&g.description), lo, hi, g.available],
            )?;
            tx.last_insert_rowid()
        }
        Some(id) => {
            tx.execute(
                "UPDATE games SET slug = ?1, name = ?2, description = ?3, min_players = ?4, max_players = ?5, available = ?6 WHERE id = ?7",
                params![slug, name, clean(&g.description), lo, hi, g.available, id],
            )?;
            tx.execute("DELETE FROM game_categories WHERE game_id = ?1", [id])?;
            id
        }
    };
    for c in &g.categories {
        tx.execute("INSERT INTO game_categories (game_id, category_slug) VALUES (?1, ?2)", params![gid, c])?;
    }
    tx.commit()?;
    Ok(gid)
}

pub fn delete_game(conn: &Connection, id: i64) -> Result<()> {
    let used = count(
        conn,
        "SELECT (SELECT COUNT(*) FROM rounds WHERE game_id = ?1) + (SELECT COUNT(*) FROM nominations WHERE game_id = ?1)",
        [id],
    )?;
    if used > 0 {
        user_bail!("This game has been played or nominated; mark it unavailable instead.");
    }
    conn.execute("DELETE FROM games WHERE id = ?1", [id])?;
    Ok(())
}

// --------------------------------------------------------- reference data

pub fn save_category(
    conn: &Connection,
    slug: &str,
    name: &str,
    icon: &str,
    description: &str,
    is_new: bool,
) -> Result<()> {
    let name = required(name, "Name")?;
    if is_new {
        let slug = pick_slug(slug, &name)?;
        if exists(conn, "SELECT 1 FROM categories WHERE slug = ?1", [&slug])? {
            user_bail!("A category \"{slug}\" already exists.");
        }
        let next = count(conn, "SELECT COALESCE(MAX(sort_order), -1) + 1 FROM categories", [])?;
        conn.execute(
            "INSERT INTO categories (slug, name, icon, description, sort_order) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![slug, name, clean(icon), clean(description), next],
        )?;
    } else {
        conn.execute(
            "UPDATE categories SET name = ?1, icon = ?2, description = ?3 WHERE slug = ?4",
            params![name, clean(icon), clean(description), slug],
        )?;
    }
    Ok(())
}

pub fn save_award(
    conn: &Connection,
    slug: &str,
    name: &str,
    icon: &str,
    description: &str,
    scope: &str,
    is_new: bool,
) -> Result<()> {
    let name = required(name, "Name")?;
    if is_new {
        let slug = pick_slug(slug, &name)?;
        if exists(conn, "SELECT 1 FROM awards WHERE slug = ?1", [&slug])? {
            user_bail!("An award \"{slug}\" already exists.");
        }
        let next = count(conn, "SELECT COALESCE(MAX(sort_order), -1) + 1 FROM awards", [])?;
        conn.execute(
            "INSERT INTO awards (slug, name, icon, description, scope, sort_order) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![slug, name, clean(icon), clean(description), scope, next],
        )?;
    } else {
        conn.execute(
            "UPDATE awards SET name = ?1, icon = ?2, description = ?3, scope = ?4 WHERE slug = ?5",
            params![name, clean(icon), clean(description), scope, slug],
        )?;
    }
    Ok(())
}

pub fn set_meta(conn: &Connection, key: &str, value: &str) -> Result<()> {
    match clean(value) {
        Some(v) => conn.execute(
            "INSERT INTO meta (key, value) VALUES (?1, ?2) ON CONFLICT (key) DO UPDATE SET value = excluded.value",
            params![key, v],
        )?,
        None => conn.execute("DELETE FROM meta WHERE key = ?1", [key])?,
    };
    Ok(())
}

// -------------------------------------------------------------- tournaments

#[derive(Clone, Debug, PartialEq)]
pub struct TournamentInput {
    pub name: String,
    pub slug: String,
    pub edition: String,
    pub start_date: String,
    pub end_date: String,
    pub location: String,
    pub status: Status,
    pub challenger_threshold: String,
    pub double_down_tokens: String,
    pub nominate_tokens: String,
    /// Points per placement, e.g. "6, 4, 3, 2, 1, 0".
    pub points: String,
}

impl Default for TournamentInput {
    fn default() -> Self {
        Self {
            name: String::new(),
            slug: String::new(),
            edition: String::new(),
            start_date: String::new(),
            end_date: String::new(),
            location: String::new(),
            status: Status::Upcoming,
            challenger_threshold: "20".into(),
            double_down_tokens: "1".into(),
            nominate_tokens: "1".into(),
            points: "6, 4, 3, 2, 1, 0".into(),
        }
    }
}

impl TournamentInput {
    pub fn from(t: &model::Tournament, points: &[(i64, i64)]) -> Self {
        Self {
            name: t.name.clone(),
            slug: t.slug.clone(),
            edition: t.edition.clone().unwrap_or_default(),
            start_date: t.start_date.clone().unwrap_or_default(),
            end_date: t.end_date.clone().unwrap_or_default(),
            location: t.location.clone().unwrap_or_default(),
            status: t.status,
            challenger_threshold: t.challenger_threshold.to_string(),
            double_down_tokens: t.double_down_tokens.to_string(),
            nominate_tokens: t.nominate_tokens.to_string(),
            points: points.iter().map(|(_, p)| p.to_string()).collect::<Vec<_>>().join(", "),
        }
    }
}

pub fn parse_points(raw: &str) -> Result<Vec<i64>> {
    let values: Vec<Option<i64>> = raw
        .split(|c: char| c == ',' || c.is_whitespace())
        .filter(|s| !s.is_empty())
        .map(|s| s.parse::<i64>().ok().filter(|v| *v >= 0))
        .collect();
    if values.is_empty() || values.iter().any(Option::is_none) {
        user_bail!("Points per placement must be whole numbers, e.g. \"6, 4, 3, 2, 1, 0\".");
    }
    Ok(values.into_iter().flatten().collect())
}

fn date(value: &str, what: &str) -> Result<Option<String>> {
    let Some(v) = clean(value) else { return Ok(None) };
    let format = time::macros::format_description!("[year]-[month]-[day]");
    if time::Date::parse(&v, format).is_err() {
        user_bail!("{what} must be a date like 2026-11-14.");
    }
    Ok(Some(v))
}

fn int(value: &str, what: &str, min: i64) -> Result<i64> {
    match opt_int(value, what, min)? {
        Some(n) => Ok(n),
        None => user_bail!("{what} is required."),
    }
}

pub fn save_tournament(
    conn: &Connection,
    id: Option<i64>,
    t: &TournamentInput,
    copy_players_from: Option<i64>,
) -> Result<i64> {
    let name = required(&t.name, "Name")?;
    let slug = pick_slug(&t.slug, &name)?;
    ensure_unique_slug(conn, Table::Tournaments, &slug, id)?;
    let threshold = int(&t.challenger_threshold, "Challenger threshold", 1)?;
    let dd = int(&t.double_down_tokens, "Double Down tokens", 0)?;
    let nom = int(&t.nominate_tokens, "Nominate tokens", 0)?;
    let points = parse_points(&t.points)?;
    let (start, end) = (date(&t.start_date, "Start date")?, date(&t.end_date, "End date")?);
    let tx = conn.unchecked_transaction()?;
    let tid = match id {
        None => {
            tx.execute(
                "INSERT INTO tournaments (slug, name, edition, start_date, end_date, location, status, challenger_threshold,
                 double_down_tokens, nominate_tokens) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![slug, name, clean(&t.edition), start, end, clean(&t.location), t.status.as_str(), threshold, dd, nom],
            )?;
            tx.last_insert_rowid()
        }
        Some(id) => {
            tx.execute(
                "UPDATE tournaments SET slug = ?1, name = ?2, edition = ?3, start_date = ?4, end_date = ?5, location = ?6,
                 status = ?7, challenger_threshold = ?8, double_down_tokens = ?9, nominate_tokens = ?10 WHERE id = ?11",
                params![slug, name, clean(&t.edition), start, end, clean(&t.location), t.status.as_str(), threshold, dd, nom, id],
            )?;
            id
        }
    };
    tx.execute("DELETE FROM tournament_points WHERE tournament_id = ?1", [tid])?;
    for (i, p) in points.iter().enumerate() {
        tx.execute("INSERT INTO tournament_points VALUES (?1, ?2, ?3)", params![tid, i as i64 + 1, p])?;
    }
    if let Some(from) = copy_players_from {
        tx.execute(
            "INSERT OR IGNORE INTO tournament_players SELECT ?1, player_id FROM tournament_players WHERE tournament_id = ?2",
            params![tid, from],
        )?;
    }
    tx.commit()?;
    Ok(tid)
}

/// Change the status; completing sets the end date to today if it's empty.
pub fn set_status(conn: &Connection, tid: i64, status: Status) -> Result<()> {
    conn.execute("UPDATE tournaments SET status = ?1 WHERE id = ?2", params![status.as_str(), tid])?;
    if status == Status::Completed {
        conn.execute("UPDATE tournaments SET end_date = COALESCE(end_date, ?1) WHERE id = ?2", params![today(), tid])?;
    }
    Ok(())
}

/// Delete a tournament that has no finished games (mistakes and tests).
pub fn delete_tournament(conn: &Connection, tid: i64) -> Result<()> {
    if exists(conn, "SELECT 1 FROM rounds WHERE tournament_id = ?1 AND completed = 1", [tid])? {
        user_bail!("This tournament has finished games and can't be deleted.");
    }
    conn.execute("DELETE FROM tournaments WHERE id = ?1", [tid])?;
    Ok(())
}

/// Register or unregister one player.
pub fn set_participant(conn: &Connection, tid: i64, pid: i64, playing: bool) -> Result<()> {
    if playing {
        conn.execute(
            "INSERT OR IGNORE INTO tournament_players (tournament_id, player_id) VALUES (?1, ?2)",
            params![tid, pid],
        )?;
        return Ok(());
    }
    let used = count(
        conn,
        "SELECT (SELECT COUNT(*) FROM results WHERE tournament_id = ?1 AND player_id = ?2) +
                (SELECT COUNT(*) FROM award_grants WHERE tournament_id = ?1 AND player_id = ?2) +
                (SELECT COUNT(*) FROM nominations WHERE tournament_id = ?1 AND player_id = ?2) +
                (SELECT COUNT(*) FROM nemesis_assignments WHERE tournament_id = ?1 AND (player_id = ?2 OR nemesis_player_id = ?2))",
        params![tid, pid],
    )?;
    if used > 0 {
        let name: String = conn.query_row("SELECT name FROM players WHERE id = ?1", [pid], |r| r.get(0))?;
        user_bail!(
            "{name} already has results, nominations, Nemeses or awards in this tournament and can't be removed."
        );
    }
    conn.execute("DELETE FROM tournament_players WHERE tournament_id = ?1 AND player_id = ?2", params![tid, pid])?;
    Ok(())
}

// ------------------------------------------------------- category positions

pub fn add_position(conn: &Connection, tid: i64, category: &str) -> Result<()> {
    let next = count(
        conn,
        "SELECT COALESCE(MAX(position), 0) + 1 FROM tournament_categories WHERE tournament_id = ?1",
        [tid],
    )?;
    conn.execute("INSERT INTO tournament_categories VALUES (?1, ?2, ?3)", params![tid, next, category])?;
    Ok(())
}

fn assert_unplayed(conn: &Connection, tid: i64, position: i64) -> Result<()> {
    if exists(conn, "SELECT 1 FROM rounds WHERE tournament_id = ?1 AND number = ?2", params![tid, position])? {
        user_bail!("Game {position} has already started; its category can't be changed.");
    }
    Ok(())
}

pub fn remove_position(conn: &Connection, tid: i64, position: i64) -> Result<()> {
    let tx = conn.unchecked_transaction()?;
    assert_unplayed(&tx, tid, position)?;
    tx.execute("DELETE FROM tournament_categories WHERE tournament_id = ?1 AND position = ?2", params![tid, position])?;
    // Close the gap so positions stay 1..n.
    for (p, _) in model::positions(&tx, tid)?.into_iter().filter(|(p, _)| *p > position) {
        assert_unplayed(&tx, tid, p)?;
        tx.execute(
            "UPDATE tournament_categories SET position = ?1 WHERE tournament_id = ?2 AND position = ?3",
            params![p - 1, tid, p],
        )?;
    }
    tx.commit()?;
    Ok(())
}

pub fn swap_positions(conn: &Connection, tid: i64, a: i64, b: i64) -> Result<()> {
    let tx = conn.unchecked_transaction()?;
    assert_unplayed(&tx, tid, a)?;
    assert_unplayed(&tx, tid, b)?;
    let list = model::positions(&tx, tid)?;
    let find = |n: i64| list.iter().find(|(p, _)| *p == n).map(|(_, c)| c.clone());
    if let (Some(ca), Some(cb)) = (find(a), find(b)) {
        let set = "UPDATE tournament_categories SET category_slug = ?1 WHERE tournament_id = ?2 AND position = ?3";
        tx.execute(set, params![cb, tid, a])?;
        tx.execute(set, params![ca, tid, b])?;
    }
    tx.commit()?;
    Ok(())
}

// ------------------------------------------------------------------- rounds

fn ensure_round(conn: &Connection, tid: i64, number: i64) -> Result<()> {
    if !exists(
        conn,
        "SELECT 1 FROM tournament_categories WHERE tournament_id = ?1 AND position = ?2",
        params![tid, number],
    )? {
        user_bail!("Game {number} has no category yet. Add it to the category order first.");
    }
    conn.execute(
        "INSERT OR IGNORE INTO rounds (tournament_id, number, completed) VALUES (?1, ?2, 0)",
        params![tid, number],
    )?;
    Ok(())
}

pub fn add_nomination(
    conn: &Connection,
    tid: i64,
    number: i64,
    game: i64,
    source: Source,
    player: Option<i64>,
    entries: i64,
) -> Result<()> {
    if source != Source::Vote && player.is_none() {
        user_bail!("Winner's picks and Nominate tokens need a player.");
    }
    if entries < 1 {
        user_bail!("Entries must be 1 or more.");
    }
    let tx = conn.unchecked_transaction()?;
    ensure_round(&tx, tid, number)?;
    tx.execute(
        "INSERT INTO nominations (tournament_id, round_number, game_id, player_id, source, entries) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![tid, number, game, if source == Source::Vote { None } else { player }, source.as_str(), entries],
    )?;
    tx.commit()?;
    Ok(())
}

pub fn remove_nomination(conn: &Connection, tid: i64, id: i64) -> Result<()> {
    conn.execute("DELETE FROM nominations WHERE id = ?1 AND tournament_id = ?2", params![id, tid])?;
    Ok(())
}

pub fn set_round_game(conn: &Connection, tid: i64, number: i64, game: Option<i64>) -> Result<()> {
    let tx = conn.unchecked_transaction()?;
    ensure_round(&tx, tid, number)?;
    tx.execute("UPDATE rounds SET game_id = ?1 WHERE tournament_id = ?2 AND number = ?3", params![game, tid, number])?;
    tx.commit()?;
    Ok(())
}

/// Pick a game from the nominations, weighted by entries. Returns the game id.
pub fn draw_game(conn: &Connection, tid: i64, number: i64) -> Result<i64> {
    let tickets: Vec<i64> = model::nominations(conn, tid, number)?
        .iter()
        .flat_map(|n| std::iter::repeat_n(n.game_id, n.entries.max(1) as usize))
        .collect();
    if tickets.is_empty() {
        user_bail!("Add nominations before drawing.");
    }
    let picked = tickets[rand::random_range(0..tickets.len())];
    set_round_game(conn, tid, number, Some(picked))?;
    Ok(picked)
}

#[derive(Clone, Debug, PartialEq)]
pub struct ResultInput {
    pub player_id: i64,
    pub placement: i64,
    pub base_points: i64,
    pub double_down: bool,
    pub nemesis_player_id: Option<i64>,
}

/// Save a round's results; `finish` also marks it completed (with checks).
pub fn save_results(conn: &Connection, tid: i64, number: i64, rows: &[ResultInput], finish: bool) -> Result<()> {
    for r in rows {
        if r.placement < 1 {
            user_bail!("Every placement must be 1 or higher.");
        }
        if r.base_points < 0 {
            user_bail!("Base points must be 0 or higher.");
        }
        if r.nemesis_player_id == Some(r.player_id) {
            user_bail!("A player can't be their own Nemesis.");
        }
    }
    if finish {
        if rows.len() < 2 {
            user_bail!("Enter results for at least two players.");
        }
        let winners = rows.iter().filter(|r| r.placement == 1).count();
        if winners != 1 {
            user_bail!("A finished game needs exactly one winner (placement 1); found {winners}.");
        }
        let game: Option<i64> = conn
            .query_row(
                "SELECT game_id FROM rounds WHERE tournament_id = ?1 AND number = ?2",
                params![tid, number],
                |r| r.get(0),
            )
            .optional()?
            .flatten();
        if game.is_none() {
            user_bail!("Choose which game was played before finishing it.");
        }
    }
    let tx = conn.unchecked_transaction()?;
    ensure_round(&tx, tid, number)?;
    tx.execute("DELETE FROM results WHERE tournament_id = ?1 AND round_number = ?2", params![tid, number])?;
    for r in rows {
        tx.execute(
            "INSERT INTO results VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![tid, number, r.player_id, r.placement, r.base_points, r.double_down, r.nemesis_player_id],
        )?;
    }
    if finish {
        tx.execute(
            "UPDATE rounds SET completed = 1, played_at = COALESCE(played_at, ?1) WHERE tournament_id = ?2 AND number = ?3",
            params![utc_now(), tid, number],
        )?;
    }
    tx.commit()?;
    Ok(())
}

/// Replace the announced Nemesis pairs of a round. Players without a Nemesis are left out.
pub fn set_nemeses(conn: &Connection, tid: i64, number: i64, pairs: &[(i64, Option<i64>)]) -> Result<()> {
    if pairs.iter().any(|(p, n)| Some(*p) == *n) {
        user_bail!("A player can't be their own Nemesis.");
    }
    let tx = conn.unchecked_transaction()?;
    ensure_round(&tx, tid, number)?;
    tx.execute("DELETE FROM nemesis_assignments WHERE tournament_id = ?1 AND round_number = ?2", params![tid, number])?;
    for (player, nemesis) in pairs {
        if let Some(nemesis) = nemesis {
            tx.execute(
                "INSERT INTO nemesis_assignments (tournament_id, round_number, player_id, nemesis_player_id) VALUES (?1, ?2, ?3, ?4)",
                params![tid, number, player, nemesis],
            )?;
        }
    }
    tx.commit()?;
    Ok(())
}

/// Independent draws: anyone but yourself. Nemeses can be shared and need not be mutual.
pub fn draw_nemeses(conn: &Connection, tid: i64, number: i64) -> Result<()> {
    let players = model::participant_ids(conn, tid)?;
    if players.len() < 2 {
        user_bail!("Register at least two players first.");
    }
    let pairs: Vec<(i64, Option<i64>)> = players
        .iter()
        .map(|p| {
            let others: Vec<i64> = players.iter().copied().filter(|o| o != p).collect();
            (*p, Some(others[rand::random_range(0..others.len())]))
        })
        .collect();
    set_nemeses(conn, tid, number, &pairs)
}

pub fn reopen_round(conn: &Connection, tid: i64, number: i64) -> Result<()> {
    if exists(
        conn,
        "SELECT 1 FROM rounds WHERE tournament_id = ?1 AND number > ?2 AND completed = 1",
        params![tid, number],
    )? {
        user_bail!("Only the most recent finished game can be reopened.");
    }
    conn.execute("UPDATE rounds SET completed = 0 WHERE tournament_id = ?1 AND number = ?2", params![tid, number])?;
    Ok(())
}

// ------------------------------------------------------------ awards/records

pub fn grant_award(conn: &Connection, tid: i64, award: &str, player: i64, detail: &str) -> Result<()> {
    conn.execute(
        "INSERT INTO award_grants (award_slug, player_id, tournament_id, detail) VALUES (?1, ?2, ?3, ?4)",
        params![award, player, tid, clean(detail)],
    )?;
    Ok(())
}

pub fn revoke_award(conn: &Connection, id: i64) -> Result<()> {
    conn.execute("DELETE FROM award_grants WHERE id = ?1", [id])?;
    Ok(())
}

pub fn add_record(conn: &Connection, tid: i64, label: &str, value: &str, player: Option<i64>) -> Result<()> {
    let (label, value) = (required(label, "Label")?, required(value, "Value")?);
    let next =
        count(conn, "SELECT COALESCE(MAX(position), 0) + 1 FROM tournament_records WHERE tournament_id = ?1", [tid])?;
    conn.execute(
        "INSERT INTO tournament_records VALUES (?1, ?2, ?3, ?4, ?5)",
        params![tid, next, label, value, player],
    )?;
    Ok(())
}

pub fn remove_record(conn: &Connection, tid: i64, position: i64) -> Result<()> {
    conn.execute("DELETE FROM tournament_records WHERE tournament_id = ?1 AND position = ?2", params![tid, position])?;
    Ok(())
}

/// Turn a database error into a readable message for the admin.
pub fn friendly(err: &anyhow::Error) -> String {
    if let Some(user) = err.downcast_ref::<UserError>() {
        return user.0.clone();
    }
    let text = format!("{err:#}");
    if text.contains("UNIQUE") || text.contains("FOREIGN KEY") || text.contains("CHECK") || text.contains("constraint")
    {
        return format!("The database rejected the change: {text}");
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;

    fn setup() -> (Connection, i64, Vec<i64>) {
        let conn = db::memory().unwrap();
        let ids: Vec<i64> = ["Ann", "Bob", "Cy"]
            .iter()
            .map(|n| save_player(&conn, None, &PlayerInput { name: (*n).into(), ..Default::default() }).unwrap())
            .collect();
        let tid =
            save_tournament(&conn, None, &TournamentInput { name: "LAN 2026".into(), ..Default::default() }, None)
                .unwrap();
        for pid in &ids {
            set_participant(&conn, tid, *pid, true).unwrap();
        }
        add_position(&conn, tid, "strategy").unwrap();
        add_position(&conn, tid, "combat").unwrap();
        (conn, tid, ids)
    }

    #[test]
    fn creates_with_slugs_and_rejects_duplicates() {
        let (conn, _, _) = setup();
        let p = model::players(&conn).unwrap();
        assert_eq!(p.iter().map(|p| p.slug.as_str()).collect::<Vec<_>>(), ["ann", "bob", "cy"]);
        let err = save_player(&conn, None, &PlayerInput { name: "Ann".into(), ..Default::default() }).unwrap_err();
        assert!(friendly(&err).contains("already uses"));
        let err = save_player(
            &conn,
            None,
            &PlayerInput { name: "Dee".into(), avatar_url: "http://x".into(), ..Default::default() },
        )
        .unwrap_err();
        assert!(friendly(&err).contains("https://"));
        assert_eq!(model::tournament_by_slug(&conn, "lan-2026").unwrap().unwrap().challenger_threshold, 20);
        assert_eq!(model::points_table(&conn, 1).unwrap().len(), 6);
    }

    #[test]
    fn finishing_needs_one_winner_and_a_game() {
        let (conn, tid, ids) = setup();
        let row = |pid, placement| ResultInput {
            player_id: pid,
            placement,
            base_points: 3,
            double_down: false,
            nemesis_player_id: None,
        };
        let err = save_results(&conn, tid, 1, &[row(ids[0], 1), row(ids[1], 2)], true).unwrap_err();
        assert!(friendly(&err).contains("Choose which game"));
        let game = save_game(&conn, None, &GameInput { name: "Catan".into(), ..GameInput::new() }).unwrap();
        set_round_game(&conn, tid, 1, Some(game)).unwrap();
        let err = save_results(&conn, tid, 1, &[row(ids[0], 1), row(ids[1], 1)], true).unwrap_err();
        assert!(friendly(&err).contains("exactly one winner"));
        save_results(&conn, tid, 1, &[row(ids[0], 1), row(ids[1], 2)], true).unwrap();
        assert!(model::rounds(&conn, tid).unwrap()[0].completed);
        // History protects participants and positions.
        assert!(set_participant(&conn, tid, ids[0], false).is_err());
        assert!(set_participant(&conn, tid, ids[2], false).is_ok());
        assert!(remove_position(&conn, tid, 1).is_err());
        assert!(delete_tournament(&conn, tid).is_err());
        reopen_round(&conn, tid, 1).unwrap();
        assert!(!model::rounds(&conn, tid).unwrap()[0].completed);
    }

    #[test]
    fn positions_reorder_and_close_gaps() {
        let (conn, tid, _) = setup();
        add_position(&conn, tid, "puzzle").unwrap();
        swap_positions(&conn, tid, 1, 2).unwrap();
        remove_position(&conn, tid, 1).unwrap();
        let cats: Vec<String> = model::positions(&conn, tid).unwrap().into_iter().map(|(_, c)| c).collect();
        assert_eq!(cats, ["strategy", "puzzle"]);
    }

    #[test]
    fn draws_from_nominations_and_nemeses() {
        let (conn, tid, ids) = setup();
        let game = save_game(&conn, None, &GameInput { name: "Catan".into(), ..GameInput::new() }).unwrap();
        assert!(draw_game(&conn, tid, 1).is_err());
        assert!(add_nomination(&conn, tid, 1, game, Source::Token, None, 1).is_err());
        add_nomination(&conn, tid, 1, game, Source::Vote, Some(ids[0]), 2).unwrap();
        assert_eq!(model::nominations(&conn, tid, 1).unwrap()[0].player_id, None, "votes have no player");
        assert_eq!(draw_game(&conn, tid, 1).unwrap(), game);
        draw_nemeses(&conn, tid, 1).unwrap();
        let pairs = model::nemesis_assignments(&conn, tid, 1).unwrap();
        assert_eq!(pairs.len(), 3);
        assert!(pairs.iter().all(|(p, n)| p != n));
    }

    #[test]
    fn parses_points() {
        assert_eq!(parse_points("6, 4 3,2").unwrap(), vec![6, 4, 3, 2]);
        assert!(parse_points("6, -1").is_err());
        assert!(parse_points("").is_err());
    }
}
