use std::path::Path;

use crate::{AgentRole, TaskPhase};

// ── Op classification ───────────────────────────────────────────────

/// Categories of code-tool operations. Mirrors ship-code's Op variants
/// but without any dependency on that crate.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, facet::Facet)]
pub enum OpKind {
    // Read-only
    Search,
    Read,
    ReadNode,
    // Mutations
    Edit,
    ReplaceNode,
    DeleteNode,
    Write,
    Run,
    Commit,
    Undo,
    // Communication
    Message,
    Submit,
}

impl OpKind {
    pub fn is_read_only(self) -> bool {
        matches!(self, Self::Search | Self::Read | Self::ReadNode)
    }

    pub fn is_mutation(self) -> bool {
        matches!(
            self,
            Self::Edit
                | Self::ReplaceNode
                | Self::DeleteNode
                | Self::Write
                | Self::Run
                | Self::Commit
                | Self::Undo
        )
    }
}

// ── Code policy ─────────────────────────────────────────────────────

/// Which code-tool operations are allowed for a given role + phase.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodePolicy {
    pub allowed_ops: &'static [OpKind],
}

/// Read-only ops available to everyone.
const READ_OPS: &[OpKind] = &[OpKind::Search, OpKind::Read, OpKind::ReadNode];

/// All ops except Submit (captain/mate working set).
const ALL_OPS_NO_SUBMIT: &[OpKind] = &[
    OpKind::Search,
    OpKind::Read,
    OpKind::ReadNode,
    OpKind::Edit,
    OpKind::ReplaceNode,
    OpKind::DeleteNode,
    OpKind::Write,
    OpKind::Run,
    OpKind::Commit,
    OpKind::Undo,
    OpKind::Message,
];

/// Full mate working set (includes Submit).
const MATE_WORKING_OPS: &[OpKind] = &[
    OpKind::Search,
    OpKind::Read,
    OpKind::ReadNode,
    OpKind::Edit,
    OpKind::ReplaceNode,
    OpKind::DeleteNode,
    OpKind::Write,
    OpKind::Run,
    OpKind::Commit,
    OpKind::Undo,
    OpKind::Message,
    OpKind::Submit,
];

/// Read + Message only (for mate in Assigned before starting work).
const READ_PLUS_MESSAGE: &[OpKind] = &[
    OpKind::Search,
    OpKind::Read,
    OpKind::ReadNode,
    OpKind::Message,
];

/// What code ops are allowed for this role and task phase?
pub fn code_policy(role: AgentRole, phase: Option<TaskPhase>) -> CodePolicy {
    let allowed_ops = match role {
        AgentRole::Captain => match phase {
            // Captain can always read. Gets full write only during RebaseConflict.
            Some(TaskPhase::RebaseConflict) => ALL_OPS_NO_SUBMIT,
            _ => READ_OPS,
        },
        AgentRole::Mate => match phase {
            Some(TaskPhase::Working) => MATE_WORKING_OPS,
            Some(TaskPhase::Assigned) => READ_PLUS_MESSAGE,
            _ => &[],
        },
        // Admiral doesn't use the code tool — has read_file and run_command directly.
        AgentRole::Admiral => &[],
    };
    CodePolicy { allowed_ops }
}

// ── Run policy ──────────────────────────────────────────────────────

/// Sandbox policy for shell command execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunPolicy {
    /// Can commands write to the worktree?
    pub worktree_writable: bool,
    /// Additional paths granted write access (dynamic exceptions).
    pub extra_write_paths: Vec<String>,
}

/// What run-command sandbox applies for this role and phase?
pub fn run_policy(role: AgentRole, phase: Option<TaskPhase>) -> RunPolicy {
    match role {
        AgentRole::Mate => RunPolicy {
            // Mate can write to worktree while working.
            worktree_writable: matches!(phase, Some(TaskPhase::Working)),
            extra_write_paths: vec![],
        },
        AgentRole::Captain => RunPolicy {
            // Captain can write only during rebase conflict resolution.
            worktree_writable: matches!(phase, Some(TaskPhase::RebaseConflict)),
            extra_write_paths: vec![],
        },
        AgentRole::Admiral => RunPolicy {
            // Admiral has no worktree.
            worktree_writable: false,
            extra_write_paths: vec![],
        },
    }
}

// ── Command nudges ──────────────────────────────────────────────────

/// A nudge is a non-blocking suggestion appended to command output.
/// It doesn't prevent execution — it teaches the agent about the workflow.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandNudge {
    /// What we think the agent is trying to do.
    pub intent: &'static str,
    /// The workflow tool they should use instead.
    pub suggestion: String,
}

/// Check if a command would benefit from a workflow nudge.
/// Returns None if the command doesn't match any known patterns.
///
/// This is NOT access control. The command still runs. This is UX:
/// "I see you're trying to do X — here's how that works in Ship."
pub fn command_nudge(command: &str, role: AgentRole, phase: Option<TaskPhase>) -> Option<CommandNudge> {
    let normalized = command.trim().to_ascii_lowercase();
    let parts: Vec<&str> = normalized.split_whitespace().collect();
    let program = parts.first().copied()?;

    if program != "git" {
        return None;
    }

    let subcommand = parts.get(1).copied().unwrap_or("");

    match (role, subcommand) {
        // Captain trying to see the diff — point them to captain_review_diff.
        (AgentRole::Captain, "diff") => Some(CommandNudge {
            intent: "view changes",
            suggestion: match phase {
                Some(TaskPhase::PendingReview) => {
                    "Use `captain_review_diff` to see the mate's work against the base branch. \
                     Raw `git diff` in the worktree may show unexpected results because of \
                     shadow commits."
                        .into()
                }
                _ => {
                    "Use `captain_review_diff` to see changes against the base branch. \
                     It handles shadow commits and shows what you actually want."
                        .into()
                }
            },
        }),

        // Captain trying git status — point them to captain_git_status.
        (AgentRole::Captain, "status") => Some(CommandNudge {
            intent: "check repository state",
            suggestion: "Use `captain_git_status` — it shows branch info, dirty state, \
                         rebase status, and conflict markers in a structured way."
                .into(),
        }),

        // Captain trying to commit — explain the shadow commit system.
        (AgentRole::Captain, "commit") => Some(CommandNudge {
            intent: "create a commit",
            suggestion: "Ship uses shadow commits internally. Use the `code` tool's `commit` \
                         operation to squash shadow commits into a real commit with your message."
                .into(),
        }),

        // Captain trying to rebase manually — there's a workflow for this.
        (AgentRole::Captain, "rebase") => Some(CommandNudge {
            intent: "rebase the branch",
            suggestion: match phase {
                Some(TaskPhase::RebaseConflict) => {
                    "Use `captain_continue_rebase` after resolving conflicts, \
                     or `captain_abort_rebase` to abandon. These handle the \
                     Ship state machine transitions."
                        .into()
                }
                _ => {
                    "Rebasing happens automatically during `captain_merge`. \
                     If conflicts arise, the task moves to RebaseConflict phase \
                     where you can resolve them."
                        .into()
                }
            },
        }),

        // Captain trying to merge — there's a workflow for this.
        (AgentRole::Captain, "merge") => Some(CommandNudge {
            intent: "merge the branch",
            suggestion: "Use `captain_merge` — it rebases onto the base branch, \
                         runs checks, and fast-forward merges. Manual `git merge` \
                         would bypass the workflow."
                .into(),
        }),

        // Captain trying to push — Ship handles this.
        (AgentRole::Captain, "push") => Some(CommandNudge {
            intent: "push changes",
            suggestion: "Ship handles pushing as part of `captain_merge`. \
                         You don't need to push manually."
                .into(),
        }),

        // Captain trying git log — that's fine, but let them know about review_diff.
        (AgentRole::Captain, "log") => Some(CommandNudge {
            intent: "view commit history",
            suggestion: "This works, but note that shadow commits may appear in the log. \
                         `captain_review_diff` shows the clean diff against the base branch."
                .into(),
        }),

        // Mate trying git anything — the captain owns git.
        (AgentRole::Mate, _) => Some(CommandNudge {
            intent: "use git",
            suggestion: "Git operations are managed by the captain. \
                         Use `mate_ask_captain` if you need git information, \
                         or use the `code` tool's `commit` operation to save your work."
                .into(),
        }),

        // Admiral trying git — they have no worktree.
        (AgentRole::Admiral, _) => Some(CommandNudge {
            intent: "use git",
            suggestion: "You don't have a worktree. Use `admiral_list_lanes` to see \
                         session status, or steer a captain if you need git information."
                .into(),
        }),

        // Captain running other git commands — no specific nudge.
        _ => None,
    }
}

// ── Sandbox profile generation ──────────────────────────────────────

/// Environment paths needed for sandbox profile generation.
pub struct SandboxEnv<'a> {
    pub home: &'a str,
    pub tmpdir: &'a str,
}

/// Generate a macOS sandbox-exec profile for the given run policy.
///
/// The profile starts with `(allow default)` then denies all writes globally,
/// then re-allows writes to specific paths based on the policy.
///
/// This is a pure function — the executor just passes the result to sandbox-exec.
pub fn sandbox_profile(
    policy: &RunPolicy,
    worktree: &Path,
    env: &SandboxEnv<'_>,
) -> String {
    let mut profile = String::from("(version 1)\n(allow default)\n(deny file-write* (subpath \"/\"))");

    // Worktree write access (if policy allows).
    if policy.worktree_writable {
        let wt = worktree.to_string_lossy();
        profile.push_str(&format!("\n(allow file-write* (subpath \"{wt}\"))"));
    }

    // System temp directories — always writable.
    profile.push_str("\n(allow file-write* (subpath \"/private/tmp\"))");
    profile.push_str("\n(allow file-write* (subpath \"/tmp\"))");
    profile.push_str("\n(allow file-write* (subpath \"/var/folders\"))");
    profile.push_str("\n(allow file-write* (subpath \"/private/var/folders\"))");
    profile.push_str(&format!(
        "\n(allow file-write* (subpath \"{}\"))",
        env.tmpdir
    ));

    // Package manager caches — always writable.
    profile.push_str(&format!(
        "\n(allow file-write* (subpath \"{}/Library/Caches\"))",
        env.home
    ));
    profile.push_str(&format!(
        "\n(allow file-write* (subpath \"{}/Library/pnpm\"))",
        env.home
    ));
    profile.push_str(&format!(
        "\n(allow file-write* (subpath \"{}/.npm\"))",
        env.home
    ));
    profile.push_str(&format!(
        "\n(allow file-write* (subpath \"{}/.pnpm-store\"))",
        env.home
    ));

    // /dev/null — always writable.
    profile.push_str("\n(allow file-write* (literal \"/dev/null\"))");

    // Dynamic exceptions (granted at runtime by human via captain request).
    for path in &policy.extra_write_paths {
        profile.push_str(&format!("\n(allow file-write* (subpath \"{path}\"))"));
    }

    profile
}

// ── Combined policy ─────────────────────────────────────────────────

/// Full sandbox policy for a role + phase. Combines code-op gating and
/// run-command sandboxing into a single queryable value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SandboxPolicy {
    pub code: CodePolicy,
    pub run: RunPolicy,
}

/// Get the full sandbox policy for a role and task phase.
pub fn sandbox_policy(role: AgentRole, phase: Option<TaskPhase>) -> SandboxPolicy {
    SandboxPolicy {
        code: code_policy(role, phase),
        run: run_policy(role, phase),
    }
}

/// Check if a specific op is allowed by the code policy.
pub fn is_op_allowed(policy: &CodePolicy, op: OpKind) -> bool {
    policy.allowed_ops.contains(&op)
}

/// Explain why an op was denied (for error messages back to the agent).
pub fn op_denied_reason(role: AgentRole, phase: Option<TaskPhase>, op: OpKind) -> String {
    let phase_desc = match phase {
        Some(p) => format!("{p:?}"),
        None => "no active task".to_string(),
    };
    let role_desc = format!("{role:?}");

    if op.is_mutation() && role == AgentRole::Captain {
        match phase {
            Some(TaskPhase::RebaseConflict) => {
                // Shouldn't happen — captain has full access during rebase conflict.
                format!("`{op:?}` should be available during RebaseConflict. This is a bug.")
            }
            _ => {
                format!(
                    "Write operations are not available to {role_desc} during {phase_desc}. \
                     The mate handles implementation — use captain_assign to delegate work, \
                     or captain_steer to redirect an active task."
                )
            }
        }
    } else if op == OpKind::Submit && role != AgentRole::Mate {
        format!("Only the mate can submit work for review.")
    } else {
        format!("`{op:?}` is not available to {role_desc} during {phase_desc}.")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Code policy tests ───────────────────────────────────────────

    #[test]
    fn captain_no_task_is_read_only() {
        let policy = code_policy(AgentRole::Captain, None);
        assert!(is_op_allowed(&policy, OpKind::Search));
        assert!(is_op_allowed(&policy, OpKind::Read));
        assert!(is_op_allowed(&policy, OpKind::ReadNode));
        assert!(!is_op_allowed(&policy, OpKind::Edit));
        assert!(!is_op_allowed(&policy, OpKind::Write));
        assert!(!is_op_allowed(&policy, OpKind::Run));
        assert!(!is_op_allowed(&policy, OpKind::Commit));
    }

    #[test]
    fn captain_working_is_read_only() {
        let policy = code_policy(AgentRole::Captain, Some(TaskPhase::Working));
        assert!(is_op_allowed(&policy, OpKind::Search));
        assert!(!is_op_allowed(&policy, OpKind::Edit));
        assert!(!is_op_allowed(&policy, OpKind::Run));
    }

    #[test]
    fn captain_review_pending_is_read_only() {
        let policy = code_policy(AgentRole::Captain, Some(TaskPhase::PendingReview));
        assert!(is_op_allowed(&policy, OpKind::Read));
        assert!(!is_op_allowed(&policy, OpKind::Edit));
    }

    #[test]
    fn captain_rebase_conflict_has_full_access() {
        let policy = code_policy(AgentRole::Captain, Some(TaskPhase::RebaseConflict));
        assert!(is_op_allowed(&policy, OpKind::Search));
        assert!(is_op_allowed(&policy, OpKind::Edit));
        assert!(is_op_allowed(&policy, OpKind::ReplaceNode));
        assert!(is_op_allowed(&policy, OpKind::Write));
        assert!(is_op_allowed(&policy, OpKind::Run));
        assert!(is_op_allowed(&policy, OpKind::Commit));
        assert!(is_op_allowed(&policy, OpKind::Undo));
        // But NOT submit — that's mate-only.
        assert!(!is_op_allowed(&policy, OpKind::Submit));
    }

    #[test]
    fn mate_working_has_everything() {
        let policy = code_policy(AgentRole::Mate, Some(TaskPhase::Working));
        assert!(is_op_allowed(&policy, OpKind::Search));
        assert!(is_op_allowed(&policy, OpKind::Edit));
        assert!(is_op_allowed(&policy, OpKind::Run));
        assert!(is_op_allowed(&policy, OpKind::Commit));
        assert!(is_op_allowed(&policy, OpKind::Submit));
        assert!(is_op_allowed(&policy, OpKind::Message));
    }

    #[test]
    fn mate_assigned_can_read_and_message() {
        let policy = code_policy(AgentRole::Mate, Some(TaskPhase::Assigned));
        assert!(is_op_allowed(&policy, OpKind::Search));
        assert!(is_op_allowed(&policy, OpKind::Read));
        assert!(is_op_allowed(&policy, OpKind::Message));
        assert!(!is_op_allowed(&policy, OpKind::Edit));
        assert!(!is_op_allowed(&policy, OpKind::Submit));
    }

    #[test]
    fn mate_no_task_has_nothing() {
        let policy = code_policy(AgentRole::Mate, None);
        assert!(!is_op_allowed(&policy, OpKind::Search));
        assert!(!is_op_allowed(&policy, OpKind::Edit));
    }

    #[test]
    fn admiral_has_no_code_ops() {
        let policy = code_policy(AgentRole::Admiral, None);
        assert!(policy.allowed_ops.is_empty());
    }

    // ── Run policy tests ────────────────────────────────────────────

    #[test]
    fn mate_working_can_write_worktree() {
        let policy = run_policy(AgentRole::Mate, Some(TaskPhase::Working));
        assert!(policy.worktree_writable);
    }

    #[test]
    fn mate_assigned_cannot_write_worktree() {
        let policy = run_policy(AgentRole::Mate, Some(TaskPhase::Assigned));
        assert!(!policy.worktree_writable);
    }

    #[test]
    fn captain_working_cannot_write_worktree() {
        let policy = run_policy(AgentRole::Captain, Some(TaskPhase::Working));
        assert!(!policy.worktree_writable);
    }

    #[test]
    fn captain_rebase_conflict_can_write_worktree() {
        let policy = run_policy(AgentRole::Captain, Some(TaskPhase::RebaseConflict));
        assert!(policy.worktree_writable);
    }

    #[test]
    fn admiral_never_writes_worktree() {
        let policy = run_policy(AgentRole::Admiral, None);
        assert!(!policy.worktree_writable);
    }

    // ── Sandbox profile tests ───────────────────────────────────────

    #[test]
    fn profile_without_worktree_write() {
        let policy = RunPolicy {
            worktree_writable: false,
            extra_write_paths: vec![],
        };
        let env = SandboxEnv {
            home: "/Users/test",
            tmpdir: "/var/folders/xx/tmp",
        };
        let profile = sandbox_profile(&policy, Path::new("/work/tree"), &env);
        assert!(profile.contains("(deny file-write* (subpath \"/\"))"));
        assert!(!profile.contains("/work/tree"));
        assert!(profile.contains("/dev/null"));
    }

    #[test]
    fn profile_with_worktree_write() {
        let policy = RunPolicy {
            worktree_writable: true,
            extra_write_paths: vec![],
        };
        let env = SandboxEnv {
            home: "/Users/test",
            tmpdir: "/var/folders/xx/tmp",
        };
        let profile = sandbox_profile(&policy, Path::new("/work/tree"), &env);
        assert!(profile.contains("(allow file-write* (subpath \"/work/tree\"))"));
    }

    #[test]
    fn profile_with_extra_write_paths() {
        let policy = RunPolicy {
            worktree_writable: false,
            extra_write_paths: vec![
                "/Users/test/.cargo/registry".to_string(),
                "/opt/shared-cache".to_string(),
            ],
        };
        let env = SandboxEnv {
            home: "/Users/test",
            tmpdir: "/tmp",
        };
        let profile = sandbox_profile(&policy, Path::new("/work/tree"), &env);
        assert!(profile.contains("(allow file-write* (subpath \"/Users/test/.cargo/registry\"))"));
        assert!(profile.contains("(allow file-write* (subpath \"/opt/shared-cache\"))"));
        // Worktree should NOT be writable.
        assert!(!profile.contains("/work/tree"));
    }

    // ── Snapshot tests ──────────────────────────────────────────────

    #[test]
    fn snapshot_profile_mate_working() {
        let policy = RunPolicy {
            worktree_writable: true,
            extra_write_paths: vec![],
        };
        let env = SandboxEnv {
            home: "/Users/amos",
            tmpdir: "/var/folders/xx/tmp",
        };
        insta::assert_snapshot!(
            "profile_mate_working",
            sandbox_profile(&policy, Path::new("/Users/amos/worktrees/lane-1"), &env)
        );
    }

    #[test]
    fn snapshot_profile_captain_readonly() {
        let policy = RunPolicy {
            worktree_writable: false,
            extra_write_paths: vec![],
        };
        let env = SandboxEnv {
            home: "/Users/amos",
            tmpdir: "/var/folders/xx/tmp",
        };
        insta::assert_snapshot!(
            "profile_captain_readonly",
            sandbox_profile(&policy, Path::new("/Users/amos/worktrees/lane-1"), &env)
        );
    }

    #[test]
    fn snapshot_profile_with_exceptions() {
        let policy = RunPolicy {
            worktree_writable: true,
            extra_write_paths: vec![
                "/Users/amos/.cargo/registry".to_string(),
                "/opt/build-cache".to_string(),
            ],
        };
        let env = SandboxEnv {
            home: "/Users/amos",
            tmpdir: "/var/folders/xx/tmp",
        };
        insta::assert_snapshot!(
            "profile_with_exceptions",
            sandbox_profile(&policy, Path::new("/Users/amos/worktrees/lane-1"), &env)
        );
    }

    #[test]
    fn snapshot_op_denied_captain_working() {
        insta::assert_snapshot!(
            "op_denied_captain_working",
            op_denied_reason(AgentRole::Captain, Some(TaskPhase::Working), OpKind::Edit)
        );
    }

    // ── Combined policy tests ───────────────────────────────────────

    #[test]
    fn combined_captain_working() {
        let policy = sandbox_policy(AgentRole::Captain, Some(TaskPhase::Working));
        // Read-only code ops.
        assert!(is_op_allowed(&policy.code, OpKind::Search));
        assert!(!is_op_allowed(&policy.code, OpKind::Edit));
        // Read-only worktree.
        assert!(!policy.run.worktree_writable);
    }

    #[test]
    fn combined_mate_working() {
        let policy = sandbox_policy(AgentRole::Mate, Some(TaskPhase::Working));
        // Full code ops.
        assert!(is_op_allowed(&policy.code, OpKind::Edit));
        assert!(is_op_allowed(&policy.code, OpKind::Submit));
        // Writable worktree.
        assert!(policy.run.worktree_writable);
    }

    #[test]
    fn combined_captain_rebase() {
        let policy = sandbox_policy(AgentRole::Captain, Some(TaskPhase::RebaseConflict));
        // Full code ops (except submit).
        assert!(is_op_allowed(&policy.code, OpKind::Edit));
        assert!(is_op_allowed(&policy.code, OpKind::Run));
        assert!(!is_op_allowed(&policy.code, OpKind::Submit));
        // Writable worktree.
        assert!(policy.run.worktree_writable);
    }

    // ── Command nudge tests ─────────────────────────────────────────

    #[test]
    fn captain_git_diff_gets_nudge() {
        let nudge = command_nudge("git diff", AgentRole::Captain, Some(TaskPhase::PendingReview));
        assert!(nudge.is_some());
        let nudge = nudge.unwrap();
        assert_eq!(nudge.intent, "view changes");
        assert!(nudge.suggestion.contains("captain_review_diff"));
    }

    #[test]
    fn captain_git_status_gets_nudge() {
        let nudge = command_nudge("git status", AgentRole::Captain, None);
        assert!(nudge.is_some());
        assert!(nudge.unwrap().suggestion.contains("captain_git_status"));
    }

    #[test]
    fn captain_git_commit_gets_nudge() {
        let nudge = command_nudge("git commit -m 'fix'", AgentRole::Captain, Some(TaskPhase::Working));
        assert!(nudge.is_some());
        assert!(nudge.unwrap().suggestion.contains("shadow commits"));
    }

    #[test]
    fn captain_git_rebase_during_conflict_gets_specific_nudge() {
        let nudge = command_nudge("git rebase --continue", AgentRole::Captain, Some(TaskPhase::RebaseConflict));
        assert!(nudge.is_some());
        assert!(nudge.unwrap().suggestion.contains("captain_continue_rebase"));
    }

    #[test]
    fn captain_git_rebase_outside_conflict() {
        let nudge = command_nudge("git rebase main", AgentRole::Captain, Some(TaskPhase::PendingReview));
        assert!(nudge.is_some());
        assert!(nudge.unwrap().suggestion.contains("captain_merge"));
    }

    #[test]
    fn captain_git_merge_gets_nudge() {
        let nudge = command_nudge("git merge main", AgentRole::Captain, Some(TaskPhase::PendingReview));
        assert!(nudge.is_some());
        assert!(nudge.unwrap().suggestion.contains("captain_merge"));
    }

    #[test]
    fn captain_git_push_gets_nudge() {
        let nudge = command_nudge("git push origin main", AgentRole::Captain, None);
        assert!(nudge.is_some());
        assert!(nudge.unwrap().suggestion.contains("captain_merge"));
    }

    #[test]
    fn captain_git_log_gets_nudge() {
        let nudge = command_nudge("git log --oneline", AgentRole::Captain, None);
        assert!(nudge.is_some());
        assert!(nudge.unwrap().suggestion.contains("shadow commits"));
    }

    #[test]
    fn mate_any_git_gets_nudge() {
        for cmd in ["git status", "git diff", "git log", "git stash"] {
            let nudge = command_nudge(cmd, AgentRole::Mate, Some(TaskPhase::Working));
            assert!(nudge.is_some(), "mate should get nudge for: {cmd}");
            assert!(nudge.unwrap().suggestion.contains("captain"));
        }
    }

    #[test]
    fn admiral_any_git_gets_nudge() {
        let nudge = command_nudge("git log", AgentRole::Admiral, None);
        assert!(nudge.is_some());
        assert!(nudge.unwrap().suggestion.contains("worktree"));
    }

    #[test]
    fn non_git_commands_no_nudge() {
        assert!(command_nudge("cargo test", AgentRole::Captain, None).is_none());
        assert!(command_nudge("ls -la", AgentRole::Mate, Some(TaskPhase::Working)).is_none());
        assert!(command_nudge("npm install", AgentRole::Admiral, None).is_none());
    }

    #[test]
    fn empty_command_no_nudge() {
        assert!(command_nudge("", AgentRole::Captain, None).is_none());
        assert!(command_nudge("   ", AgentRole::Captain, None).is_none());
    }

    #[test]
    fn captain_unknown_git_subcommand_no_nudge() {
        // git stash, git bisect, etc. — no specific nudge for captain.
        assert!(command_nudge("git stash", AgentRole::Captain, None).is_none());
        assert!(command_nudge("git bisect start", AgentRole::Captain, None).is_none());
    }
}
