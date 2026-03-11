use std::path::{Path, PathBuf};
use std::process::Output;

use tokio::process::Command;

use crate::{WorktreeError, WorktreeOps};

// r[testability.git-trait]
#[derive(Debug, Default, Clone, Copy)]
pub struct GitWorktreeOps;

// r[backend.git-shell]
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

        let output = Command::new("git")
            .arg("-C")
            .arg(repo_root)
            .arg("worktree")
            .arg("add")
            .arg("-b")
            .arg(branch_name)
            .arg(&worktree_path)
            .arg(base_branch)
            .output()
            .await
            .map_err(|error| WorktreeError {
                message: error.to_string(),
            })?;

        ensure_success(output)?;
        tracing::info!(
            branch_name = %branch_name,
            worktree_path = %worktree_path.display(),
            "created git worktree"
        );
        Ok(worktree_path)
    }

    async fn remove_worktree(&self, path: &Path, force: bool) -> Result<(), WorktreeError> {
        let repo_root = repo_root_for_worktree(path)?;

        let mut command = Command::new("git");
        command
            .arg("-C")
            .arg(repo_root)
            .arg("worktree")
            .arg("remove");
        if force {
            command.arg("--force");
        }
        command.arg(path);

        let output = command.output().await.map_err(|error| WorktreeError {
            message: error.to_string(),
        })?;

        ensure_success(output)
    }

    async fn has_uncommitted_changes(&self, path: &Path) -> Result<bool, WorktreeError> {
        let output = Command::new("git")
            .arg("-C")
            .arg(path)
            .arg("status")
            .arg("--porcelain")
            .output()
            .await
            .map_err(|error| WorktreeError {
                message: error.to_string(),
            })?;

        ensure_success_stdout(output).map(|stdout| !stdout.trim().is_empty())
    }

    async fn list_branches(&self, repo_root: &Path) -> Result<Vec<String>, WorktreeError> {
        let output = Command::new("git")
            .arg("-C")
            .arg(repo_root)
            .arg("branch")
            .arg("-a")
            .output()
            .await
            .map_err(|error| WorktreeError {
                message: error.to_string(),
            })?;

        let stdout = ensure_success_stdout(output)?;
        Ok(parse_branch_lines(&stdout))
    }

    async fn delete_branch(
        &self,
        branch_name: &str,
        force: bool,
        repo_root: &Path,
    ) -> Result<(), WorktreeError> {
        let delete_flag = if force { "-D" } else { "-d" };

        let output = Command::new("git")
            .arg("-C")
            .arg(repo_root)
            .arg("branch")
            .arg(delete_flag)
            .arg(branch_name)
            .output()
            .await
            .map_err(|error| WorktreeError {
                message: error.to_string(),
            })?;

        ensure_success(output)
    }

    async fn rebase_onto(
        &self,
        worktree_path: &Path,
        onto_branch: &str,
    ) -> Result<(), WorktreeError> {
        let output = Command::new("git")
            .arg("-C")
            .arg(worktree_path)
            .arg("rebase")
            .arg(onto_branch)
            .output()
            .await
            .map_err(|error| WorktreeError {
                message: error.to_string(),
            })?;

        if output.status.success() {
            return Ok(());
        }

        // Abort the failed rebase so the worktree is left clean.
        let _ = Command::new("git")
            .arg("-C")
            .arg(worktree_path)
            .arg("rebase")
            .arg("--abort")
            .output()
            .await;

        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(WorktreeError {
            message: format!("rebase onto {onto_branch} failed: {}", stderr.trim()),
        })
    }

    async fn merge_ff_only(&self, repo_root: &Path, branch: &str) -> Result<(), WorktreeError> {
        let output = Command::new("git")
            .arg("-C")
            .arg(repo_root)
            .arg("merge")
            .arg("--ff-only")
            .arg(branch)
            .output()
            .await
            .map_err(|error| WorktreeError {
                message: error.to_string(),
            })?;

        ensure_success(output)
    }
}

fn repo_root_for_worktree(path: &Path) -> Result<&Path, WorktreeError> {
    let worktrees_dir = path.parent().ok_or_else(|| WorktreeError {
        message: format!("invalid worktree path: {}", path.display()),
    })?;
    let ship_dir = worktrees_dir.parent().ok_or_else(|| WorktreeError {
        message: format!("invalid worktree path: {}", path.display()),
    })?;
    let repo_root = ship_dir.parent().ok_or_else(|| WorktreeError {
        message: format!("invalid worktree path: {}", path.display()),
    })?;

    if worktrees_dir.file_name().and_then(|name| name.to_str()) != Some("worktrees")
        || ship_dir.file_name().and_then(|name| name.to_str()) != Some(".ship")
    {
        return Err(WorktreeError {
            message: format!("invalid worktree path: {}", path.display()),
        });
    }

    Ok(repo_root)
}

fn ensure_success(output: Output) -> Result<(), WorktreeError> {
    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    Err(WorktreeError {
        message: stderr.trim().to_owned(),
    })
}

fn ensure_success_stdout(output: Output) -> Result<String, WorktreeError> {
    if output.status.success() {
        return Ok(String::from_utf8_lossy(&output.stdout).into_owned());
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    Err(WorktreeError {
        message: stderr.trim().to_owned(),
    })
}

async fn ensure_valid_base_ref(repo_root: &Path, base_branch: &str) -> Result<(), WorktreeError> {
    let verify_target = format!("{base_branch}^{{commit}}");
    let verify_output = Command::new("git")
        .arg("-C")
        .arg(repo_root)
        .arg("rev-parse")
        .arg("--verify")
        .arg("--quiet")
        .arg(&verify_target)
        .output()
        .await
        .map_err(|error| WorktreeError {
            message: format!("failed to resolve base ref '{base_branch}': {error}"),
        })?;

    if verify_output.status.success() {
        tracing::debug!(
            base_branch = %base_branch,
            repo_root = %repo_root.display(),
            "resolved base ref to a commit"
        );
        return Ok(());
    }

    let head_branch_output = Command::new("git")
        .arg("-C")
        .arg(repo_root)
        .arg("symbolic-ref")
        .arg("--quiet")
        .arg("--short")
        .arg("HEAD")
        .output()
        .await
        .map_err(|error| WorktreeError {
            message: format!("failed to inspect HEAD while resolving '{base_branch}': {error}"),
        })?;

    let head_branch = if head_branch_output.status.success() {
        Some(
            String::from_utf8_lossy(&head_branch_output.stdout)
                .trim()
                .to_owned(),
        )
    } else {
        None
    };

    let head_commit_output = Command::new("git")
        .arg("-C")
        .arg(repo_root)
        .arg("rev-parse")
        .arg("--verify")
        .arg("--quiet")
        .arg("HEAD^{commit}")
        .output()
        .await
        .map_err(|error| WorktreeError {
            message: format!(
                "failed to inspect HEAD commit while resolving '{base_branch}': {error}"
            ),
        })?;

    if head_branch.as_deref() == Some(base_branch) && !head_commit_output.status.success() {
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

fn parse_branch_lines(stdout: &str) -> Vec<String> {
    stdout
        .lines()
        .map(|line| line.trim_start_matches(['*', '+', ' ']))
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
        .collect()
}
