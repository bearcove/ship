use facet::Facet;
use facet_mcp::{McpServer, McpServerInfo, ToolCtx, ToolError};
use ship_service::CaptainMcpClient;
use ship_types::{AssignFileRef, CaptainAssignExtras, DirtySessionStrategy, PlanStepInput};

use super::code::{CodeArgs, CodeResult};

// ── Shared result type ──────────────────────────────────────────────

#[derive(Debug, Facet)]
pub struct TextResult {
    /// The response text.
    pub text: String,
}

// ── captain_assign ──────────────────────────────────────────────────

#[derive(Debug, Facet)]
pub struct CaptainAssignArgs {
    /// Short title for the task (under 60 chars). Shown in the UI sidebar and headers.
    title: String,
    /// Full task description with all details the mate needs.
    description: String,
    /// Reuse the mate's existing context (default false).
    #[facet(default)]
    keep: Option<bool>,
    /// Required when the session branch or worktree has leftover state.
    #[facet(default)]
    dirty_session_strategy: Option<DirtySessionStrategyArg>,
    /// Files to inline into the mate's prompt.
    #[facet(default)]
    files: Vec<FileRef>,
    /// Pre-built plan steps. If supplied, the mate skips research and goes directly to execution.
    #[facet(default)]
    plan: Vec<PlanStepArg>,
}

/// Strategy for handling leftover session state.
#[derive(Debug, Facet)]
#[repr(u8)]
#[facet(rename_all = "snake_case")]
pub enum DirtySessionStrategyArg {
    /// Continue the new task in the current worktree with the leftover state intact.
    ContinueInPlace,
    /// Save the leftover state on a timestamped branch, then reset before starting.
    SaveAndStartClean,
}

#[derive(Debug, Facet)]
pub struct FileRef {
    /// Worktree-relative file path.
    path: String,
    /// 1-based first line to include.
    #[facet(default)]
    start_line: Option<u64>,
    /// 1-based last line to include.
    #[facet(default)]
    end_line: Option<u64>,
}

#[derive(Debug, Facet)]
pub struct PlanStepArg {
    /// Short summary of the step (like a commit subject line).
    title: String,
    /// Longer explanation of what the step involves.
    description: String,
}

facet_mcp::tool! {
    /// Assign a task to the mate. The mate will start working on it immediately.
    /// Set keep=true to reuse the mate's existing context; omit or set false to restart the mate with a fresh context (default).
    /// If the session already has leftover branch or worktree state, pass dirty_session_strategy to choose whether to continue in place or save that state and start clean.
    /// IMPORTANT: Always pass files and plan. Every file you read during research must be listed in files — the mate
    /// receives the contents directly and skips re-reading them. Your step-by-step plan must be passed via plan — the mate
    /// skips research and goes straight to execution. Omitting files or plan wastes the mate's time and context window.
    pub(crate) async fn captain_assign(args: CaptainAssignArgs, ctx: &ToolCtx) -> TextResult {
        let client = ctx.get::<CaptainMcpClient>();
        let dirty_session_strategy = args.dirty_session_strategy.map(|s| match s {
            DirtySessionStrategyArg::ContinueInPlace => DirtySessionStrategy::ContinueInPlace,
            DirtySessionStrategyArg::SaveAndStartClean => DirtySessionStrategy::SaveAndStartClean,
        });
        let files = args
            .files
            .into_iter()
            .map(|f| AssignFileRef {
                path: f.path,
                start_line: f.start_line,
                end_line: f.end_line,
            })
            .collect();
        let plan = args
            .plan
            .into_iter()
            .map(|s| PlanStepInput {
                title: s.title,
                description: s.description,
            })
            .collect();
        let resp = client
            .captain_assign(
                args.title,
                args.description,
                args.keep.unwrap_or(false),
                CaptainAssignExtras {
                    files,
                    plan,
                    dirty_session_strategy,
                },
            )
            .await
            .map_err(rpc_err)?;
        rpc_result(resp)
    }
}

// ── captain_steer ───────────────────────────────────────────────────

#[derive(Debug, Facet)]
pub struct CaptainSteerArgs {
    /// Message to send to the mate.
    message: String,
    /// Replace the entire plan with these steps.
    #[facet(default)]
    new_plan: Option<Vec<PlanStepArg>>,
    /// Append these steps to the existing plan.
    #[facet(default)]
    add_steps: Option<Vec<PlanStepArg>>,
}

facet_mcp::tool! {
    /// Send direction to the mate on the current task. Fire-and-forget: returns immediately.
    /// Optionally provide new_plan to replace the entire plan or add_steps to append steps — at most one may be provided.
    pub(crate) async fn captain_steer(args: CaptainSteerArgs, ctx: &ToolCtx) -> TextResult {
        let client = ctx.get::<CaptainMcpClient>();
        let new_plan = args.new_plan.map(|steps| {
            steps
                .into_iter()
                .map(|s| PlanStepInput {
                    title: s.title,
                    description: s.description,
                })
                .collect()
        });
        let add_steps = args.add_steps.map(|steps| {
            steps
                .into_iter()
                .map(|s| PlanStepInput {
                    title: s.title,
                    description: s.description,
                })
                .collect()
        });
        let resp = client
            .captain_steer(args.message, new_plan, add_steps)
            .await
            .map_err(rpc_err)?;
        rpc_result(resp)
    }
}

// ── captain_merge ───────────────────────────────────────────────────

#[derive(Debug, Facet)]
pub struct CaptainMergeArgs {
    /// Summary of the changes being merged.
    #[facet(default)]
    summary: Option<String>,
}

facet_mcp::tool! {
    /// Merge the session branch into the base branch. Works with or without an active task — if there is no task
    /// but the session branch has commits ahead of base, it merges those. Ship handles the rebase/merge flow;
    /// do not try to do that manually with git.
    pub(crate) async fn captain_merge(args: CaptainMergeArgs, ctx: &ToolCtx) -> TextResult {
        let client = ctx.get::<CaptainMcpClient>();
        let resp = client.captain_merge(args.summary).await.map_err(rpc_err)?;
        rpc_result(resp)
    }
}

// ── captain_cancel ──────────────────────────────────────────────────

#[derive(Debug, Facet)]
pub struct CaptainCancelArgs {
    /// Reason for cancellation.
    #[facet(default)]
    reason: Option<String>,
}

facet_mcp::tool! {
    /// Cancel the current task.
    pub(crate) async fn captain_cancel(args: CaptainCancelArgs, ctx: &ToolCtx) -> TextResult {
        let client = ctx.get::<CaptainMcpClient>();
        let resp = client.captain_cancel(args.reason).await.map_err(rpc_err)?;
        rpc_result(resp)
    }
}

// ── captain_git_status ──────────────────────────────────────────────

#[derive(Debug, Facet)]
pub struct CaptainGitStatusArgs {}

facet_mcp::tool! {
    /// Inspect the current session branch state before review or accept. Reports the current branch,
    /// base branch, dirtiness, rebase state, unresolved paths, and tracked conflict markers.
    pub(crate) async fn captain_git_status(args: CaptainGitStatusArgs, ctx: &ToolCtx) -> TextResult {
        let _ = args;
        let client = ctx.get::<CaptainMcpClient>();
        let resp = client.captain_git_status().await.map_err(rpc_err)?;
        rpc_result(resp)
    }
}

// ── captain_review_diff ─────────────────────────────────────────────

#[derive(Debug, Facet)]
pub struct CaptainReviewDiffArgs {}

facet_mcp::tool! {
    /// Rebase the session branch onto the configured base branch and return the post-rebase diff
    /// that would merge right now. If the rebase conflicts, Ship leaves the rebase in progress and
    /// reports the conflicted files instead of returning a diff.
    pub(crate) async fn captain_review_diff(args: CaptainReviewDiffArgs, ctx: &ToolCtx) -> TextResult {
        let _ = args;
        let client = ctx.get::<CaptainMcpClient>();
        let resp = client.captain_review_diff().await.map_err(rpc_err)?;
        rpc_result(resp)
    }
}

// ── captain_rebase_status ───────────────────────────────────────────

#[derive(Debug, Facet)]
pub struct CaptainRebaseStatusArgs {}

facet_mcp::tool! {
    /// Inspect the current rebase state, including whether a rebase is in progress
    /// and whether it is safe to continue or abort.
    pub(crate) async fn captain_rebase_status(args: CaptainRebaseStatusArgs, ctx: &ToolCtx) -> TextResult {
        let _ = args;
        let client = ctx.get::<CaptainMcpClient>();
        let resp = client.captain_rebase_status().await.map_err(rpc_err)?;
        rpc_result(resp)
    }
}

// ── captain_notify_human ────────────────────────────────────────────

#[derive(Debug, Facet)]
pub struct CaptainNotifyHumanArgs {
    /// Message to send to the human.
    message: String,
}

facet_mcp::tool! {
    /// Ask the human for guidance using the same post-rebase review diff that Ship would merge right now.
    /// Blocks until the human responds.
    pub(crate) async fn captain_notify_human(args: CaptainNotifyHumanArgs, ctx: &ToolCtx) -> TextResult {
        let client = ctx.get::<CaptainMcpClient>();
        let resp = client
            .captain_notify_human(args.message)
            .await
            .map_err(rpc_err)?;
        rpc_result(resp)
    }
}

// ── captain_continue_rebase ─────────────────────────────────────────

#[derive(Debug, Facet)]
pub struct CaptainContinueRebaseArgs {}

facet_mcp::tool! {
    /// Continue a paused rebase after resolving conflicts. Ship refuses to continue while
    /// unmerged paths remain or tracked files still contain conflict markers.
    pub(crate) async fn captain_continue_rebase(args: CaptainContinueRebaseArgs, ctx: &ToolCtx) -> TextResult {
        let _ = args;
        let client = ctx.get::<CaptainMcpClient>();
        let resp = client.captain_continue_rebase().await.map_err(rpc_err)?;
        rpc_result(resp)
    }
}

// ── captain_abort_rebase ────────────────────────────────────────────

#[derive(Debug, Facet)]
pub struct CaptainAbortRebaseArgs {}

facet_mcp::tool! {
    /// Abort the in-progress rebase and return the session worktree to its pre-rebase state.
    pub(crate) async fn captain_abort_rebase(args: CaptainAbortRebaseArgs, ctx: &ToolCtx) -> TextResult {
        let _ = args;
        let client = ctx.get::<CaptainMcpClient>();
        let resp = client.captain_abort_rebase().await.map_err(rpc_err)?;
        rpc_result(resp)
    }
}

// ── code (captain variant) ──────────────────────────────────────────

facet_mcp::tool! {
    /// Execute one or more code operations in a single batch.
    /// Operations are executed in order. Read-only ops continue on failure;
    /// mutation ops stop the batch on the first error. Every mutation creates an undo snapshot.
    /// Use this tool for ALL file operations — search, read, edit, run commands, and commit.
    pub(crate) async fn code(args: CodeArgs, ctx: &ToolCtx) -> CodeResult {
        let client = ctx.get::<CaptainMcpClient>();
        let ops_json = facet_json::to_string(&args.ops)
            .map_err(|e| ToolError::new(format!("failed to serialize ops: {e}")))?;
        let resp = client.captain_code(ops_json).await.map_err(rpc_err)?;
        if resp.is_error {
            return Err(ToolError::new(resp.text));
        }
        Ok(CodeResult {
            text: resp.text,
            diffs: resp.diffs,
        })
    }
}

// ── Server builder ──────────────────────────────────────────────────

pub fn captain_server(ctx: ToolCtx) -> McpServer {
    McpServer::new(
        ctx,
        McpServerInfo {
            name: "ship".to_owned(),
            version: env!("CARGO_PKG_VERSION").to_owned(),
        },
    )
    .tool::<captain_assign>()
    .tool::<captain_steer>()
    .tool::<captain_merge>()
    .tool::<captain_cancel>()
    .tool::<captain_git_status>()
    .tool::<captain_review_diff>()
    .tool::<captain_rebase_status>()
    .tool::<captain_notify_human>()
    .tool::<captain_continue_rebase>()
    .tool::<captain_abort_rebase>()
    .tool::<code>()
    .tool::<super::shared::web_search>()
}

fn rpc_err(e: impl std::fmt::Debug) -> ToolError {
    ToolError::new(format!("{e:?}"))
}

fn rpc_result(resp: ship_types::McpToolCallResponse) -> Result<TextResult, ToolError> {
    if resp.is_error {
        Err(ToolError::new(resp.text))
    } else {
        Ok(TextResult { text: resp.text })
    }
}
