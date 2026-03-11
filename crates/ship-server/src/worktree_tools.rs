use rust_mcp_sdk::schema::{Tool, ToolInputSchema};
use serde_json::{Value, json};

#[derive(Clone)]
pub struct ToolDefinition {
    pub name: &'static str,
    pub description: &'static str,
    pub input_schema: Value,
}

pub fn to_sdk_tool(tool: &ToolDefinition) -> Tool {
    Tool {
        annotations: None,
        description: Some(tool.description.to_owned()),
        execution: None,
        icons: Vec::new(),
        input_schema: serde_json::from_value::<ToolInputSchema>(tool.input_schema.clone())
            .expect("tool schema should be a valid MCP input schema"),
        meta: None,
        name: tool.name.to_owned(),
        output_schema: None,
        title: None,
    }
}

pub fn search_files_tool() -> ToolDefinition {
    ToolDefinition {
        name: "search_files",
        description: "Search file contents using ripgrep. Returns line-numbered matches.\n\nExamples:\n  pattern=\"fn handle_event\", path=\"src/\"\n  pattern=\"TODO\", path=\"crates/ship-core/src/session_manager.rs\"",
        input_schema: json!({
            "type": "object",
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "Search pattern (regex)"
                },
                "path": {
                    "type": "string",
                    "description": "File or directory to search (optional, defaults to worktree root)"
                }
            },
            "required": ["pattern"],
            "additionalProperties": false,
        }),
    }
}

pub fn list_files_tool() -> ToolDefinition {
    ToolDefinition {
        name: "list_files",
        description: "List files in the worktree.\n\nExamples:\n  path=\"crates/ship-core/src/\"\n  extension=\"rs\", path=\"crates/\"\n  pattern=\"*_test*\", extension=\"rs\"",
        input_schema: json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Directory to list (optional, defaults to worktree root)"
                },
                "pattern": {
                    "type": "string",
                    "description": "Filename pattern to filter results (optional)"
                },
                "extension": {
                    "type": "string",
                    "description": "File extension to filter by, e.g. \"rs\", \"ts\" (optional)"
                }
            },
            "additionalProperties": false,
        }),
    }
}

pub fn parse_search_files_args(arguments: &Value) -> Option<(String, Option<String>)> {
    let pattern = arguments.get("pattern")?.as_str()?.to_owned();
    let path = arguments
        .get("path")
        .and_then(Value::as_str)
        .map(str::to_owned);
    Some((pattern, path))
}

pub fn parse_list_files_args(
    arguments: &Value,
) -> (Option<String>, Option<String>, Option<String>) {
    let path = arguments
        .get("path")
        .and_then(Value::as_str)
        .map(str::to_owned);
    let pattern = arguments
        .get("pattern")
        .and_then(Value::as_str)
        .map(str::to_owned);
    let extension = arguments
        .get("extension")
        .and_then(Value::as_str)
        .map(str::to_owned);
    (path, pattern, extension)
}
