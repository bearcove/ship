use facet::Facet;

/// A single operation submitted to the code tool.
///
/// Batch semantics: read-only ops (Search, Read, ReadNode) continue on
/// failure. Mutation ops (Edit, ReplaceNode, DeleteNode, Write, Run,
/// Commit) stop the batch on failure. Communication ops (Message) are
/// fire-and-forget.
#[derive(Debug, Facet)]
#[facet(rename_all = "snake_case")]
#[repr(u8)]
pub enum Op {
    // ── Read-only ────────────────────────────────────────────────
    /// Search for text and symbols in the codebase.
    Search(SearchOp),

    /// Read a file or a range of lines.
    Read(ReadOp),

    /// Read a specific symbol (function, struct, impl, etc.) by query.
    ReadNode(ReadNodeOp),

    // ── Mutations ────────────────────────────────────────────────
    /// Apply line-range edits to a file.
    Edit(EditOp),

    /// Replace a symbol's body with new content (re-parses fresh).
    ReplaceNode(ReplaceNodeOp),

    /// Delete a symbol from a file (re-parses fresh).
    DeleteNode(DeleteNodeOp),

    /// Write (create or overwrite) a file.
    Write(WriteOp),

    /// Run a shell command.
    Run(RunOp),

    /// Squash shadow commits into a real commit.
    Commit(CommitOp),

    /// Restore to a previous snapshot.
    Undo(UndoOp),

    // ── Communication ────────────────────────────────────────────
    /// Send a message to another participant.
    Message(MessageOp),

    /// Submit work for review (mate only).
    Submit(SubmitOp),
}

impl Op {
    /// Whether this operation is read-only (doesn't modify files/state).
    pub fn is_read_only(&self) -> bool {
        matches!(self, Op::Search(_) | Op::Read(_) | Op::ReadNode(_))
    }

    /// Whether this operation is a mutation (modifies files/state).
    pub fn is_mutation(&self) -> bool {
        matches!(
            self,
            Op::Edit(_)
                | Op::ReplaceNode(_)
                | Op::DeleteNode(_)
                | Op::Write(_)
                | Op::Run(_)
                | Op::Commit(_)
                | Op::Undo(_)
        )
    }
}

// ── Search ───────────────────────────────────────────────────────────

/// Search for text (and symbols) in the codebase.
#[derive(Debug, Facet)]
pub struct SearchOp {
    /// The search query — tried as regex first, then as literal.
    pub query: String,
    /// Optional path to scope the search (relative to worktree root).
    #[facet(default)]
    pub path: Option<String>,
    /// Optional file glob filter (e.g. "*.rs").
    #[facet(default)]
    pub file_glob: Option<String>,
    /// Case-sensitive search. Default false.
    #[facet(default)]
    pub case_sensitive: bool,
}

// ── Read ─────────────────────────────────────────────────────────────

/// Read a file or a range of lines from a file.
#[derive(Debug, Facet)]
pub struct ReadOp {
    /// Path to the file (relative to worktree root).
    pub file: String,
    /// Start line (1-indexed, inclusive). If omitted, starts from the beginning.
    #[facet(default)]
    pub start_line: Option<usize>,
    /// End line (1-indexed, inclusive). If omitted, reads to the end.
    #[facet(default)]
    pub end_line: Option<usize>,
}

/// Read a specific symbol by query (e.g. "fn handle_request", "impl Server").
#[derive(Debug, Facet)]
pub struct ReadNodeOp {
    /// Path to the file (relative to worktree root).
    pub file: String,
    /// Symbol query — name or kind-qualified name.
    pub query: String,
    /// Line offset within the symbol body (0-indexed). Optional.
    #[facet(default)]
    pub offset: Option<usize>,
    /// Maximum number of lines to return. Optional.
    #[facet(default)]
    pub limit: Option<usize>,
}

// ── Edit ─────────────────────────────────────────────────────────────

/// Apply edits to a file. Returns a diff and snapshot number.
#[derive(Debug, Facet)]
pub struct EditOp {
    /// Path to the file (relative to worktree root).
    pub file: String,
    /// List of edits to apply in order.
    pub edits: Vec<Edit>,
}

/// A single edit within a file.
#[derive(Debug, Facet)]
#[facet(rename_all = "snake_case")]
#[repr(u8)]
pub enum Edit {
    /// Replace a range of lines (1-indexed, inclusive) with new content.
    ReplaceLines {
        start: usize,
        end: usize,
        content: String,
    },
    /// Insert content before a line number (1-indexed).
    InsertLines { before: usize, content: String },
    /// Delete a range of lines (1-indexed, inclusive).
    DeleteLines { start: usize, end: usize },
    /// Find and replace text.
    FindReplace {
        find: String,
        replace: String,
        #[facet(default)]
        replace_all: bool,
    },
}

// ── Structural ops ───────────────────────────────────────────────────

/// Replace a symbol's body with new content.
/// Re-parses the file with tree-sitter to find the symbol fresh.
#[derive(Debug, Facet)]
pub struct ReplaceNodeOp {
    /// Path to the file (relative to worktree root).
    pub file: String,
    /// Symbol query (e.g. "fn old_function", "struct Config").
    pub query: String,
    /// The new source code to replace the symbol with.
    pub content: String,
}

/// Delete a symbol from a file.
/// Re-parses the file with tree-sitter to find the symbol fresh.
#[derive(Debug, Facet)]
pub struct DeleteNodeOp {
    /// Path to the file (relative to worktree root).
    pub file: String,
    /// Symbol query (e.g. "fn dead_code", "const UNUSED").
    pub query: String,
}

// ── Write ────────────────────────────────────────────────────────────

/// Write (create or overwrite) a file.
#[derive(Debug, Facet)]
pub struct WriteOp {
    /// Path to the file (relative to worktree root).
    pub file: String,
    /// The content to write.
    pub content: String,
}

// ── Run ──────────────────────────────────────────────────────────────

/// Run a shell command in the worktree.
#[derive(Debug, Facet)]
pub struct RunOp {
    /// The command to run (passed to sh -c).
    pub command: String,
    /// Optional working directory relative to worktree root.
    #[facet(default)]
    pub cwd: Option<String>,
    /// Optional timeout in seconds. Default 120.
    #[facet(default)]
    pub timeout_secs: Option<u64>,
}

// ── Commit ───────────────────────────────────────────────────────────

/// Squash all shadow commits into a real commit.
#[derive(Debug, Facet)]
pub struct CommitOp {
    /// The commit message.
    pub message: String,
}

// ── Undo ─────────────────────────────────────────────────────────────

/// Restore the worktree to a previous snapshot.
#[derive(Debug, Facet)]
pub struct UndoOp {
    /// Snapshot number to restore to.
    pub snapshot: u64,
}

// ── Communication ────────────────────────────────────────────────────

/// Send a message to another participant.
#[derive(Debug, Facet)]
pub struct MessageOp {
    /// Recipient: "captain", "human", or "admiral".
    pub to: String,
    /// The message text.
    pub text: String,
}

/// Submit work for review (mate only).
#[derive(Debug, Facet)]
pub struct SubmitOp {
    /// Summary of what was accomplished.
    pub summary: String,
}

// ── Result types ─────────────────────────────────────────────────────

/// Result of executing one or more operations.
#[derive(Debug, Facet)]
pub struct OpResult {
    /// Per-operation results, in order.
    pub results: Vec<SingleResult>,
}

#[derive(Debug, Facet)]
#[facet(rename_all = "snake_case")]
#[repr(u8)]
pub enum SingleResult {
    Search(SearchResult),
    Read(ReadResult),
    Edit(EditResult),
    Run(RunResult),
    Commit(CommitResult),
    Undo(UndoResult),
    Ok(OkResult),
    Error(ErrorResult),
}

#[derive(Debug, Facet)]
pub struct SearchResult {
    pub text_matches: Vec<TextMatch>,
}

#[derive(Debug, Facet)]
pub struct TextMatch {
    pub file: String,
    pub line: usize,
    pub text: String,
    pub context_before: Vec<String>,
    pub context_after: Vec<String>,
}

#[derive(Debug, Facet)]
pub struct ReadResult {
    pub file: String,
    pub content: String,
    pub start_line: usize,
    pub end_line: usize,
    pub total_lines: usize,
}

#[derive(Debug, Facet)]
pub struct EditResult {
    pub snapshot: u64,
    pub diff: String,
    pub shadow_count: u64,
    #[facet(default)]
    pub nudge: Option<String>,
}

#[derive(Debug, Facet)]
pub struct RunResult {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    /// Files changed by the command (from shadow commit diff).
    pub files_changed: Vec<String>,
    /// Snapshot number if files were changed.
    #[facet(default)]
    pub snapshot: Option<u64>,
}

#[derive(Debug, Facet)]
pub struct CommitResult {
    pub hash: String,
    pub diff: String,
}

#[derive(Debug, Facet)]
pub struct UndoResult {
    pub diff: String,
    pub snapshot: u64,
}

/// Generic success with a message (for write, message, submit, etc.)
#[derive(Debug, Facet)]
pub struct OkResult {
    pub message: String,
}

#[derive(Debug, Facet)]
pub struct ErrorResult {
    pub message: String,
}
