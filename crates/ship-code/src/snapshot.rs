use eyre::{Result, bail};
use ship_git::{CommitHash, GitContext, Rev};

/// Manages shadow git commits for undo support within a worktree.
///
/// Every mutation creates a shadow commit. The agent never sees these.
/// When the agent calls `commit`, shadow commits are squashed into one
/// clean commit with their message.
#[derive(Debug, Clone)]
pub struct SnapshotManager {
    git: GitContext,
    /// Monotonically increasing snapshot counter (per session).
    next_snapshot: u64,
    /// SHA of the last "real" commit (the squash base).
    last_real_commit: CommitHash,
}

impl SnapshotManager {
    /// Initialize snapshot management for a worktree.
    /// Records the current HEAD as the base for squashing.
    pub async fn new(git: GitContext) -> Result<Self> {
        let head = git.rev_parse(&Rev::from("HEAD")).await?;
        Ok(Self {
            git,
            next_snapshot: 1,
            last_real_commit: head,
        })
    }

    /// Create a shadow commit of the current worktree state.
    /// Returns the snapshot number and the diff from the previous state.
    pub async fn snapshot(&mut self, message: &str) -> Result<(u64, String)> {
        // Check if there are any changes to snapshot
        let status = self.git.status().await?;
        if status.is_clean() {
            return Ok((self.current_snapshot(), String::new()));
        }

        // Stage everything
        self.git.add_all().await?;

        // Get diff of what we're about to commit
        let diff = self.git.diff_cached().await?;

        // Create shadow commit
        let shadow_msg = format!("[shadow {}] {}", self.next_snapshot, message);
        self.git.commit(&shadow_msg).await?;

        let snapshot = self.next_snapshot;
        self.next_snapshot += 1;
        Ok((snapshot, diff.into_string()))
    }

    /// Restore the worktree to a previous snapshot.
    /// Returns the diff from current state to the restored state.
    pub async fn undo(&mut self, snapshot: u64) -> Result<String> {
        let target = self.find_shadow_commit(snapshot).await?;
        let head = self.git.rev_parse(&Rev::from("HEAD")).await?;

        // Get the diff we're about to apply (current -> target)
        let diff = self
            .git
            .diff(&Rev::from(&head), &Rev::from(&target))
            .await?;

        // Reset to the target snapshot
        self.git.reset_hard(&Rev::from(&target)).await?;

        Ok(diff.into_string())
    }

    /// Squash all shadow commits since the last real commit into one
    /// clean commit with the given message. Resets the shadow count.
    pub async fn squash_commit(&mut self, message: &str) -> Result<String> {
        let head = self.git.rev_parse(&Rev::from("HEAD")).await?;

        // Get the combined diff
        let diff = self
            .git
            .diff(&Rev::from(&self.last_real_commit), &Rev::from(&head))
            .await?;

        if diff.as_str().is_empty() {
            bail!("nothing to commit");
        }

        // Soft reset to the last real commit (keeps changes staged)
        self.git
            .reset_soft(&Rev::from(&self.last_real_commit))
            .await?;

        // Create the real commit
        self.git.commit(message).await?;

        // Update base
        self.last_real_commit = self.git.rev_parse(&Rev::from("HEAD")).await?;
        self.next_snapshot = 1;

        Ok(diff.into_string())
    }

    /// Number of shadow commits since last real commit.
    pub fn shadow_count(&self) -> u64 {
        self.next_snapshot.saturating_sub(1)
    }

    /// Current snapshot number (last created, or 0 if none).
    fn current_snapshot(&self) -> u64 {
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

    /// Find the shadow commit hash for a given snapshot number.
    async fn find_shadow_commit(&self, snapshot: u64) -> Result<CommitHash> {
        let range = format!("{}..HEAD", self.last_real_commit);
        let entries = self.git.log(&range).await?;

        let needle = format!("[shadow {snapshot}]");
        for entry in &entries {
            if entry.subject.contains(&needle) {
                return Ok(entry.hash.clone());
            }
        }

        bail!("snapshot {snapshot} not found")
    }
}
