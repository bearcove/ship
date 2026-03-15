use facet::Facet;

/// A single operation submitted to ship-code.
#[derive(Debug, Facet)]
#[repr(u8)]
pub enum Op {
    Search(SearchOp),
    Edit(EditOp),
    Undo(UndoOp),
}

/// Search for text (and eventually symbols) in the codebase.
#[derive(Debug, Facet)]
pub struct SearchOp {
    /// The search query — treated as literal first, then ERE, then BRE.
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

/// Edit a file. Returns a diff and snapshot number.
#[derive(Debug, Facet)]
pub struct EditOp {
    /// Path to the file (relative to worktree root).
    pub file: String,
    /// List of edits to apply in order.
    pub edits: Vec<Edit>,
}

/// A single edit within a file.
#[derive(Debug, Facet)]
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

/// Undo to a previous snapshot.
#[derive(Debug, Facet)]
pub struct UndoOp {
    /// Snapshot number to restore to.
    pub snapshot: u64,
}

/// Result of executing one or more operations.
#[derive(Debug, Facet)]
pub struct OpResult {
    /// Per-operation results, in order.
    pub results: Vec<SingleResult>,
}

#[derive(Debug, Facet)]
#[repr(u8)]
pub enum SingleResult {
    Search(SearchResult),
    Edit(EditResult),
    Undo(UndoResult),
    Error(ErrorResult),
}

#[derive(Debug, Facet)]
pub struct SearchResult {
    pub text_matches: Vec<TextMatch>,
    // Future: pub symbol_matches: Vec<SymbolMatch>,
}

#[derive(Debug, Facet)]
pub struct TextMatch {
    pub file: String,
    pub line: usize,
    pub text: String,
    /// Lines of context before the match.
    pub context_before: Vec<String>,
    /// Lines of context after the match.
    pub context_after: Vec<String>,
}

#[derive(Debug, Facet)]
pub struct EditResult {
    /// Snapshot number after this edit.
    pub snapshot: u64,
    /// Unified diff of the changes.
    pub diff: String,
    /// Total shadow commits since last real commit.
    pub shadow_count: u64,
    /// Nudge message if shadow_count is high.
    #[facet(default)]
    pub nudge: Option<String>,
}

#[derive(Debug, Facet)]
pub struct UndoResult {
    /// Diff from the state we left to the state we restored.
    pub diff: String,
    /// Current snapshot after undo.
    pub snapshot: u64,
}

#[derive(Debug, Facet)]
pub struct ErrorResult {
    pub message: String,
}
