use crate::*;

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
