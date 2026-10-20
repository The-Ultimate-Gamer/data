//! Players: quick add, search, and an editor with the player's picture.

use eframe::egui::{self, RichText, Vec2};

use super::{Action, Confirm, Data, Session, theme};
use crate::repo::{self, PlayerInput};

#[derive(Default)]
pub struct Form {
    search: String,
    quick_discord: String,
    /// The player being edited, the data revision it was loaded at, and the values.
    editing: Option<(i64, u64, PlayerInput)>,
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
    ui.horizontal(|ui| {
        theme::page_title(
            ui,
            "Players",
            Some("Everyone who has ever played. Names and pictures come from their Discord accounts."),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let linked = d.players.iter().filter(|p| p.discord_id.is_some()).count();
            let resp = ui.add_enabled(discord_ready && linked > 0, egui::Button::new("🔄 Refresh all from Discord"));
            let resp = if discord_ready {
                resp.on_hover_text("Copies every player's Discord name and picture")
            } else {
                resp.on_disabled_hover_text("Add a Discord bot token in Settings first")
            };
            if resp.clicked() {
                actions.push(Action::RefreshDiscord(None));
            }
        });
    });

    ui.horizontal(|ui| {
        let resp = theme::narrow_text(ui, &mut form.quick_discord, "Discord user ID of a new player", 260.0);
        let enter = resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        let ready = !form.quick_discord.trim().is_empty();
        if (enter || ui.add_enabled(ready, egui::Button::new("+ Add player")).clicked()) && ready {
            let input = PlayerInput {
                discord_id: std::mem::take(&mut form.quick_discord).trim().to_string(),
                ..Default::default()
            };
            let discord_id = input.discord_id.clone();
            actions.push(Action::db("Player added", move |c| repo::save_player(c, None, &input).map(|_| ())));
            if discord_ready {
                actions.push(Action::RefreshDiscord(Some(vec![discord_id])));
            }
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            theme::narrow_text(ui, &mut form.search, "Search", 160.0);
            ui.label("🔍");
        });
    });
    theme::hint(
        ui,
        "Discord user ID: in Discord, Settings → Advanced → Developer Mode, then right-click the player → Copy User ID.",
    );
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
                Some((id, _, input)) => editor(ui, ctx, s, *id, input, discord_ready, actions),
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
        let resp = theme::clickable_row(ui, selected, |ui| {
            avatar(ui, d, &p.slug, &p.name, 32.0);
            ui.vertical(|ui| {
                let name = RichText::new(&p.name).strong();
                ui.label(if selected { name.color(theme::GOLD) } else { name });
                let discord = match (&p.discord_username, &p.discord_id) {
                    (Some(user), _) => format!(" · @{user}"),
                    (None, Some(_)) => " · Discord not fetched".to_string(),
                    (None, None) => String::new(),
                };
                theme::hint(
                    ui,
                    format!(
                        "/players/{} · {} tournament{}{discord}",
                        p.slug,
                        tournaments,
                        if tournaments == 1 { "" } else { "s" }
                    ),
                );
            });
            if p.is_demo {
                theme::badge(ui, "Demo", theme::CHALLENGER);
            } else if p.discord_id.is_none() {
                theme::badge(ui, "No Discord ID", theme::DANGER);
            }
        });
        if resp.clicked() {
            form.editing = Some((p.id, d.rev, PlayerInput::from(p)));
        }
        ui.add_space(2.0);
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
    discord_ready: bool,
    actions: &mut Vec<Action>,
) {
    let d = &s.data;
    let Some(p) = d.player(id) else { return };
    theme::card(ui, |ui| {
        ui.horizontal(|ui| {
            avatar(ui, d, &p.slug, &p.name, 64.0);
            ui.vertical(|ui| {
                ui.label(RichText::new(&p.name).strong().size(20.0));
                if let Some(user) = &p.discord_username {
                    let shown = p.discord_display_name.as_deref().unwrap_or(user);
                    ui.label(theme::muted(format!("Discord: {shown} (@{user})")));
                }
                if ui.link(format!("{}players/{}", d.site_url, p.slug)).clicked() {
                    actions.push(Action::OpenUrl(format!("{}players/{}", d.site_url, p.slug)));
                }
            });
        });
        ui.add_space(6.0);
        egui::Grid::new("player-editor").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
            theme::field(ui, "Discord user ID", |ui| {
                ui.vertical(|ui| {
                    theme::narrow_text(ui, &mut input.discord_id, "e.g. 80351110224678912", 260.0);
                    if input.discord_id.trim().is_empty() && !p.is_demo {
                        ui.label(
                            RichText::new("Required: right-click them in Discord → Copy User ID.")
                                .color(theme::GOLD)
                                .small(),
                        );
                    }
                })
            });
            theme::field(ui, "Nickname", |ui| {
                let username = p.discord_username.as_deref().unwrap_or("their Discord username");
                ui.vertical(|ui| {
                    theme::narrow_text(ui, &mut input.nickname, username, 260.0);
                    theme::hint(ui, "Optional. Shown instead of their Discord username.");
                })
            });
        });
        let shown = repo::player_name(
            Some(input.nickname.as_str()),
            p.discord_username.as_deref(),
            Some(input.discord_id.trim()),
        );
        if let Some(name) = shown.filter(|n| crate::db::slugify(n) != crate::db::slugify(&p.name)) {
            ui.label(
                RichText::new(format!(
                    "⚠ Saving renames them to {name}, so links shared to /players/{} stop working.",
                    p.slug
                ))
                .color(theme::GOLD)
                .small(),
            );
        }
        ui.horizontal(|ui| {
            if theme::primary(ui, "Save").clicked() {
                let input = input.clone();
                let old_slug = p.slug.clone();
                let dir = d.avatars_dir.clone();
                let upload = d.avatars.get(&p.slug).cloned();
                let refetch = (input.discord_id.trim() != p.discord_id.as_deref().unwrap_or("") && discord_ready)
                    .then(|| input.discord_id.trim().to_string());
                actions.push(Action::db("Player saved", move |c| {
                    repo::save_player(c, Some(id), &input)?;
                    // Keep the pictures with the player when the web address changes.
                    let new_slug = crate::model::players(c)?
                        .into_iter()
                        .find(|p| p.id == id)
                        .map(|p| p.slug)
                        .unwrap_or(old_slug.clone());
                    if new_slug != old_slug {
                        if let Some(file) = upload {
                            let ext = file.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
                            std::fs::rename(&file, file.with_file_name(format!("{new_slug}.{ext}")))?;
                        }
                        let old = dir.join(crate::export::discord_picture_name(&old_slug));
                        if old.is_file() {
                            std::fs::rename(old, dir.join(crate::export::discord_picture_name(&new_slug)))?;
                        }
                    }
                    Ok(())
                }));
                if let Some(discord_id) = refetch {
                    actions.push(Action::RefreshDiscord(Some(vec![discord_id])));
                }
            }
            if let Some(discord_id) = p.discord_id.clone() {
                let resp = ui.add_enabled(discord_ready, egui::Button::new("🔄 Fetch from Discord"));
                if resp.on_disabled_hover_text("Add a Discord bot token in Settings first").clicked() {
                    actions.push(Action::RefreshDiscord(Some(vec![discord_id])));
                }
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let used = d.tournaments.iter().any(|t| t.participants.contains(&id));
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
        let shown = d.pictures.get(&p.slug).map(|(source, _)| *source);
        ui.label(match shown {
            Some(source) => format!("Showing their {}.", source.label()),
            None if p.discord_id.is_some() => "Showing initials until their Discord picture is fetched.".to_string(),
            None => "Showing initials.".to_string(),
        });
        theme::hint(
            ui,
            "Their Discord picture is used automatically. To override it, upload a picture (drop an image file on this window) or link one. An upload wins over a link.",
        );
        ui.horizontal(|ui| {
            if ui.button("Upload picture…").clicked()
                && let Some(path) =
                    rfd::FileDialog::new().add_filter("Pictures", &["jpg", "jpeg", "png", "webp", "gif"]).pick_file()
            {
                actions.push(Action::SetAvatar { slug: p.slug.clone(), from: path });
            }
            if d.avatars.contains_key(&p.slug)
                && ui.button(RichText::new("Remove upload").color(theme::DANGER)).clicked()
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
            theme::hint(ui, "Used when there's no upload. Must start with https://. Clear it to go back to Discord.");
            ui.horizontal(|ui| {
                theme::narrow_text(ui, &mut input.avatar_url, "https://…", 300.0);
                if ui.button("Save").clicked() {
                    let input = input.clone();
                    actions.push(Action::db("Picture link saved", move |c| {
                        repo::save_player(c, Some(id), &input).map(|_| ())
                    }));
                }
            });
        });
    });
}
