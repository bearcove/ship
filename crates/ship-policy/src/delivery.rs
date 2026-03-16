use crate::{AgentRole, ParticipantKind, RoomId, SessionRoom, Topology, allowed_mentions};
use std::fmt::Write as _;

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

// ── Urgent tag parsing ───────────────────────────────────────────────

/// Parse `#urgent` from message text. Returns the cleaned text (tag stripped)
/// and whether the tag was present. Case-insensitive, strips leading/trailing
/// whitespace left behind by removal.
fn parse_urgent_tag(text: &str) -> (String, bool) {
    // Case-insensitive search for #urgent as a standalone token
    let lower = text.to_lowercase();
    if let Some(pos) = lower.find("#urgent") {
        // Check it's a standalone token (not part of a longer word)
        let before_ok = pos == 0 || text.as_bytes()[pos - 1].is_ascii_whitespace();
        let end = pos + "#urgent".len();
        let after_ok = end == text.len() || text.as_bytes()[end].is_ascii_whitespace();

        if before_ok && after_ok {
            let mut clean = String::with_capacity(text.len());
            clean.push_str(text[..pos].trim_end());
            let rest = text[end..].trim_start();
            if !clean.is_empty() && !rest.is_empty() {
                clean.push(' ');
            }
            clean.push_str(rest);
            return (clean, true);
        }
    }
    (text.to_owned(), false)
}

/// Check if a message contains `#urgent` and return the cleaned text + urgency.
/// Public so prompt templates or other policy code can use the same parsing.
pub fn extract_urgency(text: &str) -> (String, Urgency) {
    let (clean, is_urgent) = parse_urgent_tag(text);
    let urgency = if is_urgent {
        Urgency::Attention
    } else {
        Urgency::Informational
    };
    (clean, urgency)
}

// ── Prompt rendering ─────────────────────────────────────────────────

/// Render a delivery for injection into an agent's prompt.
///
/// Human recipients consume typed `DeliveryContent` directly via the frontend —
/// this function is only for producing the XML-wrapped text that agents see.
///
/// `mention_hints` are `(name, role_label)` pairs the recipient can address,
/// e.g. `[("Jordan", "mate"), ("Amos", "human")]`.
pub fn render_for_prompt(delivery: &Delivery, mention_hints: &[(&str, &str)]) -> String {
    match &delivery.content {
        DeliveryContent::Message { text } => {
            wrap_message(&delivery.from, text, mention_hints)
        }

        DeliveryContent::Committed {
            step,
            commit_summary,
            diff_section,
        } => {
            let body = match step {
                Some(desc) => format!("Completed step: {desc}\n\n{commit_summary}{diff_section}"),
                None => format!("Committed:\n\n{commit_summary}{diff_section}"),
            };
            wrap_message(&delivery.from, &body, mention_hints)
        }

        DeliveryContent::Submitted { summary } => {
            let body = format!("I've submitted my work for review: {summary}");
            wrap_message(&delivery.from, &body, mention_hints)
        }

        DeliveryContent::PlanSet { plan_status } => {
            let body = format!(
                "I've set my plan.\n\n{plan_status}\n\nI'll keep you posted as I progress."
            );
            wrap_message(&delivery.from, &body, mention_hints)
        }

        DeliveryContent::Question { text } => {
            wrap_message(&delivery.from, text, mention_hints)
        }

        DeliveryContent::ActivitySummary { summary } => {
            // Summarizer is a system process, not a participant — attributed separately
            wrap_message(&delivery.from, summary, mention_hints)
        }

        DeliveryContent::Bounce { reason, allowed } => {
            let allowed_str = allowed
                .iter()
                .map(|n| format!("@{n}"))
                .collect::<Vec<_>>()
                .join(", ");
            format!("<routing>{reason} You can mention: {allowed_str}</routing>")
        }

        DeliveryContent::Denied {
            attempted_target,
            reason,
        } => {
            format!("<routing>Cannot address @{attempted_target}: {reason}</routing>")
        }

        DeliveryContent::Guidance { text } => {
            format!("<routing>{text}</routing>")
        }

        DeliveryContent::TaskAssigned { title, description } => {
            let body = format!("Task assigned: {title}\n\n{description}");
            wrap_message(&delivery.from, &body, mention_hints)
        }

        DeliveryContent::ChecksStarted { context } => {
            let body = format!("Checks started: {context}");
            wrap_message("system", &body, mention_hints)
        }

        DeliveryContent::ChecksFinished {
            context,
            all_passed,
            summary,
        } => {
            let status = if *all_passed { "passed" } else { "FAILED" };
            let body = format!("Checks {status} ({context}): {summary}");
            wrap_message("system", &body, mention_hints)
        }
    }
}

/// Wrap content in a `<message>` tag with routing hints for agent prompt injection.
fn wrap_message(from: &str, body: &str, mention_hints: &[(&str, &str)]) -> String {
    let mut out = String::new();
    let _ = write!(out, "<message from=\"{from}\">\n{body}\n</message>");
    let routing = format_routing_hint(mention_hints);
    if !routing.is_empty() {
        out.push('\n');
        out.push_str(&routing);
    }
    out
}

/// Format routing hints into a `<routing>` tag.
fn format_routing_hint(mention_hints: &[(&str, &str)]) -> String {
    if mention_hints.is_empty() {
        return String::new();
    }
    let mut parts = String::new();
    for (i, (name, label)) in mention_hints.iter().enumerate() {
        if i > 0 {
            parts.push_str(" · ");
        }
        let _ = write!(parts, "Reply to {label}: @{name}");
    }
    format!("<routing>{parts}</routing>")
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

    // Parse #urgent tag from message text
    let (clean_text, is_urgent) = parse_urgent_tag(text);
    let urgency = if is_urgent {
        Urgency::Attention
    } else {
        Urgency::Informational
    };

    // Interception: captain says @human → goes to admiral instead
    if matches!(sender.kind, ParticipantKind::Agent(AgentRole::Captain)) && target.is_human() {
        return vec![Delivery {
            to: topology.admiral.name.clone(),
            from: from.to_owned(),
            content: DeliveryContent::Message {
                text: clean_text,
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
            text: clean_text,
        },
        channel: Channel::Feed,
        urgency,
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

    let deliveries = vec![
        // Captain gets notified of commit
        Delivery {
            to: room.captain.name.clone(),
            from: room.mate.name.clone(),
            content: DeliveryContent::Committed {
                step: step_description.map(String::from),
                commit_summary: commit_summary.to_owned(),
                diff_section: diff_section.to_owned(),
            },
            channel: Channel::Feed,
            urgency: Urgency::Informational,
        },
        // Human sees it in their activity feed
        Delivery {
            to: topology.human.name.clone(),
            from: room.mate.name.clone(),
            content: DeliveryContent::Committed {
                step: step_description.map(String::from),
                commit_summary: commit_summary.to_owned(),
                diff_section: diff_section.to_owned(),
            },
            channel: Channel::Feed,
            urgency: Urgency::Informational,
        },
    ];

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
