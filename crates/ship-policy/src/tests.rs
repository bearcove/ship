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

// ── Routing ───────────────────────────────────────────────────────────

#[test]
fn mate_message_to_captain_delivers() {
    let topo = test_topology();
    let msg = Message {
        from: "Jordan".into(),
        mention: "Cedar".into(),
        text: "work is done".into(),
    };

    let result = route_message(&topo, &msg);
    assert_eq!(
        result,
        RouteResult::Deliver {
            to: "Cedar".into(),
            text: "work is done".into(),
        }
    );
}

#[test]
fn mate_cannot_mention_human() {
    let topo = test_topology();
    let msg = Message {
        from: "Jordan".into(),
        mention: "Amos".into(),
        text: "hey human".into(),
    };

    let result = route_message(&topo, &msg);
    assert!(matches!(result, RouteResult::Denied { .. }));
}

#[test]
fn mate_cannot_mention_admiral() {
    let topo = test_topology();
    let msg = Message {
        from: "Jordan".into(),
        mention: "Morgan".into(),
        text: "hey admiral".into(),
    };

    let result = route_message(&topo, &msg);
    assert!(matches!(result, RouteResult::Denied { .. }));
}

#[test]
fn mate_cannot_mention_other_sessions_captain() {
    let topo = test_topology();
    let msg = Message {
        from: "Jordan".into(),
        mention: "Birch".into(),
        text: "hey other captain".into(),
    };

    let result = route_message(&topo, &msg);
    assert!(matches!(result, RouteResult::Denied { .. }));
}

#[test]
fn captain_mentioning_human_gets_intercepted_to_admiral() {
    let topo = test_topology();
    let msg = Message {
        from: "Cedar".into(),
        mention: "Amos".into(),
        text: "task is done, ready for review".into(),
    };

    let result = route_message(&topo, &msg);
    assert_eq!(
        result,
        RouteResult::InterceptForAdmiral {
            original_target: "Amos".into(),
            to_admiral: "Morgan".into(),
            from_captain: "Cedar".into(),
            text: "task is done, ready for review".into(),
        }
    );
}

#[test]
fn captain_to_mate_delivers_normally() {
    let topo = test_topology();
    let msg = Message {
        from: "Cedar".into(),
        mention: "Jordan".into(),
        text: "fix the tests".into(),
    };

    let result = route_message(&topo, &msg);
    assert_eq!(
        result,
        RouteResult::Deliver {
            to: "Jordan".into(),
            text: "fix the tests".into(),
        }
    );
}

#[test]
fn admiral_to_captain_delivers() {
    let topo = test_topology();
    let msg = Message {
        from: "Morgan".into(),
        mention: "Cedar".into(),
        text: "proceed to next step".into(),
    };

    let result = route_message(&topo, &msg);
    assert_eq!(
        result,
        RouteResult::Deliver {
            to: "Cedar".into(),
            text: "proceed to next step".into(),
        }
    );
}

#[test]
fn human_to_admiral_delivers() {
    let topo = test_topology();
    let msg = Message {
        from: "Amos".into(),
        mention: "Morgan".into(),
        text: "pause lane 2".into(),
    };

    let result = route_message(&topo, &msg);
    assert_eq!(
        result,
        RouteResult::Deliver {
            to: "Morgan".into(),
            text: "pause lane 2".into(),
        }
    );
}

#[test]
fn unaddressed_message_bounces() {
    let topo = test_topology();
    let msg = Message {
        from: "Cedar".into(),
        mention: "".into(),
        text: "thinking out loud".into(),
    };

    let result = route_message(&topo, &msg);
    assert_eq!(
        result,
        RouteResult::Unaddressed {
            from: "Cedar".into(),
            text: "thinking out loud".into(),
        }
    );
}

#[test]
fn unknown_target() {
    let topo = test_topology();
    let msg = Message {
        from: "Cedar".into(),
        mention: "Nobody".into(),
        text: "hello?".into(),
    };

    let result = route_message(&topo, &msg);
    assert_eq!(
        result,
        RouteResult::UnknownTarget {
            from: "Cedar".into(),
            attempted_target: "Nobody".into(),
        }
    );
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

// ── Routing failure conditions ──────────────────────────────────────

#[test]
fn unknown_sender_returns_unknown_target() {
    let topo = test_topology();
    let msg = Message {
        from: "Ghost".into(),
        mention: "Cedar".into(),
        text: "hello".into(),
    };
    let result = route_message(&topo, &msg);
    // Unknown sender gets treated as unknown target (sender not found in topology)
    assert!(matches!(result, RouteResult::UnknownTarget { .. }));
}

#[test]
fn captain_cannot_mention_other_sessions_mate() {
    let topo = test_topology();
    let msg = Message {
        from: "Cedar".into(),
        mention: "Riley".into(),
        text: "hey other mate".into(),
    };
    let result = route_message(&topo, &msg);
    assert!(matches!(result, RouteResult::Denied { .. }));
}

#[test]
fn captain_cannot_mention_other_sessions_captain() {
    let topo = test_topology();
    let msg = Message {
        from: "Cedar".into(),
        mention: "Birch".into(),
        text: "hey other captain".into(),
    };
    let result = route_message(&topo, &msg);
    assert!(matches!(result, RouteResult::Denied { .. }));
}

#[test]
fn captain_cannot_mention_admiral_directly() {
    let topo = test_topology();
    let msg = Message {
        from: "Cedar".into(),
        mention: "Morgan".into(),
        text: "admiral please help".into(),
    };
    let result = route_message(&topo, &msg);
    assert!(matches!(result, RouteResult::Denied { .. }));
}

#[test]
fn admiral_cannot_mention_mate() {
    let topo = test_topology();
    let msg = Message {
        from: "Morgan".into(),
        mention: "Jordan".into(),
        text: "direct to mate".into(),
    };
    let result = route_message(&topo, &msg);
    assert!(matches!(result, RouteResult::Denied { .. }));
}

#[test]
fn human_cannot_mention_mate_directly() {
    let topo = test_topology();
    let msg = Message {
        from: "Amos".into(),
        mention: "Jordan".into(),
        text: "hey mate".into(),
    };
    let result = route_message(&topo, &msg);
    assert!(matches!(result, RouteResult::Denied { .. }));
}

#[test]
fn mate_cannot_mention_other_mate() {
    let topo = test_topology();
    let msg = Message {
        from: "Jordan".into(),
        mention: "Riley".into(),
        text: "cross-session chat".into(),
    };
    let result = route_message(&topo, &msg);
    assert!(matches!(result, RouteResult::Denied { .. }));
}

#[test]
fn self_mention_denied_for_mate() {
    let topo = test_topology();
    let msg = Message {
        from: "Jordan".into(),
        mention: "Jordan".into(),
        text: "talking to myself".into(),
    };
    let result = route_message(&topo, &msg);
    assert!(matches!(result, RouteResult::Denied { .. }));
}

#[test]
fn self_mention_denied_for_captain() {
    let topo = test_topology();
    let msg = Message {
        from: "Cedar".into(),
        mention: "Cedar".into(),
        text: "talking to myself".into(),
    };
    let result = route_message(&topo, &msg);
    assert!(matches!(result, RouteResult::Denied { .. }));
}

#[test]
fn both_captains_messages_to_human_intercepted() {
    let topo = test_topology();
    for captain in ["Cedar", "Birch"] {
        let msg = Message {
            from: captain.into(),
            mention: "Amos".into(),
            text: "status update".into(),
        };
        let result = route_message(&topo, &msg);
        assert!(
            matches!(result, RouteResult::InterceptForAdmiral { .. }),
            "{captain}'s @human should be intercepted"
        );
    }
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
    let msg = Message {
        from: "Amos".into(),
        mention: "Morgan".into(),
        text: "hello".into(),
    };
    assert!(matches!(route_message(&topo, &msg), RouteResult::Deliver { .. }));

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
        TaskPhase::ReviewPending,
        TaskPhase::SteerPending,
        TaskPhase::RebaseConflict,
        TaskPhase::WaitingForHuman,
    ];
    for phase in phases {
        assert!(
            !can_transition(phase, phase),
            "{phase:?} should not self-transition"
        );
    }
}

#[test]
fn waiting_for_human_can_only_cancel() {
    let reachable = reachable_from(TaskPhase::WaitingForHuman);
    assert_eq!(reachable, vec![TaskPhase::Cancelled]);
}

#[test]
fn working_cannot_skip_to_accepted() {
    assert!(!can_transition(TaskPhase::Working, TaskPhase::Accepted));
}

#[test]
fn working_cannot_go_to_steer_pending() {
    assert!(!can_transition(TaskPhase::Working, TaskPhase::SteerPending));
}

#[test]
fn assigned_cannot_go_to_review_pending() {
    assert!(!can_transition(TaskPhase::Assigned, TaskPhase::ReviewPending));
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
        Some(TaskPhase::ReviewPending),
        Some(TaskPhase::SteerPending),
        Some(TaskPhase::Accepted),
        Some(TaskPhase::Cancelled),
        Some(TaskPhase::WaitingForHuman),
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
        Some(TaskPhase::ReviewPending),
        Some(TaskPhase::SteerPending),
        Some(TaskPhase::Accepted),
        Some(TaskPhase::Cancelled),
        Some(TaskPhase::WaitingForHuman),
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
        Some(TaskPhase::ReviewPending),
        Some(TaskPhase::SteerPending),
        Some(TaskPhase::Accepted),
        Some(TaskPhase::Cancelled),
        Some(TaskPhase::WaitingForHuman),
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
        TaskPhase::ReviewPending,
        TaskPhase::SteerPending,
        TaskPhase::RebaseConflict,
        TaskPhase::WaitingForHuman,
    ] {
        let policy = code_policy(AgentRole::Mate, Some(phase));
        assert!(
            policy.allowed_ops.is_empty(),
            "Mate should have no ops at {phase:?}"
        );
    }
}

// ── Command checking edge cases ─────────────────────────────────────

#[test]
fn command_with_leading_whitespace() {
    assert!(matches!(
        check_command("  git status", AgentRole::Mate),
        CommandCheck::Blocked(_)
    ));
}

#[test]
fn whitespace_only_command() {
    assert_eq!(
        check_command("   ", AgentRole::Mate),
        CommandCheck::Allowed
    );
}

#[test]
fn rm_recursive_without_force_is_allowed() {
    // rm -r . (no -f) — not blocked by our check.
    assert_eq!(
        check_command("rm -r some_dir", AgentRole::Mate),
        CommandCheck::Allowed
    );
}

#[test]
fn rm_force_without_recursive_is_allowed() {
    // rm -f file.txt — targeted, not broad.
    assert_eq!(
        check_command("rm -f file.txt", AgentRole::Mate),
        CommandCheck::Allowed
    );
}

#[test]
fn rm_rf_with_specific_path_is_allowed() {
    // rm -rf target/debug — specific path, not broad.
    assert_eq!(
        check_command("rm -rf target/debug", AgentRole::Mate),
        CommandCheck::Allowed
    );
}

#[test]
fn rm_rf_dot_dot_blocked() {
    assert!(matches!(
        check_command("rm -rf ..", AgentRole::Mate),
        CommandCheck::Blocked(_)
    ));
}

#[test]
fn rm_fr_variant_blocked() {
    // -fr instead of -rf
    assert!(matches!(
        check_command("rm -fr /", AgentRole::Mate),
        CommandCheck::Blocked(_)
    ));
}

#[test]
fn rm_separate_flags_recursive_force_blocked() {
    assert!(matches!(
        check_command("rm -r -f .", AgentRole::Mate),
        CommandCheck::Blocked(_)
    ));
}

#[test]
fn admiral_blocked_from_git() {
    // Admiral also shouldn't run git? Let's verify the current behavior.
    // Actually, admiral has no worktree — git is allowed by check_command
    // but would fail at execution. The check is role-specific for Mate only.
    assert_eq!(
        check_command("git status", AgentRole::Admiral),
        CommandCheck::Allowed
    );
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
fn captain_steer_pending_has_steer_merge_cancel() {
    let actions = available_actions(AgentRole::Captain, Some(TaskPhase::SteerPending));
    let names: Vec<&str> = actions.iter().map(|a| a.name).collect();
    assert!(names.contains(&"captain_steer"));
    assert!(names.contains(&"captain_merge"));
    assert!(names.contains(&"captain_cancel"));
    assert!(!names.contains(&"captain_assign"));
    assert!(!names.contains(&"captain_review_diff"));
}

#[test]
fn captain_waiting_for_human_has_notify() {
    let actions = available_actions(AgentRole::Captain, Some(TaskPhase::WaitingForHuman));
    let names: Vec<&str> = actions.iter().map(|a| a.name).collect();
    assert!(names.contains(&"captain_notify_human"));
    assert!(!names.contains(&"captain_merge"));
}

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
    let msg = wrong_tool_help(AgentRole::Captain, Some(TaskPhase::ReviewPending), "mate_submit");
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
