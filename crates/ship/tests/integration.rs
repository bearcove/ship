use std::sync::Arc;

use roam::{ConnectionSettings, MetadataEntry, MetadataFlags, MetadataValue, NoopCaller, Parity};
use ship::AppState;
use ship_db::ShipDb;
use ship_frontend_service::FrontendClient;
use ship_policy::{AgentRole, Lane, Participant, RoomId, Topology};
use ship_runtime::Runtime;
use ship_tool_service::ToolBackendClient;
use tokio::sync::Mutex;

fn metadata_string(key: &'static str, value: String) -> MetadataEntry<'static> {
    MetadataEntry {
        key,
        value: MetadataValue::String(Box::leak(value.into_boxed_str())),
        flags: MetadataFlags::NONE,
    }
}

fn test_topology() -> Topology {
    Topology {
        human: Participant::human("Amos"),
        admiral: Participant::agent("Admiral", AgentRole::Admiral),
        lanes: vec![Lane {
            id: RoomId::from_static("session:s1"),
            captain: Participant::agent("Alex", AgentRole::Captain),
            mate: Participant::agent("Jordan", AgentRole::Mate),
        }],
    }
}

fn stub_session(id: &str) -> ship_types::PersistedSession {
    ship_types::PersistedSession {
        id: ship_types::SessionId(id.to_owned()),
        created_at: "2025-01-01T00:00:00Z".to_owned(),
        config: ship_types::SessionConfig {
            project: ship_types::ProjectName("test-project".to_owned()),
            base_branch: "main".to_owned(),
            branch_name: format!("ship/{id}/task"),
            captain_kind: ship_types::AgentKind::Claude,
            mate_kind: ship_types::AgentKind::Claude,
            captain_preset_id: None,
            mate_preset_id: None,
            captain_provider: None,
            mate_provider: None,
            captain_model_id: None,
            mate_model_id: None,
            autonomy_mode: ship_types::AutonomyMode::HumanInTheLoop,
            mcp_servers: Vec::new(),
            workflow: Default::default(),
        },
        captain: ship_types::AgentSnapshot {
            role: ship_types::Role::Captain,
            kind: ship_types::AgentKind::Claude,
            state: ship_types::AgentState::Idle,
            context_remaining_percent: None,
            preset_id: None,
            provider: None,
            model_id: None,
            available_models: Vec::new(),
            effort_config_id: None,
            effort_value_id: None,
            available_effort_values: Vec::new(),
        },
        mate: ship_types::AgentSnapshot {
            role: ship_types::Role::Mate,
            kind: ship_types::AgentKind::Claude,
            state: ship_types::AgentState::Idle,
            context_remaining_percent: None,
            preset_id: None,
            provider: None,
            model_id: None,
            available_models: Vec::new(),
            effort_config_id: None,
            effort_value_id: None,
            available_effort_values: Vec::new(),
        },
        startup_state: ship_types::SessionStartupState::Ready,
        session_event_log: Vec::new(),
        current_task: None,
        task_history: Vec::new(),
        title: Some("Test session".to_owned()),
        archived_at: None,
        captain_acp_session_id: None,
        mate_acp_session_id: None,
        is_read: false,
        captain_has_ever_assigned: false,
        captain_delegation_reminded: false,
    }
}

/// Start a ship server on a random port, returning the base URL and the runtime.
async fn start_server() -> (String, Arc<Mutex<Runtime>>) {
    let db = ShipDb::open_in_memory().unwrap();
    db.save_session(&stub_session("s1")).unwrap();
    let mut rt = Runtime::new(db);
    rt.init_topology(test_topology()).unwrap();
    let runtime = Arc::new(Mutex::new(rt));

    let state = AppState {
        runtime: runtime.clone(),
    };
    let app = ship::router(state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    (format!("ws://127.0.0.1:{}", addr.port()), runtime)
}

/// Connect as a frontend client.
async fn connect_frontend(base_url: &str) -> FrontendClient {
    let url = format!("{base_url}/ws/frontend");
    let ws_stream = tokio_tungstenite::connect_async(&url).await.unwrap().0;
    let link = roam_websocket::WsLink::new(ws_stream);
    let (client, _session_handle) = roam::initiator(link)
        .establish::<FrontendClient>(())
        .await
        .unwrap();
    client
}

/// Connect as a tool backend client with participant + room identity.
async fn connect_tool(base_url: &str, participant: &str, room_id: &str) -> ToolBackendClient {
    let url = format!("{base_url}/ws/tool");
    let ws_stream = tokio_tungstenite::connect_async(&url).await.unwrap().0;
    let link = roam_websocket::WsLink::new(ws_stream);
    let (_root_guard, session_handle) = roam::initiator(link)
        .establish::<NoopCaller>(())
        .await
        .unwrap();

    let connection = session_handle
        .open_connection(
            ConnectionSettings {
                parity: Parity::Odd,
                max_concurrent_requests: 64,
            },
            vec![
                metadata_string("ship-participant", participant.to_owned()),
                metadata_string("ship-room-id", room_id.to_owned()),
            ],
        )
        .await
        .unwrap();

    let mut driver = roam::Driver::new(connection, ());
    let caller = driver.caller();
    tokio::spawn(async move {
        driver.run().await;
    });

    ToolBackendClient::from(caller)
}

// ── Frontend tests ───────────────────────────────────────────────────

#[tokio::test]
async fn frontend_connect_returns_topology() {
    let (url, _rt) = start_server().await;
    let client = connect_frontend(&url).await;

    let snapshot = client.connect().await.unwrap();
    assert_eq!(snapshot.topology.lanes.len(), 1);
    assert_eq!(
        snapshot.topology.lanes[0].id,
        RoomId::from_static("session:s1")
    );
    assert_eq!(snapshot.topology.human.name.as_str(), "Amos");
    assert_eq!(snapshot.rooms.len(), 1);
}

// ── Tool backend tests ───────────────────────────────────────────────

#[tokio::test]
async fn tool_git_status_without_worktree() {
    let (url, _rt) = start_server().await;
    let client = connect_tool(&url, "Alex", "session:s1").await;

    let result = client.git_status().await.unwrap();
    assert!(result.is_error);
    assert!(result.text.contains("no worktree"));
}

#[tokio::test]
async fn tool_assign_task() {
    let (url, _rt) = start_server().await;
    let client = connect_tool(&url, "Alex", "session:s1").await;

    let result = client
        .assign_task("Fix the bug".to_owned(), "It crashes on startup".to_owned())
        .await
        .unwrap();
    assert!(!result.is_error, "assign_task failed: {}", result.text);
    assert!(result.text.contains("task assigned"));
}

#[tokio::test]
async fn tool_assign_then_submit() {
    let (url, _rt) = start_server().await;
    let client = connect_tool(&url, "Alex", "session:s1").await;

    let result = client
        .assign_task("Fix the bug".to_owned(), "It crashes".to_owned())
        .await
        .unwrap();
    assert!(!result.is_error);

    // Assigned → Working (mate starts work)
    {
        let mut rt = _rt.lock().await;
        rt.transition_task(
            &RoomId::from_static("session:s1"),
            ship_policy::TaskPhase::Working,
        )
        .unwrap();
    }

    // Working → PendingReview (mate submits)
    let result = client
        .submit("Fixed the crash".to_owned())
        .await
        .unwrap();
    assert!(!result.is_error, "submit failed: {}", result.text);
}

#[tokio::test]
async fn tool_send_message() {
    let (url, _rt) = start_server().await;
    let client = connect_tool(&url, "Alex", "session:s1").await;

    let result = client
        .send_message(
            ship_policy::ParticipantName::new("Jordan".to_owned()),
            "Please review".to_owned(),
        )
        .await
        .unwrap();
    assert!(!result.is_error, "send_message failed: {}", result.text);
}

#[tokio::test]
async fn tool_read_file_without_worktree() {
    let (url, _rt) = start_server().await;
    let client = connect_tool(&url, "Alex", "session:s1").await;

    let result = client
        .read_file("src/main.rs".to_owned(), None, None)
        .await
        .unwrap();
    assert!(result.is_error);
    assert!(result.text.contains("no worktree"));
}

#[tokio::test]
async fn tool_read_file_with_worktree() {
    let (url, rt) = start_server().await;

    // Create a temp dir with a file
    let tmp = tempfile::tempdir().unwrap();
    let file_path = tmp.path().join("hello.txt");
    tokio::fs::write(&file_path, "line one\nline two\nline three\n")
        .await
        .unwrap();

    // Register the worktree
    {
        let mut runtime = rt.lock().await;
        runtime.register_worktree(
            &RoomId::from_static("session:s1"),
            camino::Utf8PathBuf::from(tmp.path().to_str().unwrap()),
        );
    }

    let client = connect_tool(&url, "Alex", "session:s1").await;
    let result = client
        .read_file("hello.txt".to_owned(), None, None)
        .await
        .unwrap();
    assert!(!result.is_error, "read_file failed: {}", result.text);
    assert!(result.text.contains("line one"));
    assert!(result.text.contains("line three"));
}

#[tokio::test]
async fn tool_write_and_read_file() {
    let (url, rt) = start_server().await;

    let tmp = tempfile::tempdir().unwrap();
    {
        let mut runtime = rt.lock().await;
        runtime.register_worktree(
            &RoomId::from_static("session:s1"),
            camino::Utf8PathBuf::from(tmp.path().to_str().unwrap()),
        );
    }

    let client = connect_tool(&url, "Alex", "session:s1").await;

    let result = client
        .write_file("test.txt".to_owned(), "hello world".to_owned())
        .await
        .unwrap();
    assert!(!result.is_error, "write_file failed: {}", result.text);

    let result = client
        .read_file("test.txt".to_owned(), None, None)
        .await
        .unwrap();
    assert!(!result.is_error);
    assert_eq!(result.text, "hello world");
}

#[tokio::test]
async fn tool_list_files() {
    let (url, rt) = start_server().await;

    let tmp = tempfile::tempdir().unwrap();
    // Init a git repo so ls_files works
    tokio::process::Command::new("git")
        .args(["init"])
        .current_dir(tmp.path())
        .output()
        .await
        .unwrap();
    tokio::fs::write(tmp.path().join("foo.rs"), "fn main() {}")
        .await
        .unwrap();
    tokio::fs::write(tmp.path().join("bar.rs"), "fn bar() {}")
        .await
        .unwrap();
    tokio::process::Command::new("git")
        .args(["add", "."])
        .current_dir(tmp.path())
        .output()
        .await
        .unwrap();

    {
        let mut runtime = rt.lock().await;
        runtime.register_worktree(
            &RoomId::from_static("session:s1"),
            camino::Utf8PathBuf::from(tmp.path().to_str().unwrap()),
        );
    }

    let client = connect_tool(&url, "Alex", "session:s1").await;
    let files = client.list_files("foo".to_owned()).await.unwrap();
    assert_eq!(files, vec!["foo.rs"]);
}
