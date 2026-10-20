//! Plain rows as stored in SQLite (facts only).

use anyhow::Result;
use rusqlite::{Connection, Row, params};

#[derive(Clone, Debug, PartialEq)]
pub struct Player {
    pub id: i64,
    pub slug: String,
    pub name: String,
    pub initials: Option<String>,
    pub avatar_url: Option<String>,
    pub is_demo: bool,
    /// Overrides the Discord username as the shown name.
    pub nickname: Option<String>,
    /// Discord user id (snowflake), and the names last copied from that account.
    pub discord_id: Option<String>,
    pub discord_username: Option<String>,
    pub discord_display_name: Option<String>,
}

impl Player {
    fn from_row(r: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: r.get("id")?,
            slug: r.get("slug")?,
            name: r.get("name")?,
            initials: r.get("initials")?,
            avatar_url: r.get("avatar_url")?,
            is_demo: r.get("is_demo")?,
            nickname: r.get("nickname")?,
            discord_id: r.get("discord_id")?,
            discord_username: r.get("discord_username")?,
            discord_display_name: r.get("discord_display_name")?,
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Game {
    pub id: i64,
    pub slug: String,
    pub name: String,
    pub description: Option<String>,
    pub min_players: Option<i64>,
    pub max_players: Option<i64>,
    pub available: bool,
    pub is_demo: bool,
    /// Where to get or play it (https only).
    pub url: Option<String>,
    pub categories: Vec<String>,
}

impl Game {
    fn from_row(r: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: r.get("id")?,
            slug: r.get("slug")?,
            name: r.get("name")?,
            description: r.get("description")?,
            min_players: r.get("min_players")?,
            max_players: r.get("max_players")?,
            available: r.get("available")?,
            is_demo: r.get("is_demo")?,
            url: r.get("url")?,
            categories: Vec::new(),
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Category {
    pub slug: String,
    pub name: String,
    pub icon: Option<String>,
    pub description: Option<String>,
    pub sort_order: i64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Award {
    pub slug: String,
    pub name: String,
    pub icon: Option<String>,
    pub description: Option<String>,
    pub scope: String,
    pub sort_order: i64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Status {
    Upcoming,
    Ongoing,
    Completed,
}

impl Status {
    pub fn as_str(self) -> &'static str {
        match self {
            Status::Upcoming => "upcoming",
            Status::Ongoing => "ongoing",
            Status::Completed => "completed",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Status::Upcoming => "Upcoming",
            Status::Ongoing => "Live",
            Status::Completed => "Completed",
        }
    }

    pub fn parse(s: &str) -> Status {
        match s {
            "ongoing" => Status::Ongoing,
            "completed" => Status::Completed,
            _ => Status::Upcoming,
        }
    }
}

/// The tags a tournament can have: (slug, label). Names and filters use this order.
pub const TOURNAMENT_TAGS: &[(&str, &str)] = &[
    ("lan", "LAN"),
    ("online", "Online"),
    ("hybrid", "Hybrid"),
    ("digital", "Digital games"),
    ("physical", "Physical games"),
    ("weekend", "Weekend"),
    ("overnight", "Overnight"),
    ("holiday", "Holiday special"),
    ("anniversary", "Anniversary"),
    ("charity", "Charity"),
];

pub fn tag_label(slug: &str) -> &str {
    TOURNAMENT_TAGS.iter().find(|(s, _)| *s == slug).map_or(slug, |(_, l)| l)
}

#[derive(Clone, Debug, PartialEq)]
pub struct Tournament {
    pub id: i64,
    pub slug: String,
    pub name: String,
    /// Overrides the generated name (season, tags and year).
    pub custom_name: Option<String>,
    /// Tag slugs from `TOURNAMENT_TAGS`, in that order.
    pub tags: Vec<String>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub location: Option<String>,
    pub status: Status,
    pub challenger_threshold: i64,
    pub double_down_tokens: i64,
    pub nominate_tokens: i64,
    pub is_demo: bool,
}

impl Tournament {
    fn from_row(r: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: r.get("id")?,
            slug: r.get("slug")?,
            name: r.get("name")?,
            custom_name: r.get("custom_name")?,
            tags: Vec::new(),
            start_date: r.get("start_date")?,
            end_date: r.get("end_date")?,
            location: r.get("location")?,
            status: Status::parse(&r.get::<_, String>("status")?),
            challenger_threshold: r.get("challenger_threshold")?,
            double_down_tokens: r.get("double_down_tokens")?,
            nominate_tokens: r.get("nominate_tokens")?,
            is_demo: r.get("is_demo")?,
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct RoundRow {
    pub number: i64,
    pub game_id: Option<i64>,
    pub played_at: Option<String>,
    pub completed: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ResultRow {
    pub player_id: i64,
    pub placement: i64,
    pub base_points: i64,
    pub double_down: bool,
    pub nemesis_player_id: Option<i64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    Winner,
    Vote,
    Token,
}

impl Source {
    pub fn as_str(self) -> &'static str {
        match self {
            Source::Winner => "winner",
            Source::Vote => "vote",
            Source::Token => "token",
        }
    }

    pub fn parse(s: &str) -> Source {
        match s {
            "winner" => Source::Winner,
            "token" => Source::Token,
            _ => Source::Vote,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Nomination {
    pub id: i64,
    pub tournament_id: i64,
    pub round_number: i64,
    pub game_id: i64,
    pub player_id: Option<i64>,
    pub source: Source,
    pub entries: i64,
}

impl Nomination {
    fn from_row(r: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: r.get("id")?,
            tournament_id: r.get("tournament_id")?,
            round_number: r.get("round_number")?,
            game_id: r.get("game_id")?,
            player_id: r.get("player_id")?,
            source: Source::parse(&r.get::<_, String>("source")?),
            entries: r.get("entries")?,
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Grant {
    pub id: i64,
    pub award_slug: String,
    pub player_id: i64,
    pub tournament_id: Option<i64>,
    pub detail: Option<String>,
    /// The award's sort order (joined in for sorting).
    pub sort_order: i64,
}

// ------------------------------------------------------------------ queries

fn collect<T>(
    conn: &Connection,
    sql: &str,
    params: impl rusqlite::Params,
    f: impl FnMut(&Row) -> rusqlite::Result<T>,
) -> Result<Vec<T>> {
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map(params, f)?.collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

pub fn players(conn: &Connection) -> Result<Vec<Player>> {
    collect(conn, "SELECT * FROM players ORDER BY name COLLATE NOCASE", [], Player::from_row)
}

pub fn games(conn: &Connection) -> Result<Vec<Game>> {
    let mut games = collect(conn, "SELECT * FROM games ORDER BY name COLLATE NOCASE", [], Game::from_row)?;
    let cats = collect(conn, "SELECT game_id, category_slug FROM game_categories", [], |r| {
        Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?))
    })?;
    for g in &mut games {
        g.categories = cats.iter().filter(|(id, _)| *id == g.id).map(|(_, c)| c.clone()).collect();
    }
    Ok(games)
}

pub fn categories(conn: &Connection) -> Result<Vec<Category>> {
    collect(conn, "SELECT * FROM categories ORDER BY sort_order, slug", [], |r| {
        Ok(Category {
            slug: r.get("slug")?,
            name: r.get("name")?,
            icon: r.get("icon")?,
            description: r.get("description")?,
            sort_order: r.get("sort_order")?,
        })
    })
}

pub fn awards(conn: &Connection) -> Result<Vec<Award>> {
    collect(conn, "SELECT * FROM awards ORDER BY sort_order, slug", [], |r| {
        Ok(Award {
            slug: r.get("slug")?,
            name: r.get("name")?,
            icon: r.get("icon")?,
            description: r.get("description")?,
            scope: r.get("scope")?,
            sort_order: r.get("sort_order")?,
        })
    })
}

/// Oldest first; tournaments without a start date last.
pub fn tournaments(conn: &Connection) -> Result<Vec<Tournament>> {
    let mut list = collect(
        conn,
        "SELECT * FROM tournaments ORDER BY start_date IS NULL, start_date, id",
        [],
        Tournament::from_row,
    )?;
    let tags = all_tags(conn)?;
    for t in &mut list {
        t.tags = tags.iter().filter(|(id, _)| *id == t.id).map(|(_, tag)| tag.clone()).collect();
    }
    Ok(list)
}

/// (tournament id, tag) in `TOURNAMENT_TAGS` order.
fn all_tags(conn: &Connection) -> Result<Vec<(i64, String)>> {
    let mut tags: Vec<(i64, String)> =
        collect(conn, "SELECT tournament_id, tag FROM tournament_tags", [], |r| Ok((r.get(0)?, r.get(1)?)))?;
    let order = |tag: &str| TOURNAMENT_TAGS.iter().position(|(s, _)| *s == tag).unwrap_or(usize::MAX);
    tags.sort_by_key(|(id, tag)| (*id, order(tag)));
    Ok(tags)
}

pub fn tournament(conn: &Connection, id: i64) -> Result<Option<Tournament>> {
    Ok(tournaments(conn)?.into_iter().find(|t| t.id == id))
}

#[cfg(test)]
pub fn tournament_by_slug(conn: &Connection, slug: &str) -> Result<Option<Tournament>> {
    Ok(collect(conn, "SELECT * FROM tournaments WHERE slug = ?1", [slug], Tournament::from_row)?.pop())
}

pub fn positions(conn: &Connection, tid: i64) -> Result<Vec<(i64, String)>> {
    collect(
        conn,
        "SELECT position, category_slug FROM tournament_categories WHERE tournament_id = ?1 ORDER BY position",
        [tid],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )
}

pub fn rounds(conn: &Connection, tid: i64) -> Result<Vec<RoundRow>> {
    collect(conn, "SELECT * FROM rounds WHERE tournament_id = ?1 ORDER BY number", [tid], |r| {
        Ok(RoundRow {
            number: r.get("number")?,
            game_id: r.get("game_id")?,
            played_at: r.get("played_at")?,
            completed: r.get("completed")?,
        })
    })
}

/// Results of every round of a tournament: (round number, result).
pub fn results(conn: &Connection, tid: i64) -> Result<Vec<(i64, ResultRow)>> {
    collect(
        conn,
        "SELECT * FROM results WHERE tournament_id = ?1 ORDER BY round_number, placement, player_id",
        [tid],
        |r| {
            Ok((
                r.get("round_number")?,
                ResultRow {
                    player_id: r.get("player_id")?,
                    placement: r.get("placement")?,
                    base_points: r.get("base_points")?,
                    double_down: r.get("double_down")?,
                    nemesis_player_id: r.get("nemesis_player_id")?,
                },
            ))
        },
    )
}

pub fn participant_ids(conn: &Connection, tid: i64) -> Result<Vec<i64>> {
    collect(conn, "SELECT player_id FROM tournament_players WHERE tournament_id = ?1", [tid], |r| r.get(0))
}

pub fn participants(conn: &Connection, tid: i64) -> Result<Vec<Player>> {
    collect(
        conn,
        "SELECT p.* FROM players p JOIN tournament_players tp ON tp.player_id = p.id
         WHERE tp.tournament_id = ?1 ORDER BY p.name COLLATE NOCASE",
        [tid],
        Player::from_row,
    )
}

pub fn all_nominations(conn: &Connection) -> Result<Vec<Nomination>> {
    collect(conn, "SELECT * FROM nominations ORDER BY id", [], Nomination::from_row)
}

pub fn nominations(conn: &Connection, tid: i64, number: i64) -> Result<Vec<Nomination>> {
    collect(
        conn,
        "SELECT * FROM nominations WHERE tournament_id = ?1 AND round_number = ?2 ORDER BY id",
        params![tid, number],
        Nomination::from_row,
    )
}

pub fn points_table(conn: &Connection, tid: i64) -> Result<Vec<(i64, i64)>> {
    collect(
        conn,
        "SELECT placement, points FROM tournament_points WHERE tournament_id = ?1 ORDER BY placement",
        [tid],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )
}

/// Announced Nemesis pairs of a round: (player, nemesis), by player id.
pub fn nemesis_assignments(conn: &Connection, tid: i64, number: i64) -> Result<Vec<(i64, i64)>> {
    collect(
        conn,
        "SELECT player_id, nemesis_player_id FROM nemesis_assignments
         WHERE tournament_id = ?1 AND round_number = ?2 ORDER BY player_id",
        params![tid, number],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )
}

pub fn meta(conn: &Connection) -> Result<Vec<(String, String)>> {
    collect(conn, "SELECT key, value FROM meta", [], |r| Ok((r.get(0)?, r.get(1)?)))
}

pub fn demo_present(conn: &Connection) -> Result<bool> {
    Ok(conn.query_row(
        "SELECT EXISTS (SELECT 1 FROM players WHERE is_demo = 1)
             OR EXISTS (SELECT 1 FROM games WHERE is_demo = 1)
             OR EXISTS (SELECT 1 FROM tournaments WHERE is_demo = 1)",
        [],
        |r| r.get(0),
    )?)
}
