use facet::Facet;
use facet_mcp::{McpServer, McpServerInfo, ToolCtx};

/// Add two numbers together.
#[derive(Debug, Facet)]
struct AddArgs {
    /// First operand
    a: i64,
    /// Second operand
    b: i64,
}

#[derive(Debug, Facet)]
struct AddResult {
    /// The sum
    sum: i64,
}

/// Reverse a string.
#[derive(Debug, Facet)]
struct ReverseArgs {
    /// The string to reverse
    input: String,
}

#[derive(Debug, Facet)]
struct ReverseResult {
    /// The reversed string
    output: String,
}

facet_mcp::tool! {
    /// Add two numbers together and return the sum.
    async fn add(args: AddArgs, ctx: &ToolCtx) -> AddResult {
        let _ = ctx;
        AddResult { sum: args.a + args.b }
    }
}

facet_mcp::tool! {
    /// Reverse a string.
    async fn reverse(args: ReverseArgs, ctx: &ToolCtx) -> ReverseResult {
        let _ = ctx;
        ReverseResult {
            output: args.input.chars().rev().collect(),
        }
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_writer(std::io::stderr)
        .init();
    let ctx = ToolCtx::new();
    let server = McpServer::new(ctx, McpServerInfo {
        name: "facet-mcp-test".to_owned(),
        version: "0.1.0".to_owned(),
    })
    .tool::<add>()
    .tool::<reverse>();

    server.run().await.expect("server failed");
}
