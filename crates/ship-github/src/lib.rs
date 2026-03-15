mod types;

pub use types::*;

use camino::{Utf8Path, Utf8PathBuf};
use eyre::{Context, Result, bail};
use tokio::process::Command;

/// A GitHub context scoped to a specific working directory.
/// All operations run `gh` with `current_dir` set to this directory,
/// which lets `gh` auto-detect the repository from the git remote.
#[derive(Debug, Clone)]
pub struct GithubContext {
    workdir: Utf8PathBuf,
}

impl GithubContext {
    /// Create a new GithubContext for the given directory.
    /// The directory should be inside a git repo with a GitHub remote.
    pub fn new(workdir: impl Into<Utf8PathBuf>) -> Self {
        Self {
            workdir: workdir.into(),
        }
    }

    /// The working directory this context operates in.
    pub fn workdir(&self) -> &Utf8Path {
        &self.workdir
    }

    // ── Helpers ─────────────────────────────────────────────────────────

    async fn run(&self, args: &[&str]) -> Result<String> {
        let output = Command::new("gh")
            .args(args)
            .current_dir(self.workdir.as_std_path())
            .output()
            .await
            .wrap_err("failed to run gh")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            bail!("gh {} failed: {}", args.join(" "), stderr.trim());
        }

        let stdout = String::from_utf8(output.stdout)
            .wrap_err("gh output was not valid UTF-8")?;
        Ok(stdout)
    }

    // ── Repository ──────────────────────────────────────────────────────

    /// Get information about the current repository.
    pub async fn repo_info(&self) -> Result<RepoInfo> {
        let out = self
            .run(&[
                "repo",
                "view",
                "--json",
                "owner,name,defaultBranchRef,url",
            ])
            .await
            .wrap_err("failed to get repo info")?;

        let v: RepoInfoJson = facet_json::from_str(&out)
            .wrap_err("failed to parse repo info JSON")?;

        Ok(RepoInfo {
            owner: v.owner.login,
            name: v.name,
            default_branch: v.default_branch_ref.name,
            url: v.url,
        })
    }

    // ── Push ────────────────────────────────────────────────────────────

    /// Push the current branch to the remote, setting upstream tracking.
    pub async fn push_branch(&self, branch: &str) -> Result<()> {
        // Use git for push — gh doesn't have a push command.
        let output = Command::new("git")
            .args(["push", "-u", "origin", branch])
            .current_dir(self.workdir.as_std_path())
            .output()
            .await
            .wrap_err("failed to run git push")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            bail!("git push failed: {}", stderr.trim());
        }

        Ok(())
    }

    // ── Pull Requests ───────────────────────────────────────────────────

    /// Create a pull request. Returns the created PR.
    pub async fn create_pr(&self, opts: &CreatePrOptions<'_>) -> Result<PullRequest> {
        let mut args = vec![
            "pr",
            "create",
            "--title",
            opts.title,
            "--body",
            opts.body,
            "--head",
            opts.head,
            "--base",
            opts.base,
            "--json",
            PR_JSON_FIELDS,
        ];

        if opts.draft {
            args.push("--draft");
        }

        let out = self
            .run(&args)
            .await
            .wrap_err("failed to create pull request")?;

        parse_pr_json(&out)
    }

    /// Get an existing pull request by number.
    pub async fn get_pr(&self, number: u64) -> Result<PullRequest> {
        let num_str = number.to_string();
        let out = self
            .run(&["pr", "view", &num_str, "--json", PR_JSON_FIELDS])
            .await
            .wrap_err_with(|| format!("failed to get PR #{number}"))?;

        parse_pr_json(&out)
    }

    /// Find an open PR for a given head branch. Returns None if no PR exists.
    pub async fn find_pr_for_branch(&self, branch: &str) -> Result<Option<PullRequest>> {
        let out = self
            .run(&[
                "pr",
                "list",
                "--head",
                branch,
                "--state",
                "open",
                "--json",
                PR_JSON_FIELDS,
                "--limit",
                "1",
            ])
            .await
            .wrap_err("failed to list PRs")?;

        let prs: Vec<PrJson> =
            facet_json::from_str(&out).wrap_err("failed to parse PR list JSON")?;

        match prs.into_iter().next() {
            Some(pr) => Ok(Some(pr_from_json(pr))),
            None => Ok(None),
        }
    }

    /// Close a pull request (without merging).
    pub async fn close_pr(&self, number: u64) -> Result<()> {
        let num_str = number.to_string();
        self.run(&["pr", "close", &num_str])
            .await
            .wrap_err_with(|| format!("failed to close PR #{number}"))?;
        Ok(())
    }

    /// Add a comment to a pull request.
    pub async fn comment_pr(&self, number: u64, body: &str) -> Result<()> {
        let num_str = number.to_string();
        self.run(&["pr", "comment", &num_str, "--body", body])
            .await
            .wrap_err_with(|| format!("failed to comment on PR #{number}"))?;
        Ok(())
    }

    /// Mark a draft PR as ready for review.
    pub async fn mark_pr_ready(&self, number: u64) -> Result<()> {
        let num_str = number.to_string();
        self.run(&["pr", "ready", &num_str])
            .await
            .wrap_err_with(|| format!("failed to mark PR #{number} as ready"))?;
        Ok(())
    }

    /// Edit a PR's title and/or body.
    pub async fn edit_pr(
        &self,
        number: u64,
        title: Option<&str>,
        body: Option<&str>,
    ) -> Result<()> {
        let num_str = number.to_string();
        let mut args = vec!["pr", "edit", num_str.as_str()];

        if let Some(t) = title {
            args.push("--title");
            args.push(t);
        }
        if let Some(b) = body {
            args.push("--body");
            args.push(b);
        }

        self.run(&args)
            .await
            .wrap_err_with(|| format!("failed to edit PR #{number}"))?;
        Ok(())
    }

    // ── Checks ──────────────────────────────────────────────────────────

    /// Get the status checks for a PR.
    pub async fn pr_checks(&self, number: u64) -> Result<Vec<CheckRun>> {
        let num_str = number.to_string();
        let out = self
            .run(&[
                "pr",
                "checks",
                &num_str,
                "--json",
                "name,status,conclusion",
            ])
            .await
            .wrap_err_with(|| format!("failed to get checks for PR #{number}"))?;

        let checks: Vec<CheckRun> =
            facet_json::from_str(&out).wrap_err("failed to parse checks JSON")?;
        Ok(checks)
    }

    // ── Auth ────────────────────────────────────────────────────────────

    /// Check if gh is authenticated. Returns the logged-in username.
    pub async fn auth_status(&self) -> Result<String> {
        let out = self
            .run(&["auth", "status", "--hostname", "github.com"])
            .await
            .wrap_err("gh is not authenticated")?;

        // Output contains "Logged in to github.com account <username>"
        // But the exact format varies. Just return the raw output trimmed.
        Ok(out.trim().to_owned())
    }
}

// ── JSON parsing helpers ────────────────────────────────────────────────

const PR_JSON_FIELDS: &str = "number,url,title,state,headRefName,baseRefName,isDraft";

#[derive(Debug, facet::Facet)]
struct PrJson {
    number: u64,
    url: String,
    title: String,
    state: String,
    #[facet(rename = "headRefName")]
    head_ref_name: String,
    #[facet(rename = "baseRefName")]
    base_ref_name: String,
    #[facet(rename = "isDraft")]
    is_draft: bool,
}

#[derive(Debug, facet::Facet)]
struct RepoInfoJson {
    owner: OwnerJson,
    name: String,
    #[facet(rename = "defaultBranchRef")]
    default_branch_ref: BranchRefJson,
    url: String,
}

#[derive(Debug, facet::Facet)]
struct OwnerJson {
    login: String,
}

#[derive(Debug, facet::Facet)]
struct BranchRefJson {
    name: String,
}

fn parse_pr_json(json: &str) -> Result<PullRequest> {
    let pr: PrJson = facet_json::from_str(json).wrap_err("failed to parse PR JSON")?;
    Ok(pr_from_json(pr))
}

fn pr_from_json(pr: PrJson) -> PullRequest {
    PullRequest {
        number: pr.number,
        url: pr.url,
        title: pr.title,
        state: match pr.state.as_str() {
            "MERGED" => PrState::Merged,
            "CLOSED" => PrState::Closed,
            _ => PrState::Open,
        },
        head_branch: pr.head_ref_name,
        base_branch: pr.base_ref_name,
        draft: pr.is_draft,
    }
}
