//! The tournament rules. This is the only place they are implemented.
//!
//! - Points per game: `(base points + 1 if the Nemesis finished behind) × 2 if Double Down`.
//! - Challenger: the running total reaches the threshold.
//! - Champion: the first round winner who was already a Challenger before that round.
//! - Ranks: competition ranking (1, 1, 3) by points; ties are shared, never broken.

use std::collections::{BTreeMap, HashMap};

use anyhow::{Context, Result};
use rusqlite::Connection;

use crate::model::{self, Status, Tournament};

#[derive(Clone, Debug)]
pub struct ResultState {
    pub player_id: i64,
    pub placement: i64,
    pub base_points: i64,
    pub double_down: bool,
    pub nemesis_id: Option<i64>,
    pub defeated_nemesis: Option<bool>,
    pub nemesis_bonus: i64,
    pub points: i64,
}

#[derive(Clone, Debug)]
pub struct RoundState {
    pub number: i64,
    pub category: String,
    pub game_id: Option<i64>,
    pub played_at: Option<String>,
    pub completed: bool,
    pub results: Vec<ResultState>,
    pub winner_id: Option<i64>,
    pub new_challengers: Vec<i64>,
    pub decided: bool,
}

#[derive(Clone, Debug, Default)]
pub struct PlayerState {
    pub player_id: i64,
    pub points: i64,
    pub challenger_since: Option<i64>,
    pub games_played: i64,
    pub games_won: i64,
    pub podiums: i64,
    pub category_wins: Vec<String>,
    pub double_downs_used: i64,
    pub nominations_used: i64,
    pub rank: i64,
    /// Final placement once the tournament is over: the champion is 1st, everyone else follows by points.
    pub finish: Option<i64>,
}

#[derive(Clone, Debug)]
pub struct TournamentState {
    pub row: Tournament,
    pub positions: Vec<(i64, String)>,
    pub rounds: BTreeMap<i64, RoundState>,
    pub players: HashMap<i64, PlayerState>,
    pub champion_id: Option<i64>,
    pub deciding_round: Option<i64>,
    /// Completed rounds after the tournament was decided (a validation error).
    pub ignored_rounds: Vec<i64>,
    /// Player ids in standings order.
    pub order: Vec<i64>,
}

impl TournamentState {
    pub fn id(&self) -> i64 {
        self.row.id
    }

    pub fn completed_rounds(&self) -> impl DoubleEndedIterator<Item = &RoundState> {
        self.rounds.values().filter(|r| r.completed && !self.ignored_rounds.contains(&r.number))
    }

    pub fn standings(&self) -> impl Iterator<Item = &PlayerState> {
        self.order.iter().map(|pid| &self.players[pid])
    }

    pub fn finished(&self) -> bool {
        self.row.status == Status::Completed || self.champion_id.is_some()
    }

    /// Category position currently being played or selected, if the tournament is live.
    pub fn active_position(&self) -> Option<(i64, &str)> {
        if self.finished() {
            return None;
        }
        self.positions
            .iter()
            .find(|(position, _)| self.rounds.get(position).is_none_or(|r| !r.completed))
            .map(|(p, c)| (*p, c.as_str()))
    }
}

pub fn compute_tournament(
    conn: &Connection,
    row: &Tournament,
    names: &HashMap<i64, String>,
) -> Result<TournamentState> {
    let tid = row.id;
    let threshold = row.challenger_threshold;
    let positions = model::positions(conn, tid)?;
    let category_at: HashMap<i64, &String> = positions.iter().map(|(p, c)| (*p, c)).collect();

    let mut rounds = BTreeMap::new();
    for r in model::rounds(conn, tid)? {
        let category = category_at.get(&r.number).with_context(|| format!("round {} has no category", r.number))?;
        rounds.insert(
            r.number,
            RoundState {
                number: r.number,
                category: (*category).clone(),
                game_id: r.game_id,
                played_at: r.played_at,
                completed: r.completed,
                results: Vec::new(),
                winner_id: None,
                new_challengers: Vec::new(),
                decided: false,
            },
        );
    }
    for (number, r) in model::results(conn, tid)? {
        if let Some(rnd) = rounds.get_mut(&number) {
            rnd.results.push(ResultState {
                player_id: r.player_id,
                placement: r.placement,
                base_points: r.base_points,
                double_down: r.double_down,
                nemesis_id: r.nemesis_player_id,
                defeated_nemesis: None,
                nemesis_bonus: 0,
                points: 0,
            });
        }
    }

    let mut players: HashMap<i64, PlayerState> = model::participant_ids(conn, tid)?
        .into_iter()
        .map(|pid| (pid, PlayerState { player_id: pid, ..Default::default() }))
        .collect();
    {
        let mut stmt = conn.prepare(
            "SELECT player_id, COUNT(*) FROM nominations WHERE tournament_id = ?1 AND source = 'token' GROUP BY player_id",
        )?;
        let used = stmt.query_map([tid], |r| Ok((r.get::<_, Option<i64>>(0)?, r.get::<_, i64>(1)?)))?;
        for row in used {
            let (pid, n) = row?;
            if let Some(p) = pid.and_then(|pid| players.get_mut(&pid)) {
                p.nominations_used = n;
            }
        }
    }

    let mut champion_id = None;
    let mut deciding_round = None;
    let mut ignored_rounds = Vec::new();

    for (&number, rnd) in rounds.iter_mut() {
        if !rnd.completed {
            continue;
        }
        if champion_id.is_some() {
            // The tournament ended when a Challenger won; later rounds are invalid (see validate).
            ignored_rounds.push(number);
            continue;
        }

        let placement_of: HashMap<i64, i64> = rnd.results.iter().map(|r| (r.player_id, r.placement)).collect();
        for res in &mut rnd.results {
            if let Some(nemesis) = res.nemesis_id
                && let Some(&theirs) = placement_of.get(&nemesis)
            {
                res.defeated_nemesis = Some(res.placement < theirs);
            }
            res.nemesis_bonus = i64::from(res.defeated_nemesis == Some(true));
            res.points = (res.base_points + res.nemesis_bonus) * if res.double_down { 2 } else { 1 };
        }

        let winners: Vec<i64> = rnd.results.iter().filter(|r| r.placement == 1).map(|r| r.player_id).collect();
        rnd.winner_id = if winners.len() == 1 { Some(winners[0]) } else { None };

        // A player who was already a Challenger before this round and wins it is crowned.
        if let Some(winner) = rnd.winner_id
            && players.get(&winner).is_some_and(|p| p.challenger_since.is_some())
        {
            champion_id = Some(winner);
            deciding_round = Some(number);
            rnd.decided = true;
        }

        for res in &rnd.results {
            let Some(p) = players.get_mut(&res.player_id) else { continue };
            p.points += res.points;
            p.games_played += 1;
            if res.placement <= 3 {
                p.podiums += 1;
            }
            if res.double_down {
                p.double_downs_used += 1;
            }
            if Some(res.player_id) == rnd.winner_id {
                p.games_won += 1;
                p.category_wins.push(rnd.category.clone());
            }
        }

        for res in &rnd.results {
            if let Some(p) = players.get_mut(&res.player_id)
                && p.challenger_since.is_none()
                && p.points >= threshold
            {
                p.challenger_since = Some(number);
                rnd.new_challengers.push(p.player_id);
            }
        }
    }

    // Competition ranking (1, 1, 3) by points; ties are shared, never broken.
    let empty = String::new();
    let mut ordered: Vec<i64> = players.keys().copied().collect();
    ordered.sort_by(|a, b| {
        let (pa, pb) = (&players[a], &players[b]);
        pb.points
            .cmp(&pa.points)
            .then_with(|| names.get(a).unwrap_or(&empty).cmp(names.get(b).unwrap_or(&empty)))
            .then_with(|| a.cmp(b))
    });
    let mut previous: Option<(i64, i64)> = None;
    for (index, pid) in ordered.iter().enumerate() {
        let p = players.get_mut(pid).expect("player");
        p.rank = match previous {
            Some((points, rank)) if points == p.points => rank,
            _ => index as i64 + 1,
        };
        previous = Some((p.points, p.rank));
    }

    if row.status == Status::Completed || champion_id.is_some() {
        let offset = i64::from(champion_id.is_some());
        let mut previous: Option<(i64, i64)> = None;
        let others = ordered.iter().filter(|pid| Some(**pid) != champion_id);
        for (index, pid) in others.enumerate() {
            let p = players.get_mut(pid).expect("player");
            let finish = match previous {
                Some((points, finish)) if points == p.points => finish,
                _ => index as i64 + 1 + offset,
            };
            p.finish = Some(finish);
            previous = Some((p.points, finish));
        }
        if let Some(p) = champion_id.and_then(|c| players.get_mut(&c)) {
            p.finish = Some(1);
        }
    }

    Ok(TournamentState {
        row: row.clone(),
        positions,
        rounds,
        players,
        champion_id,
        deciding_round,
        ignored_rounds,
        order: ordered,
    })
}
