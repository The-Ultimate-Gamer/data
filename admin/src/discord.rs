//! Copies a player's public Discord profile (names and picture) through Discord's REST API.
//!
//! Only the admin calls Discord, with a bot token that stays in the admin's settings; the
//! website reads the copied names from the exported JSON and never talks to Discord itself.

use std::time::Duration;

use anyhow::{Context, Result, bail};
use serde::Deserialize;
use serde_json::Value;
use ureq::Agent;

const API: &str = "https://discord.com/api/v10";
const CDN: &str = "https://cdn.discordapp.com";
const MAX_PICTURE: u64 = 8 * 1024 * 1024;

/// A Discord user id ("snowflake"): 15 to 21 digits.
pub fn is_user_id(value: &str) -> bool {
    (15..=21).contains(&value.len()) && value.bytes().all(|b| b.is_ascii_digit())
}

/// The public part of a Discord account.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
pub struct DiscordUser {
    pub id: String,
    pub username: String,
    /// Display name, when the user set one.
    pub global_name: Option<String>,
    /// Avatar hash; None when the user kept Discord's default picture.
    pub avatar: Option<String>,
}

impl DiscordUser {
    /// The avatar as a 256 px PNG (animated avatars give their first frame).
    pub fn avatar_url(&self, cdn: &str) -> Option<String> {
        let hash = self.avatar.as_deref()?;
        let safe = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_');
        (is_user_id(&self.id) && safe(hash)).then(|| format!("{cdn}/avatars/{}/{hash}.png?size=256", self.id))
    }
}

pub struct Discord {
    agent: Agent,
    /// `UG_DISCORD_API` / `UG_DISCORD_CDN` override these (tests use a local stand-in).
    api: String,
    cdn: String,
    token: String,
}

impl Discord {
    pub fn new(token: impl Into<String>) -> Self {
        let agent = Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(20)))
            .http_status_as_error(false)
            .user_agent(concat!(
                "DiscordBot (https://github.com/The-Ultimate-Gamer/data, ",
                env!("CARGO_PKG_VERSION"),
                ")"
            ))
            .build()
            .into();
        Self {
            agent,
            api: std::env::var("UG_DISCORD_API").unwrap_or_else(|_| API.to_string()),
            cdn: std::env::var("UG_DISCORD_CDN").unwrap_or_else(|_| CDN.to_string()),
            token: token.into().trim().trim_start_matches("Bot ").to_string(),
        }
    }

    pub fn user(&self, id: &str) -> Result<DiscordUser> {
        if !is_user_id(id) {
            bail!("\"{id}\" is not a Discord user ID");
        }
        let resp = self
            .agent
            .get(&format!("{}/users/{id}", self.api))
            .header("Authorization", format!("Bot {}", self.token))
            .call()
            .context("could not reach Discord (check your internet connection)")?;
        let status = resp.status().as_u16();
        let body: Value = resp.into_body().read_json().unwrap_or(Value::Null);
        match status {
            200..=299 => Ok(serde_json::from_value(body).context("unexpected reply from Discord")?),
            401 => bail!("Discord refused the bot token; paste a new one in Settings"),
            404 | 400 => bail!("Discord has no user with ID {id}"),
            429 => bail!("Discord is rate-limiting requests; try again in a minute"),
            _ => {
                let message = body.get("message").and_then(Value::as_str).unwrap_or("no details");
                bail!("Discord request failed (HTTP {status}: {message})")
            }
        }
    }

    /// The user's picture as PNG bytes, or None when they use Discord's default picture.
    pub fn picture(&self, user: &DiscordUser) -> Result<Option<Vec<u8>>> {
        let Some(url) = user.avatar_url(&self.cdn) else { return Ok(None) };
        let resp = self.agent.get(&url).call().context("could not download the Discord picture")?;
        if !resp.status().is_success() {
            bail!("downloading the Discord picture failed (HTTP {})", resp.status().as_u16());
        }
        Ok(Some(resp.into_body().with_config().limit(MAX_PICTURE).read_to_vec()?))
    }
}

/// Fetch a player's Discord profile; with `picture`, also their avatar.
pub fn fetch(token: &str, id: &str, picture: bool) -> Result<(DiscordUser, Option<Vec<u8>>)> {
    let discord = Discord::new(token);
    let user = discord.user(id)?;
    let bytes = if picture { discord.picture(&user)? } else { None };
    Ok((user, bytes))
}

/// Replace a player's stored picture with a PNG.
pub fn save_picture(avatars_dir: &std::path::Path, slug: &str, png: &[u8]) -> Result<()> {
    std::fs::create_dir_all(avatars_dir)?;
    crate::export::remove_avatars(avatars_dir, slug);
    std::fs::write(avatars_dir.join(format!("{slug}.png")), png)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_ids() {
        assert!(is_user_id("80351110224678912"));
        assert!(!is_user_id("1234"));
        assert!(!is_user_id("8035111022467891a"));
        assert!(!is_user_id("https://discord.com/users/80351110224678912"));
    }

    #[test]
    fn avatar_urls() {
        let mut u = DiscordUser {
            id: "80351110224678912".into(),
            avatar: Some("a_1269e74af4df7417b13759eae50c83dc".into()),
            ..Default::default()
        };
        assert_eq!(
            u.avatar_url(CDN).as_deref(),
            Some(
                "https://cdn.discordapp.com/avatars/80351110224678912/a_1269e74af4df7417b13759eae50c83dc.png?size=256"
            )
        );
        u.avatar = Some("../x".into());
        assert_eq!(u.avatar_url(CDN), None);
        u.avatar = None;
        assert_eq!(u.avatar_url(CDN), None);
    }
}
