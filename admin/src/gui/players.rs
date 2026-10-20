//! Players: quick add, search, and an editor with the player's picture.

use eframe::egui::{self, RichText, Vec2};

use super::{Action, Confirm, Data, Session, theme};
use crate::repo::{self, PlayerInput};

#[derive(Default)]
pub struct Form {
    search: String,
    quick_name: String,
    /// The player being edited, the data revision it was loaded at, and the values.
    editing: Option<(i64, u64, PlayerInput)>,
    /// Also copy the Discord picture when fetching a profile.
    discord_picture: bool,
}

pub fn show(
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    s: &Session,
    discord_ready: bool,
    form: &mut Form,
    actions: &mut Vec<Action>,
) {
    let d = &s.data;
    theme::page_title(
        ui,
        "Players",
        Some("Everyone who has ever played. Their web address (slug) becomes their profile link."),
    );

    ui.horizontal(|ui| {
        let resp = theme::narrow_text(ui, &mut form.quick_name, "New player's name — press Enter", 300.0);
        let enter = resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        if (enter || ui.button("+ Add player").clicked()) && !form.quick_name.trim().is_empty() {
            let input = PlayerInput { name: std::mem::take(&mut form.quick_name), ..Default::default() };
            let label = format!("{} added", input.name.trim());
            actions.push(Action::db(label, move |c| repo::save_player(c, None, &input).map(|_| ())));
            resp.request_focus();
        }
        ui.add_space(16.0);
        ui.label("🔍");
        theme::narrow_text(ui, &mut form.search, "Search", 180.0);
    });
    ui.add_space(6.0);

    // Keep the editor in step with the data unless the admin is typing in it.
    if let Some((id, rev, _)) = &form.editing {
        match d.player(*id) {
            None => form.editing = None,
            Some(p) if *rev != d.rev => {
                let input = PlayerInput::from(p);
                form.editing = Some((p.id, d.rev, input));
            }
            _ => {}
        }
    }

    let editor_width = 420.0;
    ui.horizontal_top(|ui| {
        let list_width = (ui.available_width() - editor_width - 16.0).max(320.0);
        ui.vertical(|ui| {
            ui.set_width(list_width);
            theme::card(ui, |ui| list(ui, d, form, list_width));
        });
        ui.vertical(|ui| {
            ui.set_width(editor_width);
            match form.editing.as_mut() {
                Some((id, _, input)) => {
                    editor(ui, ctx, s, *id, input, actions);
                    discord(ui, s, *id, input, discord_ready, &mut form.discord_picture, actions);
                }
                None => {
                    theme::card(ui, |ui| {
                        ui.label(theme::muted("Select a player to edit their name, web address or picture."));
                    });
                }
            }
        });
    });
}

fn list(ui: &mut egui::Ui, d: &Data, form: &mut Form, _width: f32) {
    let q = form.search.trim().to_lowercase();
    let players: Vec<_> = d
        .players
        .iter()
        .filter(|p| q.is_empty() || p.name.to_lowercase().contains(&q) || p.slug.contains(&q))
        .collect();
    if players.is_empty() {
        ui.label(theme::muted(if d.players.is_empty() {
            "No players yet. Type a name above and press Enter."
        } else {
            "No player matches."
        }));
    }
    for p in players {
        let selected = form.editing.as_ref().is_some_and(|(id, _, _)| *id == p.id);
        let tournaments = d.tournaments.iter().filter(|t| t.participants.contains(&p.id)).count();
        let resp = ui
            .horizontal(|ui| {
                avatar(ui, d, &p.slug, &p.name, 32.0);
                ui.vertical(|ui| {
                    let name = RichText::new(&p.name).strong();
                    ui.label(if selected { name.color(theme::GOLD) } else { name });
                    theme::hint(
                        ui,
                        format!(
                            "/players/{} · {} tournament{}",
                            p.slug,
                            tournaments,
                            if tournaments == 1 { "" } else { "s" }
                        ),
                    );
                });
                if p.is_demo {
                    theme::badge(ui, "Demo", theme::CHALLENGER);
                }
                ui.allocate_space(Vec2::new(ui.available_width(), 1.0));
            })
            .response
            .interact(egui::Sense::click());
        if resp.hovered() {
            ui.painter().rect_stroke(
                resp.rect.expand(2.0),
                6.0,
                egui::Stroke::new(1.0_f32, theme::BORDER_STRONG),
                egui::StrokeKind::Outside,
            );
        }
        if resp.clicked() {
            form.editing = Some((p.id, d.rev, PlayerInput::from(p)));
        }
        ui.separator();
    }
}

/// The player's picture, or their initials in a circle.
pub fn avatar(ui: &mut egui::Ui, d: &Data, slug: &str, name: &str, size: f32) {
    if let Some(uri) = d.avatar_uri(slug) {
        ui.add(egui::Image::new(uri).fit_to_exact_size(Vec2::splat(size)).corner_radius(size / 2.0));
        return;
    }
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), egui::Sense::hover());
    ui.painter().circle_filled(rect.center(), size / 2.0, theme::SURFACE_3);
    let initials: String =
        name.split_whitespace().filter_map(|w| w.chars().next()).take(2).collect::<String>().to_uppercase();
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        initials,
        egui::FontId::proportional(size * 0.42),
        theme::MUTED,
    );
}

fn editor(
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    s: &Session,
    id: i64,
    input: &mut PlayerInput,
    actions: &mut Vec<Action>,
) {
    let d = &s.data;
    let Some(p) = d.player(id) else { return };
    theme::card(ui, |ui| {
        ui.horizontal(|ui| {
            avatar(ui, d, &p.slug, &p.name, 64.0);
            ui.vertical(|ui| {
                ui.label(RichText::new(&p.name).strong().size(20.0));
                if ui.link(format!("{}players/{}", d.site_url, p.slug)).clicked() {
                    actions.push(Action::OpenUrl(format!("{}players/{}", d.site_url, p.slug)));
                }
            });
        });
        ui.add_space(6.0);
        egui::Grid::new("player-editor").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
            theme::field(ui, "Name", |ui| theme::text(ui, &mut input.name, ""));
            theme::field(ui, "Web address", |ui| {
                ui.vertical(|ui| {
                    theme::text(ui, &mut input.slug, &p.slug);
                    if input.slug.trim() != p.slug && !input.slug.trim().is_empty() {
                        ui.label(
                            RichText::new("⚠ Links people already shared to this profile will stop working.")
                                .color(theme::GOLD)
                                .small(),
                        );
                    }
                })
            });
            theme::field(ui, "Initials", |ui| theme::narrow_text(ui, &mut input.initials, "optional", 80.0));
        });
        ui.horizontal(|ui| {
            if theme::primary(ui, "Save").clicked() {
                let input = input.clone();
                let old_slug = p.slug.clone();
                let avatars = d.avatars.get(&p.slug).cloned();
                actions.push(Action::db("Player saved", move |c| {
                    repo::save_player(c, Some(id), &input)?;
                    // Keep the picture with the player when the web address changes.
                    let new_slug = crate::model::players(c)?
                        .into_iter()
                        .find(|p| p.id == id)
                        .map(|p| p.slug)
                        .unwrap_or(old_slug.clone());
                    if let Some(file) = avatars.filter(|_| new_slug != old_slug) {
                        let ext = file.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
                        std::fs::rename(&file, file.with_file_name(format!("{new_slug}.{ext}")))?;
                    }
                    Ok(())
                }));
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let used = d
                    .tournaments
                    .iter()
                    .any(|t| t.participants.contains(&id) || t.grants.iter().any(|g| g.player_id == id));
                let resp = ui.add_enabled(!used, egui::Button::new(RichText::new("Delete").color(theme::DANGER)));
                if resp.on_disabled_hover_text("Players with tournament history can't be deleted").clicked() {
                    actions.push(Action::Confirm(Confirm::new(
                        format!("Delete {}?", p.name),
                        "You can undo this right after.",
                        "Delete",
                        Action::db(format!("{} deleted", p.name), move |c| repo::delete_player(c, id)),
                    )));
                }
            });
        });
    });
    ui.add_space(8.0);
    theme::card(ui, |ui| {
        theme::section_title(ui, "Picture");
        theme::hint(
            ui,
            "Stored with the data and published with it, so it never expires. Drop an image file on this window, or choose one.",
        );
        ui.horizontal(|ui| {
            if ui.button("Choose picture…").clicked()
                && let Some(path) =
                    rfd::FileDialog::new().add_filter("Pictures", &["jpg", "jpeg", "png", "webp", "gif"]).pick_file()
            {
                actions.push(Action::SetAvatar { slug: p.slug.clone(), from: path });
            }
            if d.avatars.contains_key(&p.slug)
                && ui.button(RichText::new("Remove picture").color(theme::DANGER)).clicked()
            {
                actions.push(Action::RemoveAvatar { slug: p.slug.clone() });
            }
        });
        let dropped = ctx.input(|i| i.raw.dropped_files.iter().find_map(|f| f.path.clone()));
        if let Some(path) = dropped {
            actions.push(Action::SetAvatar { slug: p.slug.clone(), from: path });
        }
        if ctx.input(|i| !i.raw.hovered_files.is_empty()) {
            theme::callout(ui, theme::GOLD, |ui| {
                ui.label(format!("Drop to use as {}'s picture", p.name));
            });
        }
        ui.add_space(4.0);
        egui::CollapsingHeader::new("Or link to a picture online").show(ui, |ui| {
            theme::hint(ui, "Used only when there's no stored picture. Must start with https://. Discord links expire, so use the Discord section below instead.");
            ui.horizontal(|ui| {
                theme::narrow_text(ui, &mut input.avatar_url, "https://…", 300.0);
                if ui.button("Save").clicked() {
                    let input = input.clone();
                    actions.push(Action::db("Picture link saved", move |c| repo::save_player(c, Some(id), &input).map(|_| ())));
                }
            });
        });
    });
}

/// Link the player's Discord account and copy its names and picture.
fn discord(
    ui: &mut egui::Ui,
    s: &Session,
    id: i64,
    input: &mut PlayerInput,
    ready: bool,
    picture: &mut bool,
    actions: &mut Vec<Action>,
) {
    let Some(p) = s.data.player(id) else { return };
    ui.add_space(8.0);
    theme::card(ui, |ui| {
        theme::section_title(ui, "Discord");
        theme::hint(
            ui,
            "Links the profile page to their Discord account. In Discord: Settings → Advanced → Developer Mode, then right-click the player → Copy User ID.",
        );
        ui.horizontal(|ui| {
            theme::narrow_text(ui, &mut input.discord_id, "User ID, e.g. 80351110224678912", 240.0);
            if input.discord_id.trim() != p.discord_id.as_deref().unwrap_or("") && ui.button("Save").clicked() {
                let input = input.clone();
                actions
                    .push(Action::db("Discord ID saved", move |c| repo::save_player(c, Some(id), &input).map(|_| ())));
            }
        });
        let Some(discord_id) = p.discord_id.clone() else { return };
        match (&p.discord_display_name, &p.discord_username) {
            (_, Some(user)) => {
                let shown = p.discord_display_name.as_deref().unwrap_or(user);
                ui.label(format!("{shown} (@{user})"));
            }
            _ => {
                ui.label(theme::muted("Name not fetched yet."));
            }
        }
        ui.horizontal(|ui| {
            let fetch = ui.add_enabled(ready, egui::Button::new("Fetch from Discord"));
            if fetch.on_disabled_hover_text("Add a Discord bot token in Settings first").clicked() {
                actions.push(Action::FetchDiscord {
                    player_id: id,
                    slug: p.slug.clone(),
                    discord_id: discord_id.clone(),
                    picture: *picture,
                });
            }
            ui.checkbox(picture, "Also use their Discord picture");
        });
        if ready {
            theme::hint(ui, "Copies their display name and @username. The picture replaces the stored one.");
        }
    });
}
