use facet_acp::*;

// ── Content blocks ─────────────────────────────────────────────────

#[test]
fn text_content_block_tagged() {
    let block = ContentBlock::Text(TextContent::new("hello"));
    let json = facet_json::to_string(&block).unwrap();
    assert!(json.contains(r#""type":"text""#), "missing type tag: {json}");
    assert!(json.contains(r#""text":"hello""#), "missing text: {json}");
}

#[test]
fn image_content_block_tagged() {
    let block = ContentBlock::Image(ImageContent::new("base64data", "image/png"));
    let json = facet_json::to_string(&block).unwrap();
    assert!(json.contains(r#""type":"image""#), "missing type tag: {json}");
    assert!(json.contains(r#""mimeType":"image/png""#), "missing camelCase mimeType: {json}");
}

#[test]
fn content_block_roundtrip() {
    let original = ContentBlock::Text(TextContent::new("round trip"));
    let json = facet_json::to_string(&original).unwrap();
    let parsed: ContentBlock = facet_json::from_str(&json).unwrap();
    assert_eq!(original, parsed);
}

// ── Tool calls ─────────────────────────────────────────────────────

#[test]
fn tool_kind_snake_case() {
    let kind = ToolKind::SwitchMode;
    let json = facet_json::to_string(&kind).unwrap();
    assert_eq!(json, r#""switch_mode""#, "ToolKind should be snake_case");
}

#[test]
fn tool_call_status_snake_case() {
    let status = ToolCallStatus::InProgress;
    let json = facet_json::to_string(&status).unwrap();
    assert_eq!(json, r#""in_progress""#);
}

#[test]
fn tool_call_camel_case_fields() {
    let tc = ToolCall::new("toolu_123", "Read file");
    let json = facet_json::to_string(&tc).unwrap();
    assert!(json.contains(r#""toolCallId":"toolu_123""#), "missing camelCase toolCallId: {json}");
}

#[test]
fn tool_call_content_tagged() {
    let content = ToolCallContent::Diff(Diff::new("/tmp/foo.rs", "fn main() {}"));
    let json = facet_json::to_string(&content).unwrap();
    assert!(json.contains(r#""type":"diff""#), "missing type tag: {json}");
    assert!(json.contains(r#""newText":"fn main() {}""#), "missing camelCase newText: {json}");
}

#[test]
fn tool_call_location_camel_case() {
    let loc = ToolCallLocation::new("/src/lib.rs").line(42);
    let json = facet_json::to_string(&loc).unwrap();
    // path should just be serialized as a string
    assert!(json.contains("lib.rs"), "missing path: {json}");
    assert!(json.contains(r#""line":42"#), "missing line: {json}");
}

// ── Plans ──────────────────────────────────────────────────────────

#[test]
fn plan_entry_status_snake_case() {
    let status = PlanEntryStatus::InProgress;
    let json = facet_json::to_string(&status).unwrap();
    assert_eq!(json, r#""in_progress""#);
}

#[test]
fn plan_roundtrip() {
    let plan = Plan::new(vec![
        PlanEntry::new("step 1", PlanEntryPriority::High, PlanEntryStatus::Completed),
        PlanEntry::new("step 2", PlanEntryPriority::Medium, PlanEntryStatus::InProgress),
    ]);
    let json = facet_json::to_string(&plan).unwrap();
    let parsed: Plan = facet_json::from_str(&json).unwrap();
    assert_eq!(plan.entries.len(), parsed.entries.len());
    assert_eq!(plan.entries[0].content, "step 1");
}

// ── Session types ──────────────────────────────────────────────────

#[test]
fn session_id_transparent() {
    let id = SessionId::new("sess-abc");
    let json = facet_json::to_string(&id).unwrap();
    assert_eq!(json, r#""sess-abc""#, "SessionId should be transparent");
}

#[test]
fn protocol_version_transparent() {
    let v = ProtocolVersion::V1;
    let json = facet_json::to_string(&v).unwrap();
    assert_eq!(json, "1", "ProtocolVersion should serialize as number");
}

// ── Initialize ─────────────────────────────────────────────────────

#[test]
fn initialize_request_camel_case() {
    let req = InitializeRequest::new(ProtocolVersion::V1)
        .client_info(Implementation::new("ship", "0.1.0"));
    let json = facet_json::to_string(&req).unwrap();
    assert!(json.contains(r#""protocolVersion":1"#), "missing camelCase protocolVersion: {json}");
    assert!(json.contains(r#""clientInfo":{""#) || json.contains(r#""clientInfo":{"#), "missing camelCase clientInfo: {json}");
}

// ── New session ────────────────────────────────────────────────────

#[test]
fn new_session_request_roundtrip() {
    let req = NewSessionRequest::new("/home/user/project");
    let json = facet_json::to_string(&req).unwrap();
    let parsed: NewSessionRequest = facet_json::from_str(&json).unwrap();
    assert_eq!(req.cwd, parsed.cwd);
}

#[test]
fn new_session_response_camel_case() {
    let resp = NewSessionResponse::new("sess-123");
    let json = facet_json::to_string(&resp).unwrap();
    assert!(json.contains(r#""sessionId":"sess-123""#), "missing camelCase sessionId: {json}");
}

// ── Prompt ─────────────────────────────────────────────────────────

#[test]
fn prompt_request_structure() {
    let req = PromptRequest::new("sess-1", vec![ContentBlock::from("hello agent")]);
    let json = facet_json::to_string(&req).unwrap();
    assert!(json.contains(r#""sessionId":"sess-1""#), "missing sessionId: {json}");
    assert!(json.contains(r#""hello agent""#), "missing content: {json}");
}

#[test]
fn stop_reason_snake_case() {
    let sr = StopReason::EndTurn;
    let json = facet_json::to_string(&sr).unwrap();
    assert_eq!(json, r#""end_turn""#);

    let sr = StopReason::MaxTokens;
    let json = facet_json::to_string(&sr).unwrap();
    assert_eq!(json, r#""max_tokens""#);

    let sr = StopReason::MaxTurnRequests;
    let json = facet_json::to_string(&sr).unwrap();
    assert_eq!(json, r#""max_turn_requests""#);
}

// ── Cancel ─────────────────────────────────────────────────────────

#[test]
fn cancel_notification_camel_case() {
    let cancel = CancelNotification::new("sess-42");
    let json = facet_json::to_string(&cancel).unwrap();
    assert!(json.contains(r#""sessionId":"sess-42""#), "missing camelCase sessionId: {json}");
}

// ── Session update (tagged enum) ───────────────────────────────────

#[test]
fn session_update_agent_message_chunk() {
    let update = SessionUpdate::AgentMessageChunk(ContentChunk::new(ContentBlock::from("thinking...")));
    let json = facet_json::to_string(&update).unwrap();
    assert!(
        json.contains(r#""sessionUpdate":"agent_message_chunk""#),
        "missing sessionUpdate tag: {json}"
    );
    assert!(json.contains(r#""thinking...""#), "missing content: {json}");
}

#[test]
fn session_update_tool_call() {
    let tc = ToolCall::new("toolu_1", "Read file");
    let update = SessionUpdate::ToolCall(tc);
    let json = facet_json::to_string(&update).unwrap();
    assert!(
        json.contains(r#""sessionUpdate":"tool_call""#),
        "missing sessionUpdate tag: {json}"
    );
    assert!(json.contains(r#""toolCallId":"toolu_1""#), "missing toolCallId: {json}");
}

#[test]
fn session_update_usage() {
    let update = SessionUpdate::UsageUpdate(UsageUpdate::new(50000, 200000));
    let json = facet_json::to_string(&update).unwrap();
    assert!(
        json.contains(r#""sessionUpdate":"usage_update""#),
        "missing sessionUpdate tag: {json}"
    );
    assert!(json.contains(r#""used":50000"#), "missing used: {json}");
    assert!(json.contains(r#""size":200000"#), "missing size: {json}");
}

// ── Session notification ───────────────────────────────────────────

#[test]
fn session_notification_structure() {
    let notif = SessionNotification::new(
        "sess-1",
        SessionUpdate::AgentMessageChunk(ContentChunk::new(ContentBlock::from("hi"))),
    );
    let json = facet_json::to_string(&notif).unwrap();
    assert!(json.contains(r#""sessionId":"sess-1""#), "missing sessionId: {json}");
    assert!(json.contains(r#""update":{""#) || json.contains(r#""update":{"#), "missing update: {json}");
}

// ── MCP servers ────────────────────────────────────────────────────

#[test]
fn mcp_server_stdio_untagged() {
    let server = McpServer::Stdio(McpServerStdio::new("my-server", "/usr/bin/server")
        .args(vec!["--flag".to_owned()])
        .env(vec![EnvVariable::new("KEY", "VAL")]));
    let json = facet_json::to_string(&server).unwrap();
    // Stdio is untagged — no "type" field
    assert!(!json.contains(r#""type""#), "stdio should not have type tag: {json}");
    assert!(json.contains(r#""name":"my-server""#), "missing name: {json}");
}

#[test]
fn mcp_server_http_tagged() {
    let server = McpServer::Http(McpServerHttp::new("api", "https://example.com")
        .headers(vec![HttpHeader::new("Authorization", "Bearer xyz")]));
    let json = facet_json::to_string(&server).unwrap();
    assert!(json.contains(r#""type":"http""#), "missing type tag: {json}");
}

// ── Permission types ───────────────────────────────────────────────

#[test]
fn permission_option_kind_snake_case() {
    let kind = PermissionOptionKind::AllowAlways;
    let json = facet_json::to_string(&kind).unwrap();
    assert_eq!(json, r#""allow_always""#);
}

#[test]
fn permission_outcome_tagged() {
    let outcome = RequestPermissionOutcome::Selected(SelectedPermissionOutcome::new("opt-1"));
    let json = facet_json::to_string(&outcome).unwrap();
    assert!(json.contains(r#""outcome":"selected""#), "missing outcome tag: {json}");
    assert!(json.contains(r#""optionId":"opt-1""#), "missing camelCase optionId: {json}");

    let cancelled = RequestPermissionOutcome::Cancelled {};
    let json = facet_json::to_string(&cancelled).unwrap();
    assert!(json.contains(r#""outcome":"cancelled""#), "missing outcome tag: {json}");
}

// ── Error ──────────────────────────────────────────────────────────

#[test]
fn error_serialization() {
    let err = Error::invalid_params().data("missing field 'name'");
    let json = facet_json::to_string(&err).unwrap();
    assert!(json.contains(r#""code":-32602"#), "missing code: {json}");
    assert!(json.contains(r#""message":"Invalid params""#), "missing message: {json}");
}

// ── JSON-RPC framing ──────────────────────────────────────────────

#[test]
fn jsonrpc_request_parse() {
    let raw = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":1}}"#;
    let msg: JsonRpcMessage = facet_json::from_str(raw).unwrap();
    assert_eq!(msg.method.as_deref(), Some("initialize"));
    assert!(msg.is_request());
    match msg.id {
        Some(JsonRpcId::Number(n)) => assert_eq!(n, 1),
        other => panic!("expected Number(1), got {other:?}"),
    }
}

#[test]
fn jsonrpc_request_string_id() {
    let raw = r#"{"jsonrpc":"2.0","id":"abc-123","method":"test"}"#;
    let msg: JsonRpcMessage = facet_json::from_str(raw).unwrap();
    assert!(msg.is_request());
    match msg.id {
        Some(JsonRpcId::Str(ref s)) => assert_eq!(s, "abc-123"),
        other => panic!("expected Str, got {other:?}"),
    }
}

#[test]
fn jsonrpc_notification_no_id() {
    let raw = r#"{"jsonrpc":"2.0","method":"session/cancel","params":{"sessionId":"sess-1"}}"#;
    let msg: JsonRpcMessage = facet_json::from_str(raw).unwrap();
    assert!(msg.is_notification());
    assert!(msg.id.is_none());
    assert_eq!(msg.method.as_deref(), Some("session/cancel"));
}

#[test]
fn jsonrpc_response_parse() {
    let raw = r#"{"jsonrpc":"2.0","id":42,"result":{"ok":true}}"#;
    let msg: JsonRpcMessage = facet_json::from_str(raw).unwrap();
    assert!(msg.is_response());
    assert!(msg.result.is_some());
    assert!(msg.method.is_none());
}

#[test]
fn jsonrpc_response_roundtrip() {
    let msg = JsonRpcMessage::response_ok(
        JsonRpcId::Number(42),
        facet_json::RawJson::from_owned(r#"{"ok":true}"#.to_owned()),
    );
    let json = facet_json::to_string(&msg).unwrap();
    assert!(json.contains(r#""jsonrpc":"2.0""#), "missing jsonrpc: {json}");
    assert!(json.contains(r#""id":42"#), "missing id: {json}");
    assert!(json.contains(r#""ok":true"#), "missing result: {json}");
    // method should NOT be present in a response
    assert!(!json.contains(r#""method""#), "response should not have method: {json}");
}

#[test]
fn jsonrpc_error_response() {
    let raw = r#"{"jsonrpc":"2.0","id":5,"error":{"code":-32603,"message":"Internal error"}}"#;
    let msg: JsonRpcMessage = facet_json::from_str(raw).unwrap();
    assert!(msg.is_response());
    assert!(msg.error.is_some());
    assert!(msg.result.is_none());
}

// ── Model/config types ─────────────────────────────────────────────

#[test]
fn set_session_model_request_camel_case() {
    let req = SetSessionModelRequest::new("sess-1", ModelId::new("claude-3"));
    let json = facet_json::to_string(&req).unwrap();
    assert!(json.contains(r#""sessionId":"sess-1""#), "missing sessionId: {json}");
    assert!(json.contains(r#""modelId":"claude-3""#), "missing camelCase modelId: {json}");
}

#[test]
fn set_session_config_option_request_camel_case() {
    let req = SetSessionConfigOptionRequest::new("sess-1", SessionConfigId::new("effort"), SessionConfigValueId::new("high"));
    let json = facet_json::to_string(&req).unwrap();
    assert!(json.contains(r#""configId":"effort""#), "missing camelCase configId: {json}");
    assert!(json.contains(r#""value":"high""#), "missing value field: {json}");
}

// ── Deserialization from realistic agent output ────────────────────

#[test]
fn parse_realistic_session_notification() {
    let raw = r#"{
        "sessionId": "sess-abc",
        "update": {
            "sessionUpdate": "agent_message_chunk",
            "content": {
                "type": "text",
                "text": "I'll help you with that."
            }
        }
    }"#;
    let notif: SessionNotification = facet_json::from_str(raw).unwrap();
    assert_eq!(notif.session_id.0.as_ref(), "sess-abc");
    match notif.update {
        SessionUpdate::AgentMessageChunk(chunk) => {
            match chunk.content {
                ContentBlock::Text(t) => assert_eq!(t.text, "I'll help you with that."),
                other => panic!("expected Text, got {other:?}"),
            }
        }
        other => panic!("expected AgentMessageChunk, got {other:?}"),
    }
}

#[test]
fn parse_realistic_tool_call_update() {
    let raw = r#"{
        "sessionId": "sess-1",
        "update": {
            "sessionUpdate": "tool_call_update",
            "toolCallId": "toolu_abc",
            "status": "completed",
            "title": "Read file",
            "content": [{
                "type": "content",
                "content": {"type": "text", "text": "file contents here"}
            }]
        }
    }"#;
    let notif: SessionNotification = facet_json::from_str(raw).unwrap();
    match notif.update {
        SessionUpdate::ToolCallUpdate(update) => {
            assert_eq!(update.tool_call_id.0.as_ref(), "toolu_abc");
            assert_eq!(update.status, Some(ToolCallStatus::Completed));
        }
        other => panic!("expected ToolCallUpdate, got {other:?}"),
    }
}

#[test]
fn parse_realistic_usage_update() {
    let raw = r#"{
        "sessionId": "sess-1",
        "update": {
            "sessionUpdate": "usage_update",
            "used": 45000,
            "size": 200000,
            "cost": {
                "amount": 0.035,
                "currency": "USD"
            }
        }
    }"#;
    let notif: SessionNotification = facet_json::from_str(raw).unwrap();
    match notif.update {
        SessionUpdate::UsageUpdate(usage) => {
            assert_eq!(usage.used, 45000);
            assert_eq!(usage.size, 200000);
            let cost = usage.cost.unwrap();
            assert_eq!(cost.currency, "USD");
        }
        other => panic!("expected UsageUpdate, got {other:?}"),
    }
}

#[test]
fn parse_realistic_new_session_response() {
    let raw = r#"{
        "sessionId": "sess-new-123",
        "configOptions": [{
            "id": "thinking",
            "name": "Thinking",
            "kind": {
                "type": "select",
                "options": [
                    {"value": "off", "name": "Off"},
                    {"value": "on", "name": "On"}
                ],
                "currentValue": "on"
            }
        }]
    }"#;
    let resp: NewSessionResponse = facet_json::from_str(raw).unwrap();
    assert_eq!(resp.session_id.0.as_ref(), "sess-new-123");
    let opts = resp.config_options.unwrap();
    assert_eq!(opts.len(), 1);
    assert_eq!(opts[0].id.0.as_ref(), "thinking");
}

#[test]
fn parse_realistic_prompt_response() {
    let raw = r#"{"stopReason": "end_turn"}"#;
    let resp: PromptResponse = facet_json::from_str(raw).unwrap();
    assert_eq!(resp.stop_reason, StopReason::EndTurn);
}

#[test]
fn parse_realistic_initialize_response() {
    let raw = r#"{
        "protocolVersion": 1,
        "agentCapabilities": {
            "prompt": {"image": true, "audio": false},
            "session": {"load": true, "resume": true},
            "mcp": {"http": false, "sse": true}
        },
        "agentInfo": {
            "name": "claude-code",
            "version": "1.0.0"
        }
    }"#;
    let resp: InitializeResponse = facet_json::from_str(raw).unwrap();
    assert_eq!(resp.protocol_version, ProtocolVersion::V1);
    let info = resp.agent_info.unwrap();
    assert_eq!(info.name, "claude-code");
}
