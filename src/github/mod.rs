//! Native GitHub access via octocrab (REST + GraphQL for Projects v2),
//! replacing the bash `gh` shell-outs. The client wraps a blocking tokio
//! runtime so the synchronous command layer can call async octocrab.

pub mod auth;
pub mod projects;

use crate::error::{Error, Result};
use octocrab::Octocrab;
use serde_json::{json, Value};
use std::path::Path;

/// An `owner/repo` pair.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Repo {
    pub owner: String,
    pub name: String,
}

impl Repo {
    pub fn nwo(&self) -> String {
        format!("{}/{}", self.owner, self.name)
    }

    /// Resolve the repo. Prefers the `GITHUB_REPOSITORY` env var (`owner/repo`,
    /// the value GitHub Actions sets) so it works in CI and proxied checkouts
    /// where the `origin` remote isn't a parseable github.com URL; otherwise it
    /// parses the `origin` remote of the git checkout at `root`.
    pub fn detect(root: &Path) -> Result<Repo> {
        if let Ok(slug) = std::env::var("GITHUB_REPOSITORY") {
            if let Some(repo) = parse_slug(slug.trim()) {
                return Ok(repo);
            }
        }
        let out = crate::process::run_captured("git remote get-url origin", root)
            .map_err(|e| Error::msg(format!("git remote get-url origin failed: {e}")))?;
        if out.status != 0 {
            return Err(Error::msg(
                "no GitHub repo: set GITHUB_REPOSITORY=owner/repo or add a github.com 'origin' remote.",
            ));
        }
        parse_remote(out.stdout.trim()).ok_or_else(|| {
            Error::msg(format!(
                "could not parse a GitHub repo from {:?} (set GITHUB_REPOSITORY=owner/repo to override)",
                out.stdout.trim()
            ))
        })
    }
}

/// Parse a bare `owner/repo` slug (e.g. `GITHUB_REPOSITORY`), stripping any
/// trailing `.git`. Returns `None` unless it's exactly two non-empty segments.
pub fn parse_slug(slug: &str) -> Option<Repo> {
    let slug = slug.trim().trim_end_matches(".git");
    let mut parts = slug.split('/');
    let owner = parts.next()?.trim();
    let name = parts.next()?.trim();
    if owner.is_empty() || name.is_empty() || parts.next().is_some() {
        return None;
    }
    Some(Repo {
        owner: owner.to_string(),
        name: name.to_string(),
    })
}

/// Parse `owner/repo` from a github remote URL (ssh or https forms).
pub fn parse_remote(url: &str) -> Option<Repo> {
    let url = url.trim();
    let rest = url
        .strip_prefix("git@github.com:")
        .or_else(|| url.strip_prefix("ssh://git@github.com/"))
        .or_else(|| url.strip_prefix("https://github.com/"))
        .or_else(|| url.strip_prefix("http://github.com/"))?;
    let rest = rest.strip_suffix(".git").unwrap_or(rest);
    let mut parts = rest.split('/');
    let owner = parts.next()?.to_string();
    let name = parts.next()?.to_string();
    if owner.is_empty() || name.is_empty() {
        return None;
    }
    Some(Repo { owner, name })
}

/// A connected GitHub client bound to one repo.
pub struct Github {
    crab: Octocrab,
    pub repo: Repo,
    rt: tokio::runtime::Runtime,
}

impl Github {
    /// Connect: resolve a token + the repo, build the client + runtime.
    pub fn connect(root: &Path) -> Result<Self> {
        let token = auth::resolve_token()?;
        let repo = Repo::detect(root)?;
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| Error::msg(format!("tokio runtime: {e}")))?;
        // `Octocrab::build()` spawns a tower Buffer task, so it must run inside
        // the runtime context — build it within `block_on`.
        let crab = rt
            .block_on(async { Octocrab::builder().personal_token(token).build() })
            .map_err(|e| Error::msg(format!("github client: {e}")))?;
        Ok(Github { crab, repo, rt })
    }

    fn block<F: std::future::Future>(&self, f: F) -> F::Output {
        self.rt.block_on(f)
    }

    fn get(&self, route: String) -> Result<Value> {
        self.block(async { self.crab.get::<Value, _, ()>(&route, None).await })
            .map_err(api_err)
    }

    fn post(&self, route: String, body: Value) -> Result<Value> {
        self.block(async { self.crab.post::<Value, Value>(&route, Some(&body)).await })
            .map_err(api_err)
    }

    fn patch(&self, route: String, body: Value) -> Result<Value> {
        self.block(async {
            self.crab
                .patch::<Value, _, Value>(&route, Some(&body))
                .await
        })
        .map_err(api_err)
    }

    /// Raw GraphQL query, for Projects v2.
    pub fn graphql(&self, query: Value) -> Result<Value> {
        self.block(async { self.crab.graphql::<Value>(&query).await })
            .map_err(api_err)
    }

    // --- labels --------------------------------------------------------------

    /// Create-or-update a label with `color` (no leading `#`) + description.
    pub fn ensure_label(&self, name: &str, color: &str, desc: &str) -> Result<()> {
        let base = format!("/repos/{}/labels", self.repo.nwo());
        let existing = self.get(format!("{base}/{}", urlenc(name)));
        let body = json!({ "name": name, "color": color, "description": desc });
        if existing.is_ok() {
            self.patch(format!("{base}/{}", urlenc(name)), body)?;
        } else {
            self.post(base, body)?;
        }
        Ok(())
    }

    // --- milestones ----------------------------------------------------------

    /// All milestones (any state).
    pub fn list_milestones(&self) -> Result<Vec<Value>> {
        let v = self.get(format!(
            "/repos/{}/milestones?state=all&per_page=100",
            self.repo.nwo()
        ))?;
        Ok(v.as_array().cloned().unwrap_or_default())
    }

    /// Create-or-update a milestone by title, returning its number.
    pub fn ensure_milestone(&self, title: &str, desc: &str) -> Result<u64> {
        if let Some(m) = self
            .list_milestones()?
            .into_iter()
            .find(|m| m.get("title").and_then(|t| t.as_str()) == Some(title))
        {
            let num = m.get("number").and_then(|n| n.as_u64()).unwrap_or(0);
            if !desc.is_empty() {
                let _ = self.patch(
                    format!("/repos/{}/milestones/{num}", self.repo.nwo()),
                    json!({ "description": desc }),
                );
            }
            return Ok(num);
        }
        let created = self.post(
            format!("/repos/{}/milestones", self.repo.nwo()),
            json!({ "title": title, "description": desc }),
        )?;
        Ok(created.get("number").and_then(|n| n.as_u64()).unwrap_or(0))
    }

    // --- issues --------------------------------------------------------------

    /// Create an issue, returning its number.
    pub fn create_issue(
        &self,
        title: &str,
        body: &str,
        labels: &[String],
        milestone: Option<u64>,
    ) -> Result<u64> {
        let mut payload = json!({ "title": title, "body": body, "labels": labels });
        if let Some(m) = milestone {
            payload["milestone"] = json!(m);
        }
        let created = self.post(format!("/repos/{}/issues", self.repo.nwo()), payload)?;
        Ok(created.get("number").and_then(|n| n.as_u64()).unwrap_or(0))
    }

    /// List issues filtered by state and (optionally) a label.
    pub fn list_issues(&self, state: &str, label: Option<&str>) -> Result<Vec<Value>> {
        let mut route = format!(
            "/repos/{}/issues?state={state}&per_page=100",
            self.repo.nwo()
        );
        if let Some(l) = label {
            route.push_str(&format!("&labels={}", urlenc(l)));
        }
        let v = self.get(route)?;
        // Filter out PRs (the issues endpoint includes them).
        Ok(v.as_array()
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .filter(|i| i.get("pull_request").is_none())
            .collect())
    }

    /// Fetch a single issue.
    pub fn get_issue(&self, number: u64) -> Result<Value> {
        self.get(format!("/repos/{}/issues/{number}", self.repo.nwo()))
    }

    /// Comment on an issue/PR.
    pub fn comment(&self, number: u64, body: &str) -> Result<()> {
        self.post(
            format!("/repos/{}/issues/{number}/comments", self.repo.nwo()),
            json!({ "body": body }),
        )?;
        Ok(())
    }

    /// List comments on an issue/PR.
    pub fn list_comments(&self, number: u64) -> Result<Vec<Value>> {
        let v = self.get(format!(
            "/repos/{}/issues/{number}/comments?per_page=100",
            self.repo.nwo()
        ))?;
        Ok(v.as_array().cloned().unwrap_or_default())
    }

    /// Update an existing comment by id.
    pub fn update_comment(&self, comment_id: u64, body: &str) -> Result<()> {
        self.patch(
            format!("/repos/{}/issues/comments/{comment_id}", self.repo.nwo()),
            json!({ "body": body }),
        )?;
        Ok(())
    }

    /// Replace an issue's labels.
    pub fn set_labels(&self, number: u64, labels: &[String]) -> Result<()> {
        self.patch(
            format!("/repos/{}/issues/{number}", self.repo.nwo()),
            json!({ "labels": labels }),
        )?;
        Ok(())
    }

    /// Set an issue's open/closed state.
    pub fn set_state(&self, number: u64, state: &str) -> Result<()> {
        self.patch(
            format!("/repos/{}/issues/{number}", self.repo.nwo()),
            json!({ "state": state }),
        )?;
        Ok(())
    }

    /// List an issue's sub-issues (GitHub sub-issues REST API). Returns an empty
    /// list if the repo/issue has none (or the API is unavailable).
    pub fn sub_issues(&self, number: u64) -> Result<Vec<Value>> {
        let route = format!(
            "/repos/{}/issues/{number}/sub_issues?per_page=100",
            self.repo.nwo()
        );
        match self.get(route) {
            Ok(v) => Ok(v.as_array().cloned().unwrap_or_default()),
            Err(_) => Ok(Vec::new()),
        }
    }

    /// Move an issue's `status:*` label, preserving all other labels.
    pub fn set_status(&self, number: u64, new_status: &str) -> Result<()> {
        let issue = self.get_issue(number)?;
        let mut labels: Vec<String> = issue
            .get("labels")
            .and_then(|l| l.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|l| l.get("name").and_then(|n| n.as_str()).map(String::from))
                    .filter(|n| !n.starts_with("status:"))
                    .collect()
            })
            .unwrap_or_default();
        labels.push(format!("status:{new_status}"));
        self.set_labels(number, &labels)
    }
}

/// Upsert a marker-tagged comment: update the one containing `marker`, else
/// create a new one. Used by `ci` (PR comment) and arch review.
pub fn upsert_marked_comment(gh: &Github, number: u64, marker: &str, body: &str) -> Result<()> {
    let comments = gh.list_comments(number)?;
    if let Some(existing) = comments.iter().find(|c| {
        c.get("body")
            .and_then(|b| b.as_str())
            .map(|b| b.contains(marker))
            .unwrap_or(false)
    }) {
        let id = existing.get("id").and_then(|i| i.as_u64()).unwrap_or(0);
        gh.update_comment(id, body)
    } else {
        gh.comment(number, body)
    }
}

fn urlenc(s: &str) -> String {
    // Minimal: encode spaces; label/milestone names rarely have other specials.
    s.replace(' ', "%20")
}

fn api_err(e: octocrab::Error) -> Error {
    // octocrab's Display for a GitHub API error is just "GitHub"; surface the
    // status + message so failures (auth, 404, rate limit) are actionable.
    let detail = match &e {
        octocrab::Error::GitHub { source, .. } => {
            format!("{} ({})", source.message, source.status_code)
        }
        other => other.to_string(),
    };
    Error::msg(format!("github API: {detail}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_ssh_remote() {
        let r = parse_remote("git@github.com:tizz98/ai-meta.git").unwrap();
        assert_eq!(r.owner, "tizz98");
        assert_eq!(r.name, "ai-meta");
    }

    #[test]
    fn parses_https_remote_with_and_without_git() {
        assert_eq!(
            parse_remote("https://github.com/a/b.git").unwrap().nwo(),
            "a/b"
        );
        assert_eq!(parse_remote("https://github.com/a/b").unwrap().nwo(), "a/b");
    }

    #[test]
    fn rejects_non_github() {
        assert!(parse_remote("https://gitlab.com/a/b.git").is_none());
        assert!(parse_remote("garbage").is_none());
    }

    #[test]
    fn parses_github_repository_slug() {
        assert_eq!(
            parse_slug("tizz98/ai-meta").unwrap().nwo(),
            "tizz98/ai-meta"
        );
        assert_eq!(
            parse_slug("tizz98/ai-meta.git").unwrap().nwo(),
            "tizz98/ai-meta"
        );
        assert!(parse_slug("nope").is_none());
        assert!(parse_slug("a/b/c").is_none());
        assert!(parse_slug("").is_none());
    }
}
