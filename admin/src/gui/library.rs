//! Reference data: categories, awards and the site's name and addresses.

use eframe::egui::{self, RichText};

use super::{Action, Session, icons, theme};
use crate::repo;

#[derive(Default)]
pub struct Form {
    rev: Option<u64>,
    categories: Vec<(String, String, String, String)>,
    awards: Vec<(String, String, String, String, String)>,
    new_category: (String, String, String),
    site: [String; 4],
}

const META_KEYS: [(&str, &str, &str); 4] = [
    ("site_name", "Site name", "The Ultimate Gamer"),
    ("site_tagline", "Tagline", "optional"),
    ("site_url", "Website address", "https://the-ultimate-gamer.github.io/"),
    ("data_url", "Data address", "<website address>data/"),
];

pub fn show(ui: &mut egui::Ui, s: &Session, form: &mut Form, actions: &mut Vec<Action>) {
    let d = &s.data;
    if form.rev != Some(d.rev) {
        form.rev = Some(d.rev);
        form.categories = d
            .categories
            .iter()
            .map(|c| {
                (
                    c.slug.clone(),
                    c.name.clone(),
                    c.icon.clone().unwrap_or_default(),
                    c.description.clone().unwrap_or_default(),
                )
            })
            .collect();
        form.awards = d
            .awards
            .iter()
            .map(|a| {
                (
                    a.slug.clone(),
                    a.name.clone(),
                    a.icon.clone().unwrap_or_default(),
                    a.description.clone().unwrap_or_default(),
                    a.scope.clone(),
                )
            })
            .collect();
        form.site = META_KEYS.map(|(key, _, _)| d.meta.get(key).cloned().unwrap_or_default());
    }

    theme::page_title(
        ui,
        "Categories & awards",
        Some("Shared by every tournament. Changes show on the website after publishing."),
    );

    theme::card(ui, |ui| {
        theme::section_title(ui, "Categories");
        egui::Grid::new("categories").num_columns(5).striped(true).spacing([10.0, 6.0]).show(ui, |ui| {
            for (slug, name, icon, description) in &mut form.categories {
                icons::picker(ui, ("category-icon", slug.as_str()), icon);
                theme::narrow_text(ui, name, "", 140.0);
                theme::narrow_text(ui, description, "description", 420.0);
                ui.label(theme::muted(slug.as_str()).small());
                if ui.small_button("Save").clicked() {
                    let (slug, name, icon, description) =
                        (slug.clone(), name.clone(), icon.clone(), description.clone());
                    actions.push(Action::db(format!("{name} saved"), move |c| {
                        repo::save_category(c, &slug, &name, &icon, &description, false)
                    }));
                }
                ui.end_row();
            }
            let (name, icon, description) = &mut form.new_category;
            icons::picker(ui, "new-category-icon", icon);
            theme::narrow_text(ui, name, "New category", 140.0);
            theme::narrow_text(ui, description, "description", 420.0);
            ui.label("");
            if ui.add_enabled(!name.trim().is_empty(), egui::Button::new("+ Add")).clicked() {
                let (name, icon, description) = std::mem::take(&mut form.new_category);
                actions.push(Action::db(format!("{} added", name.trim()), move |c| {
                    repo::save_category(c, "", &name, &icon, &description, true)
                }));
            }
            ui.end_row();
        });
    });
    ui.add_space(8.0);

    theme::card(ui, |ui| {
        theme::section_title(ui, "Awards");
        theme::hint(
            ui,
            "Awards are granted automatically by the rules, so each one is tied to a rule and new ones can't be added here. \
             You can change their name, icon and description. Click an icon to pick a new one.",
        );
        egui::Grid::new("awards").num_columns(5).striped(true).spacing([10.0, 6.0]).show(ui, |ui| {
            for (slug, name, icon, description, scope) in &mut form.awards {
                icons::picker(ui, ("award-icon", slug.as_str()), icon);
                theme::narrow_text(ui, name, "", 140.0);
                theme::narrow_text(ui, description, "description", 420.0);
                ui.label(theme::muted(if scope == "career" { "career" } else { "per tournament" }).small());
                if ui.small_button("Save").clicked() {
                    let (slug, name, icon, description, scope) =
                        (slug.clone(), name.clone(), icon.clone(), description.clone(), scope.clone());
                    actions.push(Action::db(format!("{name} saved"), move |c| {
                        repo::save_award(c, &slug, &name, &icon, &description, &scope, false)
                    }));
                }
                ui.end_row();
            }
        });
    });
    ui.add_space(8.0);

    theme::card(ui, |ui| {
        theme::section_title(ui, "Website");
        theme::hint(ui, "Used for the site title and link previews. Leave the addresses empty for the defaults.");
        egui::Grid::new("site").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
            for (i, (_, label, placeholder)) in META_KEYS.iter().enumerate() {
                theme::field(ui, label, |ui| theme::narrow_text(ui, &mut form.site[i], placeholder, 380.0));
            }
        });
        if theme::primary(ui, "Save").clicked() {
            let values = form.site.clone();
            actions.push(Action::db("Website settings saved", move |c| {
                for ((key, _, _), value) in META_KEYS.iter().zip(values.iter()) {
                    repo::set_meta(c, key, value)?;
                }
                Ok(())
            }));
        }
        ui.label(RichText::new("").small());
    });
}
