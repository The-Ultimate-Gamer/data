//! A workspace is a folder laid out like the data repository:
//!
//! ```text
//! database/ultimate-gamer.sqlite   the facts (authoritative)
//! avatars/                         player pictures
//! data/                            generated JSON (never edit by hand)
//! ```
//!
//! The desktop app keeps its own workspace in the user's data folder and syncs it with GitHub.
//! The CLI works on any folder, usually a checkout of the data repository.

use std::path::{Path, PathBuf};

use anyhow::Result;
use rusqlite::Connection;

use crate::{db, export, validate};

pub const DB_FILE: &str = "database/ultimate-gamer.sqlite";
pub const DATA_DIR: &str = "data";
pub const AVATARS_DIR: &str = "avatars";
/// The folders publishing sends to GitHub. Nothing else in the repository is ever touched.
pub const PUBLISHED_DIRS: &[&str] = &["database", "data", "avatars"];

#[derive(Clone, Debug)]
pub struct Workspace {
    pub root: PathBuf,
    pub db: PathBuf,
    pub out: PathBuf,
    pub avatars: PathBuf,
}

impl Workspace {
    pub fn at(root: impl Into<PathBuf>) -> Self {
        let root = root.into();
        Self { db: root.join(DB_FILE), out: root.join(DATA_DIR), avatars: root.join(AVATARS_DIR), root }
    }

    /// With explicit database and output paths (the CLI's `--db` and `--out`).
    pub fn custom(root: impl Into<PathBuf>, db: Option<PathBuf>, out: Option<PathBuf>) -> Self {
        let mut ws = Self::at(root);
        if let Some(db) = db {
            ws.db = db;
        }
        if let Some(out) = out {
            ws.out = out;
        }
        ws
    }

    /// The folder the desktop app manages (synced with GitHub).
    pub fn managed_root() -> PathBuf {
        directories::ProjectDirs::from("io.github", "The Ultimate Gamer", "ug-admin")
            .map_or_else(|| PathBuf::from("ug-admin-workspace"), |d| d.data_dir().join("workspace"))
    }

    pub fn exists(&self) -> bool {
        self.db.is_file()
    }

    /// Open the database, bringing older ones up to date with the schema first.
    pub fn open(&self) -> Result<Connection> {
        let conn = db::connect(&self.db)?;
        db::migrate(&conn)?;
        Ok(conn)
    }

    pub fn export(&self, conn: &Connection) -> Result<export::ExportReport> {
        export::export(conn, &self.out, Some(&self.avatars), None)
    }

    pub fn validate(&self, conn: &Connection, skip_json: bool) -> Result<validate::Report> {
        validate::validate(conn, &self.out, Some(&self.avatars), skip_json)
    }

    pub fn relative<'a>(&self, path: &'a Path) -> &'a Path {
        path.strip_prefix(&self.root).unwrap_or(path)
    }
}
