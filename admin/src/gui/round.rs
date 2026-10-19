//! Entering one game: pick the game, announce Nemeses, tap players in finishing order, finish.

use std::collections::HashMap;

use eframe::egui::{self, RichText, Vec2};
use egui_extras::{Column, TableBuilder};

use super::{Action, Confirm, Data, Page, Session, Tab, TournamentData, theme, tournaments};
use crate::model::Source;
use crate::repo::{self, ResultInput};
use crate::rules::TournamentState;

#[derive(Clone, Debug, PartialEq)]
struct Row {
    pid: i64,
    played: bool,
    placement: Option<i64>,
    base: Option<i64>,
    /// Base points typed by hand (not taken from the points table).
    base_edited: bool,
    double_down: bool,
    nemesis: Option<i64>,
}

pub struct Form {
    /// (tournament, round, data revision) the rows were loaded for.
    key: Option<(i64, i64, u64)>,
    rows: Vec<Row>,
    /// The rows differ from what's saved.
    dirty: bool,
    nom_game: Option<i64>,
    nom_source: Source,
    nom_player: Option<i64>,
    nom_entries: i64,
    preview: Option<(Vec<Row>, u64, Result<TournamentState, String>)>,
}

impl Default for Form {
    fn default() -> Self {
        Self {
            key: None,
            rows: Vec::new(),
            dirty: false,
            nom_game: None,
            nom_source: Source::Winner,
            nom_player: None,
            nom_entries: 1,
            preview: None,
        }
    }
}

impl Form {
    fn load(&mut self, d: &Data, t: &TournamentData, number: i64) {
        let results = t.round_results(number);
        let announced = t.round_nemeses(number);
        let mut participants = t.participants.clone();
        participants.sort_by_key(|p| d.player_name(*p).to_lowercase());
        self.rows = participants
            .into_iter()
            .map(|pid| {
                let r = results.iter().find(|r| r.player_id == pid);
                Row {
                    pid,
                    played: results.is_empty() || r.is_some(),
                    placement: r.map(|r| r.placement),
                    base: r.map(|r| r.base_points),
                    base_edited: r.is_some(),
                    double_down: r.is_some_and(|r| r.double_down),
                    nemesis: r.map_or_else(|| announced.get(&pid).copied(), |r| r.nemesis_player_id),
                }
            })
            .collect();
        self.dirty = false;
        self.key = Some((t.t.id, number, d.rev));
        self.preview = None;
        // The winner of the previous game usually makes the next pick.
        let previous = t.state.rounds.get(&(number - 1)).and_then(|r| r.winner_id);
        self.nom_player = previous.or(self.nom_player.filter(|p| t.participants.contains(p)));
        self.nom_source = if previous.is_some() { Source::Winner } else { Source::Vote };
        self.nom_entries = 1;
    }

    fn next_placement(&self) -> i64 {
        self.rows.iter().filter(|r| r.played).filter_map(|r| r.placement).max().unwrap_or(0) + 1
    }

    fn set_placement(&mut self, i: usize, placement: Option<i64>, points: &dyn Fn(i64) -> i64) {
        let row = &mut self.rows[i];
        row.placement = placement;
        if !row.base_edited {
            row.base = placement.map(points);
        }
        self.dirty = true;
    }

    fn inputs(&self) -> Vec<ResultInput> {
        self.rows
            .iter()
            .filter(|r| r.played)
            .map(|r| ResultInput {
                player_id: r.pid,
                placement: r.placement.unwrap_or(0),
                base_points: r.base.unwrap_or(0),
                double_down: r.double_down,
                nemesis_player_id: r.nemesis.filter(|n| *n != r.pid),
            })
            .collect()
    }

    fn complete(&self) -> bool {
        let played: Vec<&Row> = self.rows.iter().filter(|r| r.played).collect();
        played.len() >= 2 && played.iter().all(|r| r.placement.is_some() && r.base.is_some())
    }
}

pub fn show(
    ui: &mut egui::Ui,
    s: &Session,
    t: &TournamentData,
    number: i64,
    form: &mut Form,
    actions: &mut Vec<Action>,
) {
    let d = &s.data;
    let tid = t.t.id;
    let round = t.round(number);
    let completed = round.is_some_and(|r| r.completed);
    let category = t.category_at(number).unwrap_or_default().to_string();
    let reload = match form.key {
        Some((ft, fnum, rev)) => ft != tid || fnum != number || (rev != d.rev && !form.dirty),
        None => true,
    };
    if reload {
        form.load(d, t, number);
    }

    // ---- header
    ui.horizontal(|ui| {
        if ui.link(format!("⬅ {}", t.t.name)).clicked() {
            actions.push(Action::Go(Page::Tournament(tid, Tab::Overview)));
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let last = t.positions.last().map_or(number, |(p, _)| *p);
            if number < last && ui.button(format!("Game {} ➡", number + 1)).clicked() {
                actions.push(Action::Go(Page::Round(tid, number + 1)));
            }
            if number > 1 && ui.button(format!("⬅ Game {}", number - 1)).clicked() {
                actions.push(Action::Go(Page::Round(tid, number - 1)));
            }
        });
    });
    ui.heading(RichText::new(format!("Game {number} · {}", d.category_label(&category))).strong());
    if !completed {
        theme::hint(
            ui,
            "1. Pick the game  ·  2. Announce the Nemeses  ·  3. Tap players in finishing order  ·  4. Finish game",
        );
    }
    ui.add_space(4.0);

    if completed {
        outcome(ui, d, t, number, actions);
        ui.add_space(6.0);
    } else if t.state.champion_id.is_some() {
        theme::callout(ui, theme::DANGER, |ui| {
            ui.label("This tournament is already decided. Games after the deciding one don't count and are a validation error.");
        });
        ui.add_space(6.0);
    }

    ui.columns(2, |cols| {
        selection(&mut cols[0], d, t, number, &category, completed, form, actions);
        theme::card(&mut cols[1], |ui| {
            theme::section_title(ui, "Standings now");
            if t.state.order.is_empty() {
                ui.label(theme::muted("No players registered."));
            } else {
                let highlight = t.state.rounds.get(&number).map(|r| r.new_challengers.clone()).unwrap_or_default();
                tournaments::standings_table(ui, d, t, &t.state, usize::MAX, &highlight);
            }
        });
    });
    ui.add_space(8.0);

    if t.participants.len() < 2 {
        theme::callout(ui, theme::INFO, |ui| {
            ui.label("Register at least two players for this tournament first (Setup tab).");
        });
        return;
    }
    if !completed {
        nemeses(ui, d, t, number, form, actions);
        ui.add_space(8.0);
    }
    results(ui, s, t, number, completed, form, actions);
}

fn outcome(ui: &mut egui::Ui, d: &Data, t: &TournamentData, number: i64, actions: &mut Vec<Action>) {
    let Some(r) = t.state.rounds.get(&number) else { return };
    let tid = t.t.id;
    let color = if r.decided { theme::GOLD } else { theme::LIVE };
    theme::callout(ui, color, |ui| {
        let winner = r.winner_id.map(|w| d.player_name(w)).unwrap_or_default();
        if r.decided {
            ui.label(
                RichText::new(format!("👑 {winner} won as a Challenger — the Ultimate Gamer!")).size(20.0).strong(),
            );
            if t.t.status != crate::model::Status::Completed && theme::primary(ui, "Finish the tournament ➡").clicked()
            {
                actions.push(Action::Go(Page::Tournament(tid, Tab::Overview)));
            }
        } else {
            ui.label(RichText::new(format!("🏆 {winner} won game {number}.")).size(20.0).strong());
            if !r.new_challengers.is_empty() {
                let names: Vec<String> = r.new_challengers.iter().map(|p| d.player_name(*p)).collect();
                ui.label(
                    RichText::new(format!(
                        "⚔ New Challenger{}: {}",
                        if names.len() > 1 { "s" } else { "" },
                        names.join(", ")
                    ))
                    .color(theme::CHALLENGER)
                    .strong(),
                );
            }
            if let Some(next) = t.next_round().filter(|n| *n > number)
                && theme::primary(ui, &format!("Next: game {next} ➡")).clicked()
            {
                actions.push(Action::Go(Page::Round(tid, next)));
            }
        }
    });
}

#[allow(clippy::too_many_arguments)]
fn selection(
    ui: &mut egui::Ui,
    d: &Data,
    t: &TournamentData,
    number: i64,
    category: &str,
    completed: bool,
    form: &mut Form,
    actions: &mut Vec<Action>,
) {
    let tid = t.t.id;
    let current = t.round(number).and_then(|r| r.game_id);
    // Games in this category first, then the rest of the library.
    let mut games: Vec<(i64, String)> = d
        .games
        .iter()
        .filter(|g| g.available && g.categories.iter().any(|c| c == category))
        .map(|g| (g.id, format!("★ {}", g.name)))
        .collect();
    games.extend(
        d.games
            .iter()
            .filter(|g| !(g.available && g.categories.iter().any(|c| c == category)))
            .map(|g| (g.id, if g.available { g.name.clone() } else { format!("{} (unavailable)", g.name) })),
    );

    theme::card(ui, |ui| {
        theme::section_title(ui, "1 · Game");
        let nominations = t.round_nominations(number);
        if nominations.is_empty() {
            theme::hint(
                ui,
                "No nominations recorded (optional). Record them to draw at random, or just pick the game below.",
            );
        }
        let (_, nom_used) = token_counts(t, number);
        for n in &nominations {
            ui.horizontal(|ui| {
                let picked = current == Some(n.game_id);
                let name = RichText::new(d.game_name(n.game_id)).strong();
                ui.label(if picked { name.color(theme::GOLD) } else { name });
                let who = n.player_id.map(|p| format!(" · {}", d.player_name(p))).unwrap_or_default();
                let entries = if n.entries > 1 { format!(" ×{}", n.entries) } else { String::new() };
                theme::hint(ui, format!("{}{who}{entries}", n.source.label()));
                if picked {
                    theme::badge(ui, "Playing", theme::GOLD);
                }
                if !completed {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button("✖").on_hover_text("Remove nomination").clicked() {
                            let id = n.id;
                            actions
                                .push(Action::db("Nomination removed", move |c| repo::remove_nomination(c, tid, id)));
                        }
                    });
                }
            });
        }
        if !completed {
            ui.horizontal_wrapped(|ui| {
                theme::pick(ui, ("nom-game", number), &mut form.nom_game, &games, None, 200.0);
                egui::ComboBox::from_id_salt(("nom-source", number))
                    .width(140.0)
                    .selected_text(form.nom_source.label())
                    .show_ui(ui, |ui| {
                        for source in Source::ALL {
                            ui.selectable_value(&mut form.nom_source, source, source.label());
                        }
                    });
                if form.nom_source != Source::Vote {
                    let players: Vec<(i64, String)> = t
                        .participants
                        .iter()
                        .map(|p| {
                            let left = t.t.nominate_tokens - nom_used.get(p).copied().unwrap_or(0);
                            let label = if form.nom_source == Source::Token {
                                format!("{} ({left} left)", d.player_name(*p))
                            } else {
                                d.player_name(*p)
                            };
                            (*p, label)
                        })
                        .collect();
                    theme::pick(ui, ("nom-player", number), &mut form.nom_player, &players, None, 150.0);
                }
                ui.label("×");
                ui.add(egui::DragValue::new(&mut form.nom_entries).range(1..=20))
                    .on_hover_text("Entries (tickets in the draw)");
                let ready = form.nom_game.is_some() && (form.nom_source == Source::Vote || form.nom_player.is_some());
                if ui.add_enabled(ready, egui::Button::new("+ Add")).clicked()
                    && let Some(game) = form.nom_game
                {
                    let (source, player, entries) = (form.nom_source, form.nom_player, form.nom_entries);
                    actions.push(Action::db("Nomination added", move |c| {
                        repo::add_nomination(c, tid, number, game, source, player, entries)
                    }));
                    form.nom_game = None;
                }
            });
            if !nominations.is_empty() {
                let label = if current.is_some() { "🎲 Draw again" } else { "🎲 Draw at random" };
                if theme::primary(ui, label).on_hover_text("Weighted by entries").clicked() {
                    let names: HashMap<i64, String> = d.games.iter().map(|g| (g.id, g.name.clone())).collect();
                    actions.push(Action::db_msg("Game drawn", move |c| {
                        let game = repo::draw_game(c, tid, number)?;
                        Ok(format!("🎲 Drawn: {}", names.get(&game).cloned().unwrap_or_default()))
                    }));
                }
            }
            ui.separator();
        }
        ui.horizontal(|ui| {
            ui.label(theme::muted("Game being played"));
            let mut chosen = current;
            ui.add_enabled_ui(!completed, |ui| {
                if theme::pick(ui, ("round-game", number), &mut chosen, &games, Some("— not decided —"), 240.0) {
                    let label = chosen.map_or("Game cleared".to_string(), |g| format!("Playing {}", d.game_name(g)));
                    actions.push(Action::db(label, move |c| repo::set_round_game(c, tid, number, chosen)));
                }
            });
        });
        theme::hint(ui, "★ = in this category. Missing a game? Add it on the Games page.");
    });
}

fn nemeses(ui: &mut egui::Ui, d: &Data, t: &TournamentData, number: i64, form: &Form, actions: &mut Vec<Action>) {
    let tid = t.t.id;
    let announced = t.round_nemeses(number);
    theme::card(ui, |ui| {
        ui.horizontal(|ui| {
            theme::section_title(ui, "2 · Nemeses");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("🎲 Draw at random").clicked() {
                    let action = Action::db("Nemeses drawn", move |c| repo::draw_nemeses(c, tid, number));
                    if announced.is_empty() {
                        actions.push(action);
                    } else {
                        actions.push(Action::Confirm(Confirm {
                            danger: false,
                            ..Confirm::new(
                                "Draw new Nemeses?",
                                "This replaces the pairs already announced.",
                                "Draw again",
                                action,
                            )
                        }));
                    }
                }
            });
        });
        theme::hint(
            ui,
            "Public on the website until the game is played. Anyone but yourself can be drawn, so Nemeses may be shared and needn't be mutual. Each change saves right away.",
        );
        let players: Vec<(i64, String)> = form.rows.iter().map(|r| (r.pid, d.player_name(r.pid))).collect();
        egui::Grid::new(("nemeses", number)).num_columns(4).spacing([12.0, 6.0]).show(ui, |ui| {
            for (i, row) in form.rows.iter().enumerate() {
                ui.label(RichText::new(d.player_name(row.pid)).strong());
                let mut nemesis = announced.get(&row.pid).copied();
                let others: Vec<(i64, String)> = players.iter().filter(|(p, _)| *p != row.pid).cloned().collect();
                if theme::pick(ui, ("nemesis", number, row.pid), &mut nemesis, &others, Some("—"), 140.0) {
                    let mut pairs: Vec<(i64, Option<i64>)> =
                        form.rows.iter().map(|r| (r.pid, announced.get(&r.pid).copied())).collect();
                    pairs[i].1 = nemesis;
                    actions.push(Action::db("Nemesis saved", move |c| repo::set_nemeses(c, tid, number, &pairs)));
                }
                if i % 2 == 1 {
                    ui.end_row();
                }
            }
        });
    });
}

fn token_counts(t: &TournamentData, number: i64) -> (HashMap<i64, i64>, HashMap<i64, i64>) {
    let mut dd = HashMap::new();
    for (n, r) in &t.results {
        if *n != number && r.double_down {
            *dd.entry(r.player_id).or_insert(0) += 1;
        }
    }
    let mut nom = HashMap::new();
    for n in &t.nominations {
        if n.source == Source::Token
            && let Some(p) = n.player_id
        {
            *nom.entry(p).or_insert(0) += 1;
        }
    }
    (dd, nom)
}

fn results(
    ui: &mut egui::Ui,
    s: &Session,
    t: &TournamentData,
    number: i64,
    completed: bool,
    form: &mut Form,
    actions: &mut Vec<Action>,
) {
    let d = &s.data;
    let tid = t.t.id;
    let has_game = t.round(number).and_then(|r| r.game_id).is_some();
    let points = |placement: i64| t.points_for(placement);
    let (dd_used, _) = token_counts(t, number);

    // What the round would do to the standings: computed by the rules on a scratch copy.
    let preview_state = if completed {
        None
    } else if form.complete() {
        let fresh = form.preview.as_ref().is_some_and(|(rows, rev, _)| *rows == form.rows && *rev == d.rev);
        if !fresh {
            let result = s.preview_round(tid, number, &form.inputs()).map_err(|e| format!("{e:#}"));
            form.preview = Some((form.rows.clone(), d.rev, result));
        }
        form.preview.as_ref().and_then(|(_, _, r)| r.as_ref().ok().cloned())
    } else {
        None
    };
    let shown: &TournamentState = preview_state.as_ref().unwrap_or(&t.state);
    let round_state = shown.rounds.get(&number).filter(|r| r.completed);
    let points_of = |pid: i64| round_state.and_then(|r| r.results.iter().find(|x| x.player_id == pid));

    theme::card(ui, |ui| {
        ui.horizontal(|ui| {
            theme::section_title(ui, if completed { "Results" } else { "3 · Results" });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if completed {
                    let later_done = t.rounds.iter().any(|r| r.number > number && r.completed);
                    let resp = ui.add_enabled(!later_done, egui::Button::new("Reopen to edit"));
                    if resp.on_disabled_hover_text("Only the most recent finished game can be reopened").clicked() {
                        actions.push(Action::db("Game reopened — you can edit the results again", move |c| {
                            repo::reopen_round(c, tid, number)
                        }));
                    }
                } else if ui.button("Clear order").clicked() {
                    for r in &mut form.rows {
                        r.placement = None;
                        r.base_edited = false;
                        r.base = None;
                    }
                    form.dirty = true;
                }
            });
        });

        if !completed {
            theme::hint(
                ui,
                "Tap players in finishing order: 1st first. Tap the last one again to undo. For ties, edit the place numbers.",
            );
            let next = form.next_placement();
            ui.horizontal_wrapped(|ui| {
                for i in 0..form.rows.len() {
                    let row = form.rows[i].clone();
                    if !row.played {
                        continue;
                    }
                    let text = match row.placement {
                        Some(p) => format!("{}  {}", crate::export::ordinal(p), d.player_name(row.pid)),
                        None => d.player_name(row.pid),
                    };
                    let (fill, color) = match row.placement {
                        Some(1) => (theme::GOLD, theme::ON_GOLD),
                        Some(_) => (theme::GOLD_SOFT, theme::GOLD),
                        None => (theme::SURFACE_2, theme::TEXT),
                    };
                    let button = egui::Button::new(RichText::new(text).size(17.0).strong().color(color))
                        .fill(fill)
                        .min_size(Vec2::new(150.0, 48.0));
                    if ui.add(button).clicked() {
                        match row.placement {
                            None => form.set_placement(i, Some(next), &points),
                            Some(p) if p == next - 1 => form.set_placement(i, None, &points),
                            Some(_) => {}
                        }
                    }
                }
            });
            ui.add_space(6.0);
        }

        let max_points = t.max_points();
        TableBuilder::new(ui)
            .id_salt(("results", number))
            .striped(true)
            .vscroll(false)
            .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
            .column(Column::exact(60.0))
            .column(Column::remainder().at_least(130.0))
            .column(Column::exact(70.0))
            .column(Column::exact(90.0))
            .column(Column::exact(120.0))
            .column(Column::exact(160.0))
            .column(Column::exact(110.0))
            .header(24.0, |mut h| {
                for title in ["Played", "Player", "Place", "Base pts", "Double Down", "Nemesis", "= Points"] {
                    h.col(|ui| {
                        ui.label(theme::muted(title).small());
                    });
                }
            })
            .body(|mut body| {
                let players: Vec<(i64, String)> = form.rows.iter().map(|r| (r.pid, d.player_name(r.pid))).collect();
                let mut order: Vec<usize> = (0..form.rows.len()).collect();
                if completed {
                    order.sort_by_key(|i| (!form.rows[*i].played, form.rows[*i].placement.unwrap_or(999)));
                }
                for i in order {
                    body.row(32.0, |mut row| {
                        let editable = !completed;
                        let r = &mut form.rows[i];
                        let pid = r.pid;
                        let mut changed = false;
                        row.col(|ui| {
                            changed |= ui
                                .add_enabled(editable, egui::Checkbox::without_text(&mut r.played))
                                .on_hover_text("Took part in this game")
                                .changed();
                        });
                        let active = editable && r.played;
                        row.col(|ui| {
                            let name = RichText::new(d.player_name(pid)).strong();
                            ui.label(if r.played { name } else { name.color(theme::SUBTLE).strikethrough() });
                        });
                        row.col(|ui| {
                            let mut place = r.placement.unwrap_or(0);
                            let resp = ui.add_enabled(
                                active,
                                egui::DragValue::new(&mut place)
                                    .range(0..=99)
                                    .custom_formatter(|v, _| if v < 1.0 { "–".into() } else { format!("{v}") }),
                            );
                            if resp.changed() {
                                r.placement = (place >= 1).then_some(place);
                                if !r.base_edited {
                                    r.base = r.placement.map(points);
                                }
                                changed = true;
                            }
                        });
                        row.col(|ui| {
                            let mut base = r.base.unwrap_or(0);
                            let resp = ui.add_enabled(active, egui::DragValue::new(&mut base).range(0..=999));
                            if resp.changed() {
                                r.base = Some(base);
                                r.base_edited = true;
                                changed = true;
                            }
                            if max_points.is_some_and(|m| base > m) {
                                ui.label(RichText::new("⚠").color(theme::DANGER))
                                    .on_hover_text("More than any placement gives — typo?");
                            }
                        });
                        row.col(|ui| {
                            let left = t.t.double_down_tokens - dd_used.get(&pid).copied().unwrap_or(0);
                            let can = r.double_down || left > 0;
                            let label = if r.double_down { "×2".to_string() } else { format!("{left} left") };
                            changed |=
                                ui.add_enabled(active && can, egui::Checkbox::new(&mut r.double_down, label)).changed();
                        });
                        row.col(|ui| {
                            let others: Vec<(i64, String)> =
                                players.iter().filter(|(p, _)| *p != pid).cloned().collect();
                            ui.add_enabled_ui(active, |ui| {
                                changed |= theme::pick(
                                    ui,
                                    ("res-nemesis", number, pid),
                                    &mut r.nemesis,
                                    &others,
                                    Some("—"),
                                    130.0,
                                );
                            });
                        });
                        row.col(|ui| {
                            if let (true, Some(res)) = (r.played, points_of(pid)) {
                                let mut text = RichText::new(res.points.to_string()).strong().size(17.0);
                                if res.placement == 1 {
                                    text = text.color(theme::GOLD);
                                }
                                let detail = format!(
                                    "{} base{}{}",
                                    res.base_points,
                                    if res.nemesis_bonus > 0 { " + 1 Nemesis beaten" } else { "" },
                                    if res.double_down { ", ×2 Double Down" } else { "" }
                                );
                                ui.label(text).on_hover_text(detail);
                                if res.defeated_nemesis == Some(true) {
                                    ui.label(RichText::new("😈").small()).on_hover_text("Beat their Nemesis");
                                }
                            } else if r.played {
                                ui.label(theme::muted("–"));
                            }
                        });
                        if changed {
                            form.dirty = true;
                        }
                    });
                }
            });

        if completed {
            return;
        }
        ui.add_space(6.0);

        // What finishing would do, from the preview.
        if let (Some(state), Some(r)) = (preview_state.as_ref(), round_state) {
            let names = |ids: &[i64]| ids.iter().map(|p| d.player_name(*p)).collect::<Vec<_>>().join(", ");
            if r.decided {
                theme::callout(ui, theme::GOLD, |ui| {
                    ui.label(
                        RichText::new(format!(
                            "If you finish now: 👑 {} becomes the Ultimate Gamer and the tournament ends.",
                            names(&r.winner_id.into_iter().collect::<Vec<_>>())
                        ))
                        .strong(),
                    );
                });
            } else if !r.new_challengers.is_empty() {
                theme::callout(ui, theme::CHALLENGER, |ui| {
                    ui.label(format!(
                        "If you finish now: ⚔ {} become{} Challenger.",
                        names(&r.new_challengers),
                        if r.new_challengers.len() == 1 { "s" } else { "" }
                    ));
                });
            }
            let leader = state.order.first().map(|p| (d.player_name(*p), state.players[p].points));
            if let Some((name, pts)) = leader {
                theme::hint(ui, format!("Leader after this game: {name} with {pts} points."));
            }
        } else if let Some((_, _, Err(e))) = &form.preview {
            theme::hint(ui, format!("Preview unavailable: {e}"));
        }

        let played: Vec<&Row> = form.rows.iter().filter(|r| r.played).collect();
        let winners = played.iter().filter(|r| r.placement == Some(1)).count();
        let missing = played.iter().filter(|r| r.placement.is_none()).count();
        let problem = if !has_game {
            Some("Choose the game first (step 1).".to_string())
        } else if played.len() < 2 {
            Some("At least two players must take part.".to_string())
        } else if missing > 0 {
            Some(format!("{missing} player(s) still need a place."))
        } else if winners != 1 {
            Some("Exactly one player must be 1st.".to_string())
        } else {
            None
        };
        ui.horizontal(|ui| {
            let inputs = form.inputs();
            if theme::big_primary(ui, problem.is_none(), "✔  Finish game").clicked() {
                let rows = inputs.clone();
                form.dirty = false;
                actions.push(Action::db(format!("Game {number} finished — standings updated"), move |c| {
                    repo::save_results(c, tid, number, &rows, true)
                }));
            }
            if ui
                .add_enabled(form.dirty, egui::Button::new("Save draft"))
                .on_hover_text("Keep these results without finishing the game")
                .clicked()
            {
                form.dirty = false;
                actions.push(Action::db("Draft saved", move |c| repo::save_results(c, tid, number, &inputs, false)));
            }
            match &problem {
                Some(p) => {
                    ui.label(RichText::new(p).color(theme::GOLD));
                }
                None => {
                    theme::hint(ui, "Finishing updates the standings, Challengers and the champion.");
                }
            }
        });
        if form.dirty {
            theme::hint(ui, "Unsaved results: they're kept while you use this page; Save draft keeps them for later.");
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn form(players: &[i64]) -> Form {
        let rows = players
            .iter()
            .map(|pid| Row {
                pid: *pid,
                played: true,
                placement: None,
                base: None,
                base_edited: false,
                double_down: false,
                nemesis: None,
            })
            .collect();
        Form { rows, ..Form::default() }
    }

    const TABLE: [i64; 3] = [6, 4, 3];
    fn points(placement: i64) -> i64 {
        TABLE.get(placement as usize - 1).copied().unwrap_or(0)
    }

    #[test]
    fn tapping_assigns_places_and_points_in_order() {
        let mut f = form(&[10, 20, 30]);
        f.set_placement(1, Some(f.next_placement()), &points); // player 20 first
        f.set_placement(0, Some(f.next_placement()), &points);
        f.set_placement(2, Some(f.next_placement()), &points);
        let got: Vec<(i64, i64, i64)> = f.inputs().iter().map(|r| (r.player_id, r.placement, r.base_points)).collect();
        assert_eq!(got, [(10, 2, 4), (20, 1, 6), (30, 3, 3)]);
        assert!(f.complete() && f.dirty);
    }

    #[test]
    fn hand_edited_points_survive_reordering_and_absent_players_are_left_out() {
        let mut f = form(&[10, 20, 30]);
        f.rows[2].played = false;
        f.set_placement(0, Some(1), &points);
        f.rows[0].base = Some(9);
        f.rows[0].base_edited = true;
        f.set_placement(0, Some(2), &points);
        assert_eq!(f.rows[0].base, Some(9), "typed points are kept");
        assert!(!f.complete(), "player 20 has no place yet");
        f.set_placement(1, Some(1), &points);
        assert_eq!(f.inputs().len(), 2);
        assert!(f.complete());
    }

    #[test]
    fn a_player_is_never_their_own_nemesis() {
        let mut f = form(&[10, 20]);
        f.rows[0].nemesis = Some(10);
        f.rows[1].nemesis = Some(10);
        let inputs = f.inputs();
        assert_eq!(inputs[0].nemesis_player_id, None);
        assert_eq!(inputs[1].nemesis_player_id, Some(10));
    }
}
