//! Publish: what will change on the website, the data checks, and sending it to GitHub.

use eframe::egui::{self, RichText};

use super::{Action, Confirm, Deploy, Mode, Page, Session, theme};
use crate::config::Config;
use crate::github::RunState;
use crate::sync::{ChangeKind, summary};

#[derive(Default)]
pub struct Form {
    pub message: String,
}

pub fn show(
    ui: &mut egui::Ui,
    s: &Session,
    config: &Config,
    deploy: Option<&Deploy>,
    form: &mut Form,
    actions: &mut Vec<Action>,
) {
    theme::page_title(
        ui,
        "Publish",
        Some("Send your changes to GitHub. The website updates itself a minute or two later."),
    );

    match s.mode {
        Mode::Practice => {
            theme::callout(ui, theme::CHALLENGER, |ui| {
                ui.label("This is the practice database: it can't be published. Leave practice mode to work on the real data.");
            });
            checks(ui, s);
            return;
        }
        Mode::Folder => {
            theme::callout(ui, theme::INFO, |ui| {
                ui.label(format!("You opened the folder {} directly. The JSON is already exported there; commit database/, data/ and avatars/ with git and push to publish.", s.ws.root.display()));
            });
            checks(ui, s);
            return;
        }
        Mode::Synced => {}
    }

    if let Some(d) = deploy {
        deployment(ui, d, config, actions);
        ui.add_space(8.0);
    }

    let signed_in = config.github_token.is_some();
    if !signed_in {
        theme::callout(ui, theme::INFO, |ui| {
            ui.horizontal(|ui| {
                ui.label("Sign in to GitHub to publish.");
                if theme::primary(ui, "Open Settings").clicked() {
                    actions.push(Action::Go(Page::Settings));
                }
            });
        });
        ui.add_space(8.0);
    }

    ui.columns(2, |cols| {
        let ui = &mut cols[0];
        theme::card(ui, |ui| {
            theme::section_title(ui, "What will change");
            let pages: Vec<_> = s.pending.iter().filter(|c| c.label.as_deref().is_some_and(|l| !l.starts_with("Link preview"))).collect();
            let previews = s.pending.iter().filter(|c| c.label.as_deref().is_some_and(|l| l.starts_with("Link preview"))).count();
            let db_changed = s.pending.iter().any(|c| c.label.is_none());
            if s.pending.is_empty() {
                ui.label(RichText::new("✔ Nothing to publish — the website already shows everything.").color(theme::LIVE));
            } else {
                if db_changed {
                    ui.label("🗄  The database (the record of every result)");
                }
                for c in &pages {
                    let (mark, color) = match c.kind {
                        ChangeKind::New => ("+", theme::LIVE),
                        ChangeKind::Changed => ("~", theme::GOLD),
                        ChangeKind::Removed => ("−", theme::DANGER),
                    };
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(mark).color(color).strong().monospace());
                        ui.label(c.label.clone().unwrap_or_default());
                    });
                }
                if previews > 0 {
                    theme::hint(ui, format!("and {previews} link preview page(s) for sharing"));
                }
            }
        });
        theme::card(&mut cols[1], |ui| {
            theme::section_title(ui, "Send it");
            let default = summary(&s.pending);
            ui.label(theme::muted("Description (optional)"));
            ui.add(egui::TextEdit::singleline(&mut form.message).hint_text(&default).desired_width(f32::INFINITY));
            let ok = s.report.ok();
            let ready = ok && signed_in && !s.pending.is_empty() && s.sync.is_some();
            ui.horizontal(|ui| {
                if theme::big_primary(ui, ready, "🚀  Publish to the website").clicked() {
                    let message = if form.message.trim().is_empty() { default.clone() } else { form.message.trim().to_string() };
                    actions.push(Action::Publish { message, overwrite: false });
                }
                if !ok {
                    ui.label(RichText::new("Fix the errors below first.").color(theme::DANGER));
                } else if s.sync.is_none() && signed_in {
                    ui.label(RichText::new("Download the data once first (below).").color(theme::GOLD));
                }
            });
            ui.add_space(6.0);
            ui.separator();
            match &s.sync {
                Some(st) => {
                    theme::hint(ui, format!("Last synced with {} ({}) at {} — commit {}.", st.repo, st.branch, st.at.replace('T', " ").trim_end_matches('Z'), &st.commit[..7.min(st.commit.len())]));
                }
                None => {
                    theme::hint(ui, "Never synced with GitHub.");
                }
            }
            ui.horizontal(|ui| {
                let download = ui.add_enabled(signed_in, egui::Button::new("⬇ Download latest from GitHub"));
                if download.on_hover_text("Get what others published. Replaces the data on this computer.").clicked() {
                    if s.pending.is_empty() {
                        actions.push(Action::Download);
                    } else {
                        actions.push(Action::Confirm(Confirm::new(
                            "Discard your unpublished changes?",
                            "Downloading replaces the data on this computer with GitHub's. Changes you haven't published will be lost.",
                            "Download and discard mine",
                            Action::Download,
                        )));
                    }
                }
                if ui.button("View on GitHub").clicked() {
                    actions.push(Action::OpenUrl(config.repo.web_url()));
                }
            });
        });
    });
    ui.add_space(8.0);
    checks(ui, s);
}

fn deployment(ui: &mut egui::Ui, d: &Deploy, config: &Config, actions: &mut Vec<Action>) {
    let (color, text) = match (&d.run, &d.error) {
        (_, Some(e)) => (theme::SUBTLE, format!("Couldn't check the website update: {e}")),
        (None, None) => (theme::INFO, "Sent. Waiting for GitHub to start updating the website…".to_string()),
        (Some(run), None) => match run.state {
            RunState::Queued => (theme::INFO, "Sent. The website update is queued…".to_string()),
            RunState::Running => (theme::INFO, "Sent. The website is updating (checks, then deploy)…".to_string()),
            RunState::Success => (
                theme::LIVE,
                "✔ Live! The website shows your changes (it may take a few minutes for caches to clear).".to_string(),
            ),
            RunState::Failed => {
                (theme::DANGER, "The website update failed on GitHub. Open the run to see why.".to_string())
            }
        },
    };
    theme::callout(ui, color, |ui| {
        ui.horizontal(|ui| {
            if matches!(d.run.as_ref().map(|r| &r.state), None | Some(RunState::Queued | RunState::Running))
                && d.error.is_none()
            {
                ui.spinner();
            }
            ui.label(RichText::new(text).strong());
        });
        ui.horizontal(|ui| {
            if let Some(run) = &d.run
                && ui.link("Open the GitHub Actions run").clicked()
            {
                actions.push(Action::OpenUrl(run.url.clone()));
            }
            if ui.link(format!("Commit {}", &d.commit[..7.min(d.commit.len())])).clicked() {
                actions.push(Action::OpenUrl(format!("{}/commit/{}", config.repo.web_url(), d.commit)));
            }
        });
    });
}

fn checks(ui: &mut egui::Ui, s: &Session) {
    theme::card(ui, |ui| {
        theme::section_title(ui, "Data checks");
        if s.report.errors.is_empty() && s.report.warnings.is_empty() {
            ui.label(RichText::new("✔ Everything looks good.").color(theme::LIVE));
        }
        for e in &s.report.errors {
            ui.label(RichText::new(format!("✖ {e}")).color(theme::DANGER));
        }
        if !s.report.warnings.is_empty() {
            ui.add_space(4.0);
            theme::hint(ui, "Warnings don't block publishing, but are worth a look:");
            for w in &s.report.warnings {
                ui.label(RichText::new(format!("⚠ {w}")).color(theme::GOLD));
            }
        }
    });
}
