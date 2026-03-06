use std::path::Path;
use std::time::Duration;

use ship_core::{
    FakeAgentDriver, FakePromptScript, FakeSessionStore, FakeWorktreeOps, SessionManager,
    SessionStore, StopReason,
};
use ship_types::{
    AgentKind, AgentState, AutonomyMode, BlockId, BlockPatch, CloseSessionResponse, ContentBlock,
    CreateSessionRequest, McpServerConfig, McpStdioServerConfig, PlanStep, PlanStepPriority,
    PlanStepStatus, ProjectName, Role, SessionEvent, SessionEventEnvelope, SessionId, TaskId,
    TaskStatus,
};
use tokio::time::timeout;

fn make_request() -> CreateSessionRequest {
    CreateSessionRequest {
        project: ProjectName("ship-backend".to_owned()),
        captain_kind: AgentKind::Claude,
        mate_kind: AgentKind::Codex,
        base_branch: "main".to_owned(),
        mcp_servers: None,
    }
}

async fn create_session_with_task(
    manager: &mut SessionManager<FakeAgentDriver, FakeWorktreeOps, FakeSessionStore>,
    description: &str,
) -> (SessionId, TaskId) {
    let session_id = manager
        .create_session(make_request(), Path::new("/repo"))
        .await
        .expect("create session should succeed");
    let task_id = manager
        .assign(&session_id, description.to_owned())
        .await
        .expect("assign should succeed");
    (session_id, task_id)
}

fn make_manager() -> (
    SessionManager<FakeAgentDriver, FakeWorktreeOps, FakeSessionStore>,
    FakeAgentDriver,
    FakeWorktreeOps,
    FakeSessionStore,
) {
    let agent = FakeAgentDriver::default();
    let worktree = FakeWorktreeOps::default();
    let store = FakeSessionStore::default();
    let manager = SessionManager::new(agent.clone(), worktree.clone(), store.clone());
    (manager, agent, worktree, store)
}

async fn recv_task_status(
    rx: &mut tokio::sync::broadcast::Receiver<SessionEventEnvelope>,
) -> (ship_types::TaskId, TaskStatus) {
    loop {
        let envelope = timeout(Duration::from_secs(1), rx.recv())
            .await
            .expect("should receive event")
            .expect("broadcast should be open");
        if let SessionEvent::TaskStatusChanged { task_id, status } = envelope.event {
            return (task_id, status);
        }
    }
}

fn drain_replay(rx: &mut tokio::sync::broadcast::Receiver<SessionEventEnvelope>) {
    loop {
        match rx.try_recv() {
            Ok(_) => {}
            Err(tokio::sync::broadcast::error::TryRecvError::Empty) => return,
            Err(tokio::sync::broadcast::error::TryRecvError::Lagged(_)) => continue,
            Err(tokio::sync::broadcast::error::TryRecvError::Closed) => return,
        }
    }
}

fn plan_step(description: &str, priority: PlanStepPriority, status: PlanStepStatus) -> PlanStep {
    PlanStep {
        description: description.to_owned(),
        priority,
        status,
    }
}

// r[verify proto.create-session]
// r[verify session.persistent]
#[tokio::test]
async fn test_create_session() {
    let (mut manager, agent, worktree, store) = make_manager();

    agent.push_response(StopReason::EndTurn);
    agent.push_response(StopReason::EndTurn);
    agent.push_response(StopReason::EndTurn);

    let session_id = manager
        .create_session(make_request(), Path::new("/repo"))
        .await
        .expect("create session should succeed");

    let spawns = agent.spawn_records();
    assert_eq!(spawns.len(), 2);
    assert!(spawns.iter().any(|spawn| spawn.role == Role::Captain));
    assert!(spawns.iter().any(|spawn| spawn.role == Role::Mate));
    assert!(
        spawns
            .iter()
            .all(|spawn| spawn.session_config.mcp_servers.is_empty())
    );

    assert_eq!(worktree.created_paths().len(), 1);

    let persisted = store
        .load_session(&session_id)
        .await
        .expect("store load should work")
        .expect("session should be persisted");

    assert!(persisted.current_task.is_none());
    assert_eq!(
        persisted.startup_state,
        ship_types::SessionStartupState::Ready
    );
    assert!(persisted.config.mcp_servers.is_empty());
}

// r[verify acp.mcp.config]
#[tokio::test]
async fn test_create_session_persists_session_mcp_override() {
    let (mut manager, agent, _worktree, store) = make_manager();
    let mut request = make_request();
    request.mcp_servers = Some(vec![McpServerConfig::Stdio(McpStdioServerConfig {
        name: "tracey".to_owned(),
        command: "/usr/bin/tracey-mcp".to_owned(),
        args: vec!["serve".to_owned()],
        env: Vec::new(),
    })]);

    agent.push_response(StopReason::EndTurn);
    agent.push_response(StopReason::EndTurn);
    agent.push_response(StopReason::EndTurn);

    let session_id = manager
        .create_session(request, Path::new("/repo"))
        .await
        .expect("create session should succeed");

    let spawns = agent.spawn_records();
    assert_eq!(spawns.len(), 2);
    assert_eq!(spawns[0].session_config.mcp_servers.len(), 1);
    assert_eq!(spawns[1].session_config.mcp_servers.len(), 1);

    let persisted = store
        .load_session(&session_id)
        .await
        .expect("store load should work")
        .expect("session should be persisted");
    assert_eq!(persisted.config.mcp_servers.len(), 1);
}

// r[verify task.progress]
// r[verify proto.steer]
// r[verify proto.accept]
#[tokio::test]
async fn test_task_lifecycle() {
    let (mut manager, agent, _worktree, _store) = make_manager();

    agent.push_response(StopReason::EndTurn);
    agent.push_response(StopReason::EndTurn);
    agent.push_response(StopReason::EndTurn);

    let (session_id, _) = create_session_with_task(&mut manager, "Build lifecycle").await;

    manager
        .set_autonomy_mode(&session_id, AutonomyMode::Autonomous)
        .expect("mode should be set");

    let mut events = manager
        .subscribe(&session_id)
        .expect("subscribe should work");
    drain_replay(&mut events);

    agent.push_response(StopReason::EndTurn);
    agent.push_response(StopReason::EndTurn);

    manager
        .steer(&session_id, "Refactor the state machine".to_owned())
        .await
        .expect("steer should work in autonomous mode");

    let (first_task, first_status) = recv_task_status(&mut events).await;
    let (second_task, second_status) = recv_task_status(&mut events).await;

    assert_eq!(
        first_task,
        manager
            .get_session(&session_id)
            .expect("session exists")
            .current_task
            .as_ref()
            .expect("task exists")
            .record
            .id
            .clone()
    );
    assert_eq!(first_status, TaskStatus::Working);
    assert_eq!(
        second_task,
        manager
            .get_session(&session_id)
            .expect("session exists")
            .current_task
            .as_ref()
            .expect("task exists")
            .record
            .id
            .clone()
    );
    assert_eq!(second_status, TaskStatus::ReviewPending);

    manager
        .accept(&session_id)
        .await
        .expect("accept should succeed");

    let state = manager
        .get_session(&session_id)
        .expect("session should exist");
    assert!(state.current_task.is_none());
    assert_eq!(state.task_history.len(), 1);
    assert_eq!(state.task_history[0].status, TaskStatus::Accepted);
}

// r[verify event.block-id.plan]
// r[verify event.patch.plan-replace]
#[tokio::test]
async fn plan_updates_reuse_block_id_and_replace_the_full_step_list() {
    let (mut manager, agent, _worktree, _store) = make_manager();

    let first_plan = vec![
        plan_step(
            "Map the ACP shape",
            PlanStepPriority::High,
            PlanStepStatus::Pending,
        ),
        plan_step(
            "Remove stale fields",
            PlanStepPriority::Low,
            PlanStepStatus::InProgress,
        ),
    ];
    let second_plan = vec![plan_step(
        "Render priority in the UI",
        PlanStepPriority::Medium,
        PlanStepStatus::Completed,
    )];

    agent.push_script(FakePromptScript {
        expected_handle: None,
        response: Ok(ship_core::PromptResponse {
            stop_reason: StopReason::EndTurn,
        }),
        events: vec![
            SessionEvent::AgentStateChanged {
                role: Role::Mate,
                state: AgentState::Working {
                    plan: Some(first_plan.clone()),
                    activity: Some("planning".to_owned()),
                },
            },
            SessionEvent::AgentStateChanged {
                role: Role::Mate,
                state: AgentState::Working {
                    plan: Some(second_plan.clone()),
                    activity: Some("replanning".to_owned()),
                },
            },
        ],
    });
    agent.push_script(FakePromptScript {
        expected_handle: None,
        response: Ok(ship_core::PromptResponse {
            stop_reason: StopReason::EndTurn,
        }),
        events: Vec::new(),
    });

    let (session_id, _) =
        create_session_with_task(&mut manager, "Exercise plan block semantics").await;
    manager
        .set_autonomy_mode(&session_id, AutonomyMode::Autonomous)
        .expect("mode should be set");
    manager
        .steer(&session_id, "Start the mate".to_owned())
        .await
        .expect("steer should work");

    let state = manager
        .get_session(&session_id)
        .expect("session should exist");
    let task = state.current_task.expect("current task should exist");

    let plan_blocks = task
        .content_history
        .iter()
        .filter(|record| matches!(record.block, ContentBlock::PlanUpdate { .. }))
        .collect::<Vec<_>>();
    assert_eq!(plan_blocks.len(), 1);

    let final_steps = match &plan_blocks[0].block {
        ContentBlock::PlanUpdate { steps } => steps.clone(),
        other => panic!("unexpected final block: {other:?}"),
    };
    assert_eq!(final_steps, second_plan);
    assert_ne!(final_steps, first_plan);

    let plan_events = task
        .event_log
        .iter()
        .filter_map(|envelope| match &envelope.event {
            SessionEvent::BlockAppend {
                block_id,
                role,
                block: ContentBlock::PlanUpdate { steps },
            } if *role == Role::Mate => Some(("append", block_id.clone(), steps.clone())),
            SessionEvent::BlockPatch {
                block_id,
                role,
                patch: BlockPatch::PlanReplace { steps },
            } if *role == Role::Mate => Some(("patch", block_id.clone(), steps.clone())),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(plan_events.len(), 2);
    assert_eq!(plan_events[0].0, "append");
    assert_eq!(plan_events[0].2, first_plan);
    assert_eq!(plan_events[1].0, "patch");
    assert_eq!(plan_events[1].1, plan_events[0].1);
    assert_eq!(plan_events[1].2, second_plan);
}

// r[verify proto.cancel]
// r[verify task.status.terminal]
#[tokio::test]
async fn test_cancel_task() {
    let (mut manager, agent, _worktree, _store) = make_manager();

    agent.push_response(StopReason::EndTurn);
    agent.push_response(StopReason::ContextExhausted);

    let (session_id, _) = create_session_with_task(&mut manager, "Cancel me").await;

    manager
        .cancel(&session_id)
        .await
        .expect("cancel should succeed");

    assert!(agent.cancelled_handles().is_empty());

    let state = manager.get_session(&session_id).expect("session exists");
    assert!(state.current_task.is_none());
    assert_eq!(state.task_history.len(), 1);
    assert_eq!(state.task_history[0].status, TaskStatus::Cancelled);
}

// r[verify proto.resolve-permission]
// r[verify approval.request.content]
#[tokio::test]
async fn test_permission_flow() {
    let (mut manager, agent, _worktree, _store) = make_manager();

    agent.push_response(StopReason::EndTurn);
    agent.push_response(StopReason::ContextExhausted);

    let (session_id, _) = create_session_with_task(&mut manager, "Needs approvals").await;

    let mate_handle = agent
        .spawn_records()
        .into_iter()
        .find(|record| record.role == Role::Mate)
        .expect("mate should be spawned")
        .handle;

    let block_id = BlockId::new();

    let mut rx = manager
        .subscribe(&session_id)
        .expect("subscribe should work");
    drain_replay(&mut rx);

    agent.queue_notifications(
        &mate_handle,
        vec![SessionEvent::BlockAppend {
            block_id: block_id.clone(),
            role: Role::Mate,
            block: ContentBlock::Permission {
                tool_name: "write_file".to_owned(),
                arguments: "{\"path\":\"src/lib.rs\"}".to_owned(),
                description: "Write file".to_owned(),
                resolution: None,
            },
        }],
    );

    manager
        .drain_notifications(&session_id, Role::Mate)
        .await
        .expect("drain notifications should work");

    let envelope = loop {
        let envelope = timeout(Duration::from_secs(1), rx.recv())
            .await
            .expect("should receive event")
            .expect("broadcast should be open");
        if envelope.event
            == (SessionEvent::BlockAppend {
                block_id: block_id.clone(),
                role: Role::Mate,
                block: ContentBlock::Permission {
                    tool_name: "write_file".to_owned(),
                    arguments: "{\"path\":\"src/lib.rs\"}".to_owned(),
                    description: "Write file".to_owned(),
                    resolution: None,
                },
            })
        {
            break envelope;
        }
    };

    assert_eq!(
        envelope.event,
        SessionEvent::BlockAppend {
            block_id: block_id.clone(),
            role: Role::Mate,
            block: ContentBlock::Permission {
                tool_name: "write_file".to_owned(),
                arguments: "{\"path\":\"src/lib.rs\"}".to_owned(),
                description: "Write file".to_owned(),
                resolution: None,
            },
        }
    );

    manager
        .resolve_permission(&session_id, &block_id.0.to_string(), true)
        .await
        .expect("resolve permission should work");

    let state = manager
        .get_session(&session_id)
        .expect("session should exist");
    assert!(state.pending_permissions.is_empty());
    assert!(matches!(
        state.mate.state,
        AgentState::Working { .. } | AgentState::Idle
    ));
}

// r[verify autonomy.toggle]
#[tokio::test]
async fn test_autonomy_modes() {
    let (mut manager, agent, _worktree, _store) = make_manager();

    agent.push_response(StopReason::EndTurn);
    agent.push_response(StopReason::EndTurn);
    agent.push_response(StopReason::EndTurn);

    let (session_id, _) = create_session_with_task(&mut manager, "Human mode").await;

    let prompts_before = agent.prompt_log().len();
    manager
        .steer(&session_id, "Need another pass".to_owned())
        .await
        .expect("steer should work");
    let prompts_after = agent.prompt_log().len();

    let human_state = manager.get_session(&session_id).expect("session exists");
    assert_eq!(
        human_state
            .current_task
            .as_ref()
            .expect("task exists")
            .record
            .status,
        TaskStatus::SteerPending
    );
    assert_eq!(prompts_before, prompts_after);

    manager
        .set_autonomy_mode(&session_id, AutonomyMode::Autonomous)
        .expect("set autonomy mode should work");

    agent.push_response(StopReason::EndTurn);
    agent.push_response(StopReason::EndTurn);

    manager
        .steer(&session_id, "Autonomous steer".to_owned())
        .await
        .expect("autonomous steer should work");

    let autonomous_state = manager.get_session(&session_id).expect("session exists");
    assert_eq!(
        autonomous_state
            .current_task
            .as_ref()
            .expect("task exists")
            .record
            .status,
        TaskStatus::ReviewPending
    );
}

// r[verify event.subscribe]
// r[verify resilience.state-in-backend]
#[tokio::test]
async fn test_event_broadcast() {
    let (mut manager, agent, _worktree, _store) = make_manager();

    agent.push_response(StopReason::EndTurn);
    agent.push_response(StopReason::ContextExhausted);

    let (session_id, task_id) = create_session_with_task(&mut manager, "Broadcast").await;

    let mut rx1 = manager
        .subscribe(&session_id)
        .expect("subscribe should work");
    let mut rx2 = manager
        .subscribe(&session_id)
        .expect("subscribe should work");
    drain_replay(&mut rx1);
    drain_replay(&mut rx2);

    manager
        .cancel(&session_id)
        .await
        .expect("cancel should work");

    let event1 = timeout(Duration::from_secs(1), rx1.recv())
        .await
        .expect("rx1 should receive")
        .expect("broadcast should be open");
    let event2 = timeout(Duration::from_secs(1), rx2.recv())
        .await
        .expect("rx2 should receive")
        .expect("broadcast should be open");

    assert_eq!(
        event1.event,
        SessionEvent::TaskStatusChanged {
            task_id: task_id.clone(),
            status: TaskStatus::Cancelled,
        }
    );
    assert_eq!(
        event2.event,
        SessionEvent::TaskStatusChanged {
            task_id,
            status: TaskStatus::Cancelled,
        }
    );
}

// r[verify event.subscribe.replay]
// r[verify event.replay.per-subscriber]
#[tokio::test]
async fn replay_is_sent_only_to_the_new_subscriber() {
    let (mut manager, agent, _worktree, _store) = make_manager();

    agent.push_response(StopReason::EndTurn);
    agent.push_response(StopReason::ContextExhausted);

    let (session_id, task_id) = create_session_with_task(&mut manager, "Replay isolation").await;

    let mut first = manager
        .subscribe(&session_id)
        .expect("first subscribe should work");
    drain_replay(&mut first);

    let mut second = manager
        .subscribe(&session_id)
        .expect("second subscribe should work");

    let replay_started = timeout(Duration::from_secs(1), second.recv())
        .await
        .expect("second subscriber should receive replay")
        .expect("broadcast should be open");
    assert_eq!(
        replay_started.event,
        SessionEvent::TaskStarted {
            task_id: task_id.clone(),
            description: "Replay isolation".to_owned(),
        }
    );

    let no_replayed_event = timeout(Duration::from_millis(100), first.recv()).await;
    assert!(
        no_replayed_event.is_err(),
        "existing subscriber must not receive replay for a later subscriber"
    );
}

// r[verify backend.worktree-management]
// r[verify worktree.cleanup]
// r[verify worktree.cleanup-git]
#[tokio::test]
async fn test_close_session_cleans_up_clean_worktree() {
    let (mut manager, agent, worktree, store) = make_manager();

    agent.push_response(StopReason::EndTurn);
    agent.push_response(StopReason::EndTurn);
    agent.push_response(StopReason::EndTurn);

    let session_id = manager
        .create_session(make_request(), Path::new("/repo"))
        .await
        .expect("create session should work");

    let close = manager
        .close_session(&session_id, false)
        .await
        .expect("close session should work");

    assert_eq!(close, CloseSessionResponse::Closed);
    assert_eq!(agent.killed_handles().len(), 2);
    assert_eq!(
        worktree.remove_requests(),
        vec![(
            Path::new("/repo/.ship/worktrees/fake-1").to_path_buf(),
            false
        )]
    );
    assert_eq!(
        worktree.deleted_branches(),
        vec![(
            "ship/".to_owned() + &session_id.0[..8] + "/session",
            false,
            Path::new("/repo").to_path_buf()
        )]
    );
    assert!(
        manager.get_session(&session_id).is_err(),
        "closed session should be removed from active sessions"
    );
    assert!(
        store
            .load_session(&session_id)
            .await
            .expect("store load should work")
            .is_none(),
        "closed session should be deleted from persistence"
    );
}

// r[verify worktree.cleanup-uncommitted]
#[tokio::test]
async fn test_close_session_requires_confirmation_for_dirty_worktree() {
    let (mut manager, agent, worktree, _store) = make_manager();

    agent.push_response(StopReason::EndTurn);
    agent.push_response(StopReason::EndTurn);
    agent.push_response(StopReason::EndTurn);

    let session_id = manager
        .create_session(make_request(), Path::new("/repo"))
        .await
        .expect("create session should work");
    let worktree_path = worktree
        .created_paths()
        .into_iter()
        .next()
        .expect("worktree should exist");
    worktree.set_has_uncommitted_changes(worktree_path, true);

    let close = manager
        .close_session(&session_id, false)
        .await
        .expect("close session should work");

    assert_eq!(close, CloseSessionResponse::RequiresConfirmation);
    assert!(agent.killed_handles().is_empty());
    assert!(worktree.remove_requests().is_empty());
    assert!(worktree.deleted_branches().is_empty());
    assert!(
        manager.get_session(&session_id).is_ok(),
        "session should remain active until force is confirmed"
    );
}

// r[verify worktree.cleanup-uncommitted]
// r[verify worktree.cleanup-git]
#[tokio::test]
async fn test_close_session_force_deletes_dirty_worktree() {
    let (mut manager, agent, worktree, store) = make_manager();

    agent.push_response(StopReason::EndTurn);
    agent.push_response(StopReason::EndTurn);
    agent.push_response(StopReason::EndTurn);

    let session_id = manager
        .create_session(make_request(), Path::new("/repo"))
        .await
        .expect("create session should work");
    let worktree_path = worktree
        .created_paths()
        .into_iter()
        .next()
        .expect("worktree should exist");
    worktree.set_has_uncommitted_changes(worktree_path.clone(), true);

    let close = manager
        .close_session(&session_id, true)
        .await
        .expect("forced close session should work");

    assert_eq!(close, CloseSessionResponse::Closed);
    assert_eq!(agent.killed_handles().len(), 2);
    assert_eq!(worktree.remove_requests(), vec![(worktree_path, true)]);
    assert_eq!(
        worktree.deleted_branches(),
        vec![(
            "ship/".to_owned() + &session_id.0[..8] + "/session",
            true,
            Path::new("/repo").to_path_buf()
        )]
    );
    assert!(
        store
            .load_session(&session_id)
            .await
            .expect("store load should work")
            .is_none(),
        "forced close should delete persistence"
    );
}

// r[verify backend.worktree-management]
// r[verify worktree.cleanup-git]
#[tokio::test]
async fn test_close_session_keeps_session_when_cleanup_fails() {
    let (mut manager, agent, worktree, store) = make_manager();

    agent.push_response(StopReason::EndTurn);
    agent.push_response(StopReason::EndTurn);
    agent.push_response(StopReason::EndTurn);

    let session_id = manager
        .create_session(make_request(), Path::new("/repo"))
        .await
        .expect("create session should work");
    let worktree_path = worktree
        .created_paths()
        .into_iter()
        .next()
        .expect("worktree should exist");
    worktree.set_remove_error(worktree_path.clone(), "remove failed");

    let close = manager.close_session(&session_id, false).await;

    assert!(matches!(
        close,
        Err(ship_core::SessionManagerError::Worktree(message)) if message == "remove failed"
    ));
    assert!(
        manager.get_session(&session_id).is_ok(),
        "session should stay active when cleanup fails"
    );
    assert!(
        store
            .load_session(&session_id)
            .await
            .expect("store load should work")
            .is_some(),
        "failed close should keep persistence for retry"
    );
    assert_eq!(
        worktree.remove_requests(),
        Vec::<(std::path::PathBuf, bool)>::new()
    );
}
