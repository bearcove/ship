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

/// Tool definitions shared between captain and mate MCP servers.
pub fn run_command_tool() -> ToolDefinition {
    ToolDefinition {
        name: "run_command",
        description: "Run a shell command via sh -c in the current session worktree. \
Pipes, redirects, and shell syntax work directly — do NOT escape them. \
Use rg instead of grep and fd instead of find. rg uses modern regex syntax where | means alternation — do NOT backslash-escape it \
(e.g. `rg 'foo|bar'`, not `rg 'foo\\|bar'`). \
Omit cwd unless the task explicitly targets a subdirectory inside the current worktree. \
Do not pass repo-root paths or `.ship/...` prefixes.",
        input_schema: json!({
            "type": "object",
            "properties": {
                "command": { "type": "string" },
                "cwd": { "type": "string", "description": "Worktree-relative subdirectory to run in (optional)." }
            },
            "required": ["command"],
            "additionalProperties": false,
        }),
    }
}

pub fn web_search_tool() -> ToolDefinition {
    ToolDefinition {
        name: "web_search",
        description: "Search the web using Kagi FastGPT. Returns an AI-synthesized answer and a list of references.",
        input_schema: json!({
            "type": "object",
            "properties": {
                "query": { "type": "string" }
            },
            "required": ["query"],
            "additionalProperties": false,
        }),
    }
}

pub fn read_file_tool() -> ToolDefinition {
    ToolDefinition {
        name: "read_file",
        description: "Read a file in the current session worktree. Returns numbered lines. \
Paths are worktree-relative; do not pass repo-root paths or `.ship/...` prefixes. \
Use offset/limit to page through large files.",
        input_schema: json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Worktree-relative path." },
                "offset": { "type": "integer", "minimum": 1, "description": "1-based line to start from." },
                "limit": { "type": "integer", "minimum": 1, "description": "Maximum number of lines to return." }
            },
            "required": ["path"],
            "additionalProperties": false,
        }),
    }
}
