use rust_mcp_sdk::schema::{Tool, ToolInputSchema};
use serde_json::Value;

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
