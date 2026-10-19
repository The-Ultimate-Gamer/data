//! Tournaments: the list, creating one, and a tournament's overview, setup and awards.

use eframe::egui::{self, RichText};
use egui_extras::{Column, TableBuilder};

use super::{Action, Confirm, Data, Page, Session, Tab, TournamentData, color_for_rank, theme};
use crate::model::Status;
use crate::repo::{self, TournamentInput};
use crate::rules::TournamentState;

#[derive(Default)]
pub struct Form {
    creating: bool,
    new: TournamentInput,
    copy_players_from: Option<i64>,
    /// Settings being edited: (tournament id, data revision it was loaded at, values).
    settings: Option<(i64, u64, TournamentInput)>,
    add_category: Option<usize>,
    grant_award: Option<usize>,
    grant_player: Option<i64>,
    grant_detail: String,
    record_label: String,
    record_value: String,
    record_player: Option<i64>,
}

pub fn list(ui: &mut egui::Ui, s: &Session, form: &mut Form, actions: &mut Vec<Action>) {
    let d = &s.data;
    ui.horizontal(|ui| {
        theme::page_title(
            ui,
            "Tournaments",
            Some("Each LAN is a tournament. Open one to set it up and record its games."),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if !form.creating && theme::primary(ui, "+ New tournament").clicked() {
                form.creating = true;
                form.new = TournamentInput::default();
                form.copy_players_from = d.tournaments.iter().find(|t| !t.t.is_demo).map(|t| t.t.id);
            }
        });
    });

    if form.creating {
        theme::card(ui, |ui| {
            theme::section_title(ui, "New tournament");
            egui::Grid::new("new-tournament").num_columns(2).spacing([16.0, 8.0]).show(ui, |ui| {
                theme::field(ui, "Name", |ui| theme::text(ui, &mut form.new.name, "LAN 2026"));
                theme::field(ui, "Web address", |ui| {
                    ui.horizontal(|ui| {
                        let auto = crate::db::slugify(&form.new.name);
                        theme::narrow_text(
                            ui,
                            &mut form.new.slug,
                            if auto.is_empty() { "lan-2026" } else { &auto },
                            200.0,
                        );
                        theme::hint(ui, "leave empty to use the name");
                    })
                });
                theme::field(ui, "Edition", |ui| theme::text(ui, &mut form.new.edition, "optional, e.g. 3rd edition"));
                theme::field(ui, "Start date", |ui| {
                    theme::narrow_text(ui, &mut form.new.start_date, "2026-11-14", 120.0)
                });
                theme::field(ui, "Location", |ui| theme::text(ui, &mut form.new.location, "optional"));
                theme::field(ui, "Challenger threshold", |ui| {
                    ui.horizontal(|ui| {
                        theme::narrow_text(ui, &mut form.new.challenger_threshold, "20", 60.0);
                        theme::hint(ui, "points needed to become a Challenger");
                    })
                });
                theme::field(ui, "Tokens per player", |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Double Down");
                        theme::narrow_text(ui, &mut form.new.double_down_tokens, "1", 40.0);
                        ui.label("Nominate");
                        theme::narrow_text(ui, &mut form.new.nominate_tokens, "1", 40.0);
                    })
                });
                theme::field(ui, "Points per placement", |ui| {
                    ui.horizontal(|ui| {
                        theme::narrow_text(ui, &mut form.new.points, "6, 4, 3, 2, 1, 0", 200.0);
                        theme::hint(ui, "1st, 2nd, 3rd… (pre-fills results)");
                    })
                });
                theme::field(ui, "Players", |ui| {
                    let options: Vec<(i64, String)> =
                        d.tournaments.iter().map(|t| (t.t.id, format!("Same as {}", t.t.name))).collect();
                    theme::pick(
                        ui,
                        "copy-players",
                        &mut form.copy_players_from,
                        &options,
                        Some("Choose them later"),
                        260.0,
                    );
                });
            });
            ui.horizontal(|ui| {
                if theme::primary(ui, "Create tournament").clicked() {
                    let input = form.new.clone();
                    let copy = form.copy_players_from;
                    form.creating = false;
                    actions.push(Action::db_go(format!("Tournament {} created", input.name.trim()), move |c| {
                        let id = repo::save_tournament(c, None, &input, copy)?;
                        Ok(Page::Tournament(id, Tab::Setup))
                    }));
                }
                if ui.button("Cancel").clicked() {
                    form.creating = false;
                }
            });
        });
        ui.add_space(8.0);
    }

    if d.tournaments.is_empty() {
        theme::card(ui, |ui| {
            ui.label(theme::muted("No tournaments yet. Create the first one to get started."));
        });
        return;
    }
    theme::card(ui, |ui| {
        TableBuilder::new(ui)
            .striped(true)
            .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
            .column(Column::remainder().at_least(220.0))
            .column(Column::auto().at_least(90.0))
            .column(Column::auto().at_least(150.0))
            .column(Column::auto().at_least(70.0))
            .column(Column::auto().at_least(80.0))
            .column(Column::auto().at_least(140.0))
            .header(26.0, |mut h| {
                for title in ["Tournament", "Status", "Dates", "Players", "Games", "Ultimate Gamer"] {
                    h.col(|ui| {
                        ui.label(theme::muted(title).small());
                    });
                }
            })
            .body(|mut body| {
                for t in &d.tournaments {
                    body.row(34.0, |mut row| {
                        row.col(|ui| {
                            if ui.link(RichText::new(&t.t.name).strong()).clicked() {
                                actions.push(Action::Go(Page::Tournament(t.t.id, Tab::Overview)));
                            }
                            if t.t.is_demo {
                                theme::badge(ui, "Demo", theme::CHALLENGER);
                            }
                        });
                        row.col(|ui| {
                            theme::status_badge(ui, t.t.status);
                        });
                        row.col(|ui| {
                            ui.label(crate::export::date_range(t.t.start_date.as_deref(), t.t.end_date.as_deref()));
                        });
                        row.col(|ui| {
                            ui.label(t.participants.len().to_string());
                        });
                        row.col(|ui| {
                            ui.label(format!("{} / {}", t.completed(), t.positions.len()));
                        });
                        row.col(|ui| {
                            ui.label(t.state.champion_id.map_or(String::new(), |c| format!("👑 {}", d.player_name(c))));
                        });
                    });
                }
            });
    });
}

pub fn detail(
    ui: &mut egui::Ui,
    s: &Session,
    t: &TournamentData,
    tab: Tab,
    form: &mut Form,
    actions: &mut Vec<Action>,
) {
    let d = &s.data;
    let tid = t.t.id;
    if ui.link("⬅ All tournaments").clicked() {
        actions.push(Action::Go(Page::Tournaments));
    }
    ui.horizontal(|ui| {
        ui.heading(RichText::new(&t.t.name).strong());
        theme::status_badge(ui, t.t.status);
        if t.t.is_demo {
            theme::badge(ui, "Demo", theme::CHALLENGER);
        }
        ui.label(theme::muted(crate::export::date_range(t.t.start_date.as_deref(), t.t.end_date.as_deref())));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| status_buttons(ui, t, actions));
    });
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        for (value, label) in [(Tab::Overview, "Overview"), (Tab::Setup, "Setup"), (Tab::Awards, "Awards & records")] {
            if ui.selectable_label(tab == value, RichText::new(label).size(16.0)).clicked() {
                actions.push(Action::Go(Page::Tournament(tid, value)));
            }
        }
    });
    ui.separator();
    match tab {
        Tab::Overview => overview(ui, d, t, actions),
        Tab::Setup => setup(ui, d, t, form, actions),
        Tab::Awards => awards(ui, d, t, form, actions),
    }
}

fn status_buttons(ui: &mut egui::Ui, t: &TournamentData, actions: &mut Vec<Action>) {
    let tid = t.t.id;
    match t.t.status {
        Status::Upcoming => {
            let ready = t.participants.len() >= 2 && !t.positions.is_empty();
            let resp = theme::primary_enabled(ui, ready, "▶ Start tournament")
                .on_disabled_hover_text("Choose at least two players and one category on the Setup tab first.");
            if resp.clicked() {
                actions.push(Action::db("Tournament started — it's now live on the website", move |c| {
                    repo::set_status(c, tid, Status::Ongoing)
                }));
            }
        }
        Status::Ongoing => {
            if t.state.champion_id.is_some() {
                if theme::primary(ui, "🏁 Mark completed").clicked() {
                    actions
                        .push(Action::db("Tournament completed", move |c| repo::set_status(c, tid, Status::Completed)));
                }
            } else if ui.button("Mark completed…").clicked() {
                actions.push(Action::Confirm(Confirm::new(
                    "End the tournament without a champion?",
                    "No Challenger has won a game yet. Mark it completed anyway?",
                    "Mark completed",
                    Action::db("Tournament completed", move |c| repo::set_status(c, tid, Status::Completed)),
                )));
            }
        }
        Status::Completed => {
            if ui.button("Reopen").on_hover_text("Set it back to live").clicked() {
                actions.push(Action::db("Tournament reopened", move |c| repo::set_status(c, tid, Status::Ongoing)));
            }
        }
    }
}

/// Standings from the rules module. `highlight` marks players (e.g. new Challengers).
pub fn standings_table(
    ui: &mut egui::Ui,
    d: &Data,
    t: &TournamentData,
    state: &TournamentState,
    limit: usize,
    highlight: &[i64],
) {
    let id = ui.next_auto_id();
    ui.push_id(id, |ui| {
        TableBuilder::new(ui)
            .striped(true)
            .vscroll(false)
            .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
            .column(Column::exact(36.0))
            .column(Column::remainder().at_least(140.0))
            .column(Column::auto().at_least(56.0))
            .column(Column::auto().at_least(110.0))
            .column(Column::auto().at_least(80.0))
            .header(22.0, |mut h| {
                for title in ["#", "Player", "Points", "Status", "Tokens left"] {
                    h.col(|ui| {
                        ui.label(theme::muted(title).small());
                    });
                }
            })
            .body(|mut body| {
                for p in state.standings().take(limit) {
                    body.row(30.0, |mut row| {
                        row.col(|ui| {
                            ui.label(RichText::new(p.rank.to_string()).strong().color(color_for_rank(p.rank)));
                        });
                        row.col(|ui| {
                            let mut name = RichText::new(d.player_name(p.player_id)).strong();
                            if highlight.contains(&p.player_id) {
                                name = name.color(theme::CHALLENGER);
                            }
                            ui.label(name);
                        });
                        row.col(|ui| {
                            ui.label(RichText::new(p.points.to_string()).strong().size(16.0));
                        });
                        row.col(|ui| {
                            if state.champion_id == Some(p.player_id) {
                                theme::badge(ui, "👑 Ultimate Gamer", theme::GOLD);
                            } else if let Some(since) = p.challenger_since {
                                theme::badge(ui, "Challenger", theme::CHALLENGER)
                                    .on_hover_text(format!("Since game {since}"));
                            }
                        });
                        row.col(|ui| {
                            let dd = (t.t.double_down_tokens - p.double_downs_used).max(0);
                            let nom = (t.t.nominate_tokens - p.nominations_used).max(0);
                            ui.label(theme::muted(format!("DD {dd} · Nom {nom}")).small())
                                .on_hover_text(format!("{dd} Double Down and {nom} Nominate token(s) left"));
                        });
                    });
                }
            });
    });
    let remaining = state.order.len().saturating_sub(limit);
    if remaining > 0 {
        theme::hint(ui, format!("and {remaining} more"));
    }
    theme::hint(
        ui,
        format!(
            "Challenger at {} points. The first Challenger to win a game becomes the Ultimate Gamer.",
            t.t.challenger_threshold
        ),
    );
}

fn overview(ui: &mut egui::Ui, d: &Data, t: &TournamentData, actions: &mut Vec<Action>) {
    let tid = t.t.id;
    if let Some(champion) = t.state.champion_id {
        theme::callout(ui, theme::GOLD, |ui| {
            ui.label(
                RichText::new(format!(
                    "👑 {} won game {} as a Challenger — the Ultimate Gamer!",
                    d.player_name(champion),
                    t.state.deciding_round.unwrap_or(0)
                ))
                .size(19.0)
                .strong(),
            );
            if t.t.status != Status::Completed {
                ui.label("Mark the tournament completed, then grant the awards on the Awards tab.");
            }
        });
        ui.add_space(6.0);
    }
    if t.participants.len() < 2 || t.positions.is_empty() {
        theme::callout(ui, theme::INFO, |ui| {
            ui.horizontal(|ui| {
                ui.label("Choose the players and the category order first.");
                if theme::primary(ui, "Go to Setup").clicked() {
                    actions.push(Action::Go(Page::Tournament(tid, Tab::Setup)));
                }
            });
        });
        ui.add_space(6.0);
    }

    ui.columns(2, |cols| {
        theme::card(&mut cols[0], |ui| {
            theme::section_title(ui, "Standings");
            if t.state.order.is_empty() {
                ui.label(theme::muted("No players registered yet."));
            } else {
                standings_table(ui, d, t, &t.state, usize::MAX, &[]);
            }
        });
        theme::card(&mut cols[1], |ui| {
            theme::section_title(ui, "Games");
            if t.positions.is_empty() {
                ui.label(theme::muted("No categories yet."));
            }
            let next = t.next_round();
            for (n, category) in &t.positions {
                let round = t.round(*n);
                let state = t.state.rounds.get(n);
                ui.horizontal(|ui| {
                    ui.label(RichText::new(format!("{n}")).strong().color(theme::GOLD));
                    ui.vertical(|ui| {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(d.category_label(category)).strong());
                            if let Some(g) = round.and_then(|r| r.game_id) {
                                ui.label(theme::muted(format!("· {}", d.game_name(g))));
                            }
                        });
                        if let Some(winner) = state.filter(|r| r.completed).and_then(|r| r.winner_id) {
                            let mut line = format!("🏆 {}", d.player_name(winner));
                            if state.is_some_and(|r| r.decided) {
                                line.push_str(" · 👑 decided the tournament");
                            }
                            theme::hint(ui, line);
                        } else if t.state.ignored_rounds.contains(n) {
                            ui.label(
                                RichText::new("Played after the tournament was decided").color(theme::DANGER).small(),
                            );
                        }
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let done = round.is_some_and(|r| r.completed);
                        let live = t.t.status == Status::Ongoing;
                        if done {
                            if ui.small_button("View").clicked() {
                                actions.push(Action::Go(Page::Round(tid, *n)));
                            }
                        } else if next == Some(*n) && live {
                            if theme::primary(ui, &format!("▶ Enter game {n}")).clicked() {
                                actions.push(Action::Go(Page::Round(tid, *n)));
                            }
                        } else if next == Some(*n)
                            && ui
                                .small_button("Prepare")
                                .on_hover_text("Record nominations and Nemeses before starting")
                                .clicked()
                        {
                            actions.push(Action::Go(Page::Round(tid, *n)));
                        }
                    });
                });
                ui.separator();
            }
        });
    });
}

fn setup(ui: &mut egui::Ui, d: &Data, t: &TournamentData, form: &mut Form, actions: &mut Vec<Action>) {
    let tid = t.t.id;
    // Reload the settings form when the tournament or the data changed underneath it.
    let stale = form.settings.as_ref().is_none_or(|(id, rev, _)| *id != tid || *rev != d.rev);
    if stale {
        let points: Vec<(i64, i64)> = t.points.iter().enumerate().map(|(i, p)| (i as i64 + 1, *p)).collect();
        form.settings = Some((tid, d.rev, TournamentInput::from(&t.t, &points)));
    }

    ui.columns(2, |cols| {
        let ui = &mut cols[0];
        theme::card(ui, |ui| {
            theme::section_title(ui, "Players");
            theme::hint(ui, "Tick everyone who plays. Each box saves right away.");
            if d.players.is_empty() {
                ui.horizontal(|ui| {
                    ui.label(theme::muted("No players yet."));
                    if ui.button("Add players").clicked() {
                        actions.push(Action::Go(Page::Players));
                    }
                });
            }
            ui.horizontal_wrapped(|ui| {
                for p in &d.players {
                    let mut playing = t.participants.contains(&p.id);
                    let locked = playing && t.has_played(p.id);
                    let resp = ui.add_enabled(!locked, egui::Checkbox::new(&mut playing, &p.name));
                    let resp = resp.on_disabled_hover_text("Has results in this tournament");
                    if resp.changed() {
                        let (pid, name) = (p.id, p.name.clone());
                        let label = if playing { format!("{name} added") } else { format!("{name} removed") };
                        actions.push(Action::db(label, move |c| repo::set_participant(c, tid, pid, playing)));
                    }
                }
            });
            ui.label(theme::muted(format!("{} playing", t.participants.len())).small());
        });
        ui.add_space(8.0);
        theme::card(ui, |ui| {
            theme::section_title(ui, "Category order");
            theme::hint(ui, "One game per category, played in this order. Add categories as they're announced.");
            for (i, (n, category)) in t.positions.iter().enumerate() {
                let started = t.round(*n).is_some();
                ui.horizontal(|ui| {
                    ui.label(RichText::new(format!("{n}.")).strong().color(theme::GOLD));
                    ui.label(d.category_label(category));
                    if started {
                        theme::hint(ui, "started");
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let n = *n;
                        if ui.add_enabled(!started, egui::Button::new("✖").small()).on_hover_text("Remove").clicked()
                        {
                            actions.push(Action::db("Category removed", move |c| repo::remove_position(c, tid, n)));
                        }
                        let next_started = t.positions.get(i + 1).is_some_and(|(m, _)| t.round(*m).is_some());
                        let can_down = i + 1 < t.positions.len() && !started && !next_started;
                        if ui.add_enabled(can_down, egui::Button::new("⬇").small()).clicked() {
                            actions.push(Action::db("Order changed", move |c| repo::swap_positions(c, tid, n, n + 1)));
                        }
                        let prev_started = i > 0 && t.round(t.positions[i - 1].0).is_some();
                        if ui.add_enabled(i > 0 && !started && !prev_started, egui::Button::new("⬆").small()).clicked()
                        {
                            actions.push(Action::db("Order changed", move |c| repo::swap_positions(c, tid, n, n - 1)));
                        }
                    });
                });
            }
            ui.horizontal(|ui| {
                let options: Vec<(usize, String)> =
                    d.categories.iter().enumerate().map(|(i, c)| (i, d.category_label(&c.slug))).collect();
                theme::pick(ui, "add-category", &mut form.add_category, &options, None, 200.0);
                if ui.add_enabled(form.add_category.is_some(), egui::Button::new("+ Add")).clicked()
                    && let Some(c) = form.add_category.and_then(|i| d.categories.get(i))
                {
                    let slug = c.slug.clone();
                    actions.push(Action::db(format!("{} added", c.name), move |conn| {
                        repo::add_position(conn, tid, &slug)
                    }));
                }
            });
        });
        let Some((_, _, input)) = form.settings.as_mut() else { return };
        theme::card(&mut cols[1], |ui| {
            theme::section_title(ui, "Settings");
            egui::Grid::new("tournament-settings").num_columns(2).spacing([16.0, 8.0]).show(ui, |ui| {
                theme::field(ui, "Name", |ui| theme::text(ui, &mut input.name, ""));
                theme::field(ui, "Web address", |ui| {
                    ui.vertical(|ui| {
                        theme::narrow_text(ui, &mut input.slug, &t.t.slug, 220.0);
                        if input.slug.trim() != t.t.slug {
                            ui.label(
                                RichText::new("⚠ Links people already shared to this tournament will stop working.")
                                    .color(theme::GOLD)
                                    .small(),
                            );
                        }
                    })
                });
                theme::field(ui, "Edition", |ui| theme::text(ui, &mut input.edition, "optional"));
                theme::field(ui, "Dates", |ui| {
                    ui.horizontal(|ui| {
                        theme::narrow_text(ui, &mut input.start_date, "start: 2026-11-14", 140.0);
                        ui.label("to");
                        theme::narrow_text(ui, &mut input.end_date, "end (optional)", 140.0);
                    })
                });
                theme::field(ui, "Location", |ui| theme::text(ui, &mut input.location, "optional"));
                theme::field(ui, "Status", |ui| {
                    ui.horizontal(|ui| {
                        for status in Status::ALL {
                            ui.selectable_value(&mut input.status, status, status.label());
                        }
                    })
                });
                theme::field(ui, "Challenger threshold", |ui| {
                    theme::narrow_text(ui, &mut input.challenger_threshold, "", 60.0)
                });
                theme::field(ui, "Tokens per player", |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Double Down");
                        theme::narrow_text(ui, &mut input.double_down_tokens, "", 40.0);
                        ui.label("Nominate");
                        theme::narrow_text(ui, &mut input.nominate_tokens, "", 40.0);
                    })
                });
                theme::field(ui, "Points per placement", |ui| {
                    theme::narrow_text(ui, &mut input.points, "6, 4, 3, 2, 1, 0", 220.0)
                });
            });
            ui.horizontal(|ui| {
                if theme::primary(ui, "Save settings").clicked() {
                    let input = input.clone();
                    actions.push(Action::db("Settings saved", move |c| {
                        repo::save_tournament(c, Some(tid), &input, None).map(|_| ())
                    }));
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let played = t.completed() > 0;
                    let resp = ui.add_enabled(
                        !played,
                        egui::Button::new(RichText::new("Delete tournament").color(theme::DANGER)),
                    );
                    if resp.on_disabled_hover_text("Tournaments with finished games can't be deleted").clicked() {
                        actions.push(Action::Confirm(Confirm::new(
                            format!("Delete {}?", t.t.name),
                            "Its players, categories, nominations and awards are removed with it. You can undo this.",
                            "Delete",
                            Action::db_go(format!("{} deleted", t.t.name), move |c| {
                                repo::delete_tournament(c, tid)?;
                                Ok(Page::Tournaments)
                            }),
                        )));
                    }
                });
            });
        });
    });
}

fn awards(ui: &mut egui::Ui, d: &Data, t: &TournamentData, form: &mut Form, actions: &mut Vec<Action>) {
    let tid = t.t.id;
    let participants: Vec<(i64, String)> = t.participants.iter().map(|p| (*p, d.player_name(*p))).collect();
    ui.columns(2, |cols| {
        theme::card(&mut cols[0], |ui| {
            theme::section_title(ui, "Awards");
            theme::hint(
                ui,
                "Award winners are decided by the organisers. The suggestions only cover the clear-cut ones.",
            );
            let has = |award: &str, pid: i64| t.grants.iter().any(|g| g.award_slug == award && g.player_id == pid);
            let mut suggestions: Vec<(String, i64)> = Vec::new();
            if let Some(c) = t.state.champion_id.filter(|c| !has("ultimate-gamer", *c)) {
                suggestions.push(("ultimate-gamer".into(), c));
            }
            for p in t.state.standings().filter(|p| p.challenger_since.is_some() && !has("challenger", p.player_id)) {
                suggestions.push(("challenger".into(), p.player_id));
            }
            suggestions.retain(|(a, _)| d.awards.iter().any(|x| &x.slug == a));
            if !suggestions.is_empty() {
                theme::callout(ui, theme::GOLD, |ui| {
                    for (award, pid) in &suggestions {
                        ui.horizontal(|ui| {
                            ui.label(format!("{} ➡ {}", d.award_label(award), d.player_name(*pid)));
                            if ui.small_button("Grant").clicked() {
                                let (award, pid) = (award.clone(), *pid);
                                actions.push(Action::db("Award granted", move |c| {
                                    repo::grant_award(c, tid, &award, pid, "")
                                }));
                            }
                        });
                    }
                });
            }
            if t.grants.is_empty() {
                ui.label(theme::muted("No awards granted yet."));
            }
            for g in &t.grants {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(d.award_label(&g.award_slug)).strong());
                    ui.label(d.player_name(g.player_id));
                    if let Some(detail) = &g.detail {
                        theme::hint(ui, detail);
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button("✖").on_hover_text("Remove").clicked() {
                            let id = g.id;
                            actions.push(Action::db("Award removed", move |c| repo::revoke_award(c, id)));
                        }
                    });
                });
            }
            ui.separator();
            let options: Vec<(usize, String)> =
                d.awards.iter().enumerate().map(|(i, a)| (i, d.award_label(&a.slug))).collect();
            ui.horizontal_wrapped(|ui| {
                theme::pick(ui, "grant-award", &mut form.grant_award, &options, None, 180.0);
                theme::pick(ui, "grant-player", &mut form.grant_player, &participants, None, 140.0);
                theme::narrow_text(ui, &mut form.grant_detail, "detail (optional)", 160.0);
                let ready = form.grant_award.is_some() && form.grant_player.is_some();
                if ui.add_enabled(ready, egui::Button::new("Grant")).clicked()
                    && let (Some(a), Some(pid)) = (form.grant_award.and_then(|i| d.awards.get(i)), form.grant_player)
                {
                    let (award, detail) = (a.slug.clone(), std::mem::take(&mut form.grant_detail));
                    actions.push(Action::db("Award granted", move |c| repo::grant_award(c, tid, &award, pid, &detail)));
                }
            });
        });
        theme::card(&mut cols[1], |ui| {
            theme::section_title(ui, "Records");
            theme::hint(ui, "Fun facts shown on the tournament page, e.g. \"Biggest single-game haul: 12 pts\".");
            for r in &t.records {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(&r.label).strong());
                    ui.label(&r.value);
                    if let Some(p) = r.player_id {
                        theme::hint(ui, d.player_name(p));
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button("✖").clicked() {
                            let position = r.position;
                            actions.push(Action::db("Record removed", move |c| repo::remove_record(c, tid, position)));
                        }
                    });
                });
            }
            ui.separator();
            ui.horizontal_wrapped(|ui| {
                theme::narrow_text(ui, &mut form.record_label, "label", 160.0);
                theme::narrow_text(ui, &mut form.record_value, "value", 90.0);
                theme::pick(ui, "record-player", &mut form.record_player, &participants, Some("no player"), 130.0);
                let ready = !form.record_label.trim().is_empty() && !form.record_value.trim().is_empty();
                if ui.add_enabled(ready, egui::Button::new("Add")).clicked() {
                    let (label, value, player) = (
                        std::mem::take(&mut form.record_label),
                        std::mem::take(&mut form.record_value),
                        form.record_player.take(),
                    );
                    actions.push(Action::db("Record added", move |c| repo::add_record(c, tid, &label, &value, player)));
                }
            });
        });
    });
}
