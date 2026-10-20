//! The game library: every game that can be nominated or played, tagged with categories.

use eframe::egui::{self, RichText};

use super::{Action, Confirm, Data, Session, theme};
use crate::repo::{self, GameInput};

#[derive(Default)]
pub struct Form {
    search: String,
    category: Option<String>,
    hide_unavailable: bool,
    quick_name: String,
    editing: Option<(i64, u64, GameInput)>,
}

pub fn show(ui: &mut egui::Ui, s: &Session, form: &mut Form, actions: &mut Vec<Action>) {
    let d = &s.data;
    theme::page_title(
        ui,
        "Games",
        Some("The library of games. Tag each game with its categories so it's offered in the right rounds."),
    );

    ui.horizontal(|ui| {
        let resp = theme::narrow_text(ui, &mut form.quick_name, "New game's name — press Enter", 300.0);
        let enter = resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        if (enter || ui.button("+ Add game").clicked()) && !form.quick_name.trim().is_empty() {
            let mut input = GameInput::new();
            input.name = std::mem::take(&mut form.quick_name);
            // Pre-tag with the category being browsed.
            input.categories = form.category.iter().cloned().collect();
            let label = format!("{} added", input.name.trim());
            actions.push(Action::db(label, move |c| repo::save_game(c, None, &input).map(|_| ())));
            resp.request_focus();
        }
        ui.add_space(16.0);
        ui.label("🔍");
        theme::narrow_text(ui, &mut form.search, "Search", 180.0);
        ui.checkbox(&mut form.hide_unavailable, "Hide unavailable");
    });
    ui.horizontal_wrapped(|ui| {
        if theme::chip(ui, form.category.is_none(), "All").clicked() {
            form.category = None;
        }
        for c in &d.categories {
            let count = d.games.iter().filter(|g| g.categories.contains(&c.slug)).count();
            let selected = form.category.as_deref() == Some(c.slug.as_str());
            if theme::chip(ui, selected, format!("{} ({count})", d.category_label(&c.slug))).clicked() {
                form.category = if selected { None } else { Some(c.slug.clone()) };
            }
        }
    });
    ui.add_space(6.0);

    if let Some((id, rev, _)) = &form.editing {
        match d.game(*id) {
            None => form.editing = None,
            Some(g) if *rev != d.rev => form.editing = Some((g.id, d.rev, GameInput::from(g))),
            _ => {}
        }
    }

    let editor_width = 420.0;
    ui.horizontal_top(|ui| {
        let list_width = (ui.available_width() - editor_width - 16.0).max(320.0);
        ui.vertical(|ui| {
            ui.set_width(list_width);
            theme::card(ui, |ui| list(ui, d, form));
        });
        ui.vertical(|ui| {
            ui.set_width(editor_width);
            match form.editing.as_mut() {
                Some((id, _, input)) => editor(ui, d, *id, input, actions),
                None => {
                    theme::card(ui, |ui| {
                        ui.label(theme::muted("Select a game to edit it."));
                    });
                }
            }
        });
    });
}

fn list(ui: &mut egui::Ui, d: &Data, form: &mut Form) {
    let q = form.search.trim().to_lowercase();
    let games: Vec<_> = d
        .games
        .iter()
        .filter(|g| q.is_empty() || g.name.to_lowercase().contains(&q))
        .filter(|g| form.category.as_ref().is_none_or(|c| g.categories.contains(c)))
        .filter(|g| g.available || !form.hide_unavailable)
        .collect();
    if games.is_empty() {
        ui.label(theme::muted(if d.games.is_empty() {
            "No games yet. Type a name above and press Enter."
        } else {
            "No game matches."
        }));
    }
    for g in games {
        let selected = form.editing.as_ref().is_some_and(|(id, _, _)| *id == g.id);
        let played =
            d.tournaments.iter().flat_map(|t| &t.rounds).filter(|r| r.completed && r.game_id == Some(g.id)).count();
        let resp = theme::clickable_row(ui, selected, |ui| {
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    let name = RichText::new(&g.name).strong();
                    let name = if selected {
                        name.color(theme::GOLD)
                    } else if g.available {
                        name
                    } else {
                        name.color(theme::SUBTLE)
                    };
                    ui.label(name);
                    if !g.available {
                        theme::badge(ui, "Unavailable", theme::SUBTLE);
                    }
                    if g.is_demo {
                        theme::badge(ui, "Demo", theme::CHALLENGER);
                    }
                });
                let cats: Vec<String> = g.categories.iter().map(|c| d.category_label(c)).collect();
                let players = match (g.min_players, g.max_players) {
                    (Some(a), Some(b)) if a == b => format!(" · {a} players"),
                    (Some(a), Some(b)) => format!(" · {a}–{b} players"),
                    (Some(a), None) => format!(" · {a}+ players"),
                    (None, Some(b)) => format!(" · up to {b} players"),
                    (None, None) => String::new(),
                };
                let played = if played > 0 { format!(" · played {played}×") } else { String::new() };
                let cats = if cats.is_empty() { "⚠ no category".to_string() } else { cats.join(", ") };
                theme::hint(ui, format!("{cats}{players}{played}"));
            });
        });
        if resp.clicked() {
            form.editing = Some((g.id, d.rev, GameInput::from(g)));
        }
        ui.add_space(2.0);
    }
}

fn editor(ui: &mut egui::Ui, d: &Data, id: i64, input: &mut GameInput, actions: &mut Vec<Action>) {
    let Some(g) = d.game(id) else { return };
    theme::card(ui, |ui| {
        theme::section_title(ui, &g.name);
        egui::Grid::new("game-editor").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
            theme::field(ui, "Name", |ui| theme::narrow_text(ui, &mut input.name, "", 300.0));
            theme::field(ui, "Link", |ui| {
                ui.vertical(|ui| {
                    theme::narrow_text(ui, &mut input.url, "https://store.steampowered.com/…", 300.0);
                    theme::hint(ui, "Where to get or play it: Steam, itch.io, a shop… Shown on the website.");
                })
            });
            theme::field(ui, "Players", |ui| {
                ui.horizontal(|ui| {
                    theme::narrow_text(ui, &mut input.min_players, "min", 50.0);
                    ui.label("to");
                    theme::narrow_text(ui, &mut input.max_players, "max", 50.0);
                })
            });
            theme::field(ui, "Available", |ui| ui.checkbox(&mut input.available, "Can be nominated at the next LAN"));
        });
        ui.label(theme::muted("Description"));
        ui.add(
            egui::TextEdit::multiline(&mut input.description)
                .desired_rows(2)
                .desired_width(f32::INFINITY)
                .hint_text("optional"),
        );
        ui.label(theme::muted("Categories"));
        ui.horizontal_wrapped(|ui| {
            for c in &d.categories {
                let mut on = input.categories.contains(&c.slug);
                if theme::chip(ui, on, d.category_label(&c.slug)).clicked() {
                    on = !on;
                    if on {
                        input.categories.push(c.slug.clone());
                    } else {
                        input.categories.retain(|x| x != &c.slug);
                    }
                }
            }
        });
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            if theme::primary(ui, "Save").clicked() {
                let input = input.clone();
                actions.push(Action::db("Game saved", move |c| repo::save_game(c, Some(id), &input).map(|_| ())));
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let used = d.tournaments.iter().any(|t| {
                    t.rounds.iter().any(|r| r.game_id == Some(id)) || t.nominations.iter().any(|n| n.game_id == id)
                });
                let resp = ui.add_enabled(!used, egui::Button::new(RichText::new("Delete").color(theme::DANGER)));
                if resp
                    .on_disabled_hover_text("Played or nominated games can't be deleted; untick Available instead")
                    .clicked()
                {
                    actions.push(Action::Confirm(Confirm::new(
                        format!("Delete {}?", g.name),
                        "You can undo this right after.",
                        "Delete",
                        Action::db(format!("{} deleted", g.name), move |c| repo::delete_game(c, id)),
                    )));
                }
            });
        });
    });
}
