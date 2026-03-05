use std::path::{Path, PathBuf};
use std::process::Output;

use ship_types::SessionId;
use tokio::process::Command;

use crate::{WorktreeError, WorktreeOps};

// r[testability.git-trait]
#[derive(Debug, Default, Clone, Copy)]
pub struct GitWorktreeOps;

// r[backend.git-shell]
impl WorktreeOps for GitWorktreeOps {
    async fn create_worktree(
        &self,
        session_id: &SessionId,
        base_branch: &str,
        slug: &str,
        repo_root: &Path,
    ) -> Result<PathBuf, WorktreeError> {
        let short_session_id: String = session_id.0.chars().take(8).collect();
        let branch_name = format!("ship/{short_session_id}/{slug}");
        let worktree_path = repo_root
            .join(".worktrees")
            .join(format!("{short_session_id}-{slug}"));

        fs_err::tokio::create_dir_all(repo_root.join(".worktrees"))
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
            .arg(&branch_name)
            .arg(&worktree_path)
            .arg(base_branch)
            .output()
            .await
            .map_err(|error| WorktreeError {
                message: error.to_string(),
            })?;

        ensure_success(output)?;
        Ok(worktree_path)
    }

    async fn remove_worktree(&self, path: &Path, force: bool) -> Result<(), WorktreeError> {
        let repo_root = path
            .parent()
            .and_then(|parent| parent.parent())
            .ok_or_else(|| WorktreeError {
                message: format!("invalid worktree path: {}", path.display()),
            })?;

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

fn parse_branch_lines(stdout: &str) -> Vec<String> {
    stdout
        .lines()
        .map(|line| line.trim_start_matches(['*', '+', ' ']))
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
        .collect()
}
