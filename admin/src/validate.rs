//! Consistency checks for the database, and "data/ matches a fresh export".

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fs;
use std::path::Path;

use anyhow::Result;
use rusqlite::{Connection, params};
use serde_json::{Value, json};

use crate::db::{SCHEMA_VERSION, is_slug};
use crate::export::{AVATAR_EXTENSIONS, Exporter, SHARE_DIR, walk_files};
use crate::model::{self, Status};
use crate::rules::compute_tournament;

const FIX: &str = "run `ug-admin export`";

#[derive(Debug, Default, Clone, PartialEq)]
pub struct Report {
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

impl Report {
    fn error(&mut self, msg: impl Into<String>) {
        self.errors.push(msg.into());
    }

    fn warn(&mut self, msg: impl Into<String>) {
        self.warnings.push(msg.into());
    }

    pub fn merge(&mut self, other: Report) {
        self.errors.extend(other.errors);
        self.warnings.extend(other.warnings);
    }

    pub fn ok(&self) -> bool {
        self.errors.is_empty()
    }
}

/// Discord CDN links carry an expiry (`ex=...`) and stop working after about a day.
fn is_expiring_avatar(url: &str) -> bool {
    let url = url.to_lowercase();
    let hosts = ["cdn.discordapp.com/", "cdn.discordapp.net/", "media.discordapp.com/", "media.discordapp.net/"];
    let Some(rest) = url.strip_prefix("https://") else { return false };
    hosts.iter().any(|h| rest.starts_with(h)) && (rest.contains("?ex=") || rest.contains("&ex="))
}

fn list(items: &[i64]) -> String {
    format!("[{}]", items.iter().map(i64::to_string).collect::<Vec<_>>().join(", "))
}

pub fn validate_database(conn: &Connection, avatars_dir: Option<&Path>) -> Result<Report> {
    let mut report = Report::default();

    let integrity: String = conn.query_row("PRAGMA integrity_check", [], |r| r.get(0))?;
    if integrity != "ok" {
        report.error(format!("SQLite integrity check failed: {integrity}"));
    }
    {
        let mut stmt = conn.prepare("PRAGMA foreign_key_check")?;
        let rows =
            stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, Option<i64>>(1)?, r.get::<_, String>(2)?)))?;
        for row in rows {
            let (table, rowid, parent) = row?;
            let rowid = rowid.map_or("None".into(), |r| r.to_string());
            report.error(format!("Foreign key violation in {table} (rowid {rowid}) -> {parent}"));
        }
    }

    {
        let mut stmt =
            conn.prepare("SELECT name FROM players WHERE is_demo = 0 AND discord_id IS NULL ORDER BY name")?;
        for name in stmt.query_map([], |r| r.get::<_, String>(0))? {
            report.warn(format!("player {}: no Discord ID (add it on the Players page)", name?));
        }
    }
    for table in ["players", "games", "tournaments", "categories", "awards"] {
        let mut stmt = conn.prepare(&format!("SELECT slug FROM {table}"))?;
        for slug in stmt.query_map([], |r| r.get::<_, String>(0))? {
            let slug = slug?;
            if !is_slug(&slug) {
                report.error(format!("{table}: invalid slug '{slug}' (use lowercase kebab-case)"));
            }
        }
    }

    let players = model::players(conn)?;
    let names: HashMap<i64, String> = players.iter().map(|p| (p.id, p.name.clone())).collect();
    let name_or_id = |id: i64| names.get(&id).cloned().unwrap_or_else(|| id.to_string());
    let name_or_none = |id: Option<i64>| id.and_then(|id| names.get(&id).cloned()).unwrap_or_else(|| "None".into());
    let game_names: HashMap<i64, String> = model::games(conn)?.into_iter().map(|g| (g.id, g.name)).collect();

    let slugs: HashSet<&str> = players.iter().map(|p| p.slug.as_str()).collect();
    for p in &players {
        if p.avatar_url.as_deref().is_some_and(is_expiring_avatar) {
            report.warn(format!(
                "player {}: the avatar URL is a signed Discord link that expires; save the picture as avatars/{}.jpg instead",
                p.slug, p.slug
            ));
        }
    }
    if let Some(dir) = avatars_dir
        && let Ok(entries) = fs::read_dir(dir)
    {
        let mut files: Vec<String> = entries.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
        files.sort();
        for name in files {
            if name.starts_with('.') {
                continue;
            }
            let (stem, ext) = name.rfind('.').map_or((name.as_str(), ""), |i| name.split_at(i));
            // Pictures copied from Discord are named <slug>.discord.png.
            let stem = stem.strip_suffix(".discord").unwrap_or(stem);
            if !AVATAR_EXTENSIONS.contains(&ext.to_lowercase().as_str()) || stem.is_empty() {
                report.warn(format!("avatars/{name}: not a supported picture ({})", AVATAR_EXTENSIONS.join(", ")));
            } else if !slugs.contains(stem) {
                report.warn(format!("avatars/{name}: no player with the slug '{stem}'"));
            }
        }
    }

    let mut tournaments = model::tournaments(conn)?;
    tournaments.sort_by_key(|t| t.id);
    for t in &tournaments {
        let tid = t.id;
        let label = format!("tournament {}", t.slug);
        let participants: HashSet<i64> = model::participant_ids(conn, tid)?.into_iter().collect();
        let points_table: HashMap<i64, i64> = model::points_table(conn, tid)?.into_iter().collect();
        let max_points = points_table.values().max().copied();
        let rounds = model::rounds(conn, tid)?;
        let completed: Vec<i64> = rounds.iter().filter(|r| r.completed).map(|r| r.number).collect();
        let pending: Vec<i64> = rounds.iter().filter(|r| !r.completed).map(|r| r.number).collect();

        if completed != (1..=completed.len() as i64).collect::<Vec<_>>() {
            report.error(format!(
                "{label}: completed rounds must be numbered 1..n without gaps (got {})",
                list(&completed)
            ));
        }
        if pending.len() > 1 {
            report.error(format!("{label}: more than one unfinished round ({})", list(&pending)));
        }
        if let (Some(first_pending), Some(last_done)) = (pending.first(), completed.iter().max())
            && first_pending < last_done
        {
            report.error(format!("{label}: unfinished round {first_pending} comes before completed round {last_done}"));
        }

        let all_results = model::results(conn, tid)?;
        for r in &rounds {
            let number = r.number;
            let place = format!("{label} round {number}");
            let results: Vec<&model::ResultRow> =
                all_results.iter().filter(|(n, _)| *n == number).map(|(_, r)| r).collect();
            let in_round: HashSet<i64> = results.iter().map(|r| r.player_id).collect();
            if r.completed {
                if r.game_id.is_none() {
                    report.error(format!("{place}: completed round has no game"));
                }
                if results.len() < 2 {
                    report.error(format!("{place}: completed round needs results for at least two players"));
                }
                let winners = results.iter().filter(|r| r.placement == 1).count();
                if winners != 1 {
                    report.error(format!("{place}: needs exactly one winner (placement 1), found {winners}"));
                }
            } else if !results.is_empty() {
                report.error(format!("{place}: has results but is not marked completed"));
            }

            for res in &results {
                let who = name_or_id(res.player_id);
                if !participants.contains(&res.player_id) {
                    report.error(format!("{place}: {who} is not registered for the tournament"));
                }
                if let Some(nemesis) = res.nemesis_player_id
                    && !in_round.contains(&nemesis)
                {
                    report
                        .warn(format!("{place}: {who}'s Nemesis {} has no result in this round", name_or_id(nemesis)));
                }
                // Sanity check against typos: base points come from the tournament's points table.
                if let Some(max) = max_points
                    && res.base_points > max
                {
                    report.error(format!(
                        "{place}: {who} has {} base points, more than any placement gives (max {max}); check for a typo",
                        res.base_points
                    ));
                } else if let Some(expected) = points_table.get(&res.placement)
                    && res.base_points != *expected
                {
                    report.warn(format!(
                        "{place}: {who} placed {} and got {} base points; the points table gives {expected}",
                        res.placement, res.base_points
                    ));
                }
            }

            let announced: BTreeMap<i64, i64> = model::nemesis_assignments(conn, tid, number)?.into_iter().collect();
            for (player_id, nemesis) in &announced {
                for pid in [player_id, nemesis] {
                    if !participants.contains(pid) {
                        report.error(format!("{place}: Nemesis pair with {}, who is not registered", name_or_id(*pid)));
                    }
                }
            }
            if r.completed {
                for res in &results {
                    if let Some(expected) = announced.get(&res.player_id)
                        && res.nemesis_player_id != Some(*expected)
                    {
                        report.warn(format!(
                            "{place}: {}'s result has a different Nemesis than the one announced ({})",
                            name_or_id(res.player_id),
                            name_or_none(Some(*expected))
                        ));
                    }
                }
            }

            let noms = model::nominations(conn, tid, number)?;
            for n in &noms {
                if let Some(pid) = n.player_id
                    && !participants.contains(&pid)
                {
                    report.error(format!("{place}: nomination by non-participant {}", name_or_none(Some(pid))));
                }
            }
            if let Some(gid) = r.game_id
                && !noms.is_empty()
                && !noms.iter().any(|n| n.game_id == gid)
            {
                let game = game_names.get(&gid).cloned().unwrap_or_else(|| "None".into());
                report.error(format!("{place}: selected game {game} is not among the recorded nominations"));
            }
        }

        let mut stmt = conn.prepare(
            "SELECT player_id, SUM(double_down) AS used FROM results WHERE tournament_id = ?1
             GROUP BY player_id HAVING used > ?2",
        )?;
        for row in
            stmt.query_map(params![tid, t.double_down_tokens], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)))?
        {
            let (pid, used) = row?;
            report.error(format!(
                "{label}: {} used {used} Double Down tokens (max {})",
                name_or_none(Some(pid)),
                t.double_down_tokens
            ));
        }
        let mut stmt = conn.prepare(
            "SELECT player_id, COUNT(*) AS used FROM nominations WHERE tournament_id = ?1 AND source = 'token'
             GROUP BY player_id HAVING used > ?2",
        )?;
        for row in
            stmt.query_map(params![tid, t.nominate_tokens], |r| Ok((r.get::<_, Option<i64>>(0)?, r.get::<_, i64>(1)?)))?
        {
            let (pid, used) = row?;
            report.error(format!(
                "{label}: {} used {used} Nominate tokens (max {})",
                name_or_none(pid),
                t.nominate_tokens
            ));
        }

        let state = compute_tournament(conn, t, &names)?;
        let deciding = state.deciding_round.map_or("None".into(), |r| r.to_string());
        if !state.ignored_rounds.is_empty() {
            report.error(format!(
                "{label}: rounds {} were played after the tournament was decided in round {deciding}",
                list(&state.ignored_rounds)
            ));
        }
        if t.status == Status::Completed && state.champion_id.is_none() {
            report.warn(format!("{label}: marked completed but no Challenger has won a game"));
        }
        if !pending.is_empty() && state.champion_id.is_some() {
            report.error(format!("{label}: has an unfinished round although the tournament is decided"));
        }
    }

    Ok(report)
}

/// Check every JSON document's envelope and that it matches a fresh export of the database.
pub fn validate_output(conn: &Connection, out_dir: &Path, avatars_dir: Option<&Path>) -> Result<Report> {
    let mut report = Report::default();
    let exporter = Exporter::new(conn, avatars_dir)?;
    let expected = exporter.documents();

    let published = out_dir.join("avatars");
    let wanted: BTreeSet<&String> = exporter.avatar_files.values().collect();
    let present_pictures: BTreeSet<String> = fs::read_dir(&published)
        .map(|entries| {
            entries
                .flatten()
                .filter(|e| e.path().is_file())
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    for name in &wanted {
        if !present_pictures.contains(*name) {
            report.error(format!("avatars/{name}: missing — {FIX}"));
        }
    }
    for name in &present_pictures {
        if !wanted.contains(name) {
            report.error(format!("avatars/{name}: not produced by the export — {FIX}"));
        } else if let Some(dir) = avatars_dir
            && fs::read(published.join(name)).ok() != fs::read(dir.join(name)).ok()
        {
            report.error(format!("avatars/{name}: out of date — {FIX}"));
        }
    }

    let relative = |p: &Path| {
        p.strip_prefix(out_dir)
            .unwrap_or(p)
            .components()
            .map(|c| c.as_os_str().to_string_lossy())
            .collect::<Vec<_>>()
            .join("/")
    };
    let all_files = walk_files(out_dir);

    let shares = exporter.share_pages();
    let share_root = out_dir.join(SHARE_DIR);
    let present_shares: BTreeMap<String, &std::path::PathBuf> =
        all_files.iter().filter(|p| p.starts_with(&share_root)).map(|p| (relative(p), p)).collect();
    for rel in shares.keys() {
        if !present_shares.contains_key(rel) {
            report.error(format!("{rel}: missing — {FIX}"));
        }
    }
    for (rel, path) in &present_shares {
        match shares.get(rel) {
            None => report.error(format!("{rel}: not produced by the export — {FIX}")),
            Some(content) if fs::read_to_string(path).ok().as_ref() != Some(content) => {
                report.error(format!("{rel}: out of date — {FIX}"));
            }
            Some(_) => {}
        }
    }

    let present: BTreeMap<String, &std::path::PathBuf> =
        all_files.iter().filter(|p| p.extension().is_some_and(|e| e == "json")).map(|p| (relative(p), p)).collect();
    for rel in expected.keys() {
        if !present.contains_key(rel) {
            report.error(format!("{rel}: missing — {FIX}"));
        }
    }
    for rel in present.keys() {
        if !expected.contains_key(rel) {
            report.error(format!("{rel}: not produced by the export — {FIX}"));
        }
    }
    for (rel, path) in &present {
        let Some(data) = expected.get(rel) else { continue };
        let doc: Value =
            match fs::read_to_string(path).map_err(anyhow::Error::from).and_then(|t| Ok(serde_json::from_str(&t)?)) {
                Ok(doc) => doc,
                Err(err) => {
                    report.error(format!("{rel}: invalid JSON ({err})"));
                    continue;
                }
            };
        if doc.get("schemaVersion") != Some(&json!(SCHEMA_VERSION)) {
            report.error(format!("{rel}: schemaVersion must be {SCHEMA_VERSION}"));
        }
        if !doc.get("generatedAt").and_then(Value::as_str).is_some_and(|s| s.ends_with('Z')) {
            report.error(format!("{rel}: generatedAt must be an ISO 8601 UTC timestamp"));
        }
        if doc.get("data") != Some(data) {
            report.error(format!("{rel}: out of date with the database — {FIX}"));
        }
    }
    Ok(report)
}

/// Database checks plus, unless `skip_json`, the published output check.
pub fn validate(conn: &Connection, out_dir: &Path, avatars_dir: Option<&Path>, skip_json: bool) -> Result<Report> {
    let mut report = validate_database(conn, avatars_dir)?;
    if !skip_json {
        report.merge(validate_output(conn, out_dir, avatars_dir)?);
    }
    Ok(report)
}
