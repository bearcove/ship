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

pub fn write_file_tool() -> ToolDefinition {
    ToolDefinition {
        name: "write_file",
        description: "Write a file in the current task worktree. Rust files are syntax-checked with rustfmt and auto-formatted before the write is committed.",
        input_schema: json!({
            "type": "object",
            "properties": {
                "path": { "type": "string" },
                "content": { "type": "string" }
            },
            "required": ["path", "content"],
            "additionalProperties": false,
        }),
    }
}

pub fn edit_prepare_tool() -> ToolDefinition {
    ToolDefinition {
        name: "edit_prepare",
        description: "Prepare a search-and-replace edit. Returns a diff preview without modifying the file. The response includes an edit_id in the structured content (diff.edit_id) and in the text. You MUST call edit_confirm with that edit_id to apply the edit.",
        input_schema: json!({
            "type": "object",
            "properties": {
                "path": { "type": "string" },
                "old_string": { "type": "string" },
                "new_string": { "type": "string" },
                "replace_all": { "type": "boolean" }
            },
            "required": ["path", "old_string", "new_string"],
            "additionalProperties": false,
        }),
    }
}

pub fn edit_confirm_tool() -> ToolDefinition {
    ToolDefinition {
        name: "edit_confirm",
        description: "Apply a previously prepared edit. Pass the edit_id exactly as returned by edit_prepare (from the structured content diff.edit_id field, or from the text response). Runs syntax validation for Rust files. If validation fails, the file is not modified and the error is returned.",
        input_schema: json!({
            "type": "object",
            "properties": {
                "edit_id": { "type": "string" }
            },
            "required": ["edit_id"],
            "additionalProperties": false,
        }),
    }
}

fn op_schema(name: &str, props: Value, required: &[&str]) -> Value {
    let req: Vec<Value> = required.iter().map(|s| json!(s)).collect();
    let mut op_obj = serde_json::Map::new();
    op_obj.insert("type".into(), json!("object"));
    op_obj.insert("properties".into(), props);
    op_obj.insert("required".into(), Value::Array(req));

    let mut wrapper = serde_json::Map::new();
    wrapper.insert(
        "properties".into(),
        json!({ name: Value::Object(op_obj) }),
    );
    wrapper.insert("required".into(), json!([name]));
    Value::Object(wrapper)
}

fn code_tool_op_schemas() -> Vec<Value> {
    vec![
        op_schema(
            "search",
            json!({
                "query": { "type": "string", "description": "Regex or literal search query." },
                "path": { "type": "string", "description": "Scope search to this directory." },
                "file_glob": { "type": "string", "description": "File glob filter, e.g. '*.rs'." },
                "case_sensitive": { "type": "boolean" }
            }),
            &["query"],
        ),
        op_schema(
            "read",
            json!({
                "file": { "type": "string", "description": "Worktree-relative path." },
                "start_line": { "type": "integer", "description": "1-indexed start line." },
                "end_line": { "type": "integer", "description": "1-indexed end line." }
            }),
            &["file"],
        ),
        op_schema(
            "read_node",
            json!({
                "file": { "type": "string" },
                "query": { "type": "string", "description": "Symbol query, e.g. 'fn handle_request' or 'impl Server'." },
                "offset": { "type": "integer", "description": "Line offset within symbol body." },
                "limit": { "type": "integer", "description": "Max lines to return." }
            }),
            &["file", "query"],
        ),
        op_schema(
            "edit",
            json!({
                "file": { "type": "string" },
                "edits": {
                    "type": "array",
                    "items": { "type": "object", "description": "One of: {find_replace: {find, replace, replace_all?}}, {replace_lines: {start, end, content}}, {insert_lines: {before, content}}, {delete_lines: {start, end}}" }
                }
            }),
            &["file", "edits"],
        ),
        op_schema(
            "replace_node",
            json!({
                "file": { "type": "string" },
                "query": { "type": "string", "description": "Symbol query to find and replace." },
                "content": { "type": "string", "description": "New source code for the symbol." }
            }),
            &["file", "query", "content"],
        ),
        op_schema(
            "delete_node",
            json!({
                "file": { "type": "string" },
                "query": { "type": "string", "description": "Symbol query to delete." }
            }),
            &["file", "query"],
        ),
        op_schema(
            "run",
            json!({
                "command": { "type": "string", "description": "Shell command (passed to sh -c)." },
                "cwd": { "type": "string", "description": "Worktree-relative working directory." },
                "timeout_secs": { "type": "integer", "description": "Timeout in seconds (default 120)." }
            }),
            &["command"],
        ),
        op_schema(
            "commit",
            json!({ "message": { "type": "string" } }),
            &["message"],
        ),
        op_schema(
            "undo",
            json!({ "snapshot": { "type": "integer", "description": "Snapshot number to restore to." } }),
            &["snapshot"],
        ),
        op_schema(
            "message",
            json!({
                "to": { "type": "string", "description": "Recipient: captain, human, or admiral." },
                "text": { "type": "string" }
            }),
            &["to", "text"],
        ),
        op_schema(
            "submit",
            json!({ "summary": { "type": "string" } }),
            &["summary"],
        ),
    ]
}

pub fn code_tool() -> ToolDefinition {
    let op_schemas = code_tool_op_schemas();
    let mut items = serde_json::Map::new();
    items.insert("type".into(), json!("object"));
    items.insert(
        "description".into(),
        json!("Each op is an object with exactly one key (the op type) whose value is the op parameters."),
    );
    items.insert("oneOf".into(), Value::Array(op_schemas));

    let mut ops_prop = serde_json::Map::new();
    ops_prop.insert("type".into(), json!("array"));
    ops_prop.insert(
        "description".into(),
        json!("Array of operations to execute."),
    );
    ops_prop.insert("items".into(), Value::Object(items));
    ops_prop.insert("minItems".into(), json!(1));

    ToolDefinition {
        name: "code",
        description: "Execute one or more code operations in a single batch. \
Operations are executed in order. Read-only ops continue on failure; \
mutation ops stop the batch on the first error. Every mutation creates an undo snapshot. \
Use this tool for ALL file operations — search, read, edit, run commands, and commit.",
        input_schema: json!({
            "type": "object",
            "properties": { "ops": Value::Object(ops_prop) },
            "required": ["ops"],
            "additionalProperties": false,
        }),
    }
}

pub fn commit_tool() -> ToolDefinition {
    ToolDefinition {
        name: "commit",
        description: "Commit staged changes with the given message. The message is used verbatim. Optionally marks a plan step complete if step_index is provided. Call this IMMEDIATELY after finishing each step — before starting the next one. All file changes for this step must already be written. Do not run manual git commit/rebase/merge commands.",
        input_schema: json!({
            "type": "object",
            "properties": {
                "message": { "type": "string", "description": "Commit message, used verbatim." },
                "step_index": { "type": "integer", "minimum": 0, "description": "If provided, marks this plan step as complete." }
            },
            "required": ["message"],
            "additionalProperties": false,
        }),
    }
}
