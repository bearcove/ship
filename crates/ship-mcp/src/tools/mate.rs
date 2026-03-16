use facet::Facet;
use facet_mcp::{McpServer, McpServerInfo, ToolCtx, ToolError};
use ship_service::MateMcpClient;
use ship_types::PlanStepInput;

use super::code::{CodeArgs, CodeResult};

// ── Shared result type ──────────────────────────────────────────────

#[derive(Debug, Facet)]
pub struct TextResult {
    /// The response text.
    pub text: String,
}

// ── set_plan ────────────────────────────────────────────────────────

#[derive(Debug, Facet)]
pub struct PlanStepArg {
    /// Short summary of the step (like a commit subject line).
    title: String,
    /// Longer explanation of what the step involves.
    description: String,
}

#[derive(Debug, Facet)]
pub struct SetPlanArgs {
    /// The plan steps.
    steps: Vec<PlanStepArg>,
}

facet_mcp::tool! {
    /// Set (or change) the work plan. First call sets the plan and notifies the captain non-blocking.
    /// Subsequent calls mid-task are a blocking request: the captain must approve or reject the change
    /// before the mate can continue. Use this if you discover the scope has changed.
    pub(crate) async fn set_plan(args: SetPlanArgs, ctx: &ToolCtx) -> TextResult {
        let client = ctx.get::<MateMcpClient>();
        let steps: Vec<PlanStepInput> = args
            .steps
            .into_iter()
            .map(|s| PlanStepInput {
                title: s.title,
                description: s.description,
            })
            .collect();
        let resp = client.set_plan(steps).await.map_err(rpc_err)?;
        rpc_result(resp)
    }
}

// ── mate_ask_captain ────────────────────────────────────────────────

#[derive(Debug, Facet)]
pub struct AskCaptainArgs {
    /// The question to ask the captain.
    question: String,
}

facet_mcp::tool! {
    /// Ask the captain a question and wait for their response.
    /// Blocks until the captain replies via captain_steer.
    pub(crate) async fn mate_ask_captain(args: AskCaptainArgs, ctx: &ToolCtx) -> TextResult {
        let client = ctx.get::<MateMcpClient>();
        let resp = client
            .mate_ask_captain(args.question)
            .await
            .map_err(rpc_err)?;
        rpc_result(resp)
    }
}

// ── code (mate variant) ─────────────────────────────────────────────

facet_mcp::tool! {
    /// Execute one or more code operations in a single batch.
    /// Operations are executed in order. Read-only ops continue on failure;
    /// mutation ops stop the batch on the first error. Every mutation creates an undo snapshot.
    /// Use this tool for ALL file operations — search, read, edit, run commands, and commit.
    pub(crate) async fn code(args: CodeArgs, ctx: &ToolCtx) -> CodeResult {
        let client = ctx.get::<MateMcpClient>();
        let ops_json = facet_json::to_string(&args.ops)
            .map_err(|e| ToolError::new(format!("failed to serialize ops: {e}")))?;
        let resp = client.code(ops_json).await.map_err(rpc_err)?;
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

pub fn mate_server(ctx: ToolCtx) -> McpServer {
    McpServer::new(
        ctx,
        McpServerInfo {
            name: "ship".to_owned(),
            version: env!("CARGO_PKG_VERSION").to_owned(),
        },
    )
    .tool::<set_plan>()
    .tool::<mate_ask_captain>()
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
