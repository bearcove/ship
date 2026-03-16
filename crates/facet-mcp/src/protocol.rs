use facet::Facet;
use facet_json::RawJson;
use facet_json_schema::JsonSchema;

// ── JSON-RPC 2.0 framing ────────────────────────────────────────────

#[derive(Debug, Facet)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    #[facet(default)]
    pub id: Option<JsonRpcId>,
    pub method: String,
    #[facet(default)]
    pub params: Option<RawJson<'static>>,
}

#[derive(Debug, Clone, Facet)]
#[facet(untagged)]
#[repr(u8)]
pub enum JsonRpcId {
    Number(i64),
    Str(String),
}

#[derive(Debug, Facet)]
#[facet(skip_all_unless_truthy)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    #[facet(default)]
    pub id: Option<JsonRpcId>,
    #[facet(default)]
    pub result: Option<RawJson<'static>>,
    #[facet(default)]
    pub error: Option<JsonRpcError>,
}

#[derive(Debug, Facet)]
pub struct JsonRpcError {
    pub code: i64,
    pub message: String,
}

// ── MCP protocol types ──────────────────────────────────────────────

#[derive(Debug, Facet)]
pub struct InitializeParams {
    #[facet(rename = "protocolVersion")]
    pub protocol_version: String,
    pub capabilities: ClientCapabilities,
    #[facet(rename = "clientInfo")]
    pub client_info: Implementation,
}

#[derive(Debug, Facet)]
pub struct ClientCapabilities {}

#[derive(Debug, Facet)]
pub struct Implementation {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Facet)]
pub struct InitializeResult {
    #[facet(rename = "protocolVersion")]
    pub protocol_version: String,
    pub capabilities: ServerCapabilities,
    #[facet(rename = "serverInfo")]
    pub server_info: ServerInfo,
}

#[derive(Debug, Facet)]
pub struct ServerCapabilities {
    #[facet(default)]
    pub tools: Option<ToolsCapability>,
}

#[derive(Debug, Facet)]
pub struct ToolsCapability {
    #[facet(default, rename = "listChanged")]
    pub list_changed: Option<bool>,
}

#[derive(Debug, Facet)]
pub struct ServerInfo {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Facet)]
pub struct ListToolsResult {
    pub tools: Vec<ToolInfo>,
}

#[derive(Debug, Clone, Facet)]
pub struct ToolInfo {
    pub name: String,
    pub description: String,
    #[facet(rename = "inputSchema")]
    pub input_schema: JsonSchema,
    #[facet(rename = "outputSchema")]
    pub output_schema: JsonSchema,
}

#[derive(Debug, Facet)]
pub struct CallToolParams {
    pub name: String,
    #[facet(default)]
    pub arguments: Option<RawJson<'static>>,
}

#[derive(Debug, Facet)]
#[facet(skip_all_unless_truthy)]
pub struct CallToolResult {
    pub content: Vec<ContentBlock>,
    #[facet(default, rename = "isError")]
    pub is_error: Option<bool>,
}

#[derive(Debug, Facet)]
#[facet(tag = "type", rename_all = "lowercase")]
#[repr(u8)]
pub enum ContentBlock {
    Text { text: String },
}

impl CallToolResult {
    pub fn text(text: impl Into<String>) -> Self {
        Self {
            content: vec![ContentBlock::Text { text: text.into() }],
            is_error: None,
        }
    }

    pub fn error(text: impl Into<String>) -> Self {
        Self {
            content: vec![ContentBlock::Text { text: text.into() }],
            is_error: Some(true),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_initialize_request() {
        let json = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-03-26","capabilities":{},"clientInfo":{"name":"test","version":"0.1"}}}"#;
        let req: JsonRpcRequest = facet_json::from_str(json).unwrap();
        assert_eq!(req.method, "initialize");
        assert!(req.params.is_some());

        let params: InitializeParams =
            facet_json::from_str(req.params.unwrap().as_ref()).unwrap();
        assert_eq!(params.protocol_version, "2025-03-26");
        assert_eq!(params.client_info.name, "test");
    }

    #[test]
    fn parse_tools_call_request() {
        let json = r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"web_search","arguments":{"query":"rust programming"}}}"#;
        let req: JsonRpcRequest = facet_json::from_str(json).unwrap();
        assert_eq!(req.method, "tools/call");

        let params: CallToolParams =
            facet_json::from_str(req.params.unwrap().as_ref()).unwrap();
        assert_eq!(params.name, "web_search");
        assert!(params.arguments.is_some());
        let args_json = params.arguments.unwrap();
        assert!(args_json.as_ref().contains("rust programming"));
    }

    #[test]
    fn parse_notification_no_id() {
        let json = r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#;
        let req: JsonRpcRequest = facet_json::from_str(json).unwrap();
        assert!(req.id.is_none());
        assert!(req.params.is_none());
        assert_eq!(req.method, "notifications/initialized");
    }

    #[test]
    fn parse_jsonrpc_id_number() {
        let json = r#"{"jsonrpc":"2.0","id":42,"method":"test"}"#;
        let req: JsonRpcRequest = facet_json::from_str(json).unwrap();
        match req.id {
            Some(JsonRpcId::Number(n)) => assert_eq!(n, 42),
            other => panic!("expected Number(42), got {other:?}"),
        }
    }

    #[test]
    fn parse_jsonrpc_id_string() {
        let json = r#"{"jsonrpc":"2.0","id":"abc-123","method":"test"}"#;
        let req: JsonRpcRequest = facet_json::from_str(json).unwrap();
        match req.id {
            Some(JsonRpcId::Str(ref s)) => assert_eq!(s, "abc-123"),
            other => panic!("expected Str(\"abc-123\"), got {other:?}"),
        }
    }

    #[test]
    fn serialize_call_tool_result_text() {
        let result = CallToolResult::text("hello world");
        let json = facet_json::to_string(&result).unwrap();
        eprintln!("CallToolResult JSON: {json}");
        assert!(json.contains("hello world"));
        assert!(!json.contains("isError"));
    }

    #[test]
    fn serialize_call_tool_result_error() {
        let result = CallToolResult::error("something broke");
        let json = facet_json::to_string(&result).unwrap();
        assert!(json.contains("something broke"));
        assert!(json.contains("isError"));
    }

    #[test]
    fn serialize_initialize_result() {
        let result = InitializeResult {
            protocol_version: "2025-03-26".to_owned(),
            capabilities: ServerCapabilities {
                tools: Some(ToolsCapability {
                    list_changed: Some(false),
                }),
            },
            server_info: ServerInfo {
                name: "test-server".to_owned(),
                version: "0.1.0".to_owned(),
            },
        };
        let json = facet_json::to_string(&result).unwrap();
        assert!(json.contains("protocolVersion"));
        assert!(json.contains("serverInfo"));
        assert!(json.contains("listChanged"));
    }

    #[test]
    fn serialize_list_tools_result() {
        let result = ListToolsResult {
            tools: vec![ToolInfo {
                name: "web_search".to_owned(),
                description: "Search the web".to_owned(),
                input_schema: facet_json_schema::schema_for::<WebSearchArgs>(),
                output_schema: facet_json_schema::schema_for::<WebSearchArgs>(),
            }],
        };
        let json = facet_json::to_string(&result).unwrap();
        assert!(json.contains("web_search"));
        assert!(json.contains("inputSchema"));
        assert!(json.contains("query"));
    }

    #[derive(Debug, Facet)]
    struct WebSearchArgs {
        /// The search query
        query: String,
    }

    #[test]
    fn schema_from_facet_struct() {
        let schema = facet_json_schema::schema_for::<WebSearchArgs>();
        let json = facet_json::to_string_pretty(&schema).unwrap();
        assert!(json.contains(r#""type": "object"#));
        assert!(json.contains("query"));
        assert!(json.contains("required"));
        // TODO: facet-json-schema doesn't emit field-level doc comments yet
        // assert!(json.contains("The search query"));
    }

    #[test]
    fn schema_optional_fields_not_required() {
        #[derive(Debug, Facet)]
        struct ReadFileArgs {
            path: String,
            #[facet(default)]
            offset: Option<u64>,
            #[facet(default)]
            limit: Option<u64>,
        }

        let schema = facet_json_schema::schema_for::<ReadFileArgs>();
        let json = facet_json::to_string(&schema).unwrap();
        // path should be required, offset and limit should not
        assert!(json.contains("path"));
        // The required array should only contain "path"
        assert!(json.contains(r#""required":["path"]"#));
    }

    #[test]
    fn roundtrip_response() {
        let response = JsonRpcResponse {
            jsonrpc: "2.0".to_owned(),
            id: Some(JsonRpcId::Number(1)),
            result: Some(RawJson::from_owned(r#"{"tools":[]}"#.to_owned())),
            error: None,
        };
        let json = facet_json::to_string(&response).unwrap();
        assert!(json.contains(r#""jsonrpc":"2.0""#));
        assert!(json.contains(r#""id":1"#));
        assert!(json.contains(r#""tools":[]"#));
    }
}
