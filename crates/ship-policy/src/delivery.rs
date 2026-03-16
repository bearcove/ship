use crate::{AgentRole, ParticipantKind, RoomId, SessionRoom, Topology, allowed_mentions};

// ── Actions: everything that can happen in the system ────────────────

/// Something that happened in the system. Input to `route()`.
#[derive(Debug, Clone)]
pub enum Action {
    /// A participant sent a message mentioning another participant.
    MessageSent {
        from: String,
        mention: String,
        text: String,
    },

    /// A participant sent a message without addressing anyone.
    UnaddressedMessage { from: String, text: String },

    /// Mate made a commit, optionally completing a plan step.
    MateCommitted {
        session: RoomId,
        step_description: Option<String>,
        commit_summary: String,
        diff_section: String,
    },

    /// Mate submitted work for review.
    MateSubmitted { session: RoomId, summary: String },

    /// Mate set or updated the plan.
    MatePlanSet {
        session: RoomId,
        plan_status: String,
    },

    /// Mate asked the captain a question.
    MateQuestion { session: RoomId, question: String },

    /// Summarizer produced an activity digest.
    MateActivitySummary { session: RoomId, summary: String },

    /// Mate stopped without submitting — needs a nudge.
    MateForcedSubmit { session: RoomId },

    /// Task was assigned to a session.
    TaskAssigned {
        session: RoomId,
        title: String,
        description: String,
    },

    /// CI checks started.
    ChecksStarted {
        session: RoomId,
        context: String,
    },

    /// CI checks finished.
    ChecksFinished {
        session: RoomId,
        context: String,
        all_passed: bool,
        summary: String,
    },
}

// ── Deliveries: what routing produces ────────────────────────────────

/// A typed delivery to a specific participant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Delivery {
    /// Recipient participant name.
    pub to: String,
    /// Sender participant name (or "system" for system-generated deliveries).
    pub from: String,
    /// Typed content of the delivery.
    pub content: DeliveryContent,
    /// Where the delivery appears for the recipient.
    pub channel: Channel,
    /// How urgently the recipient should see this.
    pub urgency: Urgency,
}

/// What's being delivered. Typed so the frontend renders what it's told
/// instead of re-deriving presentation from raw events.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeliveryContent {
    /// A direct message from one participant to another.
    Message { text: String },

    /// Captain steering the mate (correction/direction).
    Steer { text: String },

    /// Mate committed code.
    Committed {
        step: Option<String>,
        commit_summary: String,
        diff_section: String,
    },

    /// Mate submitted work for review.
    Submitted { summary: String },

    /// Mate set or updated the plan.
    PlanSet { plan_status: String },

    /// Question from one participant to another.
    Question { text: String },

    /// Activity summary from the summarizer.
    ActivitySummary { summary: String },

    /// Bounce: message had no addressee or addressed unknown target.
    Bounce { reason: String, allowed: Vec<String> },

    /// Access denied: sender cannot address target.
    Denied {
        attempted_target: String,
        reason: String,
    },

    /// System guidance (forced submit nudge, etc.).
    Guidance { text: String },

    /// Task was assigned.
    TaskAssigned { title: String, description: String },

    /// CI checks started.
    ChecksStarted { context: String },

    /// CI checks finished.
    ChecksFinished {
        context: String,
        all_passed: bool,
        summary: String,
    },
}

/// Where a delivery appears for the recipient.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Channel {
    /// The participant's primary feed (agent prompt / human activity log).
    Feed,
    /// A notification that demands attention (human only).
    Notification,
    /// A blocking prompt that needs a response before continuing (human only).
    Blocking,
}

/// How urgently the recipient should see this.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Urgency {
    /// Background information, no interruption needed.
    Informational,
    /// Demands attention soon — may interrupt current activity.
    Attention,
    /// Blocks progress until addressed.
    Blocking,
}

// ── The unified routing function ─────────────────────────────────────

/// Route an action through the topology, producing typed deliveries.
///
/// This is the single routing table for the entire system. Every event
/// enters here; the output is a list of deliveries to specific participants
/// with typed content, channel, and urgency.
pub fn route(action: &Action, topology: &Topology) -> Vec<Delivery> {
    match action {
        Action::MessageSent {
            from,
            mention,
            text,
        } => route_message_sent(topology, from, mention, text),

        Action::UnaddressedMessage { from, text } => route_unaddressed(topology, from, text),

        Action::MateCommitted {
            session,
            step_description,
            commit_summary,
            diff_section,
        } => route_mate_committed(topology, session, step_description.as_deref(), commit_summary, diff_section),

        Action::MateSubmitted { session, summary } => route_mate_submitted(topology, session, summary),

        Action::MatePlanSet {
            session,
            plan_status,
        } => route_mate_plan_set(topology, session, plan_status),

        Action::MateQuestion { session, question } => route_mate_question(topology, session, question),

        Action::MateActivitySummary { session, summary } => {
            route_mate_activity_summary(topology, session, summary)
        }

        Action::MateForcedSubmit { session } => route_mate_forced_submit(topology, session),

        Action::TaskAssigned {
            session,
            title,
            description,
        } => route_task_assigned(topology, session, title, description),

        Action::ChecksStarted { session, context } => {
            route_checks_started(topology, session, context)
        }

        Action::ChecksFinished {
            session,
            context,
            all_passed,
            summary,
        } => route_checks_finished(topology, session, context, *all_passed, summary),
    }
}

// ── Message routing (subsumes old route_message) ─────────────────────

fn route_message_sent(
    topology: &Topology,
    from: &str,
    mention: &str,
    text: &str,
) -> Vec<Delivery> {
    // Find the sender
    let sender = match topology.find_participant(from) {
        Some(p) => p,
        None => {
            // Unknown sender — nowhere to deliver an error
            return vec![];
        }
    };

    // Find the target
    let target = match topology.find_participant(mention) {
        Some(p) => p,
        None => {
            return vec![Delivery {
                to: from.to_owned(),
                from: "system".to_owned(),
                content: DeliveryContent::Bounce {
                    reason: format!("Unknown participant: {mention}"),
                    allowed: allowed_mentions(topology, sender),
                },
                channel: Channel::Feed,
                urgency: Urgency::Attention,
            }];
        }
    };

    // Check access control
    let allowed = allowed_mentions(topology, sender);
    if !allowed.iter().any(|n| n == mention) {
        return vec![Delivery {
            to: from.to_owned(),
            from: "system".to_owned(),
            content: DeliveryContent::Denied {
                attempted_target: mention.to_owned(),
                reason: format!(
                    "{} ({:?}) cannot mention {}",
                    sender.name, sender.kind, target.name
                ),
            },
            channel: Channel::Feed,
            urgency: Urgency::Attention,
        }];
    }

    // Interception: captain says @human → goes to admiral instead
    if matches!(sender.kind, ParticipantKind::Agent(AgentRole::Captain)) && target.is_human() {
        return vec![Delivery {
            to: topology.admiral.name.clone(),
            from: from.to_owned(),
            content: DeliveryContent::Message {
                text: text.to_owned(),
            },
            channel: Channel::Feed,
            urgency: Urgency::Attention,
        }];
    }

    // Captain → Mate is a steer
    if matches!(sender.kind, ParticipantKind::Agent(AgentRole::Captain))
        && matches!(target.kind, ParticipantKind::Agent(AgentRole::Mate))
    {
        return vec![Delivery {
            to: mention.to_owned(),
            from: from.to_owned(),
            content: DeliveryContent::Steer {
                text: text.to_owned(),
            },
            channel: Channel::Feed,
            urgency: Urgency::Attention,
        }];
    }

    // Normal delivery
    vec![Delivery {
        to: mention.to_owned(),
        from: from.to_owned(),
        content: DeliveryContent::Message {
            text: text.to_owned(),
        },
        channel: Channel::Feed,
        urgency: Urgency::Informational,
    }]
}

fn route_unaddressed(topology: &Topology, from: &str, _text: &str) -> Vec<Delivery> {
    let sender = match topology.find_participant(from) {
        Some(p) => p,
        None => return vec![],
    };

    vec![Delivery {
        to: from.to_owned(),
        from: "system".to_owned(),
        content: DeliveryContent::Bounce {
            reason: "Message didn't address anyone.".to_owned(),
            allowed: allowed_mentions(topology, sender),
        },
        channel: Channel::Feed,
        urgency: Urgency::Attention,
    }]
}

// ── Session event routing ────────────────────────────────────────────

fn find_session<'a>(topology: &'a Topology, session: &RoomId) -> Option<&'a SessionRoom> {
    topology.sessions.iter().find(|s| s.id == *session)
}

fn route_mate_committed(
    topology: &Topology,
    session: &RoomId,
    step_description: Option<&str>,
    commit_summary: &str,
    diff_section: &str,
) -> Vec<Delivery> {
    let room = match find_session(topology, session) {
        Some(r) => r,
        None => return vec![],
    };

    let mut deliveries = vec![];

    // Captain gets notified of commit
    deliveries.push(Delivery {
        to: room.captain.name.clone(),
        from: room.mate.name.clone(),
        content: DeliveryContent::Committed {
            step: step_description.map(String::from),
            commit_summary: commit_summary.to_owned(),
            diff_section: diff_section.to_owned(),
        },
        channel: Channel::Feed,
        urgency: Urgency::Informational,
    });

    // Human sees it in their activity feed
    deliveries.push(Delivery {
        to: topology.human.name.clone(),
        from: room.mate.name.clone(),
        content: DeliveryContent::Committed {
            step: step_description.map(String::from),
            commit_summary: commit_summary.to_owned(),
            diff_section: diff_section.to_owned(),
        },
        channel: Channel::Feed,
        urgency: Urgency::Informational,
    });

    deliveries
}

fn route_mate_submitted(
    topology: &Topology,
    session: &RoomId,
    summary: &str,
) -> Vec<Delivery> {
    let room = match find_session(topology, session) {
        Some(r) => r,
        None => return vec![],
    };

    vec![
        // Captain gets the submission
        Delivery {
            to: room.captain.name.clone(),
            from: room.mate.name.clone(),
            content: DeliveryContent::Submitted {
                summary: summary.to_owned(),
            },
            channel: Channel::Feed,
            urgency: Urgency::Attention,
        },
        // Human sees it
        Delivery {
            to: topology.human.name.clone(),
            from: room.mate.name.clone(),
            content: DeliveryContent::Submitted {
                summary: summary.to_owned(),
            },
            channel: Channel::Feed,
            urgency: Urgency::Informational,
        },
    ]
}

fn route_mate_plan_set(
    topology: &Topology,
    session: &RoomId,
    plan_status: &str,
) -> Vec<Delivery> {
    let room = match find_session(topology, session) {
        Some(r) => r,
        None => return vec![],
    };

    vec![Delivery {
        to: room.captain.name.clone(),
        from: room.mate.name.clone(),
        content: DeliveryContent::PlanSet {
            plan_status: plan_status.to_owned(),
        },
        channel: Channel::Feed,
        urgency: Urgency::Informational,
    }]
}

fn route_mate_question(
    topology: &Topology,
    session: &RoomId,
    question: &str,
) -> Vec<Delivery> {
    let room = match find_session(topology, session) {
        Some(r) => r,
        None => return vec![],
    };

    vec![Delivery {
        to: room.captain.name.clone(),
        from: room.mate.name.clone(),
        content: DeliveryContent::Question {
            text: question.to_owned(),
        },
        channel: Channel::Feed,
        urgency: Urgency::Attention,
    }]
}

fn route_mate_activity_summary(
    topology: &Topology,
    session: &RoomId,
    summary: &str,
) -> Vec<Delivery> {
    let room = match find_session(topology, session) {
        Some(r) => r,
        None => return vec![],
    };

    vec![Delivery {
        to: room.captain.name.clone(),
        from: "summarizer".to_owned(),
        content: DeliveryContent::ActivitySummary {
            summary: summary.to_owned(),
        },
        channel: Channel::Feed,
        urgency: Urgency::Informational,
    }]
}

fn route_mate_forced_submit(topology: &Topology, session: &RoomId) -> Vec<Delivery> {
    let room = match find_session(topology, session) {
        Some(r) => r,
        None => return vec![],
    };

    vec![Delivery {
        to: room.mate.name.clone(),
        from: "system".to_owned(),
        content: DeliveryContent::Guidance {
            text: "You stopped without submitting. Call mate_submit with a summary of what you accomplished.".to_owned(),
        },
        channel: Channel::Feed,
        urgency: Urgency::Attention,
    }]
}

fn route_task_assigned(
    topology: &Topology,
    session: &RoomId,
    title: &str,
    description: &str,
) -> Vec<Delivery> {
    let room = match find_session(topology, session) {
        Some(r) => r,
        None => return vec![],
    };

    vec![
        // Human sees the assignment in their feed
        Delivery {
            to: topology.human.name.clone(),
            from: room.captain.name.clone(),
            content: DeliveryContent::TaskAssigned {
                title: title.to_owned(),
                description: description.to_owned(),
            },
            channel: Channel::Feed,
            urgency: Urgency::Informational,
        },
    ]
}

fn route_checks_started(
    topology: &Topology,
    session: &RoomId,
    context: &str,
) -> Vec<Delivery> {
    let room = match find_session(topology, session) {
        Some(r) => r,
        None => return vec![],
    };

    vec![
        // Human sees checks started
        Delivery {
            to: topology.human.name.clone(),
            from: "system".to_owned(),
            content: DeliveryContent::ChecksStarted {
                context: context.to_owned(),
            },
            channel: Channel::Feed,
            urgency: Urgency::Informational,
        },
        // Captain sees checks started
        Delivery {
            to: room.captain.name.clone(),
            from: "system".to_owned(),
            content: DeliveryContent::ChecksStarted {
                context: context.to_owned(),
            },
            channel: Channel::Feed,
            urgency: Urgency::Informational,
        },
    ]
}

fn route_checks_finished(
    topology: &Topology,
    session: &RoomId,
    context: &str,
    all_passed: bool,
    summary: &str,
) -> Vec<Delivery> {
    let room = match find_session(topology, session) {
        Some(r) => r,
        None => return vec![],
    };

    let urgency = if all_passed {
        Urgency::Informational
    } else {
        Urgency::Attention
    };

    let human_channel = if all_passed {
        Channel::Feed
    } else {
        Channel::Notification
    };

    vec![
        // Captain sees results (always needs to know)
        Delivery {
            to: room.captain.name.clone(),
            from: "system".to_owned(),
            content: DeliveryContent::ChecksFinished {
                context: context.to_owned(),
                all_passed,
                summary: summary.to_owned(),
            },
            channel: Channel::Feed,
            urgency: Urgency::Attention,
        },
        // Human sees results
        Delivery {
            to: topology.human.name.clone(),
            from: "system".to_owned(),
            content: DeliveryContent::ChecksFinished {
                context: context.to_owned(),
                all_passed,
                summary: summary.to_owned(),
            },
            channel: human_channel,
            urgency,
        },
    ]
}
