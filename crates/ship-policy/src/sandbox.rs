use std::path::Path;

use crate::{AgentRole, TaskPhase};

// ── Op classification ───────────────────────────────────────────────

/// Categories of code-tool operations. Mirrors ship-code's Op variants
/// but without any dependency on that crate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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

// ── Command blocking ────────────────────────────────────────────────

/// Result of checking a command against the block list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandCheck {
    /// Command is allowed.
    Allowed,
    /// Command is blocked with a reason.
    Blocked(&'static str),
}

/// Check whether a shell command should be blocked.
///
/// This is role-aware: mates can't run git (captain-owned),
/// and everyone is blocked from broad recursive deletes.
pub fn check_command(command: &str, role: AgentRole) -> CommandCheck {
    let normalized = command.trim().to_ascii_lowercase();
    let program = match normalized.split_whitespace().next() {
        Some(p) => p,
        None => return CommandCheck::Allowed,
    };

    // Git is captain-owned — mates can't run it directly.
    if program == "git" && role == AgentRole::Mate {
        return CommandCheck::Blocked(
            "Git commands are captain-owned. Use mate_ask_captain if you need git information.",
        );
    }

    // Prefer modern alternatives.
    if program == "find" {
        return CommandCheck::Blocked("Use fd instead of find. Example: fd -t f 'pattern' path/.");
    }
    if program == "grep" {
        return CommandCheck::Blocked("Use rg instead of grep. Example: rg 'pattern' path/.");
    }

    // Block broad recursive deletes.
    if program == "rm" {
        let has_recursive = normalized.contains(" -r")
            || normalized.contains(" -rf")
            || normalized.contains(" -fr")
            || normalized.contains(" --recursive");
        let has_force = normalized.contains(" -f")
            || normalized.contains(" -rf")
            || normalized.contains(" -fr")
            || normalized.contains(" --force");
        let broad_target = normalized.contains(" *")
            || normalized.ends_with(" .")
            || normalized.contains(" ./")
            || normalized.contains(" /")
            || normalized.contains(" ..")
            || normalized.contains(" ~");

        if has_recursive && has_force && broad_target {
            return CommandCheck::Blocked("Broad recursive delete is not allowed.");
        }
    }

    CommandCheck::Allowed
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
        let policy = code_policy(AgentRole::Captain, Some(TaskPhase::ReviewPending));
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

    // ── Command checking tests ──────────────────────────────────────

    #[test]
    fn mate_blocked_from_git() {
        assert_eq!(
            check_command("git status", AgentRole::Mate),
            CommandCheck::Blocked(
                "Git commands are captain-owned. Use mate_ask_captain if you need git information."
            )
        );
    }

    #[test]
    fn captain_can_run_git() {
        assert_eq!(
            check_command("git status", AgentRole::Captain),
            CommandCheck::Allowed
        );
    }

    #[test]
    fn find_blocked_for_everyone() {
        assert!(matches!(
            check_command("find . -name '*.rs'", AgentRole::Mate),
            CommandCheck::Blocked(_)
        ));
        assert!(matches!(
            check_command("find . -name '*.rs'", AgentRole::Captain),
            CommandCheck::Blocked(_)
        ));
    }

    #[test]
    fn grep_blocked_for_everyone() {
        assert!(matches!(
            check_command("grep -r pattern .", AgentRole::Mate),
            CommandCheck::Blocked(_)
        ));
    }

    #[test]
    fn broad_rm_blocked() {
        assert!(matches!(
            check_command("rm -rf .", AgentRole::Mate),
            CommandCheck::Blocked(_)
        ));
        assert!(matches!(
            check_command("rm -rf /", AgentRole::Captain),
            CommandCheck::Blocked(_)
        ));
        assert!(matches!(
            check_command("rm -rf ~", AgentRole::Mate),
            CommandCheck::Blocked(_)
        ));
    }

    #[test]
    fn targeted_rm_allowed() {
        assert_eq!(
            check_command("rm target/debug/foo", AgentRole::Mate),
            CommandCheck::Allowed
        );
    }

    #[test]
    fn normal_commands_allowed() {
        assert_eq!(
            check_command("cargo test", AgentRole::Mate),
            CommandCheck::Allowed
        );
        assert_eq!(
            check_command("npm install", AgentRole::Captain),
            CommandCheck::Allowed
        );
    }

    #[test]
    fn empty_command_allowed() {
        assert_eq!(check_command("", AgentRole::Mate), CommandCheck::Allowed);
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
}
