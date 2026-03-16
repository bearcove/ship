use facet::Facet;
use facet_mcp::{McpServer, McpServerInfo, Tool, ToolCtx};

// ── Args and result types ───────────────────────────────────────────

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

// ── Tool definition via macro ───────────────────────────────────────

facet_mcp::tool! {
    /// Add two numbers together and return the sum.
    async fn add(args: AddArgs, ctx: &ToolCtx) -> AddResult {
        let _ = ctx;
        Ok(AddResult { sum: args.a + args.b })
    }
}

// ── Tests ───────────────────────────────────────────────────────────

#[test]
fn tool_has_correct_name() {
    assert_eq!(add::name(), "add");
}

#[test]
fn tool_has_description_from_fn_doc() {
    assert_eq!(add::description().trim(), "Add two numbers together and return the sum.");
}

#[test]
fn tool_input_schema_has_fields() {
    let schema = facet_mcp::schema_for::<AddArgs>();
    let json = facet_json::to_string(&schema).unwrap();
    assert!(json.contains("\"a\""));
    assert!(json.contains("\"b\""));
    assert!(json.contains("integer"));
}

#[test]
fn tool_output_schema_has_fields() {
    let schema = facet_mcp::schema_for::<AddResult>();
    let json = facet_json::to_string(&schema).unwrap();
    assert!(json.contains("\"sum\""));
    assert!(json.contains("integer"));
}

#[tokio::test]
async fn call_directly() {
    let ctx = ToolCtx::new();
    let result = add::call(AddArgs { a: 10, b: 20 }, &ctx).await.unwrap();
    assert_eq!(result.sum, 30);
}

// ── Server builder tests ────────────────────────────────────────────

struct Greeting(String);

#[derive(Debug, Facet)]
struct GreetArgs {
    name: String,
}

#[derive(Debug, Facet)]
struct GreetResult {
    message: String,
}

facet_mcp::tool! {
    /// Greet someone by name.
    async fn greet(args: GreetArgs, ctx: &ToolCtx) -> GreetResult {
        let greeting = ctx.get::<Greeting>();
        Ok(GreetResult {
            message: format!("{} {}", greeting.0, args.name),
        })
    }
}

#[test]
fn server_builder_collects_tools() {
    let ctx = ToolCtx::new();
    let server = McpServer::new(ctx, McpServerInfo {
        name: "test".to_owned(),
        version: "0.1".to_owned(),
    })
    .tool::<add>()
    .tool::<greet>();

    // We can't inspect tools directly since they're private,
    // but building without panic proves registration works.
    let _ = server;
}
