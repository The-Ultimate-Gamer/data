//! Command-line interface. Without a subcommand the desktop window opens.

use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::{Result, bail};
use clap::{Args, Parser, Subcommand};

use crate::config::Config;
use crate::github::GitHub;
use crate::sync::{self, PublishOutcome, SyncState};
use crate::workspace::Workspace;
use crate::{db, seed};

#[derive(Parser)]
#[command(name = "ug-admin", version, about = "Admin tool for The Ultimate Gamer tournament data")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,

    /// Open the desktop app on this folder (a data repository checkout) instead of its own synced workspace.
    /// Publishing is then left to git.
    #[arg(long, global = false)]
    pub repo: Option<PathBuf>,
}

#[derive(Args, Clone)]
pub struct Paths {
    /// Data repository folder (default: the current folder).
    #[arg(long, default_value = ".")]
    pub repo: PathBuf,
    /// Database file (default: <repo>/database/ultimate-gamer.sqlite).
    #[arg(long)]
    pub db: Option<PathBuf>,
    /// Output folder for the JSON (default: <repo>/data).
    #[arg(long)]
    pub out: Option<PathBuf>,
}

impl Paths {
    fn workspace(&self) -> Workspace {
        Workspace::custom(&self.repo, self.db.clone(), self.out.clone())
    }
}

#[derive(Args, Clone)]
pub struct SyncArgs {
    /// The synced folder (default: the desktop app's own).
    #[arg(long)]
    pub workspace: Option<PathBuf>,
}

impl SyncArgs {
    fn workspace(&self) -> Workspace {
        Workspace::at(self.workspace.clone().unwrap_or_else(Workspace::managed_root))
    }
}

/// The GitHub client from the saved sign-in, or `UG_GITHUB_TOKEN`.
fn github() -> Result<GitHub> {
    let config = Config::load();
    let token = std::env::var("UG_GITHUB_TOKEN").ok().or(config.github_token);
    match token {
        Some(token) => Ok(GitHub::new(token, config.repo)),
        None => bail!("not signed in to GitHub: sign in from the app's Settings, or set UG_GITHUB_TOKEN"),
    }
}

#[derive(Subcommand)]
pub enum Command {
    /// Regenerate the JSON documents, pictures and share pages from the database.
    Export {
        #[command(flatten)]
        paths: Paths,
    },
    /// Check the database, and that the JSON matches a fresh export. Exits 1 on errors.
    Validate {
        #[command(flatten)]
        paths: Paths,
        /// Only check the database.
        #[arg(long)]
        skip_json: bool,
    },
    /// Create an empty database from the schema, with reference data.
    Init {
        #[command(flatten)]
        paths: Paths,
        /// Delete and recreate an existing database.
        #[arg(long)]
        force: bool,
    },
    /// Bring an existing database up to date with the schema.
    Migrate {
        #[command(flatten)]
        paths: Paths,
    },
    /// Replace the app's data with the latest version from GitHub.
    Download {
        #[command(flatten)]
        sync: SyncArgs,
    },
    /// Show what publishing would change.
    Status {
        #[command(flatten)]
        sync: SyncArgs,
    },
    /// Export, validate, then send the changes to GitHub as one commit.
    Publish {
        #[command(flatten)]
        sync: SyncArgs,
        /// Commit message (default: a summary of what changed).
        #[arg(short, long)]
        message: Option<String>,
        /// Publish even if someone else published data in the meantime (this version wins).
        #[arg(long)]
        overwrite: bool,
    },
    /// Refresh every linked player's Discord name and picture (or one with --player).
    /// Needs a Discord bot token: the app's Settings, or UG_DISCORD_TOKEN.
    Discord {
        #[command(flatten)]
        paths: Paths,
        /// Only this player (slug).
        #[arg(long)]
        player: Option<String>,
    },
    /// Load reference data (categories and awards), or manage the flagged demo data.
    Seed {
        #[command(flatten)]
        paths: Paths,
        /// Add fictional demo data, flagged so it can be removed again.
        #[arg(long, conflicts_with_all = ["remove_demo", "demo"])]
        add_demo: bool,
        /// Remove every flagged demo row.
        #[arg(long, conflicts_with = "demo")]
        remove_demo: bool,
        /// Recreate --db (never the real database) with only demo data.
        #[arg(long)]
        demo: bool,
    },
}

pub fn run(command: Command) -> Result<ExitCode> {
    match command {
        Command::Export { paths } => {
            let ws = paths.workspace();
            let conn = open_existing(&ws)?;
            let report = ws.export(&conn)?;
            println!(
                "Exported to {}: {} written, {} unchanged, {} removed",
                ws.out.display(),
                report.written.len(),
                report.unchanged.len(),
                report.removed.len()
            );
            for rel in &report.written {
                println!("  + {rel}");
            }
            for rel in &report.removed {
                println!("  - {rel}");
            }
        }
        Command::Validate { paths, skip_json } => {
            let ws = paths.workspace();
            let conn = open_existing(&ws)?;
            let report = ws.validate(&conn, skip_json)?;
            for w in &report.warnings {
                println!("warning: {w}");
            }
            for e in &report.errors {
                println!("error: {e}");
            }
            println!("{} error(s), {} warning(s)", report.errors.len(), report.warnings.len());
            if !report.ok() {
                return Ok(ExitCode::FAILURE);
            }
        }
        Command::Init { paths, force } => {
            let ws = paths.workspace();
            db::init(&ws.db, force)?;
            seed::seed_reference(&db::connect(&ws.db)?)?;
            println!("Created {}", ws.db.display());
        }
        Command::Migrate { paths } => {
            let ws = paths.workspace();
            open_existing(&ws)?;
            println!("Migrated {}", ws.db.display());
        }
        Command::Download { sync: args } => {
            let ws = args.workspace();
            let state = sync::download(&github()?, &ws)?;
            println!(
                "Downloaded {} ({}) at {} into {}",
                state.repo,
                state.branch,
                &state.commit[..7],
                ws.root.display()
            );
        }
        Command::Status { sync: args } => {
            let ws = args.workspace();
            let state = SyncState::load(&ws);
            match &state {
                Some(s) => {
                    println!("Synced with {} ({}) at commit {}", s.repo, s.branch, &s.commit[..7.min(s.commit.len())])
                }
                None => println!("Never downloaded from GitHub"),
            }
            let changes = sync::pending_changes(&ws, state.as_ref());
            if changes.is_empty() {
                println!("Nothing to publish.");
            }
            for c in changes {
                println!("  {:?} {}{}", c.kind, c.path, c.label.map(|l| format!("  ({l})")).unwrap_or_default());
            }
        }
        Command::Publish { sync: args, message, overwrite } => {
            let ws = args.workspace();
            let Some(state) = SyncState::load(&ws) else { bail!("download the data first (`ug-admin download`)") };
            let conn = open_existing(&ws)?;
            ws.export(&conn)?;
            let report = ws.validate(&conn, false)?;
            if !report.ok() {
                for e in &report.errors {
                    println!("error: {e}");
                }
                bail!("fix the errors above before publishing");
            }
            let message = message.unwrap_or_else(|| sync::summary(&sync::pending_changes(&ws, Some(&state))));
            match sync::publish(&github()?, &ws, &state, &message, overwrite)? {
                PublishOutcome::NothingToPublish => println!("Nothing to publish."),
                PublishOutcome::Published { commit, files } => {
                    println!("Published {files} file(s) as {} \"{message}\"", &commit[..7])
                }
                PublishOutcome::Conflict { theirs } => {
                    println!("Someone else published since you downloaded:");
                    for c in theirs {
                        println!("  {:?} {}", c.kind, c.path);
                    }
                    bail!("download their version, or publish again with --overwrite to replace it");
                }
            }
        }
        Command::Discord { paths, player } => {
            let token = std::env::var("UG_DISCORD_TOKEN").ok().or(Config::load().discord_token);
            let Some(token) = token else {
                bail!("no Discord bot token: add one in the app's Settings, or set UG_DISCORD_TOKEN")
            };
            let ws = paths.workspace();
            let conn = open_existing(&ws)?;
            let linked: Vec<(i64, String, String)> = crate::model::players(&conn)?
                .into_iter()
                .filter(|p| player.as_ref().is_none_or(|s| *s == p.slug))
                .filter_map(|p| Some((p.id, p.slug, p.discord_id?)))
                .collect();
            if linked.is_empty() {
                bail!("no matching player has a Discord ID");
            }
            let fetched = crate::discord::fetch_all(&token, &linked);
            for line in crate::discord::apply(&conn, &ws.avatars, &fetched)? {
                println!("{line}");
            }
            ws.export(&conn)?;
            if fetched.iter().any(|(_, _, r)| r.is_err()) {
                return Ok(ExitCode::FAILURE);
            }
        }
        Command::Seed { paths, add_demo, remove_demo, demo } => {
            let ws = paths.workspace();
            if demo {
                if ws.exists() && seed::has_real_data(&db::connect(&ws.db)?)? {
                    bail!("--demo recreates the database, which holds real data; use --add-demo instead.");
                }
                seed::create_demo_db(&ws.db)?;
                println!("Created {} with demo data", ws.db.display());
                return Ok(ExitCode::SUCCESS);
            }
            let conn = open_existing(&ws)?;
            seed::seed_reference(&conn)?;
            if add_demo {
                seed::add_demo(&conn)?;
                println!("Added demo data to {}", ws.db.display());
            } else if remove_demo {
                let r = seed::remove_demo(&conn)?;
                println!(
                    "Removed demo data from {}: {} tournaments, {} players, {} games",
                    ws.db.display(),
                    r.tournaments,
                    r.players,
                    r.games
                );
            } else {
                println!("Seeded reference data into {}", ws.db.display());
            }
        }
    }
    Ok(ExitCode::SUCCESS)
}

fn open_existing(ws: &Workspace) -> Result<rusqlite::Connection> {
    if !ws.exists() {
        bail!("{} does not exist (create it with `ug-admin init`)", ws.db.display());
    }
    ws.open()
}
