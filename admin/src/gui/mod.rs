//! The desktop app.
//!
//! Screens never touch the database directly: they return [`Action`]s. Every database change runs
//! through [`App::apply_db`], which keeps an undo snapshot, then re-exports the JSON, re-validates
//! and reloads what the screens show. Network work (GitHub) runs on a background thread.

mod autoshot;
mod data;
mod desktop;
mod games;
mod home;
mod icons;
mod library;
mod players;
mod publish;
mod round;
mod settings;
mod theme;
mod tournaments;
mod welcome;

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::{Duration, Instant};

use anyhow::{Result, anyhow};
use eframe::egui::{self, Color32, RichText};
use rusqlite::Connection;

use crate::config::Config;
use crate::github::{Access, DeviceCode, DeviceFlow, GitHub, Run, RunState};
use crate::repo::friendly;
use crate::sync::{self, Change, PublishOutcome, SyncState};
use crate::validate::Report;
use crate::workspace::Workspace;

pub use data::{Data, TournamentData};

const UNDO_LIMIT: usize = 50;

pub fn run(folder: Option<PathBuf>) -> Result<()> {
    desktop::install_launcher();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Ultimate Gamer Admin")
            .with_app_id("ug-admin")
            .with_icon(std::sync::Arc::new(app_icon()))
            .with_inner_size([1320.0, 860.0])
            .with_min_inner_size([960.0, 620.0]),
        ..Default::default()
    };
    eframe::run_native(
        "ug-admin",
        options,
        Box::new(move |cc| {
            egui_extras::install_image_loaders(&cc.egui_ctx);
            theme::apply(&cc.egui_ctx);
            Ok(Box::new(App::new(folder)))
        }),
    )
    .map_err(|e| anyhow!("could not open the window: {e}"))
}

/// Where the data lives and what publishing means for it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// The app's own folder, synced with GitHub. The normal case.
    Synced,
    /// A separate demo-only database to practise on. Never published.
    Practice,
    /// A folder chosen with `--repo` (a git checkout). Publishing is left to git.
    Folder,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tab {
    Overview,
    Setup,
    Awards,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Page {
    Home,
    Tournaments,
    Tournament(i64, Tab),
    Round(i64, i64),
    Players,
    Games,
    Library,
    Publish,
    Settings,
}

/// What a database change reports back: an optional message and page to open.
#[derive(Default)]
pub struct DbOk {
    pub message: Option<String>,
    pub go: Option<Page>,
}

pub type DbFn = Box<dyn FnOnce(&Connection) -> Result<DbOk>>;

/// Something a screen wants to happen. Applied after the frame is drawn.
pub enum Action {
    /// Change the database. `label` describes it (for the message and Undo).
    Db {
        label: String,
        f: DbFn,
    },
    Go(Page),
    Undo,
    Confirm(Confirm),
    Toast(String, bool),
    SignIn(String),
    /// Start signing in through GitHub's device flow (needs an OAuth App client id).
    DeviceSignIn,
    CancelDeviceSignIn,
    SignOut,
    Download,
    Publish {
        message: String,
        overwrite: bool,
    },
    /// Copy a picture into avatars/ as <slug>.<ext> (replacing any other picture of that player).
    SetAvatar {
        slug: String,
        from: PathBuf,
    },
    RemoveAvatar {
        slug: String,
    },
    EnterPractice {
        reset: bool,
    },
    LeavePractice,
    StartEmpty,
    SaveConfig(Config),
    OpenUrl(String),
    /// Copy names and pictures from Discord: these Discord user ids, or (None) every linked player.
    RefreshDiscord(Option<Vec<String>>),
}

impl Action {
    pub fn db(label: impl Into<String>, f: impl FnOnce(&Connection) -> Result<()> + 'static) -> Self {
        Action::Db { label: label.into(), f: Box::new(move |c| f(c).map(|()| DbOk::default())) }
    }

    /// A database change whose success message comes from the change itself.
    pub fn db_msg(label: impl Into<String>, f: impl FnOnce(&Connection) -> Result<String> + 'static) -> Self {
        Action::Db { label: label.into(), f: Box::new(move |c| f(c).map(|m| DbOk { message: Some(m), go: None })) }
    }

    /// A database change that opens a page afterwards (e.g. the tournament just created).
    pub fn db_go(label: impl Into<String>, f: impl FnOnce(&Connection) -> Result<Page> + 'static) -> Self {
        Action::Db { label: label.into(), f: Box::new(move |c| f(c).map(|p| DbOk { message: None, go: Some(p) })) }
    }
}

/// A question before something destructive.
pub struct Confirm {
    pub title: String,
    pub body: String,
    pub yes: String,
    pub danger: bool,
    pub action: Box<Action>,
}

impl Confirm {
    pub fn new(title: impl Into<String>, body: impl Into<String>, yes: impl Into<String>, action: Action) -> Self {
        Self { title: title.into(), body: body.into(), yes: yes.into(), danger: true, action: Box::new(action) }
    }
}

struct Toast {
    text: String,
    error: bool,
    until: Instant,
}

/// The open database and everything derived from it.
pub struct Session {
    pub mode: Mode,
    pub ws: Workspace,
    conn: Connection,
    pub data: Data,
    pub report: Report,
    pub sync: Option<SyncState>,
    pub pending: Vec<Change>,
    undo: Vec<(String, Vec<u8>)>,
}

impl Session {
    fn open(mode: Mode, ws: Workspace) -> Result<Self> {
        let conn = ws.open()?;
        let data = Data::load(&conn, &ws, 0)?;
        let mut session = Self {
            mode,
            sync: SyncState::load(&ws),
            ws,
            conn,
            data,
            report: Report::default(),
            pending: Vec::new(),
            undo: Vec::new(),
        };
        session.refresh()?;
        Ok(session)
    }

    /// Re-export, re-validate and reload after a change.
    fn refresh(&mut self) -> Result<()> {
        self.ws.export(&self.conn)?;
        self.report = self.ws.validate(&self.conn, false)?;
        self.data = Data::load(&self.conn, &self.ws, self.data.rev + 1)?;
        self.refresh_sync();
        Ok(())
    }

    fn refresh_sync(&mut self) {
        self.sync = SyncState::load(&self.ws);
        self.pending =
            if self.mode == Mode::Synced { sync::pending_changes(&self.ws, self.sync.as_ref()) } else { Vec::new() };
    }

    pub fn can_publish(&self) -> bool {
        self.mode == Mode::Synced
    }

    pub fn undo_label(&self) -> Option<&str> {
        self.undo.last().map(|(l, _)| l.as_str())
    }

    /// The tournament as it would be if this round finished with these results. Nothing is saved:
    /// the change runs on an in-memory copy, through the same rules as the export.
    pub fn preview_round(
        &self,
        tid: i64,
        number: i64,
        rows: &[crate::repo::ResultInput],
    ) -> Result<crate::rules::TournamentState> {
        let t = self.data.tournament(tid).ok_or_else(|| anyhow!("no tournament {tid}"))?.t.clone();
        let bytes = self.conn.serialize("main")?.to_vec();
        let mut mem = Connection::open_in_memory()?;
        mem.deserialize_read_exact("main", &bytes[..], bytes.len(), false)?;
        crate::db::configure(&mem)?;
        crate::repo::save_results(&mem, tid, number, rows, false)?;
        mem.execute(
            "UPDATE rounds SET completed = 1 WHERE tournament_id = ?1 AND number = ?2",
            rusqlite::params![tid, number],
        )?;
        let names = self.data.players.iter().map(|p| (p.id, p.name.clone())).collect();
        crate::rules::compute_tournament(&mem, &t, &names)
    }

    fn snapshot(&self) -> Option<Vec<u8>> {
        self.conn.serialize("main").ok().map(|d| d.to_vec())
    }

    /// Put the database file back exactly as it was before the last change.
    fn undo(&mut self) -> Result<Option<String>> {
        let Some((label, bytes)) = self.undo.pop() else { return Ok(None) };
        let old = std::mem::replace(&mut self.conn, Connection::open_in_memory()?);
        drop(old);
        std::fs::write(&self.ws.db, bytes)?;
        self.conn = self.ws.open()?;
        self.refresh()?;
        Ok(Some(label))
    }
}

enum Done {
    SignedIn {
        token: String,
        result: Result<Access>,
    },
    /// Device flow: the code to show (`seq` tells a cancelled attempt's replies apart).
    DeviceCode {
        seq: u64,
        result: Result<DeviceCode>,
    },
    /// Device flow: the user approved (the token) or it failed.
    DeviceToken {
        seq: u64,
        result: Result<String>,
    },
    Downloaded(Result<SyncState>),
    Published(Result<PublishOutcome>),
    Deployment {
        commit: String,
        result: Result<Option<Run>>,
    },
    Discord(Vec<(i64, String, Result<crate::discord::Profile>)>),
}

pub struct Deploy {
    pub commit: String,
    pub run: Option<Run>,
    pub checked: Instant,
    pub error: Option<String>,
}

/// A device-flow sign-in in progress.
pub struct DeviceSignIn {
    seq: u64,
    /// None until GitHub has handed out the code.
    pub code: Option<DeviceCode>,
    cancel: Arc<AtomicBool>,
}

pub struct App {
    pub config: Config,
    session: Option<Session>,
    /// A failure opening the database (shown instead of the app).
    open_error: Option<String>,
    page: Page,
    toasts: Vec<Toast>,
    confirm: Option<Confirm>,
    conflict: Option<(String, Vec<Change>)>,
    tx: Sender<Done>,
    rx: Receiver<Done>,
    /// What's running in the background, if anything.
    busy: Option<String>,
    pub deploy: Option<Deploy>,
    pub device: Option<DeviceSignIn>,
    device_seq: u64,
    folder: Option<PathBuf>,
    forms: Forms,
    autoshot: Option<autoshot::AutoShot>,
    glyphs_checked: Option<u64>,
    /// Scroll the page to this offset on the next frame.
    scroll_to: Option<f32>,
}

#[derive(Default)]
pub struct Forms {
    pub welcome: welcome::Form,
    pub players: players::Form,
    pub games: games::Form,
    pub tournaments: tournaments::Form,
    pub round: round::Form,
    pub library: library::Form,
    pub publish: publish::Form,
    pub settings: settings::Form,
}

fn practice_root() -> PathBuf {
    Workspace::managed_root().with_file_name("practice")
}

impl App {
    fn new(folder: Option<PathBuf>) -> Self {
        let (tx, rx) = channel();
        let mut app = Self {
            config: Config::load(),
            session: None,
            open_error: None,
            page: Page::Home,
            toasts: Vec::new(),
            confirm: None,
            conflict: None,
            tx,
            rx,
            busy: None,
            deploy: None,
            device: None,
            device_seq: 0,
            folder: folder.clone(),
            forms: Forms::default(),
            autoshot: autoshot::AutoShot::from_env(),
            glyphs_checked: None,
            scroll_to: None,
        };
        match folder {
            Some(dir) => app.open(Mode::Folder, Workspace::at(dir)),
            None => {
                let ws = Workspace::at(Workspace::managed_root());
                if ws.exists() {
                    app.open(Mode::Synced, ws);
                }
            }
        }
        app
    }

    fn open(&mut self, mode: Mode, ws: Workspace) {
        match Session::open(mode, ws) {
            Ok(s) => {
                self.session = Some(s);
                self.open_error = None;
            }
            Err(e) => {
                self.session = None;
                self.open_error = Some(format!("{e:#}"));
            }
        }
    }

    fn toast(&mut self, text: impl Into<String>, error: bool) {
        let secs = if error { 12 } else { 5 };
        self.toasts.push(Toast { text: text.into(), error, until: Instant::now() + Duration::from_secs(secs) });
        if self.toasts.len() > 4 {
            self.toasts.remove(0);
        }
    }

    fn github(&self) -> Option<GitHub> {
        self.config.github_token.as_ref().map(|t| GitHub::new(t.clone(), self.config.repo.clone()))
    }

    fn spawn(&mut self, what: &str, ctx: &egui::Context, job: impl FnOnce() -> Done + Send + 'static) {
        self.busy = Some(what.to_string());
        let (tx, ctx) = (self.tx.clone(), ctx.clone());
        std::thread::spawn(move || {
            let _ = tx.send(job());
            ctx.request_repaint();
        });
    }

    // ------------------------------------------------------------- actions

    fn apply(&mut self, action: Action, ctx: &egui::Context) {
        match action {
            Action::Db { label, f } => self.apply_db(label, f),
            Action::Go(page) => {
                if page != self.page {
                    self.scroll_to = Some(0.0);
                }
                self.page = page;
            }
            Action::Undo => self.undo(),
            Action::Confirm(c) => self.confirm = Some(c),
            Action::Toast(text, error) => self.toast(text, error),
            Action::OpenUrl(url) => ctx.open_url(egui::OpenUrl::new_tab(url)),
            Action::SaveConfig(config) => {
                self.config = config;
                if let Err(e) = self.config.save() {
                    self.toast(format!("Couldn't save the settings: {e:#}"), true);
                }
            }
            Action::SignIn(token) => self.sign_in(ctx, token),
            Action::DeviceSignIn => self.device_sign_in(ctx),
            Action::CancelDeviceSignIn => {
                if let Some(d) = self.device.take() {
                    d.cancel.store(true, Ordering::Relaxed);
                }
            }
            Action::SignOut => {
                self.config.github_token = None;
                self.config.github_login = None;
                let _ = self.config.save();
                self.toast("Signed out of GitHub.", false);
            }
            Action::Download => self.download(ctx),
            Action::Publish { message, overwrite } => self.publish(ctx, message, overwrite),
            Action::SetAvatar { slug, from } => self.set_avatar(ctx, &slug, &from),
            Action::RefreshDiscord(only) => {
                let Some(token) = self.config.discord_token.clone() else {
                    self.toast("Add a Discord bot token in Settings first.", true);
                    return;
                };
                let Some(s) = &self.session else { return };
                let players: Vec<(i64, String, String)> = s
                    .data
                    .players
                    .iter()
                    .filter_map(|p| Some((p.id, p.slug.clone(), p.discord_id.clone()?)))
                    .filter(|(_, _, id)| only.as_ref().is_none_or(|ids| ids.contains(id)))
                    .collect();
                if players.is_empty() {
                    return;
                }
                let what = if players.len() == 1 {
                    "Fetching the Discord profile…".to_string()
                } else {
                    format!("Fetching {} Discord profiles…", players.len())
                };
                self.spawn(&what, ctx, move || Done::Discord(crate::discord::fetch_all(&token, &players)));
            }
            Action::RemoveAvatar { slug } => {
                if let Some(s) = &self.session {
                    crate::export::remove_avatars(&s.data.avatars_dir, &slug);
                }
                self.after_file_change(ctx, "Picture removed.");
            }
            Action::EnterPractice { reset } => {
                let ws = Workspace::at(practice_root());
                if (reset || !ws.exists())
                    && let Err(e) = crate::seed::create_demo_db(&ws.db)
                {
                    self.toast(format!("Couldn't create the practice database: {e:#}"), true);
                    return;
                }
                self.open(Mode::Practice, ws);
                self.page = Page::Home;
                self.toast("Practice mode: nothing you do here reaches the website.", false);
            }
            Action::LeavePractice => {
                self.session = None;
                match &self.folder {
                    Some(dir) => self.open(Mode::Folder, Workspace::at(dir.clone())),
                    None => {
                        let ws = Workspace::at(Workspace::managed_root());
                        if ws.exists() {
                            self.open(Mode::Synced, ws);
                        }
                    }
                }
                self.page = Page::Home;
            }
            Action::StartEmpty => {
                let ws = Workspace::at(Workspace::managed_root());
                let created = crate::db::init(&ws.db, false)
                    .and_then(|()| crate::seed::seed_reference(&crate::db::connect(&ws.db)?));
                match created {
                    Ok(()) => {
                        self.open(Mode::Synced, ws);
                        self.page = Page::Home;
                    }
                    Err(e) => self.toast(format!("{e:#}"), true),
                }
            }
        }
    }

    fn apply_db(&mut self, label: String, f: DbFn) {
        let Some(s) = self.session.as_mut() else { return };
        let before = s.snapshot();
        match f(&s.conn) {
            Ok(DbOk { message, go }) => {
                if let Some(page) = go {
                    self.page = page;
                }
                if let Some(bytes) = before {
                    s.undo.push((label.clone(), bytes));
                    if s.undo.len() > UNDO_LIMIT {
                        s.undo.remove(0);
                    }
                }
                let refreshed = s.refresh();
                let msg = message.unwrap_or(label);
                match refreshed {
                    Ok(()) => self.toast(msg, false),
                    Err(e) => self.toast(format!("Saved, but exporting failed: {e:#}"), true),
                }
            }
            Err(e) => self.toast(friendly(&e), true),
        }
    }

    fn undo(&mut self) {
        let Some(s) = self.session.as_mut() else { return };
        match s.undo() {
            Ok(Some(label)) => self.toast(format!("Undone: {label}"), false),
            Ok(None) => self.toast("Nothing to undo.", false),
            Err(e) => self.toast(format!("Undo failed: {e:#}"), true),
        }
    }

    fn after_file_change(&mut self, ctx: &egui::Context, message: &str) {
        ctx.forget_all_images();
        if let Some(s) = self.session.as_mut() {
            match s.refresh() {
                Ok(()) => self.toast(message.to_string(), false),
                Err(e) => self.toast(format!("{e:#}"), true),
            }
        }
    }

    fn set_avatar(&mut self, ctx: &egui::Context, slug: &str, from: &std::path::Path) {
        let Some(s) = &self.session else { return };
        let ext = from.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
        if !crate::export::AVATAR_EXTENSIONS.contains(&format!(".{ext}").as_str()) {
            self.toast("Pictures must be .jpg, .png, .webp or .gif files.", true);
            return;
        }
        let dir = s.data.avatars_dir.clone();
        let result = (|| -> Result<()> {
            let bytes = std::fs::read(from)?;
            std::fs::create_dir_all(&dir)?;
            crate::export::remove_avatars(&dir, slug);
            std::fs::write(dir.join(format!("{slug}.{ext}")), bytes)?;
            Ok(())
        })();
        match result {
            Ok(()) => self.after_file_change(ctx, "Picture saved. It will be published with the data."),
            Err(e) => self.toast(format!("Couldn't save the picture: {e:#}"), true),
        }
    }

    fn download(&mut self, ctx: &egui::Context) {
        let Some(gh) = self.github() else {
            self.toast("Sign in to GitHub first (Settings).", true);
            return;
        };
        // Close the database: its file is about to be replaced.
        self.session = None;
        let ws = Workspace::at(Workspace::managed_root());
        self.spawn("Downloading the latest data from GitHub…", ctx, move || {
            Done::Downloaded(sync::download(&gh, &ws))
        });
    }

    fn publish(&mut self, ctx: &egui::Context, message: String, overwrite: bool) {
        let Some(gh) = self.github() else {
            self.toast("Sign in to GitHub first (Settings).", true);
            return;
        };
        let Some(s) = &self.session else { return };
        if !s.can_publish() {
            self.toast("Publishing is only possible for the synced data.", true);
            return;
        }
        if !s.report.ok() {
            self.toast("Fix the errors listed on the Publish page first.", true);
            return;
        }
        let Some(state) = s.sync.clone() else {
            self.toast("Download the data from GitHub once before publishing.", true);
            return;
        };
        let ws = s.ws.clone();
        self.spawn("Publishing to GitHub…", ctx, move || {
            Done::Published(sync::publish(&gh, &ws, &state, &message, overwrite))
        });
    }

    fn sign_in(&mut self, ctx: &egui::Context, token: String) {
        let gh = GitHub::new(token.clone(), self.config.repo.clone());
        self.spawn("Checking the GitHub sign-in…", ctx, move || Done::SignedIn { result: gh.access(), token });
    }

    /// Get a code from GitHub, then poll in the background until the user approves it in the browser.
    fn device_sign_in(&mut self, ctx: &egui::Context) {
        let Some(client_id) = crate::github::oauth_client_id() else {
            self.toast("Signing in through the browser isn't set up for this app; paste a token instead.", true);
            return;
        };
        if let Some(d) = self.device.take() {
            d.cancel.store(true, Ordering::Relaxed);
        }
        self.device_seq += 1;
        let seq = self.device_seq;
        let cancel = Arc::new(AtomicBool::new(false));
        self.device = Some(DeviceSignIn { seq, code: None, cancel: cancel.clone() });
        let (tx, ctx) = (self.tx.clone(), ctx.clone());
        std::thread::spawn(move || {
            let flow = DeviceFlow::new(client_id);
            let code = flow.start();
            let started = code.as_ref().ok().cloned();
            let _ = tx.send(Done::DeviceCode { seq, result: code });
            ctx.request_repaint();
            let Some(code) = started else { return };
            let result = flow.poll(&code, |interval| {
                let until = Instant::now() + interval;
                while Instant::now() < until {
                    if cancel.load(Ordering::Relaxed) {
                        return false;
                    }
                    std::thread::sleep(Duration::from_millis(200));
                }
                !cancel.load(Ordering::Relaxed)
            });
            let _ = tx.send(Done::DeviceToken { seq, result });
            ctx.request_repaint();
        });
    }

    fn check_deployment(&mut self, ctx: &egui::Context, commit: String) {
        let Some(gh) = self.github() else { return };
        let (tx, ctx) = (self.tx.clone(), ctx.clone());
        std::thread::spawn(move || {
            let result = gh.deployment(&commit);
            let _ = tx.send(Done::Deployment { commit, result });
            ctx.request_repaint();
        });
    }

    fn finished(&mut self, done: Done, ctx: &egui::Context) {
        // The device flow runs alongside other work, so its replies don't end `busy`.
        if !matches!(done, Done::DeviceCode { .. } | Done::DeviceToken { .. }) {
            self.busy = None;
        }
        match done {
            Done::Discord(fetched) => {
                let Some(dir) = self.session.as_ref().map(|s| s.data.avatars_dir.clone()) else { return };
                let failed: Vec<String> = fetched
                    .iter()
                    .filter_map(|(_, slug, r)| r.as_ref().err().map(|e| format!("{slug}: {e:#}")))
                    .collect();
                let ok = fetched.len() - failed.len();
                if ok > 0 {
                    let label = format!("Discord profiles updated ({ok})");
                    ctx.forget_all_images();
                    self.apply(Action::db(label, move |c| crate::discord::apply(c, &dir, &fetched).map(|_| ())), ctx);
                }
                if !failed.is_empty() {
                    self.toast(format!("Couldn't fetch from Discord — {}", failed.join("; ")), true);
                }
            }
            Done::DeviceCode { seq, result } => {
                let Some(d) = self.device.as_mut().filter(|d| d.seq == seq) else { return };
                match result {
                    Ok(code) => d.code = Some(code),
                    Err(e) => {
                        self.device = None;
                        self.toast(format!("{e:#}"), true);
                    }
                }
            }
            Done::DeviceToken { seq, result } => {
                if self.device.as_ref().is_none_or(|d| d.seq != seq) {
                    return;
                }
                self.device = None;
                match result {
                    Ok(token) => self.sign_in(ctx, token),
                    Err(e) => self.toast(format!("{e:#}"), true),
                }
            }
            Done::SignedIn { token, result } => {
                match result {
                    Ok(access) if !access.can_push => {
                        self.toast(
                        format!("Signed in as {}, but this token can't publish to {}. Give it Contents: read and write.", access.login, self.config.repo.full_name()),
                        true,
                    );
                    }
                    Ok(access) => {
                        self.config.github_token = Some(token);
                        self.config.github_login = Some(access.login.clone());
                        if let Err(e) = self.config.save() {
                            self.toast(format!("Couldn't save the sign-in: {e:#}"), true);
                        }
                        self.forms.settings.token.clear();
                        self.forms.welcome.token.clear();
                        self.toast(format!("Signed in to GitHub as {}.", access.login), false);
                        if self.session.is_none() {
                            self.download(ctx);
                        }
                    }
                    Err(e) => self.toast(format!("{e:#}"), true),
                }
            }
            Done::Downloaded(result) => {
                let ws = Workspace::at(Workspace::managed_root());
                match result {
                    Ok(state) => {
                        self.toast(
                            format!(
                                "Downloaded the latest data (commit {}).",
                                &state.commit[..7.min(state.commit.len())]
                            ),
                            false,
                        );
                    }
                    Err(e) => self.toast(format!("Download failed: {e:#}"), true),
                }
                if ws.exists() {
                    self.open(Mode::Synced, ws);
                }
            }
            Done::Published(result) => {
                if let Some(s) = self.session.as_mut() {
                    s.refresh_sync();
                }
                match result {
                    Ok(PublishOutcome::Published { commit, files }) => {
                        self.toast(format!("Sent {files} file(s) to GitHub. The website is updating now."), false);
                        self.forms.publish.message.clear();
                        self.deploy =
                            Some(Deploy { commit: commit.clone(), run: None, checked: Instant::now(), error: None });
                        self.check_deployment(ctx, commit);
                    }
                    Ok(PublishOutcome::NothingToPublish) => {
                        self.toast("Nothing to publish: GitHub already has this data.", false)
                    }
                    Ok(PublishOutcome::Conflict { theirs }) => {
                        self.conflict = Some((self.forms.publish.message.clone(), theirs));
                    }
                    Err(e) => self.toast(format!("Publishing failed; your changes are still saved here. {e:#}"), true),
                }
            }
            Done::Deployment { commit, result } => {
                if let Some(d) = self.deploy.as_mut().filter(|d| d.commit == commit) {
                    d.checked = Instant::now();
                    match result {
                        Ok(run) => {
                            d.run = run;
                            d.error = None;
                        }
                        Err(e) => d.error = Some(format!("{e:#}")),
                    }
                }
            }
        }
    }

    /// Keep checking the website update until GitHub Actions has finished.
    fn poll_deployment(&mut self, ctx: &egui::Context) {
        let Some(d) = &self.deploy else { return };
        let finished = d.run.as_ref().is_some_and(|r| matches!(r.state, RunState::Success | RunState::Failed));
        if finished || d.error.is_some() {
            return;
        }
        if d.checked.elapsed() > Duration::from_secs(8) {
            let commit = d.commit.clone();
            if let Some(d) = self.deploy.as_mut() {
                d.checked = Instant::now();
            }
            self.check_deployment(ctx, commit);
        }
        ctx.request_repaint_after(Duration::from_secs(2));
    }

    // ------------------------------------------------------------------ ui

    fn sidebar(&mut self, ui: &mut egui::Ui, actions: &mut Vec<Action>) {
        let Some(s) = &self.session else { return };
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new(" UG ").background_color(theme::GOLD).color(theme::ON_GOLD).strong().size(16.0));
            ui.label(RichText::new("ADMIN").strong().size(16.0));
        });
        ui.add_space(14.0);
        let items: [(Page, &str); 8] = [
            (Page::Home, "🏠  Dashboard"),
            (Page::Tournaments, "🏆  Tournaments"),
            (Page::Players, "👤  Players"),
            (Page::Games, "🎮  Games"),
            (Page::Library, "📚  Categories & awards"),
            (Page::Publish, "🚀  Publish"),
            (Page::Settings, "⚙  Settings"),
            (Page::Home, ""),
        ];
        for (page, label) in items.iter().take(7) {
            let active = match (self.page, page) {
                (Page::Tournament(..) | Page::Round(..), Page::Tournaments) => true,
                (a, b) => a == *b,
            };
            let resp = theme::nav_item(ui, active, label);
            if *page == Page::Publish && !s.pending.is_empty() {
                let r = resp.rect;
                ui.painter().circle_filled(egui::pos2(r.right() - 14.0, r.center().y), 4.5, theme::GOLD);
            }
            if resp.clicked() {
                actions.push(Action::Go(*page));
            }
        }

        ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
            ui.add_space(10.0);
            ui.hyperlink_to(RichText::new("View website").small(), &s.data.site_url);
            let (label, color) = match s.mode {
                Mode::Synced => (
                    s.sync.as_ref().map_or("Not synced yet".to_string(), |st| format!("Synced with {}", st.repo)),
                    theme::SUBTLE,
                ),
                Mode::Practice => ("Practice database".into(), theme::CHALLENGER),
                Mode::Folder => (
                    format!(
                        "Folder: {}",
                        s.ws.root
                            .file_name()
                            .map_or_else(|| s.ws.root.display().to_string(), |n| n.to_string_lossy().into_owned())
                    ),
                    theme::INFO,
                ),
            };
            ui.label(RichText::new(label).small().color(color));
            if s.data.demo_present {
                theme::badge(ui, "Demo data on", theme::CHALLENGER);
            }
        });
    }

    fn top_bar(&mut self, ui: &mut egui::Ui, actions: &mut Vec<Action>) {
        let Some(s) = &self.session else { return };
        ui.horizontal(|ui| {
            let undo = s.undo_label().map(str::to_string);
            let resp = ui.add_enabled(undo.is_some(), egui::Button::new("↺ Undo"));
            let resp = match &undo {
                Some(label) => resp.on_hover_text(format!("Undo: {label}  (Ctrl+Z)")),
                None => resp.on_disabled_hover_text("Nothing to undo yet"),
            };
            if resp.clicked() {
                actions.push(Action::Undo);
            }

            let errors = s.report.errors.len();
            let warnings = s.report.warnings.len();
            if errors > 0 {
                if ui
                    .add(
                        egui::Button::new(RichText::new(format!("✖ {errors} error(s)")).color(theme::DANGER))
                            .frame(false),
                    )
                    .clicked()
                {
                    actions.push(Action::Go(Page::Publish));
                }
            } else if warnings > 0 {
                if ui
                    .add(
                        egui::Button::new(RichText::new(format!("⚠ {warnings} warning(s)")).color(theme::GOLD))
                            .frame(false),
                    )
                    .clicked()
                {
                    actions.push(Action::Go(Page::Publish));
                }
            } else {
                ui.label(RichText::new("✔ Data checks pass").color(theme::LIVE).small());
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| match s.mode {
                Mode::Synced if !s.pending.is_empty() => {
                    if theme::primary(ui, "Publish ➡").clicked() {
                        actions.push(Action::Go(Page::Publish));
                    }
                    ui.label(RichText::new("Saved on this computer — not on the website yet.").color(theme::GOLD));
                }
                Mode::Synced => {
                    ui.label(RichText::new("✔ Website is up to date").color(theme::SUBTLE).small());
                }
                Mode::Practice => {
                    if ui.button("Leave practice").clicked() {
                        actions.push(Action::LeavePractice);
                    }
                    theme::badge(ui, "PRACTICE — nothing is published", theme::CHALLENGER);
                }
                Mode::Folder => {
                    ui.label(
                        RichText::new("Folder mode: commit and push with git to publish").color(theme::INFO).small(),
                    );
                }
            });
        });
    }

    fn toasts(&mut self, ctx: &egui::Context) {
        self.toasts.retain(|t| t.until > Instant::now());
        if self.toasts.is_empty() {
            return;
        }
        ctx.request_repaint_after(Duration::from_millis(500));
        egui::Area::new(egui::Id::new("toasts")).anchor(egui::Align2::RIGHT_BOTTOM, [-16.0, -16.0]).show(ctx, |ui| {
            ui.set_max_width(440.0);
            for t in &self.toasts {
                let color = if t.error { theme::DANGER } else { theme::LIVE };
                egui::Frame::new()
                    .fill(theme::SURFACE_2)
                    .stroke(egui::Stroke::new(1.0_f32, color))
                    .corner_radius(8)
                    .inner_margin(12)
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new(format!("{} {}", if t.error { "⛔" } else { "✔" }, t.text))
                                .color(theme::TEXT),
                        );
                    });
            }
        });
    }

    fn modals(&mut self, ctx: &egui::Context, actions: &mut Vec<Action>) {
        if let Some(c) = &self.confirm {
            let mut close = false;
            let mut go = false;
            egui::Modal::new(egui::Id::new("confirm")).show(ctx, |ui| {
                ui.set_max_width(460.0);
                theme::section_title(ui, &c.title);
                ui.label(&c.body);
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    let yes = if c.danger { theme::danger(ui, &c.yes) } else { theme::primary(ui, &c.yes) };
                    if yes.clicked() {
                        go = true;
                    }
                    if ui.button("Cancel").clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                        close = true;
                    }
                });
            });
            if go {
                if let Some(c) = self.confirm.take() {
                    actions.push(*c.action);
                }
            } else if close {
                self.confirm = None;
            }
        }

        if let Some((message, theirs)) = &self.conflict {
            let mut choice = None;
            egui::Modal::new(egui::Id::new("conflict")).show(ctx, |ui| {
                ui.set_max_width(560.0);
                theme::section_title(ui, "Someone else published in the meantime");
                ui.label("GitHub has data changes that this computer hasn't downloaded. The database can't be merged automatically, so one version has to win:");
                egui::ScrollArea::vertical().max_height(180.0).show(ui, |ui| {
                    for c in theirs.iter().filter(|c| c.label.as_deref().is_none_or(|l| !l.starts_with("Link preview"))) {
                        ui.label(theme::muted(format!("• {}", c.label.clone().unwrap_or_else(|| "Database".into()))));
                    }
                });
                ui.add_space(6.0);
                ui.horizontal_wrapped(|ui| {
                    if ui.button("Download theirs (discard mine)").clicked() {
                        choice = Some(0);
                    }
                    if theme::danger(ui, "Publish mine anyway (replace theirs)").clicked() {
                        choice = Some(1);
                    }
                    if ui.button("Cancel").clicked() {
                        choice = Some(2);
                    }
                });
            });
            match choice {
                Some(0) => {
                    self.conflict = None;
                    actions.push(Action::Download);
                }
                Some(1) => {
                    let message = message.clone();
                    self.conflict = None;
                    actions.push(Action::Publish { message, overwrite: true });
                }
                Some(_) => self.conflict = None,
                None => {}
            }
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        while let Ok(done) = self.rx.try_recv() {
            self.finished(done, ctx);
        }
        self.poll_deployment(ctx);
        if let Some(s) = &self.session
            && self.glyphs_checked != Some(s.data.rev)
        {
            self.glyphs_checked = Some(s.data.rev);
            let icons = s
                .data
                .categories
                .iter()
                .filter_map(|c| c.icon.clone())
                .chain(s.data.awards.iter().filter_map(|a| a.icon.clone()));
            theme::check_glyphs(ctx, icons);
        }
        let mut actions = Vec::new();
        if let Some(shot) = self.autoshot.as_mut() {
            shot.plan(self.session.as_ref().map(|s| &s.data));
            match shot.step(ctx) {
                Some((page, offset)) => {
                    self.page = page;
                    self.scroll_to = Some(offset);
                }
                None => self.autoshot = None,
            }
        }

        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::Z)) && self.busy.is_none() {
            actions.push(Action::Undo);
        }

        if let Some(what) = &self.busy {
            egui::TopBottomPanel::bottom("busy").show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(what);
                });
            });
        }

        if self.session.is_none() {
            egui::CentralPanel::default().show(ctx, |ui| {
                welcome::show(ui, self, &mut actions);
            });
        } else {
            egui::SidePanel::left("nav")
                .resizable(false)
                .exact_width(220.0)
                .show(ctx, |ui| self.sidebar(ui, &mut actions));
            egui::TopBottomPanel::top("top")
                .frame(egui::Frame::new().fill(theme::BG_RAISED).inner_margin(egui::Margin::symmetric(16, 10)))
                .show(ctx, |ui| self.top_bar(ui, &mut actions));
            egui::CentralPanel::default()
                .frame(egui::Frame::new().fill(theme::BG).inner_margin(egui::Margin::symmetric(24, 16)))
                .show(ctx, |ui| {
                    ui.add_enabled_ui(self.busy.is_none(), |ui| {
                        let mut area = egui::ScrollArea::vertical().auto_shrink([false, false]);
                        if let Some(offset) = self.scroll_to.take() {
                            area = area.vertical_scroll_offset(offset);
                        }
                        area.show(ui, |ui| {
                            ui.set_max_width(ui.available_width().min(1180.0));
                            self.page_ui(ui, ctx, &mut actions);
                        });
                    });
                });
        }

        self.modals(ctx, &mut actions);
        self.toasts(ctx);
        for action in actions {
            self.apply(action, ctx);
        }
    }
}

impl App {
    fn page_ui(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, actions: &mut Vec<Action>) {
        let Some(s) = &self.session else { return };
        let page = self.page;
        match page {
            Page::Home => home::show(ui, s, actions),
            Page::Tournaments => tournaments::list(ui, s, &mut self.forms.tournaments, actions),
            Page::Tournament(id, tab) => match s.data.tournament(id) {
                Some(t) => tournaments::detail(ui, s, t, tab, &mut self.forms.tournaments, actions),
                None => {
                    actions.push(Action::Go(Page::Tournaments));
                }
            },
            Page::Round(tid, number) => match s.data.tournament(tid) {
                Some(t) if t.category_at(number).is_some() => {
                    round::show(ui, s, t, number, &mut self.forms.round, actions)
                }
                _ => actions.push(Action::Go(Page::Tournaments)),
            },
            Page::Players => {
                let discord = self.config.discord_token.is_some();
                players::show(ui, ctx, s, discord, &mut self.forms.players, actions)
            }
            Page::Games => games::show(ui, s, &mut self.forms.games, actions),
            Page::Library => library::show(ui, s, &mut self.forms.library, actions),
            Page::Publish => publish::show(ui, s, &self.config, self.deploy.as_ref(), &mut self.forms.publish, actions),
            Page::Settings => {
                settings::show(ui, s, &self.config, self.device.as_ref(), &mut self.forms.settings, actions)
            }
        }
    }

    pub fn open_error(&self) -> Option<&str> {
        self.open_error.as_deref()
    }

    pub fn busy(&self) -> bool {
        self.busy.is_some()
    }
}

pub fn color_for_rank(rank: i64) -> Color32 {
    match rank {
        1 => theme::GOLD,
        2 => Color32::from_rgb(0xd0, 0xd4, 0xdc),
        3 => Color32::from_rgb(0xd8, 0x8f, 0x4c),
        _ => theme::MUTED,
    }
}

/// The window icon: the gold "UG" badge in assets/icon.png.
fn app_icon() -> egui::IconData {
    let image = image::load_from_memory(desktop::ICON_PNG).expect("bundled icon").into_rgba8();
    let (width, height) = image.dimensions();
    egui::IconData { rgba: image.into_raw(), width, height }
}
