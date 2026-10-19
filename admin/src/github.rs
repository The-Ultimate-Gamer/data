//! A small GitHub REST client: just what downloading and publishing the data needs.
//!
//! Publishing creates a normal commit on the branch through the Git Data API
//! (blobs → tree → commit → move the branch), so no local git installation is needed.

use std::collections::BTreeMap;
use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use ureq::Agent;

const API: &str = "https://api.github.com";
/// Where the OAuth device flow lives; `UG_GITHUB_WEB` overrides it (tests use a local stand-in).
const WEB: &str = "https://github.com";
/// The client id of The-Ultimate-Gamer's OAuth App, once the organisation registers one
/// (see README → "Sign in with GitHub"). `UG_GITHUB_CLIENT_ID` overrides it.
const OAUTH_CLIENT_ID: Option<&str> = None;
const MAX_DOWNLOAD: u64 = 512 * 1024 * 1024;

/// Which repository and branch hold the data.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepoRef {
    pub owner: String,
    pub name: String,
    pub branch: String,
}

impl Default for RepoRef {
    fn default() -> Self {
        Self { owner: "The-Ultimate-Gamer".into(), name: "data".into(), branch: "main".into() }
    }
}

impl RepoRef {
    pub fn full_name(&self) -> String {
        format!("{}/{}", self.owner, self.name)
    }

    pub fn web_url(&self) -> String {
        format!("https://github.com/{}", self.full_name())
    }
}

#[derive(Clone)]
pub struct GitHub {
    agent: Agent,
    /// The API root; `UG_GITHUB_API` overrides it (tests use a local stand-in).
    api: String,
    token: String,
    pub repo: RepoRef,
}

/// A file in a commit's tree: path -> blob SHA.
pub type Snapshot = BTreeMap<String, String>;

#[derive(Debug, Clone)]
pub struct Head {
    pub commit: String,
    pub tree: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RunState {
    Queued,
    Running,
    Success,
    Failed,
}

#[derive(Debug, Clone)]
pub struct Run {
    pub state: RunState,
    pub url: String,
}

#[derive(Debug, Clone)]
pub struct Access {
    pub login: String,
    pub can_push: bool,
}

/// The SHA git uses for a file's content, so changes can be found without uploading anything.
pub fn blob_sha(content: &[u8]) -> String {
    use sha1::{Digest, Sha1};
    let mut hasher = Sha1::new();
    hasher.update(format!("blob {}\0", content.len()).as_bytes());
    hasher.update(content);
    hasher.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

impl GitHub {
    pub fn new(token: impl Into<String>, repo: RepoRef) -> Self {
        let agent = Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(120)))
            .http_status_as_error(false)
            .user_agent(concat!("ug-admin/", env!("CARGO_PKG_VERSION")))
            .build()
            .into();
        let api = std::env::var("UG_GITHUB_API").unwrap_or_else(|_| API.to_string());
        Self { agent, api, token: token.into(), repo }
    }

    fn url(&self, path: &str) -> String {
        format!("{}/repos/{}/{}{path}", self.api, self.repo.owner, self.repo.name)
    }

    fn check(resp: ureq::http::Response<ureq::Body>, what: &str) -> Result<Value> {
        let status = resp.status().as_u16();
        let body: Value = resp.into_body().with_config().limit(64 * 1024 * 1024).read_json().unwrap_or(Value::Null);
        if (200..300).contains(&status) {
            return Ok(body);
        }
        let message = body.get("message").and_then(Value::as_str).unwrap_or("no details");
        let hint = match status {
            401 => " — the GitHub token is invalid or expired; sign in again in Settings",
            403 | 404 => " — check that the token can access the repository (Contents: read and write)",
            409 | 422 => " — the branch moved while publishing; try again",
            _ => "",
        };
        bail!("{what} failed (HTTP {status}: {message}){hint}")
    }

    fn get(&self, url: &str, what: &str) -> Result<Value> {
        let resp = self
            .agent
            .get(url)
            .header("Authorization", format!("Bearer {}", self.token))
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .call()
            .with_context(|| format!("{what}: could not reach GitHub (check your internet connection)"))?;
        Self::check(resp, what)
    }

    fn send(&self, method: &str, url: &str, body: &Value, what: &str) -> Result<Value> {
        let builder = match method {
            "POST" => self.agent.post(url),
            "PATCH" => self.agent.patch(url),
            other => bail!("unsupported method {other}"),
        };
        let resp = builder
            .header("Authorization", format!("Bearer {}", self.token))
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .send_json(body)
            .with_context(|| format!("{what}: could not reach GitHub (check your internet connection)"))?;
        Self::check(resp, what)
    }

    fn str_field(v: &Value, pointer: &str) -> Result<String> {
        v.pointer(pointer)
            .and_then(Value::as_str)
            .map(str::to_string)
            .ok_or_else(|| anyhow!("unexpected reply from GitHub (no {pointer})"))
    }

    /// Who the token belongs to, and whether it may publish to the repository.
    pub fn access(&self) -> Result<Access> {
        let user = self.get(&format!("{}/user", self.api), "Checking the GitHub sign-in")?;
        let repo = self.get(&self.url(""), "Opening the data repository")?;
        Ok(Access {
            login: Self::str_field(&user, "/login")?,
            can_push: repo.pointer("/permissions/push").and_then(Value::as_bool).unwrap_or(false),
        })
    }

    pub fn head(&self) -> Result<Head> {
        let r = self.get(&self.url(&format!("/git/ref/heads/{}", self.repo.branch)), "Reading the branch")?;
        let commit = Self::str_field(&r, "/object/sha")?;
        let c = self.get(&self.url(&format!("/git/commits/{commit}")), "Reading the latest commit")?;
        Ok(Head { tree: Self::str_field(&c, "/tree/sha")?, commit })
    }

    /// Files (path -> blob SHA) of a tree below the given top-level folders.
    pub fn snapshot(&self, tree: &str, dirs: &[&str]) -> Result<Snapshot> {
        let t = self.get(&self.url(&format!("/git/trees/{tree}?recursive=1")), "Listing the published files")?;
        if t.get("truncated").and_then(Value::as_bool) == Some(true) {
            bail!("the repository is too large to list in one request");
        }
        let mut files = Snapshot::new();
        for entry in t.get("tree").and_then(Value::as_array).into_iter().flatten() {
            let (Some(path), Some(sha), Some("blob")) = (
                entry.get("path").and_then(Value::as_str),
                entry.get("sha").and_then(Value::as_str),
                entry.get("type").and_then(Value::as_str),
            ) else {
                continue;
            };
            if dirs.iter().any(|d| path.starts_with(&format!("{d}/"))) {
                files.insert(path.to_string(), sha.to_string());
            }
        }
        Ok(files)
    }

    /// The whole repository at `commit` as a .tar.gz archive.
    pub fn tarball(&self, commit: &str) -> Result<Vec<u8>> {
        let resp = self
            .agent
            .get(&self.url(&format!("/tarball/{commit}")))
            .header("Authorization", format!("Bearer {}", self.token))
            .header("X-GitHub-Api-Version", "2022-11-28")
            .call()
            .context("Downloading the data: could not reach GitHub (check your internet connection)")?;
        if !resp.status().is_success() {
            return Self::check(resp, "Downloading the data").map(|_| Vec::new());
        }
        Ok(resp.into_body().with_config().limit(MAX_DOWNLOAD).read_to_vec()?)
    }

    fn create_blob(&self, content: &[u8]) -> Result<String> {
        let encoded = base64::engine::general_purpose::STANDARD.encode(content);
        let r = self.send(
            "POST",
            &self.url("/git/blobs"),
            &json!({"content": encoded, "encoding": "base64"}),
            "Uploading a file",
        )?;
        Self::str_field(&r, "/sha")
    }

    /// Commit `changes` (path -> new content, or None to delete) on top of `head` and move the branch.
    pub fn commit(&self, head: &Head, changes: &BTreeMap<String, Option<Vec<u8>>>, message: &str) -> Result<String> {
        let mut entries = Vec::new();
        for (path, content) in changes {
            let sha = match content {
                Some(bytes) => Value::String(self.create_blob(bytes)?),
                None => Value::Null,
            };
            entries.push(json!({"path": path, "mode": "100644", "type": "blob", "sha": sha}));
        }
        let tree = self.send(
            "POST",
            &self.url("/git/trees"),
            &json!({"base_tree": head.tree, "tree": entries}),
            "Preparing the commit",
        )?;
        let tree = Self::str_field(&tree, "/sha")?;
        let commit = self.send(
            "POST",
            &self.url("/git/commits"),
            &json!({"message": message, "tree": tree, "parents": [head.commit]}),
            "Creating the commit",
        )?;
        let commit = Self::str_field(&commit, "/sha")?;
        self.send(
            "PATCH",
            &self.url(&format!("/git/refs/heads/{}", self.repo.branch)),
            &json!({"sha": commit, "force": false}),
            "Publishing the commit",
        )?;
        Ok(commit)
    }

    /// The publish workflow run for a commit, if GitHub has started one.
    pub fn deployment(&self, commit: &str) -> Result<Option<Run>> {
        let runs = self
            .get(&self.url(&format!("/actions/runs?head_sha={commit}&per_page=10")), "Checking the website update")?;
        let run = runs
            .get("workflow_runs")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .find(|r| r.get("path").and_then(Value::as_str).is_some_and(|p| p.ends_with("publish.yml")));
        let Some(run) = run else { return Ok(None) };
        let status = run.get("status").and_then(Value::as_str).unwrap_or("");
        let conclusion = run.get("conclusion").and_then(Value::as_str).unwrap_or("");
        let state = match (status, conclusion) {
            ("completed", "success") => RunState::Success,
            ("completed", _) => RunState::Failed,
            ("queued" | "waiting" | "pending" | "requested", _) => RunState::Queued,
            _ => RunState::Running,
        };
        Ok(Some(Run { state, url: run.get("html_url").and_then(Value::as_str).unwrap_or("").to_string() }))
    }
}

/// The OAuth App client id for device-flow sign-in, if there is one.
pub fn oauth_client_id() -> Option<String> {
    std::env::var("UG_GITHUB_CLIENT_ID")
        .ok()
        .map(|id| id.trim().to_string())
        .filter(|id| !id.is_empty())
        .or_else(|| OAUTH_CLIENT_ID.map(str::to_string))
}

/// The code the user types on GitHub to approve this app.
#[derive(Debug, Clone)]
pub struct DeviceCode {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in: Duration,
    pub interval: Duration,
}

/// Sign-in through GitHub's OAuth device flow: no token to create or paste.
pub struct DeviceFlow {
    agent: Agent,
    web: String,
    client_id: String,
}

impl DeviceFlow {
    pub fn new(client_id: impl Into<String>) -> Self {
        let agent = Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(30)))
            .http_status_as_error(false)
            .user_agent(concat!("ug-admin/", env!("CARGO_PKG_VERSION")))
            .build()
            .into();
        let web = std::env::var("UG_GITHUB_WEB").unwrap_or_else(|_| WEB.to_string());
        Self { agent, web, client_id: client_id.into() }
    }

    fn post(&self, path: &str, form: &[(&str, &str)], what: &str) -> Result<Value> {
        let resp = self
            .agent
            .post(&format!("{}{path}", self.web))
            .header("Accept", "application/json")
            .send_form(form.iter().copied())
            .with_context(|| format!("{what}: could not reach GitHub (check your internet connection)"))?;
        GitHub::check(resp, what)
    }

    /// Ask GitHub for a code the user approves at `verification_uri`.
    pub fn start(&self) -> Result<DeviceCode> {
        let what = "Starting the GitHub sign-in";
        let r = self.post("/login/device/code", &[("client_id", &self.client_id), ("scope", "repo")], what)?;
        if let Some(error) = r.get("error").and_then(Value::as_str) {
            bail!("{what} failed ({})", r.get("error_description").and_then(Value::as_str).unwrap_or(error));
        }
        let secs = |key: &str, default: u64| Duration::from_secs(r.get(key).and_then(Value::as_u64).unwrap_or(default));
        Ok(DeviceCode {
            device_code: GitHub::str_field(&r, "/device_code")?,
            user_code: GitHub::str_field(&r, "/user_code")?,
            verification_uri: GitHub::str_field(&r, "/verification_uri")?,
            expires_in: secs("expires_in", 900),
            interval: secs("interval", 5),
        })
    }

    /// Poll until the user approves (the token), declines, or the code expires.
    /// `wait` sleeps between polls and returns false to give up (the user cancelled).
    pub fn poll(&self, code: &DeviceCode, mut wait: impl FnMut(Duration) -> bool) -> Result<String> {
        let what = "Finishing the GitHub sign-in";
        let deadline = std::time::Instant::now() + code.expires_in;
        let mut interval = code.interval;
        loop {
            if !wait(interval) {
                bail!("the GitHub sign-in was cancelled");
            }
            if std::time::Instant::now() > deadline {
                bail!("the sign-in code expired; start the GitHub sign-in again");
            }
            let r = self.post(
                "/login/oauth/access_token",
                &[
                    ("client_id", &self.client_id),
                    ("device_code", &code.device_code),
                    ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
                ],
                what,
            )?;
            if let Some(token) = r.get("access_token").and_then(Value::as_str) {
                return Ok(token.to_string());
            }
            match r.get("error").and_then(Value::as_str) {
                Some("authorization_pending") => {}
                Some("slow_down") => {
                    // GitHub sends the new minimum interval; otherwise add five seconds as the spec says.
                    interval = r
                        .get("interval")
                        .and_then(Value::as_u64)
                        .map(Duration::from_secs)
                        .unwrap_or(interval + Duration::from_secs(5));
                }
                Some("expired_token") => bail!("the sign-in code expired; start the GitHub sign-in again"),
                Some("access_denied") => bail!("the GitHub sign-in was declined"),
                Some(error) => {
                    bail!("{what} failed ({})", r.get("error_description").and_then(Value::as_str).unwrap_or(error))
                }
                None => bail!("unexpected reply from GitHub (no access_token)"),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blob_sha_matches_git() {
        // `printf 'hello\n' | git hash-object --stdin`
        assert_eq!(blob_sha(b"hello\n"), "ce013625030ba8dba906f756967f9e9ca394464a");
        assert_eq!(blob_sha(b""), "e69de29bb2d1d6434b8b29ae775ad8c2e48c5391");
    }

    /// A stand-in for github.com that answers each request with the next canned JSON body,
    /// and hands back the request lines and bodies it saw.
    fn mock(replies: Vec<&'static str>) -> (String, std::thread::JoinHandle<Vec<String>>) {
        use std::io::{BufRead, BufReader, Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let handle = std::thread::spawn(move || {
            let mut seen = Vec::new();
            for reply in replies {
                let (stream, _) = listener.accept().unwrap();
                let mut reader = BufReader::new(stream);
                let mut request = String::new();
                reader.read_line(&mut request).unwrap();
                let mut length = 0;
                loop {
                    let mut line = String::new();
                    reader.read_line(&mut line).unwrap();
                    if line.trim().is_empty() {
                        break;
                    }
                    if let Some((name, value)) = line.split_once(':')
                        && name.eq_ignore_ascii_case("content-length")
                    {
                        length = value.trim().parse().unwrap();
                    }
                }
                let mut body = vec![0; length];
                reader.read_exact(&mut body).unwrap();
                seen.push(format!("{} {}", request.trim(), String::from_utf8(body).unwrap()));
                let mut stream = reader.into_inner();
                write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{reply}",
                    reply.len()
                )
                .unwrap();
            }
            seen
        });
        (base, handle)
    }

    #[test]
    fn device_flow_polls_until_the_token_arrives() {
        let (base, server) = mock(vec![
            r#"{"device_code":"dev-1","user_code":"ABCD-1234","verification_uri":"https://github.com/login/device","expires_in":900,"interval":5}"#,
            r#"{"error":"authorization_pending"}"#,
            r#"{"error":"slow_down","interval":10}"#,
            r#"{"error":"slow_down"}"#,
            r#"{"access_token":"gho_secret","token_type":"bearer","scope":"repo"}"#,
        ]);
        let flow = DeviceFlow { web: base, ..DeviceFlow::new("client-1") };
        let code = flow.start().unwrap();
        assert_eq!(code.user_code, "ABCD-1234");
        assert_eq!(code.verification_uri, "https://github.com/login/device");
        assert_eq!(code.interval, Duration::from_secs(5));

        let mut waits = Vec::new();
        let token = flow
            .poll(&code, |d| {
                waits.push(d.as_secs());
                true
            })
            .unwrap();
        assert_eq!(token, "gho_secret");
        // pending keeps the interval, slow_down takes GitHub's new one or adds five seconds.
        assert_eq!(waits, [5, 5, 10, 15]);

        let seen = server.join().unwrap();
        assert!(seen[0].starts_with("POST /login/device/code "));
        assert!(seen[0].contains("client_id=client-1") && seen[0].contains("scope=repo"));
        assert!(seen[1].starts_with("POST /login/oauth/access_token "));
        assert!(seen[1].contains("device_code=dev-1"));
        assert!(seen[1].contains("grant_type=urn%3Aietf%3Aparams%3Aoauth%3Agrant-type%3Adevice_code"));
    }

    #[test]
    fn device_flow_reports_a_declined_or_cancelled_sign_in() {
        let (base, server) = mock(vec![r#"{"error":"access_denied"}"#]);
        let flow = DeviceFlow { web: base, ..DeviceFlow::new("client-1") };
        let code = DeviceCode {
            device_code: "dev-1".into(),
            user_code: "ABCD-1234".into(),
            verification_uri: String::new(),
            expires_in: Duration::from_secs(900),
            interval: Duration::ZERO,
        };
        let err = flow.poll(&code, |_| true).unwrap_err();
        assert!(err.to_string().contains("declined"), "{err}");
        server.join().unwrap();

        let err = flow.poll(&code, |_| false).unwrap_err();
        assert!(err.to_string().contains("cancelled"), "{err}");
    }
}
