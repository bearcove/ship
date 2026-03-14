use std::fmt;
use std::path::{Path, PathBuf};
use std::time::Duration;

use globset::{Glob, GlobSetBuilder};
use ship_types::HookDef;
use tokio::process::Command;
use tokio::task::JoinSet;
use tokio::time::timeout;

const DEFAULT_HOOK_TIMEOUT: Duration = Duration::from_secs(5 * 60);

/// The outcome of running a single hook.
#[derive(Debug, Clone)]
pub struct HookOutcome {
    pub name: String,
    pub command: String,
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
    pub exit_code: Option<i32>,
}

impl HookOutcome {
    pub fn failed(&self) -> bool {
        !self.success
    }
}

/// Error returned when one or more hooks fail. Contains all outcomes so the
/// caller can surface every failure at once rather than stopping at the first.
#[derive(Debug)]
pub struct HooksRunError {
    pub outcomes: Vec<HookOutcome>,
}

impl fmt::Display for HooksRunError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let failed: Vec<&HookOutcome> = self.outcomes.iter().filter(|o| o.failed()).collect();
        write!(f, "{} hook(s) failed:", failed.len())?;
        for outcome in &failed {
            write!(
                f,
                "\n\nhook: {}\ncommand: {}",
                outcome.name, outcome.command
            )?;
            if let Some(code) = outcome.exit_code {
                write!(f, "\nexit code: {code}")?;
            }
            if !outcome.stdout.trim().is_empty() {
                write!(f, "\nstdout:\n{}", indent(outcome.stdout.trim()))?;
            }
            if !outcome.stderr.trim().is_empty() {
                write!(f, "\nstderr:\n{}", indent(outcome.stderr.trim()))?;
            }
        }
        Ok(())
    }
}

impl std::error::Error for HooksRunError {}

fn indent(s: &str) -> String {
    s.lines()
        .map(|l| format!("  {l}"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Run all hooks in parallel. All hooks run to completion regardless of
/// individual failures. Returns `Ok` with all outcomes when every hook
/// succeeds, or `Err` with all outcomes when any hook fails.
///
/// Hooks with a `glob` list are skipped if none of the worktree's changed
/// files match any of the patterns.
pub async fn run_hooks(
    hooks: &[HookDef],
    worktree_root: &Path,
) -> Result<Vec<HookOutcome>, HooksRunError> {
    if hooks.is_empty() {
        return Ok(Vec::new());
    }

    // Only pay the cost of a git invocation if at least one hook has globs.
    let changed_files: Option<Vec<String>> = if hooks.iter().any(|h| !h.glob.is_empty()) {
        Some(collect_changed_files(worktree_root).await)
    } else {
        None
    };

    let mut join_set = JoinSet::new();

    for hook in hooks {
        if !hook.glob.is_empty() {
            if let Some(ref files) = changed_files {
                if !any_file_matches(&hook.glob, files) {
                    // No changed file matches — skip this hook entirely.
                    continue;
                }
            }
        }

        let name = hook.name.clone();
        let command = hook.command.clone();
        let cwd = worktree_root.join(hook.cwd.as_deref().unwrap_or("."));
        join_set.spawn(run_single_hook(name, command, cwd));
    }

    let mut outcomes = Vec::new();
    while let Some(result) = join_set.join_next().await {
        match result {
            Ok(outcome) => outcomes.push(outcome),
            Err(join_err) => {
                // A task panicked or was cancelled — treat as failure.
                outcomes.push(HookOutcome {
                    name: "<unknown>".to_owned(),
                    command: String::new(),
                    success: false,
                    stdout: String::new(),
                    stderr: format!("hook task failed to join: {join_err}"),
                    exit_code: None,
                });
            }
        }
    }

    // Sort by name so output is deterministic regardless of completion order.
    outcomes.sort_by(|a, b| a.name.cmp(&b.name));

    if outcomes.iter().any(|o| o.failed()) {
        Err(HooksRunError { outcomes })
    } else {
        Ok(outcomes)
    }
}

/// Returns the list of files that differ from HEAD in the worktree.
/// Falls back to an empty list if git is unavailable or there is no HEAD yet,
/// which causes glob-filtered hooks to be skipped rather than incorrectly run.
async fn collect_changed_files(worktree_root: &Path) -> Vec<String> {
    let output = Command::new("git")
        .args(["diff", "--name-only", "HEAD"])
        .current_dir(worktree_root)
        .output()
        .await;

    match output {
        Ok(out) if out.status.success() => String::from_utf8_lossy(&out.stdout)
            .lines()
            .filter(|l| !l.is_empty())
            .map(|l| l.to_owned())
            .collect(),
        // No HEAD yet (empty repo) or any other git error — run all hooks.
        _ => Vec::new(),
    }
}

/// Returns true if any file in `files` matches at least one glob in `patterns`.
/// Invalid glob patterns are silently ignored (they never match).
fn any_file_matches(patterns: &[String], files: &[String]) -> bool {
    if files.is_empty() {
        // No changed files recorded — don't skip (e.g. fresh worktree).
        return true;
    }

    let mut builder = GlobSetBuilder::new();
    let mut any_valid = false;
    for pattern in patterns {
        if let Ok(glob) = Glob::new(pattern) {
            builder.add(glob);
            any_valid = true;
        }
    }

    if !any_valid {
        return true;
    }

    let Ok(globset) = builder.build() else {
        return true;
    };

    files.iter().any(|f| globset.is_match(f))
}

async fn run_single_hook(name: String, command: String, cwd: PathBuf) -> HookOutcome {
    let result = timeout(
        DEFAULT_HOOK_TIMEOUT,
        Command::new("sh")
            .args(["-c", &command])
            .current_dir(&cwd)
            .output(),
    )
    .await;

    match result {
        Err(_elapsed) => HookOutcome {
            name,
            command,
            success: false,
            stdout: String::new(),
            stderr: format!("hook timed out after {}s", DEFAULT_HOOK_TIMEOUT.as_secs()),
            exit_code: None,
        },
        Ok(Err(io_err)) => HookOutcome {
            name,
            command,
            success: false,
            stdout: String::new(),
            stderr: format!("failed to spawn hook process: {io_err}"),
            exit_code: None,
        },
        Ok(Ok(output)) => HookOutcome {
            name,
            command,
            success: output.status.success(),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            exit_code: output.status.code(),
        },
    }
}
