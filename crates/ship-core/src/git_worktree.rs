use std::path::{Path, PathBuf};
use std::process::Output;

use tokio::process::Command;

use crate::{RebaseOutcome, WorktreeError, WorktreeOps};

// r[testability.git-trait]
#[derive(Debug, Default, Clone, Copy)]
pub struct GitWorktreeOps;

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

    async fn current_branch(&self, worktree_path: &Path) -> Result<String, WorktreeError> {
        let output = Command::new("git")
            .arg("-C")
            .arg(worktree_path)
            .args(["rev-parse", "--abbrev-ref", "HEAD"])
            .output()
            .await
            .map_err(|error| WorktreeError {
                message: error.to_string(),
            })?;

        Ok(ensure_success_stdout(output)?.trim().to_owned())
    }

    async fn is_rebase_in_progress(&self, worktree_path: &Path) -> Result<bool, WorktreeError> {
        Ok(git_path_exists(worktree_path, "rebase-merge").await?
            || git_path_exists(worktree_path, "rebase-apply").await?)
    }

    async fn unmerged_paths(&self, worktree_path: &Path) -> Result<Vec<String>, WorktreeError> {
        git_diff_filter_paths(worktree_path, "U").await
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
        let output = Command::new("git")
            .arg("-C")
            .arg(worktree_path)
            .arg("diff")
            .arg(format!("{base_branch}..HEAD"))
            .output()
            .await
            .map_err(|error| WorktreeError {
                message: error.to_string(),
            })?;

        ensure_success_stdout(output)
    }

    async fn commit_all(&self, worktree_path: &Path, message: &str) -> Result<(), WorktreeError> {
        let add_output = Command::new("git")
            .arg("-C")
            .arg(worktree_path)
            .arg("add")
            .arg("-A")
            .output()
            .await
            .map_err(|e| WorktreeError {
                message: e.to_string(),
            })?;
        ensure_success(add_output)?;

        let commit_output = Command::new("git")
            .arg("-C")
            .arg(worktree_path)
            .arg("commit")
            .arg("-m")
            .arg(message)
            .output()
            .await
            .map_err(|e| WorktreeError {
                message: e.to_string(),
            })?;
        ensure_success(commit_output)
    }

    async fn list_branches(&self, repo_root: &Path) -> Result<Vec<String>, WorktreeError> {
        let output = Command::new("git")
            .arg("-C")
            .arg(repo_root)
            .arg("branch")
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

    async fn rebase_onto_conflict_ok(
        &self,
        worktree_path: &Path,
        onto_branch: &str,
    ) -> Result<RebaseOutcome, WorktreeError> {
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
            return Ok(RebaseOutcome::Clean);
        }

        // Check for actual merge conflicts vs unexpected git failure.
        let conflict_output = Command::new("git")
            .arg("-C")
            .arg(worktree_path)
            .arg("diff")
            .arg("--name-only")
            .arg("--diff-filter=U")
            .output()
            .await
            .map_err(|error| WorktreeError {
                message: error.to_string(),
            })?;

        let conflict_files: Vec<String> = String::from_utf8_lossy(&conflict_output.stdout)
            .lines()
            .filter(|l| !l.is_empty())
            .map(|l| l.to_owned())
            .collect();

        if !conflict_files.is_empty() {
            return Ok(RebaseOutcome::Conflict {
                files: conflict_files,
            });
        }

        // No conflict markers — something else went wrong. Abort and surface the error.
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

    async fn rebase_continue(&self, worktree_path: &Path) -> Result<RebaseOutcome, WorktreeError> {
        // Stage first so that resolved files are no longer marked as unmerged.
        // Previously we checked unmerged_paths before staging, which blocked
        // continue_rebase even after the user had resolved all conflicts.
        let add_output = Command::new("git")
            .arg("-C")
            .arg(worktree_path)
            .arg("add")
            .arg("-A")
            .output()
            .await
            .map_err(|error| WorktreeError {
                message: error.to_string(),
            })?;

        if !add_output.status.success() {
            let stderr = String::from_utf8_lossy(&add_output.stderr);
            return Err(WorktreeError {
                message: format!("git add -A failed: {}", stderr.trim()),
            });
        }

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

        let continue_output = Command::new("git")
            .arg("-C")
            .arg(worktree_path)
            .arg("rebase")
            .arg("--continue")
            .env("GIT_EDITOR", "true")
            .output()
            .await
            .map_err(|error| WorktreeError {
                message: error.to_string(),
            })?;

        if continue_output.status.success() {
            return Ok(RebaseOutcome::Clean);
        }

        let conflict_files = self.unmerged_paths(worktree_path).await?;
        if !conflict_files.is_empty() {
            return Ok(RebaseOutcome::Conflict {
                files: conflict_files,
            });
        }

        let stderr = String::from_utf8_lossy(&continue_output.stderr);
        Err(WorktreeError {
            message: format!("rebase --continue failed: {}", stderr.trim()),
        })
    }

    async fn rebase_abort(&self, worktree_path: &Path) -> Result<(), WorktreeError> {
        let output = Command::new("git")
            .arg("-C")
            .arg(worktree_path)
            .arg("rebase")
            .arg("--abort")
            .output()
            .await
            .map_err(|error| WorktreeError {
                message: error.to_string(),
            })?;

        ensure_success(output)
    }

    async fn reset_to_base(
        &self,
        worktree_path: &Path,
        base_branch: &str,
    ) -> Result<(), WorktreeError> {
        let repo_root = repo_root_for_worktree(worktree_path)?;
        ensure_valid_base_ref(repo_root, base_branch).await?;

        let output = Command::new("git")
            .arg("-C")
            .arg(worktree_path)
            .arg("reset")
            .arg("--hard")
            .arg(base_branch)
            .output()
            .await
            .map_err(|error| WorktreeError {
                message: error.to_string(),
            })?;

        ensure_success(output)
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

    // r[proto.archive-session.safety-check]
    async fn branch_unmerged_commits(
        &self,
        branch_name: &str,
        base_branch: &str,
        repo_root: &Path,
    ) -> Result<Vec<String>, WorktreeError> {
        let range = format!("{base_branch}..{branch_name}");
        let output = Command::new("git")
            .arg("-C")
            .arg(repo_root)
            .arg("log")
            .arg("--oneline")
            .arg(&range)
            .output()
            .await
            .map_err(|error| WorktreeError {
                message: error.to_string(),
            })?;

        if !output.status.success() {
            // Branch may not exist (already deleted) — treat as fully merged.
            return Ok(Vec::new());
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let commits: Vec<String> = stdout
            .lines()
            .filter(|l| !l.is_empty())
            .map(str::to_owned)
            .collect();
        Ok(commits)
    }
}

async fn git_diff_filter_paths(
    worktree_path: &Path,
    diff_filter: &str,
) -> Result<Vec<String>, WorktreeError> {
    let output = Command::new("git")
        .arg("-C")
        .arg(worktree_path)
        .arg("diff")
        .arg("--name-only")
        .arg(format!("--diff-filter={diff_filter}"))
        .output()
        .await
        .map_err(|error| WorktreeError {
            message: error.to_string(),
        })?;

    Ok(ensure_success_stdout(output)?
        .lines()
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
        .collect())
}

async fn git_path_exists(worktree_path: &Path, suffix: &str) -> Result<bool, WorktreeError> {
    let output = Command::new("git")
        .arg("-C")
        .arg(worktree_path)
        .args(["rev-parse", "--git-path", suffix])
        .output()
        .await
        .map_err(|error| WorktreeError {
            message: error.to_string(),
        })?;

    let git_path = ensure_success_stdout(output)?;
    Ok(fs_err::tokio::metadata(git_path.trim()).await.is_ok())
}

async fn tracked_conflict_marker_details(
    worktree_path: &Path,
) -> Result<Vec<(String, Vec<usize>)>, WorktreeError> {
    let output = Command::new("git")
        .arg("-C")
        .arg(worktree_path)
        .args(["ls-files", "-z"])
        .output()
        .await
        .map_err(|error| WorktreeError {
            message: error.to_string(),
        })?;

    let stdout = ensure_success_stdout(output)?;
    let mut details = Vec::new();
    for relative in stdout.split('\0').filter(|path| !path.is_empty()) {
        let file_path = worktree_path.join(relative);
        let bytes = fs_err::tokio::read(&file_path)
            .await
            .map_err(|error| WorktreeError {
                message: format!("failed to read {}: {error}", file_path.display()),
            })?;
        let marker_lines = conflict_marker_line_numbers(&bytes);
        if !marker_lines.is_empty() {
            details.push((relative.to_owned(), marker_lines));
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
