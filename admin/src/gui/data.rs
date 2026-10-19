//! Everything the screens show, loaded in one go after every change.

use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;

use anyhow::Result;
use rusqlite::Connection;

use crate::export::{Exporter, avatar_files};
use crate::model::{self, Award, Category, Game, Nomination, Player, Record, ResultRow, RoundRow, Tournament};
use crate::rules::TournamentState;
use crate::workspace::Workspace;

pub struct TournamentData {
    pub t: Tournament,
    pub positions: Vec<(i64, String)>,
    pub rounds: Vec<RoundRow>,
    pub participants: Vec<i64>,
    pub points: Vec<i64>,
    pub nominations: Vec<Nomination>,
    pub results: Vec<(i64, ResultRow)>,
    /// (round, player, Nemesis) announced before each round.
    pub nemeses: Vec<(i64, i64, i64)>,
    pub grants: Vec<model::Grant>,
    pub records: Vec<Record>,
    /// Standings, Challengers and champion, from the rules module.
    pub state: TournamentState,
}

impl TournamentData {
    pub fn round(&self, number: i64) -> Option<&RoundRow> {
        self.rounds.iter().find(|r| r.number == number)
    }

    pub fn category_at(&self, number: i64) -> Option<&str> {
        self.positions.iter().find(|(p, _)| *p == number).map(|(_, c)| c.as_str())
    }

    pub fn completed(&self) -> usize {
        self.rounds.iter().filter(|r| r.completed).count()
    }

    /// The next game to enter: the first category position without a finished round.
    pub fn next_round(&self) -> Option<i64> {
        if self.state.champion_id.is_some() {
            return None;
        }
        self.positions.iter().map(|(p, _)| *p).find(|p| !self.round(*p).is_some_and(|r| r.completed))
    }

    pub fn round_results(&self, number: i64) -> Vec<&ResultRow> {
        self.results.iter().filter(|(n, _)| *n == number).map(|(_, r)| r).collect()
    }

    pub fn round_nominations(&self, number: i64) -> Vec<&Nomination> {
        self.nominations.iter().filter(|n| n.round_number == number).collect()
    }

    pub fn round_nemeses(&self, number: i64) -> HashMap<i64, i64> {
        self.nemeses.iter().filter(|(n, _, _)| *n == number).map(|(_, p, n)| (*p, *n)).collect()
    }

    pub fn points_for(&self, placement: i64) -> i64 {
        usize::try_from(placement - 1).ok().and_then(|i| self.points.get(i)).copied().unwrap_or(0)
    }

    pub fn max_points(&self) -> Option<i64> {
        self.points.iter().max().copied()
    }

    pub fn has_played(&self, pid: i64) -> bool {
        self.results.iter().any(|(_, r)| r.player_id == pid)
    }
}

pub struct Data {
    /// Bumped on every reload, so screens know when to refresh their forms.
    pub rev: u64,
    pub players: Vec<Player>,
    pub games: Vec<Game>,
    pub categories: Vec<Category>,
    pub awards: Vec<Award>,
    /// Newest first.
    pub tournaments: Vec<TournamentData>,
    pub demo_present: bool,
    pub meta: BTreeMap<String, String>,
    /// Player slug -> picture file in avatars/.
    pub avatars: BTreeMap<String, PathBuf>,
    pub avatars_dir: PathBuf,
    pub site_url: String,
}

impl Data {
    pub fn load(conn: &Connection, ws: &Workspace, rev: u64) -> Result<Self> {
        let exporter = Exporter::new(conn, Some(&ws.avatars))?;
        let mut states = exporter.states;
        let all_nominations = model::all_nominations(conn)?;
        let grants = model::grants(conn)?;
        let mut tournaments = Vec::new();
        for t in exporter.tournaments.iter().rev() {
            let tid = t.id;
            let rounds = model::rounds(conn, tid)?;
            let mut nemeses = Vec::new();
            for r in &rounds {
                nemeses.extend(
                    model::nemesis_assignments(conn, tid, r.number)?.into_iter().map(|(p, n)| (r.number, p, n)),
                );
            }
            tournaments.push(TournamentData {
                t: t.clone(),
                positions: model::positions(conn, tid)?,
                rounds,
                participants: model::participants(conn, tid)?.into_iter().map(|p| p.id).collect(),
                points: model::points_table(conn, tid)?.into_iter().map(|(_, p)| p).collect(),
                nominations: all_nominations.iter().filter(|n| n.tournament_id == tid).cloned().collect(),
                results: model::results(conn, tid)?,
                nemeses,
                grants: grants.iter().filter(|g| g.tournament_id == Some(tid)).cloned().collect(),
                records: model::records(conn, tid)?,
                state: states.remove(&tid).expect("state for every tournament"),
            });
        }
        // Newest first: by start date, undated (planned) ones on top.
        tournaments.sort_by(|a, b| {
            (b.t.start_date.is_none(), &b.t.start_date, b.t.id).cmp(&(
                a.t.start_date.is_none(),
                &a.t.start_date,
                a.t.id,
            ))
        });
        let players = model::players(conn)?;
        let slugs: Vec<&str> = players.iter().map(|p| p.slug.as_str()).collect();
        let avatars = avatar_files(Some(&ws.avatars), &slugs)
            .into_iter()
            .map(|(slug, file)| (slug, ws.avatars.join(file)))
            .collect();
        let meta: BTreeMap<String, String> = model::meta(conn)?.into_iter().collect();
        let site_url = crate::export::with_slash(
            meta.get("site_url").filter(|s| !s.is_empty()).map_or(crate::export::DEFAULT_SITE_URL, String::as_str),
        );
        Ok(Self {
            rev,
            games: model::games(conn)?,
            categories: model::categories(conn)?,
            awards: model::awards(conn)?,
            tournaments,
            demo_present: model::demo_present(conn)?,
            players,
            meta,
            avatars,
            avatars_dir: ws.avatars.clone(),
            site_url,
        })
    }

    pub fn player(&self, id: i64) -> Option<&Player> {
        self.players.iter().find(|p| p.id == id)
    }

    pub fn player_name(&self, id: i64) -> String {
        self.player(id).map_or_else(|| format!("#{id}"), |p| p.name.clone())
    }

    pub fn game(&self, id: i64) -> Option<&Game> {
        self.games.iter().find(|g| g.id == id)
    }

    pub fn game_name(&self, id: i64) -> String {
        self.game(id).map_or_else(|| format!("#{id}"), |g| g.name.clone())
    }

    pub fn category(&self, slug: &str) -> Option<&Category> {
        self.categories.iter().find(|c| c.slug == slug)
    }

    /// "♟️ Strategy"
    pub fn category_label(&self, slug: &str) -> String {
        match self.category(slug) {
            Some(c) => format!("{} {}", super::theme::icon(c.icon.as_deref()), c.name).trim().to_string(),
            None => slug.to_string(),
        }
    }

    pub fn award_label(&self, slug: &str) -> String {
        match self.awards.iter().find(|a| a.slug == slug) {
            Some(a) => format!("{} {}", super::theme::icon(a.icon.as_deref()), a.name).trim().to_string(),
            None => slug.to_string(),
        }
    }

    pub fn tournament(&self, id: i64) -> Option<&TournamentData> {
        self.tournaments.iter().find(|t| t.t.id == id)
    }

    /// The tournament to focus on: the live one, else the next upcoming, else the newest.
    pub fn focus(&self) -> Option<&TournamentData> {
        use crate::model::Status;
        let real = || self.tournaments.iter().filter(|t| !t.t.is_demo || !self.has_real_tournament());
        real()
            .find(|t| t.t.status == Status::Ongoing)
            .or_else(|| real().rfind(|t| t.t.status == Status::Upcoming))
            .or_else(|| real().next())
    }

    fn has_real_tournament(&self) -> bool {
        self.tournaments.iter().any(|t| !t.t.is_demo)
    }

    pub fn avatar_uri(&self, slug: &str) -> Option<String> {
        self.avatars.get(slug).map(|p| format!("file://{}", p.display()))
    }
}
