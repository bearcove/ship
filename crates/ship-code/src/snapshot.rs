use eyre::{Result, WrapErr, bail};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Manages shadow git commits for undo support within a worktree.
///
/// Every mutation creates a shadow commit. The agent never sees these.
/// When the agent calls `commit`, shadow commits are squashed into one
/// clean commit with their message.
pub struct SnapshotManager {
    /// Path to the worktree root.
    worktree: PathBuf,
    /// Monotonically increasing snapshot counter (per session).
    next_snapshot: u64,
    /// SHA of the last "real" commit (the squash base).
    last_real_commit: String,
}

impl SnapshotManager {
    /// Initialize snapshot management for a worktree.
    /// Records the current HEAD as the base for squashing.
    pub fn new(worktree: &Path) -> Result<Self> {
        let head = git_rev_parse(worktree, "HEAD")?;
        Ok(Self {
            worktree: worktree.to_owned(),
            next_snapshot: 1,
            last_real_commit: head,
        })
    }

    /// Create a shadow commit of the current worktree state.
    /// Returns the snapshot number and the diff from the previous state.
    pub fn snapshot(&mut self, message: &str) -> Result<(u64, String)> {
        // Get diff before committing
        let diff = self.diff_working()?;

        if diff.is_empty() {
            // Nothing changed, don't create a snapshot
            return Ok((self.next_snapshot.saturating_sub(1), String::new()));
        }

        // Stage everything and create shadow commit
        git(
            &self.worktree,
            &["add", "-A"],
        )?;
        git(
            &self.worktree,
            &["commit", "-m", &format!("[shadow {}] {}", self.next_snapshot, message)],
        )?;

        let snapshot = self.next_snapshot;
        self.next_snapshot += 1;
        Ok((snapshot, diff))
    }

    /// Restore the worktree to a previous snapshot.
    /// Returns the diff from current state to the restored state.
    pub fn undo(&mut self, snapshot: u64) -> Result<String> {
        // Find the shadow commit for this snapshot
        let target = self.find_shadow_commit(snapshot)?;
        let current_head = git_rev_parse(&self.worktree, "HEAD")?;

        // Get the diff we're about to apply (current -> target)
        let diff = git(
            &self.worktree,
            &["diff", &current_head, &target],
        )?;

        // Reset to the target snapshot
        git(&self.worktree, &["reset", "--hard", &target])?;

        Ok(diff)
    }

    /// Squash all shadow commits since the last real commit into one
    /// clean commit with the given message. Resets the shadow count.
    pub fn squash_commit(&mut self, message: &str) -> Result<String> {
        // Get the combined diff for the commit message
        let diff = git(
            &self.worktree,
            &["diff", &self.last_real_commit, "HEAD"],
        )?;

        if diff.is_empty() {
            bail!("nothing to commit");
        }

        // Soft reset to the last real commit (keeps changes staged)
        git(
            &self.worktree,
            &["reset", "--soft", &self.last_real_commit],
        )?;

        // Create the real commit
        git(
            &self.worktree,
            &["commit", "-m", message],
        )?;

        // Update base
        self.last_real_commit = git_rev_parse(&self.worktree, "HEAD")?;
        self.next_snapshot = 1;

        Ok(diff)
    }

    /// Number of shadow commits since last real commit.
    pub fn shadow_count(&self) -> u64 {
        self.next_snapshot.saturating_sub(1)
    }

    /// Get a nudge message if the shadow count is high.
    pub fn nudge(&self) -> Option<String> {
        let count = self.shadow_count();
        if count >= 100 {
            Some(format!(
                "You have {count} uncommitted edits. Consider committing."
            ))
        } else {
            None
        }
    }

    /// Diff of working tree against HEAD.
    fn diff_working(&self) -> Result<String> {
        // Include both staged and unstaged changes
        let unstaged = git(&self.worktree, &["diff"])?;
        let untracked = git(
            &self.worktree,
            &["ls-files", "--others", "--exclude-standard"],
        )?;

        let mut diff = unstaged;
        if !untracked.is_empty() {
            // Stage and diff untracked files to get their content as a diff
            git(&self.worktree, &["add", "-A"])?;
            let full = git(&self.worktree, &["diff", "--cached"])?;
            // Unstage so we don't interfere with the snapshot flow
            git(&self.worktree, &["reset"])?;
            diff = full;
        } else if diff.is_empty() {
            // Check staged changes too
            diff = git(&self.worktree, &["diff", "--cached"])?;
        }

        Ok(diff)
    }

    /// Find the shadow commit SHA for a given snapshot number.
    fn find_shadow_commit(&self, snapshot: u64) -> Result<String> {
        let log = git(
            &self.worktree,
            &[
                "log",
                "--oneline",
                "--grep",
                &format!("[shadow {snapshot}]"),
                "--fixed-strings",
                &format!("{}..HEAD", self.last_real_commit),
            ],
        )?;

        let first_line = log
            .lines()
            .next()
            .ok_or_else(|| eyre::eyre!("snapshot {snapshot} not found"))?;

        let sha = first_line
            .split_whitespace()
            .next()
            .ok_or_else(|| eyre::eyre!("could not parse SHA from: {first_line}"))?;

        Ok(sha.to_owned())
    }
}

/// Run a git command in the given worktree, return stdout.
fn git(worktree: &Path, args: &[&str]) -> Result<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(worktree)
        .output()
        .wrap_err_with(|| format!("failed to run git {}", args.join(" ")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("git {} failed: {}", args.join(" "), stderr.trim());
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

/// Get the SHA for a ref.
fn git_rev_parse(worktree: &Path, rev: &str) -> Result<String> {
    git(worktree, &["rev-parse", rev])
}
