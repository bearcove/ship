use std::sync::Arc;

use ship_policy::{ParticipantName, PlanStep, RoomId};
use ship_runtime::Runtime;
use ship_tool_service::*;
use tokio::sync::Mutex;

/// Implementation of the ToolBackend roam service, backed by ship-runtime.
///
/// Each MCP server process gets its own roam connection with a ToolBackendImpl
/// scoped to a specific participant and room. The `caller` identity is set at
/// connection time, not per-call.
#[derive(Clone)]
pub struct ToolBackendImpl {
    runtime: Arc<Mutex<Runtime>>,
    participant: ParticipantName,
    room_id: RoomId,
}

impl ToolBackendImpl {
    pub fn new(
        runtime: Arc<Mutex<Runtime>>,
        participant: ParticipantName,
        room_id: RoomId,
    ) -> Self {
        Self {
            runtime,
            participant,
            room_id,
        }
    }
}

impl ToolBackend for ToolBackendImpl {
    // ── File I/O ────────────────────────────────────────────────────

    async fn read_file(&self, path: String, offset: Option<u64>, limit: Option<u64>) -> ToolResult {
        let rt = self.runtime.lock().await;
        let Some(git) = rt.git_context(&self.room_id) else {
            return tool_err("no worktree registered for this lane");
        };
        let file_path = git.worktree().join(&path);
        match tokio::fs::read_to_string(file_path.as_std_path()).await {
            Ok(content) => {
                let lines: Vec<&str> = content.lines().collect();
                let start = offset.unwrap_or(0) as usize;
                let end = limit
                    .map(|l| (start + l as usize).min(lines.len()))
                    .unwrap_or(lines.len());
                let slice = if start < lines.len() {
                    lines[start..end].join("\n")
                } else {
                    String::new()
                };
                tool_ok(slice)
            }
            Err(e) => tool_err(format!("read_file: {e}")),
        }
    }

    async fn write_file(&self, path: String, content: String) -> ToolResult {
        let rt = self.runtime.lock().await;
        let Some(git) = rt.git_context(&self.room_id) else {
            return tool_err("no worktree registered for this lane");
        };
        let file_path = git.worktree().join(&path);
        drop(rt);
        match tokio::fs::write(file_path.as_std_path(), &content).await {
            Ok(()) => tool_ok(format!("wrote {} bytes to {path}", content.len())),
            Err(e) => tool_err(format!("write_file: {e}")),
        }
    }

    async fn edit_prepare(
        &self,
        _path: String,
        _old_string: String,
        _new_string: String,
        _replace_all: bool,
    ) -> ToolResult {
        // TODO: integrate with ship-code edit engine
        tool_err("edit_prepare not yet implemented")
    }

    async fn edit_confirm(&self, _edit_id: String) -> ToolResult {
        tool_err("edit_confirm not yet implemented")
    }

    // ── Shell ───────────────────────────────────────────────────────

    async fn run_command(&self, command: String, cwd: Option<String>) -> ToolResult {
        let rt = self.runtime.lock().await;
        let Some(git) = rt.git_context(&self.room_id) else {
            return tool_err("no worktree registered for this lane");
        };
        let worktree = git.worktree().to_owned();
        drop(rt);

        let work_dir = match cwd {
            Some(ref d) => worktree.join(d),
            None => worktree,
        };

        let output = tokio::process::Command::new("sh")
            .arg("-c")
            .arg(&command)
            .current_dir(work_dir.as_std_path())
            .output()
            .await;

        match output {
            Ok(out) => {
                let stdout = String::from_utf8_lossy(&out.stdout);
                let stderr = String::from_utf8_lossy(&out.stderr);
                let text = if out.status.success() {
                    format!("{stdout}{stderr}")
                } else {
                    format!("exit {}\n{stdout}{stderr}", out.status)
                };
                ToolResult {
                    text,
                    is_error: !out.status.success(),
                }
            }
            Err(e) => tool_err(format!("run_command: {e}")),
        }
    }

    // ── Git ─────────────────────────────────────────────────────────

    async fn git_status(&self) -> ToolResult {
        let rt = self.runtime.lock().await;
        let Some(git) = rt.git_context(&self.room_id) else {
            return tool_err("no worktree registered for this lane");
        };
        let git = git.clone();
        drop(rt);

        match git.status().await {
            Ok(status) => {
                if status.is_clean() {
                    tool_ok("working tree clean")
                } else {
                    let lines: Vec<String> = status
                        .entries
                        .iter()
                        .map(|e| format!("{}{} {}", e.index, e.worktree, e.path))
                        .collect();
                    tool_ok(lines.join("\n"))
                }
            }
            Err(e) => tool_err(format!("git_status: {e}")),
        }
    }

    async fn review_diff(&self) -> ToolResult {
        let rt = self.runtime.lock().await;
        let Some(git) = rt.git_context(&self.room_id) else {
            return tool_err("no worktree registered for this lane");
        };
        let git = git.clone();
        drop(rt);

        match git.diff_cached().await {
            Ok(diff) => tool_ok(diff.into_string()),
            Err(e) => tool_err(format!("review_diff: {e}")),
        }
    }

    async fn commit(&self, message: String, _step_index: Option<u64>) -> ToolResult {
        let rt = self.runtime.lock().await;
        let Some(git) = rt.git_context(&self.room_id) else {
            return tool_err("no worktree registered for this lane");
        };
        let git = git.clone();
        drop(rt);

        match git.commit(&message).await {
            Ok(info) => tool_ok(format!("{} {}", info.hash, info.subject)),
            Err(e) => tool_err(format!("commit: {e}")),
        }
    }

    async fn rebase_status(&self) -> ToolResult {
        let rt = self.runtime.lock().await;
        let Some(git) = rt.git_context(&self.room_id) else {
            return tool_err("no worktree registered for this lane");
        };
        let git = git.clone();
        drop(rt);

        match git.is_rebasing().await {
            Ok(true) => match git.unmerged_files().await {
                Ok(files) => {
                    let list: Vec<String> = files.iter().map(|f| f.to_string()).collect();
                    tool_ok(format!("rebasing, conflicts:\n{}", list.join("\n")))
                }
                Err(e) => tool_err(format!("rebase_status: {e}")),
            },
            Ok(false) => tool_ok("no rebase in progress"),
            Err(e) => tool_err(format!("rebase_status: {e}")),
        }
    }

    async fn continue_rebase(&self) -> ToolResult {
        let rt = self.runtime.lock().await;
        let Some(git) = rt.git_context(&self.room_id) else {
            return tool_err("no worktree registered for this lane");
        };
        let git = git.clone();
        drop(rt);

        match git.rebase_continue().await {
            Ok(ship_git::RebaseOutcome::Success) => tool_ok("rebase completed"),
            Ok(ship_git::RebaseOutcome::Conflict { conflicting_files }) => {
                let list: Vec<String> = conflicting_files.iter().map(|f| f.to_string()).collect();
                tool_err(format!("conflicts remain:\n{}", list.join("\n")))
            }
            Err(e) => tool_err(format!("continue_rebase: {e}")),
        }
    }

    async fn abort_rebase(&self) -> ToolResult {
        let rt = self.runtime.lock().await;
        let Some(git) = rt.git_context(&self.room_id) else {
            return tool_err("no worktree registered for this lane");
        };
        let git = git.clone();
        drop(rt);

        match git.rebase_abort().await {
            Ok(()) => tool_ok("rebase aborted"),
            Err(e) => tool_err(format!("abort_rebase: {e}")),
        }
    }

    async fn diff_stats(&self) -> DiffStats {
        let rt = self.runtime.lock().await;
        let Some(git) = rt.git_context(&self.room_id) else {
            return DiffStats {
                lines_added: 0,
                lines_removed: 0,
                files_changed: 0,
            };
        };
        let git = git.clone();
        drop(rt);

        match git.diff_numstat_head().await {
            Ok(stats) => DiffStats {
                lines_added: stats.total_added() as u64,
                lines_removed: stats.total_removed() as u64,
                files_changed: stats.files_changed() as u64,
            },
            Err(_) => DiffStats {
                lines_added: 0,
                lines_removed: 0,
                files_changed: 0,
            },
        }
    }

    // ── Task lifecycle ──────────────────────────────────────────────

    async fn assign_task(&self, title: String, description: String) -> ToolResult {
        let mut rt = self.runtime.lock().await;
        match rt.assign_task(&self.room_id, title, description) {
            Ok(task_id) => tool_ok(format!("task assigned: {task_id}")),
            Err(e) => tool_err(format!("assign_task: {e}")),
        }
    }

    async fn submit(&self, summary: String) -> ToolResult {
        let mut rt = self.runtime.lock().await;
        match rt.transition_task(&self.room_id, ship_policy::TaskPhase::PendingReview) {
            Ok(()) => tool_ok(format!("submitted: {summary}")),
            Err(e) => tool_err(format!("submit: {e}")),
        }
    }

    async fn cancel_task(&self, reason: Option<String>) -> ToolResult {
        let mut rt = self.runtime.lock().await;
        match rt.transition_task(&self.room_id, ship_policy::TaskPhase::Cancelled) {
            Ok(()) => tool_ok(format!(
                "task cancelled{}",
                reason.map(|r| format!(": {r}")).unwrap_or_default()
            )),
            Err(e) => tool_err(format!("cancel_task: {e}")),
        }
    }

    async fn merge(&self, _summary: Option<String>) -> ToolResult {
        // TODO: git merge ff-only + transition to Accepted
        tool_err("merge not yet implemented")
    }

    // ── Plan ────────────────────────────────────────────────────────

    async fn set_plan(&self, _steps: Vec<PlanStep>) -> ToolResult {
        // TODO: store plan on the current task
        tool_err("set_plan not yet implemented")
    }

    // ── Messaging ───────────────────────────────────────────────────

    async fn send_message(&self, mention: ParticipantName, text: String) -> ToolResult {
        let mut rt = self.runtime.lock().await;
        let full_text = format!("@{mention} {text}");
        let block_id = match rt.open_block(
            &self.room_id,
            Some(self.participant.clone()),
            None,
            ship_policy::BlockContent::Text { text: full_text },
        ) {
            Ok(id) => id,
            Err(e) => return tool_err(format!("send_message: {e}")),
        };

        match rt.seal_block(&self.room_id, &block_id) {
            Ok(deliveries) => {
                let n = deliveries.len();
                if let Err(e) = rt.process_deliveries(deliveries) {
                    return tool_err(format!("send_message delivery: {e}"));
                }
                tool_ok(format!("sent, {n} delivery(ies) routed"))
            }
            Err(e) => tool_err(format!("send_message seal: {e}")),
        }
    }

    async fn ask(&self, question: String) -> ToolResult {
        // Asking is just sending a message to the captain with a special format.
        // The routing will handle it via policy.
        let mut rt = self.runtime.lock().await;
        let block_id = match rt.open_block(
            &self.room_id,
            Some(self.participant.clone()),
            None,
            ship_policy::BlockContent::Text {
                text: format!("[question] {question}"),
            },
        ) {
            Ok(id) => id,
            Err(e) => return tool_err(format!("ask: {e}")),
        };

        match rt.seal_block(&self.room_id, &block_id) {
            Ok(deliveries) => {
                if let Err(e) = rt.process_deliveries(deliveries) {
                    return tool_err(format!("ask delivery: {e}"));
                }
                tool_ok("question sent")
            }
            Err(e) => tool_err(format!("ask seal: {e}")),
        }
    }

    async fn notify_human(&self, message: String) -> ToolResult {
        // Create a block tagged for the human — routing handles delivery.
        let mut rt = self.runtime.lock().await;
        let human_name = rt.topology().human.name.clone();
        let full_text = format!("@{human_name} {message}");

        let block_id = match rt.open_block(
            &self.room_id,
            Some(self.participant.clone()),
            None,
            ship_policy::BlockContent::Text { text: full_text },
        ) {
            Ok(id) => id,
            Err(e) => return tool_err(format!("notify_human: {e}")),
        };

        match rt.seal_block(&self.room_id, &block_id) {
            Ok(deliveries) => {
                if let Err(e) = rt.process_deliveries(deliveries) {
                    return tool_err(format!("notify_human delivery: {e}"));
                }
                tool_ok("human notified")
            }
            Err(e) => tool_err(format!("notify_human seal: {e}")),
        }
    }

    async fn list_files(&self, query: String) -> Vec<String> {
        let rt = self.runtime.lock().await;
        let Some(git) = rt.git_context(&self.room_id) else {
            return Vec::new();
        };
        let git = git.clone();
        drop(rt);

        match git.ls_files().await {
            Ok(files) => {
                let q = query.to_lowercase();
                files
                    .into_iter()
                    .filter(|f| f.as_str().to_lowercase().contains(&q))
                    .map(|f| f.to_string())
                    .collect()
            }
            Err(_) => Vec::new(),
        }
    }

    // ── Unified code tool ───────────────────────────────────────────

    async fn code(&self, _ops_json: String) -> ToolResult {
        // TODO: integrate with ship-code engine
        tool_err("code tool not yet implemented")
    }
}

fn tool_ok(text: impl Into<String>) -> ToolResult {
    ToolResult {
        text: text.into(),
        is_error: false,
    }
}

fn tool_err(text: impl Into<String>) -> ToolResult {
    ToolResult {
        text: text.into(),
        is_error: true,
    }
}
