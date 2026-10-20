//! Per-user settings of the desktop app (GitHub sign-in and repository).

use std::fs;
use std::path::PathBuf;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::github::RepoRef;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// A GitHub token with Contents read/write access to the data repository.
    pub github_token: Option<String>,
    /// The GitHub user the token belongs to (shown in the app).
    pub github_login: Option<String>,
    pub repo: RepoRef,
    /// A Discord bot token, used only to copy players' names and pictures from Discord.
    pub discord_token: Option<String>,
}

impl Config {
    fn path() -> PathBuf {
        directories::ProjectDirs::from("io.github", "The Ultimate Gamer", "ug-admin")
            .map_or_else(|| PathBuf::from("ug-admin-config.json"), |d| d.config_dir().join("config.json"))
    }

    pub fn load() -> Self {
        fs::read_to_string(Self::path()).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
    }

    /// Saved readable by the current user only, because it holds the token.
    pub fn save(&self) -> Result<()> {
        let path = Self::path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let text = serde_json::to_string_pretty(self)?;
        #[cfg(unix)]
        {
            use std::io::Write;
            use std::os::unix::fs::OpenOptionsExt;
            let mut file = fs::OpenOptions::new().write(true).create(true).truncate(true).mode(0o600).open(&path)?;
            file.write_all(text.as_bytes())?;
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o600))?;
        }
        #[cfg(not(unix))]
        fs::write(&path, text)?;
        Ok(())
    }

    pub fn location() -> PathBuf {
        Self::path()
    }
}

/// Reuse the GitHub CLI's sign-in, if `gh` is installed and logged in.
pub fn token_from_gh_cli() -> Option<String> {
    let out = std::process::Command::new("gh").args(["auth", "token"]).output().ok()?;
    let token = String::from_utf8(out.stdout).ok()?.trim().to_string();
    (out.status.success() && !token.is_empty()).then_some(token)
}
