use std::path::{Path, PathBuf};

use eyre::Result;

use crate::edit;
use crate::ops::{Edit, EditOp, Op, SearchOp, UndoOp};
use crate::search;
use crate::snapshot::SnapshotManager;
use crate::structural;
use crate::symbols;
use crate::truncate;

/// The main engine that executes code operations.
/// Owns the snapshot manager and dispatches operations.
pub struct Engine {
    worktree: PathBuf,
    snapshots: SnapshotManager,
}

/// The result of executing a batch of operations.
/// Always includes status information to teach the agent.
pub struct BatchResult {
    /// Human-readable output for each operation.
    pub entries: Vec<String>,
    /// Current snapshot number after all operations.
    pub current_snapshot: u64,
    /// Total shadow commits since last real commit.
    pub shadow_count: u64,
}

impl BatchResult {
    /// Format the full response, including status footer.
    pub fn format(&self) -> String {
        let mut output = self.entries.join("\n---\n");

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

        if let Some(nudge) = self.nudge() {
            output.push_str(&format!("\n⚠ {nudge}"));
        }

        output
    }

    fn nudge(&self) -> Option<String> {
        if self.shadow_count >= 100 {
            Some(format!(
                "You have {} uncommitted edits. Consider committing.",
                self.shadow_count
            ))
        } else {
            None
        }
    }
}

/// Result of a single operation within a batch.
enum OpOutcome {
    /// Operation succeeded, here's the output text.
    Ok(String),
    /// Operation failed, here's a helpful error message.
    Err(String),
}

impl Engine {
    /// Create a new engine for the given worktree.
    pub async fn new(worktree: PathBuf, snapshots: SnapshotManager) -> Self {
        Self {
            worktree,
            snapshots,
        }
    }

    /// Execute a batch of operations.
    pub async fn execute(&mut self, ops: &[Op]) -> BatchResult {
        let mut entries = Vec::new();

        for op in ops {
            let outcome = match op {
                Op::Search(search_op) => self.exec_search(search_op),
                Op::Edit(edit_op) => self.exec_edit(edit_op).await,
                Op::Undo(undo_op) => self.exec_undo(undo_op).await,
            };

            match outcome {
                OpOutcome::Ok(text) => entries.push(text),
                OpOutcome::Err(text) => entries.push(format!("ERROR: {text}")),
            }
        }

        BatchResult {
            entries,
            current_snapshot: self.snapshots.shadow_count(),
            shadow_count: self.snapshots.shadow_count(),
        }
    }

    fn exec_search(&self, op: &SearchOp) -> OpOutcome {
        match search::search(&self.worktree, op) {
            Ok(output) => {
                let formatted = search::format_output(&output);
                if formatted.trim().is_empty() || formatted.contains("No matches found") {
                    // Provide helpful guidance on empty results
                    OpOutcome::Ok(self.format_empty_search(op))
                } else {
                    OpOutcome::Ok(formatted)
                }
            }
            Err(e) => OpOutcome::Err(format!(
                "Search failed: {e}\n\
                 Hint: the query is tried as regex first, then as literal text. \
                 Special characters like {{ }} ( ) are handled automatically."
            )),
        }
    }

    async fn exec_edit(&mut self, op: &EditOp) -> OpOutcome {
        let file_path = self.worktree.join(&op.file);

        // Read the file
        let source = match std::fs::read_to_string(&file_path) {
            Ok(s) => s,
            Err(e) => {
                return OpOutcome::Err(format!(
                    "Cannot read '{}': {e}\n\
                     Hint: paths are relative to the worktree root.",
                    op.file
                ));
            }
        };

        // Apply edits
        let (new_content, diff) = match edit::apply_edits(&source, Path::new(&op.file), &op.edits) {
            Ok(result) => result,
            Err(e) => {
                return OpOutcome::Err(format!(
                    "Edit failed on '{}': {e}\n\
                     Hint: line numbers are 1-indexed. The file has {} lines.",
                    op.file,
                    source.lines().count()
                ));
            }
        };

        // Write the file
        if let Err(e) = std::fs::write(&file_path, &new_content) {
            return OpOutcome::Err(format!("Failed to write '{}': {e}", op.file));
        }

        // Create shadow commit
        match self.snapshots.snapshot(&format!("edit {}", op.file)).await {
            Ok((snapshot, _)) => {
                OpOutcome::Ok(format!("{diff}\nSnapshot: {snapshot}"))
            }
            Err(e) => {
                OpOutcome::Err(format!(
                    "Edit applied but snapshot failed: {e}\n\
                     The file has been modified but undo may not work correctly."
                ))
            }
        }
    }

    async fn exec_undo(&mut self, op: &UndoOp) -> OpOutcome {
        let current = self.snapshots.shadow_count();

        if current == 0 {
            return OpOutcome::Err(
                "Nothing to undo. No edits have been made yet.".to_owned()
            );
        }

        if op.snapshot == 0 {
            return OpOutcome::Err(format!(
                "Snapshot 0 does not exist. Valid range: 1-{current}.\n\
                 Hint: use {{ \"op\": \"undo\", \"snapshot\": N }} where N is the \
                 snapshot you want to restore to."
            ));
        }

        if op.snapshot > current {
            return OpOutcome::Err(format!(
                "Snapshot {} does not exist. Current snapshot: {current}. \
                 Valid range: 1-{current}.\n\
                 Hint: you can only undo to snapshots that have already been created.",
                op.snapshot
            ));
        }

        match self.snapshots.undo(op.snapshot).await {
            Ok(diff) => {
                if diff.is_empty() {
                    OpOutcome::Ok(format!(
                        "Restored to snapshot {}. No diff (already at that state).",
                        op.snapshot
                    ))
                } else {
                    OpOutcome::Ok(format!(
                        "Restored to snapshot {}.\n\n{diff}",
                        op.snapshot
                    ))
                }
            }
            Err(e) => OpOutcome::Err(format!(
                "Undo to snapshot {} failed: {e}\n\
                 Current snapshot: {current}. Valid range: 1-{current}.",
                op.snapshot
            )),
        }
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use ship_git::{BranchName, GitContext};

    async fn setup_engine() -> (Engine, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let worktree_path = camino::Utf8PathBuf::from(dir.path().to_str().unwrap());
        let git: GitContext = GitContext::init(worktree_path, &BranchName::new("main"))
            .await
            .unwrap();
        git.config_set("user.name", "Test").await.unwrap();
        git.config_set("user.email", "test@test.com").await.unwrap();

        // Write a sample file and make initial commit
        std::fs::write(
            dir.path().join("lib.rs"),
            "fn hello() {\n    println!(\"hello\");\n}\n\nfn world() {\n    println!(\"world\");\n}\n",
        )
        .unwrap();
        git.add_all().await.unwrap();
        git.commit("initial").await.unwrap();

        let snapshots = SnapshotManager::new(git).await.unwrap();
        let engine = Engine::new(dir.path().to_owned(), snapshots).await;
        (engine, dir)
    }

    #[tokio::test]
    async fn undo_with_no_edits() {
        let (mut engine, _dir) = setup_engine().await;
        let result = engine
            .execute(&[Op::Undo(UndoOp { snapshot: 1 })])
            .await;
        let output = result.format();
        insta::assert_snapshot!(output);
    }

    #[tokio::test]
    async fn undo_snapshot_zero() {
        let (mut engine, _dir) = setup_engine().await;

        // Make an edit first so there's a snapshot
        engine
            .execute(&[Op::Edit(EditOp {
                file: "lib.rs".to_owned(),
                edits: vec![Edit::FindReplace {
                    find: "hello".to_owned(),
                    replace: "greetings".to_owned(),
                    replace_all: true,
                }],
            })])
            .await;

        let result = engine
            .execute(&[Op::Undo(UndoOp { snapshot: 0 })])
            .await;
        let output = result.format();
        insta::assert_snapshot!(output);
    }

    #[tokio::test]
    async fn undo_snapshot_too_high() {
        let (mut engine, _dir) = setup_engine().await;

        // Make one edit
        engine
            .execute(&[Op::Edit(EditOp {
                file: "lib.rs".to_owned(),
                edits: vec![Edit::FindReplace {
                    find: "hello".to_owned(),
                    replace: "greetings".to_owned(),
                    replace_all: true,
                }],
            })])
            .await;

        let result = engine
            .execute(&[Op::Undo(UndoOp { snapshot: 99 })])
            .await;
        let output = result.format();
        insta::assert_snapshot!(output);
    }

    #[tokio::test]
    async fn edit_nonexistent_file() {
        let (mut engine, _dir) = setup_engine().await;
        let result = engine
            .execute(&[Op::Edit(EditOp {
                file: "nope.rs".to_owned(),
                edits: vec![Edit::FindReplace {
                    find: "x".to_owned(),
                    replace: "y".to_owned(),
                    replace_all: false,
                }],
            })])
            .await;
        let output = result.format();
        insta::assert_snapshot!(output);
    }

    #[tokio::test]
    async fn edit_invalid_line_range() {
        let (mut engine, _dir) = setup_engine().await;
        let result = engine
            .execute(&[Op::Edit(EditOp {
                file: "lib.rs".to_owned(),
                edits: vec![Edit::ReplaceLines {
                    start: 0,
                    end: 5,
                    content: "bad".to_owned(),
                }],
            })])
            .await;
        let output = result.format();
        insta::assert_snapshot!(output);
    }

    #[tokio::test]
    async fn edit_find_replace_not_found() {
        let (mut engine, _dir) = setup_engine().await;
        let result = engine
            .execute(&[Op::Edit(EditOp {
                file: "lib.rs".to_owned(),
                edits: vec![Edit::FindReplace {
                    find: "nonexistent_string".to_owned(),
                    replace: "whatever".to_owned(),
                    replace_all: false,
                }],
            })])
            .await;
        let output = result.format();
        insta::assert_snapshot!(output);
    }

    #[tokio::test]
    async fn search_no_results_gives_hints() {
        let (mut engine, _dir) = setup_engine().await;
        let result = engine
            .execute(&[Op::Search(SearchOp {
                query: "zzz_nonexistent_zzz".to_owned(),
                path: None,
                file_glob: Some("*.rs".to_owned()),
                case_sensitive: true,
            })])
            .await;
        let output = result.format();
        insta::assert_snapshot!(output);
    }

    #[tokio::test]
    async fn successful_edit_shows_snapshot() {
        let (mut engine, _dir) = setup_engine().await;
        let result = engine
            .execute(&[Op::Edit(EditOp {
                file: "lib.rs".to_owned(),
                edits: vec![Edit::FindReplace {
                    find: "hello".to_owned(),
                    replace: "greetings".to_owned(),
                    replace_all: false,
                }],
            })])
            .await;
        let output = result.format();
        // Should contain the diff and snapshot info
        assert!(output.contains("Snapshot: 1"), "missing snapshot number: {output}");
        assert!(output.contains("undo available"), "missing undo hint: {output}");
    }

    #[tokio::test]
    async fn batch_multiple_operations() {
        let (mut engine, _dir) = setup_engine().await;
        let result = engine
            .execute(&[
                Op::Search(SearchOp {
                    query: "hello".to_owned(),
                    path: None,
                    file_glob: None,
                    case_sensitive: false,
                }),
                Op::Edit(EditOp {
                    file: "lib.rs".to_owned(),
                    edits: vec![Edit::FindReplace {
                        find: "hello".to_owned(),
                        replace: "greetings".to_owned(),
                        replace_all: true,
                    }],
                }),
            ])
            .await;
        assert_eq!(result.entries.len(), 2);
        assert!(result.current_snapshot >= 1);
    }

    #[tokio::test]
    async fn status_footer_always_present() {
        let (mut engine, _dir) = setup_engine().await;

        // Even a search-only result should have a status footer
        let result = engine
            .execute(&[Op::Search(SearchOp {
                query: "hello".to_owned(),
                path: None,
                file_glob: None,
                case_sensitive: false,
            })])
            .await;
        let output = result.format();
        assert!(output.contains("[snapshot:"), "missing status footer: {output}");
    }
}
