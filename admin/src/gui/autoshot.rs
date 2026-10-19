//! Developer aid: with `UG_AUTOSHOT=<folder>` the app visits every screen, saves a PNG of each
//! and quits. Used to check that every screen renders, without clicking through by hand.

use std::path::PathBuf;

use eframe::egui;

use super::{Page, Tab};

pub struct AutoShot {
    dir: PathBuf,
    /// (file name, page, scroll offset)
    pages: Vec<(String, Page, f32)>,
    index: usize,
    frames: u32,
    requested: bool,
}

impl AutoShot {
    pub fn from_env() -> Option<Self> {
        let dir = PathBuf::from(std::env::var_os("UG_AUTOSHOT")?);
        std::fs::create_dir_all(&dir).ok()?;
        Some(Self { dir, pages: Vec::new(), index: 0, frames: 0, requested: false })
    }

    /// The screens to visit, once the data is known.
    pub fn plan(&mut self, data: Option<&super::Data>) {
        if !self.pages.is_empty() {
            return;
        }
        let mut pages = vec![("home".to_string(), Page::Home), ("tournaments".into(), Page::Tournaments)];
        if let Some(t) = data.and_then(|d| d.focus()) {
            let id = t.t.id;
            pages.push(("tournament-overview".into(), Page::Tournament(id, Tab::Overview)));
            pages.push(("tournament-setup".into(), Page::Tournament(id, Tab::Setup)));
            pages.push(("tournament-awards".into(), Page::Tournament(id, Tab::Awards)));
            if let Some(n) = t.next_round() {
                pages.push(("round-next".into(), Page::Round(id, n)));
            }
            if let Some(done) = t.rounds.iter().rfind(|r| r.completed) {
                pages.push(("round-finished".into(), Page::Round(id, done.number)));
            }
        }
        pages.extend([
            ("players".into(), Page::Players),
            ("games".into(), Page::Games),
            ("library".into(), Page::Library),
            ("publish".into(), Page::Publish),
            ("settings".into(), Page::Settings),
        ]);
        let long = ["home", "round-next", "round-finished", "tournament-overview"];
        self.pages = pages
            .into_iter()
            .flat_map(|(name, page)| {
                let more = long.contains(&name.as_str()).then(|| (format!("{name}-lower"), page, 640.0));
                std::iter::once((name, page, 0.0)).chain(more)
            })
            .collect();
    }

    /// The page and scroll offset to show this frame; `None` when done (the window is then closed).
    pub fn step(&mut self, ctx: &egui::Context) -> Option<(Page, f32)> {
        let saved = ctx.input(|i| {
            i.raw.events.iter().find_map(|e| match e {
                egui::Event::Screenshot { image, .. } => Some(image.clone()),
                _ => None,
            })
        });
        if let Some(image) = saved {
            let (name, _, _) = &self.pages[self.index];
            let path = self.dir.join(format!("{:02}-{name}.png", self.index + 1));
            let [w, h] = image.size;
            let bytes: Vec<u8> = image.pixels.iter().flat_map(|c| c.to_array()).collect();
            if let Err(e) = image::save_buffer(&path, &bytes, w as u32, h as u32, image::ColorType::Rgba8) {
                eprintln!("autoshot: {e}");
            }
            eprintln!("autoshot: saved {}", path.display());
            self.index += 1;
            self.frames = 0;
            self.requested = false;
        }
        if self.index >= self.pages.len() {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return None;
        }
        self.frames += 1;
        // Give layout and image loading a few frames to settle.
        if self.frames > 8 && !self.requested {
            self.requested = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
        }
        ctx.request_repaint();
        let (_, page, offset) = &self.pages[self.index];
        Some((*page, *offset))
    }
}
