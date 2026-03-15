use std::path::{Path, PathBuf};
use std::time::Duration;

use eyre::Result;
use tokio::process::Command;

use crate::callbacks::EngineCallbacks;
use crate::edit;
use crate::ops::*;
use crate::search;
use crate::snapshot::SnapshotManager;
use crate::structural;

/// The main engine that executes code operations.
/// Owns the snapshot manager and dispatches operations.
pub struct Engine<C: EngineCallbacks> {
    worktree: PathBuf,
    snapshots: SnapshotManager,
    callbacks: C,
}

/// The result of executing a batch of operations.
/// Always includes status information to teach the agent.
pub struct BatchResult {
    /// Human-readable output for each operation.
    pub entries: Vec<OpEntry>,
    /// Current snapshot number after all operations.
    pub current_snapshot: u64,
    /// Total shadow commits since last real commit.
    pub shadow_count: u64,
    /// Whether the batch was stopped early due to a mutation failure.
    pub stopped_early: bool,
    /// Number of ops that were skipped.
    pub skipped: usize,
}

pub struct OpEntry {
    pub op_name: String,
    pub output: String,
    pub is_error: bool,
}

impl BatchResult {
    /// Format the full response, including status footer.
    pub fn format(&self) -> String {
        let mut output = String::new();

        for (i, entry) in self.entries.iter().enumerate() {
            if i > 0 {
                output.push_str("\n---\n");
            }
            if entry.is_error {
                output.push_str(&format!("ERROR ({}): {}", entry.op_name, entry.output));
            } else {
                output.push_str(&entry.output);
            }
        }

        if self.stopped_early {
            output.push_str(&format!(
                "\n\n{} remaining op(s) were not executed due to the error above.",
                self.skipped
            ));
        }

        // Always append status footer
        output.push_str(&format!(
            "\n\n[snapshot: {} | shadow commits: {}",
            self.current_snapshot, self.shadow_count,
        ));
        if self.current_snapshot > 0 {
            output.push_str(&format!(
                " | undo available: 1-{}",
                self.current_snapshot
            ));
        }
        output.push(']');

        if self.shadow_count >= 100 {
            output.push_str(&format!(
                "\n⚠ You have {} uncommitted edits. Consider committing.",
                self.shadow_count
            ));
        }

        output
    }
}

impl<C: EngineCallbacks> Engine<C> {
    /// Create a new engine for the given worktree.
    pub fn new(worktree: PathBuf, snapshots: SnapshotManager, callbacks: C) -> Self {
        Self {
            worktree,
            snapshots,
            callbacks,
        }
    }

    /// Execute a batch of operations with stop-on-mutation-error semantics.
    ///
    /// Read-only ops continue on failure. Mutation ops stop the batch.
    pub async fn execute(&mut self, ops: &[Op]) -> BatchResult {
        let mut entries = Vec::new();
        let mut stopped_early = false;
        let mut skipped = 0;

        for (i, op) in ops.iter().enumerate() {
            let (op_name, result) = self.dispatch(op).await;

            match result {
                Ok(text) => {
                    entries.push(OpEntry {
                        op_name,
                        output: text,
                        is_error: false,
                    });
                }
                Err(err_text) => {
                    entries.push(OpEntry {
                        op_name: op_name.clone(),
                        output: err_text,
                        is_error: true,
                    });

                    // Read-only ops don't stop the batch
                    if op.is_mutation() {
                        stopped_early = true;
                        skipped = ops.len() - i - 1;
                        break;
                    }
                }
            }
        }

        BatchResult {
            entries,
            current_snapshot: self.snapshots.shadow_count(),
            shadow_count: self.snapshots.shadow_count(),
            stopped_early,
            skipped,
        }
    }

    /// Dispatch a single operation. Returns (op_name, result).
    async fn dispatch(&mut self, op: &Op) -> (String, Result<String, String>) {
        match op {
            Op::Search(o) => ("search".into(), self.exec_search(o)),
            Op::Read(o) => ("read".into(), self.exec_read(o)),
            Op::ReadNode(o) => ("read_node".into(), self.exec_read_node(o)),
            Op::Edit(o) => ("edit".into(), self.exec_edit(o).await),
            Op::ReplaceNode(o) => ("replace_node".into(), self.exec_replace_node(o).await),
            Op::DeleteNode(o) => ("delete_node".into(), self.exec_delete_node(o).await),
            Op::Write(o) => ("write".into(), self.exec_write(o).await),
            Op::Run(o) => ("run".into(), self.exec_run(o).await),
            Op::Commit(o) => ("commit".into(), self.exec_commit(o).await),
            Op::Undo(o) => ("undo".into(), self.exec_undo(o).await),
            Op::Message(o) => ("message".into(), self.exec_message(o).await),
            Op::Submit(o) => ("submit".into(), self.exec_submit(o).await),
        }
    }

    // ── Read-only ops ────────────────────────────────────────────

    fn exec_search(&self, op: &SearchOp) -> Result<String, String> {
        match search::search(&self.worktree, op) {
            Ok(output) => {
                let formatted = search::format_output(&output);
                if output.text_matches.is_empty() && output.symbol_matches.is_empty() {
                    Ok(self.format_empty_search(op))
                } else {
                    Ok(formatted)
                }
            }
            Err(e) => Err(format!(
                "Search failed: {e}\n\
                 Hint: the query is tried as regex first, then as literal text."
            )),
        }
    }

    fn exec_read(&self, op: &ReadOp) -> Result<String, String> {
        let file_path = self.worktree.join(&op.file);
        let source = std::fs::read_to_string(&file_path)
            .map_err(|e| format!(
                "Cannot read '{}': {e}\n\
                 Hint: paths are relative to the worktree root.",
                op.file
            ))?;

        let lines: Vec<&str> = source.lines().collect();
        let total = lines.len();
        let start = op.start_line.unwrap_or(1).max(1);
        let end = op.end_line.unwrap_or(total).min(total);

        if start > total {
            return Err(format!(
                "Start line {start} is beyond end of file ({total} lines)."
            ));
        }

        let selected: Vec<String> = lines[start - 1..end]
            .iter()
            .enumerate()
            .map(|(i, line)| format!("{:>4} │ {}", start + i, line))
            .collect();

        let mut output = format!("{}:{}-{} ({total} lines total)\n", op.file, start, end);
        output.push_str(&selected.join("\n"));

        Ok(output)
    }

    fn exec_read_node(&self, op: &ReadNodeOp) -> Result<String, String> {
        let file_path = self.worktree.join(&op.file);
        let source = std::fs::read_to_string(&file_path)
            .map_err(|e| format!("Cannot read '{}': {e}", op.file))?;

        match structural::read_node(
            Path::new(&op.file),
            &source,
            &op.query,
            op.offset,
            op.limit,
        ) {
            Ok(result) => {
                let mut output = format!(
                    "{} {} [{}:{}-{}]",
                    result.kind, result.name, op.file, result.start_line, result.end_line,
                );
                if let Some(parent) = &result.parent {
                    output.push_str(&format!(" (in {parent})"));
                }
                if result.windowed {
                    output.push_str(&format!(
                        " (showing partial, {}/{} lines)",
                        result.text.lines().count(),
                        result.total_lines
                    ));
                }
                output.push_str(&format!("\n```\n{}\n```", result.text));
                Ok(output)
            }
            Err(_) => {
                // Try to suggest similar symbols
                let suggestion = self.suggest_symbols(&source, &op.query);
                Err(format!(
                    "No symbol matching '{}' in '{}'.\n{suggestion}",
                    op.query, op.file
                ))
            }
        }
    }

    // ── Mutation ops ─────────────────────────────────────────────

    async fn exec_edit(&mut self, op: &EditOp) -> Result<String, String> {
        self.check_mutation()?;

        let file_path = self.worktree.join(&op.file);
        let source = std::fs::read_to_string(&file_path)
            .map_err(|e| format!(
                "Cannot read '{}': {e}\n\
                 Hint: paths are relative to the worktree root.",
                op.file
            ))?;

        let (new_content, diff) =
            edit::apply_edits(&source, Path::new(&op.file), &op.edits)
                .map_err(|e| format!(
                    "Edit failed on '{}': {e}\n\
                     Hint: line numbers are 1-indexed. The file has {} lines.",
                    op.file,
                    source.lines().count()
                ))?;

        std::fs::write(&file_path, &new_content)
            .map_err(|e| format!("Failed to write '{}': {e}", op.file))?;

        let (snapshot, _) = self.snapshots
            .snapshot(&format!("edit {}", op.file))
            .await
            .map_err(|e| format!("Edit applied but snapshot failed: {e}"))?;

        Ok(format!("{diff}\nSnapshot: {snapshot}"))
    }

    async fn exec_replace_node(&mut self, op: &ReplaceNodeOp) -> Result<String, String> {
        self.check_mutation()?;

        let file_path = self.worktree.join(&op.file);
        let source = std::fs::read_to_string(&file_path)
            .map_err(|e| format!("Cannot read '{}': {e}", op.file))?;

        let (new_content, diff) =
            structural::replace_node(Path::new(&op.file), &source, &op.query, &op.content)
                .map_err(|e| {
                    let suggestion = self.suggest_symbols(&source, &op.query);
                    format!("Replace failed in '{}': {e}\n{suggestion}", op.file)
                })?;

        std::fs::write(&file_path, &new_content)
            .map_err(|e| format!("Failed to write '{}': {e}", op.file))?;

        let (snapshot, _) = self.snapshots
            .snapshot(&format!("replace_node {} in {}", op.query, op.file))
            .await
            .map_err(|e| format!("Replace applied but snapshot failed: {e}"))?;

        Ok(format!("{diff}\nSnapshot: {snapshot}"))
    }

    async fn exec_delete_node(&mut self, op: &DeleteNodeOp) -> Result<String, String> {
        self.check_mutation()?;

        let file_path = self.worktree.join(&op.file);
        let source = std::fs::read_to_string(&file_path)
            .map_err(|e| format!("Cannot read '{}': {e}", op.file))?;

        let (new_content, diff) =
            structural::delete_node(Path::new(&op.file), &source, &op.query)
                .map_err(|e| {
                    let suggestion = self.suggest_symbols(&source, &op.query);
                    format!("Delete failed in '{}': {e}\n{suggestion}", op.file)
                })?;

        std::fs::write(&file_path, &new_content)
            .map_err(|e| format!("Failed to write '{}': {e}", op.file))?;

        let (snapshot, _) = self.snapshots
            .snapshot(&format!("delete_node {} in {}", op.query, op.file))
            .await
            .map_err(|e| format!("Delete applied but snapshot failed: {e}"))?;

        Ok(format!("{diff}\nSnapshot: {snapshot}"))
    }

    async fn exec_write(&mut self, op: &WriteOp) -> Result<String, String> {
        self.check_mutation()?;

        let file_path = self.worktree.join(&op.file);

        // Create parent directories if needed
        if let Some(parent) = file_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Cannot create directory for '{}': {e}", op.file))?;
        }

        let existed = file_path.exists();
        std::fs::write(&file_path, &op.content)
            .map_err(|e| format!("Failed to write '{}': {e}", op.file))?;

        let (snapshot, diff) = self.snapshots
            .snapshot(&format!("write {}", op.file))
            .await
            .map_err(|e| format!("Write succeeded but snapshot failed: {e}"))?;

        let action = if existed { "Updated" } else { "Created" };
        let mut output = format!(
            "{action} '{}' ({} bytes)",
            op.file,
            op.content.len()
        );
        if !diff.is_empty() {
            output.push_str(&format!("\n{diff}"));
        }
        output.push_str(&format!("\nSnapshot: {snapshot}"));
        Ok(output)
    }

    async fn exec_run(&mut self, op: &RunOp) -> Result<String, String> {
        self.check_mutation()?;

        let cwd = match &op.cwd {
            Some(dir) => self.worktree.join(dir),
            None => self.worktree.clone(),
        };

        let timeout = Duration::from_secs(op.timeout_secs.unwrap_or(120));

        let result = tokio::time::timeout(
            timeout,
            Command::new("sh")
                .arg("-c")
                .arg(&op.command)
                .current_dir(&cwd)
                .output(),
        )
        .await;

        let output = match result {
            Ok(Ok(output)) => output,
            Ok(Err(e)) => return Err(format!("Failed to run command: {e}")),
            Err(_) => return Err(format!(
                "Command timed out after {}s: {}",
                timeout.as_secs(),
                op.command,
            )),
        };

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let exit_code = output.status.code().unwrap_or(-1);

        // Shadow commit if files changed
        let (snapshot, diff) = self.snapshots
            .snapshot(&format!("run: {}", op.command))
            .await
            .map_err(|e| format!("Command ran but snapshot failed: {e}"))?;

        let mut result_text = String::new();

        if exit_code != 0 {
            result_text.push_str(&format!("Exit code: {exit_code}\n"));
        }

        if !stdout.is_empty() {
            result_text.push_str(&stdout);
            if !stdout.ends_with('\n') {
                result_text.push('\n');
            }
        }

        if !stderr.is_empty() {
            result_text.push_str(&format!("stderr:\n{stderr}"));
        }

        if !diff.is_empty() {
            result_text.push_str(&format!(
                "\nFiles changed:\n{diff}\nSnapshot: {snapshot} (undo available)"
            ));
        }

        if result_text.is_empty() {
            result_text = "(no output)".to_owned();
        }

        Ok(result_text)
    }

    async fn exec_commit(&mut self, op: &CommitOp) -> Result<String, String> {
        self.check_mutation()?;

        let diff = self.snapshots
            .squash_commit(&op.message)
            .await
            .map_err(|e| format!("Commit failed: {e}"))?;

        self.callbacks
            .on_commit("", &op.message)
            .await
            .map_err(|e| format!("Commit created but callback failed: {e}"))?;

        Ok(format!("Committed: {}\n\n{diff}", op.message))
    }

    async fn exec_undo(&mut self, op: &UndoOp) -> Result<String, String> {
        let current = self.snapshots.shadow_count();

        if current == 0 {
            return Err("Nothing to undo. No edits have been made yet.".to_owned());
        }

        if op.snapshot == 0 {
            return Err(format!(
                "Snapshot 0 does not exist. Valid range: 1-{current}.\n\
                 Hint: use {{ \"op\": \"undo\", \"snapshot\": N }} where N is the \
                 snapshot you want to restore to."
            ));
        }

        if op.snapshot > current {
            return Err(format!(
                "Snapshot {} does not exist. Current snapshot: {current}. \
                 Valid range: 1-{current}.\n\
                 Hint: you can only undo to snapshots that have already been created.",
                op.snapshot
            ));
        }

        match self.snapshots.undo(op.snapshot).await {
            Ok(diff) => {
                if diff.is_empty() {
                    Ok(format!(
                        "Restored to snapshot {}. No diff (already at that state).",
                        op.snapshot
                    ))
                } else {
                    Ok(format!("Restored to snapshot {}.\n\n{diff}", op.snapshot))
                }
            }
            Err(e) => Err(format!(
                "Undo to snapshot {} failed: {e}\n\
                 Current snapshot: {current}. Valid range: 1-{current}.",
                op.snapshot
            )),
        }
    }

    // ── Communication ops ────────────────────────────────────────

    async fn exec_message(&self, op: &MessageOp) -> Result<String, String> {
        let valid_targets = ["captain", "human", "admiral", "mate"];
        if !valid_targets.contains(&op.to.as_str()) {
            return Err(format!(
                "Unknown recipient '{}'. Valid targets: {}.",
                op.to,
                valid_targets.join(", ")
            ));
        }

        // Can't message yourself
        if op.to == self.callbacks.caller_role() {
            return Err(format!(
                "Cannot send a message to yourself ({}).",
                op.to
            ));
        }

        self.callbacks
            .send_message(&op.to, &op.text)
            .await
            .map_err(|e| format!("Failed to send message: {e}"))?;

        Ok(format!("Message sent to @{}.", op.to))
    }

    async fn exec_submit(&self, op: &SubmitOp) -> Result<String, String> {
        if self.callbacks.caller_role() != "mate" {
            return Err(
                "Only the mate can submit work. Use message to communicate instead."
                    .to_owned(),
            );
        }

        self.callbacks
            .submit(&op.summary)
            .await
            .map_err(|e| format!("Submit failed: {e}"))?;

        Ok(format!("Work submitted for review: {}", op.summary))
    }

    // ── Helpers ──────────────────────────────────────────────────

    fn check_mutation(&self) -> Result<(), String> {
        self.callbacks
            .check_mutation_allowed()
            .map_err(|e| e.to_string())
    }

    fn format_empty_search(&self, op: &SearchOp) -> String {
        let mut msg = String::from("No matches found");
        if let Some(ref path) = op.path {
            msg.push_str(&format!(" in '{path}'"));
        }
        msg.push_str(&format!(" for query '{}'", op.query));
        if let Some(ref glob) = op.file_glob {
            msg.push_str(&format!(" (filtered to {glob})"));
        }
        msg.push_str(".\n\nHint: ");
        if op.case_sensitive {
            msg.push_str("try case_sensitive: false. ");
        }
        if op.file_glob.is_some() {
            msg.push_str("Try removing the file_glob filter. ");
        }
        if op.path.is_some() {
            msg.push_str("Try removing the path filter to search the whole worktree. ");
        }
        msg.push_str("The query is tried as regex first, then as literal text.");
        msg
    }

    fn suggest_symbols(&self, source: &str, _query: &str) -> String {
        let symbols = match crate::symbols::extract_rust_symbols(source) {
            Ok(s) => s,
            Err(_) => return String::new(),
        };

        if symbols.is_empty() {
            return String::new();
        }

        // Show up to 5 symbols as suggestions
        let suggestions: Vec<String> = symbols
            .iter()
            .filter(|s| s.name.is_some())
            .take(5)
            .map(|s| {
                format!(
                    "  {} {} (line {})",
                    s.kind,
                    s.name.as_deref().unwrap_or("?"),
                    s.start_line,
                )
            })
            .collect();

        if suggestions.is_empty() {
            return String::new();
        }

        format!("Available symbols:\n{}", suggestions.join("\n"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::callbacks::TestCallbacks;
    use ship_git::{BranchName, GitContext};

    async fn setup_engine() -> (Engine<TestCallbacks>, tempfile::TempDir) {
        setup_engine_with(TestCallbacks::mate()).await
    }

    async fn setup_engine_with(callbacks: TestCallbacks) -> (Engine<TestCallbacks>, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let worktree_path = camino::Utf8PathBuf::from(dir.path().to_str().unwrap());
        let git: GitContext = GitContext::init(worktree_path, &BranchName::new("main"))
            .await
            .unwrap();
        git.config_set("user.name", "Test").await.unwrap();
        git.config_set("user.email", "test@test.com").await.unwrap();

        std::fs::write(
            dir.path().join("lib.rs"),
            "fn hello() {\n    println!(\"hello\");\n}\n\nfn world() {\n    println!(\"world\");\n}\n",
        )
        .unwrap();
        git.add_all().await.unwrap();
        git.commit("initial").await.unwrap();

        let snapshots = SnapshotManager::new(git).await.unwrap();
        let engine = Engine::new(dir.path().to_owned(), snapshots, callbacks);
        (engine, dir)
    }

    // ── Undo error cases ─────────────────────────────────────────

    #[tokio::test]
    async fn undo_with_no_edits() {
        let (mut engine, _dir) = setup_engine().await;
        let result = engine
            .execute(&[Op::Undo(UndoOp { snapshot: 1 })])
            .await;
        insta::assert_snapshot!(result.format());
    }

    #[tokio::test]
    async fn undo_snapshot_zero() {
        let (mut engine, _dir) = setup_engine().await;
        engine.execute(&[Op::Edit(EditOp {
            file: "lib.rs".into(),
            edits: vec![Edit::FindReplace {
                find: "hello".into(),
                replace: "greetings".into(),
                replace_all: true,
            }],
        })]).await;
        let result = engine.execute(&[Op::Undo(UndoOp { snapshot: 0 })]).await;
        insta::assert_snapshot!(result.format());
    }

    #[tokio::test]
    async fn undo_snapshot_too_high() {
        let (mut engine, _dir) = setup_engine().await;
        engine.execute(&[Op::Edit(EditOp {
            file: "lib.rs".into(),
            edits: vec![Edit::FindReplace {
                find: "hello".into(),
                replace: "greetings".into(),
                replace_all: true,
            }],
        })]).await;
        let result = engine.execute(&[Op::Undo(UndoOp { snapshot: 99 })]).await;
        insta::assert_snapshot!(result.format());
    }

    // ── Edit error cases ─────────────────────────────────────────

    #[tokio::test]
    async fn edit_nonexistent_file() {
        let (mut engine, _dir) = setup_engine().await;
        let result = engine.execute(&[Op::Edit(EditOp {
            file: "nope.rs".into(),
            edits: vec![Edit::FindReplace {
                find: "x".into(),
                replace: "y".into(),
                replace_all: false,
            }],
        })]).await;
        insta::assert_snapshot!(result.format());
    }

    #[tokio::test]
    async fn edit_invalid_line_range() {
        let (mut engine, _dir) = setup_engine().await;
        let result = engine.execute(&[Op::Edit(EditOp {
            file: "lib.rs".into(),
            edits: vec![Edit::ReplaceLines {
                start: 0,
                end: 5,
                content: "bad".into(),
            }],
        })]).await;
        insta::assert_snapshot!(result.format());
    }

    #[tokio::test]
    async fn edit_find_replace_not_found() {
        let (mut engine, _dir) = setup_engine().await;
        let result = engine.execute(&[Op::Edit(EditOp {
            file: "lib.rs".into(),
            edits: vec![Edit::FindReplace {
                find: "nonexistent_string".into(),
                replace: "whatever".into(),
                replace_all: false,
            }],
        })]).await;
        insta::assert_snapshot!(result.format());
    }

    // ── Search ───────────────────────────────────────────────────

    #[tokio::test]
    async fn search_no_results_gives_hints() {
        let (mut engine, _dir) = setup_engine().await;
        let result = engine.execute(&[Op::Search(SearchOp {
            query: "zzz_nonexistent_zzz".into(),
            path: None,
            file_glob: Some("*.rs".into()),
            case_sensitive: true,
        })]).await;
        insta::assert_snapshot!(result.format());
    }

    // ── Read ─────────────────────────────────────────────────────

    #[tokio::test]
    async fn read_file_full() {
        let (mut engine, _dir) = setup_engine().await;
        let result = engine.execute(&[Op::Read(ReadOp {
            file: "lib.rs".into(),
            start_line: None,
            end_line: None,
        })]).await;
        insta::assert_snapshot!(result.format());
    }

    #[tokio::test]
    async fn read_file_range() {
        let (mut engine, _dir) = setup_engine().await;
        let result = engine.execute(&[Op::Read(ReadOp {
            file: "lib.rs".into(),
            start_line: Some(2),
            end_line: Some(4),
        })]).await;
        insta::assert_snapshot!(result.format());
    }

    #[tokio::test]
    async fn read_node_found() {
        let (mut engine, _dir) = setup_engine().await;
        let result = engine.execute(&[Op::ReadNode(ReadNodeOp {
            file: "lib.rs".into(),
            query: "fn hello".into(),
            offset: None,
            limit: None,
        })]).await;
        insta::assert_snapshot!(result.format());
    }

    #[tokio::test]
    async fn read_node_not_found_suggests() {
        let (mut engine, _dir) = setup_engine().await;
        let result = engine.execute(&[Op::ReadNode(ReadNodeOp {
            file: "lib.rs".into(),
            query: "fn nonexistent".into(),
            offset: None,
            limit: None,
        })]).await;
        insta::assert_snapshot!(result.format());
    }

    // ── Batch semantics ──────────────────────────────────────────

    #[tokio::test]
    async fn mutation_error_stops_batch() {
        let (mut engine, _dir) = setup_engine().await;
        let result = engine.execute(&[
            Op::Search(SearchOp {
                query: "hello".into(),
                path: None,
                file_glob: None,
                case_sensitive: false,
            }),
            Op::Edit(EditOp {
                file: "nope.rs".into(),
                edits: vec![Edit::FindReplace {
                    find: "x".into(),
                    replace: "y".into(),
                    replace_all: false,
                }],
            }),
            Op::Search(SearchOp {
                query: "world".into(),
                path: None,
                file_glob: None,
                case_sensitive: false,
            }),
        ]).await;
        assert!(result.stopped_early);
        assert_eq!(result.skipped, 1);
        assert_eq!(result.entries.len(), 2); // search + failed edit
    }

    #[tokio::test]
    async fn read_error_continues_batch() {
        let (mut engine, _dir) = setup_engine().await;
        let result = engine.execute(&[
            Op::Read(ReadOp {
                file: "nonexistent.rs".into(),
                start_line: None,
                end_line: None,
            }),
            Op::Search(SearchOp {
                query: "hello".into(),
                path: None,
                file_glob: None,
                case_sensitive: false,
            }),
        ]).await;
        assert!(!result.stopped_early);
        assert_eq!(result.entries.len(), 2);
        assert!(result.entries[0].is_error);
        assert!(!result.entries[1].is_error);
    }

    // ── Mutation lock ────────────────────────────────────────────

    #[tokio::test]
    async fn captain_locked_rejects_mutations() {
        let (mut engine, _dir) = setup_engine_with(TestCallbacks::captain_locked()).await;
        let result = engine.execute(&[Op::Edit(EditOp {
            file: "lib.rs".into(),
            edits: vec![Edit::FindReplace {
                find: "hello".into(),
                replace: "greetings".into(),
                replace_all: false,
            }],
        })]).await;
        let output = result.format();
        assert!(output.contains("mate is currently working"), "missing lock message: {output}");
    }

    #[tokio::test]
    async fn captain_locked_allows_reads() {
        let (mut engine, _dir) = setup_engine_with(TestCallbacks::captain_locked()).await;
        let result = engine.execute(&[Op::Read(ReadOp {
            file: "lib.rs".into(),
            start_line: None,
            end_line: None,
        })]).await;
        assert!(!result.entries[0].is_error);
    }

    // ── Communication ────────────────────────────────────────────

    #[tokio::test]
    async fn message_invalid_recipient() {
        let (mut engine, _dir) = setup_engine().await;
        let result = engine.execute(&[Op::Message(MessageOp {
            to: "nobody".into(),
            text: "hello".into(),
        })]).await;
        let output = result.format();
        assert!(output.contains("Unknown recipient"), "missing error: {output}");
    }

    #[tokio::test]
    async fn message_to_self_rejected() {
        let (mut engine, _dir) = setup_engine().await;
        let result = engine.execute(&[Op::Message(MessageOp {
            to: "mate".into(),
            text: "talking to myself".into(),
        })]).await;
        let output = result.format();
        assert!(output.contains("Cannot send a message to yourself"), "missing error: {output}");
    }

    #[tokio::test]
    async fn submit_only_for_mate() {
        let (mut engine, _dir) = setup_engine_with(TestCallbacks::captain()).await;
        let result = engine.execute(&[Op::Submit(SubmitOp {
            summary: "done".into(),
        })]).await;
        let output = result.format();
        assert!(output.contains("Only the mate"), "missing error: {output}");
    }

    // ── Successful edit shows snapshot ───────────────────────────

    #[tokio::test]
    async fn successful_edit_shows_snapshot() {
        let (mut engine, _dir) = setup_engine().await;
        let result = engine.execute(&[Op::Edit(EditOp {
            file: "lib.rs".into(),
            edits: vec![Edit::FindReplace {
                find: "hello".into(),
                replace: "greetings".into(),
                replace_all: false,
            }],
        })]).await;
        let output = result.format();
        assert!(output.contains("Snapshot: 1"), "missing snapshot: {output}");
        assert!(output.contains("undo available"), "missing undo hint: {output}");
    }

    #[tokio::test]
    async fn status_footer_always_present() {
        let (mut engine, _dir) = setup_engine().await;
        let result = engine.execute(&[Op::Search(SearchOp {
            query: "hello".into(),
            path: None,
            file_glob: None,
            case_sensitive: false,
        })]).await;
        let output = result.format();
        assert!(output.contains("[snapshot:"), "missing footer: {output}");
    }
}
