//! Database access: opening, creating and migrating the SQLite file.

use std::path::Path;

use anyhow::{Context, Result, bail};
use rusqlite::Connection;

/// The table definitions, compiled into the binary so it can create a database anywhere.
pub const SCHEMA_SQL: &str = include_str!("../../schema/schema.sql");

/// Version of the JSON contract. Bump only for breaking changes, after the website supports it.
pub const SCHEMA_VERSION: i64 = 1;

/// Columns added after the first release: (table, column, definition).
const ADDED_COLUMNS: &[(&str, &str, &str)] = &[
    ("players", "is_demo", "INTEGER NOT NULL DEFAULT 0 CHECK (is_demo IN (0, 1))"),
    ("games", "is_demo", "INTEGER NOT NULL DEFAULT 0 CHECK (is_demo IN (0, 1))"),
    ("tournaments", "is_demo", "INTEGER NOT NULL DEFAULT 0 CHECK (is_demo IN (0, 1))"),
    (
        "players",
        "discord_id",
        "TEXT CHECK (discord_id IS NULL OR (length(discord_id) BETWEEN 15 AND 21 AND discord_id NOT GLOB '*[^0-9]*'))",
    ),
    ("players", "discord_username", "TEXT"),
    ("players", "discord_display_name", "TEXT"),
];

pub fn connect(path: &Path) -> Result<Connection> {
    let conn = Connection::open(path).with_context(|| format!("opening {}", path.display()))?;
    configure(&conn)?;
    Ok(conn)
}

pub fn configure(conn: &Connection) -> Result<()> {
    conn.execute_batch("PRAGMA foreign_keys = ON; PRAGMA busy_timeout = 3000;")?;
    Ok(())
}

pub fn create_schema(conn: &Connection) -> Result<()> {
    conn.execute_batch(SCHEMA_SQL)?;
    Ok(())
}

/// An in-memory database with the schema and reference data (tests).
#[cfg(test)]
pub fn memory() -> Result<Connection> {
    let conn = Connection::open_in_memory()?;
    configure(&conn)?;
    create_schema(&conn)?;
    crate::seed::seed_reference(&conn)?;
    Ok(conn)
}

/// Create an empty database from the schema. Refuses to overwrite unless `force`.
pub fn init(path: &Path, force: bool) -> Result<()> {
    if path.exists() {
        if !force {
            bail!("{} already exists (use --force to recreate it)", path.display());
        }
        std::fs::remove_file(path)?;
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let conn = connect(path)?;
    create_schema(&conn)?;
    conn.execute(
        "INSERT OR REPLACE INTO meta (key, value) VALUES ('schema_version', ?1)",
        [SCHEMA_VERSION.to_string()],
    )?;
    Ok(())
}

/// Tables of earlier versions whose facts are now derived (awards) or dropped (records).
const DROPPED_TABLES: &[&str] = &["award_grants", "tournament_records"];

/// Bring an existing database up to date: add new columns, then new tables and indexes.
pub fn migrate(conn: &Connection) -> Result<()> {
    let tx = conn.unchecked_transaction()?;
    for table in DROPPED_TABLES {
        tx.execute_batch(&format!("DROP TABLE IF EXISTS {table}"))?;
    }
    for (table, column, definition) in ADDED_COLUMNS {
        let mut stmt = tx.prepare(&format!("PRAGMA table_info({table})"))?;
        let existing: Vec<String> = stmt.query_map([], |r| r.get::<_, String>("name"))?.collect::<Result<_, _>>()?;
        if !existing.is_empty() && !existing.iter().any(|c| c == column) {
            tx.execute_batch(&format!("ALTER TABLE {table} ADD COLUMN {column} {definition}"))?;
        }
    }
    tx.execute_batch(SCHEMA_SQL)?;
    tx.commit()?;
    Ok(())
}

/// A change that can nest inside another one (SQLite savepoints): rolled back when dropped
/// without `commit`. Derefs to the connection.
pub struct Tx<'a> {
    conn: &'a Connection,
    name: String,
    done: bool,
}

pub fn begin(conn: &Connection) -> Result<Tx<'_>> {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let name = format!("sp{}", NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed));
    conn.execute_batch(&format!("SAVEPOINT {name}"))?;
    Ok(Tx { conn, name, done: false })
}

impl Tx<'_> {
    pub fn commit(mut self) -> Result<()> {
        self.conn.execute_batch(&format!("RELEASE {}", self.name))?;
        self.done = true;
        Ok(())
    }
}

impl std::ops::Deref for Tx<'_> {
    type Target = Connection;
    fn deref(&self) -> &Connection {
        self.conn
    }
}

impl Drop for Tx<'_> {
    fn drop(&mut self) {
        if !self.done {
            let _ = self.conn.execute_batch(&format!("ROLLBACK TO {0}; RELEASE {0}", self.name));
        }
    }
}

pub fn utc_now() -> String {
    let now = time::OffsetDateTime::now_utc();
    now.format(time::macros::format_description!("[year]-[month]-[day]T[hour]:[minute]:[second]Z"))
        .expect("valid format")
}

pub fn today() -> String {
    utc_now()[..10].to_string()
}

/// Lowercase kebab-case: `abc`, `abc-123`. Slugs become URLs.
pub fn is_slug(slug: &str) -> bool {
    !slug.is_empty()
        && slug
            .split('-')
            .all(|part| !part.is_empty() && part.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit()))
}

/// Turn a display name into a slug suggestion: "Mario Kart 8!" -> "mario-kart-8".
pub fn slugify(text: &str) -> String {
    let mut out = String::new();
    let mut dash = false;
    for ch in text.chars().flat_map(fold_accent) {
        if ch.is_ascii_alphanumeric() {
            if dash && !out.is_empty() {
                out.push('-');
            }
            dash = false;
            out.push(ch.to_ascii_lowercase());
        } else {
            dash = true;
        }
    }
    out
}

/// Strip common Latin accents so "Pokémon" becomes "pokemon".
fn fold_accent(ch: char) -> Vec<char> {
    let plain = match ch {
        'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' | 'À' | 'Á' | 'Â' | 'Ã' | 'Ä' | 'Å' => 'a',
        'ç' | 'Ç' => 'c',
        'è' | 'é' | 'ê' | 'ë' | 'È' | 'É' | 'Ê' | 'Ë' => 'e',
        'ì' | 'í' | 'î' | 'ï' | 'Ì' | 'Í' | 'Î' | 'Ï' => 'i',
        'ñ' | 'Ñ' => 'n',
        'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ø' | 'Ò' | 'Ó' | 'Ô' | 'Õ' | 'Ö' | 'Ø' => 'o',
        'ù' | 'ú' | 'û' | 'ü' | 'Ù' | 'Ú' | 'Û' | 'Ü' => 'u',
        'ý' | 'ÿ' | 'Ý' => 'y',
        'ß' => return vec!['s', 's'],
        'æ' | 'Æ' => return vec!['a', 'e'],
        other => other,
    };
    vec![plain]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugs() {
        assert!(is_slug("abc-123"));
        assert!(!is_slug("Abc"));
        assert!(!is_slug("a--b"));
        assert!(!is_slug("-a"));
        assert!(!is_slug(""));
        assert_eq!(slugify("Mario Kart 8 Deluxe!"), "mario-kart-8-deluxe");
        assert_eq!(slugify("  Pokémon: Ça va?  "), "pokemon-ca-va");
    }
}
