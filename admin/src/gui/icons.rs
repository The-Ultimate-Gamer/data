//! An emoji picker for category and award icons: click the icon, pick one from the palette
//! (hover shows its name), or type/paste any emoji in the box at the bottom.

use eframe::egui::{self, FontFamily, FontId, RichText};

use super::theme;

const GROUPS: &[(&str, &[(&str, &str)])] = &[
    (
        "Games",
        &[
            ("🎮", "controller"),
            ("🕹️", "joystick"),
            ("🎲", "dice"),
            ("🃏", "joker"),
            ("♟️", "chess"),
            ("🧩", "puzzle"),
            ("🎯", "target"),
            ("🏁", "chequered flag"),
            ("🏎️", "race car"),
            ("⚽", "football"),
            ("🏀", "basketball"),
            ("🎳", "bowling"),
            ("🎱", "pool"),
            ("🏓", "ping pong"),
            ("🥊", "boxing glove"),
            ("⚔️", "crossed swords"),
            ("🔫", "blaster"),
            ("🏹", "bow"),
            ("🛡️", "shield"),
            ("🍄", "mushroom"),
            ("🌀", "cyclone"),
            ("🧠", "brain"),
            ("🛠️", "tools"),
            ("⏳", "hourglass"),
            ("⏱️", "stopwatch"),
            ("🤝", "handshake"),
            ("🚀", "rocket"),
            ("🧱", "bricks"),
            ("🗺️", "map"),
            ("🎵", "music"),
            ("🎨", "palette"),
            ("📚", "books"),
        ],
    ),
    (
        "Awards",
        &[
            ("👑", "crown"),
            ("🏆", "trophy"),
            ("🥇", "gold medal"),
            ("🥈", "silver medal"),
            ("🥉", "bronze medal"),
            ("🏅", "medal"),
            ("🎖️", "military medal"),
            ("⭐", "star"),
            ("🌟", "glowing star"),
            ("✨", "sparkles"),
            ("🔥", "fire"),
            ("💎", "gem"),
            ("📈", "chart up"),
            ("📉", "chart down"),
            ("🙏", "folded hands"),
            ("🗳️", "ballot box"),
            ("🌱", "seedling"),
            ("🐕", "dog"),
            ("☢️", "radioactive"),
            ("🏛️", "monument"),
            ("💪", "muscle"),
            ("⚡", "lightning"),
            ("🧊", "ice"),
            ("🎉", "party"),
        ],
    ),
    (
        "Faces & creatures",
        &[
            ("😈", "devil"),
            ("👻", "ghost"),
            ("💀", "skull"),
            ("🤖", "robot"),
            ("👾", "space invader"),
            ("😎", "cool"),
            ("🤯", "mind blown"),
            ("🥳", "celebrating"),
            ("🤡", "clown"),
            ("🐉", "dragon"),
            ("🦄", "unicorn"),
            ("🐢", "turtle"),
            ("🐇", "rabbit"),
            ("🦊", "fox"),
            ("🩹", "bandage"),
            ("🍕", "pizza"),
        ],
    ),
];

/// A button showing `value`; clicking opens the palette. Returns true when the icon changed.
pub fn picker(ui: &mut egui::Ui, id: impl std::hash::Hash, value: &mut String) -> bool {
    let mut changed = false;
    let shown = if value.trim().is_empty() { "＋".to_string() } else { value.clone() };
    ui.push_id(id, |ui| {
        ui.menu_button(RichText::new(shown).size(20.0), |ui| {
            ui.set_max_width(360.0);
            let font = FontId::new(20.0, FontFamily::Proportional);
            for (group, icons) in GROUPS {
                ui.label(theme::muted(*group).small());
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(2.0, 2.0);
                    for (icon, name) in *icons {
                        // Skip emoji the app's fonts can't draw (they'd show as boxes here).
                        let drawable = ui.fonts_mut(|f| {
                            icon.chars()
                                .filter(|c| !matches!(c, '\u{fe0f}' | '\u{200d}'))
                                .all(|c| f.has_glyph(&font, c))
                        });
                        if !drawable {
                            continue;
                        }
                        let selected = value == icon;
                        if ui
                            .add(
                                egui::Button::selectable(selected, RichText::new(*icon).size(20.0))
                                    .min_size(egui::vec2(34.0, 34.0)),
                            )
                            .on_hover_text(*name)
                            .clicked()
                        {
                            *value = (*icon).to_string();
                            changed = true;
                            ui.close();
                        }
                    }
                });
            }
            ui.separator();
            ui.horizontal(|ui| {
                ui.label(theme::muted("Or paste one:"));
                changed |= theme::narrow_text(ui, value, "e.g. 🎯", 80.0).changed();
                theme::hint(ui, "Windows: Win + .   macOS: Ctrl + Cmd + Space");
            });
        });
    });
    changed
}
