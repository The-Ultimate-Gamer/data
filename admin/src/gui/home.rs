//! Dashboard: what's live, what to do next, and whether the website is up to date.

use eframe::egui::{self, RichText};

use super::{Action, Mode, Page, Session, Tab, TournamentData, theme, tournaments};
use crate::model::Status;

struct Step {
    title: &'static str,
    detail: String,
    done: bool,
    action: &'static str,
    page: Page,
}

pub fn show(ui: &mut egui::Ui, s: &Session, actions: &mut Vec<Action>) {
    let d = &s.data;
    let subtitle = match s.mode {
        Mode::Synced => "Your changes are saved on this computer as you go; Publish puts them on the website.",
        Mode::Practice => "Practice database with fictional data. Experiment freely: nothing here is published.",
        Mode::Folder => "Working directly in a folder. Commit and push with git to publish.",
    };
    theme::page_title(ui, "Dashboard", Some(subtitle));

    let focus = d.focus();
    if let Some(t) = focus {
        live_card(ui, s, t, actions);
        ui.add_space(8.0);
    }

    ui.columns(2, |cols| {
        theme::card(&mut cols[0], |ui| {
            theme::section_title(ui, "Getting ready");
            for step in steps(s, focus) {
                ui.horizontal(|ui| {
                    let (mark, color) = if step.done { ("✔", theme::LIVE) } else { ("○", theme::SUBTLE) };
                    ui.label(RichText::new(mark).color(color).size(18.0));
                    ui.vertical(|ui| {
                        ui.label(RichText::new(step.title).strong());
                        theme::hint(ui, &step.detail);
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let clicked = if step.done { ui.small_button(step.action).clicked() } else { theme::primary(ui, step.action).clicked() };
                        if clicked {
                            actions.push(Action::Go(step.page));
                        }
                    });
                });
                ui.separator();
            }
        });
        theme::card(&mut cols[1], |ui| {
            theme::section_title(ui, "At a glance");
            ui.horizontal(|ui| {
                theme::stat(ui, d.players.len(), "players");
                ui.add_space(24.0);
                theme::stat(ui, d.games.len(), "games");
                ui.add_space(24.0);
                theme::stat(ui, d.tournaments.len(), "tournaments");
            });
            ui.add_space(8.0);
            if s.report.errors.is_empty() {
                ui.label(RichText::new("✔ All data checks pass.").color(theme::LIVE));
            } else {
                ui.label(RichText::new(format!("✖ {} problem(s) must be fixed before publishing.", s.report.errors.len())).color(theme::DANGER));
            }
            if !s.report.warnings.is_empty() {
                ui.label(RichText::new(format!("⚠ {} warning(s) worth a look.", s.report.warnings.len())).color(theme::GOLD));
            }
            if s.mode == Mode::Synced {
                if s.pending.is_empty() {
                    ui.label(theme::muted("The website shows everything you've entered."));
                } else {
                    let pages = s.pending.iter().filter(|c| c.label.as_ref().is_some_and(|l| !l.starts_with("Link preview"))).count();
                    ui.label(RichText::new(format!("{pages} page(s) on the website will change when you publish.")).color(theme::GOLD));
                }
            }
            if d.demo_present {
                ui.add_space(4.0);
                theme::callout(ui, theme::CHALLENGER, |ui| {
                    ui.label("Fictional demo data is loaded, and the website shows a demo banner. Remove it in Settings before the real tournament.");
                });
            }
        });
    });
}

fn live_card(ui: &mut egui::Ui, s: &Session, t: &TournamentData, actions: &mut Vec<Action>) {
    let d = &s.data;
    let color = match t.t.status {
        Status::Ongoing => theme::LIVE,
        Status::Upcoming => theme::INFO,
        Status::Completed => theme::GOLD,
    };
    theme::callout(ui, color, |ui| {
        ui.horizontal(|ui| {
            theme::status_badge(ui, t.t.status);
            if t.t.is_demo {
                theme::badge(ui, "Demo", theme::CHALLENGER);
            }
            ui.label(RichText::new(&t.t.name).strong().size(20.0));
            let lap = t.next_round().map_or(1, |n| t.state.lap(n));
            let lap = if lap > 1 { format!(" · lap {lap}") } else { String::new() };
            ui.label(theme::muted(format!(
                "{} game{} played{lap}",
                t.completed(),
                if t.completed() == 1 { "" } else { "s" }
            )));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("Open tournament").clicked() {
                    actions.push(Action::Go(Page::Tournament(t.t.id, Tab::Overview)));
                }
            });
        });
        if let Some(champion) = t.state.champion_id {
            ui.label(
                RichText::new(format!("👑 {} is the Ultimate Gamer!", d.player_name(champion)))
                    .size(20.0)
                    .color(theme::GOLD)
                    .strong(),
            );
        } else if t.t.status == Status::Ongoing {
            match t.next_round() {
                Some(n) => {
                    let category = d.category_label(t.category_at(n).unwrap_or(""));
                    let game = t.round(n).and_then(|r| r.game_id).map(|g| d.game_name(g));
                    ui.horizontal(|ui| {
                        if theme::big_primary(ui, true, &format!("▶  Enter game {n}")).clicked() {
                            actions.push(Action::Go(Page::Round(t.t.id, n)));
                        }
                        ui.label(
                            RichText::new(format!("{category}{}", game.map(|g| format!(" · {g}")).unwrap_or_default()))
                                .size(17.0),
                        );
                    });
                }
                None => {
                    ui.label("All planned categories are played. Add more categories on the tournament's Setup tab.");
                }
            }
        } else if t.t.status == Status::Upcoming {
            ui.label("Not started yet. Finish the setup, then press Start tournament on its page.");
        }
        if !t.state.order.is_empty() && t.completed() > 0 {
            ui.add_space(4.0);
            tournaments::standings_table(ui, d, t, &t.state, 5, &[]);
        }
    });
}

fn steps(s: &Session, focus: Option<&TournamentData>) -> Vec<Step> {
    let d = &s.data;
    let real_players = d.players.iter().filter(|p| !p.is_demo).count();
    let real_games = d.games.iter().filter(|g| !g.is_demo).count();
    let tournament_page = |tab| focus.map_or(Page::Tournaments, |t| Page::Tournament(t.t.id, tab));
    let mut steps = vec![
        Step {
            title: "Add the players",
            detail: format!("{real_players} player{} added", if real_players == 1 { "" } else { "s" }),
            done: real_players >= 2,
            action: "Players",
            page: Page::Players,
        },
        Step {
            title: "Build the game library",
            detail: format!("{real_games} game{}, each tagged with categories", if real_games == 1 { "" } else { "s" }),
            done: real_games > 0,
            action: "Games",
            page: Page::Games,
        },
        Step {
            title: "Create the tournament",
            detail: focus
                .map_or("Name, date, Challenger threshold and points per placement".into(), |t| t.t.name.clone()),
            done: focus.is_some(),
            action: "Tournaments",
            page: Page::Tournaments,
        },
        Step {
            title: "Choose who plays and the category order",
            detail: focus.map_or("On the tournament's Setup tab".into(), |t| {
                format!("{} players · {} categories", t.participants.len(), t.positions.len())
            }),
            done: focus.is_some_and(|t| t.participants.len() >= 2 && !t.positions.is_empty()),
            action: "Setup",
            page: tournament_page(Tab::Setup),
        },
        Step {
            title: "Start the tournament",
            detail: "Makes it the live tournament on the website's homepage".into(),
            done: focus.is_some_and(|t| t.t.status != Status::Upcoming),
            action: "Open",
            page: tournament_page(Tab::Overview),
        },
    ];
    let next = focus.and_then(|t| t.next_round().map(|n| (t.t.id, n)));
    steps.push(Step {
        title: "Record each game",
        detail: focus.map_or("Tap players in finishing order, then Finish game".into(), |t| {
            format!("{} of {} played", t.completed(), t.positions.len())
        }),
        done: focus.is_some_and(|t| t.t.status == Status::Completed || t.completed() > 0),
        action: if next.is_some() { "Enter next game" } else { "Open" },
        page: next.map_or(tournament_page(Tab::Overview), |(tid, n)| Page::Round(tid, n)),
    });
    if s.mode == Mode::Synced {
        steps.push(Step {
            title: "Publish",
            detail: "Send your changes to the website".into(),
            done: s.pending.is_empty(),
            action: "Publish",
            page: Page::Publish,
        });
    }
    steps
}
