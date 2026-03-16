use crate::*;
use crate::prompts::*;
use sailfish::TemplateOnce;

fn test_topology() -> Topology {
    Topology {
        human: Participant::human("Amos"),
        admiral: Participant::agent("Morgan", AgentRole::Admiral),
        sessions: vec![
            SessionRoom {
                id: RoomId("lane-1".into()),
                captain: Participant::agent("Cedar", AgentRole::Captain),
                mate: Participant::agent("Jordan", AgentRole::Mate),
            },
            SessionRoom {
                id: RoomId("lane-2".into()),
                captain: Participant::agent("Birch", AgentRole::Captain),
                mate: Participant::agent("Riley", AgentRole::Mate),
            },
        ],
    }
}

// ── Identity ──────────────────────────────────────────────────────────

#[test]
fn participant_role() {
    let captain = Participant::agent("Cedar", AgentRole::Captain);
    assert_eq!(captain.role(), Some(AgentRole::Captain));

    let human = Participant::human("Amos");
    assert_eq!(human.role(), None);
    assert!(human.is_human());
}

// ── Topology lookup ───────────────────────────────────────────────────

#[test]
fn find_participant_across_topology() {
    let topo = test_topology();

    assert_eq!(topo.find_participant("Amos").unwrap().is_human(), true);
    assert_eq!(
        topo.find_participant("Morgan").unwrap().role(),
        Some(AgentRole::Admiral)
    );
    assert_eq!(
        topo.find_participant("Cedar").unwrap().role(),
        Some(AgentRole::Captain)
    );
    assert_eq!(
        topo.find_participant("Jordan").unwrap().role(),
        Some(AgentRole::Mate)
    );
    assert!(topo.find_participant("Nobody").is_none());
}

#[test]
fn admiral_room_contains_admiral_and_all_captains() {
    let topo = test_topology();
    let members = topo.admiral_room_members();
    let names: Vec<&str> = members.iter().map(|p| p.name.as_str()).collect();

    assert!(names.contains(&"Morgan"));
    assert!(names.contains(&"Cedar"));
    assert!(names.contains(&"Birch"));
    assert!(!names.contains(&"Jordan"));
    assert!(!names.contains(&"Riley"));
    assert!(!names.contains(&"Amos"));
}

#[test]
fn session_room_contains_captain_and_mate() {
    let topo = test_topology();
    let members = topo.session_room_members(&RoomId("lane-1".into())).unwrap();
    let names: Vec<&str> = members.iter().map(|p| p.name.as_str()).collect();

    assert_eq!(names, vec!["Cedar", "Jordan"]);
}

#[test]
fn session_room_not_found() {
    let topo = test_topology();
    assert!(topo.session_room_members(&RoomId("nope".into())).is_none());
}

// ── Allowed mentions ──────────────────────────────────────────────────

#[test]
fn mate_can_only_mention_captain() {
    let topo = test_topology();
    let jordan = topo.find_participant("Jordan").unwrap();
    let allowed = allowed_mentions(&topo, jordan);

    assert_eq!(allowed, vec!["Cedar"]);
}

#[test]
fn captain_can_mention_mate_and_human() {
    let topo = test_topology();
    let cedar = topo.find_participant("Cedar").unwrap();
    let allowed = allowed_mentions(&topo, cedar);

    assert!(allowed.contains(&"Jordan".to_string()));
    assert!(allowed.contains(&"Amos".to_string()));
    assert!(!allowed.contains(&"Morgan".to_string()));
    assert!(!allowed.contains(&"Birch".to_string()));
}

#[test]
fn admiral_can_mention_all_captains() {
    let topo = test_topology();
    let morgan = topo.find_participant("Morgan").unwrap();
    let allowed = allowed_mentions(&topo, morgan);

    assert!(allowed.contains(&"Cedar".to_string()));
    assert!(allowed.contains(&"Birch".to_string()));
    assert!(!allowed.contains(&"Jordan".to_string()));
    assert!(!allowed.contains(&"Amos".to_string()));
}

#[test]
fn human_can_mention_admiral_and_captains() {
    let topo = test_topology();
    let amos = topo.find_participant("Amos").unwrap();
    let allowed = allowed_mentions(&topo, amos);

    assert!(allowed.contains(&"Morgan".to_string()));
    assert!(allowed.contains(&"Cedar".to_string()));
    assert!(allowed.contains(&"Birch".to_string()));
    assert!(!allowed.contains(&"Jordan".to_string()));
}

// ── Prompt snapshots ─────────────────────────────────────────────────

#[test]
fn snapshot_captain_prompt() {
    let prompt = CaptainPrompt {
        captain_name: "Cedar".into(),
        mate_name: "Jordan".into(),
        human_name: "Amos".into(),
        admiral_name: Some("Morgan".into()),
        state_summary: "New session, no active task. Greet the human and wait for direction.".into(),
    };
    insta::assert_snapshot!("captain_prompt", prompt.render_once().unwrap());
}

#[test]
fn snapshot_captain_prompt_no_admiral() {
    let prompt = CaptainPrompt {
        captain_name: "Cedar".into(),
        mate_name: "Jordan".into(),
        human_name: "Amos".into(),
        admiral_name: None,
        state_summary: "New session, no active task. Greet the human and wait for direction.".into(),
    };
    insta::assert_snapshot!("captain_prompt_no_admiral", prompt.render_once().unwrap());
}

#[test]
fn snapshot_mate_prompt() {
    let prompt = MatePrompt {
        mate_name: "Jordan".into(),
        captain_name: "Cedar".into(),
        human_name: "Amos".into(),
        task_description: "Refactor the auth middleware to use the new session store.".into(),
    };
    insta::assert_snapshot!("mate_prompt", prompt.render_once().unwrap());
}

#[test]
fn snapshot_admiral_prompt() {
    let prompt = AdmiralPrompt {
        admiral_name: "Morgan".into(),
        human_name: "Amos".into(),
        lanes: vec![
            LaneInfo {
                captain_name: "Cedar".into(),
                label: "auth-refactor".into(),
                status_summary: "working, step 3/5".into(),
            },
            LaneInfo {
                captain_name: "Birch".into(),
                label: "logging-migration".into(),
                status_summary: "idle, finished step 1".into(),
            },
        ],
    };
    insta::assert_snapshot!("admiral_prompt", prompt.render_once().unwrap());
}

// ── Message wrapping snapshots ───────────────────────────────────────

#[test]
fn snapshot_wrap_mate_to_captain() {
    let wrapped = wrap_message(
        "Jordan",
        "I've completed the refactor and all tests pass.",
        &captain_routing_hint("Jordan", "Amos"),
    );
    insta::assert_snapshot!("wrap_mate_to_captain", wrapped);
}

#[test]
fn snapshot_wrap_captain_steer_to_mate() {
    let wrapped = wrap_message(
        "Cedar",
        "Focus on the error handling first, the UI can wait.",
        &mate_routing_hint(),
    );
    insta::assert_snapshot!("wrap_captain_steer_to_mate", wrapped);
}

#[test]
fn snapshot_bounce_for_captain() {
    let topo = test_topology();
    let bounce = bounce_for(&topo, "Cedar").unwrap();
    insta::assert_snapshot!("bounce_captain", bounce);
}

#[test]
fn snapshot_bounce_for_mate() {
    let topo = test_topology();
    let bounce = bounce_for(&topo, "Jordan").unwrap();
    insta::assert_snapshot!("bounce_mate", bounce);
}

// ── Topology edge cases ─────────────────────────────────────────────

#[test]
fn empty_topology_no_sessions() {
    let topo = Topology {
        human: Participant::human("Amos"),
        admiral: Participant::agent("Morgan", AgentRole::Admiral),
        sessions: vec![],
    };
    // Admiral room has just the admiral (no captains).
    let members = topo.admiral_room_members();
    assert_eq!(members.len(), 1);
    assert_eq!(members[0].name, "Morgan");

    // Human mentions admiral — delivers.
    let action = Action::MessageSent {
        from: "Amos".into(),
        mention: "Morgan".into(),
        text: "hello".into(),
    };
    let deliveries = route(&action, &topo);
    assert_eq!(deliveries.len(), 1);
    assert!(matches!(&deliveries[0].content, DeliveryContent::Message { .. }));

    // Human has no captains to mention.
    let amos = topo.find_participant("Amos").unwrap();
    let allowed = allowed_mentions(&topo, amos);
    assert_eq!(allowed, vec!["Morgan"]);
}

#[test]
fn find_participant_ci_works() {
    let topo = test_topology();
    assert!(topo.find_participant_ci("cedar").is_some());
    assert!(topo.find_participant_ci("CEDAR").is_some());
    assert!(topo.find_participant_ci("CeDaR").is_some());
    assert!(topo.find_participant_ci("nobody").is_none());
}

#[test]
fn session_for_participant_finds_by_captain_or_mate() {
    let topo = test_topology();
    let session = topo.session_for_participant("Cedar").unwrap();
    assert_eq!(session.id, RoomId("lane-1".into()));

    let session = topo.session_for_participant("Jordan").unwrap();
    assert_eq!(session.id, RoomId("lane-1".into()));

    assert!(topo.session_for_participant("Morgan").is_none());
    assert!(topo.session_for_participant("Amos").is_none());
}

// ── Names edge cases ────────────────────────────────────────────────

#[test]
fn pick_more_than_available() {
    let all = name_pool();
    // Ask for more names than exist — should return all available.
    let picked = pick_names(all.len() + 100, &[]);
    assert_eq!(picked.len(), all.len());
}

#[test]
fn pick_with_all_taken() {
    let all: Vec<&str> = name_pool().to_vec();
    let picked = pick_names(5, &all);
    assert!(picked.is_empty());
}

#[test]
fn pick_zero_names() {
    let picked = pick_names(0, &[]);
    assert!(picked.is_empty());
}

// ── Transition edge cases ───────────────────────────────────────────

#[test]
fn self_transitions_not_allowed() {
    let phases = [
        TaskPhase::Assigned,
        TaskPhase::Working,
        TaskPhase::PendingReview,
        TaskPhase::RebaseConflict,
    ];
    for phase in phases {
        assert!(
            !can_transition(phase, phase),
            "{phase:?} should not self-transition"
        );
    }
}

#[test]
fn working_cannot_skip_to_accepted() {
    assert!(!can_transition(TaskPhase::Working, TaskPhase::Accepted));
}

#[test]
fn assigned_cannot_go_to_review_pending() {
    assert!(!can_transition(TaskPhase::Assigned, TaskPhase::PendingReview));
}

#[test]
fn rebase_conflict_cannot_go_to_working() {
    assert!(!can_transition(TaskPhase::RebaseConflict, TaskPhase::Working));
}

#[test]
fn terminal_is_terminal() {
    assert!(TaskPhase::Accepted.is_terminal());
    assert!(TaskPhase::Cancelled.is_terminal());
    assert!(!TaskPhase::Working.is_terminal());
    assert!(!TaskPhase::Assigned.is_terminal());
}

// ── Sandbox failure conditions ──────────────────────────────────────

#[test]
fn captain_read_only_at_every_non_rebase_phase() {
    let phases = [
        None,
        Some(TaskPhase::Assigned),
        Some(TaskPhase::Working),
        Some(TaskPhase::PendingReview),
        Some(TaskPhase::Accepted),
        Some(TaskPhase::Cancelled),
    ];
    for phase in phases {
        let policy = code_policy(AgentRole::Captain, phase);
        assert!(
            is_op_allowed(&policy, OpKind::Read),
            "Captain should always have Read at {phase:?}"
        );
        assert!(
            !is_op_allowed(&policy, OpKind::Edit),
            "Captain should NOT have Edit at {phase:?}"
        );
        assert!(
            !is_op_allowed(&policy, OpKind::Write),
            "Captain should NOT have Write at {phase:?}"
        );
        assert!(
            !is_op_allowed(&policy, OpKind::Submit),
            "Captain should NEVER have Submit at {phase:?}"
        );
    }
}

#[test]
fn captain_rebase_is_the_only_writable_phase() {
    let phases = [
        None,
        Some(TaskPhase::Assigned),
        Some(TaskPhase::Working),
        Some(TaskPhase::PendingReview),
        Some(TaskPhase::Accepted),
        Some(TaskPhase::Cancelled),
    ];
    for phase in phases {
        let policy = run_policy(AgentRole::Captain, phase);
        assert!(
            !policy.worktree_writable,
            "Captain worktree should be read-only at {phase:?}"
        );
    }
    // Only rebase conflict is writable.
    let policy = run_policy(AgentRole::Captain, Some(TaskPhase::RebaseConflict));
    assert!(policy.worktree_writable);
}

#[test]
fn mate_only_writable_when_working() {
    let read_only_phases = [
        None,
        Some(TaskPhase::Assigned),
        Some(TaskPhase::PendingReview),
        Some(TaskPhase::Accepted),
        Some(TaskPhase::Cancelled),
        Some(TaskPhase::RebaseConflict),
    ];
    for phase in read_only_phases {
        let policy = run_policy(AgentRole::Mate, phase);
        assert!(
            !policy.worktree_writable,
            "Mate worktree should be read-only at {phase:?}"
        );
    }
}

#[test]
fn mate_no_ops_at_terminal_phases() {
    for phase in [TaskPhase::Accepted, TaskPhase::Cancelled] {
        let policy = code_policy(AgentRole::Mate, Some(phase));
        assert!(
            policy.allowed_ops.is_empty(),
            "Mate should have no ops at {phase:?}"
        );
    }
}

#[test]
fn mate_no_ops_at_non_work_phases() {
    for phase in [
        TaskPhase::PendingReview,
        TaskPhase::RebaseConflict,
    ] {
        let policy = code_policy(AgentRole::Mate, Some(phase));
        assert!(
            policy.allowed_ops.is_empty(),
            "Mate should have no ops at {phase:?}"
        );
    }
}

// ── Op denied reason edge cases ─────────────────────────────────────

#[test]
fn op_denied_submit_for_captain() {
    let reason = op_denied_reason(AgentRole::Captain, Some(TaskPhase::Working), OpKind::Submit);
    assert!(reason.contains("Only the mate"));
}

#[test]
fn op_denied_submit_for_admiral() {
    let reason = op_denied_reason(AgentRole::Admiral, None, OpKind::Submit);
    assert!(reason.contains("Only the mate"));
}

#[test]
fn op_denied_read_for_mate_no_task() {
    let reason = op_denied_reason(AgentRole::Mate, None, OpKind::Read);
    assert!(reason.contains("no active task"));
}

// ── Help failure conditions ─────────────────────────────────────────

#[test]
fn captain_accepted_can_assign_again() {
    let actions = available_actions(AgentRole::Captain, Some(TaskPhase::Accepted));
    let names: Vec<&str> = actions.iter().map(|a| a.name).collect();
    assert!(names.contains(&"captain_assign"));
    assert!(!names.contains(&"captain_merge"));
}

#[test]
fn captain_cancelled_can_assign_again() {
    let actions = available_actions(AgentRole::Captain, Some(TaskPhase::Cancelled));
    let names: Vec<&str> = actions.iter().map(|a| a.name).collect();
    assert!(names.contains(&"captain_assign"));
}

#[test]
fn mate_assigned_cannot_submit() {
    let actions = available_actions(AgentRole::Mate, Some(TaskPhase::Assigned));
    let names: Vec<&str> = actions.iter().map(|a| a.name).collect();
    assert!(names.contains(&"code"));
    assert!(names.contains(&"mate_ask_captain"));
    assert!(!names.contains(&"mate_submit"));
}

#[test]
fn tool_help_for_nonexistent_tool() {
    let help = tool_help(AgentRole::Captain, None, "nonexistent_tool");
    assert!(help.is_none());
}

#[test]
fn wrong_tool_for_every_role() {
    // Mate using captain tools.
    let msg = wrong_tool_help(AgentRole::Mate, Some(TaskPhase::Working), "captain_assign");
    assert!(msg.contains("Unknown tool"));

    // Captain using mate tools.
    let msg = wrong_tool_help(AgentRole::Captain, Some(TaskPhase::PendingReview), "mate_submit");
    assert!(msg.contains("Unknown tool"));
}

// ── Bounce failure conditions ───────────────────────────────────────

#[test]
fn bounce_for_unknown_participant() {
    let topo = test_topology();
    assert!(bounce_for(&topo, "Ghost").is_none());
}

#[test]
fn bounce_for_human_lists_admiral_and_captains() {
    let topo = test_topology();
    // Humans get a bounce too — they need to @mention someone.
    let bounce = bounce_for(&topo, "Amos").unwrap();
    assert!(bounce.contains("Morgan"));
    assert!(bounce.contains("Cedar"));
    assert!(bounce.contains("Birch"));
}

#[test]
fn bounce_for_admiral() {
    let topo = test_topology();
    let bounce = bounce_for(&topo, "Morgan");
    // Admiral should get a bounce message listing captains.
    assert!(bounce.is_some());
}

// ── Delivery routing ─────────────────────────────────────────────────

#[test]
fn delivery_mate_message_to_captain() {
    let topo = test_topology();
    let action = Action::MessageSent {
        from: "Jordan".into(),
        mention: "Cedar".into(),
        text: "work is done".into(),
    };
    let deliveries = route(&action, &topo);
    assert_eq!(deliveries.len(), 1);
    assert_eq!(deliveries[0].to, "Cedar");
    assert_eq!(deliveries[0].from, "Jordan");
    assert!(matches!(
        &deliveries[0].content,
        DeliveryContent::Message { text } if text == "work is done"
    ));
}

#[test]
fn delivery_captain_to_mate_is_message() {
    let topo = test_topology();
    let action = Action::MessageSent {
        from: "Cedar".into(),
        mention: "Jordan".into(),
        text: "fix the tests".into(),
    };
    let deliveries = route(&action, &topo);
    assert_eq!(deliveries.len(), 1);
    assert_eq!(deliveries[0].to, "Jordan");
    assert!(matches!(
        &deliveries[0].content,
        DeliveryContent::Message { text } if text == "fix the tests"
    ));
}

#[test]
fn delivery_captain_human_intercepted_to_admiral() {
    let topo = test_topology();
    let action = Action::MessageSent {
        from: "Cedar".into(),
        mention: "Amos".into(),
        text: "task is done".into(),
    };
    let deliveries = route(&action, &topo);
    assert_eq!(deliveries.len(), 1);
    assert_eq!(deliveries[0].to, "Morgan");
    assert_eq!(deliveries[0].from, "Cedar");
    assert!(matches!(&deliveries[0].content, DeliveryContent::Message { .. }));
}

#[test]
fn delivery_denied_bounces_to_sender() {
    let topo = test_topology();
    let action = Action::MessageSent {
        from: "Jordan".into(),
        mention: "Amos".into(),
        text: "hey human".into(),
    };
    let deliveries = route(&action, &topo);
    assert_eq!(deliveries.len(), 1);
    assert_eq!(deliveries[0].to, "Jordan");
    assert!(matches!(&deliveries[0].content, DeliveryContent::Denied { .. }));
}

#[test]
fn delivery_unknown_target_bounces() {
    let topo = test_topology();
    let action = Action::MessageSent {
        from: "Cedar".into(),
        mention: "Nobody".into(),
        text: "hello?".into(),
    };
    let deliveries = route(&action, &topo);
    assert_eq!(deliveries.len(), 1);
    assert_eq!(deliveries[0].to, "Cedar");
    assert!(matches!(&deliveries[0].content, DeliveryContent::Bounce { .. }));
}

#[test]
fn delivery_unaddressed_bounces() {
    let topo = test_topology();
    let action = Action::UnaddressedMessage {
        from: "Cedar".into(),
        text: "thinking out loud".into(),
    };
    let deliveries = route(&action, &topo);
    assert_eq!(deliveries.len(), 1);
    assert_eq!(deliveries[0].to, "Cedar");
    assert!(matches!(&deliveries[0].content, DeliveryContent::Bounce { .. }));
}

#[test]
fn delivery_mate_committed_notifies_captain_and_human() {
    let topo = test_topology();
    let action = Action::MateCommitted {
        session: RoomId("lane-1".into()),
        step_description: Some("Add error handling".into()),
        commit_summary: "feat: add error handling".into(),
        diff_section: "+42 -3".into(),
    };
    let deliveries = route(&action, &topo);
    assert_eq!(deliveries.len(), 2);

    // Captain gets it
    assert_eq!(deliveries[0].to, "Cedar");
    assert_eq!(deliveries[0].from, "Jordan");
    assert!(matches!(
        &deliveries[0].content,
        DeliveryContent::Committed { step: Some(s), .. } if s == "Add error handling"
    ));

    // Human gets it
    assert_eq!(deliveries[1].to, "Amos");
    assert_eq!(deliveries[1].urgency, Urgency::Informational);
}

#[test]
fn delivery_mate_submitted_notifies_captain_and_human() {
    let topo = test_topology();
    let action = Action::MateSubmitted {
        session: RoomId("lane-1".into()),
        summary: "Refactored auth middleware".into(),
    };
    let deliveries = route(&action, &topo);
    assert_eq!(deliveries.len(), 2);

    assert_eq!(deliveries[0].to, "Cedar");
    assert_eq!(deliveries[0].urgency, Urgency::Attention);
    assert!(matches!(&deliveries[0].content, DeliveryContent::Submitted { .. }));

    assert_eq!(deliveries[1].to, "Amos");
    assert_eq!(deliveries[1].urgency, Urgency::Informational);
}

#[test]
fn delivery_mate_plan_set_notifies_captain() {
    let topo = test_topology();
    let action = Action::MatePlanSet {
        session: RoomId("lane-1".into()),
        plan_status: "3 steps, starting with tests".into(),
    };
    let deliveries = route(&action, &topo);
    assert_eq!(deliveries.len(), 1);
    assert_eq!(deliveries[0].to, "Cedar");
    assert!(matches!(&deliveries[0].content, DeliveryContent::PlanSet { .. }));
}

#[test]
fn delivery_mate_question_notifies_captain_with_attention() {
    let topo = test_topology();
    let action = Action::MateQuestion {
        session: RoomId("lane-1".into()),
        question: "Should I use async here?".into(),
    };
    let deliveries = route(&action, &topo);
    assert_eq!(deliveries.len(), 1);
    assert_eq!(deliveries[0].to, "Cedar");
    assert_eq!(deliveries[0].urgency, Urgency::Attention);
    assert!(matches!(&deliveries[0].content, DeliveryContent::Question { .. }));
}

#[test]
fn delivery_activity_summary_from_summarizer() {
    let topo = test_topology();
    let action = Action::MateActivitySummary {
        session: RoomId("lane-1".into()),
        summary: "Mate edited 3 files".into(),
    };
    let deliveries = route(&action, &topo);
    assert_eq!(deliveries.len(), 1);
    assert_eq!(deliveries[0].to, "Cedar");
    assert_eq!(deliveries[0].from, "summarizer");
    assert!(matches!(&deliveries[0].content, DeliveryContent::ActivitySummary { .. }));
}

#[test]
fn delivery_forced_submit_nudges_mate() {
    let topo = test_topology();
    let action = Action::MateForcedSubmit {
        session: RoomId("lane-1".into()),
    };
    let deliveries = route(&action, &topo);
    assert_eq!(deliveries.len(), 1);
    assert_eq!(deliveries[0].to, "Jordan");
    assert!(matches!(&deliveries[0].content, DeliveryContent::Guidance { .. }));
}

#[test]
fn delivery_task_assigned_notifies_human() {
    let topo = test_topology();
    let action = Action::TaskAssigned {
        session: RoomId("lane-1".into()),
        title: "Fix auth bug".into(),
        description: "The login flow is broken".into(),
    };
    let deliveries = route(&action, &topo);
    assert_eq!(deliveries.len(), 1);
    assert_eq!(deliveries[0].to, "Amos");
    assert!(matches!(&deliveries[0].content, DeliveryContent::TaskAssigned { .. }));
}

#[test]
fn delivery_checks_finished_failed_notifies_human() {
    let topo = test_topology();
    let action = Action::ChecksFinished {
        session: RoomId("lane-1".into()),
        context: "pre-merge".into(),
        all_passed: false,
        summary: "2 hooks failed".into(),
    };
    let deliveries = route(&action, &topo);
    assert_eq!(deliveries.len(), 2);

    // Captain gets attention
    let captain_d = deliveries.iter().find(|d| d.to == "Cedar").unwrap();
    assert_eq!(captain_d.urgency, Urgency::Attention);

    // Human gets notification channel (not just feed)
    let human_d = deliveries.iter().find(|d| d.to == "Amos").unwrap();
    assert_eq!(human_d.channel, Channel::Notification);
    assert_eq!(human_d.urgency, Urgency::Attention);
}

#[test]
fn delivery_checks_finished_passed_is_informational() {
    let topo = test_topology();
    let action = Action::ChecksFinished {
        session: RoomId("lane-1".into()),
        context: "post-commit".into(),
        all_passed: true,
        summary: "all green".into(),
    };
    let deliveries = route(&action, &topo);
    let human_d = deliveries.iter().find(|d| d.to == "Amos").unwrap();
    assert_eq!(human_d.channel, Channel::Feed);
    assert_eq!(human_d.urgency, Urgency::Informational);
}

#[test]
fn delivery_unknown_session_returns_empty() {
    let topo = test_topology();
    let action = Action::MateCommitted {
        session: RoomId("nonexistent".into()),
        step_description: None,
        commit_summary: "oops".into(),
        diff_section: "".into(),
    };
    assert!(route(&action, &topo).is_empty());
}

#[test]
fn delivery_both_captains_human_intercepted() {
    let topo = test_topology();
    for (captain, admiral) in [("Cedar", "Morgan"), ("Birch", "Morgan")] {
        let action = Action::MessageSent {
            from: captain.into(),
            mention: "Amos".into(),
            text: "status update".into(),
        };
        let deliveries = route(&action, &topo);
        assert_eq!(deliveries.len(), 1);
        assert_eq!(deliveries[0].to, admiral);
    }
}

#[test]
fn delivery_lane2_commit_goes_to_lane2_captain() {
    let topo = test_topology();
    let action = Action::MateCommitted {
        session: RoomId("lane-2".into()),
        step_description: None,
        commit_summary: "fix: typo".into(),
        diff_section: "+1 -1".into(),
    };
    let deliveries = route(&action, &topo);
    assert_eq!(deliveries[0].to, "Birch");
    assert_eq!(deliveries[0].from, "Riley");
}
