//! Entering one game: tick the games that can be picked, draw the game and the Nemeses in one
//! click, then enter where everyone finished. Points, Nemesis bonuses, Challengers, the champion
//! and awards all follow from the rules.

use std::collections::HashMap;

use eframe::egui::{self, RichText, Vec2};
use egui_extras::{Column, TableBuilder};

use super::{Action, Data, Page, Session, Tab, TournamentData, theme, tournaments};
use crate::model::Source;
use crate::repo::{self, Candidate, ResultInput};
use crate::rules::TournamentState;

#[derive(Clone, Debug, PartialEq)]
struct Row {
    pid: i64,
    played: bool,
    placement: Option<i64>,
    double_down: bool,
}

#[derive(Default)]
pub struct Form {
    /// (tournament, round, data revision) the form was loaded for.
    key: Option<(i64, i64, u64)>,
    rows: Vec<Row>,
    /// The results differ from what's saved.
    dirty: bool,
    /// Games ticked for the draw.
    candidates: Vec<Candidate>,
    show_other_games: bool,
    /// Picking again although a game is drawn.
    redraw: bool,
    preview: Option<(Vec<Row>, u64, Result<TournamentState, String>)>,
}

impl Form {
    fn load(&mut self, d: &Data, t: &TournamentData, number: i64) {
        let results = t.round_results(number);
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
                    double_down: r.is_some_and(|r| r.double_down),
                }
            })
            .collect();
        self.candidates = t
            .round_nominations(number)
            .iter()
            .map(|n| Candidate { game: n.game_id, source: n.source, player: n.player_id })
            .collect();
        self.dirty = false;
        self.redraw = false;
        self.key = Some((t.t.id, number, d.rev));
        self.preview = None;
    }

    fn next_placement(&self) -> i64 {
        self.rows.iter().filter(|r| r.played).filter_map(|r| r.placement).max().unwrap_or(0) + 1
    }

    /// Results to save. Base points come from the tournament's points table and the Nemesis is
    /// the one drawn before the game.
    fn inputs(&self, points: &dyn Fn(i64) -> i64, nemesis: &HashMap<i64, i64>) -> Vec<ResultInput> {
        self.rows
            .iter()
            .filter(|r| r.played)
            .map(|r| {
                let placement = r.placement.unwrap_or(0);
                ResultInput {
                    player_id: r.pid,
                    placement,
                    base_points: if placement >= 1 { points(placement) } else { 0 },
                    double_down: r.double_down,
                    nemesis_player_id: nemesis.get(&r.pid).copied().filter(|n| *n != r.pid),
                }
            })
            .collect()
    }

    fn complete(&self) -> bool {
        let played: Vec<&Row> = self.rows.iter().filter(|r| r.played).collect();
        played.len() >= 2 && played.iter().all(|r| r.placement.is_some())
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
            let last = t.next_round().unwrap_or(0).max(t.rounds.iter().map(|r| r.number).max().unwrap_or(0));
            if number < last && ui.button(format!("Game {} ➡", number + 1)).clicked() {
                actions.push(Action::Go(Page::Round(tid, number + 1)));
            }
            if number > 1 && ui.button(format!("⬅ Game {}", number - 1)).clicked() {
                actions.push(Action::Go(Page::Round(tid, number - 1)));
            }
        });
    });
    ui.horizontal(|ui| {
        ui.heading(RichText::new(format!("Game {number} · {}", d.category_label(&category))).strong());
        let lap = t.state.lap(number);
        if lap > 1 {
            theme::badge(ui, &format!("Lap {lap}"), theme::INFO)
                .on_hover_text("All categories have been played; the order started over.");
        }
    });
    if !completed {
        theme::hint(
            ui,
            "1. Tick the games that can be picked and draw  ·  2. Enter where everyone finished  ·  3. Finish game",
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

    theme::card(ui, |ui| {
        theme::section_title(ui, "Standings");
        if t.state.order.is_empty() {
            ui.label(theme::muted("No players registered."));
        } else {
            let highlight = t.state.rounds.get(&number).map(|r| r.new_challengers.clone()).unwrap_or_default();
            tournaments::standings_table(ui, d, t, &t.state, usize::MAX, &highlight);
        }
    });
    ui.add_space(8.0);

    if t.participants.len() < 2 {
        theme::callout(ui, theme::INFO, |ui| {
            ui.label("Register at least two players for this tournament first (Setup tab).");
        });
        return;
    }
    game_card(ui, d, t, number, &category, completed, form, actions);
    ui.add_space(8.0);
    if completed || round.and_then(|r| r.game_id).is_some() {
        results(ui, s, t, number, completed, form, actions);
    }
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
            ui.label("The tournament is complete. Its awards are on the Awards tab; publish to show them.");
            if theme::primary(ui, "See the awards ➡").clicked() {
                actions.push(Action::Go(Page::Tournament(tid, Tab::Awards)));
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

/// Nominate tokens each player used in the other rounds.
fn tokens_used_elsewhere(t: &TournamentData, number: i64) -> HashMap<i64, i64> {
    let mut used = HashMap::new();
    for n in &t.nominations {
        if n.source == Source::Token
            && n.round_number != number
            && let Some(p) = n.player_id
        {
            *used.entry(p).or_insert(0) += 1;
        }
    }
    used
}

fn source_label(d: &Data, c: &Candidate) -> String {
    match (c.source, c.player) {
        (Source::Winner, Some(p)) => format!("{}'s pick", d.player_name(p)),
        (Source::Token, Some(p)) => format!("{}'s Nominate token", d.player_name(p)),
        _ => "In the draw".to_string(),
    }
}

#[allow(clippy::too_many_arguments)]
fn game_card(
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
    let nemeses = t.round_nemeses(number);

    theme::card(ui, |ui| {
        theme::section_title(ui, "Game & Nemeses");
        if let Some(game) = current.filter(|_| completed || !form.redraw) {
            ui.label(RichText::new(format!("🎮 {}", d.game_name(game))).size(22.0).strong().color(theme::GOLD));
            let others: Vec<String> = t
                .round_nominations(number)
                .iter()
                .filter(|n| n.game_id != game)
                .map(|n| d.game_name(n.game_id))
                .collect();
            if !others.is_empty() {
                theme::hint(ui, format!("Drawn from {} games, also {}.", others.len() + 1, others.join(", ")));
            }
            ui.add_space(4.0);
            if nemeses.is_empty() {
                ui.label(theme::muted("No Nemeses drawn for this game."));
            } else {
                ui.label(theme::muted("Nemeses (public on the website until the game is played)"));
                let mut pairs: Vec<(String, String)> =
                    nemeses.iter().map(|(p, n)| (d.player_name(*p), d.player_name(*n))).collect();
                pairs.sort();
                egui::Grid::new(("nemesis-pairs", number))
                    .num_columns(3)
                    .spacing([28.0, 6.0])
                    .min_col_width(200.0)
                    .show(ui, |ui| {
                        for (i, (player, nemesis)) in pairs.iter().enumerate() {
                            let mut job = egui::text::LayoutJob::default();
                            let body = egui::TextFormat { color: theme::TEXT, ..Default::default() };
                            job.append(
                                player,
                                0.0,
                                egui::TextFormat { font_id: egui::FontId::proportional(15.0), ..body.clone() },
                            );
                            job.append(
                                &format!("  →  {nemesis}"),
                                0.0,
                                egui::TextFormat {
                                    font_id: egui::FontId::proportional(15.0),
                                    color: theme::MUTED,
                                    ..body
                                },
                            );
                            ui.label(job);
                            if i % 3 == 2 {
                                ui.end_row();
                            }
                        }
                    });
            }
            if !completed {
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    if nemeses.is_empty() && ui.button("🎲 Draw Nemeses").clicked() {
                        actions.push(Action::db("Nemeses drawn", move |c| repo::draw_nemeses(c, tid, number)));
                    }
                    if ui.button("Change or redraw…").clicked() {
                        form.redraw = true;
                    }
                });
            }
            return;
        }

        // ---- choosing the candidates
        theme::hint(
            ui,
            "Tick every game that can be picked, then draw: one click picks the game and everyone's Nemesis. \
             Mark a winner's pick or a Nominate token where one was used.",
        );
        let previous_winner = t.state.rounds.get(&(number - 1)).and_then(|r| r.winner_id);
        let used = tokens_used_elsewhere(t, number);
        // (choice index, label) -> (source, player)
        let mut choices: Vec<(Source, Option<i64>, String)> = vec![(Source::Vote, None, "In the draw".into())];
        if let Some(w) = previous_winner {
            choices.push((Source::Winner, Some(w), format!("{}'s pick (last winner)", d.player_name(w))));
        }
        for p in &t.participants {
            let left = t.t.nominate_tokens - used.get(p).copied().unwrap_or(0);
            if left > 0 {
                choices.push((
                    Source::Token,
                    Some(*p),
                    format!("{}'s Nominate token ({left} left)", d.player_name(*p)),
                ));
            }
        }

        let in_category = |g: &crate::model::Game| g.available && g.categories.iter().any(|c| c == category);
        let mut list: Vec<&crate::model::Game> = d.games.iter().filter(|g| in_category(g)).collect();
        let picked_elsewhere =
            d.games.iter().filter(|g| !in_category(g) && form.candidates.iter().any(|c| c.game == g.id)).count();
        if form.show_other_games || picked_elsewhere > 0 {
            list.extend(
                d.games
                    .iter()
                    .filter(|g| !in_category(g) && (g.available || form.candidates.iter().any(|c| c.game == g.id))),
            );
        }
        if list.is_empty() {
            ui.label(theme::muted(
                "No available games in this category yet. Add some on the Games page, or show other games.",
            ));
        }
        egui::Grid::new(("candidates", number)).num_columns(3).spacing([12.0, 4.0]).show(ui, |ui| {
            for g in list {
                let pos = form.candidates.iter().position(|c| c.game == g.id);
                let mut ticked = pos.is_some();
                let label =
                    if in_category(g) { g.name.clone() } else { format!("{} ({})", g.name, other_categories(d, g)) };
                if ui.checkbox(&mut ticked, RichText::new(label).strong()).changed() {
                    match pos {
                        Some(i) if !ticked => {
                            form.candidates.remove(i);
                        }
                        None if ticked => {
                            form.candidates.push(Candidate { game: g.id, source: Source::Vote, player: None })
                        }
                        _ => {}
                    }
                }
                if let Some(i) = form.candidates.iter().position(|c| c.game == g.id) {
                    let c = form.candidates[i];
                    let mut selected = choices.iter().position(|(s, p, _)| *s == c.source && *p == c.player);
                    let options: Vec<(usize, String)> =
                        choices.iter().enumerate().map(|(i, (_, _, l))| (i, l.clone())).collect();
                    if selected.is_none() {
                        // A saved token or pick that isn't offered any more (e.g. no tokens left).
                        ui.label(theme::muted(source_label(d, &c)));
                    } else if theme::pick(ui, ("candidate-source", number, g.id), &mut selected, &options, None, 240.0)
                        && let Some((source, player, _)) = selected.and_then(|i| choices.get(i))
                    {
                        form.candidates[i].source = *source;
                        form.candidates[i].player = *player;
                    }
                } else {
                    ui.label("");
                }
                ui.label("");
                ui.end_row();
            }
        });
        ui.checkbox(&mut form.show_other_games, "Show games from other categories");
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            let n = form.candidates.len();
            let label = match n {
                0 => "🎲 Draw game & Nemeses".to_string(),
                1 => "🎲 Play this game & draw Nemeses".to_string(),
                _ => format!("🎲 Draw from {n} games & draw Nemeses"),
            };
            if theme::big_primary(ui, n > 0, &label).clicked() {
                let candidates = form.candidates.clone();
                let names: HashMap<i64, String> = d.games.iter().map(|g| (g.id, g.name.clone())).collect();
                actions.push(Action::db_msg("Game and Nemeses drawn", move |c| {
                    let game = repo::draw_round(c, tid, number, &candidates)?;
                    Ok(format!("🎲 {} — Nemeses drawn", names.get(&game).cloned().unwrap_or_default()))
                }));
            }
            if current.is_some() && ui.button("Cancel").clicked() {
                form.redraw = false;
                form.candidates = t
                    .round_nominations(number)
                    .iter()
                    .map(|n| Candidate { game: n.game_id, source: n.source, player: n.player_id })
                    .collect();
            }
            if n == 0 {
                theme::hint(ui, "Tick at least one game.");
            }
        });
    });
}

fn other_categories(d: &Data, g: &crate::model::Game) -> String {
    g.categories.iter().map(|c| d.category_label(c)).collect::<Vec<_>>().join(", ")
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
    let points = |placement: i64| t.points_for(placement);
    let announced = t.round_nemeses(number);
    let nemesis: HashMap<i64, i64> = if completed {
        t.round_results(number).iter().filter_map(|r| Some((r.player_id, r.nemesis_player_id?))).collect()
    } else {
        announced
    };
    let dd_used: HashMap<i64, i64> = {
        let mut used = HashMap::new();
        for (n, r) in &t.results {
            if *n != number && r.double_down {
                *used.entry(r.player_id).or_insert(0) += 1;
            }
        }
        used
    };

    // What the round would do to the standings: computed by the rules on a scratch copy.
    let preview_state = if completed {
        None
    } else if form.complete() {
        let fresh = form.preview.as_ref().is_some_and(|(rows, rev, _)| *rows == form.rows && *rev == d.rev);
        if !fresh {
            let result = s.preview_round(tid, number, &form.inputs(&points, &nemesis)).map_err(|e| format!("{e:#}"));
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
            theme::section_title(ui, "Results");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if completed {
                    let later_done = t.rounds.iter().any(|r| r.number > number && r.completed);
                    let resp = ui.add_enabled(!later_done, egui::Button::new("Reopen to edit"));
                    if resp.on_disabled_hover_text("Only the most recent finished game can be reopened").clicked() {
                        actions.push(Action::db("Game reopened — you can edit the results again", move |c| {
                            repo::reopen_round(c, tid, number)
                        }));
                    }
                } else if ui.button("Clear places").clicked() {
                    for r in &mut form.rows {
                        r.placement = None;
                    }
                    form.dirty = true;
                }
            });
        });

        if !completed {
            theme::hint(
                ui,
                "Tap players in finishing order (1st first; tap the last one again to undo). For a tie, give them the same place: two 2nds, then 4th. \
                 Points, Nemesis bonuses and Double Downs are worked out for you.",
            );
            let next = form.next_placement();
            ui.horizontal_wrapped(|ui| {
                for row in form.rows.iter_mut().filter(|r| r.played) {
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
                            None => row.placement = Some(next),
                            Some(p) if p == next - 1 => row.placement = None,
                            Some(_) => {}
                        }
                        form.dirty = true;
                    }
                }
            });
            ui.add_space(6.0);
        }

        TableBuilder::new(ui)
            .id_salt(("results", number))
            .striped(true)
            .vscroll(false)
            .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
            .column(Column::exact(60.0))
            .column(Column::remainder().at_least(130.0))
            .column(Column::exact(80.0))
            .column(Column::exact(120.0))
            .column(Column::exact(150.0))
            .column(Column::exact(110.0))
            .header(24.0, |mut h| {
                for title in ["Played", "Player", "Place", "Double Down", "Nemesis", "= Points"] {
                    h.col(|ui| {
                        ui.label(theme::muted(title).small());
                    });
                }
            })
            .body(|mut body| {
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
                                changed = true;
                            }
                        });
                        row.col(|ui| {
                            let left = t.t.double_down_tokens - dd_used.get(&pid).copied().unwrap_or(0);
                            let can = r.double_down || left > 0;
                            let label = if r.double_down { "×2".to_string() } else { format!("{left} left") };
                            changed |=
                                ui.add_enabled(active && can, egui::Checkbox::new(&mut r.double_down, label)).changed();
                        });
                        row.col(|ui| match nemesis.get(&pid) {
                            Some(n) => {
                                ui.label(d.player_name(*n));
                            }
                            None => {
                                ui.label(theme::muted("–"));
                            }
                        });
                        row.col(|ui| {
                            if let (true, Some(res)) = (r.played, points_of(pid)) {
                                let mut text = RichText::new(res.points.to_string()).strong().size(17.0);
                                if res.placement == 1 {
                                    text = text.color(theme::GOLD);
                                }
                                let detail = format!(
                                    "{} for {}{}{}",
                                    res.base_points,
                                    crate::export::ordinal(res.placement),
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
        let problem = if played.len() < 2 {
            Some("At least two players must take part.".to_string())
        } else if missing > 0 {
            Some(format!("{missing} player(s) still need a place."))
        } else if winners != 1 {
            Some("Exactly one player must be 1st (ties are fine further down).".to_string())
        } else {
            None
        };
        ui.horizontal(|ui| {
            let inputs = form.inputs(&points, &nemesis);
            if theme::big_primary(ui, problem.is_none(), "✔  Finish game").clicked() {
                let rows = inputs.clone();
                form.dirty = false;
                actions.push(Action::db(format!("Game {number} finished — standings updated"), move |c| {
                    repo::save_results(c, tid, number, &rows, true)
                }));
            }
            if ui
                .add_enabled(form.dirty, egui::Button::new("Save draft"))
                .on_hover_text("Keep these places without finishing the game")
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
                    theme::hint(ui, "Finishing updates the standings, Challengers, the champion and the awards.");
                }
            }
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn form(players: &[i64]) -> Form {
        let rows =
            players.iter().map(|pid| Row { pid: *pid, played: true, placement: None, double_down: false }).collect();
        Form { rows, ..Form::default() }
    }

    const TABLE: [i64; 3] = [6, 4, 3];
    fn points(placement: i64) -> i64 {
        TABLE.get(placement as usize - 1).copied().unwrap_or(0)
    }

    #[test]
    fn points_come_from_the_table_and_ties_share_them() {
        let mut f = form(&[10, 20, 30]);
        f.rows[1].placement = Some(1);
        f.rows[0].placement = Some(2);
        f.rows[2].placement = Some(2);
        let got: Vec<(i64, i64, i64)> =
            f.inputs(&points, &HashMap::new()).iter().map(|r| (r.player_id, r.placement, r.base_points)).collect();
        assert_eq!(got, [(10, 2, 4), (20, 1, 6), (30, 2, 4)]);
        assert!(f.complete());
        assert_eq!(f.next_placement(), 3);
    }

    #[test]
    fn nemeses_come_from_the_draw_and_absent_players_are_left_out() {
        let mut f = form(&[10, 20, 30]);
        f.rows[2].played = false;
        f.rows[0].placement = Some(1);
        assert!(!f.complete(), "player 20 has no place yet");
        f.rows[1].placement = Some(2);
        let nemesis = HashMap::from([(10, 20), (20, 20)]);
        let inputs = f.inputs(&points, &nemesis);
        assert_eq!(inputs.len(), 2);
        assert_eq!(inputs[0].nemesis_player_id, Some(20));
        assert_eq!(inputs[1].nemesis_player_id, None, "never your own Nemesis");
    }
}
