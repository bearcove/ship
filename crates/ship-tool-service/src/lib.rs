use ship_policy::{ParticipantName, PlanStep, RoomId};

/// Result of a tool call — either text content or an error.
#[derive(Debug, Clone, facet::Facet)]
pub struct ToolResult {
    pub text: String,
    pub is_error: bool,
}

/// Diff stats for the current worktree.
#[derive(Debug, Clone, facet::Facet)]
pub struct DiffStats {
    pub lines_added: u64,
    pub lines_removed: u64,
    pub files_changed: u64,
}

/// Identifies which agent is calling — set per-connection, not per-call.
#[derive(Debug, Clone, facet::Facet)]
pub struct CallerIdentity {
    pub participant: ParticipantName,
    pub room_id: RoomId,
}

// r[tool.rpc]
#[roam::service]
pub trait ToolBackend {
    // ── File I/O ────────────────────────────────────────────────────

    /// Read a file from the worktree.
    async fn read_file(
        &self,
        path: String,
        offset: Option<u64>,
        limit: Option<u64>,
    ) -> ToolResult;

    /// Write a file to the worktree.
    async fn write_file(&self, path: String, content: String) -> ToolResult;

    /// Prepare a string replacement edit (returns an edit_id for confirmation).
    async fn edit_prepare(
        &self,
        path: String,
        old_string: String,
        new_string: String,
        replace_all: bool,
    ) -> ToolResult;

    /// Confirm a prepared edit.
    async fn edit_confirm(&self, edit_id: String) -> ToolResult;

    // ── Shell ───────────────────────────────────────────────────────

    /// Run a command in the worktree.
    async fn run_command(&self, command: String, cwd: Option<String>) -> ToolResult;

    // ── Git ─────────────────────────────────────────────────────────

    /// Get git status of the worktree.
    async fn git_status(&self) -> ToolResult;

    /// Get the diff for review.
    async fn review_diff(&self) -> ToolResult;

    /// Commit staged changes. If step_index is set, marks that plan step done.
    async fn commit(&self, message: String, step_index: Option<u64>) -> ToolResult;

    /// Get rebase conflict status.
    async fn rebase_status(&self) -> ToolResult;

    /// Continue a rebase after resolving conflicts.
    async fn continue_rebase(&self) -> ToolResult;

    /// Abort an in-progress rebase.
    async fn abort_rebase(&self) -> ToolResult;

    /// Get worktree diff stats (lines added/removed since base).
    async fn diff_stats(&self) -> DiffStats;

    // ── Task lifecycle ──────────────────────────────────────────────

    /// Assign a task to the mate in this lane.
    async fn assign_task(
        &self,
        title: String,
        description: String,
    ) -> ToolResult;

    /// Submit work for review (mate → captain).
    async fn submit(&self, summary: String) -> ToolResult;

    /// Cancel the current task.
    async fn cancel_task(&self, reason: Option<String>) -> ToolResult;

    /// Merge the current task's branch (captain approves).
    async fn merge(&self, summary: Option<String>) -> ToolResult;

    // ── Plan ────────────────────────────────────────────────────────

    /// Set the work plan (list of steps).
    async fn set_plan(&self, steps: Vec<PlanStep>) -> ToolResult;

    // ── Messaging ───────────────────────────────────────────────────

    /// Send a message to another participant (routed through policy).
    async fn send_message(&self, mention: ParticipantName, text: String) -> ToolResult;

    /// Ask a question (mate → captain, blocks until answered).
    async fn ask(&self, question: String) -> ToolResult;

    /// Post a message to the human (escalation).
    async fn notify_human(&self, message: String) -> ToolResult;

    /// List files in the worktree matching a query.
    async fn list_files(&self, query: String) -> Vec<String>;

    // ── Unified code tool ───────────────────────────────────────────

    /// Execute a batch of code operations (the unified ship-code tool).
    async fn code(&self, ops_json: String) -> ToolResult;
}
