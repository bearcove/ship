/// Compare facet-acp serialization output against agent-client-protocol (serde) output byte-for-byte.
use agent_client_protocol as acp;

#[test]
fn initialize_request_matches() {
    // Reference (serde)
    let ref_req = acp::InitializeRequest::new(acp::ProtocolVersion::LATEST)
        .client_capabilities(
            acp::ClientCapabilities::new()
                .terminal(true)
                .fs(acp::FileSystemCapability::new()
                    .read_text_file(true)
                    .write_text_file(true)),
        )
        .client_info(acp::Implementation::new("ship", "0.1.0"));
    let ref_json = serde_json::to_string(&ref_req).unwrap();

    // Ours (facet)
    let our_req = facet_acp::InitializeRequest::new(facet_acp::ProtocolVersion::LATEST)
        .client_capabilities(facet_acp::ClientCapabilities {
            fs: facet_acp::FileSystemCapability {
                read_text_file: true,
                write_text_file: true,
                meta: None,
            },
            terminal: true,
            meta: None,
        })
        .client_info(facet_acp::Implementation::new("ship", "0.1.0"));
    let our_json = facet_json::to_string(&our_req).unwrap();

    eprintln!("REF: {ref_json}");
    eprintln!("OUR: {our_json}");

    // Parse both as generic JSON values for structural comparison
    let ref_val: serde_json::Value = serde_json::from_str(&ref_json).unwrap();
    let our_val: serde_json::Value = serde_json::from_str(&our_json).unwrap();
    assert_eq!(ref_val, our_val, "initialize request mismatch");
}

#[test]
fn new_session_request_matches() {
    let ref_req = acp::NewSessionRequest::new("/home/user/project");
    let ref_json = serde_json::to_string(&ref_req).unwrap();

    let our_req = facet_acp::NewSessionRequest::new("/home/user/project");
    let our_json = facet_json::to_string(&our_req).unwrap();

    eprintln!("REF: {ref_json}");
    eprintln!("OUR: {our_json}");

    let ref_val: serde_json::Value = serde_json::from_str(&ref_json).unwrap();
    let our_val: serde_json::Value = serde_json::from_str(&our_json).unwrap();
    assert_eq!(ref_val, our_val, "new_session request mismatch");
}

#[test]
fn prompt_request_matches() {
    let ref_req = acp::PromptRequest::new(
        acp::SessionId::new("sess-1"),
        vec![acp::ContentBlock::from("hello agent")],
    );
    let ref_json = serde_json::to_string(&ref_req).unwrap();

    let our_req = facet_acp::PromptRequest::new(
        "sess-1",
        vec![facet_acp::ContentBlock::from("hello agent")],
    );
    let our_json = facet_json::to_string(&our_req).unwrap();

    eprintln!("REF: {ref_json}");
    eprintln!("OUR: {our_json}");

    let ref_val: serde_json::Value = serde_json::from_str(&ref_json).unwrap();
    let our_val: serde_json::Value = serde_json::from_str(&our_json).unwrap();
    assert_eq!(ref_val, our_val, "prompt request mismatch");
}

#[test]
fn cancel_notification_matches() {
    let ref_req = acp::CancelNotification::new(acp::SessionId::new("sess-1"));
    let ref_json = serde_json::to_string(&ref_req).unwrap();

    let our_req = facet_acp::CancelNotification::new("sess-1");
    let our_json = facet_json::to_string(&our_req).unwrap();

    eprintln!("REF: {ref_json}");
    eprintln!("OUR: {our_json}");

    let ref_val: serde_json::Value = serde_json::from_str(&ref_json).unwrap();
    let our_val: serde_json::Value = serde_json::from_str(&our_json).unwrap();
    assert_eq!(ref_val, our_val, "cancel notification mismatch");
}

#[test]
fn set_session_model_request_matches() {
    let ref_req = acp::SetSessionModelRequest::new(
        acp::SessionId::new("sess-1"),
        acp::ModelId::new("claude-3"),
    );
    let ref_json = serde_json::to_string(&ref_req).unwrap();

    let our_req = facet_acp::SetSessionModelRequest::new("sess-1", facet_acp::ModelId::new("claude-3"));
    let our_json = facet_json::to_string(&our_req).unwrap();

    eprintln!("REF: {ref_json}");
    eprintln!("OUR: {our_json}");

    let ref_val: serde_json::Value = serde_json::from_str(&ref_json).unwrap();
    let our_val: serde_json::Value = serde_json::from_str(&our_json).unwrap();
    assert_eq!(ref_val, our_val, "set_session_model request mismatch");
}

#[test]
fn mcp_server_stdio_matches() {
    let ref_server = acp::McpServer::Stdio(
        acp::McpServerStdio::new("my-server", "/usr/bin/server")
            .args(vec!["--flag".to_owned()])
            .env(vec![acp::EnvVariable::new("KEY", "VAL")]),
    );
    let ref_json = serde_json::to_string(&ref_server).unwrap();

    let our_server = facet_acp::McpServer::Stdio(
        facet_acp::McpServerStdio::new("my-server", "/usr/bin/server")
            .args(vec!["--flag".to_owned()])
            .env(vec![facet_acp::EnvVariable::new("KEY", "VAL")]),
    );
    let our_json = facet_json::to_string(&our_server).unwrap();

    eprintln!("REF: {ref_json}");
    eprintln!("OUR: {our_json}");

    let ref_val: serde_json::Value = serde_json::from_str(&ref_json).unwrap();
    let our_val: serde_json::Value = serde_json::from_str(&our_json).unwrap();
    assert_eq!(ref_val, our_val, "mcp_server stdio mismatch");
}

#[test]
fn content_block_text_matches() {
    let ref_block = acp::ContentBlock::from("hello");
    let ref_json = serde_json::to_string(&ref_block).unwrap();

    let our_block = facet_acp::ContentBlock::from("hello");
    let our_json = facet_json::to_string(&our_block).unwrap();

    eprintln!("REF: {ref_json}");
    eprintln!("OUR: {our_json}");

    let ref_val: serde_json::Value = serde_json::from_str(&ref_json).unwrap();
    let our_val: serde_json::Value = serde_json::from_str(&our_json).unwrap();
    assert_eq!(ref_val, our_val, "content block text mismatch");
}

#[test]
fn tool_call_matches() {
    let ref_tc = acp::ToolCall::new(acp::ToolCallId::new("toolu_1"), "Read file")
        .kind(acp::ToolKind::Read)
        .status(acp::ToolCallStatus::Completed);
    let ref_json = serde_json::to_string(&ref_tc).unwrap();

    let our_tc = facet_acp::ToolCall::new("toolu_1", "Read file")
        .kind(facet_acp::ToolKind::Read)
        .status(facet_acp::ToolCallStatus::Completed);
    let our_json = facet_json::to_string(&our_tc).unwrap();

    eprintln!("REF: {ref_json}");
    eprintln!("OUR: {our_json}");

    let ref_val: serde_json::Value = serde_json::from_str(&ref_json).unwrap();
    let our_val: serde_json::Value = serde_json::from_str(&our_json).unwrap();
    assert_eq!(ref_val, our_val, "tool call mismatch");
}

/// Also test that we can deserialize what the reference produces
#[test]
fn deserialize_ref_initialize_response() {
    let ref_resp = acp::InitializeResponse::new(acp::ProtocolVersion::LATEST)
        .agent_capabilities(acp::AgentCapabilities::default())
        .agent_info(acp::Implementation::new("test-agent", "1.0.0"));
    let ref_json = serde_json::to_string(&ref_resp).unwrap();

    eprintln!("REF InitializeResponse: {ref_json}");

    let our_resp: facet_acp::InitializeResponse = facet_json::from_str(&ref_json).unwrap();
    assert_eq!(our_resp.protocol_version, facet_acp::ProtocolVersion::V1);
    assert_eq!(our_resp.agent_info.unwrap().name, "test-agent");
}

#[test]
fn deserialize_ref_new_session_response() {
    let ref_resp = acp::NewSessionResponse::new(acp::SessionId::new("sess-abc"));
    let ref_json = serde_json::to_string(&ref_resp).unwrap();

    eprintln!("REF NewSessionResponse: {ref_json}");

    let our_resp: facet_acp::NewSessionResponse = facet_json::from_str(&ref_json).unwrap();
    assert_eq!(our_resp.session_id.0.as_ref(), "sess-abc");
}
