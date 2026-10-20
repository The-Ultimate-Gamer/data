//! Builds every public JSON document (and share page) from one snapshot of the database.

use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::Path;

use anyhow::Result;
use rusqlite::Connection;
use serde_json::{Map, Value, json};

use crate::db::{SCHEMA_VERSION, utc_now};
use crate::model::{self, Category, Game, Grant, Nomination, Player, Source, Status, Tournament};
use crate::rules::{RoundState, TournamentState, compute_tournament};

pub const AVATAR_EXTENSIONS: &[&str] = &[".jpg", ".jpeg", ".png", ".webp", ".gif"];
pub const DEFAULT_SITE_URL: &str = "https://the-ultimate-gamer.github.io/";
pub const SHARE_DIR: &str = "share";
const RECENT_RESULTS: usize = 6;

pub struct Exporter {
    pub players: Vec<Player>,
    player_index: HashMap<i64, usize>,
    pub avatar_files: BTreeMap<String, String>,
    pub games: Vec<Game>,
    game_index: HashMap<i64, usize>,
    game_categories: Vec<(i64, String)>,
    categories: Vec<Category>,
    category_order: HashMap<String, usize>,
    awards: Vec<model::Award>,
    pub tournaments: Vec<Tournament>,
    tournament_rank: HashMap<i64, usize>,
    pub states: HashMap<i64, TournamentState>,
    /// Awards from the rules (`compute_awards`), for awards still defined in the database.
    pub grants: Vec<Grant>,
    /// (tournament, round) -> nominations, in id order; `nomination_keys` keeps first-seen order.
    nominations: HashMap<(i64, i64), Vec<Nomination>>,
    nomination_keys: Vec<(i64, i64)>,
    meta: HashMap<String, String>,
    points_tables: HashMap<i64, Vec<(i64, i64)>>,
    nemesis_assignments: HashMap<(i64, i64), Vec<(i64, i64)>>,
    category_use: HashMap<String, i64>,
}

fn opt<T: Into<Value>>(v: Option<T>) -> Value {
    v.map_or(Value::Null, Into::into)
}

impl Exporter {
    pub fn new(conn: &Connection, avatars_dir: Option<&Path>) -> Result<Self> {
        let players = model::players(conn)?;
        let player_index = players.iter().enumerate().map(|(i, p)| (p.id, i)).collect();
        let slugs: Vec<&str> = players.iter().map(|p| p.slug.as_str()).collect();
        let uploads = avatar_files(avatars_dir, &slugs);
        // The picture file each player publishes: an upload, else their Discord picture (unless a link is set).
        let avatar_files = players
            .iter()
            .filter_map(|p| match player_picture(avatars_dir, &uploads, p)? {
                (PictureSource::Link, _) => None,
                (_, file) => Some((p.slug.clone(), file)),
            })
            .collect();
        let names: HashMap<i64, String> = players.iter().map(|p| (p.id, p.name.clone())).collect();

        let mut games = model::games(conn)?;
        for g in &mut games {
            g.categories.clear();
        }
        let game_index = games.iter().enumerate().map(|(i, g)| (g.id, i)).collect();
        let game_categories = {
            let mut stmt = conn.prepare("SELECT game_id, category_slug FROM game_categories")?;
            stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<Result<Vec<_>, _>>()?
        };

        let categories = model::categories(conn)?;
        let category_order = categories.iter().enumerate().map(|(i, c)| (c.slug.clone(), i)).collect();
        let awards = model::awards(conn)?;
        let tournaments = model::tournaments(conn)?;
        let tournament_rank = tournaments.iter().enumerate().map(|(i, t)| (t.id, i)).collect();
        let mut states = HashMap::new();
        for t in &tournaments {
            states.insert(t.id, compute_tournament(conn, t, &names)?);
        }
        // The status follows the games (see rules.rs).
        let tournaments: Vec<Tournament> =
            tournaments.into_iter().map(|t| Tournament { status: states[&t.id].row.status, ..t }).collect();

        let mut nominations: HashMap<(i64, i64), Vec<Nomination>> = HashMap::new();
        let mut nomination_keys = Vec::new();
        for n in model::all_nominations(conn)? {
            let key = (n.tournament_id, n.round_number);
            if !nominations.contains_key(&key) {
                nomination_keys.push(key);
            }
            nominations.entry(key).or_default().push(n);
        }

        let meta = model::meta(conn)?.into_iter().collect();

        let mut points_tables: HashMap<i64, Vec<(i64, i64)>> = HashMap::new();
        let mut nemesis_assignments: HashMap<(i64, i64), Vec<(i64, i64)>> = HashMap::new();
        for t in &tournaments {
            points_tables.insert(t.id, model::points_table(conn, t.id)?);
            for number in states[&t.id].rounds.keys() {
                let pairs = model::nemesis_assignments(conn, t.id, *number)?;
                if !pairs.is_empty() {
                    nemesis_assignments.insert((t.id, *number), pairs);
                }
            }
        }

        let category_use = {
            let mut stmt = conn.prepare(
                "SELECT category_slug, COUNT(DISTINCT tournament_id) FROM tournament_categories GROUP BY category_slug",
            )?;
            stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<Result<HashMap<_, _>, _>>()?
        };

        let grants: Vec<Grant> = crate::rules::compute_awards(conn, &tournaments, &states)?
            .into_iter()
            .enumerate()
            .filter_map(|(i, g)| {
                let award = awards.iter().find(|a| a.slug == g.award)?;
                Some(Grant {
                    id: i as i64,
                    award_slug: g.award.to_string(),
                    player_id: g.player_id,
                    tournament_id: g.tournament_id,
                    detail: g.detail,
                    sort_order: award.sort_order,
                })
            })
            .collect();

        Ok(Self {
            players,
            player_index,
            avatar_files,
            games,
            game_index,
            game_categories,
            categories,
            category_order,
            awards,
            tournaments,
            tournament_rank,
            states,
            grants,
            nominations,
            nomination_keys,
            meta,
            points_tables,
            nemesis_assignments,
            category_use,
        })
    }

    // ---- refs

    fn player(&self, pid: i64) -> &Player {
        &self.players[self.player_index[&pid]]
    }

    fn name(&self, pid: i64) -> &str {
        &self.player(pid).name
    }

    /// A picture hosted with the data wins over avatar_url; relative paths resolve against the data URL.
    pub fn avatar_url(&self, pid: i64) -> Option<String> {
        let p = self.player(pid);
        match self.avatar_files.get(&p.slug) {
            Some(local) => Some(format!("avatars/{local}")),
            None => p.avatar_url.clone(),
        }
    }

    fn player_ref(&self, pid: Option<i64>) -> Value {
        let Some(pid) = pid else { return Value::Null };
        let p = self.player(pid);
        let mut r = Map::new();
        r.insert("slug".into(), p.slug.clone().into());
        r.insert("name".into(), p.name.clone().into());
        if let Some(avatar) = self.avatar_url(pid).filter(|a| !a.is_empty()) {
            r.insert("avatarUrl".into(), avatar.into());
        }
        Value::Object(r)
    }

    fn game_ref(&self, gid: Option<i64>) -> Value {
        let Some(gid) = gid else { return Value::Null };
        let g = &self.games[self.game_index[&gid]];
        match &g.url {
            Some(url) => json!({"slug": g.slug, "name": g.name, "url": url}),
            None => json!({"slug": g.slug, "name": g.name}),
        }
    }

    fn tournament_ref(t: &Tournament) -> Value {
        json!({"slug": t.slug, "name": t.name})
    }

    fn tournament_by_id(&self, tid: i64) -> &Tournament {
        &self.tournaments[self.tournament_rank[&tid]]
    }

    fn rank_of(&self, tid: Option<i64>) -> i64 {
        tid.and_then(|t| self.tournament_rank.get(&t)).map_or(-1, |r| *r as i64)
    }

    fn noms(&self, tid: i64, number: i64) -> &[Nomination] {
        self.nominations.get(&(tid, number)).map_or(&[], Vec::as_slice)
    }

    // ---- selection / rounds

    fn selection(&self, state: &TournamentState, number: i64, category: &str) -> Value {
        let selected = state.rounds.get(&number).and_then(|r| r.game_id);
        let nominations: Vec<Value> = self
            .noms(state.id(), number)
            .iter()
            .map(|n| {
                json!({
                    "game": self.game_ref(Some(n.game_id)),
                    "nominatedBy": self.player_ref(n.player_id),
                    "source": n.source.as_str(),
                    "entries": n.entries,
                })
            })
            .collect();
        json!({
            "status": if selected.is_some() { "drawn" } else { "pending" },
            "category": category,
            "nominations": nominations,
            "selected": self.game_ref(selected),
        })
    }

    fn game_result(&self, state: &TournamentState, rnd: &RoundState) -> Value {
        let mut results: Vec<_> = rnd.results.iter().collect();
        results.sort_by(|a, b| {
            a.placement.cmp(&b.placement).then_with(|| self.name(a.player_id).cmp(self.name(b.player_id)))
        });
        let results: Vec<Value> = results
            .iter()
            .map(|r| {
                json!({
                    "player": self.player_ref(Some(r.player_id)),
                    "placement": r.placement,
                    "points": r.points,
                    "basePoints": r.base_points,
                    "nemesisBonus": r.nemesis_bonus,
                    "doubleDown": r.double_down,
                    "nemesis": self.player_ref(r.nemesis_id),
                    "defeatedNemesis": opt(r.defeated_nemesis),
                })
            })
            .collect();
        let mut doc = json!({
            "number": rnd.number,
            "category": rnd.category,
            "game": self.game_ref(rnd.game_id),
            "tournament": Self::tournament_ref(&state.row),
            "playedAt": rnd.played_at,
            "winner": self.player_ref(rnd.winner_id),
            "results": results,
            "newChallengers": rnd.new_challengers.iter().map(|p| self.player_ref(Some(*p))).collect::<Vec<_>>(),
            "decidedTournament": rnd.decided,
        });
        if !self.noms(state.id(), rnd.number).is_empty() {
            doc["selection"] = self.selection(state, rnd.number, &rnd.category);
        }
        doc
    }

    // ---- tournaments

    fn is_first_tournament(&self, pid: i64, tid: i64) -> bool {
        let first = self
            .states
            .iter()
            .filter(|(_, s)| s.players.contains_key(&pid))
            .map(|(t, _)| self.tournament_rank[t])
            .min();
        first == Some(self.tournament_rank[&tid])
    }

    fn grants_sorted(&self) -> Vec<&Grant> {
        let mut grants: Vec<&Grant> = self.grants.iter().collect();
        grants.sort_by_key(|g| (g.sort_order, g.id));
        grants
    }

    fn standings(&self, state: &TournamentState) -> Vec<Value> {
        let t = &state.row;
        let mut grants_by_player: HashMap<i64, Vec<&str>> = HashMap::new();
        for g in self.grants_sorted() {
            if g.tournament_id == Some(state.id()) {
                grants_by_player.entry(g.player_id).or_default().push(&g.award_slug);
            }
        }
        state
            .standings()
            .map(|p| {
                json!({
                    "rank": p.rank,
                    "player": self.player_ref(Some(p.player_id)),
                    "points": p.points,
                    "isChallenger": p.challenger_since.is_some(),
                    "challengerSinceGame": p.challenger_since,
                    "isChampion": Some(p.player_id) == state.champion_id,
                    "tokens": {
                        "doubleDown": (t.double_down_tokens - p.double_downs_used).max(0),
                        "nominate": (t.nominate_tokens - p.nominations_used).max(0),
                    },
                    "gamesPlayed": p.games_played,
                    "gamesWon": p.games_won,
                    "categoryWins": p.category_wins,
                    "awards": grants_by_player.get(&p.player_id).cloned().unwrap_or_default(),
                    "isFirstTournament": self.is_first_tournament(p.player_id, state.id()),
                })
            })
            .collect()
    }

    /// One entry per game: every game played, then (while live) the rest of the current pass
    /// through the category order. Categories repeat when the order starts over.
    fn category_progress(&self, state: &TournamentState) -> Vec<Value> {
        let len = state.positions.len() as i64;
        if len == 0 {
            return Vec::new();
        }
        let played = state.completed_rounds().map(|r| r.number).max().unwrap_or(0);
        let next = state.next_round();
        let last = match next {
            // Show the whole current lap, so the schedule ahead is visible.
            Some(n) => crate::rules::lap(&state.positions, n) * len,
            None => played,
        };
        (1..=last)
            .map(|number| {
                let rnd = state.rounds.get(&number);
                let done = rnd.is_some_and(|r| r.completed) && !state.ignored_rounds.contains(&number);
                let status = if done {
                    "completed"
                } else if next == Some(number) {
                    "active"
                } else {
                    "upcoming"
                };
                json!({
                    "order": number,
                    "category": crate::rules::category_for(&state.positions, number),
                    "lap": state.lap(number),
                    "status": status,
                    "gameNumber": rnd.map(|_| number),
                    "game": rnd.map_or(Value::Null, |r| self.game_ref(r.game_id)),
                    "winner": if done { self.player_ref(rnd.and_then(|r| r.winner_id)) } else { Value::Null },
                })
            })
            .collect()
    }

    fn tournament_awards(&self, state: &TournamentState) -> Vec<Value> {
        self.grants_sorted()
            .into_iter()
            .filter(|g| g.tournament_id == Some(state.id()))
            .map(|g| json!({"award": g.award_slug, "player": self.player_ref(Some(g.player_id)), "detail": g.detail}))
            .collect()
    }

    fn tournament_summary(&self, state: &TournamentState) -> Map<String, Value> {
        let t = &state.row;
        let completed = state.completed_rounds().count();
        let points_table: Vec<Value> = self.points_tables[&t.id]
            .iter()
            .map(|(placement, points)| json!({"placement": placement, "points": points}))
            .collect();
        let leaders: Vec<Value> = state
            .standings()
            .take(3)
            .map(|p| json!({"rank": p.rank, "player": self.player_ref(Some(p.player_id)), "points": p.points}))
            .collect();
        let Value::Object(map) = json!({
            "slug": t.slug,
            "name": t.name,
            "tags": t.tags.iter().map(|tag| crate::model::tag_label(tag)).collect::<Vec<_>>(),
            "startDate": t.start_date,
            "endDate": t.end_date,
            "location": t.location,
            "status": t.status.as_str(),
            "isDemo": t.is_demo,
            "challengerThreshold": t.challenger_threshold,
            "pointsTable": points_table,
            "playerCount": state.players.len(),
            "gamesPlayed": completed,
            // Distinct categories played; games loop through the order until someone wins.
            "categoriesCompleted": completed.min(state.positions.len()),
            "categoriesPlanned": state.positions.len(),
            "lap": state.next_round().or_else(|| state.completed_rounds().map(|r| r.number).max()).map(|n| state.lap(n)),
            "champion": self.player_ref(state.champion_id),
            "leaders": leaders,
            "awards": self.tournament_awards(state),
        }) else {
            unreachable!()
        };
        map
    }

    fn live_fields(&self, state: &TournamentState) -> Map<String, Value> {
        let value = match state.active_position() {
            None => json!({"currentCategory": null, "currentGame": null, "nextSelection": null, "nextNemeses": null}),
            Some((position, category)) => {
                let rnd = state.rounds.get(&position);
                let live = state.row.status == Status::Ongoing;
                let next_nemeses = if live {
                    let mut pairs: Vec<&(i64, i64)> = self
                        .nemesis_assignments
                        .get(&(state.id(), position))
                        .map(|v| v.iter().collect())
                        .unwrap_or_default();
                    pairs.sort_by_key(|(pid, _)| self.name(*pid).to_lowercase());
                    Value::Array(
                        pairs
                            .into_iter()
                            .map(|(p, n)| json!({"player": self.player_ref(Some(*p)), "nemesis": self.player_ref(Some(*n))}))
                            .collect(),
                    )
                } else {
                    Value::Null
                };
                json!({
                    "currentCategory": category,
                    "currentGame": rnd.map_or(Value::Null, |r| self.game_ref(r.game_id)),
                    "nextSelection": if live { self.selection(state, position, category) } else { Value::Null },
                    "nextNemeses": next_nemeses,
                })
            }
        };
        let Value::Object(map) = value else { unreachable!() };
        map
    }

    fn tournament_detail(&self, state: &TournamentState) -> Value {
        let mut doc = self.tournament_summary(state);
        doc.extend(self.live_fields(state));
        doc.insert("standings".into(), self.standings(state).into());
        doc.insert("categories".into(), self.category_progress(state).into());
        doc.insert(
            "games".into(),
            state.completed_rounds().map(|r| self.game_result(state, r)).collect::<Vec<_>>().into(),
        );
        Value::Object(doc)
    }

    fn by_status(&self, status: Status) -> Vec<&TournamentState> {
        self.tournaments.iter().filter(|t| t.status == status).map(|t| &self.states[&t.id]).collect()
    }

    /// The tournament on the homepage: ongoing, else next upcoming, else most recent completed.
    pub fn featured(&self) -> Option<&TournamentState> {
        self.by_status(Status::Ongoing)
            .last()
            .copied()
            .or_else(|| self.by_status(Status::Upcoming).first().copied())
            .or_else(|| self.by_status(Status::Completed).last().copied())
    }

    fn current_ongoing(&self) -> Option<&TournamentState> {
        self.by_status(Status::Ongoing).last().copied()
    }

    // ---- players

    fn mine(&self, pid: i64) -> Vec<(&Tournament, &TournamentState)> {
        self.tournaments
            .iter()
            .map(|t| (t, &self.states[&t.id]))
            .filter(|(_, s)| s.players.contains_key(&pid))
            .collect()
    }

    fn player_summary(&self, pid: i64) -> Map<String, Value> {
        let p = self.player(pid);
        let mine = self.mine(pid);
        let current = self.current_ongoing();
        let best = mine
            .iter()
            .filter(|(t, _)| t.status == Status::Completed)
            .map(|(_, s)| {
                let ps = &s.players[&pid];
                ps.finish.filter(|f| *f != 0).unwrap_or(ps.rank)
            })
            .min();
        let mut doc = json!({
            "slug": p.slug,
            "name": p.name,
            "initials": p.initials,
            "avatarUrl": self.avatar_url(pid),
            "firstTournament": mine.first().map_or(Value::Null, |(t, _)| Self::tournament_ref(t)),
            "tournamentsPlayed": mine.len(),
            "ultimateGamerWins": mine.iter().filter(|(_, s)| s.champion_id == Some(pid)).count(),
            "challengerAppearances": mine.iter().filter(|(_, s)| s.players[&pid].challenger_since.is_some()).count(),
            "gameWins": mine.iter().map(|(_, s)| s.players[&pid].games_won).sum::<i64>(),
            "podiumFinishes": mine.iter().map(|(_, s)| s.players[&pid].podiums).sum::<i64>(),
            "awardCount": self.grants.iter().filter(|g| g.player_id == pid).count(),
            // The crown counts as 1st: finish, not points rank.
            "bestPlacement": best,
            "current": null,
            "discord": p.discord_id.as_ref().map_or(Value::Null, |id| json!({
                "id": id,
                "username": p.discord_username,
                "displayName": p.discord_display_name,
            })),
        });
        if let Some(current) = current
            && let Some(ps) = current.players.get(&pid)
        {
            doc["current"] = json!({
                "tournament": Self::tournament_ref(&current.row),
                "rank": ps.rank,
                "points": ps.points,
            });
        }
        let Value::Object(map) = doc else { unreachable!() };
        map
    }

    fn player_profile(&self, pid: i64) -> Value {
        let mut mine = self.mine(pid);
        mine.reverse();

        let mut category_wins: Vec<(String, i64)> = Vec::new();
        let mut matches = Vec::new();
        let (mut defeated, mut defeated_by, mut times_assigned) = (0i64, 0i64, 0i64);
        let mut rivals: Vec<(i64, i64, i64)> = Vec::new(); // (rival, nemesisWins, nemesisLosses)
        let (mut dd_used, mut dd_wins, mut dd_bonus) = (0i64, 0i64, 0i64);

        fn rival(rivals: &mut Vec<(i64, i64, i64)>, id: i64) -> &mut (i64, i64, i64) {
            let index = match rivals.iter().position(|r| r.0 == id) {
                Some(i) => i,
                None => {
                    rivals.push((id, 0, 0));
                    rivals.len() - 1
                }
            };
            &mut rivals[index]
        }

        for (t, s) in &mine {
            for rnd in s.completed_rounds().rev() {
                for res in &rnd.results {
                    if res.player_id == pid {
                        if Some(res.player_id) == rnd.winner_id {
                            match category_wins.iter_mut().find(|(c, _)| *c == rnd.category) {
                                Some(entry) => entry.1 += 1,
                                None => category_wins.push((rnd.category.clone(), 1)),
                            }
                        }
                        if res.double_down {
                            dd_used += 1;
                            dd_wins += i64::from(res.placement == 1);
                            dd_bonus += res.points / 2;
                        }
                        if let Some(nemesis) = res.nemesis_id {
                            times_assigned += 1;
                            let r = rival(&mut rivals, nemesis);
                            if res.defeated_nemesis == Some(true) {
                                defeated += 1;
                                r.1 += 1;
                            }
                        }
                        matches.push(json!({
                            "tournament": Self::tournament_ref(t),
                            "gameNumber": rnd.number,
                            "category": rnd.category,
                            "game": self.game_ref(rnd.game_id),
                            "placement": res.placement,
                            "points": res.points,
                            "doubleDown": res.double_down,
                            "nemesis": self.player_ref(res.nemesis_id),
                            "defeatedNemesis": opt(res.defeated_nemesis),
                        }));
                    } else if res.nemesis_id == Some(pid) && res.defeated_nemesis == Some(true) {
                        defeated_by += 1;
                        rival(&mut rivals, res.player_id).2 += 1;
                    }
                }
            }
        }

        let token_noms: Vec<&Nomination> = self
            .nomination_keys
            .iter()
            .flat_map(|k| &self.nominations[k])
            .filter(|n| n.player_id == Some(pid) && n.source == Source::Token)
            .collect();
        let selected = token_noms
            .iter()
            .filter(|n| {
                self.states[&n.tournament_id].rounds.get(&n.round_number).is_some_and(|r| r.game_id == Some(n.game_id))
            })
            .count();

        let mut grants: Vec<&Grant> = self.grants.iter().filter(|g| g.player_id == pid).collect();
        grants.sort_by_key(|g| (-self.rank_of(g.tournament_id), g.sort_order, g.id));

        category_wins.sort_by_key(|(c, n)| (-n, self.category_order.get(c).copied().unwrap_or(99)));
        rivals.sort_by_key(|(id, _, _)| self.name(*id).to_lowercase());

        let nemesis = if times_assigned != 0 || defeated_by != 0 {
            json!({"defeated": defeated, "defeatedBy": defeated_by, "timesAssigned": times_assigned})
        } else {
            Value::Null
        };

        let mut doc = self.player_summary(pid);
        doc.insert(
            "tournaments".into(),
            mine.iter()
                .map(|(t, s)| {
                    let ps = &s.players[&pid];
                    json!({
                        "tournament": Self::tournament_ref(t),
                        "startDate": t.start_date,
                        "status": t.status.as_str(),
                        "placement": ps.rank,
                        "finish": ps.finish,
                        "points": ps.points,
                        "gamesWon": ps.games_won,
                        "podiums": ps.podiums,
                        "wasChallenger": ps.challenger_since.is_some(),
                        "wasChampion": s.champion_id == Some(pid),
                    })
                })
                .collect::<Vec<_>>()
                .into(),
        );
        doc.insert(
            "awards".into(),
            grants
                .iter()
                .map(|g| {
                    json!({
                        "award": g.award_slug,
                        "tournament": g.tournament_id.map_or(Value::Null, |t| Self::tournament_ref(self.tournament_by_id(t))),
                        "detail": g.detail,
                    })
                })
                .collect::<Vec<_>>()
                .into(),
        );
        doc.insert(
            "categoryWins".into(),
            category_wins.iter().map(|(c, n)| json!({"category": c, "wins": n})).collect::<Vec<_>>().into(),
        );
        doc.insert("nemesis".into(), nemesis);
        doc.insert("doubleDown".into(), json!({"used": dd_used, "wins": dd_wins, "bonusPoints": dd_bonus}));
        doc.insert("nominate".into(), json!({"used": token_noms.len(), "selected": selected}));
        doc.insert("matches".into(), matches.into());
        doc.insert(
            "rivals".into(),
            rivals
                .iter()
                .map(|(id, wins, losses)| {
                    json!({"player": self.player_ref(Some(*id)), "nemesisWins": wins, "nemesisLosses": losses})
                })
                .collect::<Vec<_>>()
                .into(),
        );
        Value::Object(doc)
    }

    // ---- library

    fn games_doc(&self) -> Value {
        let mut game_cats: HashMap<i64, Vec<&str>> = HashMap::new();
        for (gid, c) in &self.game_categories {
            game_cats.entry(*gid).or_default().push(c);
        }
        let mut played: HashMap<i64, Vec<(usize, i64, &Tournament)>> = HashMap::new();
        for t in &self.tournaments {
            for rnd in self.states[&t.id].completed_rounds() {
                if let Some(gid) = rnd.game_id {
                    played.entry(gid).or_default().push((self.tournament_rank[&t.id], rnd.number, t));
                }
            }
        }
        let mut nominated: HashMap<i64, i64> = HashMap::new();
        for noms in self.nominations.values() {
            for n in noms {
                *nominated.entry(n.game_id).or_default() += 1;
            }
        }

        let games: Vec<Value> = self
            .games
            .iter()
            .map(|g| {
                let mut plays = played.get(&g.id).cloned().unwrap_or_default();
                plays.sort_by_key(|(rank, number, _)| (*rank, *number));
                let mut cats = game_cats.get(&g.id).cloned().unwrap_or_default();
                cats.sort_by_key(|c| self.category_order.get(*c).copied().unwrap_or(99));
                let last_played = plays.last().map_or(Value::Null, |(_, number, t)| {
                    json!({
                        "tournament": Self::tournament_ref(t),
                        "gameNumber": number,
                        "playedAt": self.states[&t.id].rounds[number].played_at,
                    })
                });
                json!({
                    "slug": g.slug,
                    "name": g.name,
                    "categories": cats,
                    "description": g.description,
                    "minPlayers": g.min_players,
                    "maxPlayers": g.max_players,
                    "timesPlayed": plays.len(),
                    "timesNominated": nominated.get(&g.id).copied().unwrap_or(0),
                    "lastPlayed": last_played,
                    "available": g.available,
                    "url": g.url,
                })
            })
            .collect();
        json!({"games": games})
    }

    // ---- share pages

    fn site_name(&self) -> String {
        self.meta.get("site_name").cloned().unwrap_or_else(|| "The Ultimate Gamer".into())
    }

    pub fn site_url(&self) -> String {
        with_slash(self.meta.get("site_url").filter(|s| !s.is_empty()).map_or(DEFAULT_SITE_URL, String::as_str))
    }

    pub fn data_url(&self) -> String {
        match self.meta.get("data_url").filter(|s| !s.is_empty()) {
            Some(url) => with_slash(url),
            None => format!("{}data/", self.site_url()),
        }
    }

    fn absolute_avatar(&self, pid: Option<i64>) -> Option<String> {
        let avatar = self.avatar_url(pid?).filter(|a| !a.is_empty())?;
        if avatar.starts_with("https://") {
            return Some(avatar);
        }
        if avatar.contains(':') || avatar.starts_with('/') { None } else { Some(self.data_url() + &avatar) }
    }

    fn player_description(&self, pid: i64) -> String {
        let s = self.player_summary(pid);
        let int = |key: &str| s[key].as_i64().unwrap_or(0);
        let mut parts = Vec::new();
        if int("ultimateGamerWins") != 0 {
            parts.push(format!("{}× Ultimate Gamer", int("ultimateGamerWins")));
        }
        parts.push(plural(int("tournamentsPlayed"), "tournament"));
        if let Some(best) = s["bestPlacement"].as_i64().filter(|b| *b != 0) {
            parts.push(format!("best finish {}", ordinal(best)));
        }
        parts.push(plural(int("gameWins"), "game win"));
        if int("awardCount") != 0 {
            parts.push(plural(int("awardCount"), "award"));
        }
        if let Some(c) = s["current"].as_object() {
            parts.push(format!(
                "live: {} in points ({} pts) in {}",
                ordinal(c["rank"].as_i64().unwrap_or(0)),
                c["points"],
                c["tournament"]["name"].as_str().unwrap_or("")
            ));
        }
        parts.join(" · ")
    }

    fn tournament_description(&self, state: &TournamentState) -> String {
        let t = &state.row;
        let status = match t.status {
            Status::Ongoing => "Live now",
            Status::Upcoming => "Upcoming",
            Status::Completed => "Completed",
        };
        let mut parts = vec![status.to_string()];
        let dates = date_range(t.start_date.as_deref(), t.end_date.as_deref());
        if !dates.is_empty() {
            parts.push(dates);
        }
        parts.push(plural(state.players.len() as i64, "player"));
        let played = state.completed_rounds().count();
        if state.finished() {
            parts.push(plural(played as i64, "game"));
        } else if !state.positions.is_empty() {
            parts.push(format!("{} played", plural(played as i64, "game")));
        }
        if let Some(champion) = state.champion_id {
            parts.push(format!("Ultimate Gamer: {}", self.name(champion)));
        } else if let Some(first) = state.order.first()
            && state.completed_rounds().next().is_some()
        {
            parts.push(format!("leader: {} ({} pts)", self.name(*first), state.players[first].points));
        }
        parts.join(" · ")
    }

    /// Static HTML with link-preview tags that redirects to the website (path -> HTML).
    pub fn share_pages(&self) -> BTreeMap<String, String> {
        let site = self.site_name();
        let site_url = self.site_url();
        let mut pages = BTreeMap::new();
        for p in &self.players {
            pages.insert(
                format!("{SHARE_DIR}/players/{}/index.html", p.slug),
                share_page(
                    &site,
                    &p.name,
                    &self.player_description(p.id),
                    &format!("{site_url}players/{}", p.slug),
                    self.absolute_avatar(Some(p.id)).as_deref(),
                    "profile",
                ),
            );
        }
        for t in &self.tournaments {
            let state = &self.states[&t.id];
            pages.insert(
                format!("{SHARE_DIR}/tournaments/{}/index.html", t.slug),
                share_page(
                    &site,
                    &t.name,
                    &self.tournament_description(state),
                    &format!("{site_url}tournaments/{}", t.slug),
                    self.absolute_avatar(state.champion_id).as_deref(),
                    "website",
                ),
            );
        }
        pages
    }

    // ---- documents

    pub fn documents(&self) -> BTreeMap<String, Value> {
        let mut docs = BTreeMap::new();
        let ongoing = self.current_ongoing();
        docs.insert(
            "site.json".into(),
            json!({
                "name": self.site_name(),
                "tagline": self.meta.get("site_tagline"),
                "currentTournament": ongoing.map_or(Value::Null, |s| Self::tournament_ref(&s.row)),
                "demo": self.tournaments.iter().any(|t| t.is_demo) || self.players.iter().any(|p| p.is_demo),
            }),
        );

        let leaderboard = match self.featured() {
            Some(featured) => {
                let recent: Vec<Value> = featured
                    .completed_rounds()
                    .rev()
                    .take(RECENT_RESULTS)
                    .map(|r| self.game_result(featured, r))
                    .collect();
                let mut doc = Map::new();
                doc.insert("tournament".into(), Value::Object(self.tournament_summary(featured)));
                doc.extend(self.live_fields(featured));
                doc.insert("standings".into(), self.standings(featured).into());
                doc.insert("categories".into(), self.category_progress(featured).into());
                doc.insert("recentResults".into(), recent.into());
                Value::Object(doc)
            }
            None => json!({
                "tournament": null,
                "currentCategory": null,
                "currentGame": null,
                "nextSelection": null,
                "nextNemeses": null,
                "standings": [],
                "categories": [],
                "recentResults": [],
            }),
        };
        docs.insert("leaderboard.json".into(), leaderboard);

        let categories: Vec<Value> = self
            .categories
            .iter()
            .map(|c| {
                json!({
                    "slug": c.slug,
                    "name": c.name,
                    "icon": c.icon,
                    "description": c.description,
                    "tournamentsUsed": self.category_use.get(&c.slug).copied().unwrap_or(0),
                })
            })
            .collect();
        docs.insert("categories.json".into(), json!({"categories": categories}));

        let awards: Vec<Value> = self
            .awards
            .iter()
            .map(|a| {
                let mut grants: Vec<&Grant> = self.grants.iter().filter(|g| g.award_slug == a.slug).collect();
                grants.sort_by_key(|g| (-self.rank_of(g.tournament_id), g.id));
                let recipients: Vec<Value> = grants
                    .iter()
                    .map(|g| {
                        json!({
                            "award": g.award_slug,
                            "player": self.player_ref(Some(g.player_id)),
                            "tournament": g.tournament_id.map_or(Value::Null, |t| Self::tournament_ref(self.tournament_by_id(t))),
                            "detail": g.detail,
                        })
                    })
                    .collect();
                json!({
                    "slug": a.slug,
                    "name": a.name,
                    "icon": a.icon,
                    "description": a.description,
                    "scope": a.scope,
                    "recipients": recipients,
                })
            })
            .collect();
        docs.insert("awards.json".into(), json!({"awards": awards}));

        docs.insert("games/index.json".into(), self.games_doc());

        let players: Vec<Value> = self.players.iter().map(|p| Value::Object(self.player_summary(p.id))).collect();
        docs.insert("players/index.json".into(), json!({"players": players}));
        for p in &self.players {
            docs.insert(format!("players/{}.json", p.slug), json!({"player": self.player_profile(p.id)}));
        }

        let newest_first: Vec<Value> = self
            .tournaments
            .iter()
            .rev()
            .map(|t| Value::Object(self.tournament_summary(&self.states[&t.id])))
            .collect();
        docs.insert("tournaments/index.json".into(), json!({"tournaments": newest_first}));
        for t in &self.tournaments {
            docs.insert(
                format!("tournaments/{}.json", t.slug),
                json!({"tournament": self.tournament_detail(&self.states[&t.id])}),
            );
        }
        docs
    }
}

// ----------------------------------------------------------------------- output

#[derive(Debug, Default, Clone)]
pub struct ExportReport {
    pub written: Vec<String>,
    pub unchanged: Vec<String>,
    pub removed: Vec<String>,
}

pub fn envelope(data: &Value, generated_at: &str) -> Value {
    json!({"schemaVersion": SCHEMA_VERSION, "generatedAt": generated_at, "data": data})
}

/// Pretty JSON exactly like Python's `json.dumps(ensure_ascii=False, indent=2)` plus a newline.
pub fn dump(document: &Value) -> String {
    let mut out = serde_json::to_string_pretty(document).expect("serializable");
    out.push('\n');
    out
}

fn rel_path(base: &Path, path: &Path) -> String {
    path.strip_prefix(base)
        .unwrap_or(path)
        .components()
        .map(|c| c.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

/// Every file under `dir` (recursively), sorted.
pub fn walk_files(dir: &Path) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(entries) = fs::read_dir(&d) else { continue };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.is_file() {
                out.push(path);
            }
        }
    }
    out.sort();
    out
}

/// Write documents, keeping the previous generatedAt for unchanged files and removing stale ones.
pub fn write_documents(docs: &BTreeMap<String, Value>, out_dir: &Path, now: Option<&str>) -> Result<ExportReport> {
    let now = now.map_or_else(utc_now, str::to_string);
    let mut report = ExportReport::default();
    fs::create_dir_all(out_dir)?;

    for (rel, data) in docs {
        let target = out_dir.join(rel);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        if let Ok(text) = fs::read_to_string(&target)
            && let Ok(previous) = serde_json::from_str::<Value>(&text)
            && previous.get("schemaVersion") == Some(&json!(SCHEMA_VERSION))
            && previous.get("data") == Some(data)
        {
            report.unchanged.push(rel.clone());
            continue;
        }
        fs::write(&target, dump(&envelope(data, &now)))?;
        report.written.push(rel.clone());
    }

    for existing in walk_files(out_dir) {
        if existing.extension().is_none_or(|e| e != "json") {
            continue;
        }
        let rel = rel_path(out_dir, &existing);
        if docs.contains_key(&rel) {
            continue;
        }
        // Only remove documents this exporter generated.
        let ours = fs::read_to_string(&existing)
            .ok()
            .and_then(|t| serde_json::from_str::<Value>(&t).ok())
            .is_some_and(|v| v.get("schemaVersion").is_some());
        if ours {
            fs::remove_file(&existing)?;
            report.removed.push(rel);
        }
    }
    Ok(report)
}

/// Write share pages under out_dir/share/ and remove ones that are no longer exported.
pub fn sync_share_pages(pages: &BTreeMap<String, String>, out_dir: &Path) -> Result<ExportReport> {
    let mut report = ExportReport::default();
    for (rel, content) in pages {
        let target = out_dir.join(rel);
        if fs::read_to_string(&target).is_ok_and(|c| c == *content) {
            continue;
        }
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&target, content)?;
        report.written.push(rel.clone());
    }
    let share_root = out_dir.join(SHARE_DIR);
    for existing in walk_files(&share_root) {
        let rel = rel_path(out_dir, &existing);
        if !pages.contains_key(&rel) {
            fs::remove_file(&existing)?;
            report.removed.push(rel);
        }
    }
    remove_empty_dirs(&share_root);
    Ok(report)
}

fn remove_empty_dirs(dir: &Path) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        if entry.path().is_dir() {
            remove_empty_dirs(&entry.path());
            let _ = fs::remove_dir(entry.path()); // fails (and is kept) when not empty
        }
    }
}

/// Picture file name (`slug.ext`) for a supported extension, case-insensitively.
pub fn avatar_stem(name: &str) -> Option<&str> {
    let dot = name.rfind('.')?;
    let (stem, ext) = name.split_at(dot);
    (!stem.is_empty() && AVATAR_EXTENSIONS.contains(&ext.to_lowercase().as_str())).then_some(stem)
}

/// The file name of a player's picture copied from Discord (kept apart from uploaded pictures).
pub fn discord_picture_name(slug: &str) -> String {
    format!("{slug}.discord.png")
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PictureSource {
    Upload,
    Link,
    Discord,
}

impl PictureSource {
    pub fn label(self) -> &'static str {
        match self {
            Self::Upload => "uploaded picture",
            Self::Link => "picture link",
            Self::Discord => "Discord picture",
        }
    }
}

/// Which picture a player shows: an uploaded file, else a picture link, else their Discord
/// picture. Returns the file name in the avatars folder (or the link). None means initials.
pub fn player_picture(
    avatars_dir: Option<&Path>,
    uploads: &BTreeMap<String, String>,
    p: &Player,
) -> Option<(PictureSource, String)> {
    if let Some(file) = uploads.get(&p.slug) {
        return Some((PictureSource::Upload, file.clone()));
    }
    if let Some(url) = p.avatar_url.as_ref().filter(|u| !u.is_empty()) {
        return Some((PictureSource::Link, url.clone()));
    }
    let name = discord_picture_name(&p.slug);
    avatars_dir.filter(|d| d.join(&name).is_file()).map(|_| (PictureSource::Discord, name))
}

/// Delete every stored picture of a player.
pub fn remove_avatars(dir: &Path, slug: &str) {
    for ext in AVATAR_EXTENSIONS {
        let _ = std::fs::remove_file(dir.join(format!("{slug}{ext}")));
        let _ = std::fs::remove_file(dir.join(format!("{slug}{}", ext.to_uppercase())));
    }
}

/// Map player slug -> file name for pictures in avatars_dir named after an existing player.
pub fn avatar_files(avatars_dir: Option<&Path>, slugs: &[&str]) -> BTreeMap<String, String> {
    let mut found = BTreeMap::new();
    let Some(dir) = avatars_dir else { return found };
    let Ok(entries) = fs::read_dir(dir) else { return found };
    let mut names: Vec<String> = entries
        .flatten()
        .filter(|e| e.path().is_file())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    for name in names {
        if let Some(stem) = avatar_stem(&name)
            && slugs.contains(&stem)
        {
            found.entry(stem.to_string()).or_insert(name.clone());
        }
    }
    found
}

/// Copy player pictures to out_dir/avatars/ and remove pictures that are no longer used.
pub fn sync_avatars(
    files: &BTreeMap<String, String>,
    avatars_dir: Option<&Path>,
    out_dir: &Path,
) -> Result<ExportReport> {
    let mut report = ExportReport::default();
    let target_dir = out_dir.join("avatars");
    let wanted: Vec<&String> = files.values().collect();
    if let Some(src_dir) = avatars_dir
        && !wanted.is_empty()
    {
        fs::create_dir_all(&target_dir)?;
        for name in &wanted {
            let (source, target) = (src_dir.join(name), target_dir.join(name));
            let bytes = fs::read(&source)?;
            if fs::read(&target).ok().as_ref() != Some(&bytes) {
                fs::write(&target, &bytes)?;
                report.written.push(format!("avatars/{name}"));
            }
        }
    }
    if let Ok(entries) = fs::read_dir(&target_dir) {
        let mut existing: Vec<_> = entries.flatten().map(|e| e.path()).filter(|p| p.is_file()).collect();
        existing.sort();
        for path in existing {
            let name = path.file_name().unwrap_or_default().to_string_lossy().into_owned();
            if !wanted.contains(&&name) {
                fs::remove_file(&path)?;
                report.removed.push(format!("avatars/{name}"));
            }
        }
    }
    Ok(report)
}

/// Regenerate `out_dir` from the database: JSON documents, player pictures and share pages.
pub fn export(
    conn: &Connection,
    out_dir: &Path,
    avatars_dir: Option<&Path>,
    now: Option<&str>,
) -> Result<ExportReport> {
    let exporter = Exporter::new(conn, avatars_dir)?;
    let mut report = write_documents(&exporter.documents(), out_dir, now)?;
    let pictures = sync_avatars(&exporter.avatar_files, avatars_dir, out_dir)?;
    let shares = sync_share_pages(&exporter.share_pages(), out_dir)?;
    for extra in [pictures, shares] {
        report.written.extend(extra.written);
        report.removed.extend(extra.removed);
    }
    Ok(report)
}

// ---------------------------------------------------------------------- helpers

pub fn with_slash(url: &str) -> String {
    if url.ends_with('/') { url.to_string() } else { format!("{url}/") }
}

pub fn ordinal(n: i64) -> String {
    if (11..=13).contains(&(n % 100)) {
        return format!("{n}th");
    }
    let suffix = match n % 10 {
        1 => "st",
        2 => "nd",
        3 => "rd",
        _ => "th",
    };
    format!("{n}{suffix}")
}

pub fn plural(n: i64, word: &str) -> String {
    format!("{n} {word}{}", if n == 1 { "" } else { "s" })
}

/// "7 Nov 2025", or "7 Nov 2025 – 8 Nov 2025" for a range.
pub fn date_range(start: Option<&str>, end: Option<&str>) -> String {
    fn fmt(value: Option<&str>) -> String {
        const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
        let Some(value) = value else { return String::new() };
        let parts: Vec<&str> = value.split('-').collect();
        let [y, m, d] = parts.as_slice() else { return String::new() };
        let (Ok(y), Ok(m), Ok(d)) = (y.parse::<i32>(), m.parse::<u8>(), d.parse::<u8>()) else { return String::new() };
        let Ok(month) = time::Month::try_from(m) else { return String::new() };
        if time::Date::from_calendar_date(y, month, d).is_err() {
            return String::new();
        }
        format!("{d} {} {y:04}", MONTHS[m as usize - 1])
    }
    let (a, b) = (fmt(start), fmt(end));
    if !a.is_empty() && !b.is_empty() && a != b {
        format!("{a} – {b}")
    } else if !a.is_empty() {
        a
    } else {
        b
    }
}

pub fn escape_html(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#x27;"),
            c => out.push(c),
        }
    }
    out
}

/// A tiny page for link previews (Discord, WhatsApp…). Crawlers read the tags; people are redirected.
pub fn share_page(site: &str, title: &str, description: &str, target: &str, image: Option<&str>, kind: &str) -> String {
    let e = escape_html;
    let full_title = format!("{title} · {site}");
    let mut tags = vec![
        format!(r#"<meta property="og:site_name" content="{}">"#, e(site)),
        format!(r#"<meta property="og:type" content="{kind}">"#),
        format!(r#"<meta property="og:title" content="{}">"#, e(&full_title)),
        format!(r#"<meta property="og:description" content="{}">"#, e(description)),
        format!(r#"<meta name="description" content="{}">"#, e(description)),
        r#"<meta name="twitter:card" content="summary">"#.to_string(),
    ];
    if let Some(image) = image {
        tags.push(format!(r#"<meta property="og:image" content="{}">"#, e(image)));
    }
    let head = tags.join("\n\t\t");
    let (title, target) = (e(&full_title), e(target));
    format!(
        "<!doctype html>
<html lang=\"en\">
\t<head>
\t\t<meta charset=\"utf-8\">
\t\t<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">
\t\t<meta name=\"robots\" content=\"noindex\">
\t\t<title>{title}</title>
\t\t{head}
\t\t<meta http-equiv=\"refresh\" content=\"0; url={target}\">
\t</head>
\t<body>
\t\t<p><a href=\"{target}\">{title}</a></p>
\t</body>
</html>
"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn helpers() {
        assert_eq!(ordinal(1), "1st");
        assert_eq!(ordinal(12), "12th");
        assert_eq!(ordinal(23), "23rd");
        assert_eq!(plural(1, "game"), "1 game");
        assert_eq!(plural(2, "game"), "2 games");
        assert_eq!(date_range(Some("2025-11-07"), Some("2025-11-08")), "7 Nov 2025 – 8 Nov 2025");
        assert_eq!(date_range(Some("2025-11-07"), Some("2025-11-07")), "7 Nov 2025");
        assert_eq!(date_range(None, Some("bad")), "");
        assert_eq!(escape_html(r#"<a href="x">'&'</a>"#), "&lt;a href=&quot;x&quot;&gt;&#x27;&amp;&#x27;&lt;/a&gt;");
        assert_eq!(avatar_stem("tnoy.JPG"), Some("tnoy"));
        assert_eq!(avatar_stem("notes.txt"), None);
    }
}
