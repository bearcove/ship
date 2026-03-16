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
#[facet(skip_all_unless_truthy)]
pub struct JsonRpcError {
    pub code: i64,
    pub message: String,
    #[facet(default)]
    pub data: Option<RawJson<'static>>,
}

// ── Shared types ────────────────────────────────────────────────────

#[derive(Debug, Clone, Facet)]
#[facet(skip_all_unless_truthy)]
pub struct Implementation {
    pub name: String,
    #[facet(default)]
    pub title: Option<String>,
    pub version: String,
    #[facet(default)]
    pub description: Option<String>,
    #[facet(default, rename = "websiteUrl")]
    pub website_url: Option<String>,
    #[facet(default)]
    pub icons: Option<Vec<Icon>>,
}

#[derive(Debug, Clone, Facet)]
#[facet(skip_all_unless_truthy)]
pub struct Icon {
    pub url: String,
    #[facet(default, rename = "mimeType")]
    pub mime_type: Option<String>,
    #[facet(default)]
    pub size: Option<String>,
}

#[derive(Debug, Clone, Facet)]
#[facet(skip_all_unless_truthy)]
pub struct Annotations {
    #[facet(default)]
    pub audience: Option<Vec<String>>,
    #[facet(default)]
    pub priority: Option<f64>,
    #[facet(default, rename = "lastModified")]
    pub last_modified: Option<String>,
}

// ── Initialize ──────────────────────────────────────────────────────

#[derive(Debug, Facet)]
#[facet(skip_all_unless_truthy)]
pub struct InitializeParams {
    #[facet(rename = "protocolVersion")]
    pub protocol_version: String,
    pub capabilities: ClientCapabilities,
    #[facet(rename = "clientInfo")]
    pub client_info: Implementation,
    #[facet(default)]
    pub _meta: Option<RawJson<'static>>,
}

#[derive(Debug, Facet)]
#[facet(skip_all_unless_truthy)]
pub struct ClientCapabilities {
    #[facet(default)]
    pub experimental: Option<RawJson<'static>>,
    #[facet(default)]
    pub roots: Option<RootsCapability>,
    #[facet(default)]
    pub sampling: Option<RawJson<'static>>,
    #[facet(default)]
    pub elicitation: Option<RawJson<'static>>,
    #[facet(default)]
    pub tasks: Option<RawJson<'static>>,
}

#[derive(Debug, Facet)]
#[facet(skip_all_unless_truthy)]
pub struct RootsCapability {
    #[facet(default, rename = "listChanged")]
    pub list_changed: Option<bool>,
}

#[derive(Debug, Facet)]
#[facet(skip_all_unless_truthy)]
pub struct InitializeResult {
    #[facet(rename = "protocolVersion")]
    pub protocol_version: String,
    pub capabilities: ServerCapabilities,
    #[facet(rename = "serverInfo")]
    pub server_info: Implementation,
    #[facet(default)]
    pub instructions: Option<String>,
    #[facet(default)]
    pub _meta: Option<RawJson<'static>>,
}

#[derive(Debug, Facet)]
#[facet(skip_all_unless_truthy)]
pub struct ServerCapabilities {
    #[facet(default)]
    pub experimental: Option<RawJson<'static>>,
    #[facet(default)]
    pub logging: Option<RawJson<'static>>,
    #[facet(default)]
    pub completions: Option<RawJson<'static>>,
    #[facet(default)]
    pub prompts: Option<PromptsCapability>,
    #[facet(default)]
    pub resources: Option<ResourcesCapability>,
    #[facet(default)]
    pub tools: Option<ToolsCapability>,
    #[facet(default)]
    pub tasks: Option<RawJson<'static>>,
}

#[derive(Debug, Facet)]
#[facet(skip_all_unless_truthy)]
pub struct ToolsCapability {
    #[facet(default, rename = "listChanged")]
    pub list_changed: Option<bool>,
}

#[derive(Debug, Facet)]
#[facet(skip_all_unless_truthy)]
pub struct PromptsCapability {
    #[facet(default, rename = "listChanged")]
    pub list_changed: Option<bool>,
}

#[derive(Debug, Facet)]
#[facet(skip_all_unless_truthy)]
pub struct ResourcesCapability {
    #[facet(default)]
    pub subscribe: Option<bool>,
    #[facet(default, rename = "listChanged")]
    pub list_changed: Option<bool>,
}

// ── Tools ───────────────────────────────────────────────────────────

#[derive(Debug, Facet)]
#[facet(skip_all_unless_truthy)]
pub struct ListToolsResult {
    pub tools: Vec<ToolInfo>,
    #[facet(default, rename = "nextCursor")]
    pub next_cursor: Option<String>,
    #[facet(default)]
    pub _meta: Option<RawJson<'static>>,
}

#[derive(Debug, Clone, Facet)]
#[facet(skip_all_unless_truthy)]
pub struct ToolInfo {
    pub name: String,
    #[facet(default)]
    pub title: Option<String>,
    #[facet(default)]
    pub description: Option<String>,
    #[facet(rename = "inputSchema")]
    pub input_schema: JsonSchema,
    #[facet(default, rename = "outputSchema")]
    pub output_schema: Option<JsonSchema>,
    #[facet(default)]
    pub annotations: Option<ToolAnnotations>,
    #[facet(default)]
    pub execution: Option<ToolExecution>,
    #[facet(default)]
    pub icons: Option<Vec<Icon>>,
    #[facet(default)]
    pub _meta: Option<RawJson<'static>>,
}

#[derive(Debug, Clone, Facet)]
#[facet(skip_all_unless_truthy)]
pub struct ToolAnnotations {
    #[facet(default)]
    pub title: Option<String>,
    #[facet(default, rename = "readOnlyHint")]
    pub read_only_hint: Option<bool>,
    #[facet(default, rename = "destructiveHint")]
    pub destructive_hint: Option<bool>,
    #[facet(default, rename = "idempotentHint")]
    pub idempotent_hint: Option<bool>,
    #[facet(default, rename = "openWorldHint")]
    pub open_world_hint: Option<bool>,
}

#[derive(Debug, Clone, Facet)]
#[facet(skip_all_unless_truthy)]
pub struct ToolExecution {
    #[facet(default, rename = "taskSupport")]
    pub task_support: Option<String>,
}

// ── Call Tool ───────────────────────────────────────────────────────

#[derive(Debug, Facet)]
#[facet(skip_all_unless_truthy)]
pub struct CallToolParams {
    pub name: String,
    #[facet(default)]
    pub arguments: Option<RawJson<'static>>,
    #[facet(default)]
    pub _meta: Option<RawJson<'static>>,
}

#[derive(Debug, Facet)]
#[facet(skip_all_unless_truthy)]
pub struct CallToolResult {
    pub content: Vec<ContentBlock>,
    #[facet(default, rename = "structuredContent")]
    pub structured_content: Option<RawJson<'static>>,
    #[facet(default, rename = "isError")]
    pub is_error: Option<bool>,
    #[facet(default)]
    pub _meta: Option<RawJson<'static>>,
}

// ── Content Blocks ──────────────────────────────────────────────────

#[derive(Debug, Facet)]
#[facet(tag = "type", rename_all = "snake_case", skip_all_unless_truthy)]
#[repr(u8)]
pub enum ContentBlock {
    Text {
        text: String,
        #[facet(default)]
        annotations: Option<Annotations>,
    },
    Image {
        data: String,
        #[facet(rename = "mimeType")]
        mime_type: String,
        #[facet(default)]
        annotations: Option<Annotations>,
    },
    Audio {
        data: String,
        #[facet(rename = "mimeType")]
        mime_type: String,
        #[facet(default)]
        annotations: Option<Annotations>,
    },
    ResourceLink {
        name: String,
        uri: String,
        #[facet(default)]
        description: Option<String>,
        #[facet(default, rename = "mimeType")]
        mime_type: Option<String>,
        #[facet(default)]
        annotations: Option<Annotations>,
    },
}

// ── Helpers ─────────────────────────────────────────────────────────

impl CallToolResult {
    pub fn text(text: impl Into<String>) -> Self {
        Self {
            content: vec![ContentBlock::Text {
                text: text.into(),
                annotations: None,
            }],
            structured_content: None,
            is_error: None,
            _meta: None,
        }
    }

    pub fn error(text: impl Into<String>) -> Self {
        Self {
            content: vec![ContentBlock::Text {
                text: text.into(),
                annotations: None,
            }],
            structured_content: None,
            is_error: Some(true),
            _meta: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_initialize_request() {
        let json = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"test","version":"0.1"}}}"#;
        let req: JsonRpcRequest = facet_json::from_str(json).unwrap();
        assert_eq!(req.method, "initialize");
        assert!(req.params.is_some());

        let params: InitializeParams =
            facet_json::from_str(req.params.unwrap().as_ref()).unwrap();
        assert_eq!(params.protocol_version, "2025-11-25");
        assert_eq!(params.client_info.name, "test");
    }

    #[test]
    fn parse_initialize_request_with_capabilities() {
        let json = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{"roots":{"listChanged":true},"sampling":{}},"clientInfo":{"name":"test","version":"0.1"}}}"#;
        let req: JsonRpcRequest = facet_json::from_str(json).unwrap();
        let params: InitializeParams =
            facet_json::from_str(req.params.unwrap().as_ref()).unwrap();
        assert!(params.capabilities.roots.is_some());
        assert!(params.capabilities.roots.unwrap().list_changed == Some(true));
        assert!(params.capabilities.sampling.is_some());
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
    fn serialize_content_block_text() {
        let block = ContentBlock::Text {
            text: "hello".to_owned(),
            annotations: None,
        };
        let json = facet_json::to_string(&block).unwrap();
        assert!(json.contains(r#""type":"text""#));
        assert!(json.contains(r#""text":"hello""#));
    }

    #[test]
    fn serialize_content_block_image() {
        let block = ContentBlock::Image {
            data: "base64data".to_owned(),
            mime_type: "image/png".to_owned(),
            annotations: None,
        };
        let json = facet_json::to_string(&block).unwrap();
        assert!(json.contains(r#""type":"image""#));
        assert!(json.contains(r#""mimeType":"image/png""#));
    }

    #[test]
    fn serialize_call_tool_result_text() {
        let result = CallToolResult::text("hello world");
        let json = facet_json::to_string(&result).unwrap();
        assert!(json.contains("hello world"));
        assert!(json.contains(r#""type":"text""#));
        assert!(!json.contains("isError"));
        assert!(!json.contains("structuredContent"));
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
            protocol_version: "2025-11-25".to_owned(),
            capabilities: ServerCapabilities {
                tools: Some(ToolsCapability {
                    list_changed: Some(false),
                }),
                experimental: None,
                logging: None,
                completions: None,
                prompts: None,
                resources: None,
                tasks: None,
            },
            server_info: Implementation {
                name: "test-server".to_owned(),
                title: None,
                version: "0.1.0".to_owned(),
                description: None,
                website_url: None,
                icons: None,
            },
            instructions: None,
            _meta: None,
        };
        let json = facet_json::to_string(&result).unwrap();
        assert!(json.contains("protocolVersion"));
        assert!(json.contains("serverInfo"));
        assert!(json.contains("listChanged"));
        assert!(!json.contains("instructions"));
    }

    #[test]
    fn serialize_initialize_result_with_instructions() {
        let result = InitializeResult {
            protocol_version: "2025-11-25".to_owned(),
            capabilities: ServerCapabilities {
                tools: None,
                experimental: None,
                logging: None,
                completions: None,
                prompts: None,
                resources: None,
                tasks: None,
            },
            server_info: Implementation {
                name: "test".to_owned(),
                title: None,
                version: "0.1".to_owned(),
                description: None,
                website_url: None,
                icons: None,
            },
            instructions: Some("Use this server for math.".to_owned()),
            _meta: None,
        };
        let json = facet_json::to_string(&result).unwrap();
        assert!(json.contains("Use this server for math."));
    }

    #[derive(Debug, Facet)]
    struct WebSearchArgs {
        /// The search query
        query: String,
    }

    #[test]
    fn serialize_list_tools_result() {
        let result = ListToolsResult {
            tools: vec![ToolInfo {
                name: "web_search".to_owned(),
                title: None,
                description: Some("Search the web".to_owned()),
                input_schema: facet_json_schema::schema_for::<WebSearchArgs>(),
                output_schema: None,
                annotations: None,
                execution: None,
                icons: None,
                _meta: None,
            }],
            next_cursor: None,
            _meta: None,
        };
        let json = facet_json::to_string(&result).unwrap();
        assert!(json.contains("web_search"));
        assert!(json.contains("inputSchema"));
        assert!(json.contains("query"));
        assert!(!json.contains("outputSchema"));
    }

    #[test]
    fn serialize_tool_with_annotations() {
        let tool = ToolInfo {
            name: "delete_file".to_owned(),
            title: None,
            description: Some("Delete a file".to_owned()),
            input_schema: facet_json_schema::schema_for::<WebSearchArgs>(),
            output_schema: None,
            annotations: Some(ToolAnnotations {
                title: None,
                read_only_hint: Some(false),
                destructive_hint: Some(true),
                idempotent_hint: None,
                open_world_hint: None,
            }),
            execution: None,
            icons: None,
            _meta: None,
        };
        let json = facet_json::to_string(&tool).unwrap();
        assert!(json.contains("destructiveHint"));
        assert!(json.contains("readOnlyHint"));
        assert!(!json.contains("idempotentHint"));
    }

    #[test]
    fn schema_from_facet_struct() {
        let schema = facet_json_schema::schema_for::<WebSearchArgs>();
        let json = facet_json::to_string_pretty(&schema).unwrap();
        assert!(json.contains(r#""type": "object"#));
        assert!(json.contains("query"));
        assert!(json.contains("required"));
        assert!(json.contains("The search query"));
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
        assert!(json.contains("path"));
        assert!(json.contains(r#""required":["path"]"#));
    }

    #[test]
    fn parse_call_tool_params_with_arguments() {
        let json = r#"{"name":"add","arguments":{"a":3,"b":4}}"#;
        let params: CallToolParams = facet_json::from_str(json).unwrap();
        assert_eq!(params.name, "add");
        let args = params.arguments.unwrap();
        assert!(args.as_ref().contains("3"));
        assert!(args.as_ref().contains("4"));
    }

    #[test]
    fn parse_call_tool_params_nested_in_rawjson() {
        // This simulates the actual server flow: params arrives as RawJson,
        // then we parse it into CallToolParams
        let outer = r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"add","arguments":{"a":3,"b":4}}}"#;
        let req: JsonRpcRequest = facet_json::from_str(outer).unwrap();
        let raw_params = req.params.unwrap();
        let params: CallToolParams = facet_json::from_str(raw_params.as_ref()).unwrap();
        assert_eq!(params.name, "add");
        let args = params.arguments.unwrap();
        assert!(args.as_ref().contains("3"));
    }

    #[test]
    fn parse_rawjson_from_temporary_string() {
        // Simulate what the transport does: parse from a String that gets
        // dropped, then use the RawJson afterwards. If RawJson borrows
        // into the input instead of owning, this is use-after-free.
        let req: JsonRpcRequest = {
            let line = String::from(
                r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"add","arguments":{"a":3,"b":4}}}"#
            );
            facet_json::from_str::<JsonRpcRequest>(&line).unwrap()
            // line is dropped here
        };
        // If RawJson borrowed into line, this is reading freed memory
        let raw_params = req.params.unwrap();
        let params: CallToolParams = facet_json::from_str(raw_params.as_ref()).unwrap();
        assert_eq!(params.name, "add");
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

    #[test]
    fn jsonrpc_error_with_data() {
        let error = JsonRpcError {
            code: -32600,
            message: "Invalid Request".to_owned(),
            data: Some(RawJson::from_owned(r#"{"detail":"missing method"}"#.to_owned())),
        };
        let json = facet_json::to_string(&error).unwrap();
        assert!(json.contains("-32600"));
        assert!(json.contains("missing method"));
    }

    #[test]
    fn jsonrpc_error_without_data() {
        let error = JsonRpcError {
            code: -32601,
            message: "Method not found".to_owned(),
            data: None,
        };
        let json = facet_json::to_string(&error).unwrap();
        assert!(!json.contains("data"));
    }
}
