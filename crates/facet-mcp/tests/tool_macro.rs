use facet::Facet;
use facet_mcp::{CallToolResult, ToolCtx};

// ── Args and result types ───────────────────────────────────────────

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

// ── Tool definition via macro ───────────────────────────────────────

facet_mcp::tool! {
    async fn add(args: AddArgs, ctx: &ToolCtx) -> AddResult {
        let _ = ctx; // unused in this simple example
        AddResult { sum: args.a + args.b }
    }
}

// ── Tests ───────────────────────────────────────────────────────────

#[test]
fn tool_def_has_correct_name() {
    let def = add::def();
    assert_eq!(def.name, "add");
}

#[test]
fn tool_def_has_description_from_args_doc() {
    let def = add::def();
    assert_eq!(def.description, "Add two numbers together.");
}

#[test]
fn tool_def_input_schema_has_fields() {
    let def = add::def();
    let json = facet_json::to_string(&def.input_schema).unwrap();
    assert!(json.contains("\"a\""));
    assert!(json.contains("\"b\""));
    assert!(json.contains("integer"));
}

#[test]
fn tool_def_output_schema_has_fields() {
    let def = add::def();
    let json = facet_json::to_string(&def.output_schema).unwrap();
    assert!(json.contains("\"sum\""));
    assert!(json.contains("integer"));
}

#[tokio::test]
async fn dispatch_valid_args() {
    let ctx = ToolCtx::new();
    let raw = facet_json::RawJson::from_owned(r#"{"a": 3, "b": 4}"#.to_owned());
    let result = add::dispatch(&raw, &ctx).await;
    assert!(result.is_error.is_none());
    // result.content[0] should be text containing the serialized AddResult
    match &result.content[0] {
        facet_mcp::ContentBlock::Text { text } => {
            assert!(text.contains("7"));
        }
    }
}

#[tokio::test]
async fn dispatch_invalid_args() {
    let ctx = ToolCtx::new();
    let raw = facet_json::RawJson::from_owned(r#"{"x": 1}"#.to_owned());
    let result = add::dispatch(&raw, &ctx).await;
    assert_eq!(result.is_error, Some(true));
}

#[tokio::test]
async fn call_directly() {
    let ctx = ToolCtx::new();
    let result = add::call(AddArgs { a: 10, b: 20 }, &ctx).await;
    assert_eq!(result.sum, 30);
}
