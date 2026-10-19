//! Keeps a workspace in step with GitHub: download the latest data, see what changed, publish.

use std::collections::BTreeMap;
use std::fs;
use std::io::Read;
use std::path::{Component, Path};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use crate::export::walk_files;
use crate::github::{GitHub, Head, Snapshot, blob_sha};
use crate::workspace::{PUBLISHED_DIRS, Workspace};

const STATE_FILE: &str = ".ug-admin-sync.json";

/// What the workspace was last downloaded from or published to.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SyncState {
    /// owner/name of the repository.
    pub repo: String,
    pub branch: String,
    pub commit: String,
    /// Published files at that commit: path -> blob SHA.
    pub files: Snapshot,
    /// When it happened (UTC).
    pub at: String,
}

impl SyncState {
    pub fn load(ws: &Workspace) -> Option<Self> {
        let text = fs::read_to_string(ws.root.join(STATE_FILE)).ok()?;
        serde_json::from_str(&text).ok()
    }

    fn save(&self, ws: &Workspace) -> Result<()> {
        fs::write(ws.root.join(STATE_FILE), serde_json::to_string_pretty(self)?)?;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChangeKind {
    New,
    Changed,
    Removed,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Change {
    pub path: String,
    pub kind: ChangeKind,
    /// What this file is on the website, e.g. "Player profile: nova". None for the database itself.
    pub label: Option<String>,
}

/// Published files in the workspace right now: path -> blob SHA.
pub fn local_snapshot(ws: &Workspace) -> Snapshot {
    let mut files = Snapshot::new();
    for dir in PUBLISHED_DIRS {
        for path in walk_files(&ws.root.join(dir)) {
            let rel =
                ws.relative(&path).components().map(|c| c.as_os_str().to_string_lossy()).collect::<Vec<_>>().join("/");
            if is_ignored(&rel) {
                continue;
            }
            if let Ok(bytes) = fs::read(&path) {
                files.insert(rel, blob_sha(&bytes));
            }
        }
    }
    files
}

/// SQLite side files and OS clutter are never published.
fn is_ignored(rel: &str) -> bool {
    let name = rel.rsplit('/').next().unwrap_or(rel);
    name == ".DS_Store" || name.ends_with("-journal") || name.ends_with("-wal") || name.ends_with("-shm")
}

/// Differences between two snapshots, from `base` to `current`.
pub fn diff(base: &Snapshot, current: &Snapshot) -> Vec<Change> {
    let mut changes = Vec::new();
    for (path, sha) in current {
        match base.get(path) {
            None => changes.push(change(path, ChangeKind::New)),
            Some(old) if old != sha => changes.push(change(path, ChangeKind::Changed)),
            _ => {}
        }
    }
    for path in base.keys() {
        if !current.contains_key(path) {
            changes.push(change(path, ChangeKind::Removed));
        }
    }
    changes.sort_by(|a, b| a.path.cmp(&b.path));
    changes
}

fn change(path: &str, kind: ChangeKind) -> Change {
    Change { path: path.to_string(), kind, label: describe(path) }
}

/// Human-readable description of what a published file is on the website.
pub fn describe(path: &str) -> Option<String> {
    if path.starts_with("database/") {
        return None;
    }
    let rel = path.strip_prefix("data/").unwrap_or(path);
    let fixed = match rel {
        "site.json" => Some("Site settings"),
        "leaderboard.json" => Some("Homepage standings"),
        "categories.json" => Some("Categories"),
        "awards.json" => Some("Awards page"),
        "games/index.json" => Some("Games page"),
        "players/index.json" => Some("Players page"),
        "tournaments/index.json" => Some("Tournaments page"),
        _ => None,
    };
    if let Some(label) = fixed {
        return Some(label.to_string());
    }
    let stem = |s: &str| s.rsplit_once('.').map_or(s.to_string(), |(a, _)| a.to_string());
    if let Some(name) = rel.strip_prefix("avatars/") {
        return Some(format!("Player picture: {}", stem(name)));
    }
    if let Some(rest) = rel.strip_prefix("share/") {
        let rest = rest.strip_suffix("/index.html").unwrap_or(rest);
        if let Some(slug) = rest.strip_prefix("players/") {
            return Some(format!("Link preview: player {slug}"));
        }
        if let Some(slug) = rest.strip_prefix("tournaments/") {
            return Some(format!("Link preview: tournament {slug}"));
        }
    }
    if let Some(name) = rel.strip_prefix("players/") {
        return Some(format!("Player profile: {}", stem(name)));
    }
    if let Some(name) = rel.strip_prefix("tournaments/") {
        return Some(format!("Tournament page: {}", stem(name)));
    }
    Some(rel.to_string())
}

/// A commit message such as "Update Homepage standings, Player profile: nova and 3 more".
pub fn summary(changes: &[Change]) -> String {
    let mut labels: Vec<&str> = Vec::new();
    for c in changes {
        if let Some(label) = c.label.as_deref()
            && !label.starts_with("Link preview")
            && !labels.contains(&label)
        {
            labels.push(label);
        }
    }
    if labels.is_empty() {
        return "Update tournament data".into();
    }
    let head = labels.iter().take(4).copied().collect::<Vec<_>>().join(", ");
    if labels.len() > 4 { format!("Update {head} and {} more", labels.len() - 4) } else { format!("Update {head}") }
}

/// Unpublished changes in the workspace, compared with what it was last synced to.
pub fn pending_changes(ws: &Workspace, state: Option<&SyncState>) -> Vec<Change> {
    let empty = Snapshot::new();
    diff(state.map_or(&empty, |s| &s.files), &local_snapshot(ws))
}

// --------------------------------------------------------------- download

/// Replace the workspace's published folders with the branch's latest version.
pub fn download(gh: &GitHub, ws: &Workspace) -> Result<SyncState> {
    let head = gh.head()?;
    let archive = gh.tarball(&head.commit)?;
    let staging = ws.root.join(".ug-admin-download");
    let _ = fs::remove_dir_all(&staging);
    fs::create_dir_all(&staging)?;
    extract(&archive, &staging).context("unpacking the download")?;

    if !staging.join(crate::workspace::DB_FILE).is_file() {
        let _ = fs::remove_dir_all(&staging);
        bail!("{} has no {} on {}", gh.repo.full_name(), crate::workspace::DB_FILE, gh.repo.branch);
    }
    fs::create_dir_all(&ws.root)?;
    for dir in PUBLISHED_DIRS {
        let target = ws.root.join(dir);
        let _ = fs::remove_dir_all(&target);
        if staging.join(dir).exists() {
            fs::rename(staging.join(dir), &target)?;
        }
    }
    let _ = fs::remove_dir_all(&staging);

    let state = SyncState {
        repo: gh.repo.full_name(),
        branch: gh.repo.branch.clone(),
        commit: head.commit,
        files: local_snapshot(ws),
        at: crate::db::utc_now(),
    };
    state.save(ws)?;
    Ok(state)
}

/// Unpack only the published folders from a GitHub tarball (which has one top-level folder).
fn extract(archive: &[u8], into: &Path) -> Result<()> {
    let mut tar = tar::Archive::new(flate2::read::GzDecoder::new(archive));
    for entry in tar.entries()? {
        let mut entry = entry?;
        if !entry.header().entry_type().is_file() {
            continue;
        }
        let path = entry.path()?.into_owned();
        let parts: Vec<&str> = path
            .components()
            .skip(1) // "<owner>-<repo>-<sha>/"
            .map(|c| match c {
                Component::Normal(s) => s.to_str(),
                _ => None,
            })
            .collect::<Option<_>>()
            .unwrap_or_default();
        if parts.is_empty() || !PUBLISHED_DIRS.contains(&parts[0]) {
            continue;
        }
        let target = parts.iter().fold(into.to_path_buf(), |p, part| p.join(part));
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes)?;
        fs::write(target, bytes)?;
    }
    Ok(())
}

// ---------------------------------------------------------------- publish

#[derive(Debug)]
pub enum PublishOutcome {
    /// Nothing differs from GitHub.
    NothingToPublish,
    Published {
        commit: String,
        files: usize,
    },
    /// Someone else published data since this workspace was last synced.
    Conflict {
        theirs: Vec<Change>,
    },
}

/// Send the workspace's published folders to GitHub as one commit.
///
/// Refuses (returns `Conflict`) when GitHub has data changes this workspace hasn't seen,
/// unless `overwrite` is set, in which case this workspace's version wins.
pub fn publish(
    gh: &GitHub,
    ws: &Workspace,
    state: &SyncState,
    message: &str,
    overwrite: bool,
) -> Result<PublishOutcome> {
    if state.repo != gh.repo.full_name() || state.branch != gh.repo.branch {
        bail!(
            "this data was downloaded from {} ({}); download from {} first",
            state.repo,
            state.branch,
            gh.repo.full_name()
        );
    }
    let head: Head = gh.head()?;
    let remote = gh.snapshot(&head.tree, PUBLISHED_DIRS)?;
    if head.commit != state.commit && remote != state.files && !overwrite {
        return Ok(PublishOutcome::Conflict { theirs: diff(&state.files, &remote) });
    }

    let local = local_snapshot(ws);
    let changes = diff(&remote, &local);
    if changes.is_empty() {
        let state = SyncState { commit: head.commit, files: local, at: crate::db::utc_now(), ..state.clone() };
        state.save(ws)?;
        return Ok(PublishOutcome::NothingToPublish);
    }
    let mut contents = BTreeMap::new();
    for c in &changes {
        let content = match c.kind {
            ChangeKind::Removed => None,
            _ => Some(fs::read(ws.root.join(&c.path))?),
        };
        contents.insert(c.path.clone(), content);
    }
    let commit = gh.commit(&head, &contents, message)?;
    let state = SyncState { commit: commit.clone(), files: local, at: crate::db::utc_now(), ..state.clone() };
    state.save(ws)?;
    Ok(PublishOutcome::Published { commit, files: changes.len() })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn describes_files() {
        assert_eq!(describe("database/ultimate-gamer.sqlite"), None);
        assert_eq!(describe("data/leaderboard.json").as_deref(), Some("Homepage standings"));
        assert_eq!(describe("data/players/nova.json").as_deref(), Some("Player profile: nova"));
        assert_eq!(describe("data/share/players/nova/index.html").as_deref(), Some("Link preview: player nova"));
        assert_eq!(describe("avatars/nova.jpg").as_deref(), Some("Player picture: nova"));
    }

    #[test]
    fn diffs_and_summarises() {
        let base: Snapshot = [("a".into(), "1".into()), ("data/site.json".into(), "2".into())].into();
        let now: Snapshot = [("data/site.json".into(), "3".into()), ("data/players/x.json".into(), "4".into())].into();
        let changes = diff(&base, &now);
        assert_eq!(
            changes.iter().map(|c| (c.path.as_str(), c.kind)).collect::<Vec<_>>(),
            vec![
                ("a", ChangeKind::Removed),
                ("data/players/x.json", ChangeKind::New),
                ("data/site.json", ChangeKind::Changed),
            ]
        );
        assert_eq!(summary(&changes), "Update a, Player profile: x, Site settings");
    }

    #[test]
    fn extracts_only_published_folders() {
        let mut builder = tar::Builder::new(flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast()));
        for (path, body) in [
            ("org-data-abc/database/x.sqlite", "db"),
            ("org-data-abc/scripts/a.py", "no"),
            ("org-data-abc/data/site.json", "{}"),
        ] {
            let mut header = tar::Header::new_gnu();
            header.set_size(body.len() as u64);
            header.set_mode(0o644);
            header.set_cksum();
            builder.append_data(&mut header, path, body.as_bytes()).unwrap();
        }
        let archive = builder.into_inner().unwrap().finish().unwrap();
        let dir = tempfile::tempdir().unwrap();
        extract(&archive, dir.path()).unwrap();
        assert_eq!(fs::read_to_string(dir.path().join("database/x.sqlite")).unwrap(), "db");
        assert!(dir.path().join("data/site.json").exists());
        assert!(!dir.path().join("scripts").exists());
    }
}

#[cfg(test)]
mod repo_tests {
    use super::*;

    /// The snapshot of the real repository must match git's own index for the committed files.
    #[test]
    fn snapshot_matches_git_index() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let Ok(out) = std::process::Command::new("git")
            .current_dir(&root)
            .args(["ls-files", "-s", "--", "database", "data", "avatars"])
            .output()
        else {
            return; // git not installed
        };
        let index: Snapshot = String::from_utf8_lossy(&out.stdout)
            .lines()
            .filter_map(|l| {
                let (meta, path) = l.split_once('\t')?;
                Some((path.to_string(), meta.split_whitespace().nth(1)?.to_string()))
            })
            .collect();
        if index.is_empty() {
            return;
        }
        let local = local_snapshot(&Workspace::at(&root));
        for (path, sha) in &index {
            if local.get(path) != Some(sha) {
                // Only fails when the working tree differs from git, which `git status` would show.
                let dirty = std::process::Command::new("git")
                    .current_dir(&root)
                    .args(["diff", "--quiet", "--", path])
                    .status()
                    .map_or(true, |s| !s.success());
                assert!(dirty, "{path}: {:?} != {sha}", local.get(path));
            }
        }
    }
}
