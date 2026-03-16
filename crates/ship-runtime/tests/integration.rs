use camino::Utf8PathBuf;
use ship_db::ShipDb;
use ship_git::{BranchName, GitContext, Rev};
use ship_policy::{
    AgentRole, BlockContent, Lane, Participant, ParticipantName, RoomId, TaskPhase, Topology,
};
use ship_runtime::Runtime;

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

    // Seal it — no mention, so deliveries come from unaddressed routing.
    let deliveries = rt.seal_block(&room_id, &block_id).unwrap();
    // Unaddressed message from captain in a session room — policy decides what happens.
    let _ = deliveries;

    let blocks = rt.blocks(&room_id).unwrap();
    assert_eq!(blocks.len(), 1);
    assert!(blocks[0].is_sealed());
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
    let _ = rt.seal_block(&room_id, &b1).unwrap();

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
    let _ = rt.seal_block(&room_id, &b2).unwrap();

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
    let _ = rt.seal_block(&room_id, &b3).unwrap();

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

// ── Policy tests ──────────────────────────────────────────────────

#[test]
fn non_member_cannot_post() {
    let mut rt = test_runtime();
    let room_id = RoomId::from_static("session:s1");

    // Admiral is not a member of session:s1.
    let result = rt.open_block(
        &room_id,
        Some(ParticipantName::new("Admiral".to_owned())),
        None,
        BlockContent::Text {
            text: "I shouldn't be here".to_owned(),
        },
    );
    assert!(result.is_err());
}

#[test]
fn mention_produces_deliveries() {
    let mut rt = test_runtime();
    let room_id = RoomId::from_static("session:s1");

    // Alex (captain) mentions Jordan (mate) — this is allowed.
    let block_id = rt
        .open_block(
            &room_id,
            Some(ParticipantName::new("Alex".to_owned())),
            None,
            BlockContent::Text {
                text: "@Jordan please fix the tests".to_owned(),
            },
        )
        .unwrap();

    let deliveries = rt.seal_block(&room_id, &block_id).unwrap();
    assert!(
        !deliveries.is_empty(),
        "mention of mate by captain should produce deliveries"
    );
    assert!(
        deliveries.iter().any(|d| d.to.as_str() == "Jordan"),
        "should have a delivery to Jordan, got: {deliveries:?}"
    );

    // Process the deliveries — they should land as blocks in the room.
    let delivered = rt.process_deliveries(deliveries).unwrap();
    assert!(delivered > 0, "at least one delivery should be processed");

    // The room should now have more blocks: the original + the delivered ones.
    let blocks = rt.blocks(&room_id).unwrap();
    assert!(
        blocks.len() > 1,
        "room should have original block + delivered block(s), got {}",
        blocks.len()
    );

    // The delivered block should be addressed to Jordan.
    let delivered_block = blocks.iter().find(|b| {
        b.to.as_ref().is_some_and(|to| to.as_str() == "Jordan")
    });
    assert!(
        delivered_block.is_some(),
        "should have a block addressed to Jordan"
    );
    // And it should be sealed (deliveries are complete).
    assert!(delivered_block.unwrap().is_sealed());
}

// ── Task lifecycle tests ──────────────────────────────────────────

#[test]
fn task_happy_path() {
    let mut rt = test_runtime();
    let room_id = RoomId::from_static("session:s1");

    // No task initially.
    assert!(rt.current_task(&room_id).unwrap().is_none());

    // Assign a task.
    let task_id = rt
        .assign_task(&room_id, "Fix the bug".to_owned(), "It's broken".to_owned())
        .unwrap();

    // Task should be Assigned.
    let task = rt.current_task(&room_id).unwrap().unwrap();
    assert_eq!(task.id, task_id);
    assert_eq!(task.phase, TaskPhase::Assigned);
    assert_eq!(task.title, "Fix the bug");

    // Transition: Assigned → Working
    rt.transition_task(&room_id, TaskPhase::Working).unwrap();
    assert_eq!(
        rt.current_phase(&room_id).unwrap(),
        Some(TaskPhase::Working)
    );

    // Transition: Working → PendingReview
    rt.transition_task(&room_id, TaskPhase::PendingReview).unwrap();
    assert_eq!(
        rt.current_phase(&room_id).unwrap(),
        Some(TaskPhase::PendingReview)
    );

    // Transition: PendingReview → Accepted (terminal)
    rt.transition_task(&room_id, TaskPhase::Accepted).unwrap();

    // Task is now terminal — current_task should be None.
    assert!(rt.current_task(&room_id).unwrap().is_none());
}

#[test]
fn task_steer_sends_back_to_working() {
    let mut rt = test_runtime();
    let room_id = RoomId::from_static("session:s1");

    rt.assign_task(&room_id, "Task".to_owned(), "Desc".to_owned())
        .unwrap();
    rt.transition_task(&room_id, TaskPhase::Working).unwrap();
    rt.transition_task(&room_id, TaskPhase::PendingReview).unwrap();

    // Captain steers: PendingReview → Working
    rt.transition_task(&room_id, TaskPhase::Working).unwrap();
    assert_eq!(
        rt.current_phase(&room_id).unwrap(),
        Some(TaskPhase::Working)
    );
}

#[test]
fn task_invalid_transition_rejected() {
    let mut rt = test_runtime();
    let room_id = RoomId::from_static("session:s1");

    rt.assign_task(&room_id, "Task".to_owned(), "Desc".to_owned())
        .unwrap();

    // Assigned → Accepted is valid (early accept).
    // But Assigned → PendingReview is NOT valid.
    let result = rt.transition_task(&room_id, TaskPhase::PendingReview);
    assert!(result.is_err());
}

#[test]
fn task_cancel_from_any_phase() {
    let mut rt = test_runtime();
    let room_id = RoomId::from_static("session:s1");

    rt.assign_task(&room_id, "Task".to_owned(), "Desc".to_owned())
        .unwrap();
    rt.transition_task(&room_id, TaskPhase::Working).unwrap();

    // Cancel from Working.
    rt.transition_task(&room_id, TaskPhase::Cancelled).unwrap();
    assert!(rt.current_task(&room_id).unwrap().is_none());
}

#[test]
fn cannot_assign_task_while_one_is_active() {
    let mut rt = test_runtime();
    let room_id = RoomId::from_static("session:s1");

    rt.assign_task(&room_id, "Task 1".to_owned(), "Desc".to_owned())
        .unwrap();

    let result = rt.assign_task(&room_id, "Task 2".to_owned(), "Desc".to_owned());
    assert!(result.is_err());
}

#[test]
fn can_assign_new_task_after_previous_completes() {
    let mut rt = test_runtime();
    let room_id = RoomId::from_static("session:s1");

    // First task.
    rt.assign_task(&room_id, "Task 1".to_owned(), "Desc".to_owned())
        .unwrap();
    rt.transition_task(&room_id, TaskPhase::Working).unwrap();
    rt.transition_task(&room_id, TaskPhase::PendingReview).unwrap();
    rt.transition_task(&room_id, TaskPhase::Accepted).unwrap();

    // Lane is idle now — second task.
    let task_id = rt
        .assign_task(&room_id, "Task 2".to_owned(), "More work".to_owned())
        .unwrap();

    let task = rt.current_task(&room_id).unwrap().unwrap();
    assert_eq!(task.id, task_id);
    assert_eq!(task.title, "Task 2");
    assert_eq!(task.phase, TaskPhase::Assigned);
}

// ── Git commit recording ────────────────────────────────────────────

async fn setup_git_repo() -> (GitContext, Utf8PathBuf) {
    let dir = Utf8PathBuf::try_from(
        std::env::temp_dir().join(format!("ship-runtime-test-{}", std::process::id())),
    )
    .unwrap();
    let sub = dir.join(format!("{}", ulid::Ulid::new()));
    let ctx = GitContext::init(&sub, &BranchName::new("main"))
        .await
        .expect("git init");
    ctx.config_set("user.email", "test@test.com").await.unwrap();
    ctx.config_set("user.name", "Test").await.unwrap();

    // Initial commit so HEAD exists.
    tokio::fs::write(sub.join("README.md").as_std_path(), "# hello\n")
        .await
        .unwrap();
    ctx.add_all().await.unwrap();
    ctx.commit("initial commit").await.unwrap();

    (ctx, sub)
}

#[tokio::test]
async fn record_commit_tracks_diff_stats() {
    let (git_ctx, repo_dir) = setup_git_repo().await;
    let base = git_ctx.rev_parse(&Rev::from("HEAD")).await.unwrap();

    // Make a change: add 3 lines, remove 1.
    tokio::fs::write(
        repo_dir.join("README.md").as_std_path(),
        "# hello\nline 2\nline 3\n",
    )
    .await
    .unwrap();
    tokio::fs::write(
        repo_dir.join("new_file.txt").as_std_path(),
        "some content\n",
    )
    .await
    .unwrap();
    git_ctx.add_all().await.unwrap();
    git_ctx.commit("add stuff").await.unwrap();

    // Set up runtime with the git context.
    let mut rt = test_runtime();
    let room_id = RoomId::from_static("session:s1");
    rt.register_worktree(&room_id, repo_dir.clone());

    // Assign a task so there's something to record against.
    rt.assign_task(&room_id, "Fix bug".to_owned(), "Details".to_owned())
        .unwrap();
    rt.transition_task(&room_id, TaskPhase::Working).unwrap();

    // Record the commit.
    let base_rev = Rev::from(base);
    let stats = rt.record_commit(&room_id, &base_rev).await.unwrap();
    let stats = stats.expect("should have stats");

    assert!(stats.lines_added > 0);
    assert!(stats.files_changed > 0);

    // Task should have updated cumulative stats.
    let task = rt.current_task(&room_id).unwrap().unwrap();
    assert_eq!(task.lines_added, stats.lines_added);
    assert_eq!(task.lines_removed, stats.lines_removed);
    assert_eq!(task.commit_count, 1);

    // Second commit — stats accumulate.
    let base2 = git_ctx.rev_parse(&Rev::from("HEAD")).await.unwrap();
    tokio::fs::write(
        repo_dir.join("another.txt").as_std_path(),
        "more\nlines\nhere\n",
    )
    .await
    .unwrap();
    git_ctx.add_all().await.unwrap();
    git_ctx.commit("add another").await.unwrap();

    let stats2 = rt
        .record_commit(&room_id, &Rev::from(base2))
        .await
        .unwrap()
        .unwrap();

    let task = rt.current_task(&room_id).unwrap().unwrap();
    assert_eq!(task.lines_added, stats.lines_added + stats2.lines_added);
    assert_eq!(task.commit_count, 2);

    // Cleanup.
    let _ = tokio::fs::remove_dir_all(repo_dir.as_std_path()).await;
}

#[tokio::test]
async fn record_commit_returns_none_without_git_context() {
    let mut rt = test_runtime();
    let room_id = RoomId::from_static("session:s1");
    rt.assign_task(&room_id, "Task".to_owned(), "Desc".to_owned())
        .unwrap();

    let result = rt
        .record_commit(&room_id, &Rev::from("HEAD"))
        .await
        .unwrap();
    assert!(result.is_none());
}

#[tokio::test]
async fn record_commit_returns_none_without_active_task() {
    let (_git_ctx, repo_dir) = setup_git_repo().await;
    let mut rt = test_runtime();
    let room_id = RoomId::from_static("session:s1");
    rt.register_worktree(&room_id, repo_dir.clone());

    // No task assigned.
    let result = rt
        .record_commit(&room_id, &Rev::from("HEAD"))
        .await
        .unwrap();
    assert!(result.is_none());

    let _ = tokio::fs::remove_dir_all(repo_dir.as_std_path()).await;
}
