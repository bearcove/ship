use std::path::PathBuf;

use ship_core::{JsonSessionStore, SessionStore};
use ship_types::{
    AgentKind, AgentSnapshot, AgentState, AutonomyMode, BlockId, ContentBlock, CurrentTask,
    McpServerConfig, McpStdioServerConfig, PersistedSession, ProjectName, Role, SessionConfig,
    SessionEvent, SessionEventEnvelope, SessionId, TaskContentRecord, TaskId, TaskRecord,
    TaskStatus,
};

fn make_temp_dir(test_name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ship-core-{test_name}-{}", ulid::Ulid::new()));
    std::fs::create_dir_all(&dir).expect("temp dir should be created");
    dir
}

fn make_persisted_session(id: &str, description: &str) -> PersistedSession {
    PersistedSession {
        id: SessionId(id.to_owned()),
        config: SessionConfig {
            project: ProjectName("ship-backend".to_owned()),
            base_branch: "main".to_owned(),
            branch_name: format!("ship/{}/task", &id[..8]),
            captain_kind: AgentKind::Claude,
            mate_kind: AgentKind::Codex,
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
        },
        mate: AgentSnapshot {
            role: Role::Mate,
            kind: AgentKind::Codex,
            state: AgentState::Working {
                plan: None,
                activity: Some("coding".to_owned()),
            },
            context_remaining_percent: Some(75),
        },
        current_task: Some(CurrentTask {
            record: TaskRecord {
                id: TaskId::new(),
                description: description.to_owned(),
                status: TaskStatus::Working,
            },
            content_history: vec![TaskContentRecord {
                block_id: BlockId::new(),
                role: Role::Mate,
                block: ContentBlock::Text {
                    text: "Started implementation".to_owned(),
                },
            }],
            event_log: vec![SessionEventEnvelope {
                seq: 0,
                event: SessionEvent::TaskStarted {
                    task_id: TaskId::new(),
                    description: description.to_owned(),
                },
            }],
        }),
        task_history: Vec::new(),
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
