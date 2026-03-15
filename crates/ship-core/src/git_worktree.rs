use std::path::{Path, PathBuf};

use camino::Utf8Path;
use ship_git::{BranchName, GitContext, Rev};

use crate::{RebaseOutcome, WorktreeError, WorktreeOps};

// r[testability.git-trait]
#[derive(Debug, Default, Clone, Copy)]
pub struct GitWorktreeOps;

fn ctx(path: &Path) -> Result<GitContext, WorktreeError> {
    let utf8 = Utf8Path::from_path(path).ok_or_else(|| WorktreeError {
        message: format!("path is not valid UTF-8: {}", path.display()),
    })?;
    Ok(GitContext::new(utf8))
}

fn utf8(path: &Path) -> Result<&Utf8Path, WorktreeError> {
    Utf8Path::from_path(path).ok_or_else(|| WorktreeError {
        message: format!("path is not valid UTF-8: {}", path.display()),
    })
}

// r[backend.git-shell]
#[async_trait::async_trait]
impl WorktreeOps for GitWorktreeOps {
    // r[worktree.path]
    async fn create_worktree(
        &self,
        branch_name: &str,
        worktree_dir: &str,
        base_branch: &str,
        repo_root: &Path,
    ) -> Result<PathBuf, WorktreeError> {
        tracing::info!(
            branch_name = %branch_name,
            worktree_dir = %worktree_dir,
            base_branch = %base_branch,
            repo_root = %repo_root.display(),
            "resolving base ref before worktree creation"
        );
        ensure_valid_base_ref(repo_root, base_branch).await?;

        let worktree_path = repo_root.join(".ship").join(worktree_dir);

        fs_err::tokio::create_dir_all(repo_root.join(".ship"))
            .await
            .map_err(|error| WorktreeError {
                message: error.to_string(),
            })?;

        let git = ctx(repo_root)?;
        git.worktree_add(
            utf8(&worktree_path)?,
            &BranchName::new(branch_name),
            &Rev::new(base_branch),
        )
        .await?;

        tracing::info!(
            branch_name = %branch_name,
            worktree_path = %worktree_path.display(),
            "created git worktree"
        );
        Ok(worktree_path)
    }

    async fn remove_worktree(&self, path: &Path, force: bool) -> Result<(), WorktreeError> {
        let repo_root = repo_root_for_worktree(path)?;
        let git = ctx(repo_root)?;
        git.worktree_remove(utf8(path)?, force).await?;
        Ok(())
    }

    async fn has_uncommitted_changes(&self, path: &Path) -> Result<bool, WorktreeError> {
        let git = ctx(path)?;
        let status = git.status().await?;
        Ok(!status.is_clean())
    }

    async fn current_branch(&self, worktree_path: &Path) -> Result<String, WorktreeError> {
        let git = ctx(worktree_path)?;
        let branch = git.branch_name().await?;
        Ok(branch.into_string())
    }

    async fn is_rebase_in_progress(&self, worktree_path: &Path) -> Result<bool, WorktreeError> {
        let git = ctx(worktree_path)?;
        Ok(git.is_rebasing().await?)
    }

    async fn unmerged_paths(&self, worktree_path: &Path) -> Result<Vec<String>, WorktreeError> {
        let git = ctx(worktree_path)?;
        let paths = git.unmerged_files().await?;
        Ok(paths.into_iter().map(|p| p.into_string()).collect())
    }

    async fn tracked_conflict_marker_paths(
        &self,
        worktree_path: &Path,
    ) -> Result<Vec<String>, WorktreeError> {
        let mut paths = Vec::new();
        for (relative, marker_lines) in tracked_conflict_marker_details(worktree_path).await? {
            if !marker_lines.is_empty() {
                paths.push(relative);
            }
        }
        Ok(paths)
    }

    async fn tracked_conflict_marker_locations(
        &self,
        worktree_path: &Path,
    ) -> Result<Vec<String>, WorktreeError> {
        let mut locations = Vec::new();
        for (relative, marker_lines) in tracked_conflict_marker_details(worktree_path).await? {
            for line in marker_lines {
                locations.push(format!("{relative}:{line}"));
            }
        }
        Ok(locations)
    }

    async fn review_diff(
        &self,
        worktree_path: &Path,
        base_branch: &str,
    ) -> Result<String, WorktreeError> {
        let git = ctx(worktree_path)?;
        let diff = git.diff(&Rev::new(base_branch), &Rev::new("HEAD")).await?;
        Ok(diff.into_string())
    }

    async fn commit_all(&self, worktree_path: &Path, message: &str) -> Result<(), WorktreeError> {
        let git = ctx(worktree_path)?;
        git.add_all().await?;
        git.commit(message).await?;
        Ok(())
    }

    async fn list_branches(&self, repo_root: &Path) -> Result<Vec<String>, WorktreeError> {
        let git = ctx(repo_root)?;
        let branches = git.branch_list().await?;
        Ok(branches.into_iter().map(|b| b.into_string()).collect())
    }

    async fn delete_branch(
        &self,
        branch_name: &str,
        force: bool,
        repo_root: &Path,
    ) -> Result<(), WorktreeError> {
        let git = ctx(repo_root)?;
        git.branch_delete(&BranchName::new(branch_name), force)
            .await?;
        Ok(())
    }

    async fn rebase_onto(
        &self,
        worktree_path: &Path,
        onto_branch: &str,
    ) -> Result<(), WorktreeError> {
        let git = ctx(worktree_path)?;
        let outcome = git.rebase(&Rev::new(onto_branch)).await?;

        match outcome {
            ship_git::RebaseOutcome::Success => Ok(()),
            ship_git::RebaseOutcome::Conflict { .. } => {
                // Abort the failed rebase so the worktree is left clean.
                git.rebase_abort_quiet().await;
                Err(WorktreeError {
                    message: format!("rebase onto {onto_branch} failed"),
                })
            }
        }
    }

    async fn rebase_onto_conflict_ok(
        &self,
        worktree_path: &Path,
        onto_branch: &str,
    ) -> Result<RebaseOutcome, WorktreeError> {
        let git = ctx(worktree_path)?;
        let outcome = git.rebase(&Rev::new(onto_branch)).await?;

        match outcome {
            ship_git::RebaseOutcome::Success => Ok(RebaseOutcome::Clean),
            ship_git::RebaseOutcome::Conflict { conflicting_files } => {
                let files: Vec<String> =
                    conflicting_files.into_iter().map(|p| p.into_string()).collect();
                if files.is_empty() {
                    // No conflict markers — something else went wrong. Abort and surface the error.
                    git.rebase_abort_quiet().await;
                    Err(WorktreeError {
                        message: format!("rebase onto {onto_branch} failed"),
                    })
                } else {
                    Ok(RebaseOutcome::Conflict { files })
                }
            }
        }
    }

    async fn rebase_continue(&self, worktree_path: &Path) -> Result<RebaseOutcome, WorktreeError> {
        let git = ctx(worktree_path)?;

        // Stage first so that resolved files are no longer marked as unmerged.
        git.add_all().await?;

        // After staging, check for conflict markers that would indicate
        // the user resolved the merge status but left markers in the file.
        let marker_locations = self
            .tracked_conflict_marker_locations(worktree_path)
            .await?;
        if !marker_locations.is_empty() {
            return Err(WorktreeError {
                message: format!(
                    "cannot continue rebase while conflict markers remain in tracked files (path:line):\n{}",
                    marker_locations.join("\n")
                ),
            });
        }

        let outcome = git.rebase_continue().await?;

        match outcome {
            ship_git::RebaseOutcome::Success => Ok(RebaseOutcome::Clean),
            ship_git::RebaseOutcome::Conflict { conflicting_files } => {
                let files: Vec<String> =
                    conflicting_files.into_iter().map(|p| p.into_string()).collect();
                Ok(RebaseOutcome::Conflict { files })
            }
        }
    }

    async fn rebase_abort(&self, worktree_path: &Path) -> Result<(), WorktreeError> {
        let git = ctx(worktree_path)?;
        git.rebase_abort().await?;
        Ok(())
    }

    async fn reset_to_base(
        &self,
        worktree_path: &Path,
        base_branch: &str,
    ) -> Result<(), WorktreeError> {
        let repo_root = repo_root_for_worktree(worktree_path)?;
        ensure_valid_base_ref(repo_root, base_branch).await?;

        let git = ctx(worktree_path)?;
        git.reset_hard(&Rev::new(base_branch)).await?;
        Ok(())
    }

    async fn merge_ff_only(
        &self,
        repo_root: &Path,
        branch: &str,
        into_branch: &str,
    ) -> Result<(), WorktreeError> {
        let git = ctx(repo_root)?;

        // Check whether into_branch is currently checked out in the repo root.
        // `git fetch .` refuses to update a checked-out branch, so we fall back
        // to `git merge --ff-only` in that case (which works because the branch
        // IS checked out).
        let current_branch = git.branch_name().await?;

        if current_branch.as_str() == into_branch {
            // into_branch is checked out — use merge --ff-only directly.
            git.merge_ff_only(&Rev::new(branch)).await?;
        } else {
            // into_branch is NOT checked out — use `git fetch .` to update the
            // ref directly without needing a checkout.
            git.fetch_local(&Rev::new(branch), &BranchName::new(into_branch))
                .await?;
        }
        Ok(())
    }

    // r[proto.archive-session.safety-check]
    async fn branch_unmerged_commits(
        &self,
        branch_name: &str,
        base_branch: &str,
        repo_root: &Path,
    ) -> Result<Vec<String>, WorktreeError> {
        let git = ctx(repo_root)?;
        let range = format!("{base_branch}..{branch_name}");

        // Branch may not exist (already deleted) — treat as fully merged.
        match git.log_oneline(&range).await {
            Ok(commits) => Ok(commits),
            Err(_) => Ok(Vec::new()),
        }
    }
}

async fn tracked_conflict_marker_details(
    worktree_path: &Path,
) -> Result<Vec<(String, Vec<usize>)>, WorktreeError> {
    let git = ctx(worktree_path)?;
    let files = git.ls_files().await?;

    let mut details = Vec::new();
    for relative in files {
        let file_path = worktree_path.join(relative.as_str());
        let bytes = fs_err::tokio::read(&file_path)
            .await
            .map_err(|error| WorktreeError {
                message: format!("failed to read {}: {error}", file_path.display()),
            })?;
        let marker_lines = conflict_marker_line_numbers(&bytes);
        if !marker_lines.is_empty() {
            details.push((relative.into_string(), marker_lines));
        }
    }
    Ok(details)
}

fn conflict_marker_line_numbers(bytes: &[u8]) -> Vec<usize> {
    bytes
        .split(|&byte| byte == b'\n')
        .enumerate()
        .filter_map(|(index, line)| {
            let line = line.strip_suffix(b"\r").unwrap_or(line);
            if line.starts_with(&repeated_byte(b'<'))
                || line.starts_with(&repeated_byte(b'='))
                || line.starts_with(&repeated_byte(b'>'))
            {
                Some(index + 1)
            } else {
                None
            }
        })
        .collect()
}

fn repeated_byte(byte: u8) -> [u8; 7] {
    [byte; 7]
}

fn repo_root_for_worktree(path: &Path) -> Result<&Path, WorktreeError> {
    let mut current = path;
    loop {
        let parent = current.parent().ok_or_else(|| WorktreeError {
            message: format!("invalid worktree path: {}", path.display()),
        })?;
        if parent.file_name().and_then(|n| n.to_str()) == Some(".ship") {
            return parent.parent().ok_or_else(|| WorktreeError {
                message: format!("invalid worktree path: {}", path.display()),
            });
        }
        current = parent;
        if path
            .components()
            .count()
            .saturating_sub(current.components().count())
            > 3
        {
            return Err(WorktreeError {
                message: format!("invalid worktree path: {}", path.display()),
            });
        }
    }
}

async fn ensure_valid_base_ref(repo_root: &Path, base_branch: &str) -> Result<(), WorktreeError> {
    let git = ctx(repo_root)?;

    // Check if the base ref resolves to a commit
    if git.ref_exists(&Rev::new(base_branch)).await? {
        tracing::debug!(
            base_branch = %base_branch,
            repo_root = %repo_root.display(),
            "resolved base ref to a commit"
        );
        return Ok(());
    }

    // Base ref doesn't resolve — check if it's an unborn branch
    let head_branch = match git.branch_name().await {
        Ok(b) => Some(b.into_string()),
        Err(_) => None,
    };

    let head_exists = git.ref_exists(&Rev::new("HEAD")).await?;

    if head_branch.as_deref() == Some(base_branch) && !head_exists {
        tracing::warn!(
            base_branch = %base_branch,
            repo_root = %repo_root.display(),
            "base branch is unborn"
        );
        return Err(WorktreeError {
            message: format!(
                "base branch '{base_branch}' is unborn: the repository has no commits on that branch yet"
            ),
        });
    }

    tracing::warn!(
        base_branch = %base_branch,
        repo_root = %repo_root.display(),
        "base branch/ref does not resolve to a commit"
    );
    Err(WorktreeError {
        message: format!("base branch/ref '{base_branch}' does not resolve to a commit"),
    })
}
