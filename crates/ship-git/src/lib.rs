mod types;

pub use types::*;

use camino::{Utf8Path, Utf8PathBuf};

use eyre::{Context, Result, bail};
use tokio::process::Command;

/// A git context scoped to a specific worktree directory.
/// All operations run `git` with `current_dir` set to this worktree.
#[derive(Debug, Clone)]
pub struct GitContext {
    worktree: Utf8PathBuf,
}

impl GitContext {
    /// Create a new GitContext for the given worktree directory.
    pub fn new(worktree: impl Into<Utf8PathBuf>) -> Self {
        Self {
            worktree: worktree.into(),
        }
    }

    /// The worktree directory this context operates on.
    pub fn worktree(&self) -> &Utf8Path {
        &self.worktree
    }

    /// Initialize a new git repository at the given path.
    /// Returns a GitContext for the new repo.
    pub async fn init(path: impl Into<Utf8PathBuf>, branch: &BranchName) -> Result<Self> {
        let path = path.into();
        tokio::fs::create_dir_all(&path)
            .await
            .wrap_err_with(|| format!("creating directory {path}"))?;
        let ctx = Self::new(path);
        ctx.run(&["init", "-b", branch.as_str()]).await?;
        Ok(ctx)
    }

    // ── Private runners ──────────────────────────────────────────────

    /// Run a git command and return its stdout (trimmed).
    /// Fails if the command exits with a non-zero status.
    async fn run(&self, args: &[&str]) -> Result<String> {
        let output = Command::new("git")
            .args(args)
            .current_dir(self.worktree.as_std_path())
            .output()
            .await
            .wrap_err_with(|| format!("spawning git {}", args.first().unwrap_or(&"")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            bail!(
                "git {} failed (exit {}):\nstderr: {}\nstdout: {}",
                args.join(" "),
                output.status,
                stderr.trim(),
                stdout.trim(),
            );
        }

        let stdout = String::from_utf8(output.stdout)
            .wrap_err("git output was not valid UTF-8")?;
        Ok(stdout.trim_end_matches('\n').to_owned())
    }

    /// Run a git command, returning the raw Output.
    /// Does NOT check exit status — caller is responsible.
    async fn run_raw(&self, args: &[&str]) -> Result<std::process::Output> {
        Command::new("git")
            .args(args)
            .current_dir(self.worktree.as_std_path())
            .output()
            .await
            .wrap_err_with(|| format!("spawning git {}", args.first().unwrap_or(&"")))
    }

    // ── Rev operations ───────────────────────────────────────────────

    /// Resolve a revision to a commit hash.
    pub async fn rev_parse(&self, rev: &Rev) -> Result<CommitHash> {
        let out = self
            .run(&["rev-parse", rev.as_str()])
            .await
            .wrap_err_with(|| format!("rev-parse {rev}"))?;
        Ok(CommitHash::new(out))
    }

    /// Get the current branch name via `symbolic-ref --short HEAD`.
    pub async fn branch_name(&self) -> Result<BranchName> {
        let out = self
            .run(&["symbolic-ref", "--short", "HEAD"])
            .await
            .wrap_err("getting current branch name")?;
        Ok(BranchName::new(out))
    }

    /// Find the merge base of two revisions.
    pub async fn merge_base(&self, a: &Rev, b: &Rev) -> Result<CommitHash> {
        let out = self
            .run(&["merge-base", a.as_str(), b.as_str()])
            .await
            .wrap_err_with(|| format!("merge-base {a} {b}"))?;
        Ok(CommitHash::new(out))
    }

    /// Count commits reachable from `rev`.
    pub async fn rev_list_count(&self, rev: &Rev) -> Result<usize> {
        let out = self
            .run(&["rev-list", "--count", rev.as_str()])
            .await
            .wrap_err_with(|| format!("rev-list --count {rev}"))?;
        out.parse::<usize>()
            .wrap_err_with(|| format!("parsing rev-list count: {out:?}"))
    }

    /// Check whether a ref exists (resolves to a commit).
    pub async fn ref_exists(&self, rev: &Rev) -> Result<bool> {
        let output = self
            .run_raw(&["rev-parse", "--verify", "--quiet", &format!("{}^{{commit}}", rev)])
            .await?;
        Ok(output.status.success())
    }

    // ── Staging ──────────────────────────────────────────────────────

    /// Stage all changes (`git add -A`).
    pub async fn add_all(&self) -> Result<()> {
        self.run(&["add", "-A"])
            .await
            .wrap_err("git add -A")?;
        Ok(())
    }

    /// Stage specific paths.
    pub async fn add(&self, paths: &[&Utf8Path]) -> Result<()> {
        let path_strs: Vec<&str> = paths.iter().map(|p| p.as_str()).collect();
        let mut args = vec!["add", "--"];
        args.extend(path_strs);
        self.run(&args).await.wrap_err("git add")?;
        Ok(())
    }

    // ── Commit ───────────────────────────────────────────────────────

    /// Create a commit with the given message.
    /// Returns the hash and subject of the new commit.
    pub async fn commit(&self, message: &str) -> Result<CommitInfo> {
        self.run(&["commit", "-m", message])
            .await
            .wrap_err("git commit")?;

        let hash = self
            .run(&["rev-parse", "HEAD"])
            .await
            .wrap_err("reading HEAD after commit")?;
        let subject = self
            .run(&["log", "-1", "--format=%s", "HEAD"])
            .await
            .wrap_err("reading commit subject")?;

        Ok(CommitInfo {
            hash: CommitHash::new(hash),
            subject,
        })
    }

    // ── Diff ─────────────────────────────────────────────────────────

    /// Raw unified diff between two revisions.
    pub async fn diff(&self, a: &Rev, b: &Rev) -> Result<Diff> {
        let out = self
            .run(&["diff", &format!("{a}..{b}")])
            .await
            .wrap_err_with(|| format!("diff {a}..{b}"))?;
        Ok(Diff::new(out))
    }

    /// Raw unified diff of staged changes.
    pub async fn diff_cached(&self) -> Result<Diff> {
        let out = self.run(&["diff", "--cached"]).await.wrap_err("diff --cached")?;
        Ok(Diff::new(out))
    }

    /// Returns true if there are staged changes (non-empty cached diff).
    pub async fn diff_cached_quiet(&self) -> Result<bool> {
        let output = self.run_raw(&["diff", "--cached", "--quiet"]).await?;
        // Exit 0 = no diff, exit 1 = has diff
        Ok(!output.status.success())
    }

    /// Raw unified diff of unstaged working tree changes.
    pub async fn diff_working(&self) -> Result<Diff> {
        let out = self.run(&["diff"]).await.wrap_err("diff (working)")?;
        Ok(Diff::new(out))
    }

    /// Numstat diff between two revisions.
    pub async fn diff_numstat(&self, a: &Rev, b: &Rev) -> Result<DiffStats> {
        let out = self
            .run(&["diff", "--numstat", &format!("{a}..{b}")])
            .await
            .wrap_err_with(|| format!("diff --numstat {a}..{b}"))?;
        parse_numstat(&out)
    }

    /// Numstat diff of uncommitted changes (staged + unstaged) against a rev.
    pub async fn diff_numstat_against(&self, rev: &Rev) -> Result<DiffStats> {
        let out = self
            .run(&["diff", "--numstat", rev.as_str()])
            .await
            .wrap_err_with(|| format!("diff --numstat {rev}"))?;
        parse_numstat(&out)
    }

    /// Numstat of uncommitted changes against HEAD.
    pub async fn diff_numstat_head(&self) -> Result<DiffStats> {
        let out = self
            .run(&["diff", "--numstat", "HEAD"])
            .await
            .wrap_err("diff --numstat HEAD")?;
        parse_numstat(&out)
    }

    // ── Log ──────────────────────────────────────────────────────────

    /// Log commits in a range, returning abbreviated hash + subject.
    pub async fn log(&self, range: &str) -> Result<Vec<LogEntry>> {
        let out = self
            .run(&["log", "--format=%h %s", range])
            .await
            .wrap_err_with(|| format!("log {range}"))?;
        Ok(parse_log(&out))
    }

    /// Log commits as one-line strings (for display).
    pub async fn log_oneline(&self, range: &str) -> Result<Vec<String>> {
        let out = self
            .run(&["log", "--oneline", range])
            .await
            .wrap_err_with(|| format!("log --oneline {range}"))?;
        Ok(out.lines().map(|l| l.to_owned()).collect())
    }

    // ── Show ─────────────────────────────────────────────────────────

    /// Show a commit's diff (no commit header).
    pub async fn show(&self, rev: &Rev) -> Result<Diff> {
        let out = self
            .run(&["show", "--format=", rev.as_str()])
            .await
            .wrap_err_with(|| format!("show {rev}"))?;
        Ok(Diff::new(out))
    }

    /// Show a file at a specific revision (`git show <rev>:<path>`).
    pub async fn show_file(&self, rev: &Rev, path: &Utf8Path) -> Result<String> {
        let spec = format!("{rev}:{path}");
        self.run(&["show", &spec])
            .await
            .wrap_err_with(|| format!("show {spec}"))
    }

    /// Show numstat for a commit.
    pub async fn show_numstat(&self, rev: &Rev) -> Result<DiffStats> {
        let out = self
            .run(&["show", "--numstat", "--format=", rev.as_str()])
            .await
            .wrap_err_with(|| format!("show --numstat {rev}"))?;
        parse_numstat(&out)
    }

    /// Show --stat --shortstat for a commit (no commit header).
    pub async fn show_stat(&self, rev: &Rev) -> Result<String> {
        self.run(&["show", "--stat", "--shortstat", "--format=", rev.as_str()])
            .await
            .wrap_err_with(|| format!("show --stat {rev}"))
    }

    /// Get the subject line of a single commit.
    pub async fn commit_subject(&self, rev: &Rev) -> Result<String> {
        self.run(&["log", "-1", "--format=%s", rev.as_str()])
            .await
            .wrap_err_with(|| format!("log -1 --format=%s {rev}"))
    }

    // ── Branch operations ────────────────────────────────────────────

    /// Create and switch to a new branch.
    pub async fn checkout_new_branch(&self, name: &BranchName) -> Result<()> {
        self.run(&["checkout", "-b", name.as_str()])
            .await
            .wrap_err_with(|| format!("checkout -b {name}"))?;
        Ok(())
    }

    /// Switch to an existing branch.
    pub async fn checkout(&self, branch: &BranchName) -> Result<()> {
        self.run(&["checkout", branch.as_str()])
            .await
            .wrap_err_with(|| format!("checkout {branch}"))?;
        Ok(())
    }

    /// List branch names.
    pub async fn branch_list(&self) -> Result<Vec<BranchName>> {
        let out = self.run(&["branch"]).await.wrap_err("branch")?;
        Ok(parse_branch_lines(&out))
    }

    /// List all branches (including remote-tracking).
    pub async fn branch_list_all(&self) -> Result<Vec<BranchName>> {
        let out = self.run(&["branch", "-a"]).await.wrap_err("branch -a")?;
        Ok(parse_branch_lines(&out))
    }

    /// Delete a branch.
    pub async fn branch_delete(&self, name: &BranchName, force: bool) -> Result<()> {
        let flag = if force { "-D" } else { "-d" };
        self.run(&["branch", flag, name.as_str()])
            .await
            .wrap_err_with(|| format!("branch {flag} {name}"))?;
        Ok(())
    }

    // ── Reset ────────────────────────────────────────────────────────

    /// Hard reset to a revision.
    pub async fn reset_hard(&self, rev: &Rev) -> Result<()> {
        self.run(&["reset", "--hard", rev.as_str()])
            .await
            .wrap_err_with(|| format!("reset --hard {rev}"))?;
        Ok(())
    }

    /// Soft reset to a revision.
    pub async fn reset_soft(&self, rev: &Rev) -> Result<()> {
        self.run(&["reset", "--soft", rev.as_str()])
            .await
            .wrap_err_with(|| format!("reset --soft {rev}"))?;
        Ok(())
    }

    // ── Rebase ───────────────────────────────────────────────────────

    /// Rebase the current branch onto the given revision.
    /// Returns Success or Conflict (with list of conflicting files).
    pub async fn rebase(&self, onto: &Rev) -> Result<RebaseOutcome> {
        let output = self.run_raw(&["rebase", onto.as_str()]).await?;
        if output.status.success() {
            return Ok(RebaseOutcome::Success);
        }

        // Rebase failed — check for conflicts
        let conflicting_files = self.unmerged_files().await?;
        if conflicting_files.is_empty() {
            // Not a conflict — some other failure
            let stderr = String::from_utf8_lossy(&output.stderr);
            bail!("git rebase {onto} failed:\n{}", stderr.trim());
        }

        Ok(RebaseOutcome::Conflict { conflicting_files })
    }

    /// Continue an in-progress rebase (after resolving conflicts and staging).
    /// Sets `GIT_EDITOR=true` to skip the editor for squash/reword commits.
    pub async fn rebase_continue(&self) -> Result<RebaseOutcome> {
        let output = Command::new("git")
            .args(["rebase", "--continue"])
            .env("GIT_EDITOR", "true")
            .current_dir(self.worktree.as_std_path())
            .output()
            .await
            .wrap_err("spawning git rebase --continue")?;

        if output.status.success() {
            return Ok(RebaseOutcome::Success);
        }

        let conflicting_files = self.unmerged_files().await?;
        if conflicting_files.is_empty() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            bail!("git rebase --continue failed:\n{}", stderr.trim());
        }

        Ok(RebaseOutcome::Conflict { conflicting_files })
    }

    /// Abort an in-progress rebase.
    pub async fn rebase_abort(&self) -> Result<()> {
        self.run(&["rebase", "--abort"])
            .await
            .wrap_err("rebase --abort")?;
        Ok(())
    }

    /// Abort an in-progress rebase, ignoring errors.
    /// Useful when cleaning up after a failed rebase.
    pub async fn rebase_abort_quiet(&self) {
        let _ = self.run_raw(&["rebase", "--abort"]).await;
    }

    /// Check if a rebase is in progress.
    pub async fn is_rebasing(&self) -> Result<bool> {
        for suffix in &["rebase-merge", "rebase-apply"] {
            let out = self
                .run(&["rev-parse", "--git-path", suffix])
                .await?;
            let trimmed = out.trim();
            let path = if Utf8Path::new(trimmed).is_absolute() {
                Utf8PathBuf::from(trimmed)
            } else {
                self.worktree.join(trimmed)
            };
            if path.as_std_path().exists() {
                return Ok(true);
            }
        }
        Ok(false)
    }

    // ── Merge ────────────────────────────────────────────────────────

    /// Fast-forward merge only.
    pub async fn merge_ff_only(&self, branch: &Rev) -> Result<()> {
        self.run(&["merge", "--ff-only", branch.as_str()])
            .await
            .wrap_err_with(|| format!("merge --ff-only {branch}"))?;
        Ok(())
    }

    // ── Status ───────────────────────────────────────────────────────

    /// Parse `git status --porcelain` output.
    pub async fn status(&self) -> Result<Status> {
        let out = self
            .run(&["status", "--porcelain"])
            .await
            .wrap_err("status --porcelain")?;
        Ok(parse_status(&out))
    }

    /// List unmerged files (conflict markers present).
    pub async fn unmerged_files(&self) -> Result<Vec<Utf8PathBuf>> {
        let out = self
            .run(&["diff", "--name-only", "--diff-filter=U"])
            .await
            .wrap_err("diff --diff-filter=U")?;
        Ok(out
            .lines()
            .filter(|l| !l.is_empty())
            .map(|l| Utf8PathBuf::from(l))
            .collect())
    }

    /// List files that differ from the given revision (`git diff --name-only <rev>`).
    pub async fn diff_name_only(&self, rev: &Rev) -> Result<Vec<Utf8PathBuf>> {
        let out = self
            .run(&["diff", "--name-only", rev.as_str()])
            .await
            .wrap_err_with(|| format!("diff --name-only {rev}"))?;
        Ok(out
            .lines()
            .filter(|l| !l.is_empty())
            .map(Utf8PathBuf::from)
            .collect())
    }

    /// List all tracked files (null-separated internally, returned as Vec).
    pub async fn ls_files(&self) -> Result<Vec<Utf8PathBuf>> {
        let output = Command::new("git")
            .args(["ls-files", "-z"])
            .current_dir(self.worktree.as_std_path())
            .output()
            .await
            .wrap_err("spawning git ls-files")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            bail!("git ls-files failed: {}", stderr.trim());
        }

        Ok(output
            .stdout
            .split(|&b| b == 0)
            .filter(|s| !s.is_empty())
            .map(|s| Utf8PathBuf::from(String::from_utf8_lossy(s).as_ref()))
            .collect())
    }

    // ── Fetch ────────────────────────────────────────────────────────

    /// Fetch a refspec from a remote.
    pub async fn fetch(&self, remote: &RemoteName, refspec: &str) -> Result<()> {
        self.run(&["fetch", remote.as_str(), refspec])
            .await
            .wrap_err_with(|| format!("fetch {remote} {refspec}"))?;
        Ok(())
    }

    /// Fetch-style update: `git fetch . <from>:<to>`.
    pub async fn fetch_local(&self, from: &Rev, to: &BranchName) -> Result<()> {
        self.run(&["fetch", ".", &format!("{from}:{to}")])
            .await
            .wrap_err_with(|| format!("fetch . {from}:{to}"))?;
        Ok(())
    }

    // ── Worktree ─────────────────────────────────────────────────────

    /// Add a new worktree with a new branch based on `base`.
    /// Returns a GitContext for the new worktree.
    pub async fn worktree_add(
        &self,
        path: &Utf8Path,
        branch: &BranchName,
        base: &Rev,
    ) -> Result<GitContext> {
        self.run(&["worktree", "add", "-b", branch.as_str(), path.as_str(), base.as_str()])
            .await
            .wrap_err_with(|| format!("worktree add -b {branch} {path} {base}"))?;
        Ok(GitContext::new(path))
    }

    /// Remove a worktree.
    pub async fn worktree_remove(&self, path: &Utf8Path, force: bool) -> Result<()> {
        let mut args = vec!["worktree", "remove"];
        if force {
            args.push("--force");
        }
        args.push(path.as_str());
        self.run(&args)
            .await
            .wrap_err_with(|| format!("worktree remove {path}"))?;
        Ok(())
    }

    // ── Stash ────────────────────────────────────────────────────────

    /// Stash the current working tree changes.
    pub async fn stash_push(&self) -> Result<()> {
        self.run(&["stash", "push"])
            .await
            .wrap_err("stash push")?;
        Ok(())
    }

    /// Pop the latest stash entry.
    pub async fn stash_pop(&self) -> Result<()> {
        self.run(&["stash", "pop"])
            .await
            .wrap_err("stash pop")?;
        Ok(())
    }

    // ── Config ───────────────────────────────────────────────────────

    /// Set a git config value (local to the repo).
    pub async fn config_set(&self, key: &str, value: &str) -> Result<()> {
        self.run(&["config", key, value])
            .await
            .wrap_err_with(|| format!("config {key} {value}"))?;
        Ok(())
    }
}

// ── Parsing helpers ──────────────────────────────────────────────────

fn parse_branch_lines(output: &str) -> Vec<BranchName> {
    output
        .lines()
        .map(|line| line.trim_start_matches(['*', '+', ' ']))
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(BranchName::new)
        .collect()
}

fn parse_numstat(output: &str) -> Result<DiffStats> {
    let mut entries = Vec::new();
    for line in output.lines() {
        if line.is_empty() {
            continue;
        }
        let mut parts = line.split('\t');
        let added = parts
            .next()
            .ok_or_else(|| eyre::eyre!("numstat: missing added column"))?;
        let removed = parts
            .next()
            .ok_or_else(|| eyre::eyre!("numstat: missing removed column"))?;
        let path = parts
            .next()
            .ok_or_else(|| eyre::eyre!("numstat: missing path column"))?;

        // Binary files show "-" instead of a number
        let added = if added == "-" { 0 } else { added.parse()? };
        let removed = if removed == "-" { 0 } else { removed.parse()? };

        entries.push(NumstatEntry {
            added,
            removed,
            path: Utf8PathBuf::from(path),
        });
    }
    Ok(DiffStats { entries })
}

fn parse_log(output: &str) -> Vec<LogEntry> {
    output
        .lines()
        .filter(|l| !l.is_empty())
        .map(|line| {
            let (hash, subject) = line.split_once(' ').unwrap_or((line, ""));
            LogEntry {
                hash: CommitHash::new(hash),
                subject: subject.to_owned(),
            }
        })
        .collect()
}

fn parse_status(output: &str) -> Status {
    let entries = output
        .lines()
        .filter(|l| l.len() >= 3)
        .map(|line| {
            let bytes = line.as_bytes();
            StatusEntry {
                index: bytes[0] as char,
                worktree: bytes[1] as char,
                path: Utf8PathBuf::from(&line[3..]),
            }
        })
        .collect();
    Status { entries }
}

#[cfg(test)]
mod tests;
