//! First start: connect to GitHub and download the data, or practise on demo data.

use eframe::egui::{self, RichText};

use super::{Action, App, DeviceSignIn, theme};

#[derive(Default)]
pub struct Form {
    pub token: String,
}

const TOKEN_URL: &str = "https://github.com/settings/personal-access-tokens/new";

/// "Sign in with GitHub" through the browser (device flow), or the code to approve while it runs.
/// Shows nothing when no OAuth App client id is set; returns whether it showed anything.
pub fn device_sign_in(ui: &mut egui::Ui, device: Option<&DeviceSignIn>, busy: bool, actions: &mut Vec<Action>) -> bool {
    if crate::github::oauth_client_id().is_none() {
        return false;
    }
    let Some(d) = device else {
        if theme::primary_enabled(ui, !busy, "Sign in with GitHub").clicked() {
            actions.push(Action::DeviceSignIn);
        }
        theme::hint(ui, "Opens GitHub in your browser, where you approve this app with a one-time code.");
        return true;
    };
    match &d.code {
        None => {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label("Asking GitHub for a sign-in code…");
            });
        }
        Some(code) => {
            ui.label("Enter this code on GitHub, then approve the app:");
            ui.label(RichText::new(&code.user_code).monospace().strong().size(26.0).color(theme::GOLD));
            ui.horizontal(|ui| {
                if theme::primary_enabled(ui, true, "Copy the code and open GitHub").clicked() {
                    ui.ctx().copy_text(code.user_code.clone());
                    actions.push(Action::OpenUrl(code.verification_uri.clone()));
                }
                ui.spinner();
                ui.label(theme::muted("Waiting for you to approve…"));
            });
        }
    }
    if ui.button("Cancel").clicked() {
        actions.push(Action::CancelDeviceSignIn);
    }
    true
}

pub fn show(ui: &mut egui::Ui, app: &mut App, actions: &mut Vec<Action>) {
    egui::ScrollArea::vertical().show(ui, |ui| {
        ui.vertical_centered(|ui| {
            ui.set_max_width(720.0);
            ui.add_space(40.0);
            ui.label(RichText::new(" UG ").background_color(theme::GOLD).color(theme::ON_GOLD).strong().size(28.0));
            ui.add_space(8.0);
            ui.heading(RichText::new("Ultimate Gamer Admin").strong().size(30.0));
            ui.label(theme::muted("Record every game, then publish the results to the website."));
            ui.add_space(24.0);
        });
        ui.vertical_centered(|ui| {
            ui.set_max_width(720.0);
            ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                if let Some(err) = app.open_error() {
                    theme::callout(ui, theme::DANGER, |ui| {
                        ui.label(RichText::new("The database couldn't be opened").strong());
                        ui.label(err);
                    });
                    ui.add_space(8.0);
                }
                let busy = app.busy();
                theme::card(ui, |ui| {
                    theme::section_title(ui, "Connect to GitHub");
                    ui.label(format!(
                        "The data lives in the {} repository. Sign in once and the app downloads it; Publish sends your changes back.",
                        app.config.repo.full_name()
                    ));
                    if let Some(login) = &app.config.github_login
                        && app.config.github_token.is_some()
                    {
                        ui.horizontal(|ui| {
                            ui.label(format!("Signed in as {login}."));
                            if theme::primary_enabled(ui, !busy, "Download the data").clicked() {
                                actions.push(Action::Download);
                            }
                        });
                        return;
                    }
                    ui.add_space(4.0);
                    let device = device_sign_in(ui, app.device.as_ref(), busy, actions);
                    if device {
                        ui.add_space(8.0);
                        ui.label(theme::muted("Or use a token instead:"));
                    }
                    egui::CollapsingHeader::new("How to get a token (2 minutes)").default_open(!device).show(ui, |ui| {
                        ui.horizontal_wrapped(|ui| {
                            ui.label("1.");
                            if ui.link("Create a fine-grained token on GitHub").clicked() {
                                actions.push(Action::OpenUrl(TOKEN_URL.into()));
                            }
                        });
                        ui.label(format!("2. Resource owner: {}. Repository access: only {}.", app.config.repo.owner, app.config.repo.full_name()));
                        ui.label("3. Permissions ➡ Contents: Read and write, Actions: Read-only.");
                        ui.label("4. Generate it and paste it below. It is stored only on this computer.");
                    });
                    ui.horizontal(|ui| {
                        ui.add(egui::TextEdit::singleline(&mut app.forms.welcome.token).password(true).hint_text("github_pat_…").desired_width(360.0));
                        let ready = !app.forms.welcome.token.trim().is_empty() && !busy;
                        if theme::primary_enabled(ui, ready, "Sign in and download").clicked() {
                            actions.push(Action::SignIn(app.forms.welcome.token.trim().to_string()));
                        }
                    });
                    ui.horizontal(|ui| {
                        theme::hint(ui, "Already use the GitHub CLI?");
                        if ui.add_enabled(!busy, egui::Button::new("Use my `gh` login").small()).clicked() {
                            match crate::config::token_from_gh_cli() {
                                Some(token) => actions.push(Action::SignIn(token)),
                                None => actions.push(Action::Toast("The GitHub CLI isn't installed or isn't logged in (`gh auth login`).".into(), true)),
                            }
                        }
                    });
                });
                ui.add_space(8.0);
                theme::card(ui, |ui| {
                    theme::section_title(ui, "Just look around");
                    ui.label("Open a practice database full of fictional demo data. Nothing you do there is published.");
                    if ui.add_enabled(!busy, egui::Button::new("🎮 Practise with demo data")).clicked() {
                        actions.push(Action::EnterPractice { reset: false });
                    }
                });
                ui.add_space(8.0);
                theme::card(ui, |ui| {
                    theme::section_title(ui, "Brand-new setup");
                    ui.label("Start an empty database on this computer. Only for the very first time; normally you download the data from GitHub.");
                    if ui.add_enabled(!busy, egui::Button::new("Start an empty database")).clicked() {
                        actions.push(Action::StartEmpty);
                    }
                });
            });
        });
    });
}
