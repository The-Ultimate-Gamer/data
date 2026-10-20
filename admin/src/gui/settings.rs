//! Settings: GitHub sign-in and repository, practice mode, demo data, and where things are stored.

use eframe::egui::{self, RichText};

use super::{Action, Confirm, DeviceSignIn, Mode, Session, theme, welcome};
use crate::config::Config;
use crate::seed;

#[derive(Default)]
pub struct Form {
    pub token: String,
    discord_token: String,
    repo: Option<(String, String, String)>,
}

pub fn show(
    ui: &mut egui::Ui,
    s: &Session,
    config: &Config,
    device: Option<&DeviceSignIn>,
    form: &mut Form,
    actions: &mut Vec<Action>,
) {
    theme::page_title(ui, "Settings", None);
    let repo = form
        .repo
        .get_or_insert_with(|| (config.repo.owner.clone(), config.repo.name.clone(), config.repo.branch.clone()));

    ui.columns(2, |cols| {
        theme::card(&mut cols[0], |ui| {
            theme::section_title(ui, "GitHub");
            match (&config.github_login, &config.github_token) {
                (Some(login), Some(_)) => {
                    ui.horizontal(|ui| {
                        ui.label(format!("Signed in as {login}."));
                        if ui.button("Sign out").clicked() {
                            actions.push(Action::SignOut);
                        }
                    });
                }
                _ => {
                    ui.label(theme::muted("Not signed in. Publishing and downloading need a GitHub sign-in."));
                }
            }
            if welcome::device_sign_in(ui, device, false, actions) {
                ui.add_space(4.0);
                ui.label(theme::muted("Or use a token:"));
            }
            ui.horizontal(|ui| {
                ui.add(egui::TextEdit::singleline(&mut form.token).password(true).hint_text("paste a new token").desired_width(260.0));
                if ui.add_enabled(!form.token.trim().is_empty(), egui::Button::new("Sign in")).clicked() {
                    actions.push(Action::SignIn(form.token.trim().to_string()));
                }
                if ui.button("Use `gh` login").clicked() {
                    match crate::config::token_from_gh_cli() {
                        Some(token) => actions.push(Action::SignIn(token)),
                        None => actions.push(Action::Toast("The GitHub CLI isn't installed or isn't logged in.".into(), true)),
                    }
                }
            });
            theme::hint(ui, "A fine-grained token for the data repository with Contents: Read and write and Actions: Read-only.");
            ui.add_space(6.0);
            ui.label(theme::muted("Repository"));
            ui.horizontal(|ui| {
                theme::narrow_text(ui, &mut repo.0, "owner", 150.0);
                ui.label("/");
                theme::narrow_text(ui, &mut repo.1, "name", 110.0);
                ui.label("branch");
                theme::narrow_text(ui, &mut repo.2, "main", 80.0);
            });
            let changed = (repo.0.trim(), repo.1.trim(), repo.2.trim()) != (config.repo.owner.as_str(), config.repo.name.as_str(), config.repo.branch.as_str());
            if changed {
                ui.horizontal(|ui| {
                    if ui.button("Save repository").clicked() {
                        let mut next = config.clone();
                        next.repo.owner = repo.0.trim().into();
                        next.repo.name = repo.1.trim().into();
                        next.repo.branch = repo.2.trim().into();
                        actions.push(Action::SaveConfig(next));
                        actions.push(Action::Toast("Saved. Download from the new repository before publishing.".into(), false));
                    }
                    theme::hint(ui, "You'll need to download from it before publishing.");
                });
            }
        });

        cols[0].add_space(8.0);
        theme::card(&mut cols[0], |ui| {
            theme::section_title(ui, "Discord");
            ui.label("Optional. Lets the Players page copy a player's Discord name and picture. Only this app talks to Discord, and only when you press Fetch.");
            if config.discord_token.is_some() {
                ui.horizontal(|ui| {
                    ui.label("A bot token is saved.");
                    if ui.button("Remove").clicked() {
                        let mut next = config.clone();
                        next.discord_token = None;
                        actions.push(Action::SaveConfig(next));
                    }
                });
            }
            ui.horizontal(|ui| {
                ui.add(egui::TextEdit::singleline(&mut form.discord_token).password(true).hint_text("paste a bot token").desired_width(260.0));
                if ui.add_enabled(!form.discord_token.trim().is_empty(), egui::Button::new("Save")).clicked() {
                    let mut next = config.clone();
                    next.discord_token = Some(std::mem::take(&mut form.discord_token).trim().to_string());
                    actions.push(Action::SaveConfig(next));
                    actions.push(Action::Toast("Discord bot token saved.".into(), false));
                }
            });
            if ui.link("Create a bot at discord.com/developers/applications → Bot → Reset Token").clicked() {
                actions.push(Action::OpenUrl("https://discord.com/developers/applications".into()));
            }
            theme::hint(ui, "The bot needs no permissions and doesn't have to join a server. The token is stored only in this computer's settings file.");
        });

        let ui = &mut cols[1];
        theme::card(ui, |ui| {
            theme::section_title(ui, "Practice");
            ui.label("A separate database full of fictional data, to try things out. Nothing in it is ever published.");
            ui.horizontal(|ui| {
                if s.mode == Mode::Practice {
                    if ui.button("Leave practice").clicked() {
                        actions.push(Action::LeavePractice);
                    }
                    if ui.button("Reset practice data").clicked() {
                        actions.push(Action::Confirm(Confirm::new(
                            "Reset the practice database?",
                            "Everything you did in practice mode is replaced by fresh demo data.",
                            "Reset",
                            Action::EnterPractice { reset: true },
                        )));
                    }
                } else if ui.button("🎮 Enter practice mode").clicked() {
                    actions.push(Action::EnterPractice { reset: false });
                }
            });
        });
        ui.add_space(8.0);
        theme::card(ui, |ui| {
            theme::section_title(ui, "Demo data on the website");
            ui.label("To show the website off before the first real tournament, add fictional players, games and two tournaments. They're flagged, the website shows a demo banner, and removing them takes out exactly those rows.");
            ui.horizontal(|ui| {
                if s.data.demo_present {
                    if theme::danger(ui, "Remove demo data").clicked() {
                        actions.push(Action::Confirm(Confirm::new(
                            "Remove the demo data?",
                            "Every fictional tournament, player and unused game is deleted. Real data isn't touched. Publish afterwards to take it off the website.",
                            "Remove",
                            Action::db_msg("Demo data removed", |c| {
                                let r = seed::remove_demo(c)?;
                                Ok(format!("Removed {} demo tournaments, {} players and {} games. Publish to update the website.", r.tournaments, r.players, r.games))
                            }),
                        )));
                    }
                } else if ui.button("Add demo data").clicked() {
                    actions.push(Action::db("Demo data added — publish to show it on the website", seed::add_demo));
                }
            });
        });
        ui.add_space(8.0);
        theme::card(ui, |ui| {
            theme::section_title(ui, "Storage");
            ui.label(theme::muted("Data folder"));
            ui.label(RichText::new(s.ws.root.display().to_string()).monospace().small());
            ui.label(theme::muted("Settings file"));
            ui.label(RichText::new(Config::location().display().to_string()).monospace().small());
            ui.horizontal(|ui| {
                if ui.button("Open data folder").clicked() {
                    actions.push(Action::OpenUrl(format!("file://{}", s.ws.root.display())));
                }
            });
            theme::hint(ui, format!("ug-admin {}", env!("CARGO_PKG_VERSION")));
        });
    });
}
