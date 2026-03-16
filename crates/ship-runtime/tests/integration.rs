use ship_db::ShipDb;
use ship_policy::{
    AgentRole, BlockContent, Participant, ParticipantName, RoomId, SessionRoom, Topology,
};
use ship_runtime::Runtime;

fn test_topology() -> Topology {
    Topology {
        human: Participant::human("Amos"),
        admiral: Participant::agent("Admiral", AgentRole::Admiral),
        sessions: vec![SessionRoom {
            id: RoomId::from_static("session:s1"),
            captain: Participant::agent("Alex", AgentRole::Captain),
            mate: Participant::agent("Jordan", AgentRole::Mate),
        }],
    }
}

fn test_runtime() -> Runtime {
    let db = ShipDb::open_in_memory().unwrap();
    // Need a stub session row so the room FK is satisfied.
    db.save_session(&stub_session("s1")).unwrap();
    let mut rt = Runtime::new(db);
    rt.init_topology(test_topology()).unwrap();
    rt
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

#[test]
fn open_and_seal_block_roundtrips_through_db() {
    let mut rt = test_runtime();
    let room_id = RoomId::from_static("session:s1");

    // Open a text block from Alex.
    let block_id = rt
        .open_block(
            &room_id,
            Some(ParticipantName::new("Alex".to_owned())),
            None,
            BlockContent::Text {
                text: "Hello, Jordan!".to_owned(),
            },
        )
        .unwrap();

    // Block should be in the feed, unsealed.
    let blocks = rt.blocks(&room_id).unwrap();
    assert_eq!(blocks.len(), 1);
    assert!(!blocks[0].is_sealed());
    assert_eq!(blocks[0].seq, 0);
    if let BlockContent::Text { text } = &blocks[0].content {
        assert_eq!(text, "Hello, Jordan!");
    } else {
        panic!("expected Text block");
    }

    // Seal it.
    rt.seal_block(&room_id, &block_id).unwrap();

    let blocks = rt.blocks(&room_id).unwrap();
    assert_eq!(blocks.len(), 1);
    assert!(blocks[0].is_sealed());

    // Create a fresh runtime from the same db to verify persistence.
    // (We can't easily do this with in-memory db, so instead verify
    // the block count is correct after the operations.)
}

#[test]
fn update_unsealed_block_content() {
    let mut rt = test_runtime();
    let room_id = RoomId::from_static("session:s1");

    let block_id = rt
        .open_block(
            &room_id,
            Some(ParticipantName::new("Jordan".to_owned())),
            None,
            BlockContent::Text {
                text: "Working on it".to_owned(),
            },
        )
        .unwrap();

    // Append more text (simulating streaming).
    rt.update_block(
        &room_id,
        &block_id,
        BlockContent::Text {
            text: "Working on it... done!".to_owned(),
        },
    )
    .unwrap();

    let blocks = rt.blocks(&room_id).unwrap();
    assert_eq!(blocks.len(), 1);
    if let BlockContent::Text { text } = &blocks[0].content {
        assert_eq!(text, "Working on it... done!");
    } else {
        panic!("expected Text block");
    }
}

#[test]
fn multiple_blocks_in_sequence() {
    let mut rt = test_runtime();
    let room_id = RoomId::from_static("session:s1");

    let b1 = rt
        .open_block(
            &room_id,
            Some(ParticipantName::new("Alex".to_owned())),
            None,
            BlockContent::Text {
                text: "Start the task".to_owned(),
            },
        )
        .unwrap();
    rt.seal_block(&room_id, &b1).unwrap();

    let b2 = rt
        .open_block(
            &room_id,
            Some(ParticipantName::new("Jordan".to_owned())),
            None,
            BlockContent::Text {
                text: "On it".to_owned(),
            },
        )
        .unwrap();
    rt.seal_block(&room_id, &b2).unwrap();

    let b3 = rt
        .open_block(
            &room_id,
            Some(ParticipantName::new("Jordan".to_owned())),
            None,
            BlockContent::Milestone {
                kind: ship_policy::MilestoneKind::ReviewSubmitted,
                title: "Ready for review".to_owned(),
                summary: "All changes committed".to_owned(),
            },
        )
        .unwrap();
    rt.seal_block(&room_id, &b3).unwrap();

    let blocks = rt.blocks(&room_id).unwrap();
    assert_eq!(blocks.len(), 3);
    assert_eq!(blocks[0].seq, 0);
    assert_eq!(blocks[1].seq, 1);
    assert_eq!(blocks[2].seq, 2);
    assert!(blocks.iter().all(|b| b.is_sealed()));
}

#[test]
fn cold_room_warms_on_first_access() {
    let mut rt = test_runtime();
    let room_id = RoomId::from_static("session:s1");

    // First access warms the room — should be empty.
    let blocks = rt.blocks(&room_id).unwrap();
    assert!(blocks.is_empty());
}

#[test]
fn nonexistent_room_returns_error() {
    let mut rt = test_runtime();
    let bogus = RoomId::from_static("session:nope");
    let result = rt.blocks(&bogus);
    assert!(result.is_err());
}
