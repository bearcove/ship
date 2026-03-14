use std::path::PathBuf;

use ship_core::{JsonSessionStore, SessionGitNames, SessionStore};
use ship_types::{
    AgentKind, AgentSnapshot, AgentState, AutonomyMode, BlockId, ContentBlock, CurrentTask,
    McpServerConfig, McpStdioServerConfig, PersistedSession, ProjectName, Role, SessionConfig,
    SessionEvent, SessionEventEnvelope, SessionId, SessionStartupState, TaskContentRecord, TaskId,
    TaskRecord, TaskStatus,
};

#[derive(Debug, Clone, facet::Facet)]
struct LegacySessionConfig {
    project: ProjectName,
    base_branch: String,
    branch_name: String,
    captain_kind: AgentKind,
    mate_kind: AgentKind,
    autonomy_mode: AutonomyMode,
}

#[derive(Debug, Clone, facet::Facet)]
struct LegacyPersistedSession {
    id: SessionId,
    config: LegacySessionConfig,
    captain: AgentSnapshot,
    mate: AgentSnapshot,
    current_task: Option<CurrentTask>,
    task_history: Vec<TaskRecord>,
}

fn make_temp_dir(test_name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ship-core-{test_name}-{}", ulid::Ulid::new()));
    std::fs::create_dir_all(&dir).expect("temp dir should be created");
    dir
}

fn make_persisted_session(id: &str, description: &str) -> PersistedSession {
    let id = SessionId(id.to_owned());
    let branch_name = SessionGitNames::from_session_id(&id).branch_name;

    PersistedSession {
        id,
        created_at: "2026-01-01T00:00:00Z".to_owned(),
        config: SessionConfig {
            project: ProjectName("ship-backend".to_owned()),
            base_branch: "main".to_owned(),
            branch_name,
            captain_kind: AgentKind::Claude,
            mate_kind: AgentKind::Codex,
            captain_preset_id: None,
            mate_preset_id: None,
            captain_provider: Some(AgentKind::Claude.default_provider_id()),
            mate_provider: Some(AgentKind::Codex.default_provider_id()),
            captain_model_id: None,
            mate_model_id: None,
            autonomy_mode: AutonomyMode::HumanInTheLoop,
            mcp_servers: vec![McpServerConfig::Stdio(McpStdioServerConfig {
                name: "filesystem".to_owned(),
                command: "/usr/bin/fs-mcp".to_owned(),
                args: vec!["--root".to_owned(), "/repo".to_owned()],
                env: Vec::new(),
            })],
        },
        captain: AgentSnapshot {
            role: Role::Captain,
            kind: AgentKind::Claude,
            state: AgentState::Idle,
            context_remaining_percent: Some(80),
            preset_id: None,
            provider: Some(AgentKind::Claude.default_provider_id()),
            model_id: None,
            available_models: Vec::new(),
            effort_config_id: None,
            effort_value_id: None,
            available_effort_values: Vec::new(),
        },
        mate: AgentSnapshot {
            role: Role::Mate,
            kind: AgentKind::Codex,
            state: AgentState::Working {
                plan: None,
                activity: Some("coding".to_owned()),
            },
            context_remaining_percent: Some(75),
            preset_id: None,
            provider: Some(AgentKind::Codex.default_provider_id()),
            model_id: None,
            available_models: Vec::new(),
            effort_config_id: None,
            effort_value_id: None,
            available_effort_values: Vec::new(),
        },
        startup_state: SessionStartupState::Ready,
        session_event_log: Vec::new(),
        current_task: Some(CurrentTask {
            record: TaskRecord {
                id: TaskId::new(),
                title: description.to_owned(),
                description: description.to_owned(),
                status: TaskStatus::Working,
                steps: Vec::new(),
                assigned_at: None,
                completed_at: None,
            },
            pending_mate_guidance: None,
            content_history: vec![TaskContentRecord {
                block_id: BlockId::new(),
                role: Role::Mate,
                block: ContentBlock::Text {
                    text: "Started implementation".to_owned(),
                    source: ship_types::TextSource::AgentMessage,
                },
            }],
            event_log: vec![SessionEventEnvelope {
                seq: 0,
                timestamp: "2026-01-01T00:00:00Z".to_owned(),
                event: SessionEvent::TaskStarted {
                    task_id: TaskId::new(),
                    title: description.to_owned(),
                    description: description.to_owned(),
                    steps: Vec::new(),
                },
            }],
        }),
        task_history: Vec::new(),
        title: None,
        archived_at: None,
        captain_acp_session_id: None,
        mate_acp_session_id: None,
    }
}

// r[verify testability.persistence-trait]
#[tokio::test]
async fn json_store_round_trip() {
    let dir = make_temp_dir("json-store");
    let store = JsonSessionStore::new(dir.clone());

    let first = make_persisted_session("01J00000000000000000000001", "Implement store");
    let second = make_persisted_session("01J00000000000000000000002", "Add tests");

    store
        .save_session(&first)
        .await
        .expect("first save should work");
    store
        .save_session(&second)
        .await
        .expect("second save should work");

    let loaded = store
        .load_session(&first.id)
        .await
        .expect("load should work")
        .expect("session should exist");
    assert_eq!(loaded.id.0, first.id.0);
    assert_eq!(loaded.config.branch_name, first.config.branch_name);
    assert_eq!(loaded.config.mcp_servers, first.config.mcp_servers);
    assert_eq!(
        loaded
            .current_task
            .as_ref()
            .expect("task should exist")
            .record
            .description,
        "Implement store"
    );

    let missing = store
        .load_session(&SessionId("01J00000000000000000000009".to_owned()))
        .await
        .expect("missing load should work");
    assert!(missing.is_none());

    let sessions = store.list_sessions().await.expect("list should work");
    assert_eq!(sessions.len(), 2);
    assert!(
        sessions
            .iter()
            .any(|session| session.id.0 == "01J00000000000000000000001")
    );
    assert!(
        sessions
            .iter()
            .any(|session| session.id.0 == "01J00000000000000000000002")
    );

    store
        .delete_session(&first.id)
        .await
        .expect("delete should work");
    let after_delete = store
        .load_session(&first.id)
        .await
        .expect("load after delete should work");
    assert!(after_delete.is_none());

    let sessions_after = store
        .list_sessions()
        .await
        .expect("list after delete should work");
    assert_eq!(sessions_after.len(), 1);
    assert_eq!(sessions_after[0].id.0, "01J00000000000000000000002");

    let _ = std::fs::remove_dir_all(&dir);
}

// r[verify session.persistent]
#[tokio::test]
async fn json_store_loads_legacy_sessions_without_mcp_servers() {
    let dir = make_temp_dir("json-store-legacy");
    let store = JsonSessionStore::new(dir.clone());
    let id = SessionId("01J00000000000000000000003".to_owned());

    let legacy = LegacyPersistedSession {
        id: id.clone(),
        config: LegacySessionConfig {
            project: ProjectName("ship-backend".to_owned()),
            base_branch: "main".to_owned(),
            branch_name: "ship/01J00000/task".to_owned(),
            captain_kind: AgentKind::Claude,
            mate_kind: AgentKind::Codex,
            autonomy_mode: AutonomyMode::HumanInTheLoop,
        },
        captain: AgentSnapshot {
            role: Role::Captain,
            kind: AgentKind::Claude,
            state: AgentState::Idle,
            context_remaining_percent: None,
            preset_id: None,
            provider: Some(AgentKind::Claude.default_provider_id()),
            model_id: None,
            available_models: Vec::new(),
            effort_config_id: None,
            effort_value_id: None,
            available_effort_values: Vec::new(),
        },
        mate: AgentSnapshot {
            role: Role::Mate,
            kind: AgentKind::Codex,
            state: AgentState::Idle,
            context_remaining_percent: None,
            preset_id: None,
            provider: Some(AgentKind::Codex.default_provider_id()),
            model_id: None,
            available_models: Vec::new(),
            effort_config_id: None,
            effort_value_id: None,
            available_effort_values: Vec::new(),
        },
        current_task: None,
        task_history: Vec::new(),
    };

    let bytes = facet_json::to_vec_pretty(&legacy).expect("legacy session should serialize");
    std::fs::write(dir.join(format!("{}.json", id.0)), bytes).expect("legacy session should write");

    let loaded = store
        .load_session(&id)
        .await
        .expect("legacy session should load")
        .expect("legacy session should exist");

    assert!(loaded.config.mcp_servers.is_empty());
    assert_eq!(
        store.list_sessions().await.expect("list should work").len(),
        1
    );

    let _ = std::fs::remove_dir_all(&dir);
}
