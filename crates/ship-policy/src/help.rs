use crate::{AgentRole, TaskPhase};

/// A single available action with its short and long descriptions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionHelp {
    /// Tool or action name, e.g. "captain_assign"
    pub name: &'static str,
    /// One-line description for short hints
    pub short: &'static str,
    /// Multi-line help with examples, for long-form help
    pub long: &'static str,
}

/// Short hint: what can this role do right now?
/// Returns a compact list suitable for injection after state transitions.
pub fn short_hint(role: AgentRole, phase: Option<TaskPhase>) -> String {
    let actions = available_actions(role, phase);
    if actions.is_empty() {
        return "No actions available right now.".to_string();
    }
    let mut out = String::from("Available actions:\n");
    for action in &actions {
        out.push_str(&format!("- `{}` — {}\n", action.name, action.short));
    }
    out
}

/// Long-form help for a specific tool. Returns None if the tool isn't
/// recognized or isn't available for this role/phase.
pub fn tool_help(role: AgentRole, phase: Option<TaskPhase>, tool_name: &str) -> Option<String> {
    let actions = available_actions(role, phase);
    let action = actions.iter().find(|a| a.name == tool_name)?;
    Some(format!(
        "## `{}`\n\n{}\n\n{}\n",
        action.name, action.short, action.long
    ))
}

/// Full long-form help for all actions available to this role/phase.
pub fn full_help(role: AgentRole, phase: Option<TaskPhase>) -> String {
    let actions = available_actions(role, phase);
    if actions.is_empty() {
        return "No actions available right now.".to_string();
    }
    let mut out = String::new();
    for action in &actions {
        out.push_str(&format!(
            "## `{}`\n\n{}\n\n{}\n\n",
            action.name, action.short, action.long
        ));
    }
    out
}

/// Error help: when a tool is used incorrectly or at the wrong time.
pub fn wrong_tool_help(
    role: AgentRole,
    phase: Option<TaskPhase>,
    tool_name: &str,
) -> String {
    let phase_desc = match phase {
        Some(p) => format!("{p:?}"),
        None => "no active task".to_string(),
    };

    // Is this a real tool but not available now?
    let all_tools = all_actions_for_role(role);
    if let Some(action) = all_tools.iter().find(|a| a.name == tool_name) {
        return format!(
            "`{}` is not available during {phase_desc}. {}\n\n{}\n",
            action.name,
            action.short,
            short_hint(role, phase),
        );
    }

    format!(
        "Unknown tool `{tool_name}`.\n\n{}\n",
        short_hint(role, phase),
    )
}

/// What actions are available for a given role and task phase?
pub fn available_actions(role: AgentRole, phase: Option<TaskPhase>) -> Vec<&'static ActionHelp> {
    match role {
        AgentRole::Captain => captain_actions(phase),
        AgentRole::Mate => mate_actions(phase),
        AgentRole::Admiral => admiral_actions(),
    }
}

/// All actions a role could ever use (for wrong-tool diagnostics).
fn all_actions_for_role(role: AgentRole) -> Vec<&'static ActionHelp> {
    match role {
        AgentRole::Captain => CAPTAIN_ACTIONS.iter().collect(),
        AgentRole::Mate => MATE_ACTIONS.iter().collect(),
        AgentRole::Admiral => ADMIRAL_ACTIONS.iter().collect(),
    }
}

fn captain_actions(phase: Option<TaskPhase>) -> Vec<&'static ActionHelp> {
    let always: &[&str] = &["code", "captain_git_status", "web_search"];

    let phase_tools: &[&str] = match phase {
        None => &["captain_assign"],
        Some(TaskPhase::Assigned) => &["captain_cancel"],
        Some(TaskPhase::Working) => &["captain_cancel"],
        Some(TaskPhase::ReviewPending) => &[
            "captain_merge",
            "captain_steer",
            "captain_cancel",
            "captain_review_diff",
            "captain_rebase_status",
        ],
        Some(TaskPhase::SteerPending) => &["captain_steer", "captain_merge", "captain_cancel"],
        Some(TaskPhase::RebaseConflict) => &[
            "captain_continue_rebase",
            "captain_abort_rebase",
            "captain_cancel",
        ],
        Some(TaskPhase::WaitingForHuman) => &["captain_notify_human"],
        Some(TaskPhase::Accepted | TaskPhase::Cancelled) => &["captain_assign"],
    };

    CAPTAIN_ACTIONS
        .iter()
        .filter(|a| always.contains(&a.name) || phase_tools.contains(&a.name))
        .collect()
}

fn mate_actions(phase: Option<TaskPhase>) -> Vec<&'static ActionHelp> {
    match phase {
        Some(TaskPhase::Working) => MATE_ACTIONS.iter().collect(),
        Some(TaskPhase::Assigned) => {
            // Mate is about to start — code + ask available
            MATE_ACTIONS
                .iter()
                .filter(|a| a.name != "mate_submit")
                .collect()
        }
        _ => vec![],
    }
}

fn admiral_actions() -> Vec<&'static ActionHelp> {
    ADMIRAL_ACTIONS.iter().collect()
}

// ── Action definitions ───────────────────────────────────────────────

static CAPTAIN_ACTIONS: &[ActionHelp] = &[
    ActionHelp {
        name: "captain_assign",
        short: "Assign a task to your mate",
        long: "Delegates implementation work to the mate. Include a title, description, \
               relevant files, and a step-by-step plan.\n\
               \n\
               Example: assign a task to refactor auth middleware\n\
               ```\n\
               captain_assign(\n\
                 title: \"Refactor auth middleware\",\n\
                 description: \"Replace session token storage with...\",\n\
                 extras: { files: [...], plan: [...] }\n\
               )\n\
               ```",
    },
    ActionHelp {
        name: "captain_steer",
        short: "Send course-correction to the mate",
        long: "Sends feedback to redirect the mate's work. Steers are for correction only — \
               never use them to add scope. New work waits for merge.\n\
               \n\
               Example:\n\
               ```\n\
               captain_steer(message: \"Focus on error handling, skip the UI for now\")\n\
               ```",
    },
    ActionHelp {
        name: "captain_merge",
        short: "Accept the mate's work and merge",
        long: "Accepts the submitted work, rebases onto the base branch, runs checks, \
               and fast-forward merges. Only available after the mate has submitted.\n\
               \n\
               If checks fail, fix the issues with `code` first, then try again.",
    },
    ActionHelp {
        name: "captain_cancel",
        short: "Cancel the current task",
        long: "Cancels the active task. The mate is stopped and the task is archived. \
               Use when the task is no longer needed or needs to be restarted from scratch.",
    },
    ActionHelp {
        name: "captain_review_diff",
        short: "See the diff of the mate's work",
        long: "Shows the cumulative diff of all changes on the session branch vs the base branch. \
               Use this to review before merging or to understand what the mate has done.",
    },
    ActionHelp {
        name: "captain_git_status",
        short: "Check git status of the session worktree",
        long: "Shows branch info, dirty state, rebase status, and conflict markers. \
               Useful for understanding the current state before taking action.",
    },
    ActionHelp {
        name: "captain_rebase_status",
        short: "Check the state of an in-progress rebase",
        long: "Shows which files are conflicted and whether you can continue or abort. \
               Only relevant when the task is in RebaseConflict state.",
    },
    ActionHelp {
        name: "captain_continue_rebase",
        short: "Continue rebase after resolving conflicts",
        long: "After fixing conflict markers with `code`, call this to continue the rebase. \
               If more conflicts remain, the task stays in RebaseConflict.",
    },
    ActionHelp {
        name: "captain_abort_rebase",
        short: "Abort the current rebase",
        long: "Abandons the rebase and returns to the pre-rebase state. \
               Use when conflicts are too complex to resolve inline.",
    },
    ActionHelp {
        name: "captain_notify_human",
        short: "Block and wait for human input",
        long: "Sends a message (with optional diff) to the human and blocks until they respond. \
               Use for decisions that genuinely require human judgment.",
    },
    ActionHelp {
        name: "code",
        short: "Read, search, edit files, run commands, commit",
        long: "Batch file operations: search, read, read_node, edit, replace_node, \
               delete_node, run, commit, undo. Always available.\n\
               \n\
               Example — search then read:\n\
               ```json\n\
               {\"ops\": [{\"search\": {\"query\": \"fn handle\"}}, \
               {\"read\": {\"file\": \"src/lib.rs\"}}]}\n\
               ```",
    },
    ActionHelp {
        name: "web_search",
        short: "Search the web",
        long: "Search the web for documentation, APIs, or other information. Always available.",
    },
];

static MATE_ACTIONS: &[ActionHelp] = &[
    ActionHelp {
        name: "code",
        short: "Read, search, edit files, run commands, commit",
        long: "Batch file operations: search, read, read_node, edit, replace_node, \
               delete_node, run, commit, undo.\n\
               \n\
               Example — edit with find/replace:\n\
               ```json\n\
               {\"ops\": [{\"edit\": {\"file\": \"src/lib.rs\", \
               \"edits\": [{\"find_replace\": {\"find\": \"old\", \"replace\": \"new\"}}]}}]}\n\
               ```",
    },
    ActionHelp {
        name: "mate_submit",
        short: "Submit work for captain review (blocks)",
        long: "Submits your work with a summary. Blocks until the captain accepts, steers, or cancels.\n\
               After submitting, do not send further messages until you get a response.\n\
               \n\
               Example:\n\
               ```\n\
               mate_submit(summary: \"Refactored auth middleware, all tests pass\")\n\
               ```",
    },
    ActionHelp {
        name: "mate_ask_captain",
        short: "Ask the captain a blocking question",
        long: "Blocks until the captain responds. Use when genuinely stuck or need a decision.\n\
               For non-blocking updates, just write a message with @captain instead.\n\
               \n\
               Example:\n\
               ```\n\
               mate_ask_captain(question: \"Should I keep backward compat or clean break?\")\n\
               ```",
    },
];

static ADMIRAL_ACTIONS: &[ActionHelp] = &[
    ActionHelp {
        name: "admiral_list_lanes",
        short: "See all active sessions and their status",
        long: "Lists every active lane with captain name, task status, and progress.",
    },
    ActionHelp {
        name: "admiral_create_lane",
        short: "Create a new session/lane",
        long: "Starts a new session for a project. Picks agents and creates the worktree.",
    },
    ActionHelp {
        name: "admiral_steer_captain",
        short: "Send a message to a captain",
        long: "Delivers a message to a specific captain. Use to give direction, \
               approve work, or coordinate between lanes.",
    },
    ActionHelp {
        name: "admiral_post_to_human",
        short: "Surface a message to the human",
        long: "Posts to the human's activity feed. Only use for decisions that \
               genuinely require human judgment. You are the buffer — handle what you can.",
    },
    ActionHelp {
        name: "admiral_list_projects",
        short: "List registered projects",
        long: "Shows all projects the system knows about.",
    },
    ActionHelp {
        name: "read_file",
        short: "Read any file by absolute path",
        long: "Read-only file access. You cannot edit files — go through a captain for that.",
    },
    ActionHelp {
        name: "run_command",
        short: "Run a shell command",
        long: "Execute a command with an absolute cwd path. Read-only — no destructive operations.",
    },
    ActionHelp {
        name: "web_search",
        short: "Search the web",
        long: "Search the web for documentation or information.",
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn captain_no_task_can_assign() {
        let actions = available_actions(AgentRole::Captain, None);
        let names: Vec<&str> = actions.iter().map(|a| a.name).collect();
        assert!(names.contains(&"captain_assign"));
        assert!(!names.contains(&"captain_merge"));
        assert!(!names.contains(&"captain_steer"));
    }

    #[test]
    fn captain_review_pending_can_merge_or_steer() {
        let actions = available_actions(AgentRole::Captain, Some(TaskPhase::ReviewPending));
        let names: Vec<&str> = actions.iter().map(|a| a.name).collect();
        assert!(names.contains(&"captain_merge"));
        assert!(names.contains(&"captain_steer"));
        assert!(names.contains(&"captain_review_diff"));
        assert!(!names.contains(&"captain_assign"));
    }

    #[test]
    fn captain_rebase_conflict_can_continue_or_abort() {
        let actions = available_actions(AgentRole::Captain, Some(TaskPhase::RebaseConflict));
        let names: Vec<&str> = actions.iter().map(|a| a.name).collect();
        assert!(names.contains(&"captain_continue_rebase"));
        assert!(names.contains(&"captain_abort_rebase"));
        assert!(!names.contains(&"captain_merge"));
    }

    #[test]
    fn captain_always_has_code_and_git_status() {
        for phase in [None, Some(TaskPhase::Working), Some(TaskPhase::ReviewPending)] {
            let actions = available_actions(AgentRole::Captain, phase);
            let names: Vec<&str> = actions.iter().map(|a| a.name).collect();
            assert!(names.contains(&"code"), "missing code for {phase:?}");
            assert!(
                names.contains(&"captain_git_status"),
                "missing git_status for {phase:?}"
            );
        }
    }

    #[test]
    fn mate_working_has_all_tools() {
        let actions = available_actions(AgentRole::Mate, Some(TaskPhase::Working));
        let names: Vec<&str> = actions.iter().map(|a| a.name).collect();
        assert!(names.contains(&"code"));
        assert!(names.contains(&"mate_submit"));
        assert!(names.contains(&"mate_ask_captain"));
    }

    #[test]
    fn mate_no_task_has_nothing() {
        let actions = available_actions(AgentRole::Mate, None);
        assert!(actions.is_empty());
    }

    #[test]
    fn wrong_tool_known_but_unavailable() {
        let msg = wrong_tool_help(AgentRole::Captain, None, "captain_merge");
        assert!(msg.contains("not available during no active task"));
        assert!(msg.contains("captain_assign"));
    }

    #[test]
    fn wrong_tool_unknown() {
        let msg = wrong_tool_help(AgentRole::Captain, None, "frobnicate");
        assert!(msg.contains("Unknown tool"));
    }

    #[test]
    fn tool_help_returns_long_form() {
        let help = tool_help(AgentRole::Captain, None, "captain_assign").unwrap();
        assert!(help.contains("Delegates implementation work"));
    }

    #[test]
    fn tool_help_unavailable_returns_none() {
        let help = tool_help(AgentRole::Captain, None, "captain_merge");
        assert!(help.is_none());
    }

    #[test]
    fn snapshot_short_hint_captain_no_task() {
        insta::assert_snapshot!(
            "hint_captain_no_task",
            short_hint(AgentRole::Captain, None)
        );
    }

    #[test]
    fn snapshot_short_hint_captain_review() {
        insta::assert_snapshot!(
            "hint_captain_review",
            short_hint(AgentRole::Captain, Some(TaskPhase::ReviewPending))
        );
    }

    #[test]
    fn snapshot_short_hint_mate_working() {
        insta::assert_snapshot!(
            "hint_mate_working",
            short_hint(AgentRole::Mate, Some(TaskPhase::Working))
        );
    }

    #[test]
    fn snapshot_wrong_tool_captain() {
        insta::assert_snapshot!(
            "wrong_tool_captain_merge_no_task",
            wrong_tool_help(AgentRole::Captain, None, "captain_merge")
        );
    }

    #[test]
    fn snapshot_full_help_admiral() {
        insta::assert_snapshot!(
            "full_help_admiral",
            full_help(AgentRole::Admiral, None)
        );
    }
}
