use facet::Facet;
use ship_types::McpDiffContent;

/// Args for the `code` tool (shared between captain and mate).
#[derive(Debug, Facet)]
pub struct CodeArgs {
    /// Array of operations to execute.
    pub ops: Vec<CodeOp>,
}

/// Result for the `code` tool.
#[derive(Debug, Facet)]
pub struct CodeResult {
    /// The result text.
    pub text: String,
    /// Diffs produced by mutation operations.
    #[facet(default)]
    pub diffs: Vec<McpDiffContent>,
}

/// A single code operation. Each op is an object with exactly one key (the op type)
/// whose value is the op parameters.
#[derive(Debug, Facet)]
#[repr(u8)]
#[facet(rename_all = "snake_case")]
#[allow(dead_code)]
pub enum CodeOp {
    /// Search files using regex or literal query.
    Search(SearchOp),
    /// Read a file (optionally a range of lines).
    Read(ReadOp),
    /// Read a specific symbol (function, struct, impl block) from a file.
    ReadNode(ReadNodeOp),
    /// Apply edits to a file.
    Edit(EditOp),
    /// Replace an entire symbol with new content.
    ReplaceNode(ReplaceNodeOp),
    /// Delete a symbol from a file.
    DeleteNode(DeleteNodeOp),
    /// Run a shell command.
    Run(RunOp),
    /// Commit staged changes.
    Commit(CommitOp),
    /// Undo to a previous snapshot.
    Undo(UndoOp),
    /// Send a message to another agent.
    Message(MessageOp),
    /// Submit completed work.
    Submit(SubmitOp),
}

#[derive(Debug, Facet)]
pub struct SearchOp {
    /// Regex or literal search query.
    pub query: String,
    /// Scope search to this directory.
    #[facet(default)]
    pub path: Option<String>,
    /// File glob filter, e.g. '*.rs'.
    #[facet(default)]
    pub file_glob: Option<String>,
    /// Whether the search is case-sensitive.
    #[facet(default)]
    pub case_sensitive: Option<bool>,
}

#[derive(Debug, Facet)]
pub struct ReadOp {
    /// Worktree-relative path.
    pub file: String,
    /// 1-indexed start line.
    #[facet(default)]
    pub start_line: Option<i64>,
    /// 1-indexed end line.
    #[facet(default)]
    pub end_line: Option<i64>,
}

#[derive(Debug, Facet)]
pub struct ReadNodeOp {
    /// Worktree-relative path.
    pub file: String,
    /// Symbol query, e.g. 'fn handle_request' or 'impl Server'.
    pub query: String,
    /// Line offset within symbol body.
    #[facet(default)]
    pub offset: Option<i64>,
    /// Max lines to return.
    #[facet(default)]
    pub limit: Option<i64>,
}

#[derive(Debug, Facet)]
pub struct EditOp {
    /// Worktree-relative path.
    pub file: String,
    /// Array of edit actions to apply.
    pub edits: Vec<EditAction>,
}

/// A single edit action within an edit operation.
#[derive(Debug, Facet)]
#[repr(u8)]
#[facet(rename_all = "snake_case")]
#[allow(dead_code)]
pub enum EditAction {
    /// Find and replace text.
    FindReplace(FindReplaceAction),
    /// Replace a range of lines.
    ReplaceLines(ReplaceLinesAction),
    /// Insert lines before a given line.
    InsertLines(InsertLinesAction),
    /// Delete a range of lines.
    DeleteLines(DeleteLinesAction),
}

#[derive(Debug, Facet)]
pub struct FindReplaceAction {
    /// Text to find.
    pub find: String,
    /// Replacement text.
    pub replace: String,
    /// Replace all occurrences (default: first only).
    #[facet(default)]
    pub replace_all: Option<bool>,
}

#[derive(Debug, Facet)]
pub struct ReplaceLinesAction {
    /// 1-indexed start line.
    pub start: i64,
    /// 1-indexed end line (inclusive).
    pub end: i64,
    /// New content for the range.
    pub content: String,
}

#[derive(Debug, Facet)]
pub struct InsertLinesAction {
    /// Insert before this 1-indexed line.
    pub before: i64,
    /// Content to insert.
    pub content: String,
}

#[derive(Debug, Facet)]
pub struct DeleteLinesAction {
    /// 1-indexed start line.
    pub start: i64,
    /// 1-indexed end line (inclusive).
    pub end: i64,
}

#[derive(Debug, Facet)]
pub struct ReplaceNodeOp {
    /// Worktree-relative path.
    pub file: String,
    /// Symbol query to find and replace.
    pub query: String,
    /// New source code for the symbol.
    pub content: String,
}

#[derive(Debug, Facet)]
pub struct DeleteNodeOp {
    /// Worktree-relative path.
    pub file: String,
    /// Symbol query to delete.
    pub query: String,
}

#[derive(Debug, Facet)]
pub struct RunOp {
    /// Shell command (passed to sh -c).
    pub command: String,
    /// Worktree-relative working directory.
    #[facet(default)]
    pub cwd: Option<String>,
    /// Timeout in seconds (default 120).
    #[facet(default)]
    pub timeout_secs: Option<i64>,
}

#[derive(Debug, Facet)]
pub struct CommitOp {
    /// Commit message.
    pub message: String,
}

#[derive(Debug, Facet)]
pub struct UndoOp {
    /// Snapshot number to restore to.
    pub snapshot: i64,
}

#[derive(Debug, Facet)]
pub struct MessageOp {
    /// Recipient: captain, human, or admiral.
    pub to: String,
    /// Message text.
    pub text: String,
}

#[derive(Debug, Facet)]
pub struct SubmitOp {
    /// Summary of the completed work.
    pub summary: String,
}
